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

mod craft;

use fourfour_analysis::decode::decode_mono;
use fourfour_analysis::service::{locate_models, Analyzer, PoolOptions};

const BIG_ALLOC: usize = 2 << 30;
const HANG_SECONDS: u64 = 30;
const VM_LIMIT_KB: u64 = 16 << 20;
const MUTATION_KINDS: [&str; 11] = ["intact", "truncate", "bitflips", "zero", "garbage", "header-len", "swap-ext", "cut-flip", "dup-chunk", "header-only", "delete"];
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
    label: String,
    bytes: Vec<u8>,
}

struct Case {
    id: usize,
    seed: usize,
    mutation: usize,
    kind: &'static str,
    base_format: &'static str,
    label: String,
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
    profile: String,
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
        "intact" => {}
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
    Case { id, seed, mutation, kind, base_format: base.format, label: base.label.clone(), ext, bytes }
}

fn ffmpeg(args: &[&str]) -> bool {
    Command::new("ffmpeg").args(["-y", "-loglevel", "error"]).args(args).status().map(|s| s.success()).unwrap_or(false)
}

fn files(dir: &Path, ext: &str, count: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
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
                found.push(p);
            }
        }
        if found.len() >= count {
            break;
        }
    }
    found.truncate(count);
    assert!(!found.is_empty(), "no .{ext} under {}", dir.display());
    found
}

fn codec_args(format: &str) -> Vec<&'static str> {
    match format {
        "mp3" => vec!["-c:a", "libmp3lame", "-b:a", "128k"],
        "m4a" => vec!["-c:a", "aac", "-b:a", "128k"],
        "ogg" => vec!["-c:a", "libvorbis"],
        _ => vec![],
    }
}

fn encode(work_dir: &Path, name: &str, format: &str, ext: &str, input: &[&str], shape: &[&str]) -> Option<Vec<u8>> {
    let out = work_dir.join(format!("{name}.{ext}"));
    if !out.exists() {
        let mut args: Vec<&str> = input.to_vec();
        args.extend_from_slice(shape);
        args.extend(["-metadata", "title=Fuzz Base", "-metadata", "artist=Fuzz Artist"]);
        args.extend(codec_args(format));
        args.push(out.to_str().unwrap());
        if !ffmpeg(&args) {
            let _ = std::fs::remove_file(&out);
            return None;
        }
    }
    std::fs::read(&out).ok()
}

fn datasets() -> PathBuf {
    Path::new(&std::env::var("HOME").unwrap()).join(".cache/ff-datasets")
}

