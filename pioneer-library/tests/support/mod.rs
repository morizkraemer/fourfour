#![allow(dead_code)]

use std::f32::consts::TAU;
use std::path::Path;

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, WaveformPreview};
use pioneer_usb_writer::scanner;

pub const SAMPLE_RATE: u32 = 44_100;

pub fn write_wav(path: &Path, frequency: f32, seconds: f32) {
    let sample_count = (SAMPLE_RATE as f32 * seconds) as u32;
    let data_len = sample_count * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for n in 0..sample_count {
        let t = n as f32 / SAMPLE_RATE as f32;
        let sample = ((t * frequency * TAU).sin() * 8_000.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

pub fn make_analysis() -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid {
            beats: vec![
                Beat { bar_position: 1, time_ms: 0, tempo: 12800 },
                Beat { bar_position: 2, time_ms: 469, tempo: 12800 },
            ],
        },
        waveform: WaveformPreview { data: [0u8; 400] },
        bpm: 128.0,
        key: "1A".to_string(),
        energy: Some(5),
        cue_points: vec![],
        color_waveform: None,
    }
}

/// Generates `count` short WAVs spread over `count / 50 + 1` artist folders,
/// imports them as analyzed tracks and returns their library ids.
pub fn populate_library(
    library: &LocalLibrary,
    folder: &Path,
    count: usize,
    seconds: f32,
) -> Vec<i64> {
    for i in 0..count {
        let artist = format!("artist{:02}", i / 50);
        write_wav(
            &folder.join(&artist).join(format!("track{i:04}.wav")),
            110.0 + (i % 40) as f32 * 11.0,
            seconds,
        );
    }
    let tracks = scanner::scan_directory(folder).unwrap();
    let ids = library.add_tracks(&tracks).unwrap();
    for id in &ids {
        library.set_analysis(*id, &make_analysis()).unwrap();
    }
    ids
}
