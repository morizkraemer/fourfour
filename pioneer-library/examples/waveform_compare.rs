//! Compares fourfour's waveform sections with the ones Rekordbox wrote for the same audio.
//!
//! ```text
//! cargo run --release -p pioneer-library --example waveform_compare -- <command> [options]
//!
//! commands
//!   list                    show the selected tracks and their tune/test split
//!   score                   compare with the current tuning (options: --set tune|test|all, --tuning file.json)
//!   scan                    which filter chain, feature and release best correlate with Rekordbox's PWV7 bands
//!   fit-derive              fit the PWV7 -> PWV3/PWV5/PWAV/PWV2 derivation constants against Rekordbox's own PWV7
//!   fit                     coordinate-descent fit of the tuning on the tune half, then evaluate both halves
//!   hiband                  search the high band (filter, feature, release, gain, curve) for correlation and matching level distribution
//!   png <dir>               render ours vs Rekordbox for --png-count tracks of the test half
//!
//! options
//!   --n 40                  tracks to select (even positions tune, odd positions test)
//!   --library <dir>         folder with master.db and share/ (default benchmark/datasets/rekordbox-library)
//!   --audio <dir>           audio root (default benchmark/datasets/rekordbox-testdata-2025)
//!   --strip <prefix>        Rekordbox path prefix to strip
//!   --out tuning.json       where `fit` writes the fitted tuning
//! ```
//!
//! Read-only on the library. The band data is the pure `Waveforms` of
//! `pioneer_usb_writer::waveform`, the same bytes the export writes.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use pioneer_library::rekordbox;
use pioneer_usb_writer::waveform::{Derivation, Meter, Tuning, Waveforms, analyze};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const DEFAULT_LIBRARY: &str = "benchmark/datasets/rekordbox-library";
const DEFAULT_AUDIO: &str = "benchmark/datasets/rekordbox-testdata-2025";
const DEFAULT_STRIP: &str = "/Users/merlinkraemer/local/2-Areas/1-Mukke/";
const THREADS: usize = 4;

