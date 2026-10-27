use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use fourfour_analysis::service::{analyze_batch, locate_models, models_present, Analyzer, PoolOptions};

const RATE: u32 = 44_100;

fn kick_track(path: &Path, bpm: f64, seconds: f64) {
    let samples = (RATE as f64 * seconds) as usize;
    let beat = RATE as f64 * 60.0 / bpm;
    let mut pcm: Vec<i16> = Vec::with_capacity(samples);
    for i in 0..samples {
        let since = (i as f64 % beat) / RATE as f64;
        let kick = (-since * 25.0).exp() * (2.0 * std::f64::consts::PI * (50.0 + 80.0 * (-since * 40.0).exp()) * since).sin();
        let hat = if (i as f64 % (beat / 2.0)) < 200.0 { 0.1 * ((i * 7919) % 200) as f64 / 100.0 - 0.1 } else { 0.0 };
        pcm.push(((kick * 0.8 + hat) * i16::MAX as f64) as i16);
    }
    let data_len = (pcm.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ff-generated-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn loading_without_weights_fails_with_the_directory_in_the_message() {
    let empty = scratch().join("no-weights");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(!models_present(&empty));
    let err = match Analyzer::load(&empty, PoolOptions::default()) {
        Ok(_) => panic!("load must fail without weights"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("onnx") || err.contains("No such file") || err.contains("mel_spectrogram"), "{err}");
}

#[test]
#[ignore = "needs model weights (prototyping/analysis-packaging/fetch-models.sh)"]
fn two_generated_tracks_analyse_in_a_batch() {
    let models = locate_models(None).expect("model weights");
    let dir = scratch();
    let tracks = [(dir.join("a.wav"), 120.0, 30.0), (dir.join("b.wav"), 128.0, 30.0)];
    for (path, bpm, seconds) in &tracks {
        kick_track(path, *bpm, *seconds);
    }
    let paths: Vec<PathBuf> = tracks.iter().map(|t| t.0.clone()).collect();
    let analyzer = Analyzer::load(&models, PoolOptions { max_threads: 2, ..PoolOptions::default() }).unwrap();
    let progress = AtomicUsize::new(0);
    let results = analyze_batch(&analyzer, &paths, &AtomicBool::new(false), |event| {
        assert!(event.result.is_ok());
        progress.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(progress.load(Ordering::SeqCst), 2);
    for ((_, bpm, seconds), result) in tracks.iter().zip(results) {
        let r = result.expect("ran").expect("analysed");
        assert!((r.bpm - bpm).abs() < bpm * 0.02, "bpm {} vs {bpm}", r.bpm);
        let expected_beats = (seconds * bpm / 60.0) as usize;
        assert!(r.beat_grid.beats.len().abs_diff(expected_beats) <= 2);
        assert!(r.beat_grid.beats.iter().all(|b| (1..=4).contains(&b.bar_position)));
        assert!(r.waveform.data.iter().any(|b| *b != 0));
        assert!(r.color_waveform.as_ref().unwrap().detail.len().abs_diff((seconds * 150.0) as usize) <= 2);
    }
}

#[test]
#[ignore = "needs model weights (prototyping/analysis-packaging/fetch-models.sh)"]
fn cancelled_batch_runs_nothing() {
    let models = locate_models(None).expect("model weights");
    let dir = scratch();
    kick_track(&dir.join("c.wav"), 124.0, 10.0);
    let analyzer = Analyzer::load(&models, PoolOptions::default()).unwrap();
    let results = analyze_batch(&analyzer, &[dir.join("c.wav")], &AtomicBool::new(true), |_| panic!("no progress when cancelled"));
    assert!(results[0].is_none());
}
