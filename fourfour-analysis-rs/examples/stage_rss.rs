use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fourfour_analysis::analyze::Control;
use fourfour_analysis::bt::BeatEngine;
use fourfour_analysis::decode::{decode_mono, resample_to};
use fourfour_analysis::service::{chunk_slots, locate_models, total_memory_mb, available_memory_mb, BEAT_MODEL};
use fourfour_analysis::{cyankey, waveform};

fn rss_mb() -> f64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<f64>().ok())
        .map_or(0.0, |kb| kb / 1024.0)
}

struct Sampler {
    samples: Arc<Mutex<Vec<(f64, f64)>>>,
    start: Instant,
}

impl Sampler {
    fn peak_between(&self, from: f64, to: f64) -> f64 {
        self.samples.lock().unwrap().iter().filter(|(t, _)| *t >= from && *t <= to).map(|(_, r)| *r).fold(0.0, f64::max)
    }

    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

fn main() -> anyhow::Result<()> {
    let path = PathBuf::from(std::env::args().nth(1).expect("audio file"));
    let threads: usize = std::env::args().nth(2).map_or(std::thread::available_parallelism().map_or(1, |n| n.get()), |v| v.parse().unwrap());
    let samples = Arc::new(Mutex::new(Vec::new()));
    let start = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    {
        let (samples, stop) = (samples.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                samples.lock().unwrap().push((start.elapsed().as_secs_f64(), rss_mb()));
                std::thread::sleep(Duration::from_millis(5));
            }
        });
    }
    let s = Sampler { samples, start };
    let report = |name: &str, from: f64, to: f64, s: &Sampler| {
        println!("{name:<18} peak {:>7.0} MB  after {:>7.0} MB  {:>6.1} s", s.peak_between(from, to), rss_mb(), to - from);
    };

    let dir = locate_models(None).expect("models");
    let engine = BeatEngine::load(&dir, BEAT_MODEL, false)?;
    engine.share_chunk_threads(chunk_slots(threads, 0.5, total_memory_mb(), available_memory_mb()));
    let t0 = s.now();
    report("models", 0.0, t0, &s);

    let native = decode_mono(&path)?;
    let t1 = s.now();
    report("decode", t0, t1, &s);
    let gapless = native.gapless_samples();

    let wf = waveform::compute(&native.samples, native.rate);
    let t2 = s.now();
    report("waveform", t1, t2, &s);

    let mono44: Cow<[f32]> = match native.rate {
        44100 => Cow::Borrowed(gapless),
        rate => Cow::Owned(resample_to(gapless, rate, 44100)?),
    };
    let t3 = s.now();
    report("resample 44.1k", t2, t3, &s);
    let key = cyankey::detect(&mono44, 44100, 2);
    drop(mono44);
    let t4 = s.now();
    report("key", t3, t4, &s);

    let cancel = AtomicBool::new(false);
    let bt = engine.analyze(gapless, native.rate, threads, &Control::new(&cancel))?;
    let t5 = s.now();
    let r = t4 + bt.resample_ms / 1000.0;
    let m = r + bt.mel_ms / 1000.0;
    report("resample 22.05k", t4, r, &s);
    report("mel", r, m, &s);
    report("beat chunks", m, t5, &s);
    stop.store(true, Ordering::Relaxed);
    println!(
        "duration {:.0} s  rate {}  key {:?}  beats {}  wf {}  overall peak {:.0} MB",
        native.samples.len() as f64 / native.rate as f64,
        native.rate,
        key.map(|k| k.class),
        bt.beats.len(),
        waveform::checksum(&wf),
        s.peak_between(0.0, t5)
    );
    Ok(())
}