fn decode(path: &Path, mut sink: impl FnMut(&[f32], u32, usize)) -> Result<()> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| anyhow!("no audio track"))?;
    let id = track.id;
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio) => {
                let spec = *audio.spec();
                let channels = spec.channels.count();
                let needed = audio.capacity() * channels;
                if buf.as_ref().is_none_or(|b| b.capacity() < needed) {
                    buf = Some(SampleBuffer::new(audio.capacity() as u64, spec));
                }
                let b = buf.as_mut().unwrap();
                b.copy_interleaved_ref(audio);
                sink(b.samples(), spec.rate, channels);
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Mono {
    rate: u32,
    samples: Vec<f32>,
}

fn decode_mono(path: &Path) -> Result<Mono> {
    let mut samples = Vec::new();
    let mut rate = 0;
    decode(path, |chunk, r, channels| {
        rate = r;
        for frame in chunk.chunks_exact(channels.max(1)) {
            samples.push(frame.iter().sum::<f32>() / channels as f32);
        }
    })?;
    Ok(Mono { rate, samples })
}

fn measure(mono: &Mono, tuning: Tuning) -> Waveforms {
    let mut meter = Meter::with_tuning(mono.rate, 1, tuning);
    for chunk in mono.samples.chunks(1 << 16) {
        meter.push(chunk);
    }
    analyze(&meter.finish())
}

#[derive(Default, Clone)]
struct Reference {
    pwv3: Vec<u8>,
    pwv5: Vec<u16>,
    pwv7: Vec<[u8; 3]>,
    pwv6: Vec<[u8; 3]>,
    pwv4: Vec<[u8; 6]>,
    pwav: Vec<u8>,
    pwv2: Vec<u8>,
    gains: [u16; 3],
}

fn be32(b: &[u8], at: usize) -> usize {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize
}

fn sections(bytes: &[u8]) -> Vec<(String, &[u8])> {
    let mut out = Vec::new();
    let mut at = be32(bytes, 4);
    while at + 12 <= bytes.len() {
        let total = be32(bytes, at + 8);
        if total == 0 || at + total > bytes.len() {
            break;
        }
        out.push((String::from_utf8_lossy(&bytes[at..at + 4]).to_string(), &bytes[at..at + total]));
        at += total;
    }
    out
}

fn triples(data: &[u8]) -> Vec<[u8; 3]> {
    data.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

fn read_reference(dat: &Path) -> Result<Reference> {
    let mut r = Reference::default();
    for (ext, bytes) in [
        ("DAT", std::fs::read(dat)?),
        ("EXT", std::fs::read(dat.with_extension("EXT"))?),
        ("2EX", std::fs::read(dat.with_extension("2EX"))?),
    ] {
        for (tag, s) in sections(&bytes) {
            match (ext, tag.as_str()) {
                ("DAT", "PWAV") => r.pwav = s[20..].to_vec(),
                ("DAT", "PWV2") => r.pwv2 = s[20..].to_vec(),
                ("EXT", "PWV3") => r.pwv3 = s[24..].to_vec(),
                ("EXT", "PWV5") => {
                    r.pwv5 = s[24..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect()
                }
                ("EXT", "PWV4") => {
                    r.pwv4 = s[24..].chunks_exact(6).map(|c| c.try_into().unwrap()).collect()
                }
                ("2EX", "PWV7") => r.pwv7 = triples(&s[24..]),
                ("2EX", "PWV6") => r.pwv6 = triples(&s[20..]),
                ("2EX", "PWVC") => {
                    for j in 0..3 {
                        r.gains[j] = u16::from_be_bytes([s[14 + j * 2], s[15 + j * 2]]);
                    }
                }
                _ => {}
            }
        }
    }
    if r.pwv7.is_empty() || r.pwv3.is_empty() || r.pwav.len() != 400 {
        return Err(anyhow!("incomplete reference sections"));
    }
    Ok(r)
}

struct Entry {
    name: String,
    audio: PathBuf,
    dat: PathBuf,
    reference: Reference,
    tune: bool,
}

struct Selection {
    entries: Vec<Entry>,
}

struct Options {
    n: usize,
    library: PathBuf,
    audio: PathBuf,
    strip: String,
    set: String,
    tuning: Option<PathBuf>,
    derivation: Option<PathBuf>,
    out: PathBuf,
    png_count: usize,
}

fn select(o: &Options) -> Result<Selection> {
    let source = rekordbox::load_masterdb(&o.library.join("master.db"), Some(&o.library.join("share")))?;
    let mut candidates: Vec<(String, PathBuf, PathBuf, f64)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for t in &source.tracks {
        let Some(rel) = t.path.strip_prefix(&o.strip) else { continue };
        let audio = o.audio.join(rel);
        let Some(dat) = t.anlz_dat.clone() else { continue };
        if !audio.exists() || !dat.exists() || !dat.with_extension("2EX").exists() || !dat.with_extension("EXT").exists() {
            continue;
        }
        if !seen.insert(audio.clone()) {
            continue;
        }
        let secs = t.track.duration_secs;
        if !(90.0..=420.0).contains(&secs) {
            continue;
        }
        candidates.push((rel.to_string(), audio, dat, secs));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    let mut by_ext: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, c) in candidates.iter().enumerate() {
        let ext = Path::new(&c.0).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        by_ext.entry(ext).or_default().push(i);
    }
    let mut exts: Vec<_> = by_ext.keys().cloned().collect();
    exts.sort();
    let mut picked: Vec<usize> = Vec::new();
    let per_ext = o.n.div_ceil(exts.len().max(1));
    for e in &exts {
        let v = &by_ext[e];
        let step = (v.len() as f64 / per_ext as f64).max(1.0);
        let mut k = 0.0;
        while (k as usize) < v.len() && picked.iter().filter(|&&p| v.contains(&p)).count() < per_ext {
            picked.push(v[k as usize]);
            k += step;
        }
    }
    picked.sort();
    picked.truncate(o.n);
    let mut entries = Vec::new();
    for (pos, i) in picked.into_iter().enumerate() {
        let (name, audio, dat, _) = candidates[i].clone();
        let reference = match read_reference(&dat) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skip {name}: {e}");
                continue;
            }
        };
        entries.push(Entry { name, audio, dat, reference, tune: pos % 2 == 0 });
    }
    Ok(Selection { entries })
}

#[derive(Default, Clone, Copy)]
struct Acc {
    n: f64,
    abs: f64,
    exact: f64,
    w2: f64,
    sx: f64,
    sy: f64,
    sxx: f64,
    syy: f64,
    sxy: f64,
}

impl Acc {
    fn add(&mut self, ours: f64, theirs: f64) {
        let d = (ours - theirs).abs();
        self.n += 1.0;
        self.abs += d;
        self.exact += (d < 0.5) as u8 as f64;
        self.w2 += (d <= 2.0) as u8 as f64;
        self.sx += ours;
        self.sy += theirs;
        self.sxx += ours * ours;
        self.syy += theirs * theirs;
        self.sxy += ours * theirs;
    }

    fn merge(&mut self, o: &Acc) {
        self.n += o.n;
        self.abs += o.abs;
        self.exact += o.exact;
        self.w2 += o.w2;
        self.sx += o.sx;
        self.sy += o.sy;
        self.sxx += o.sxx;
        self.syy += o.syy;
        self.sxy += o.sxy;
    }

    fn mae(&self) -> f64 {
        self.abs / self.n.max(1.0)
    }

    fn corr(&self) -> f64 {
        let n = self.n.max(1.0);
        let cov = self.sxy / n - (self.sx / n) * (self.sy / n);
        let vx = self.sxx / n - (self.sx / n).powi(2);
        let vy = self.syy / n - (self.sy / n).powi(2);
        if vx <= 1e-12 || vy <= 1e-12 { 0.0 } else { cov / (vx.sqrt() * vy.sqrt()) }
    }
}

const SERIES: [&str; 27] = [
    "PWV7 low", "PWV7 mid", "PWV7 high", "PWV6 low", "PWV6 mid", "PWV6 high", "PWV5 height", "PWV5 low lvl",
    "PWV5 mid lvl", "PWV5 high lvl", "PWV4 b0", "PWV4 b1", "PWV4 b2", "PWV4 b3 (low)", "PWV4 b4 (mid)",
    "PWV4 b5 (high)", "PWV3 height", "PWV3 whiteness", "PWAV height", "PWAV whiteness", "PWV2", "PWVC low",
    "PWVC mid", "PWVC high", "columns PWV7", "", "",
];

type Metrics = Vec<Acc>;

fn compare(ours: &Waveforms, rb: &Reference) -> Metrics {
    let mut m = vec![Acc::default(); SERIES.len()];
    let n = ours.pwv7.len().min(rb.pwv7.len());
    for i in 0..n {
        for j in 0..3 {
            m[j].add(ours.pwv7[i][j] as f64, rb.pwv7[i][j] as f64);
        }
    }
    for i in 0..ours.pwv6.len().min(rb.pwv6.len()) {
        for j in 0..3 {
            m[3 + j].add(ours.pwv6[i][j] as f64, rb.pwv6[i][j] as f64);
        }
    }
    for i in 0..ours.pwv5.len().min(rb.pwv5.len()) {
        let (a, b) = (ours.pwv5[i], rb.pwv5[i]);
        m[6].add(((a >> 2) & 31) as f64, ((b >> 2) & 31) as f64);
        m[7].add(((a >> 13) & 7) as f64, ((b >> 13) & 7) as f64);
        m[8].add(((a >> 10) & 7) as f64, ((b >> 10) & 7) as f64);
        m[9].add(((a >> 7) & 7) as f64, ((b >> 7) & 7) as f64);
    }
    for i in 0..ours.pwv4.len().min(rb.pwv4.len()) {
        for j in 0..6 {
            m[10 + j].add(ours.pwv4[i][j] as f64, rb.pwv4[i][j] as f64);
        }
    }
    for i in 0..ours.pwv3.len().min(rb.pwv3.len()) {
        m[16].add((ours.pwv3[i] & 31) as f64, (rb.pwv3[i] & 31) as f64);
        m[17].add((ours.pwv3[i] >> 5) as f64, (rb.pwv3[i] >> 5) as f64);
    }
    for i in 0..ours.pwav.len().min(rb.pwav.len()) {
        m[18].add((ours.pwav[i] & 31) as f64, (rb.pwav[i] & 31) as f64);
        m[19].add((ours.pwav[i] >> 5) as f64, (rb.pwav[i] >> 5) as f64);
    }
    for i in 0..ours.pwv2.len().min(rb.pwv2.len()) {
        m[20].add(ours.pwv2[i] as f64, rb.pwv2[i] as f64);
    }
    for j in 0..3 {
        m[21 + j].add(ours.gains[j] as f64, rb.gains[j] as f64);
    }
    m[24].add(ours.pwv7.len() as f64, rb.pwv7.len() as f64);
    m
}

fn merge_metrics(all: &[Metrics]) -> Metrics {
    let mut out = vec![Acc::default(); SERIES.len()];
    for m in all {
        for (o, a) in out.iter_mut().zip(m) {
            o.merge(a);
        }
    }
    out
}

fn print_metrics(title: &str, m: &Metrics) {
    println!("\n{title}");
    println!("{:<18} {:>9} {:>9} {:>8} {:>8} {:>8}", "series", "columns", "exact %", "+-2 %", "MAE", "corr");
    for (name, a) in SERIES.iter().zip(m) {
        if name.is_empty() || a.n == 0.0 {
            continue;
        }
        if name.starts_with("PWVC") || name.starts_with("columns") {
            println!("{name:<18} {:>9} {:>9.1} {:>8.1} {:>8.2} {:>8}", a.n as u64, 100.0 * a.exact / a.n, 100.0 * a.w2 / a.n, a.mae(), "-");
            continue;
        }
        println!(
            "{name:<18} {:>9} {:>9.1} {:>8.1} {:>8.2} {:>8.3}",
            a.n as u64,
            100.0 * a.exact / a.n,
            100.0 * a.w2 / a.n,
            a.mae(),
            a.corr()
        );
    }
}

fn parallel<T: Send, R: Send>(items: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let queue = std::sync::Mutex::new(items.into_iter().enumerate().collect::<Vec<_>>());
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                loop {
                    let next = queue.lock().unwrap().pop();
                    let Some((i, item)) = next else { break };
                    let r = f(item);
                    results.lock().unwrap().push((i, r));
                }
            });
        }
    });
    let mut v = results.into_inner().unwrap();
    v.sort_by_key(|(i, _)| *i);
    v.into_iter().map(|(_, r)| r).collect()
}

fn load_tuning(o: &Options) -> Result<Tuning> {
    match &o.tuning {
        Some(p) => Ok(serde_json::from_slice(&std::fs::read(p)?)?),
        None => Ok(Tuning::default()),
    }
}

fn load_derivation(o: &Options) -> Result<Derivation> {
    match &o.derivation {
        Some(p) => Ok(serde_json::from_slice(&std::fs::read(p)?)?),
        None => Ok(Derivation::default()),
    }
}

/// Per track: the full-precision sections of `analyze`, and the sections the export
/// writes, derived from the `PWV7` bytes alone.
fn eval_set(
    entries: &[&Entry],
    tuning: Tuning,
    derivation: Derivation,
    monos: &HashMap<PathBuf, Mono>,
) -> Vec<(Metrics, Metrics)> {
    let items: Vec<&&Entry> = entries.iter().collect();
    parallel(items, |e| {
        let mono = monos.get(&e.audio);
        let owned;
        let mono = match mono {
            Some(m) => m,
            None => {
                owned = decode_mono(&e.audio).expect("decode");
                &owned
            }
        };
        let direct = measure(mono, tuning);
        let derived = Waveforms::from_bands(direct.pwv7.clone(), &tuning, &derivation);
        let exported = match std::env::var_os("WAVEFORM_COMPARE_BANDS_ONLY") {
            Some(_) => derived,
            None => derived.with_exact(&direct.color_waveform().exact.expect("measured audio carries exact columns")),
        };
        (compare(&direct, &e.reference), compare(&exported, &e.reference))
    })
}

