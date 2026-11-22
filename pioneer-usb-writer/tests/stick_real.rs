use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pioneer_usb_writer::stick::device_library::{self, table};
use pioneer_usb_writer::stick::dump::normalised_row;
use pioneer_usb_writer::stick::{LibraryKind, Stick};

const UNALIGNED_TRACK_ROWS_ON_THE_2026_11_22_STICK: usize = 2;
const REAL_STICK_ENV: &str = "FOURFOUR_REAL_STICK";

fn real_stick() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os(REAL_STICK_ENV)?);
    root.join("PIONEER").join("rekordbox").join("export.pdb").is_file().then_some(root)
}

fn analysis_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            analysis_files(&path, out);
        } else if matches!(path.extension().and_then(|ext| ext.to_str()), Some("DAT" | "EXT" | "2EX"))
            && path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("ANLZ"))
        {
            out.push(path);
        }
    }
}

#[test]
fn real_rekordbox_stick_reads_from_export_pdb() {
    let Some(root) = real_stick() else { return };
    let stick = Stick::read(&root).unwrap();
    let library = stick.library().unwrap();
    assert_eq!(stick.primary_kind(), Some(LibraryKind::DeviceLibrary));
    assert!(library.tracks.len() > stick.one_library.as_ref().map_or(0, |one| one.tracks.len()));
    assert_eq!(library.device.track_count as usize, library.tracks.len());
    let with_artist = library.tracks.iter().filter(|track| !library.artist_name(track.artist_id).is_empty()).count();
    let with_file = library.tracks.iter().filter(|track| track.path.starts_with("/Contents/")).count();
    let in_playlists: usize = library.playlists.iter().map(|playlist| library.playlist_tracks(playlist.id).len()).sum();
    let listed: usize = library.playlists.iter().map(|playlist| playlist.track_ids.len()).sum();
    println!(
        "tracks={} with_artist={with_artist} with_file={with_file} playlists={} entries={listed} resolved={in_playlists} histories={}",
        library.tracks.len(),
        library.playlists.len(),
        library.histories.len()
    );
    assert_eq!(with_file, library.tracks.len());
    assert_eq!(in_playlists, listed);
}

#[test]
fn real_rekordbox_rows_survive_the_model() {
    let Some(root) = real_stick() else { return };
    let bytes = std::fs::read(root.join("PIONEER/rekordbox/export.pdb")).unwrap();
    let original = device_library::table_rows(&bytes).unwrap();
    let model = device_library::model_from_tables(&original);
    let encoded = device_library::encode_tables(&model);

    let mut mismatches: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for rows in &original {
        let mut expected: Vec<Vec<u8>> = rows.rows.iter().map(|row| normalised_row(rows.table, row)).collect();
        let mut actual: Vec<Vec<u8>> = encoded
            .iter()
            .find(|candidate| candidate.table == rows.table)
            .map(|candidate| candidate.rows.iter().map(|row| normalised_row(rows.table, row)).collect())
            .unwrap_or_default();
        expected.sort();
        actual.sort();
        let missing = expected.iter().filter(|row| actual.binary_search(row).is_err()).count();
        if std::env::var_os("FOURFOUR_REAL_STICK_DEBUG").is_some() {
            let extra: Vec<&Vec<u8>> = actual.iter().filter(|row| expected.binary_search(row).is_err()).collect();
            for row in expected.iter().filter(|row| actual.binary_search(row).is_err()) {
                let close = extra.iter().max_by_key(|other| row.iter().zip(other.iter()).take_while(|(a, b)| a == b).count());
                let at = close.map_or(0, |other| row.iter().zip(other.iter()).take_while(|(a, b)| a == b).count());
                let from = at.saturating_sub(6);
                println!(
                    "DEBUG {} len={} other_len={:?} first_diff={at} head={:02x?} expected={:02x?} actual={:02x?}",
                    table::name(rows.table),
                    row.len(),
                    close.map(|other| other.len()),
                    &row[..12.min(row.len())],
                    &row[from..(at + 10).min(row.len())],
                    close.map(|other| other[from.min(other.len())..(at + 10).min(other.len())].to_vec()),
                );
            }
        }
        if missing > 0 || expected.len() != actual.len() {
            mismatches.insert(table::name(rows.table), (missing, expected.len()));
        }
    }
    println!("rows that do not survive (table: missing of total): {mismatches:?}");
    let unexplained: Vec<(&str, usize)> = mismatches.iter().map(|(name, (missing, _))| (*name, *missing)).collect();
    assert!(
        unexplained.iter().all(|(name, missing)| *name == "tracks" && *missing <= UNALIGNED_TRACK_ROWS_ON_THE_2026_11_22_STICK),
        "export.pdb rows changed on the way through the model: {mismatches:?}"
    );

    let rebuilt = device_library::build(&model).unwrap();
    let reread = device_library::read(&rebuilt).unwrap();
    assert_eq!(reread, model);
}

#[test]
fn real_rekordbox_analysis_files_are_carried_byte_for_byte() {
    let Some(root) = real_stick() else { return };
    let mut files = Vec::new();
    analysis_files(&root.join("PIONEER/USBANLZ"), &mut files);
    assert!(!files.is_empty());
    let mut changed = 0;
    let mut unreadable = 0;
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        match rbl_anlz::parse(&bytes) {
            Ok(parsed) => {
                let stick_path = parsed.path().unwrap_or_default();
                if parsed.to_bytes() != bytes || parsed.with_path(&stick_path) != bytes {
                    changed += 1;
                }
            }
            Err(_) => unreadable += 1,
        }
    }
    println!("analysis files={} changed={changed} unreadable={unreadable}", files.len());
    assert_eq!((changed, unreadable), (0, 0));
}
