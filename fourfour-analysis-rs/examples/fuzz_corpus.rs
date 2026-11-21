use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use fourfour_analysis::decode::decode_mono;
use fourfour_analysis::service::{locate_models, Analyzer, PoolOptions};

const BIG_ALLOC: usize = 2 << 30;
const HANG_SECONDS: u64 = 30;
const VM_LIMIT_KB: u64 = 16 << 20;
const MUTATION_KINDS: [&str; 10] = ["truncate", "bitflips", "zero", "garbage", "header-len", "swap-ext", "cut-flip", "dup-chunk", "header-only", "delete"];
const FORMATS: [(&str, &str); 6] = [("mp3", "mp3"), ("flac", "flac"), ("wav", "wav"), ("m4a", "m4a"), ("aiff", "aiff"), ("ogg", "ogg")];

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

struct Counting;

fn note_alloc(size: usize) {
    let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() >= BIG_ALLOC {
            eprintln!("BIGALLOC {} bytes", layout.size());
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            note_alloc(layout.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.size() >= BIG_ALLOC {
            eprintln!("BIGALLOC {} bytes", layout.size());
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            note_alloc(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size >= BIG_ALLOC {
            eprintln!("BIGALLOC {new_size} bytes");
            return std::ptr::null_mut();
        }
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            note_alloc(new_size);
        }
        p
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

struct Base {
    format: &'static str,
    bytes: Vec<u8>,
}

struct Case {
    id: usize,
    seed: usize,
    mutation: usize,
    kind: &'static str,
    base_format: &'static str,
    ext: &'static str,
    bytes: Vec<u8>,
}

struct Settings {
    seeds: usize,
    muts: usize,
    analyze_every: usize,
    work: PathBuf,
    out: PathBuf,
    shards: usize,
}

fn mutate(bases: &[Base], settings_muts: usize, id: usize) -> Case {
    let seed = id / settings_muts;
    let mutation = id % settings_muts;
    let mut rng = Rng((seed as u64) << 20 | mutation as u64);
    let base = &bases[seed % bases.len()];
    let mut bytes = base.bytes.clone();
    let kind = MUTATION_KINDS[mutation % MUTATION_KINDS.len()];
    let mut ext = FORMATS.iter().find(|(f, _)| *f == base.format).unwrap().1;
    let len = bytes.len();
    match kind {
        "truncate" => bytes.truncate(rng.below(len)),
        "bitflips" => {
            for _ in 0..1 + rng.below(64) {
                let i = rng.below(len);
                bytes[i] ^= 1 << rng.below(8);
            }
        }
        "zero" => {
            let start = rng.below(len);
            let end = (start + 1 + rng.below(len / 4 + 1)).min(len);
            bytes[start..end].fill(0);
        }
        "garbage" => {
            let start = rng.below(len);
            let end = (start + 1 + rng.below(4096)).min(len);
            for b in &mut bytes[start..end] {
                *b = rng.next() as u8;
            }
        }
        "header-len" => {
            let window = if rng.below(2) == 0 { 128.min(len) } else { len };
            for _ in 0..1 + rng.below(3) {
                if window < 4 {
                    break;
                }
                let at = rng.below(window - 3);
                let value: u32 = [0xFFFF_FFFF, 0, 0x7FFF_FFFF, 0x8000_0000, rng.next() as u32, 1][rng.below(6)];
                let be = rng.below(2) == 0;
                let raw = if be { value.to_be_bytes() } else { value.to_le_bytes() };
                bytes[at..at + 4].copy_from_slice(&raw);
            }
        }
        "swap-ext" => {
            let others: Vec<&'static str> = FORMATS.iter().map(|(_, e)| *e).filter(|e| *e != ext).collect();
            ext = others[rng.below(others.len())];
        }
        "cut-flip" => {
            bytes.truncate(1 + rng.below(len));
            for _ in 0..1 + rng.below(16) {
                let i = rng.below(bytes.len());
                bytes[i] ^= 1 << rng.below(8);
            }
        }
        "dup-chunk" => {
            let start = rng.below(len);
            let end = (start + 1 + rng.below(8192)).min(len);
            let chunk = bytes[start..end].to_vec();
            let at = rng.below(len);
            bytes.splice(at..at, chunk);
        }
        "header-only" => bytes.truncate(rng.below(512.min(len) + 1)),
        _ => {
            let start = rng.below(len);
            let end = (start + 1 + rng.below(len / 3 + 1)).min(len);
            bytes.drain(start..end);
        }
    }
    Case { id, seed, mutation, kind, base_format: base.format, ext, bytes }
}

fn ffmpeg(args: &[&str]) {
    let status = Command::new("ffmpeg").args(["-y", "-loglevel", "error"]).args(args).status().expect("ffmpeg");
    assert!(status.success(), "ffmpeg {args:?}");
}

fn first_file(dir: &Path, ext: &str) -> PathBuf {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.components().any(|c| c.as_os_str() == "__MACOSX") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == ext) {
                return p;
            }
        }
    }
    panic!("no .{ext} under {}", dir.display());
}

fn build_bases(work: &Path) -> Vec<Base> {
    let dir = work.join("bases");
    std::fs::create_dir_all(&dir).unwrap();
    let datasets = Path::new(&std::env::var("HOME").unwrap()).join(".cache/ff-datasets");
    let sources = [first_file(&datasets.join("giantsteps-plus-key/audio"), "mp3"), first_file(&datasets.join("fsl10k/audio"), "wav")];
    let mut bases = Vec::new();
    for (s, source) in sources.iter().enumerate() {
        for (format, ext) in FORMATS {
            let out = dir.join(format!("s{s}.{ext}"));
            if !out.exists() {
                let mut args = vec!["-ss", "20", "-t", "6", "-i", source.to_str().unwrap(), "-ac", "2", "-ar", "44100", "-metadata", "title=Fuzz Base", "-metadata", "artist=Fuzz Artist"];
                match format {
                    "mp3" => args.extend(["-c:a", "libmp3lame", "-b:a", "128k"]),
                    "m4a" => args.extend(["-c:a", "aac", "-b:a", "128k"]),
                    "ogg" => args.extend(["-c:a", "libvorbis"]),
                    _ => {}
                }
                args.push(out.to_str().unwrap());
                ffmpeg(&args);
            }
            bases.push(Base { format, bytes: std::fs::read(&out).unwrap() });
        }
    }
    bases
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    let text = payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
    let place = LAST_PANIC.lock().map(|l| l.clone()).unwrap_or_default();
    format!("{text} @ {place}")
}

fn guard<T>(stage: &str, problems: &mut Vec<String>, work: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(v) => Some(v),
        Err(p) => {
            problems.push(format!("PANIC {stage}: {}", panic_message(p)));
            None
        }
    }
}