fn direct_only(pairs: &[(Metrics, Metrics)]) -> Vec<Metrics> {
    pairs.iter().map(|(d, _)| d.clone()).collect()
}

fn export_only(pairs: &[(Metrics, Metrics)]) -> Vec<Metrics> {
    pairs.iter().map(|(_, e)| e.clone()).collect()
}

fn per_track_lines(entries: &[&Entry], metrics: &[Metrics]) {
    println!(
        "\n{:<44} {:>7} {:>7} {:>7} {:>7}  {:>13} {:>13} {:>13}",
        "track", "7 low", "7 mid", "7 high", "h PWV3", "gain low o/r", "gain mid o/r", "gain high o/r"
    );
    for (e, m) in entries.iter().zip(metrics) {
        let name: String = e.name.chars().rev().take(42).collect::<Vec<_>>().into_iter().rev().collect();
        let g = |i: usize| format!("{}/{}", m[i].sx as u32, m[i].sy as u32);
        println!(
            "{name:<44} {:>7.2} {:>7.2} {:>7.2} {:>7.2}  {:>13} {:>13} {:>13}",
            m[0].mae(),
            m[1].mae(),
            m[2].mae(),
            m[16].mae(),
            g(21),
            g(22),
            g(23)
        );
    }
}

fn loss(metrics: &[Metrics]) -> f64 {
    let m = merge_metrics(metrics);
    let band = |i: usize| m[i].mae() / 127.0;
    (band(0) + band(1) + band(2)) / 3.0 + 0.5 * ((m[3].mae() + m[4].mae() + m[5].mae()) / 3.0) / 127.0
        + 0.3 * (m[6].mae() / 31.0)
}

fn tuning_vec(t: &Tuning) -> Vec<f64> {
    vec![
        t.low_hz, t.mid_low_hz, t.mid_high_hz, t.high_hz, t.whiteness_lowpass_hz,
        t.release[0], t.release[1], t.release[2],
        t.gain_target[0], t.gain_target[1], t.gain_target[2],
        t.band_scale[0], t.band_scale[1], t.band_scale[2],
        t.pwv6_scale[0], t.pwv6_scale[1], t.pwv6_scale[2],
        t.pwv4_scale[0], t.pwv4_scale[1], t.pwv4_scale[2],
    ]
}

fn vec_tuning(base: &Tuning, v: &[f64]) -> Tuning {
    let mut t = *base;
    t.low_hz = v[0];
    t.mid_low_hz = v[1];
    t.mid_high_hz = v[2];
    t.high_hz = v[3];
    t.whiteness_lowpass_hz = v[4];
    t.release = [v[5].min(0.999), v[6].min(0.999), v[7].min(0.999)];
    t.gain_target = [v[8], v[9], v[10]];
    t.band_scale = [v[11], v[12], v[13]];
    t.pwv6_scale = [v[14], v[15], v[16]];
    t.pwv4_scale = [v[17], v[18], v[19]];
    t
}

fn fit(o: &Options, sel: &Selection) -> Result<()> {
    let tune: Vec<&Entry> = sel.entries.iter().filter(|e| e.tune).collect();
    let test: Vec<&Entry> = sel.entries.iter().filter(|e| !e.tune).collect();
    println!("decoding {} tune tracks", tune.len());
    let monos: HashMap<PathBuf, Mono> = parallel(tune.iter().collect::<Vec<_>>(), |e| {
        (e.audio.clone(), decode_mono(&e.audio).expect("decode"))
    })
    .into_iter()
    .collect();
    let base = load_tuning(o)?;
    let mut best_v = tuning_vec(&base);
    let mut best = loss(&direct_only(&eval_set(&tune, base, Derivation::default(), &monos)));
    println!("start loss {best:.5}");
    let mut step = 0.25;
    for round in 0..6 {
        let mut improved = false;
        for k in 0..best_v.len() {
            for dir in [1.0 + step, 1.0 / (1.0 + step)] {
                let mut v = best_v.clone();
                v[k] *= dir;
                let t = vec_tuning(&base, &v);
                let l = loss(&direct_only(&eval_set(&tune, t, Derivation::default(), &monos)));
                if l < best - 1e-6 {
                    best = l;
                    best_v = v;
                    improved = true;
                    println!("round {round} param {k} -> {:.4} loss {best:.5}", best_v[k]);
                    break;
                }
            }
        }
        if !improved {
            step /= 2.0;
        }
        std::fs::write(&o.out, serde_json::to_vec_pretty(&vec_tuning(&base, &best_v))?)?;
    }
    let tuned = vec_tuning(&base, &best_v);
    println!("\nfitted tuning written to {}\n{}", o.out.display(), serde_json::to_string_pretty(&tuned)?);
    let tm = direct_only(&eval_set(&tune, tuned, Derivation::default(), &monos));
    print_metrics("TUNE half (fitted tuning, direct)", &merge_metrics(&tm));
    drop(monos);
    let hm = direct_only(&eval_set(&test, tuned, Derivation::default(), &HashMap::new()));
    print_metrics("HELD-OUT half (fitted tuning, direct)", &merge_metrics(&hm));
    let hb = direct_only(&eval_set(&test, base, Derivation::default(), &HashMap::new()));
    print_metrics("HELD-OUT half (starting tuning, direct)", &merge_metrics(&hb));
    Ok(())
}

fn derivation_vec(d: &Derivation) -> Vec<f64> {
    vec![
        d.loudness_weight[0],
        d.loudness_weight[1],
        d.loudness_weight[2],
        d.bass_share_gain,
        d.loudness_power,
        d.level_weight[0],
        d.level_weight[1],
        d.level_weight[2],
    ]
}

fn vec_derivation(base: &Derivation, v: &[f64]) -> Derivation {
    let mut d = *base;
    d.loudness_weight = [v[0], v[1], v[2]];
    d.bass_share_gain = v[3];
    d.loudness_power = v[4].clamp(0.5, 12.0);
    d.level_weight = [v[5], v[6], v[7]];
    d
}

fn derived_metrics(entries: &[&Entry], tuning: &Tuning, d: &Derivation) -> Metrics {
    let all: Vec<Metrics> = entries
        .iter()
        .map(|e| compare(&Waveforms::from_bands(e.reference.pwv7.clone(), tuning, d), &e.reference))
        .collect();
    merge_metrics(&all)
}

/// What the derivation gets wrong when it is fed Rekordbox's own PWV7.
fn derivation_loss(entries: &[&Entry], tuning: &Tuning, d: &Derivation) -> f64 {
    let m = derived_metrics(entries, tuning, d);
    m[6].mae() / 31.0
        + (m[7].mae() + m[8].mae() + m[9].mae()) / 21.0
        + m[16].mae() / 31.0
        + m[17].mae() / 7.0
        + m[18].mae() / 31.0
        + m[19].mae() / 7.0
        + m[20].mae() / 15.0
}

fn fit_derivation(o: &Options, sel: &Selection) -> Result<()> {
    let tune: Vec<&Entry> = sel.entries.iter().filter(|e| e.tune).collect();
    let test: Vec<&Entry> = sel.entries.iter().filter(|e| !e.tune).collect();
    let tuning = load_tuning(o)?;
    let base = Derivation::default();
    let mut best_v = derivation_vec(&base);
    let mut best = derivation_loss(&tune, &tuning, &base);
    println!("start derivation loss {best:.5}");
    let mut step = 0.3;
    for round in 0..14 {
        let mut improved = false;
        for k in 0..best_v.len() {
            for dir in [1.0 + step, 1.0 / (1.0 + step)] {
                let mut v = best_v.clone();
                v[k] *= dir;
                let l = derivation_loss(&tune, &tuning, &vec_derivation(&base, &v));
                if l < best - 1e-7 {
                    best = l;
                    best_v = v;
                    improved = true;
                    println!("round {round} param {k} -> {:.4} loss {best:.5}", best_v[k]);
                    break;
                }
            }
        }
        if !improved {
            step /= 2.0;
        }
    }
    let fitted = vec_derivation(&base, &best_v);
    std::fs::write(&o.out, serde_json::to_vec_pretty(&fitted)?)?;
    println!("\nfitted derivation written to {}\n{}", o.out.display(), serde_json::to_string_pretty(&fitted)?);
    print_metrics("TUNE half, derived from Rekordbox PWV7 (fitted)", &derived_metrics(&tune, &tuning, &fitted));
    print_metrics("HELD-OUT half, derived from Rekordbox PWV7 (fitted)", &derived_metrics(&test, &tuning, &fitted));
    print_metrics("HELD-OUT half, derived from Rekordbox PWV7 (starting)", &derived_metrics(&test, &tuning, &base));
    Ok(())
}


