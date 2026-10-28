//! Measures the 150 columns per second `PWV7` series and the 2400 columns per second detail series of audio
//! files, prints the time each takes and the bytes of each, and writes `<name>.pwv7` (3 interleaved bytes per
//! column), `<name>.pwv6` (the 1200 column overview, same layout) and `<name>.ffwd`
//! (`DetailSeries::to_bytes`) for the zoom screenshots (`app/perf/zoom-detail-shots.mjs`).
//!
//! `waveform_detail_dump <out dir> <audio>...`
#[allow(dead_code)]
#[path = "../band_analysis.rs"]
mod band_analysis;

use std::path::Path;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("usage: waveform_detail_dump <out dir> <audio>...");
    std::fs::create_dir_all(&out).expect("create out dir");
    for audio in args {
        let audio = Path::new(&audio);
        let name = audio.file_stem().and_then(|s| s.to_str()).expect("file name");

        let started = Instant::now();
        let waves = band_analysis::measure_waveforms(audio).expect("measure waveforms");
        let pwv7_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let detail = band_analysis::measure_detail(audio).expect("measure detail");
        let detail_ms = started.elapsed().as_secs_f64() * 1000.0;

        let pwv7: Vec<u8> = waves.pwv7.iter().flatten().copied().collect();
        let bytes = detail.to_bytes();
        std::fs::write(Path::new(&out).join(format!("{name}.pwv7")), &pwv7).expect("write pwv7");
        std::fs::write(Path::new(&out).join(format!("{name}.ffwd")), &bytes).expect("write ffwd");
        let pwv6: Vec<u8> = waves.pwv6.iter().flatten().copied().collect();
        std::fs::write(Path::new(&out).join(format!("{name}.pwv6")), &pwv6).expect("write pwv6");
        println!(
            "{name}: {:.1} s, PWV7 {} columns {} bytes {pwv7_ms:.0} ms, detail {} columns {} bytes {detail_ms:.0} ms",
            waves.pwv7.len() as f64 / 150.0,
            waves.pwv7.len(),
            pwv7.len(),
            detail.len(),
            bytes.len(),
        );
    }
}