fn build_bases(work: &Path, profile: &str) -> Vec<Base> {
    let dir = work.join(format!("bases-{profile}"));
    std::fs::create_dir_all(&dir).unwrap();
    let mp3s = files(&datasets().join("giantsteps-plus-key/audio"), "mp3", 8);
    let wavs = files(&datasets().join("fsl10k/audio"), "wav", 2);
    let mut bases = Vec::new();
    let mut push = |format: &'static str, label: String, bytes: Vec<u8>| bases.push(Base { format, label, bytes });
    match profile {
        "base" => {
            for (s, source) in [&mp3s[0], &wavs[0]].into_iter().enumerate() {
                for (format, ext) in FORMATS {
                    let input = ["-ss", "20", "-t", "6", "-i", source.to_str().unwrap()];
                    let bytes = encode(&dir, &format!("s{s}"), format, ext, &input, &["-ac", "2", "-ar", "44100"]).unwrap();
                    push(format, format!("{format}-s{s}"), bytes);
                }
            }
        }
        "hostile" => {
            let source = mp3s[0].to_str().unwrap();
            let input = ["-ss", "20", "-t", "6", "-i", source];
            let shape = ["-ac", "2", "-ar", "44100"];
            let get = |format: &str, ext: &str| encode(&dir, &format!("h-{format}"), format, ext, &input, &shape).unwrap();
            let hostile: [(&'static str, Vec<craft::Variant>); 6] = [
                ("mp3", craft::mp3_variants(&get("mp3", "mp3"))),
                ("flac", craft::flac_variants(&get("flac", "flac"))),
                ("wav", craft::wav_variants(&get("wav", "wav"))),
                ("aiff", craft::aiff_variants(&get("aiff", "aiff"))),
                ("m4a", craft::m4a_variants(&get("m4a", "m4a"))),
                ("ogg", craft::ogg_variants(&get("ogg", "ogg"))),
            ];
            for (format, variants) in hostile {
                for v in variants {
                    push(format, v.label, v.bytes);
                }
            }
        }
        "odd" => {
            let source = mp3s[1].to_str().unwrap();
            let input = ["-ss", "20", "-t", "30", "-i", source];
            for (format, ext) in FORMATS {
                for (rate, channels) in [("8000", "1"), ("192000", "2"), ("48000", "6"), ("44100", "1"), ("11025", "2"), ("96000", "1")] {
                    if let Some(bytes) = encode(&dir, &format!("odd-{rate}-{channels}"), format, ext, &input, &["-ac", channels, "-ar", rate]) {
                        push(format, format!("{format}-{rate}hz-{channels}ch"), bytes);
                    }
                }
            }
        }
        "long" => {
            let excerpt = dir.join("excerpt.wav");
            if !excerpt.exists() {
                assert!(ffmpeg(&["-ss", "20", "-t", "30", "-i", mp3s[2].to_str().unwrap(), "-ac", "2", "-ar", "44100", excerpt.to_str().unwrap()]));
            }
            let input = ["-stream_loop", "-1", "-i", excerpt.to_str().unwrap(), "-t", "900"];
            for (format, ext) in FORMATS {
                let shape: &[&str] = if format == "wav" || format == "aiff" { &["-ac", "1", "-ar", "16000"] } else { &["-ac", "2", "-ar", "44100"] };
                if let Some(bytes) = encode(&dir, "long", format, ext, &input, shape) {
                    push(format, format!("{format}-15min"), bytes);
                }
            }
        }
        "real" => {
            let sources: Vec<&PathBuf> = mp3s.iter().take(6).chain(wavs.iter()).collect();
            for (s, source) in sources.into_iter().enumerate() {
                for (format, ext) in FORMATS {
                    let input = ["-i", source.to_str().unwrap()];
                    if let Some(bytes) = encode(&dir, &format!("r{s}"), format, ext, &input, &["-ac", "2", "-ar", "44100"]) {
                        push(format, format!("{format}-real{s}"), bytes);
                    }
                }
            }
        }
        other => panic!("unknown profile {other}"),
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

fn run_stage(stages: &mut Vec<(u128, f64)>, name: &str, problems: &mut Vec<String>, work: &mut dyn FnMut(&mut Vec<String>)) {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let t = Instant::now();
    work(problems);
    let mb = PEAK.load(Ordering::Relaxed).saturating_sub(base) as f64 / 1048576.0;
    if mb >= (BIG_ALLOC >> 20) as f64 {
        problems.push(format!("ALLOC {name} peak {mb:.0} MB"));
    }
    stages.push((t.elapsed().as_millis(), mb));
}

fn child(settings: &Settings, start: usize, end: usize) {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC {info}");
        if let Ok(mut last) = LAST_PANIC.lock() {
            *last = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        }
    }));
    let bases = build_bases(&settings.work, &settings.profile);
    let analyzer = (settings.analyze_every > 0).then(|| {
        let models = locate_models(None).expect("model weights");
        Analyzer::load(&models, PoolOptions::default()).unwrap()
    });
    let library = pioneer_library::LocalLibrary::open_in_memory().unwrap();
    let dir = settings.work.join(format!("case-{}-{start}", settings.profile));
    std::fs::create_dir_all(&dir).unwrap();
    let out = std::io::stdout();
    for id in start..end {
        let case = mutate(&bases, settings.muts, id);
        let path = dir.join(format!("case.{}", case.ext));
        std::fs::write(&path, &case.bytes).unwrap();
        writeln!(out.lock(), "BEGIN {id}").unwrap();
        out.lock().flush().unwrap();
        let started = Instant::now();
        let mut problems = Vec::new();
        let mut clean_errors = 0;
        let mut stages = Vec::new();
        let mut decoded = None;
        run_stage(&mut stages, "decode", &mut problems, &mut |problems| decoded = guard("decode", problems, || decode_mono(&path)));
        match &decoded {
            Some(Err(_)) => clean_errors += 1,
            Some(Ok(m)) if m.samples.is_empty() || m.rate == 0 || m.gapless.end > m.samples.len() || m.gapless.start > m.gapless.end => {
                problems.push(format!("BAD decode Ok with {} samples at {} Hz gapless {:?}", m.samples.len(), m.rate, m.gapless))
            }
            _ => {}
        }
        let mut tags = None;
        run_stage(&mut stages, "tags", &mut problems, &mut |problems| tags = guard("tags", problems, || pioneer_usb_writer::scanner::read_track_metadata(&path, 1)));
        match tags {
            Some(Ok(track)) => run_stage(&mut stages, "import", &mut problems, &mut |problems| {
                guard("import", problems, || library.add_track(&track));
            }),
            Some(Err(_)) => {
                clean_errors += 1;
                stages.push((0, 0.0));
            }
            None => stages.push((0, 0.0)),
        }
        guard("status", &mut problems, || pioneer_library::file_status(&path));
        let mut analysis_mb = 0.0;
        if let (Some(analyzer), true, Some(Ok(_))) = (&analyzer, id % settings.analyze_every.max(1) == 0, &decoded) {
            let before = stages.len();
            run_stage(&mut stages, "analyze", &mut problems, &mut |problems| {
                if let Some(Err(_)) = guard("analyze", problems, || analyzer.analyze(&path)) {
                    clean_errors += 1;
                }
            });
            analysis_mb = stages[before].1;
        }
        drop(decoded);
        let peak_mb = stages.iter().map(|s| s.1).fold(analysis_mb, f64::max);
        let rss_mb = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmHWM")).and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())).unwrap_or(0.0) / 1024.0;
        let stage_text: Vec<String> = stages.iter().take(3).map(|(ms, mb)| format!("{ms} {mb:.0}")).collect();
        let outcome = if problems.is_empty() { if clean_errors > 0 { "err" } else { "ok" } } else { "FAIL" };
        let detail = problems.join(" | ").replace(['\t', '\n'], " ");
        writeln!(out.lock(), "END {id} {outcome} {} {peak_mb:.0} {} {rss_mb:.0} {detail}", started.elapsed().as_millis(), stage_text.join(" ")).unwrap();
        out.lock().flush().unwrap();
        let _ = std::fs::remove_file(&path);
    }
}