#[derive(Clone, Copy)]
struct Bq {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Bq {
    fn make(high: bool, fc: f64, rate: f64) -> Bq {
        let w = 2.0 * std::f64::consts::PI * fc / rate;
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let b = if high {
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0]
        } else {
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0]
        };
        Bq { b: [b[0] / a0, b[1] / a0, b[2] / a0], a: [-2.0 * c / a0, (1.0 - alpha) / a0], x: [0.0; 2], y: [0.0; 2] }
    }

    fn step(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1] - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// Per-column peak / rms / mean-abs of a filter chain (`(high, fc)` stages), 150 columns per second.
fn column_features(mono: &Mono, stages: &[(bool, f64)]) -> [Vec<f64>; 3] {
    let rate = mono.rate as f64;
    let mut chain: Vec<Bq> = stages.iter().map(|&(h, fc)| Bq::make(h, fc, rate)).collect();
    let n = ((mono.samples.len() as f64 * 150.0 / rate).ceil() as usize).max(1);
    let mut peak = vec![0.0f64; n];
    let mut sq = vec![0.0f64; n];
    let mut ab = vec![0.0f64; n];
    let mut cnt = vec![0.0f64; n];
    let mut col = 0usize;
    let mut next_edge = (rate / 150.0).round() as usize;
    for (i, &x) in mono.samples.iter().enumerate() {
        while i >= next_edge {
            col += 1;
            next_edge = (((col + 1) as f64) * rate / 150.0).round() as usize;
        }
        let mut v = x as f64;
        for f in chain.iter_mut() {
            v = f.step(v);
        }
        let a = v.abs();
        if col < n {
            peak[col] = peak[col].max(a);
            sq[col] += v * v;
            ab[col] += a;
            cnt[col] += 1.0;
        }
    }
    let rms: Vec<f64> = sq.iter().zip(&cnt).map(|(s, c)| if *c > 0.0 { (s / c).sqrt() } else { 0.0 }).collect();
    let mab: Vec<f64> = ab.iter().zip(&cnt).map(|(s, c)| if *c > 0.0 { s / c } else { 0.0 }).collect();
    [peak, rms, mab]
}

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..a.len().min(b.len()) {
        sa += a[i];
        sb += b[i];
        saa += a[i] * a[i];
        sbb += b[i] * b[i];
        sab += a[i] * b[i];
    }
    let cov = sab / n - sa / n * sb / n;
    let va = saa / n - (sa / n).powi(2);
    let vb = sbb / n - (sb / n).powi(2);
    if va <= 1e-18 || vb <= 1e-18 { 0.0 } else { cov / (va.sqrt() * vb.sqrt()) }
}

fn release_env(x: &[f64], release: f64) -> Vec<f64> {
    let mut out = Vec::with_capacity(x.len());
    let mut prev = 0.0;
    for &v in x {
        prev = v.max(prev * release);
        out.push(prev);
    }
    out
}

/// For each band, the filter chain, feature and release whose envelope correlates best with Rekordbox's PWV7 band.
fn scan_bands(sel: &Selection) {
    let tune: Vec<&Entry> = sel.entries.iter().filter(|e| e.tune).collect();
    let monos: Vec<Mono> = parallel(tune.iter().collect::<Vec<_>>(), |e| decode_mono(&e.audio).expect("decode"));
    let low: Vec<Vec<(bool, f64)>> = [60.0, 80.0, 100.0, 120.0, 150.0, 200.0, 250.0, 300.0, 400.0].iter().map(|&f| vec![(false, f)]).collect();
    let mut mid = Vec::new();
    for hp in [100.0, 150.0, 200.0, 250.0, 300.0, 400.0, 500.0] {
        for lp in [800.0, 1000.0, 1500.0, 2000.0, 2500.0, 3000.0, 4000.0] {
            mid.push(vec![(true, hp), (false, lp)]);
        }
    }
    let high: Vec<Vec<(bool, f64)>> = [1000.0, 1500.0, 2000.0, 2500.0, 3000.0, 4000.0, 5000.0, 6000.0, 8000.0].iter().map(|&f| vec![(true, f)]).collect();
    let mut high = high;
    for f in [2000.0, 2500.0, 3000.0, 4000.0, 5000.0, 6000.0, 8000.0, 10000.0] {
        high.push(vec![(true, f), (true, f)]);
    }
    for f in [3000.0, 4000.0, 6000.0] {
        high.push(vec![(true, f), (true, f), (true, f)]);
    }
    let only: Option<usize> = std::env::var("SCAN_BAND").ok().and_then(|v| v.parse().ok());
    let banks: Vec<(&str, Vec<Vec<(bool, f64)>>)> = [("low", low), ("mid", mid), ("high", high)]
        .into_iter()
        .enumerate()
        .map(|(i, (n, b))| (n, if only.is_none_or(|o| o == i) { b } else { Vec::new() }))
        .collect();
    for (j, (name, bank)) in banks.iter().enumerate() {
        let mut results: Vec<(f64, String)> = Vec::new();
        for chain in bank {
            let feats: Vec<[Vec<f64>; 3]> = parallel(monos.iter().collect::<Vec<_>>(), |m| column_features(m, chain));
            for (fi, fname) in ["peak", "rms", "meanabs"].iter().enumerate() {
                for release in [0.0, 0.8, 0.85, 0.9, 0.93, 0.95, 0.97] {
                    let mut total = 0.0;
                    for (k, e) in tune.iter().enumerate() {
                        let env = release_env(&feats[k][fi], release);
                        let rb: Vec<f64> = e.reference.pwv7.iter().map(|b| b[j] as f64).collect();
                        total += pearson(&env, &rb);
                    }
                    let desc: Vec<String> = chain.iter().map(|(h, f)| format!("{}{}", if *h { "hp" } else { "lp" }, f)).collect();
                    results.push((total / tune.len() as f64, format!("{} {} release {}", desc.join("+"), fname, release)));
                }
            }
        }
        if results.is_empty() {
            continue;
        }
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        println!("\nband {name}: best mean per-track correlation with Rekordbox PWV7");
        for (r, d) in results.iter().take(8) {
            println!("  {r:.4}  {d}");
        }
    }
}


#[derive(Clone, Copy, Debug)]
struct HiCandidate {
    stages: usize,
    hz: f64,
    feature: usize,
    release: f64,
    target: f64,
    scale: f64,
    curve: f64,
}

struct HiScore {
    corr: f64,
    mae: f64,
    mean: (f64, f64),
    p75: (u32, u32),
    p90: (u32, u32),
    p99: (u32, u32),
    level: f64,
    loss: f64,
}

fn hi_quantile(hist: &[f64; 128], q: f64) -> u32 {
    let total: f64 = hist.iter().sum();
    let mut acc = 0.0;
    for (i, h) in hist.iter().enumerate() {
        acc += h;
        if acc >= q * total {
            return i as u32;
        }
    }
    127
}