fn child(settings: &Settings, start: usize, end: usize) {
    std::panic::set_hook(Box::new(|info| {
        if let Ok(mut last) = LAST_PANIC.lock() {
            *last = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        }
    }));
    let bases = build_bases(&settings.work);
    let analyzer = (settings.analyze_every > 0).then(|| {
        let models = locate_models(None).expect("model weights");
        Analyzer::load(&models, PoolOptions::default()).unwrap()
    });
    let library = pioneer_library::LocalLibrary::open_in_memory().unwrap();
    let dir = settings.work.join(format!("case-{start}"));
    std::fs::create_dir_all(&dir).unwrap();
    let out = std::io::stdout();
    for id in start..end {
        let case = mutate(&bases, settings.muts, id);
        let path = dir.join(format!("case.{}", case.ext));
        std::fs::write(&path, &case.bytes).unwrap();
        writeln!(out.lock(), "BEGIN {id}").unwrap();
        out.lock().flush().unwrap();
        let baseline = LIVE.load(Ordering::Relaxed);
        PEAK.store(baseline, Ordering::Relaxed);
        let started = Instant::now();
        let mut problems = Vec::new();
        let mut clean_errors = 0;
        let decoded = guard("decode", &mut problems, || decode_mono(&path));
        match &decoded {
            Some(Err(_)) => clean_errors += 1,
            Some(Ok(m)) if m.samples.is_empty() || m.rate == 0 || m.gapless.end > m.samples.len() || m.gapless.start > m.gapless.end => {
                problems.push(format!("BAD decode Ok with {} samples at {} Hz gapless {:?}", m.samples.len(), m.rate, m.gapless))
            }
            _ => {}
        }
        match guard("tags", &mut problems, || pioneer_usb_writer::scanner::read_track_metadata(&path, 1)) {
            Some(Ok(track)) => {
                guard("import", &mut problems, || library.add_track(&track));
            }
            Some(Err(_)) => clean_errors += 1,
            None => {}
        }
        guard("status", &mut problems, || pioneer_library::file_status(&path));
        if let (Some(analyzer), true, Some(Ok(_))) = (&analyzer, id % settings.analyze_every.max(1) == 0, &decoded) {
            if let Some(Err(_)) = guard("analyze", &mut problems, || analyzer.analyze(&path)) {
                clean_errors += 1;
            }
        }
        drop(decoded);
        let peak_mb = PEAK.load(Ordering::Relaxed).saturating_sub(baseline) as f64 / 1048576.0;
        if peak_mb >= (BIG_ALLOC >> 20) as f64 {
            problems.push(format!("ALLOC peak {peak_mb:.0} MB"));
        }
        let outcome = if problems.is_empty() { if clean_errors > 0 { "err" } else { "ok" } } else { "FAIL" };
        let detail = problems.join(" | ").replace(['\t', '\n'], " ");
        writeln!(out.lock(), "END {id} {outcome} {} {peak_mb:.0} {detail}", started.elapsed().as_millis()).unwrap();
        out.lock().flush().unwrap();
        let _ = std::fs::remove_file(&path);
    }
}

