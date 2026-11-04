use std::path::{Path, PathBuf};

use fourfour_analysis::decode::{decode_mono, decode_mono_measured};

fn write_wav(path: &Path, rate: u32, channels: u16, pcm: &[i16]) {
    let data_len = (pcm.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2 * channels as u32).to_le_bytes());
    bytes.extend_from_slice(&(2 * channels).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ff-loudness-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn sine_pcm(rate: u32, channels: usize, dbfs: f64, seconds: f64) -> Vec<i16> {
    let amplitude = 10f64.powf(dbfs / 20.0) * i16::MAX as f64;
    (0..(rate as f64 * seconds) as usize)
        .flat_map(|i| {
            let v = (amplitude * (2.0 * std::f64::consts::PI * 997.0 * i as f64 / rate as f64).sin()) as i16;
            std::iter::repeat(v).take(channels)
        })
        .collect()
}

#[test]
fn decoded_wav_is_measured_without_changing_the_samples() {
    let dir = scratch("tones");
    for (rate, channels, expected) in [(44_100, 2u16, -26.0), (48_000, 2, -26.0), (44_100, 1, -29.01)] {
        let path = dir.join(format!("tone-{rate}-{channels}.wav"));
        write_wav(&path, rate, channels, &sine_pcm(rate, channels as usize, -26.0, 10.0));
        let plain = decode_mono(&path).unwrap();
        let measured = decode_mono_measured(&path, true).unwrap();
        assert_eq!(plain.samples, measured.samples);
        assert_eq!(plain.gapless, measured.gapless);
        assert!(plain.loudness.is_none());
        let l = measured.loudness.expect("loudness");
        assert!((l.lufs - expected).abs() < 0.05, "{rate}/{channels}: {}", l.lufs);
        assert!((l.gain_db - (-14.0 - l.lufs)).abs() < 1e-9);
        assert!((l.peak_db + 26.0).abs() < 0.1, "{}", l.peak_db);
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn silent_wav_has_no_loudness() {
    let dir = scratch("silence");
    let path = dir.join("silence.wav");
    write_wav(&path, 44_100, 2, &vec![0i16; 44_100 * 2 * 3]);
    let measured = decode_mono_measured(&path, true).unwrap();
    assert!(measured.loudness.is_none());
    std::fs::remove_dir_all(&dir).ok();
}