fn hi_eval(c: &HiCandidate, feats: &HashMap<(usize, u64), Vec<[Vec<f64>; 3]>>, entries: &[&Entry]) -> HiScore {
    let f = &feats[&(c.stages, c.hz as u64)];
    let mut acc = Acc::default();
    let (mut ho, mut hr) = ([0.0f64; 128], [0.0f64; 128]);
    let mut level_err = 0.0;
    for (k, e) in entries.iter().enumerate() {
        let mut outs: Vec<f64> = Vec::new();
        let mut rbs: Vec<f64> = Vec::new();
        let env = release_env(&f[k][c.feature], c.release);
        let max = env.iter().cloned().fold(0.0, f64::max);
        let gain = if max > 0.0 { (c.target / max).round().clamp(80.0, 500.0) } else { 80.0 };
        for (i, &v) in env.iter().enumerate().take(e.reference.pwv7.len()) {
            let lin = (c.scale * gain * v).min(127.0);
            let out = (127.0 * (lin / 127.0).powf(c.curve)).round().min(127.0);
            let rb = e.reference.pwv7[i][2] as f64;
            acc.add(out, rb);
            outs.push(out);
            rbs.push(rb);
            ho[out as usize] += 1.0;
            hr[rb as usize] += 1.0;
        }
        let top = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let from = v.len() * 9 / 10;
            v[from..].iter().sum::<f64>() / (v.len() - from).max(1) as f64
        };
        level_err += ((top(&mut outs) + 1.0) / (top(&mut rbs) + 1.0)).ln().abs();
    }
    let level = level_err / entries.len().max(1) as f64;
    let n = acc.n.max(1.0);
    let q = |q: f64| (hi_quantile(&ho, q), hi_quantile(&hr, q));
    let (p75, p90, p99) = (q(0.75), q(0.9), q(0.99));
    let ratio = |p: (u32, u32)| ((p.0 as f64 + 1.0) / (p.1 as f64 + 1.0)).ln().abs();
    let mean = (acc.sx / n, acc.sy / n);
    let loss = (1.0 - acc.corr()) + 0.15 * (ratio(p75) + ratio(p90) + ratio(p99) + (mean.0 / mean.1).ln().abs()) + 0.5 * level;
    HiScore { corr: acc.corr(), mae: acc.mae(), mean, p75, p90, p99, level, loss }
}

fn hi_line(c: &HiCandidate, s: &HiScore) -> String {
    format!(
        "loss {:.4} corr {:.3} mae {:.2} level {:.3} mean {:.1}/{:.1} p75 {}/{} p90 {}/{} p99 {}/{}  | stages {} hz {} feat {} rel {:.3} target {:.1} scale {:.3} curve {:.2}",
        s.loss, s.corr, s.mae, s.level, s.mean.0, s.mean.1, s.p75.0, s.p75.1, s.p90.0, s.p90.1, s.p99.0, s.p99.1,
        c.stages, c.hz, ["peak", "rms", "meanabs"][c.feature], c.release, c.target, c.scale, c.curve
    )
}

fn hiband(sel: &Selection) {
    let tune: Vec<&Entry> = sel.entries.iter().filter(|e| e.tune).collect();
    let monos: Vec<Mono> = parallel(tune.iter().collect::<Vec<_>>(), |e| decode_mono(&e.audio).expect("decode"));
    let mut feats: HashMap<(usize, u64), Vec<[Vec<f64>; 3]>> = HashMap::new();
    let quick = std::env::var_os("HIBAND_QUICK").is_some();
    let hzs: Vec<f64> = if quick { vec![2500.0, 3000.0, 4000.0, 5500.0] } else { vec![2500.0, 3000.0, 3500.0, 4000.0, 4746.0, 5500.0, 6500.0, 8000.0] };
    let stage_list: Vec<usize> = if quick { vec![1] } else { vec![1, 2] };
    for &stages in &stage_list {
        for &hz in &hzs {
            let chain: Vec<(bool, f64)> = (0..stages).map(|_| (true, hz)).collect();
            feats.insert((stages, hz as u64), parallel(monos.iter().collect::<Vec<_>>(), |m| column_features(m, &chain)));
        }
    }
    let base = HiCandidate { stages: 1, hz: 3000.0, feature: 0, release: 0.90, target: 140.0, scale: 1.0, curve: 2.08 };
    println!("current: {}", hi_line(&base, &hi_eval(&base, &feats, &tune)));
    let mut results: Vec<(f64, HiCandidate, HiScore)> = Vec::new();
    let releases: Vec<f64> = if quick { vec![0.8, 0.86, 0.9, 0.93] } else { vec![0.0, 0.4, 0.6, 0.7, 0.765, 0.85] };
    let targets: Vec<f64> = if quick { vec![100.0, 140.0, 200.0] } else { vec![78.2, 100.0, 130.0, 170.0] };
    let curves: Vec<f64> = if quick { vec![1.4, 1.8, 2.08, 2.4] } else { vec![0.8, 1.0, 1.25] };
    for &stages in &stage_list {
        for &hz in &hzs {
            for feature in 0..(if quick { 2 } else { 3 }) {
                for &release in &releases {
                    for &target in &targets {
                        for &curve in &curves {
                            let c = HiCandidate { stages, hz, feature, release, target, scale: 1.0, curve };
                            let s = hi_eval(&c, &feats, &tune);
                            results.push((s.loss, c, s));
                        }
                    }
                }
            }
        }
    }
    let mut refined: Vec<(f64, HiCandidate, HiScore)> = Vec::new();
    for feature in [0usize, 1] {
        for &stages in &stage_list {
            for &hz in &hzs {
                for start in [(0.9, 140.0, 2.08)] {
                    let mut c = HiCandidate { stages, hz, feature, release: start.0, target: start.1, scale: 1.0, curve: start.2 };
                    let mut best = hi_eval(&c, &feats, &tune);
                    let mut step = 0.2;
                    for _ in 0..40 {
                        let mut improved = false;
                        for k in 0..3 {
                            for dir in [1.0 + step, 1.0 / (1.0 + step)] {
                                let mut t = c;
                                match k {
                                    0 => t.release = (t.release * dir).min(0.98),
                                    1 => t.target *= dir,
                                    _ => t.curve *= dir,
                                }
                                let s = hi_eval(&t, &feats, &tune);
                                if s.loss < best.loss - 1e-5 {
                                    c = t;
                                    best = s;
                                    improved = true;
                                    break;
                                }
                            }
                        }
                        if !improved {
                            step /= 2.0;
                            if step < 0.01 {
                                break;
                            }
                        }
                    }
                    refined.push((best.loss, c, best));
                }
            }
        }
    }
    results.extend(refined);
    results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for (_, c, s) in results.iter().take(25) {
        println!("{}", hi_line(c, s));
    }
    println!("\nbest corr:");
    results.sort_by(|a, b| b.2.corr.partial_cmp(&a.2.corr).unwrap());
    for (_, c, s) in results.iter().take(8) {
        println!("{}", hi_line(c, s));
    }
}

/// Mean Rekordbox PWV7 byte against our normalised envelope, per band: shows whether the byte scale is linear.
fn curve(sel: &Selection) {
    let tune: Vec<&Entry> = sel.entries.iter().filter(|e| e.tune).collect();
    let monos: Vec<Mono> = parallel(tune.iter().collect::<Vec<_>>(), |e| decode_mono(&e.audio).expect("decode"));
    let chains: [(Vec<(bool, f64)>, f64); 3] = [
        (vec![(false, 300.0)], 0.97),
        (vec![(true, 250.0), (false, 1500.0)], 0.93),
        (vec![(true, 3000.0)], 0.85),
    ];
    for (j, (chain, release)) in chains.iter().enumerate() {
        let feats: Vec<Vec<f64>> = parallel(monos.iter().collect::<Vec<_>>(), |m| release_env(&column_features(m, chain)[0], *release));
        let mut bins = vec![(0.0f64, 0.0f64, 0.0f64); 20];
        let mut gains = Vec::new();
        for (k, e) in tune.iter().enumerate() {
            let max = feats[k].iter().cloned().fold(0.0, f64::max).max(1e-9);
            let rbmax = e.reference.pwv7.iter().map(|b| b[j]).max().unwrap_or(0) as f64;
            gains.push((rbmax, e.reference.gains[j], (max * 1000.0).round() / 1000.0, (rbmax / max).round()));
            for (i, &v) in feats[k].iter().enumerate() {
                if i >= e.reference.pwv7.len() {
                    break;
                }
                let x = v / max;
                let b = ((x * 20.0) as usize).min(19);
                let y = e.reference.pwv7[i][j] as f64 / rbmax.max(1.0);
                bins[b].0 += 1.0;
                bins[b].1 += y;
                bins[b].2 += y * y;
            }
        }
        println!("\nband {j}: our envelope / track max (x) -> Rekordbox byte / track max byte (mean, sd)");
        for (b, (n, s, ss)) in bins.iter().enumerate() {
            if *n > 0.0 {
                let m = s / n;
                println!("  x {:.3}  n {:>8}  mean {:.3}  sd {:.3}", (b as f64 + 0.5) / 20.0, *n as u64, m, (ss / n - m * m).max(0.0).sqrt());
            }
        }
        println!("  (rb max byte, rb gain, our max env, implied scale) per track:");
        for g in &gains {
            println!("    {:?}", g);
        }
    }
}


