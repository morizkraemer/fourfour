use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, ColorWaveform, Track, WaveformPreview,
};
use pioneer_usb_writer::writer::anlz::{write_anlz_2ex, write_anlz_ext};
use tempfile::TempDir;

const SECTION_HEADER_LEN: usize = 24;
const FILE_HEADER_LEN: usize = 0x1C;

fn track(duration_secs: f64) -> Track {
    Track {
        usb_path: "/Contents/Long/long.wav".into(),
        source_path: PathBuf::from("long.wav"),
        title: "Long".into(),
        artist: "Long".into(),
        album: String::new(),
        genre: String::new(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2020,
        disc_number: 1,
        track_number: 1,
        tempo: 12800,
        key: "8A".into(),
        duration_secs,
        sample_rate: 44100,
        bitrate: 320,
        file_size: 0,
        id: 1,
        artwork: None,
    }
}

fn detail_value(i: usize) -> [u8; 3] {
    [(i % 127) as u8 + 1, (i % 113) as u8, (i % 101) as u8]
}

fn analysis(entry_count: usize) -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid { beats: vec![Beat { bar_position: 1, time_ms: 0, tempo: 12800 }] },
        waveform: WaveformPreview { data: [0xA0u8; 400] },
        bpm: 128.0,
        key: "8A".into(),
        energy: None,
        cue_points: vec![],
        color_waveform: Some(ColorWaveform {
            detail: (0..entry_count).map(detail_value).collect(),
            overview: vec![[10, 20, 30]; 1200],
        }),
    }
}

fn entry_count(duration_secs: f64) -> usize {
    (duration_secs * 150.0).round() as usize
}

fn section<'a>(file: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
    let mut at = FILE_HEADER_LEN;
    while at + 12 <= file.len() {
        let total = u32::from_be_bytes(file[at + 8..at + 12].try_into().unwrap()) as usize;
        if &file[at..at + 4] == tag {
            return &file[at..at + total];
        }
        at += total;
    }
    panic!("section {} not found", String::from_utf8_lossy(tag));
}

fn write_ext(duration_secs: f64) -> Result<Vec<u8>, String> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("long.EXT");
    let analysis = analysis(entry_count(duration_secs));
    let track = track(duration_secs);
    let outcome = catch_unwind(AssertUnwindSafe(|| write_anlz_ext(&path, &track, &analysis)));
    match outcome {
        Err(panic) => Err(panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "panic".into())),
        Ok(Err(e)) => Err(format!("write_anlz_ext error: {e}")),
        Ok(Ok(())) => Ok(std::fs::read(&path).unwrap()),
    }
}

fn write_2ex(duration_secs: f64, detail_len: usize) -> Vec<u8> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("long.2EX");
    let written = write_anlz_2ex(&path, &track(duration_secs), &analysis(detail_len)).unwrap();
    assert!(written);
    std::fs::read(&path).unwrap()
}

fn assert_long_detail_is_sane(duration_secs: f64) {
    let entries = entry_count(duration_secs);
    let file = write_ext(duration_secs).unwrap_or_else(|e| panic!("{duration_secs}s track: {e}"));

    let pwv5 = section(&file, b"PWV5");
    assert_eq!(pwv5.len(), SECTION_HEADER_LEN + entries * 2);
    assert_eq!(u32::from_be_bytes(pwv5[16..20].try_into().unwrap()) as usize, entries);
    let pwv3 = section(&file, b"PWV3");
    assert_eq!(pwv3.len(), SECTION_HEADER_LEN + entries);

    let two_ex = write_2ex(duration_secs, entries);
    let pwv7 = section(&two_ex, b"PWV7");
    assert_eq!(pwv7.len(), SECTION_HEADER_LEN + entries * 3);
    assert_eq!(u32::from_be_bytes(pwv7[16..20].try_into().unwrap()) as usize, entries);
    for i in 0..entries {
        let at = SECTION_HEADER_LEN + i * 3;
        assert_eq!(pwv7[at..at + 3], detail_value(i), "PWV7 entry {i}");
    }
}

fn assert_stretched_detail_follows_source(duration_secs: f64) {
    let entries = entry_count(duration_secs);
    let source_len = entries / 2 + 1;
    let two_ex = write_2ex(duration_secs, source_len);
    let pwv7 = section(&two_ex, b"PWV7");
    for i in 0..entries {
        let source = (i as u64 * source_len as u64 / entries as u64) as usize;
        let at = SECTION_HEADER_LEN + i * 3;
        assert_eq!(pwv7[at..at + 3], detail_value(source), "PWV7 entry {i}");
    }
}

#[test]
fn color_detail_is_correct_for_5_minute_track() {
    assert_long_detail_is_sane(300.0);
}

#[test]
fn color_detail_is_correct_for_10_minute_track() {
    assert_long_detail_is_sane(600.0);
}

#[test]
fn color_detail_is_correct_for_20_minute_track() {
    assert_long_detail_is_sane(1200.0);
}

#[test]
fn stretched_detail_follows_source_for_10_minute_track() {
    assert_stretched_detail_follows_source(600.0);
}

#[test]
fn stretched_detail_follows_source_for_60_minute_track() {
    assert_stretched_detail_follows_source(3600.0);
}

#[test]
#[ignore = "measurement: prints the shortest failing duration in seconds; run with --nocapture"]
fn report_shortest_failing_duration() {
    let fails = |secs: u32| write_ext(secs as f64).is_err();
    let (mut lo, mut hi) = (60u32, 1200u32);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if fails(mid) { hi = mid } else { lo = mid + 1 }
    }
    println!("shortest failing whole-second duration = {lo} s ({} entries)", entry_count(lo as f64));
}
