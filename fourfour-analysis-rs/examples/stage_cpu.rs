use std::borrow::Cow;
use std::path::PathBuf;
use std::time::Instant;

use fourfour_analysis::bt::MODEL_RATE;
use fourfour_analysis::decode::{decode_mono, resample_to};
use fourfour_analysis::cyankey;
use pioneer_usb_writer::waveform::{analyze, Meter};

const REPS: usize = 3;

fn process_cpu_ms() -> f64 {
    let text = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let tail = text.rsplit(')').next().unwrap_or("");
    let fields: Vec<&str> = tail.split_whitespace().collect();
    let ticks = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (ticks(11) + ticks(12)) * 10.0
}

fn fnv(bits: impl Iterator<Item = u64>) -> u64 {
    bits.fold(0xcbf29ce484222325u64, |h, b| (h ^ b).wrapping_mul(0x100000001b3))
}

fn timed<T>(name: &str, line: &mut Vec<String>, mut f: impl FnMut() -> T) -> T {
    let (c, t) = (process_cpu_ms(), Instant::now());
    let mut out = f();
    for _ in 1..REPS {
        out = f();
    }
    let n = REPS as f64;
    line.push(format!("{name}={:.0}/{:.0}", (process_cpu_ms() - c) / n, t.elapsed().as_secs_f64() * 1000.0 / n));
    out
}

fn main() -> anyhow::Result<()> {
    let threads: usize = std::env::var("STAGE_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    for path in std::env::args().skip(1).map(PathBuf::from) {
        let mut line = Vec::new();
        let native = timed("decode", &mut line, || decode_mono(&path))?;
        let rate = native.rate;
        let gapless = native.gapless_samples();
        let measured = timed("wf_meter", &mut line, || {
            let mut meter = Meter::new(rate, 1);
            meter.push(&native.samples);
            meter.finish()
        });
        let _wf = timed("wf_columns", &mut line, || analyze(&measured));
        let mono44: Cow<[f32]> = match rate {
            44100 => Cow::Borrowed(gapless),
            rate => Cow::Owned(timed("resample44", &mut line, || resample_to(gapless, rate, 44100))?),
        };
        let key = timed("key", &mut line, || cyankey::detect(&mono44, 44100, threads));
        let samples_hash = fnv(native.samples.iter().map(|s| s.to_bits() as u64));
        let key_hash = key.map_or(0, |k| fnv([k.class as u64, k.confidence.to_bits(), k.tuning_cents.to_bits()].into_iter()));
        let mono44_hash = fnv(mono44.iter().map(|s| s.to_bits() as u64));
        drop(mono44);
        let _r22 = timed("resample22", &mut line, || resample_to(gapless, rate, MODEL_RATE))?;
        let r22_hash = fnv(_r22.iter().map(|s| s.to_bits() as u64));
        println!("{}\t{}\t{:.0}\t{}", path.file_name().unwrap().to_string_lossy(), rate, native.samples.len() as f64 / rate as f64, line.join(" "));
        eprintln!("HASH\t{}\tsamples={samples_hash:016x} key={key_hash:016x} r22={r22_hash:016x} m44={mono44_hash:016x}", path.file_name().unwrap().to_string_lossy());
    }
    Ok(())
}