/// Correlation of our band envelopes with Rekordbox's PWV7 when ours is shifted by `lag` columns, per file type.
fn lag_scan(sel: &Selection) {
    let tune: Vec<&Entry> = sel.entries.iter().collect();
    let monos: Vec<Mono> = parallel(tune.iter().collect::<Vec<_>>(), |e| decode_mono(&e.audio).expect("decode"));
    let chains: [(Vec<(bool, f64)>, f64); 3] = [
        (vec![(false, 300.0)], 0.97),
        (vec![(true, 250.0), (false, 1500.0)], 0.93),
        (vec![(true, 3000.0)], 0.85),
    ];
    for (j, (chain, release)) in chains.iter().enumerate() {
        let feats: Vec<Vec<f64>> = parallel(monos.iter().collect::<Vec<_>>(), |m| column_features(m, chain)[0].clone());
        println!("\nband {j}");
        let mut by_ext: std::collections::BTreeMap<String, Vec<Vec<f64>>> = Default::default();
        for (k, e) in tune.iter().enumerate() {
            let ext = Path::new(&e.name).extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
            let rb: Vec<f64> = e.reference.pwv7.iter().map(|b| b[j] as f64).collect();
            let mut row = Vec::new();
            for lag in -4i32..=4 {
                let shifted: Vec<f64> = (0..rb.len() as i32)
                    .map(|i| {
                        let s = i - lag;
                        if s >= 0 && (s as usize) < feats[k].len() { feats[k][s as usize] } else { 0.0 }
                    })
                    .collect();
                row.push(pearson(&release_env(&shifted, *release), &rb));
            }
            by_ext.entry(ext).or_default().push(row);
        }
        for (ext, rows) in by_ext {
            let mean: Vec<String> = (0..9).map(|l| format!("{:.4}", rows.iter().map(|r| r[l]).sum::<f64>() / rows.len() as f64)).collect();
            println!("  {ext:<5} ({} tracks) lags -4..4: {}", rows.len(), mean.join(" "));
        }
    }
}


/// Which statistic of our band envelope times Rekordbox's gain is the most constant, per band.
fn gain_stats(sel: &Selection) {
    let entries: Vec<&Entry> = sel.entries.iter().collect();
    let monos: Vec<Mono> = parallel(entries.iter().collect::<Vec<_>>(), |e| decode_mono(&e.audio).expect("decode"));
    let t = Tuning::default();
    let chains: [(Vec<(bool, f64)>, f64); 3] = [
        (vec![(false, t.low_hz)], t.release[0]),
        (vec![(true, t.mid_low_hz), (false, t.mid_high_hz)], t.release[1]),
        (vec![(true, t.high_hz)], t.release[2]),
    ];
    let names = ["max", "p99.9", "p99.5", "p99", "p97", "p95", "p90", "p80", "mean", "raw max"];
    for (j, (chain, release)) in chains.iter().enumerate() {
        let feats: Vec<(Vec<f64>, f64)> = parallel(monos.iter().collect::<Vec<_>>(), |m| {
            let raw = column_features(m, chain)[0].clone();
            let rawmax = raw.iter().cloned().fold(0.0, f64::max);
            (release_env(&raw, *release), rawmax)
        });
        let mut per_stat: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
        for (k, e) in entries.iter().enumerate() {
            let gain = e.reference.gains[j] as f64;
            if gain <= 81.0 || gain >= 299.0 {
                continue;
            }
            let mut v = feats[k].0.clone();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let q = |p: f64| v[(((v.len() - 1) as f64) * p) as usize];
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let stats = [q(1.0), q(0.999), q(0.995), q(0.99), q(0.97), q(0.95), q(0.90), q(0.80), mean, feats[k].1];
            for (i, s) in stats.iter().enumerate() {
                per_stat[i].push(gain * s);
            }
        }
        println!("\nband {j}: gain x statistic over {} tracks with gain strictly between floor and cap", per_stat[0].len());
        for (i, name) in names.iter().enumerate() {
            let v = &per_stat[i];
            if v.is_empty() {
                continue;
            }
            let m = v.iter().sum::<f64>() / v.len() as f64;
            let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
            println!("  {name:<8} mean {m:>8.3}  sd {sd:>7.3}  cv {:.3}", sd / m.max(1e-9));
        }
    }
}

fn render_png(path: &Path, ours: &Waveforms, rb: &Reference, window: Option<usize>) -> Result<()> {
    const W: usize = 1200;
    const HALF: usize = 140;
    let colors: [[f32; 3]; 3] = [[24.0, 96.0, 255.0], [255.0, 160.0, 32.0], [255.0, 255.0, 255.0]];
    let mut img = image::RgbImage::from_pixel(W as u32, (HALF * 2 + 10) as u32, image::Rgb([14, 14, 18]));
    let draw = |img: &mut image::RgbImage, data: &[[u8; 3]], y0: usize| {
        if data.is_empty() {
            return;
        }
        for x in 0..W {
            let a = x * data.len() / W;
            let b = ((x + 1) * data.len() / W).clamp(a + 1, data.len());
            let mut v = [0u8; 3];
            for j in 0..3 {
                v[j] = data[a..b].iter().map(|c| c[j]).max().unwrap_or(0);
            }
            let mid = y0 + HALF / 2;
            for j in 0..3 {
                let h = (v[j] as f32 / 127.0 * (HALF as f32 / 2.0 - 2.0)).round() as i32;
                for dy in -h..=h {
                    let y = (mid as i32 + dy) as u32;
                    img.put_pixel(x as u32, y, image::Rgb([colors[j][0] as u8, colors[j][1] as u8, colors[j][2] as u8]));
                }
            }
        }
    };
    let crop = |data: &[[u8; 3]]| -> Vec<[u8; 3]> {
        match window {
            Some(w) if data.len() > w => {
                let start = data.len() / 2 - w / 2;
                data[start..start + w].to_vec()
            }
            _ => data.to_vec(),
        }
    };
    draw(&mut img, &crop(&ours.pwv7), 0);
    draw(&mut img, &crop(&rb.pwv7), HALF + 10);
    img.save(path)?;
    Ok(())
}

fn parse_options(args: &[String]) -> Options {
    let mut o = Options {
        n: 40,
        library: PathBuf::from(DEFAULT_LIBRARY),
        audio: PathBuf::from(DEFAULT_AUDIO),
        strip: DEFAULT_STRIP.into(),
        set: "all".into(),
        tuning: None,
        derivation: None,
        out: PathBuf::from("tuning.json"),
        png_count: 4,
    };
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--n" => o.n = v.parse().unwrap_or(40),
            "--library" => o.library = v.into(),
            "--audio" => o.audio = v.into(),
            "--strip" => o.strip = v,
            "--set" => o.set = v,
            "--tuning" => o.tuning = Some(v.into()),
            "--derivation" => o.derivation = Some(v.into()),
            "--out" => o.out = v.into(),
            "--png-count" => o.png_count = v.parse().unwrap_or(4),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    o
}

