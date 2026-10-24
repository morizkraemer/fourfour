use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, Track, WaveformPreview};
use pioneer_usb_writer::writer::filesystem::write_usb;
use tempfile::TempDir;

const PAGE: usize = 4096;

fn write_wav(path: &Path) {
    let sample_rate = 8000u32;
    let data_len = sample_rate * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.extend(std::iter::repeat(0u8).take(data_len as usize));
    std::fs::write(path, bytes).unwrap();
}

fn analysis() -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid { beats: vec![Beat { bar_position: 1, time_ms: 0, tempo: 12800 }] },
        waveform: WaveformPreview { data: [0xA0u8; 400] },
        bpm: 128.0,
        key: "8A".into(),
        energy: None,
        cue_points: vec![],
        color_waveform: None,
    }
}

fn synthetic_tracks(source: &Path, count: u32, text_len: usize) -> Vec<Track> {
    let wav = source.join("clip.wav");
    write_wav(&wav);
    let file_size = std::fs::metadata(&wav).unwrap().len();
    (1..=count)
        .map(|id| Track {
            usb_path: format!("/Contents/Artist {id}/clip{id}.wav"),
            source_path: wav.clone(),
            title: format!("{id:05} {}", "t".repeat(text_len)),
            artist: format!("{id:05} {}", "a".repeat(text_len)),
            album: format!("Album {id}"),
            genre: "House".into(),
            label: "Label".into(),
            remixer: String::new(),
            comment: String::new(),
            year: 2020,
            disc_number: 1,
            track_number: id,
            tempo: 12800,
            key: "8A".into(),
            duration_secs: 2.0,
            sample_rate: 8000,
            bitrate: 128,
            file_size,
            id,
            artwork: None,
        })
        .collect()
}

fn pdb_track_row_count(pdb: &[u8]) -> usize {
    assert_eq!(pdb.len() % PAGE, 0);
    let mut count = 0;
    for page in 1..pdb.len() / PAGE {
        let p = &pdb[page * PAGE..(page + 1) * PAGE];
        let table_type = u32::from_le_bytes(p[0x08..0x0C].try_into().unwrap());
        let is_header_page = p[0x1B] == 0x64;
        if table_type == 0x00 && !is_header_page {
            count += p[0x18] as usize;
        }
    }
    count
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "panic".into())
}

fn write_and_count(count: u32, text_len: usize) -> Result<usize, String> {
    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let tracks = synthetic_tracks(source.path(), count, text_len);
    let analyses = vec![analysis(); tracks.len()];
    let usb_path = usb.path().to_path_buf();
    let outcome = catch_unwind(AssertUnwindSafe(|| write_usb(&usb_path, &tracks, &analyses, &[])));
    match outcome {
        Err(panic) => Err(panic_message(panic)),
        Ok(Err(e)) => Err(format!("write_usb error: {e}")),
        Ok(Ok(_)) => {
            let pdb = std::fs::read(usb.path().join("PIONEER/rekordbox/export.pdb")).unwrap();
            Ok(pdb_track_row_count(&pdb))
        }
    }
}

fn assert_roundtrip(count: u32, text_len: usize) {
    let rows = write_and_count(count, text_len).unwrap_or_else(|e| panic!("{count} tracks (text_len {text_len}): {e}"));
    assert_eq!(rows, count as usize);
}

fn smallest_failing(text_len: usize) -> Option<u32> {
    let fails = |n: u32| write_and_count(n, text_len).map_or(true, |rows| rows != n as usize);
    if !fails(2000) {
        return None;
    }
    let (mut lo, mut hi) = (1u32, 2000u32);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if fails(mid) { hi = mid } else { lo = mid + 1 }
    }
    Some(lo)
}

#[test]
fn pdb_roundtrips_100_tracks() {
    assert_roundtrip(100, 0);
}

#[test]
#[ignore = "known bug: PDB split_into_pages wraps u16 heap offsets past 64 KiB, see docs/architecture/writer-bugs-2026-10.md"]
fn pdb_roundtrips_200_tracks() {
    assert_roundtrip(200, 0);
}

#[test]
#[ignore = "known bug: PDB split_into_pages wraps u16 heap offsets past 64 KiB, see docs/architecture/writer-bugs-2026-10.md"]
fn pdb_roundtrips_500_tracks() {
    assert_roundtrip(500, 0);
}

#[test]
#[ignore = "known bug: PDB split_into_pages wraps u16 heap offsets past 64 KiB, see docs/architecture/writer-bugs-2026-10.md"]
fn pdb_roundtrips_2000_tracks() {
    assert_roundtrip(2000, 0);
}

#[test]
#[ignore = "known bug: PDB split_into_pages wraps u16 heap offsets past 64 KiB, see docs/architecture/writer-bugs-2026-10.md"]
fn pdb_roundtrips_100_tracks_with_long_strings() {
    assert_roundtrip(100, 100);
}

#[test]
#[ignore = "measurement: prints the smallest failing track count; run with --nocapture"]
fn report_smallest_failing_track_count() {
    for text_len in [0usize, 30, 100, 200] {
        println!("text_len={text_len}: smallest failing N = {:?}", smallest_failing(text_len));
    }
}
