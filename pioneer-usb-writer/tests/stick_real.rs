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

#[test]
fn real_rekordbox_stick_copies_through_the_v2_writer() {
    use pioneer_usb_writer::stick::dump::{diff_dumps, dump_stick, DumpOptions};
    use pioneer_usb_writer::stick::emit::copy_stick;

    let Some(root) = real_stick() else { return };
    let source = Stick::read(&root).unwrap();
    let copy = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    copy_stick(&source, LibraryKind::DeviceLibrary, copy.path(), false).unwrap();

    let options = DumpOptions { raw: true, normalise: true, pages: false };
    let compared = |root: &Path| -> String {
        dump_stick(root, options)
            .unwrap()
            .lines()
            .filter(|line| line.starts_with("pdb ") || line.starts_with("anlz "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let differing = diff_dumps(&compared(&root), &compared(copy.path()));
    let kinds: BTreeMap<String, usize> = differing.iter().fold(BTreeMap::new(), |mut kinds, line| {
        let kind = line.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
        *kinds.entry(kind).or_default() += 1;
        kinds
    });
    println!("lines that differ after the copy, by kind: {kinds:?}");
    assert!(differing.iter().all(|line| line.contains("pdb raw tracks ")), "{kinds:?}");
    assert!(differing.len() <= 2 * UNALIGNED_TRACK_ROWS_ON_THE_2026_11_22_STICK, "{kinds:?}");

    let reread = Stick::read(copy.path()).unwrap();
    assert_eq!(reread.device_library, source.device_library);
}

#[test]
fn real_rekordbox_stick_keeps_every_track_when_ours_are_added() {
    use pioneer_usb_writer::models::{AnalysisResult, BeatGrid, Playlist, Track, WaveformPreview};
    use pioneer_usb_writer::stick::emit::copy_stick;
    use pioneer_usb_writer::stick::plan::{apply, plan, Ownership, Wanted};
    use pioneer_usb_writer::stick::StickDevice;

    let Some(root) = real_stick() else { return };
    let source = Stick::read(&root).unwrap();
    let work = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let usb = work.path().join("usb");
    copy_stick(&source, LibraryKind::DeviceLibrary, &usb, false).unwrap();
    std::fs::copy(root.join("PIONEER/rekordbox/exportLibrary.db"), usb.join("PIONEER/rekordbox/exportLibrary.db"))
        .unwrap();
    let theirs = Stick::read(&usb).unwrap();
    let their_tracks = theirs.device_library.as_ref().unwrap().tracks.len();
    assert_eq!(theirs.one_library.as_ref().unwrap().tracks.len(), 0);
    let their_rows = |usb: &Path| -> BTreeMap<u32, Vec<u8>> {
        let bytes = std::fs::read(usb.join("PIONEER/rekordbox/export.pdb")).unwrap();
        device_library::table_rows(&bytes)
            .unwrap()
            .into_iter()
            .find(|rows| rows.table == 0)
            .unwrap()
            .rows
            .iter()
            .map(|row| (u32::from_le_bytes(row[0x48..0x4c].try_into().unwrap()), normalised_row(0, row)))
            .collect()
    };
    let before = their_rows(&usb);

    let tracks: Vec<Track> = (1..=3u32)
        .map(|id| {
            let source_path = work.path().join(format!("fourfour-test-{id}.wav"));
            std::fs::write(&source_path, vec![0u8; 4096]).unwrap();
            Track {
                usb_path: format!("/Contents/fourfour test artist/fourfour-test-{id}.wav"),
                source_path,
                title: format!("fourfour test {id}"),
                artist: "fourfour test artist".into(),
                key: "8A".into(),
                tempo: 12800,
                duration_secs: 1.0,
                sample_rate: 44100,
                bitrate: 1411,
                file_size: 4096,
                id,
                album: String::new(),
                genre: String::new(),
                label: String::new(),
                remixer: String::new(),
                comment: String::new(),
                year: 0,
                disc_number: 0,
                track_number: 0,
                artwork: None,
            }
        })
        .collect();
    let analyses: Vec<AnalysisResult> = tracks
        .iter()
        .map(|_| AnalysisResult {
            beat_grid: BeatGrid { beats: Vec::new() },
            waveform: WaveformPreview { data: [0; 400] },
            bpm: 128.0,
            key: "8A".into(),
            energy: None,
            cue_points: Vec::new(),
            color_waveform: None,
        })
        .collect();
    let playlists =
        vec![Playlist { id: 1, name: "fourfour test".into(), track_ids: vec![1, 2, 3], ..Default::default() }];
    let wanted = Wanted { tracks: &tracks, analyses: &analyses, playlists: &playlists, labels: &[] };
    let device = StickDevice { name: "X".into(), created_date: "2026-11-22".into(), ..StickDevice::default() };

    let planned = plan(&theirs, wanted, &Ownership::default(), device.clone()).unwrap();
    assert_eq!(planned.foreign_track_count, their_tracks);
    apply(&usb, &planned, wanted).unwrap();

    let after = Stick::read(&usb).unwrap();
    let pdb = after.device_library.as_ref().unwrap();
    let one = after.one_library.as_ref().unwrap();
    let after_rows = their_rows(&usb);
    let changed = before.iter().filter(|(id, row)| after_rows.get(id) != Some(row)).count();
    let with_cues = one.tracks.iter().filter(|track| !track.cues.is_empty()).count();
    println!(
        "before={their_tracks} after_pdb={} after_one={} foreign_rows_changed={changed} playlists={} one_library_tracks_with_cues={with_cues}",
        pdb.tracks.len(),
        one.tracks.len(),
        pdb.playlists.len()
    );
    assert_eq!((pdb.tracks.len(), one.tracks.len()), (their_tracks + 3, their_tracks + 3));
    assert!(changed <= UNALIGNED_TRACK_ROWS_ON_THE_2026_11_22_STICK);
    assert_eq!(pdb.playlists.len(), theirs.device_library.as_ref().unwrap().playlists.len() + 1);
    assert_eq!(pdb.histories, theirs.device_library.as_ref().unwrap().histories);
    assert_eq!(pdb.my_tags, source.device_library.as_ref().unwrap().my_tags);
    assert_eq!(one.my_tags.len(), pdb.my_tags.len());
    assert!(!pdb.my_tags.is_empty());
    assert!(plan(&after, wanted, &planned.ownership, device).unwrap().is_noop());
}

#[test]
fn real_rekordbox_my_tag_rows_survive_the_model() {
    use pioneer_usb_writer::stick::ext_library;

    let Some(root) = real_stick() else { return };
    let path = root.join("PIONEER/rekordbox/exportExt.pdb");
    if !path.is_file() {
        return;
    }
    let bytes = std::fs::read(path).unwrap();
    let original = ext_library::table_rows(&bytes).unwrap();
    let ext = ext_library::from_tables(&original);
    let encoded = ext_library::encode_tables(&ext);
    let without_slot = |table: u32, rows: &[Vec<u8>]| -> Vec<Vec<u8>> {
        let mut rows: Vec<Vec<u8>> = rows.iter().map(|row| ext_library::normalised_row(table, row)).collect();
        rows.sort();
        rows
    };
    let mut differing = BTreeMap::new();
    for rows in &original {
        let expected = without_slot(rows.table, &rows.rows);
        let actual = without_slot(rows.table, &encoded[rows.table as usize].rows);
        let missing = expected.iter().filter(|row| actual.binary_search(row).is_err()).count();
        if std::env::var_os("FOURFOUR_REAL_STICK_DEBUG").is_some() {
            for row in expected.iter().filter(|row| actual.binary_search(row).is_err()) {
                println!("DEBUG ext table={} len={} head={:02x?}", rows.table, row.len(), &row[..0x24.min(row.len())]);
            }
            for row in actual.iter().filter(|row| expected.binary_search(row).is_err()) {
                println!("DEBUG ours table={} len={} head={:02x?}", rows.table, row.len(), &row[..0x24.min(row.len())]);
            }
        }
        if missing > 0 || expected.len() != actual.len() {
            differing.insert(rows.table, (missing, expected.len(), actual.len()));
        }
    }
    println!(
        "my tags={} tagged tracks={} master={} rows that differ (table: missing, theirs, ours)={differing:?}",
        ext.my_tags.len(),
        ext.track_tags.len(),
        ext.master_db_id != 0
    );
    assert!(differing.is_empty(), "{differing:?}");
    assert_eq!(ext_library::read(&ext_library::build(&ext)).unwrap(), ext);
}