/// `track <outdir>` with `WAVEFORM_TRACKS="Title=audio path,Title=audio path"`: one JSON per track with
/// Rekordbox's own sections (from `--library`), the beat grid and our bytes measured from the audio.
fn dump_tracks(o: &Options, args: &[String]) -> Result<()> {
    let dir = PathBuf::from(args.get(1).context("track needs an output directory")?);
    std::fs::create_dir_all(&dir)?;
    let spec = std::env::var("WAVEFORM_TRACKS").context("set WAVEFORM_TRACKS to Title=audio path,...")?;
    let source = rekordbox::load_masterdb(&o.library.join("master.db"), Some(&o.library.join("share")))?;
    let tuning = load_tuning(o)?;
    let flat3 = |v: &[[u8; 3]]| -> Vec<u8> { v.iter().flatten().copied().collect() };
    let flat6 = |v: &[[u8; 6]]| -> Vec<u8> { v.iter().flatten().copied().collect() };
    for item in spec.split(',') {
        let (title, audio) = item.split_once('=').context("Title=audio path")?;
        let t = source
            .tracks
            .iter()
            .find(|t| t.track.title.to_lowercase() == title.to_lowercase() && t.anlz_dat.as_ref().is_some_and(|d| d.exists()))
            .with_context(|| format!("no track with analysis matches {title}"))?;
        let dat = t.anlz_dat.clone().unwrap();
        let reference = read_reference(&dat)?;
        let beats = pioneer_usb_writer::reader::anlz::read_anlz(&dat)?
            .beat_grid
            .beats
            .iter()
            .map(|b| serde_json::json!({ "time_ms": b.time_ms, "bar_position": b.bar_position }))
            .collect::<Vec<_>>();
        let mono = decode_mono(Path::new(audio))?;
        let ours = measure(&mono, tuning);
        let fixture = serde_json::json!({
            "name": t.track.title,
            "audio": audio,
            "duration_secs": mono.samples.len() as f64 / mono.rate as f64,
            "beats": beats,
            "rekordbox": {
                "pwv7": flat3(&reference.pwv7), "pwv6": flat3(&reference.pwv6), "pwv4": flat6(&reference.pwv4),
                "pwv5": reference.pwv5, "pwv3": reference.pwv3, "pwav": reference.pwav, "gains": reference.gains,
            },
            "ours": {
                "pwv7": flat3(&ours.pwv7), "pwv6": flat3(&ours.pwv6), "pwv4": flat6(&ours.pwv4),
                "pwv5": ours.pwv5, "pwv3": ours.pwv3, "pwav": ours.pwav, "gains": ours.gains,
            },
        });
        let path = dir.join(format!("{}.json", title.to_lowercase().replace(' ', "-")));
        std::fs::write(&path, serde_json::to_vec(&fixture)?)?;
        println!("{} {} ({} columns)", path.display(), t.track.title, reference.pwv7.len());
    }
    Ok(())
}

#[derive(Clone)]
struct Shape {
    levels: [[u64; 128]; 3],
    rises: [[u64; 128]; 3],
    falls: [f64; 3],
    fall_n: [f64; 3],
}

impl Shape {
    fn new() -> Self {
        Shape { levels: [[0; 128]; 3], rises: [[0; 128]; 3], falls: [0.0; 3], fall_n: [0.0; 3] }
    }

    fn add(&mut self, pwv7: &[[u8; 3]]) {
        for (i, c) in pwv7.iter().enumerate() {
            for j in 0..3 {
                self.levels[j][c[j].min(127) as usize] += 1;
                if i > 0 {
                    let prev = pwv7[i - 1][j];
                    if c[j] > prev {
                        self.rises[j][(c[j] - prev).min(127) as usize] += 1;
                    } else if c[j] < prev && prev > 20 {
                        self.falls[j] += c[j] as f64 / prev as f64;
                        self.fall_n[j] += 1.0;
                    }
                }
            }
        }
    }

    fn merge(&mut self, o: &Shape) {
        for j in 0..3 {
            for k in 0..128 {
                self.levels[j][k] += o.levels[j][k];
                self.rises[j][k] += o.rises[j][k];
            }
            self.falls[j] += o.falls[j];
            self.fall_n[j] += o.fall_n[j];
        }
    }
}

fn percentile_of(hist: &[u64; 128], p: f64) -> usize {
    let total: u64 = hist.iter().sum();
    let target = (total as f64 * p).ceil() as u64;
    let mut seen = 0;
    for (v, n) in hist.iter().enumerate() {
        seen += n;
        if seen >= target {
            return v;
        }
    }
    127
}

fn mean_of(hist: &[u64; 128]) -> f64 {
    let total: u64 = hist.iter().sum();
    hist.iter().enumerate().map(|(v, n)| v as f64 * *n as f64).sum::<f64>() / total.max(1) as f64
}

/// Pooled byte distribution (p50 p90 p99), mean rise per rising column (attack) with its p90, and the mean
/// fall ratio per falling column (decay), ours against Rekordbox, over one half of the selection.
fn shape(o: &Options, sel: &Selection) -> Result<()> {
    let tuning = load_tuning(o)?;
    let entries: Vec<&Entry> = sel.entries.iter().filter(|e| match o.set.as_str() {
        "tune" => e.tune,
        "test" => !e.tune,
        _ => true,
    }).collect();
    let per = parallel(entries.iter().collect::<Vec<_>>(), |e| {
        let mono = decode_mono(&e.audio).expect("decode");
        let ours = measure(&mono, tuning);
        let n = ours.pwv7.len().min(e.reference.pwv7.len());
        let (mut a, mut b) = (Shape::new(), Shape::new());
        a.add(&ours.pwv7[..n]);
        b.add(&e.reference.pwv7[..n]);
        (a, b)
    });
    let (mut ours, mut rb) = (Shape::new(), Shape::new());
    for (a, b) in &per {
        ours.merge(a);
        rb.merge(b);
    }
    for (j, name) in ["low", "mid", "high"].iter().enumerate() {
        let line = |s: &Shape| {
            format!(
                "p50 {:3} p90 {:3} p99 {:3} mean {:5.1} | rise mean {:5.2} p90 {:3} | fall ratio {:.3}",
                percentile_of(&s.levels[j], 0.5),
                percentile_of(&s.levels[j], 0.9),
                percentile_of(&s.levels[j], 0.99),
                mean_of(&s.levels[j]),
                mean_of(&s.rises[j]),
                percentile_of(&s.rises[j], 0.9),
                s.falls[j] / s.fall_n[j].max(1.0),
            )
        };
        println!("{name:5} rekordbox {}", line(&rb));
        println!("{name:5} ours      {}", line(&ours));
    }
    Ok(())
}

/// `refdump <dir>`: per selected track, Rekordbox's PWV7 / PWV6 / gains and ours, as flat byte arrays in JSON.
fn refdump(o: &Options, sel: &Selection, dir: &str) -> Result<()> {
    let tuning = load_tuning(o)?;
    std::fs::create_dir_all(dir)?;
    let flat3 = |v: &[[u8; 3]]| -> Vec<u8> { v.iter().flatten().copied().collect() };
    let entries: Vec<&Entry> = sel.entries.iter().collect();
    let done = parallel(entries.iter().collect::<Vec<_>>(), |e| {
        let mono = decode_mono(&e.audio).expect("decode");
        let ours = measure(&mono, tuning);
        serde_json::json!({
            "name": e.name, "tune": e.tune,
            "rb": { "pwv7": flat3(&e.reference.pwv7), "pwv6": flat3(&e.reference.pwv6), "gains": e.reference.gains },
            "ours": { "pwv7": flat3(&ours.pwv7), "pwv6": flat3(&ours.pwv6), "gains": ours.gains },
        })
    });
    for (i, v) in done.iter().enumerate() {
        std::fs::write(format!("{dir}/{i:02}.json"), serde_json::to_vec(v)?)?;
    }
    println!("wrote {} tracks to {dir}", done.len());
    Ok(())
}

