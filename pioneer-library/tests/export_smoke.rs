use std::collections::BTreeSet;
use std::f32::consts::TAU;
use std::path::{Path, PathBuf};

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models::{
    AnalysisResult, Beat, BeatGrid, ExistingUsbState, WaveformPreview,
};
use pioneer_usb_writer::{reader, scanner};
use tempfile::TempDir;

const SAMPLE_RATE: u32 = 44_100;

fn write_wav(path: &Path, frequency: f32) {
    let sample_count = SAMPLE_RATE / 4;
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

fn make_fixture_folder(root: &Path) -> PathBuf {
    let folder = root.join("fixture");
    write_wav(&folder.join("alpha.wav"), 220.0);
    write_wav(&folder.join("sub/bravo.wav"), 330.0);
    write_wav(&folder.join("sub/charlie.wav"), 440.0);
    folder
}

fn make_analysis() -> AnalysisResult {
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

fn import_folder(library: &LocalLibrary, folder: &Path) -> Vec<i64> {
    let tracks = scanner::scan_directory(folder).unwrap();
    let ids = library.add_tracks(&tracks).unwrap();
    for id in &ids {
        library.set_analysis(*id, &make_analysis()).unwrap();
    }
    ids
}

fn track_id_by_title(library: &LocalLibrary, title: &str) -> i64 {
    let tracks = library.get_all_tracks().unwrap();
    let track = tracks.iter().find(|t| t.title == title).unwrap();
    track.id as i64
}

fn read_usb(usb: &Path) -> ExistingUsbState {
    reader::read_usb_state(usb).unwrap().expect("USB state present")
}

fn usb_titles(state: &ExistingUsbState) -> BTreeSet<String> {
    state.tracks.iter().map(|t| t.title.clone()).collect()
}

fn usb_playlist_names(state: &ExistingUsbState) -> BTreeSet<String> {
    state.playlists.iter().map(|p| p.name.clone()).collect()
}

fn library_titles(library: &LocalLibrary) -> BTreeSet<String> {
    library.get_all_tracks().unwrap().into_iter().map(|t| t.title).collect()
}

fn library_playlist_names(library: &LocalLibrary) -> BTreeSet<String> {
    library.get_all_playlists().unwrap().into_iter().map(|p| p.name).collect()
}

fn names(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn assert_usb_base_tree(usb: &Path) {
    for dir in ["PIONEER/rekordbox", "PIONEER/USBANLZ", "Contents"] {
        assert!(usb.join(dir).is_dir(), "missing dir {dir}");
    }
    assert!(usb.join("PIONEER/rekordbox/export.pdb").is_file());
}

fn assert_track_files_on_usb(usb: &Path, state: &ExistingUsbState, expected_audio: usize) {
    for track in &state.tracks {
        let audio = usb.join(track.usb_path.trim_start_matches('/'));
        assert!(audio.is_file(), "missing audio {}", audio.display());
    }
    let audio_files = walk_files(&usb.join("Contents"));
    assert_eq!(audio_files.len(), expected_audio, "audio files: {audio_files:?}");
    let anlz_files = walk_files(&usb.join("PIONEER/USBANLZ"));
    assert_eq!(anlz_files.len(), expected_audio * 2, "anlz files: {anlz_files:?}");
    assert!(anlz_files.iter().any(|f| f.ends_with("ANLZ0000.DAT")));
    assert!(anlz_files.iter().any(|f| f.ends_with("ANLZ0000.EXT")));
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn import_export_then_incremental_sync() {
    let workspace = TempDir::new().unwrap();
    let fixture = make_fixture_folder(workspace.path());
    let usb = workspace.path().join("usb");
    std::fs::create_dir_all(&usb).unwrap();
    let library = LocalLibrary::open(&workspace.path().join("library.db")).unwrap();

    let ids = import_folder(&library, &fixture);
    assert_eq!(ids.len(), 3);
    assert_eq!(library.track_count().unwrap(), 3);
    assert_eq!(library.get_unanalyzed_track_ids().unwrap().len(), 0);

    let alpha = track_id_by_title(&library, "alpha");
    let bravo = track_id_by_title(&library, "bravo");
    let charlie = track_id_by_title(&library, "charlie");
    let warmup = library.create_playlist("Warmup").unwrap();
    let peak = library.create_playlist("Peak").unwrap();
    library.set_playlist_tracks(warmup, &[alpha, bravo]).unwrap();
    library.set_playlist_tracks(peak, &[bravo, charlie]).unwrap();
    assert_eq!(library.get_all_playlists().unwrap().len(), 2);

    library.write_usb(&usb).unwrap();

    assert_usb_base_tree(&usb);
    let state = read_usb(&usb);
    assert_eq!(state.tracks.len(), 3);
    assert_eq!(state.playlists.len(), 2);
    assert_eq!(usb_titles(&state), library_titles(&library));
    assert_eq!(usb_playlist_names(&state), library_playlist_names(&library));
    assert_eq!(usb_playlist_names(&state), names(&["Warmup", "Peak"]));
    assert_track_files_on_usb(&usb, &state, 3);

    let delta_path = fixture.join("delta.wav");
    write_wav(&delta_path, 550.0);
    let delta_track = scanner::read_track_metadata(&delta_path, 0).unwrap();
    let delta = library.add_track(&delta_track).unwrap();
    library.set_analysis(delta, &make_analysis()).unwrap();
    library.remove_track(alpha).unwrap();
    library.rename_playlist(warmup, "Opening").unwrap();
    library.set_playlist_tracks(warmup, &[bravo, delta]).unwrap();

    assert_eq!(library.track_count().unwrap(), 3);
    assert_eq!(library_titles(&library), names(&["bravo", "charlie", "delta"]));
    assert_eq!(library_playlist_names(&library), names(&["Opening", "Peak"]));

    let report = library.sync_usb(&usb).unwrap();

    assert_eq!(report.tracks_added, 1);
    assert_eq!(report.tracks_removed, 1);
    assert_eq!(report.tracks_replaced, 0);
    assert_eq!(report.tracks_added + report.tracks_updated + report.tracks_unchanged, 3);

    assert_usb_base_tree(&usb);
    let state = read_usb(&usb);
    assert_eq!(state.tracks.len(), 3);
    assert_eq!(state.playlists.len(), 2);
    assert_eq!(usb_titles(&state), library_titles(&library));
    assert_eq!(usb_playlist_names(&state), library_playlist_names(&library));
    assert_eq!(usb_playlist_names(&state), names(&["Opening", "Peak"]));
    assert_track_files_on_usb(&usb, &state, 3);
    assert!(!usb.join("Contents/Unknown Artist/alpha.wav").exists());
    assert!(usb.join("Contents/Unknown Artist/delta.wav").is_file());
    let opening = state.playlists.iter().find(|p| p.name == "Opening").unwrap();
    assert_eq!(opening.track_ids.len(), 2);
}
