//! Runs the Rekordbox import against a real desktop library. Ignored by
//! default; needs local data that is never committed:
//!
//! ```text
//! FOURFOUR_RB_LIBRARY=/path/to/rekordbox-library \
//! FOURFOUR_RB_AUDIO=/path/to/audio-root \
//! FOURFOUR_RB_STRIP=/Users/someone/Music/ \
//! cargo test -p pioneer-library --test rekordbox_real_library -- --ignored --nocapture
//! ```
//!
//! `FOURFOUR_RB_LIBRARY` holds `master.db` and `share/PIONEER`. Only counts are
//! printed.

use std::path::PathBuf;

use pioneer_library::LocalLibrary;
use pioneer_library::rekordbox::{self, ImportOptions};

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set")))
}

#[test]
#[ignore = "needs a real Rekordbox library on disk (FOURFOUR_RB_LIBRARY)"]
fn import_real_library_prints_counts() {
    let library_dir = env_path("FOURFOUR_RB_LIBRARY");
    let options = ImportOptions {
        remap_from: std::env::var("FOURFOUR_RB_STRIP").ok(),
        remap_to: Some(env_path("FOURFOUR_RB_AUDIO").to_string_lossy().to_string()),
        include_missing: false,
    };

    let source = rekordbox::load_masterdb(&library_dir.join("master.db"), Some(&library_dir.join("share")))
        .expect("master.db loads");

    let scratch = tempfile::tempdir().unwrap();
    let library = LocalLibrary::open(&scratch.path().join("library.db")).unwrap();

    let preview = library.rekordbox_preview(&source, &options).unwrap();
    println!("PREVIEW tracks={} playlists={} folders={} smart_skipped={}", preview.tracks, preview.playlists, preview.folders, preview.skipped_smart_playlists);
    println!("PREVIEW files_found={} files_missing={} conflicts={}", preview.files_found, preview.files_missing, preview.conflicts);
    println!("PREVIEW tracks_with_grid={} hot_cues={} memory_cues={}", preview.tracks_with_grid, preview.hot_cues, preview.memory_cues);
    println!("PREVIEW detected_root={}", preview.detected_root);

    let report = library
        .import_rekordbox_from(&source, &options, &|_| {})
        .expect("import succeeds");
    println!("IMPORT {report:?}");

    let tracks = library.get_all_tracks().unwrap();
    let playlists = library.get_all_playlists().unwrap();
    let analyzed = tracks.len() - library.get_unanalyzed_track_ids().unwrap().len();
    println!(
        "LIBRARY tracks={} analyzed={} playlists={} playlist_entries={}",
        tracks.len(),
        analyzed,
        playlists.len(),
        playlists.iter().map(|p| p.track_ids.len()).sum::<usize>()
    );

    assert_eq!(report.tracks_imported as usize, tracks.len());
    assert_eq!(report.files_found, preview.files_found);

    let again = library
        .import_rekordbox_from(&source, &options, &|_| {})
        .expect("second import succeeds");
    println!("REIMPORT {again:?}");
    assert_eq!(again.tracks_imported, 0);
    assert_eq!(library.get_all_tracks().unwrap().len(), tracks.len());
}
