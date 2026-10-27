use std::path::Path;

use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, Playlist, Track, WaveformPreview};
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


fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn string_at(bytes: &[u8], at: usize) -> String {
    let marker = bytes[at];
    if marker == 0x03 {
        return String::new();
    }
    if marker == 0x40 {
        let total = u16_at(bytes, at + 1) as usize;
        return String::from_utf8(bytes[at + 4..at + total].to_vec()).unwrap();
    }
    assert_eq!(marker & 1, 1, "unexpected string marker {marker:#x}");
    let len = (marker >> 1) as usize - 1;
    String::from_utf8(bytes[at + 1..at + 1 + len].to_vec()).unwrap()
}

fn page_rows(page: &[u8], table_type: u32, page_index: u32) -> Vec<Vec<u8>> {
    let small = page[0x18] as usize;
    let num_rows = if small < 255 { small } else { u16_at(page, 0x22) as usize + 1 };
    let free = u16_at(page, 0x1C) as usize;
    let used = u16_at(page, 0x1E) as usize;
    let row_group_bytes = (num_rows / 16) * 36 + if num_rows % 16 > 0 { (num_rows % 16) * 2 + 4 } else { 0 };
    assert_eq!(0x28 + used + free + row_group_bytes, PAGE, "page {page_index} of table {table_type:#x}");

    let mut offsets = Vec::new();
    for group in 0..num_rows.div_ceil(16) {
        let rows_in_group = (num_rows - group * 16).min(16);
        let group_end = PAGE - group * 36;
        let flags = u16_at(page, group_end - 4);
        assert_eq!(flags, ((1u32 << rows_in_group) - 1) as u16);
        for r in 0..rows_in_group {
            offsets.push(u16_at(page, group_end - 4 - 2 * (r + 1)) as usize);
        }
    }
    assert!(offsets.windows(2).all(|w| w[0] < w[1]), "row offsets not increasing on page {page_index}");
    offsets
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = offsets.get(i + 1).copied().unwrap_or(used);
            assert!(end <= used, "row runs past heap on page {page_index}");
            page[0x28 + start..0x28 + end].to_vec()
        })
        .collect()
}

fn table_rows(pdb: &[u8], table_type: u32) -> Vec<Vec<u8>> {
    assert_eq!(pdb.len() % PAGE, 0);
    let pointer = (0..u32_at(pdb, 0x08) as usize)
        .map(|i| 0x1C + i * 16)
        .find(|&at| u32_at(pdb, at) == table_type)
        .unwrap();
    let empty_candidate = u32_at(pdb, pointer + 4);
    let header_page = u32_at(pdb, pointer + 8) as usize;
    let last_page = u32_at(pdb, pointer + 12);

    let mut rows = Vec::new();
    let mut page_index = u32_at(pdb, header_page * PAGE + 0x0C);
    let mut reached_last = false;
    while page_index != empty_candidate {
        assert!(!reached_last, "chain continues past last_page for table {table_type:#x}");
        reached_last = page_index == last_page;
        let page = &pdb[page_index as usize * PAGE..(page_index as usize + 1) * PAGE];
        assert_eq!(u32_at(page, 0x04), page_index);
        assert_eq!(u32_at(page, 0x08), table_type);
        rows.extend(page_rows(page, table_type, page_index));
        page_index = u32_at(page, 0x0C);
    }
    assert!(reached_last, "chain did not reach last_page for table {table_type:#x}");
    rows
}

fn assert_roundtrip(count: u32, text_len: usize) {
    let playlists = vec![
        Playlist { id: 1, name: "All".into(), track_ids: (1..=count).collect() },
        Playlist { id: 2, name: "Evens".into(), track_ids: (1..=count).filter(|id| id % 2 == 0).collect() },
    ];
    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let tracks = synthetic_tracks(source.path(), count, text_len);
    let analyses = vec![analysis(); tracks.len()];
    write_usb(usb.path(), &tracks, &analyses, &playlists).unwrap();
    let pdb = std::fs::read(usb.path().join("PIONEER/rekordbox/export.pdb")).unwrap();

    let track_rows = table_rows(&pdb, 0x00);
    assert_eq!(track_rows.len(), count as usize);
    for (row, track) in track_rows.iter().zip(&tracks) {
        assert_eq!(u32_at(row, 0x48), track.id);
        assert_eq!(string_at(row, u16_at(row, 0x5E + 17 * 2) as usize), track.title);
        assert_eq!(string_at(row, u16_at(row, 0x5E + 20 * 2) as usize), track.usb_path);
    }

    let artist_rows = table_rows(&pdb, 0x02);
    assert_eq!(artist_rows.len(), count as usize);
    for (row, track) in artist_rows.iter().zip(&tracks) {
        assert_eq!(string_at(row, 0x0A), track.artist);
    }
    assert_eq!(table_rows(&pdb, 0x03).len(), count as usize);

    let tree: Vec<(u32, String)> = table_rows(&pdb, 0x07)
        .iter()
        .map(|r| (u32_at(r, 12), string_at(r, 20)))
        .collect();
    assert_eq!(tree, vec![(1, "All".to_string()), (2, "Evens".to_string())]);

    let entries: Vec<(u32, u32)> = table_rows(&pdb, 0x08)
        .iter()
        .map(|r| (u32_at(r, 8), u32_at(r, 4)))
        .collect();
    let expected: Vec<(u32, u32)> = playlists
        .iter()
        .flat_map(|p| p.track_ids.iter().map(move |&t| (p.id, t)))
        .collect();
    assert_eq!(entries, expected);
}

#[test]
fn pdb_roundtrips_100_tracks() {
    assert_roundtrip(100, 0);
}

#[test]
fn pdb_roundtrips_200_tracks() {
    assert_roundtrip(200, 0);
}

#[test]
fn pdb_roundtrips_1000_tracks() {
    assert_roundtrip(1000, 0);
}

#[test]
fn pdb_roundtrips_5000_tracks() {
    assert_roundtrip(5000, 0);
}

#[test]
fn pdb_roundtrips_300_tracks_with_long_strings() {
    assert_roundtrip(300, 200);
}
