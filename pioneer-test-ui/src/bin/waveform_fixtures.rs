//! Side-by-side fixtures for the waveform comparison page (`app/compare.html`).
//!
//! Input: a folder of `NN.json` written by `waveform_compare fixtures` (audio path and the
//! Rekordbox PWV7 / PWV6 / PWAV bytes of the same track, plus its beat grid).
//! Output: one `NN.json` per track with the `get_analysis_data` colour fields built from our
//! analysis (measured from the audio, stored in a temporary library, read back through the
//! app's payload code) and from Rekordbox's bytes through the same code, plus the numbers that
//! compare the two byte streams.
#[allow(dead_code)]
#[path = "../analysis_payload.rs"]
mod analysis_payload;
#[allow(dead_code)]
#[path = "../band_analysis.rs"]
mod band_analysis;
#[allow(dead_code)]
#[path = "../dto.rs"]
mod dto;
#[allow(dead_code)]
#[path = "../track_infos.rs"]
mod track_infos;

use std::path::{Path, PathBuf};
use std::time::Instant;

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models::{AnalysisResult, BeatGrid, ColorWaveform, Track};

fn triples(bytes: &[u8]) -> Vec<[u8; 3]> {
    bytes.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

fn bytes_of(value: &serde_json::Value) -> Vec<u8> {
    value.as_array().expect("byte array").iter().map(|v| v.as_u64().unwrap() as u8).collect()
}

fn track(duration_secs: f64, source: &Path) -> Track {
    Track {
        id: 0,
        source_path: source.to_path_buf(),
        usb_path: "/Contents/fixture.mp3".to_string(),
        title: "Fixture".to_string(),
        artist: "Fixture".to_string(),
        album: String::new(),
        genre: String::new(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2025,
        disc_number: 1,
        track_number: 1,
        tempo: 12_400,
        key: "8A".to_string(),
        duration_secs,
        sample_rate: 44_100,
        bitrate: 320,
        file_size: 1,
        artwork: None,
    }
}

struct Series {
    mae: f64,
    corr: f64,
}

fn compare(ours: &[[u8; 3]], theirs: &[[u8; 3]], band: usize) -> Series {
    let n = ours.len().min(theirs.len()) as f64;
    let (mut abs, mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (a, b) in ours.iter().zip(theirs) {
        let (x, y) = (f64::from(a[band]), f64::from(b[band]));
        abs += (x - y).abs();
        sx += x;
        sy += y;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
    }
    let cov = sxy / n - (sx / n) * (sy / n);
    let var = (sxx / n - (sx / n).powi(2)) * (syy / n - (sy / n).powi(2));
    Series { mae: abs / n, corr: if var > 0.0 { cov / var.sqrt() } else { 0.0 } }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (PathBuf::from(&args[1]), PathBuf::from(&args[2]));
    std::fs::create_dir_all(&output).unwrap();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&input)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();

    for file in files {
        let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let name = fixture["name"].as_str().unwrap().to_string();
        let audio = PathBuf::from(fixture["audio"].as_str().unwrap());
        let rb_pwv7 = triples(&bytes_of(&fixture["pwv7"]));
        let rb_pwv6 = triples(&bytes_of(&fixture["pwv6"]));
        let rb_pwav = bytes_of(&fixture["pwav"]);

        let started = Instant::now();
        let waves = band_analysis::measure_waveforms(&audio).unwrap();
        let measure_ms = started.elapsed().as_secs_f64() * 1000.0;
        let duration_secs = waves.pwv7.len() as f64 / 150.0;

        let work = std::env::temp_dir().join(format!("ff-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&work).unwrap();
        let lib = LocalLibrary::open(&work.join("library.db")).unwrap();
        let id = lib.add_track(&track(duration_secs, &audio)).unwrap();
        let analysis = AnalysisResult {
            beat_grid: BeatGrid { beats: Vec::new() },
            waveform: waves.preview().expect("400-column preview"),
            bpm: 0.0,
            key: String::new(),
            energy: None,
            cue_points: Vec::new(),
            color_waveform: Some(waves.color_waveform()),
        };
        lib.set_analysis(id, &analysis).unwrap();
        let ours = analysis_payload::build_analysis_json(&lib, id).unwrap();
        let load_state_row = track_infos::build_track_infos(&lib)
            .unwrap()
            .remove(0)
            .waveform_bands
            .map(|bytes| dto::base64_encode(&bytes));
        std::fs::remove_dir_all(&work).ok();

        let rb_cw = ColorWaveform { detail: rb_pwv7.clone(), overview: rb_pwv6.clone(), exact: None };
        let rekordbox = serde_json::Value::Object(analysis_payload::waveform_fields(Some(&rb_cw), Some(&rb_pwav), true));

        let stats = |ours: &[[u8; 3]], theirs: &[[u8; 3]]| {
            (0..3)
                .map(|band| {
                    let s = compare(ours, theirs, band);
                    serde_json::json!({ "mae": s.mae, "corr": s.corr })
                })
                .collect::<Vec<_>>()
        };
        let out = serde_json::json!({
            "name": name,
            "duration_ms": (duration_secs * 1000.0).round(),
            "beats": fixture["beats"],
            "ours": { "analysis": ours, "load_state_row": load_state_row },
            "rekordbox": { "analysis": rekordbox },
            "columns": { "ours": waves.pwv7.len(), "rekordbox": rb_pwv7.len() },
            "pwv7": stats(&waves.pwv7, &rb_pwv7),
            "pwv6": stats(&waves.pwv6, &rb_pwv6),
            "measure_ms": measure_ms,
        });
        let path = output.join(file.file_name().unwrap());
        std::fs::write(&path, serde_json::to_vec(&out).unwrap()).unwrap();
        println!(
            "{} {} measure {:.0} ms, PWV7 corr {:.3}/{:.3}/{:.3}",
            path.display(),
            name,
            measure_ms,
            out["pwv7"][0]["corr"].as_f64().unwrap(),
            out["pwv7"][1]["corr"].as_f64().unwrap(),
            out["pwv7"][2]["corr"].as_f64().unwrap(),
        );
    }
}
