use std::path::{Path, PathBuf};

use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, CuePoint, Playlist, SyncReport, Track, WaveformPreview,
};
use pioneer_usb_writer::reader::{anlz::read_anlz, read_usb_state};
use pioneer_usb_writer::writer::anlz::{anlz_ext_path_for_track, anlz_path_for_track};
use pioneer_usb_writer::writer::filesystem::{sync_usb, write_usb};
use rusqlite::Connection;
use tempfile::TempDir;

const PAGE: usize = 4096;
const ONELIBRARY_KEY: &str = "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls";

fn write_wav(path: &Path, seconds: u32) {
    let sample_rate = 8000u32;
    let data_len = sample_rate * seconds * 2;
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
    for i in 0..sample_rate * seconds {
        let sample = ((i as f32 * 440.0 * std::f32::consts::TAU / sample_rate as f32).sin() * 8000.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

fn cover_png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(32, 32, image::Rgb([200, 30, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

struct Fixture {
    _source: TempDir,
    usb: TempDir,
    tracks: Vec<Track>,
    analyses: Vec<AnalysisResult>,
    playlists: Vec<Playlist>,
}

fn track(source_dir: &Path, id: u32, file: &str, title: &str, artist: &str, key: &str, tempo: u32) -> Track {
    let source_path = source_dir.join(file);
    write_wav(&source_path, 2);
    Track {
        usb_path: format!("/Contents/{artist}/{file}"),
        source_path: source_path.clone(),
        title: title.into(),
        artist: artist.into(),
        album: format!("{artist} LP"),
        genre: "House".into(),
        label: "Test Label".into(),
        remixer: String::new(),
        comment: String::new(),
        year: 2020,
        disc_number: 1,
        track_number: id,
        tempo,
        key: key.into(),
        duration_secs: 2.0,
        sample_rate: 8000,
        bitrate: 128,
        file_size: std::fs::metadata(&source_path).unwrap().len(),
        id,
        artwork: None,
    }
}

fn analysis(tempo: u32, key: &str, cues: Vec<CuePoint>) -> AnalysisResult {
    let beat_ms = 6_000_000 / tempo;
    let beats = (0..8)
        .map(|i| Beat { bar_position: (i % 4) as u8 + 1, time_ms: i * beat_ms, tempo })
        .collect();
    let mut data = [0u8; 400];
    for (i, b) in data.iter_mut().enumerate() {
        *b = (i % 32) as u8 | 0xA0;
    }
    AnalysisResult {
        beat_grid: BeatGrid { beats },
        waveform: WaveformPreview { data },
        bpm: tempo as f64 / 100.0,
        key: key.into(),
        energy: None,
        cue_points: cues,
        color_waveform: None,
    }
}

fn fixture() -> Fixture {
    let source = TempDir::new().unwrap();
    let mut t1 = track(source.path(), 1, "one.wav", "First Light", "Alpha", "8A", 12800);
    t1.artwork = Some(cover_png());
    let t2 = track(source.path(), 2, "two.wav", "Second Wind", "Beta", "5B", 12400);
    let t3 = track(source.path(), 3, "three.wav", "Third Rail", "Alpha", "", 13000);
    let cues = vec![
        CuePoint { hot_cue_number: 0, time_ms: 500, loop_time_ms: None },
        CuePoint { hot_cue_number: 1, time_ms: 1000, loop_time_ms: None },
        CuePoint { hot_cue_number: 2, time_ms: 1500, loop_time_ms: Some(1900) },
    ];
    let analyses = vec![analysis(12800, "8A", cues), analysis(12400, "5B", vec![]), analysis(13000, "", vec![])];
    let playlists = vec![
        Playlist { id: 1, name: "Warmup".into(), track_ids: vec![2, 1] },
        Playlist { id: 2, name: "Peak".into(), track_ids: vec![3] },
    ];
    Fixture {
        _source: source,
        usb: TempDir::new().unwrap(),
        tracks: vec![t1, t2, t3],
        analyses,
        playlists,
    }
}

fn exported() -> Fixture {
    let f = fixture();
    write_usb(f.usb.path(), &f.tracks, &f.analyses, &f.playlists).unwrap();
    f
}

fn u16le(b: &[u8], at: usize) -> usize {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap()) as usize
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn decode_devicesql_string(b: &[u8]) -> String {
    match b[0] {
        0x40 => {
            let len = u16le(b, 1);
            String::from_utf8(b[4..len].to_vec()).unwrap()
        }
        0x90 => {
            let len = u16le(b, 1);
            let units: Vec<u16> = b[4..len].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            String::from_utf16(&units).unwrap()
        }
        header => {
            let len = (header >> 1) as usize - 1;
            String::from_utf8(b[1..1 + len].to_vec()).unwrap()
        }
    }
}

struct PdbRow {
    bytes: Vec<u8>,
}

struct Pdb {
    bytes: Vec<u8>,
}

impl Pdb {
    fn open(usb: &Path) -> Self {
        let bytes = std::fs::read(usb.join("PIONEER/rekordbox/export.pdb")).unwrap();
        assert_eq!(bytes.len() % PAGE, 0);
        Pdb { bytes }
    }

    /// Rows of all data pages of `table_type`, in page order. Page layout: 0x28-byte
    /// page header (row count at 0x18, flags at 0x1B), heap from 0x28, and row groups
    /// growing down from the page end: per group of up to 16 rows, 36 bytes holding the
    /// row offsets in reverse order, then a u16 presence bitmask and a u16 unknown.
    fn rows(&self, table_type: u32) -> Vec<PdbRow> {
        let mut rows = Vec::new();
        for page in 1..self.bytes.len() / PAGE {
            let p = &self.bytes[page * PAGE..(page + 1) * PAGE];
            let is_header_page = p[0x1B] == 0x64;
            if u32le(p, 0x08) != table_type || is_header_page {
                continue;
            }
            for i in 0..p[0x18] as usize {
                let group_end = PAGE - (i / 16) * 36;
                let offset = u16le(p, group_end - 4 - 2 * (i % 16 + 1));
                rows.push(PdbRow { bytes: p[0x28 + offset..].to_vec() });
            }
        }
        rows
    }
}

fn track_string(row: &PdbRow, string_index: usize) -> String {
    let at = u16le(&row.bytes, 0x5E + string_index * 2);
    decode_devicesql_string(&row.bytes[at..])
}

fn onelibrary(usb: &Path) -> Connection {
    let conn = Connection::open(usb.join("PIONEER/rekordbox/exportLibrary.db")).unwrap();
    conn.execute_batch(&format!("PRAGMA key = '{ONELIBRARY_KEY}';")).unwrap();
    conn
}

fn anlz_dat(usb: &Path, track: &Track) -> PathBuf {
    usb.join(anlz_path_for_track(track))
}

#[test]
fn write_usb_creates_expected_layout() {
    let f = exported();
    let usb = f.usb.path();
    for t in &f.tracks {
        assert!(usb.join(t.usb_path.trim_start_matches('/')).is_file(), "{}", t.usb_path);
        assert!(anlz_dat(usb, t).is_file());
        assert!(usb.join(anlz_ext_path_for_track(t)).is_file());
    }
    assert!(usb.join("PIONEER/rekordbox/export.pdb").is_file());
    assert!(usb.join("PIONEER/rekordbox/exportLibrary.db").is_file());
}

#[test]
fn write_usb_rejects_mismatched_track_and_analysis_counts() {
    let f = fixture();
    let err = write_usb(f.usb.path(), &f.tracks, &f.analyses[..2], &f.playlists).unwrap_err();
    assert!(err.to_string().contains("same length"), "{err}");
}

#[test]
fn pdb_header_declares_20_tables_and_page_size() {
    let f = exported();
    let pdb = Pdb::open(f.usb.path());
    assert_eq!(u32le(&pdb.bytes, 0x04), PAGE as u32);
    assert_eq!(u32le(&pdb.bytes, 0x08), 20);
    assert_eq!(u32le(&pdb.bytes, 0x0C) as usize, pdb.bytes.len() / PAGE);
}

#[test]
fn pdb_tracks_roundtrip() {
    let f = exported();
    let rows = Pdb::open(f.usb.path()).rows(0x00);
    assert_eq!(rows.len(), 3);
    for (row, t) in rows.iter().zip(&f.tracks) {
        assert_eq!(u32le(&row.bytes, 0x48), t.id);
        assert_eq!(u32le(&row.bytes, 0x38), t.tempo);
        assert_eq!(u32le(&row.bytes, 0x08), t.sample_rate);
        assert_eq!(u32le(&row.bytes, 0x10) as u64, t.file_size);
        assert_eq!(track_string(row, 17), t.title);
        assert_eq!(track_string(row, 20), t.usb_path);
    }
    let key_ids: Vec<u32> = rows.iter().map(|r| u32le(&r.bytes, 0x20)).collect();
    assert_eq!(key_ids, vec![2, 14, 0]);
}

#[test]
fn pdb_artists_are_deduplicated_and_referenced_by_tracks() {
    let f = exported();
    let pdb = Pdb::open(f.usb.path());
    let artists: Vec<(u32, String)> = pdb
        .rows(0x02)
        .iter()
        .map(|r| (u32le(&r.bytes, 4), decode_devicesql_string(&r.bytes[10..])))
        .collect();
    assert_eq!(artists, vec![(1, "Alpha".to_string()), (2, "Beta".to_string())]);
    let track_artist_ids: Vec<u32> = pdb.rows(0x00).iter().map(|r| u32le(&r.bytes, 0x44)).collect();
    assert_eq!(track_artist_ids, vec![1, 2, 1]);
}

#[test]
fn pdb_playlists_roundtrip() {
    let f = exported();
    let pdb = Pdb::open(f.usb.path());
    let playlists: Vec<(u32, String)> = pdb
        .rows(0x07)
        .iter()
        .map(|r| (u32le(&r.bytes, 12), decode_devicesql_string(&r.bytes[20..])))
        .collect();
    assert_eq!(playlists, vec![(1, "Warmup".to_string()), (2, "Peak".to_string())]);

    let entries: Vec<(u32, u32)> = pdb
        .rows(0x08)
        .iter()
        .map(|r| (u32le(&r.bytes, 8), u32le(&r.bytes, 4)))
        .collect();
    assert_eq!(entries, vec![(1, 2), (1, 1), (2, 3)]);
}

#[test]
fn pdb_artwork_row_exists_only_for_tracks_with_cover() {
    let f = exported();
    let rows = Pdb::open(f.usb.path()).rows(0x0D);
    assert_eq!(rows.len(), 1);
    assert_eq!(u32le(&rows[0].bytes, 0), 1);
    assert_eq!(decode_devicesql_string(&rows[0].bytes[4..]), "/PIONEER/Artwork/00001/a1.jpg");
}

#[test]
fn artwork_files_are_written_at_both_sizes() {
    let f = exported();
    let dir = f.usb.path().join("PIONEER/Artwork/00001");
    for (name, side) in [("a1.jpg", 80), ("a1_m.jpg", 240), ("b1.jpg", 80), ("b1_m.jpg", 240)] {
        let img = image::open(dir.join(name)).unwrap();
        assert_eq!((img.width(), img.height()), (side, side), "{name}");
    }
    assert!(!dir.join("a2.jpg").exists());
}

#[test]
fn anlz_dat_roundtrips_beats_waveform_and_cues() {
    let f = exported();
    let read = read_anlz(&anlz_dat(f.usb.path(), &f.tracks[0])).unwrap();
    let expected = &f.analyses[0];

    assert_eq!(read.beat_grid.beats.len(), expected.beat_grid.beats.len());
    for (got, want) in read.beat_grid.beats.iter().zip(&expected.beat_grid.beats) {
        assert_eq!((got.bar_position, got.time_ms, got.tempo), (want.bar_position, want.time_ms, want.tempo));
    }
    assert_eq!(read.bpm, 128.0);
    assert_eq!(read.waveform.data, expected.waveform.data);

    let mut cues: Vec<_> = read.cue_points.iter().map(|c| (c.hot_cue_number, c.time_ms, c.loop_time_ms)).collect();
    cues.sort();
    assert_eq!(cues, vec![(0, 500, None), (1, 1000, None), (2, 1500, Some(1900))]);
}

#[test]
fn anlz_dat_embeds_utf16be_usb_path() {
    let f = exported();
    let bytes = std::fs::read(anlz_dat(f.usb.path(), &f.tracks[1])).unwrap();
    assert_eq!(&bytes[..4], b"PMAI");
    assert_eq!(u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize, bytes.len());
    assert_eq!(&bytes[0x1C..0x20], b"PPTH");
    let path_len = u32::from_be_bytes(bytes[0x28..0x2C].try_into().unwrap()) as usize;
    let units: Vec<u16> = bytes[0x2C..0x2C + path_len]
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    assert_eq!(String::from_utf16(&units).unwrap(), "/Contents/Beta/two.wav\0");
}

#[test]
fn onelibrary_roundtrips_tracks_playlists_and_artwork() {
    let f = exported();
    let state = read_usb_state(f.usb.path()).unwrap().unwrap();

    assert_eq!(state.tracks.len(), 3);
    assert_eq!(state.next_track_id, 4);
    for (got, want) in state.tracks.iter().zip(&f.tracks) {
        assert_eq!(got.id, want.id);
        assert_eq!(got.usb_path, want.usb_path);
        assert_eq!(got.title, want.title);
        assert_eq!(got.artist, want.artist);
        assert_eq!(got.album, want.album);
        assert_eq!(got.tempo, want.tempo);
        assert_eq!(got.key, want.key);
        assert_eq!(got.file_size, want.file_size);
    }
    let has_artwork: Vec<bool> = state.tracks.iter().map(|t| t.has_artwork).collect();
    assert_eq!(has_artwork, vec![true, false, false]);

    let playlists: Vec<_> = state.playlists.iter().map(|p| (p.id, p.name.as_str(), p.track_ids.clone())).collect();
    assert_eq!(playlists, vec![(1, "Warmup", vec![2, 1]), (2, "Peak", vec![3])]);
}

#[test]
fn onelibrary_stores_cues_in_microseconds() {
    let f = exported();
    let conn = onelibrary(f.usb.path());
    let mut stmt = conn
        .prepare("SELECT content_id, kind, inUsec, outUsec, isActiveLoop FROM cue ORDER BY cue_id")
        .unwrap();
    let rows: Vec<(u32, i32, i64, i64, i32)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        rows,
        vec![(1, 0, 500_000, 0, 0), (1, 1, 1_000_000, 0, 0), (1, 2, 1_500_000, 1_900_000, 1)]
    );
}

#[test]
fn read_usb_state_is_none_on_blank_drive() {
    let blank = TempDir::new().unwrap();
    assert!(read_usb_state(blank.path()).unwrap().is_none());
}

#[test]
fn sync_usb_on_blank_drive_adds_everything() {
    let f = fixture();
    let report: SyncReport = sync_usb(f.usb.path(), &f.tracks, &f.analyses, &f.playlists).unwrap();
    assert_eq!((report.tracks_added, report.tracks_removed, report.tracks_unchanged), (3, 0, 0));
    let state = read_usb_state(f.usb.path()).unwrap().unwrap();
    assert_eq!(state.tracks.len(), 3);
    assert_eq!(state.playlists.len(), 2);
    assert!(anlz_dat(f.usb.path(), &f.tracks[0]).is_file());
}

#[test]
fn sync_usb_second_run_skips_unchanged_and_removes_dropped_tracks() {
    let f = fixture();
    sync_usb(f.usb.path(), &f.tracks, &f.analyses, &f.playlists).unwrap();

    let again = sync_usb(f.usb.path(), &f.tracks, &f.analyses, &f.playlists).unwrap();
    assert_eq!((again.tracks_added, again.tracks_unchanged), (0, 3));

    let kept_playlists = vec![Playlist { id: 1, name: "Warmup".into(), track_ids: vec![2, 1] }];
    let trimmed = sync_usb(f.usb.path(), &f.tracks[..2], &f.analyses[..2], &kept_playlists).unwrap();
    assert_eq!(trimmed.tracks_removed, 1);

    let dropped = &f.tracks[2];
    assert!(!f.usb.path().join(dropped.usb_path.trim_start_matches('/')).exists());
    assert!(!anlz_dat(f.usb.path(), dropped).exists());
    let state = read_usb_state(f.usb.path()).unwrap().unwrap();
    assert_eq!(state.tracks.len(), 2);
}