struct Row {
    id: usize,
    outcome: String,
    ms: u128,
    peak_mb: String,
    stages: Vec<f64>,
    detail: String,
}

fn run_shard(settings: &Settings, start: usize, end: usize) -> Vec<Row> {
    let exe = std::env::current_exe().unwrap();
    let mut rows = Vec::new();
    let mut next = start;
    let log = settings.work.join(format!("child-{}-{start}.log", settings.profile));
    while next < end {
        let stderr = std::fs::OpenOptions::new().create(true).append(true).open(&log).unwrap();
        let script = format!("ulimit -v {VM_LIMIT_KB}; exec nice -n 10 \"$0\" child --seeds {} --muts {} --analyze-every {} --profile {} --work {} --range {next} {end}", settings.seeds, settings.muts, settings.analyze_every, settings.profile, settings.work.display());
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
                    let mut parts = line.splitn(13, ' ');
                    match (parts.next(), parts.next().and_then(|v| v.parse::<usize>().ok())) {
                        (Some("BEGIN"), Some(id)) => current = Some(id),
                        (Some("END"), Some(id)) => {
                            let outcome = parts.next().unwrap_or("").to_string();
                            let ms = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                            let peak_mb = parts.next().unwrap_or("").to_string();
                            let numbers: Vec<f64> = (0..7).map(|_| parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0)).collect();
                            let detail = parts.next().unwrap_or("").to_string();
                            rows.push(Row { id, outcome, ms, peak_mb, stages: numbers, detail });
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
                rows.push(Row { id, outcome: kind.into(), ms: if kind == "HANG" { HANG_SECONDS as u128 * 1000 } else { 0 }, peak_mb: String::new(), stages: vec![0.0; 7], detail });
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
    let bases = build_bases(&settings.work, &settings.profile);
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
    let fails_dir = settings.work.join(format!("fails-{}", settings.profile));
    std::fs::create_dir_all(&fails_dir).unwrap();
    let mut tsv = String::from("id\tseed\tmut\tkind\tbase\text\toutcome\tms\tpeak_mb\tdecode_ms\tdecode_mb\ttags_ms\ttags_mb\timport_ms\timport_mb\trss_mb\tdetail\n");
    let mut by_kind: BTreeMap<(String, &str), usize> = BTreeMap::new();
    let mut by_signature: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for row in &rows {
        let case = mutate(&bases, settings.muts, row.id);
        tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", row.id, case.seed, case.mutation, case.kind, case.label, case.ext, row.outcome, row.ms, row.peak_mb, row.stages.iter().map(|v| format!("{v:.0}")).collect::<Vec<_>>().join("\t"), row.detail));
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
    let max_of = |i: usize| rows.iter().map(|r| r.stages[i]).fold(0.0, f64::max);
    println!("max decode {:.0} ms {:.0} MB | tags {:.0} ms {:.0} MB | import {:.0} ms {:.0} MB | process peak RSS {:.0} MB | slowest case {} ms", max_of(0), max_of(1), max_of(2), max_of(3), max_of(4), max_of(5), max_of(6), rows.iter().map(|r| r.ms).max().unwrap_or(0));
    println!("tags >= 256 MB: {} | tags or import >= 5 s: {} | case >= 10 s: {}", rows.iter().filter(|r| r.stages[3] >= 256.0).count(), rows.iter().filter(|r| r.stages[2] >= 5000.0 || r.stages[4] >= 5000.0).count(), rows.iter().filter(|r| r.ms >= 10_000).count());
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
        profile: flag("--profile", "base"),
    };
    if args.first().map(String::as_str) == Some("dump") {
        let bases = build_bases(&settings.work, &settings.profile);
        let case = mutate(&bases, settings.muts, args[1].parse().unwrap());
        let out = Path::new(&args[2]).join(format!("case.{}", case.ext));
        std::fs::write(&out, &case.bytes).unwrap();
        println!("{} {} {}", out.display(), case.label, case.kind);
    } else if args.first().map(String::as_str) == Some("child") {
        let at = args.iter().position(|a| a == "--range").unwrap();
        child(&settings, args[at + 1].parse().unwrap(), args[at + 2].parse().unwrap());
    } else {
        driver(settings);
    }
}
