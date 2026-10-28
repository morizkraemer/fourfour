use std::path::{Path, PathBuf};
use std::process::Command;

use fourfour_analysis::decode::decode_mono;
use fourfour_analysis::service::{locate_models, Analyzer, PoolOptions};

const RATE: u32 = 44_100;
const SECONDS: f64 = 12.0;
const BPM: f64 = 120.0;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ff-truncated-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn kick_pcm() -> Vec<i16> {
    let beat = RATE as f64 * 60.0 / BPM;
    (0..(RATE as f64 * SECONDS) as usize)
        .map(|i| {
            let since = (i as f64 % beat) / RATE as f64;
            let kick = (-since * 25.0).exp() * (2.0 * std::f64::consts::PI * (50.0 + 80.0 * (-since * 40.0).exp()) * since).sin();
            (kick * 0.8 * i16::MAX as f64) as i16
        })
        .collect()
}

fn write_wav(path: &Path, pcm: &[i16]) {
    let data_len = (pcm.len() * 2) as u32;
    let mut bytes = Vec::new();
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

fn encode_with_ffmpeg(wav: &Path, out: &Path) -> bool {
    Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(wav)
        .arg(out)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn cut_copy(src: &Path, dst: &Path, keep_fraction: f64) {
    let bytes = std::fs::read(src).unwrap();
    let keep = (bytes.len() as f64 * keep_fraction) as usize;
    std::fs::write(dst, &bytes[..keep]).unwrap();
}

fn assert_decodes_cut(src: &Path, name: &str, must_flag: bool) {
    for (i, fraction) in [0.37, 0.61, 0.83, 0.97].into_iter().enumerate() {
        let cut = src.with_file_name(format!("cut{i}-{name}"));
        cut_copy(src, &cut, fraction);
        let mono = decode_mono(&cut).unwrap_or_else(|e| panic!("{name} cut at {fraction}: {e}"));
        assert!(mono.truncated || !must_flag, "{name} cut at {fraction} not flagged");
        let seconds = mono.samples.len() as f64 / mono.rate as f64;
        assert!(seconds > 1.0 && seconds < SECONDS, "{name} cut at {fraction}: {seconds}");
    }
}

#[test]
fn intact_wav_is_not_flagged() {
    let dir = scratch("intact");
    let wav = dir.join("full.wav");
    write_wav(&wav, &kick_pcm());
    let mono = decode_mono(&wav).unwrap();
    assert!(!mono.truncated);
    assert_eq!(mono.samples.len(), (RATE as f64 * SECONDS) as usize);
}

#[test]
fn cut_wav_keeps_the_decoded_part() {
    let dir = scratch("wav");
    let wav = dir.join("full.wav");
    write_wav(&wav, &kick_pcm());
    assert_decodes_cut(&wav, "full.wav", true);
}

#[test]
fn cut_flac_and_mp3_keep_the_decoded_part() {
    let dir = scratch("coded");
    let wav = dir.join("full.wav");
    write_wav(&wav, &kick_pcm());
    for name in ["full.flac", "full.mp3"] {
        let coded = dir.join(name);
        if !encode_with_ffmpeg(&wav, &coded) {
            eprintln!("skipping {name}: ffmpeg unavailable");
            continue;
        }
        assert_decodes_cut(&coded, name, name.ends_with("flac"));
    }
}

#[test]
fn zero_length_and_header_only_files_error_cleanly() {
    let dir = scratch("empty");
    let empty = dir.join("empty.wav");
    std::fs::write(&empty, b"").unwrap();
    assert!(decode_mono(&empty).is_err());

    let wav = dir.join("full.wav");
    write_wav(&wav, &kick_pcm());
    let header_only = dir.join("header.wav");
    cut_copy(&wav, &header_only, 0.0001);
    assert!(decode_mono(&header_only).is_err());

    let under_a_second = dir.join("short.wav");
    write_wav(&under_a_second, &kick_pcm()[..RATE as usize / 2]);
    let bytes = std::fs::read(&under_a_second).unwrap();
    std::fs::write(&under_a_second, &bytes[..bytes.len() - 10]).unwrap();
    assert!(decode_mono(&under_a_second).is_err());
}

#[test]
#[ignore = "needs model weights (prototyping/analysis-packaging/fetch-models.sh)"]
fn cut_wav_still_gets_a_bpm_and_grid() {
    let models = locate_models(None).expect("model weights");
    let dir = scratch("analyse");
    let wav = dir.join("full.wav");
    write_wav(&wav, &kick_pcm());
    let cut = dir.join("cut.wav");
    cut_copy(&wav, &cut, 0.7);
    let analyzer = Analyzer::load(&models, PoolOptions::default()).unwrap();
    let result = analyzer.analyze(&cut).expect("analysed");
    assert!((result.bpm - BPM).abs() < BPM * 0.02, "bpm {}", result.bpm);
    assert!(!result.beat_grid.beats.is_empty());
}