/// Per-column peak of the high band for four ways of treating the two channels: mono sum, louder channel,
/// rms across channels, mean of the channels' absolute values (150 columns per second).
fn stereo_features(path: &Path, hz: f64) -> [Vec<f64>; 4] {
    let mut out: [Vec<f64>; 4] = Default::default();
    let mut state: Option<(Bq, Bq, f64, usize, usize)> = None;
    let mut acc = [0.0f64; 4];
    let _ = decode(path, |chunk, rate, channels| {
        let st = state.get_or_insert_with(|| {
            let r = rate as f64;
            (Bq::make(true, hz, r), Bq::make(true, hz, r), r, 0usize, (r / 150.0).round() as usize)
        });
        let mut frame_index = st.3;
        for frame in chunk.chunks_exact(channels.max(1)) {
            let l = frame[0] as f64;
            let r = if channels > 1 { frame[1] as f64 } else { l };
            while frame_index >= st.4 {
                for k in 0..4 {
                    out[k].push(acc[k]);
                    acc[k] = 0.0;
                }
                let col = out[0].len();
                st.4 = (((col + 1) as f64) * st.2 / 150.0).round() as usize;
            }
            let hl = st.0.step(l);
            let hr = st.1.step(r);
            acc[0] = acc[0].max(((hl + hr) / 2.0).abs());
            acc[1] = acc[1].max(hl.abs().max(hr.abs()));
            acc[2] = acc[2].max(((hl * hl + hr * hr) / 2.0).sqrt());
            acc[3] = acc[3].max((hl.abs() + hr.abs()) / 2.0);
            frame_index += 1;
        }
        st.3 = frame_index;
    });
    for k in 0..4 {
        out[k].push(acc[k]);
    }
    out
}

fn stereo(o: &Options, sel: &Selection) {
    let release = 0.90;
    let target = 140.0;
    let curve: f64 = 2.08;
    let entries: Vec<&Entry> = sel.entries.iter().collect();
    let feats = parallel(entries.iter().collect::<Vec<_>>(), |e| stereo_features(&e.audio, 3000.0));
    let names = ["mono sum", "louder channel", "rms of channels", "mean of |L|,|R|"];
    for half in ["tune", "test"] {
        for (k, name) in names.iter().enumerate() {
            let mut acc = Acc::default();
            let mut level = 0.0;
            let mut tracks = 0.0;
            let mut level_signed = 0.0;
            for (i, e) in entries.iter().enumerate() {
                if (half == "tune") != e.tune {
                    continue;
                }
                let env = release_env(&feats[i][k], release);
                let max = env.iter().cloned().fold(0.0, f64::max);
                let gain = if max > 0.0 { (target / max).round().clamp(80.0, 500.0) } else { 80.0 };
                let (mut outs, mut rbs) = (Vec::new(), Vec::new());
                for (c, &v) in env.iter().enumerate().take(e.reference.pwv7.len()) {
                    let lin = (gain * v).min(127.0);
                    let out = (127.0 * (lin / 127.0).powf(curve)).round().min(127.0);
                    let rb = e.reference.pwv7[c][2] as f64;
                    acc.add(out, rb);
                    outs.push(out);
                    rbs.push(rb);
                }
                let top = |v: &mut Vec<f64>| {
                    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let from = v.len() * 9 / 10;
                    v[from..].iter().sum::<f64>() / (v.len() - from).max(1) as f64
                };
                let r = ((top(&mut outs) + 1.0) / (top(&mut rbs) + 1.0)).ln();
                level += r.abs();
                level_signed += r;
                tracks += 1.0;
            }
            println!(
                "{half} high {name:18} corr {:.3} mae {:.2} | top-decile level error per track: mean abs {:.3} (log), mean signed {:+.3}",
                acc.corr(), acc.mae(), level / tracks, level_signed / tracks
            );
        }
    }
    let _ = o;
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_else(|| "score".into());
    let o = parse_options(&args);
    if cmd == "track" {
        return dump_tracks(&o, &args);
    }
    let sel = select(&o).context("select tracks")?;
    println!("{} tracks selected ({} tune, {} test)", sel.entries.len(), sel.entries.iter().filter(|e| e.tune).count(), sel.entries.iter().filter(|e| !e.tune).count());
    match cmd.as_str() {
        "list" => {
            for e in &sel.entries {
                println!("{} {}", if e.tune { "tune" } else { "test" }, e.name);
            }
        }
        "score" => {
            let tuning = load_tuning(&o)?;
            let entries: Vec<&Entry> = sel
                .entries
                .iter()
                .filter(|e| match o.set.as_str() {
                    "tune" => e.tune,
                    "test" => !e.tune,
                    _ => true,
                })
                .collect();
            let derivation = load_derivation(&o)?;
            let pairs = eval_set(&entries, tuning, derivation, &HashMap::new());
            per_track_lines(&entries, &direct_only(&pairs));
            print_metrics(
                &format!("{} tracks, set {}: direct (analyze)", entries.len(), o.set),
                &merge_metrics(&direct_only(&pairs)),
            );
            print_metrics(
                &format!("{} tracks, set {}: export path (PWV7 bands plus measured PWV3/PWV5/PWVC columns; bands only with WAVEFORM_COMPARE_BANDS_ONLY set)", entries.len(), o.set),
                &merge_metrics(&export_only(&pairs)),
            );
        }
        "fixtures" => {
            let dir = PathBuf::from(args.get(1).context("fixtures needs an output directory")?);
            std::fs::create_dir_all(&dir)?;
            let picks: Vec<String> = std::env::var("WAVEFORM_FIXTURE_PICK")
                .context("set WAVEFORM_FIXTURE_PICK to comma separated track name parts")?
                .split(',')
                .map(str::to_string)
                .collect();
            let flat = |bands: &[[u8; 3]]| -> Vec<u8> { bands.iter().flatten().copied().collect() };
            for (k, pick) in picks.iter().enumerate() {
                let e = sel.entries.iter().find(|e| e.name.contains(pick.as_str())).with_context(|| format!("no track matches {pick}"))?;
                let beats = pioneer_usb_writer::reader::anlz::read_anlz(&e.dat)?
                    .beat_grid
                    .beats
                    .iter()
                    .map(|b| serde_json::json!({ "time_ms": b.time_ms, "bar_position": b.bar_position }))
                    .collect::<Vec<_>>();
                let fixture = serde_json::json!({
                    "name": e.name,
                    "audio": e.audio,
                    "pwv7": flat(&e.reference.pwv7),
                    "pwv6": flat(&e.reference.pwv6),
                    "pwav": e.reference.pwav,
                    "beats": beats,
                });
                let path = dir.join(format!("{:02}.json", k + 1));
                std::fs::write(&path, serde_json::to_vec(&fixture)?)?;
                println!("{} {}", path.display(), e.name);
            }
        }
        "shape" => shape(&o, &sel)?,
        "stereo" => stereo(&o, &sel),
        "refdump" => refdump(&o, &sel, args.get(1).context("refdump needs an output directory")?)?,
        "fit" => fit(&o, &sel)?,
        "fit-derive" => fit_derivation(&o, &sel)?,
        "scan" => scan_bands(&sel),
        "curve" => curve(&sel),
        "hiband" => hiband(&sel),
        "lag" => lag_scan(&sel),
        "gainstat" => gain_stats(&sel),
        "png" => {
            let dir = PathBuf::from(args.get(1).context("png needs an output directory")?);
            std::fs::create_dir_all(&dir)?;
            let tuning = load_tuning(&o)?;
            let test: Vec<&Entry> = sel.entries.iter().filter(|e| !e.tune).collect();
            let step = (test.len() / o.png_count.max(1)).max(1);
            for (k, e) in test.iter().step_by(step).take(o.png_count).enumerate() {
                let mono = decode_mono(&e.audio)?;
                let ours = measure(&mono, tuning);
                let name = format!("{:02}_{}.png", k + 1, e.name.rsplit('/').next().unwrap_or("track").replace(|c: char| !c.is_alphanumeric(), "_"));
                render_png(&dir.join(&name), &ours, &e.reference, None)?;
                println!("{}", dir.join(&name).display());
                let zoom = name.replace(".png", "_zoom.png");
                render_png(&dir.join(&zoom), &ours, &e.reference, Some(1200))?;
                println!("{}", dir.join(zoom).display());
            }
        }
        other => return Err(anyhow!("unknown command {other}")),
    }
    Ok(())
}