struct Row {
    id: usize,
    outcome: String,
    ms: u128,
    peak_mb: String,
    detail: String,
}

fn run_shard(settings: &Settings, start: usize, end: usize) -> Vec<Row> {
    let exe = std::env::current_exe().unwrap();
    let mut rows = Vec::new();
    let mut next = start;
    let log = settings.work.join(format!("child-{start}.log"));
    while next < end {
        let stderr = std::fs::OpenOptions::new().create(true).append(true).open(&log).unwrap();
        let script = format!("ulimit -v {VM_LIMIT_KB}; exec nice -n 10 \"$0\" child --seeds {} --muts {} --analyze-every {} --work {} --range {next} {end}", settings.seeds, settings.muts, settings.analyze_every, settings.work.display());
        let mut proc = Command::new("sh").arg("-c").arg(script).arg(&exe).stdout(Stdio::piped()).stderr(stderr).spawn().unwrap();
        let (tx, rx) = mpsc::channel();
        let stdout = proc.stdout.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut current: Option<usize> = None;
        let mut trouble: Option<(usize, &str)> = None;
        loop {
            match rx.recv_timeout(Duration::from_secs(HANG_SECONDS)) {
                Ok(line) => {
                    let mut parts = line.splitn(6, ' ');
                    match (parts.next(), parts.next().and_then(|v| v.parse::<usize>().ok())) {
                        (Some("BEGIN"), Some(id)) => current = Some(id),
                        (Some("END"), Some(id)) => {
                            let outcome = parts.next().unwrap_or("").to_string();
                            let ms = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                            let peak_mb = parts.next().unwrap_or("").to_string();
                            let detail = parts.next().unwrap_or("").to_string();
                            rows.push(Row { id, outcome, ms, peak_mb, detail });
                            current = None;
                            next = id + 1;
                        }
                        _ => {}
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = proc.kill();
                    trouble = current.map(|id| (id, "HANG"));
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    trouble = current.map(|id| (id, "CRASH"));
                    break;
                }
            }
        }
        let status = proc.wait().ok();
        match trouble {
            Some((id, kind)) => {
                let tail = std::fs::read_to_string(&log).unwrap_or_default().lines().rev().take(3).collect::<Vec<_>>().join(" / ");
                let detail = format!("{kind} {:?} {tail}", status).replace(['\t', '\n'], " ");
                rows.push(Row { id, outcome: kind.into(), ms: if kind == "HANG" { HANG_SECONDS as u128 * 1000 } else { 0 }, peak_mb: String::new(), detail });
                next = id + 1;
            }
            None if next < end && current.is_none() => {
                eprintln!("child exited early at {next}: {status:?}");
                break;
            }
            None => {}
        }
    }
    rows
}

fn signature(detail: &str) -> String {
    let d = detail.split(" | ").next().unwrap_or("");
    let d = d.split("(core").next().unwrap_or(d);
    let d: String = d.chars().take(110).collect();
    d.chars().filter(|c| !c.is_ascii_digit()).collect()
}

fn driver(settings: Settings) {
    std::fs::create_dir_all(&settings.work).unwrap();
    let bases = build_bases(&settings.work);
    let total = settings.seeds * settings.muts;
    let per = total.div_ceil(settings.shards);
    let settings = std::sync::Arc::new(settings);
    let started = Instant::now();
    let handles: Vec<_> = (0..settings.shards)
        .map(|s| {
            let settings = settings.clone();
            std::thread::spawn(move || run_shard(&settings, s * per, ((s + 1) * per).min(total)))
        })
        .collect();
    let mut rows: Vec<Row> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    rows.sort_by_key(|r| r.id);
    let fails_dir = settings.work.join("fails");
    std::fs::create_dir_all(&fails_dir).unwrap();
    let mut tsv = String::from("id\tseed\tmut\tkind\tbase\text\toutcome\tms\tpeak_mb\tdetail\n");
    let mut by_kind: BTreeMap<(String, &str), usize> = BTreeMap::new();
    let mut by_signature: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for row in &rows {
        let case = mutate(&bases, settings.muts, row.id);
        tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", row.id, case.seed, case.mutation, case.kind, case.base_format, case.ext, row.outcome, row.ms, row.peak_mb, row.detail));
        let kind = if row.outcome == "FAIL" {
            if row.detail.contains("PANIC") { "panic" } else if row.detail.contains("ALLOC") { "alloc" } else { "bad" }
        } else if row.outcome == "HANG" || row.outcome == "CRASH" {
            if row.detail.contains("BIGALLOC") || row.detail.contains("memory allocation") { "alloc" } else if row.outcome == "HANG" { "hang" } else { "crash" }
        } else {
            continue;
        };
        *by_kind.entry((case.base_format.to_string(), kind)).or_default() += 1;
        by_signature.entry(format!("{kind}: {}", signature(&row.detail))).or_insert((0, row.id)).0 += 1;
        std::fs::write(fails_dir.join(format!("{}-{}.{}", row.id, case.kind, case.ext)), &case.bytes).unwrap();
    }
    std::fs::write(&settings.out, tsv).unwrap();
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    for ((_, kind), n) in &by_kind {
        *totals.entry(kind).or_default() += n;
    }
    println!("cases run: {} of {} in {:.0}s", rows.len(), total, started.elapsed().as_secs_f64());
    println!("failures by kind: {totals:?}");
    println!("failures by format and kind: {by_kind:?}");
    let mut sigs: Vec<_> = by_signature.into_iter().collect();
    sigs.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    for (sig, (n, first)) in sigs {
        println!("{n:5}  first id {first}  {sig}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str, default: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or(default.to_string());
    let home = std::env::var("HOME").unwrap();
    let settings = Settings {
        seeds: flag("--seeds", "200").parse().unwrap(),
        muts: flag("--muts", "50").parse().unwrap(),
        analyze_every: flag("--analyze-every", "25").parse().unwrap(),
        work: PathBuf::from(flag("--work", &format!("{home}/.cache/hc-L5-work"))),
        out: PathBuf::from(flag("--out", &format!("{home}/.cache/hc-L5-work/results.tsv"))),
        shards: flag("--shards", "3").parse().unwrap(),
    };
    if args.first().map(String::as_str) == Some("child") {
        let at = args.iter().position(|a| a == "--range").unwrap();
        child(&settings, args[at + 1].parse().unwrap(), args[at + 2].parse().unwrap());
    } else {
        driver(settings);
    }
}
