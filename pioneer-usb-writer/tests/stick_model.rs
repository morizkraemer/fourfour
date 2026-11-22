use std::path::Path;

use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, CuePoint, Playlist, Track, WaveformPreview};
use pioneer_usb_writer::stick::device_library::{self, table};
use pioneer_usb_writer::stick::dump::{diff_dumps, dump_stick, raw_row_lines, DumpOptions};
use pioneer_usb_writer::stick::emit::{copy_stick, write_databases, write_usb_v2};
use pioneer_usb_writer::stick::{LibraryKind, Stick, StickDevice, StickModel};
use pioneer_usb_writer::writer::filesystem::write_usb;
use tempfile::TempDir;

const RAW: DumpOptions = DumpOptions { raw: true, normalise: true, pages: true };

fn write_wav(path: &Path) {
    let sample_rate = 8000u32;
    let data_len = sample_rate * 2 * 2;
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
    bytes.extend(std::iter::repeat_n(0u8, data_len as usize));
    std::fs::write(path, bytes).unwrap();
}

fn cover_png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(32, 32, image::Rgb([200, 30, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn track(source_dir: &Path, id: u32, file: &str, title: &str, artist: &str, key: &str, tempo: u32) -> Track {
    let source_path = source_dir.join(file);
    write_wav(&source_path);
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
    let beats = (0..8).map(|i| Beat { bar_position: (i % 4) as u8 + 1, time_ms: i * beat_ms, tempo }).collect();
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

struct Library {
    _source: TempDir,
    tracks: Vec<Track>,
    analyses: Vec<AnalysisResult>,
    playlists: Vec<Playlist>,
}

fn library() -> Library {
    library_with_third_artist("alpha")
}

fn library_with_third_artist(third_artist: &str) -> Library {
    let source = TempDir::new().unwrap();
    let mut first = track(source.path(), 1, "one.wav", "First Light", "Alpha", "8A", 12800);
    first.artwork = Some(cover_png());
    first.comment = "c".repeat(140);
    let mut second = track(source.path(), 2, "two.wav", "Zweiter Wind \u{00fc} \u{65e5}\u{672c} \u{0416}", "Beta", "5B", 12400);
    second.remixer = "Gamma".into();
    let third = track(source.path(), 3, "three.wav", "Third Rail", third_artist, "", 13000);
    let cues = vec![
        CuePoint { hot_cue_number: 0, time_ms: 500, loop_time_ms: None, ..Default::default() },
        CuePoint { hot_cue_number: 1, time_ms: 1000, loop_time_ms: None, name: "drop".into(), ..Default::default() },
        CuePoint { hot_cue_number: 2, time_ms: 1500, loop_time_ms: Some(1900), ..Default::default() },
    ];
    Library {
        _source: source,
        analyses: vec![analysis(12800, "8A", cues), analysis(12400, "5B", vec![]), analysis(13000, "", vec![])],
        tracks: vec![first, second, third],
        playlists: vec![
            Playlist { id: 1, name: "Sets".into(), is_folder: true, ..Default::default() },
            Playlist { id: 2, name: "Warmup".into(), parent_id: 1, track_ids: vec![2, 1], ..Default::default() },
            Playlist { id: 3, name: "Peak".into(), track_ids: vec![3], ..Default::default() },
        ],
    }
}

fn device(date: &str) -> StickDevice {
    StickDevice { name: "FFTEST".into(), created_date: date.into(), ..StickDevice::default() }
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn exported_v2(library: &Library, date: &str) -> (TempDir, StickModel) {
    let usb = TempDir::new().unwrap();
    let model = write_usb_v2(usb.path(), &library.tracks, &library.analyses, &library.playlists, device(date)).unwrap();
    (usb, model)
}

fn comparable(model: &StickModel) -> Vec<String> {
    let mut lines: Vec<String> = model
        .tracks
        .iter()
        .map(|track| {
            format!(
                "{} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} {} {} {}",
                track.id,
                track.title,
                track.path,
                model.artist_name(track.artist_id),
                model.artist_name(track.remixer_id),
                model.album_name(track.album_id),
                model.genre_name(track.genre_id),
                model.label_name(track.label_id),
                model.key_name(track.key_id),
                track.tempo_x100,
                track.duration_secs,
                track.file_size,
                model.artwork_path(track.artwork_id),
            )
        })
        .collect();
    lines.extend(model.playlists.iter().map(|playlist| {
        format!(
            "playlist {} {} {} {:?} {:?}",
            playlist.id, playlist.parent_id, playlist.is_folder, playlist.name, playlist.track_ids
        )
    }));
    lines.sort();
    lines
}

#[test]
fn v2_export_reads_back_from_both_databases() {
    let library = library();
    let (usb, written) = exported_v2(&library, "2026-11-22");
    let stick = Stick::read(usb.path()).unwrap();
    let pdb = stick.device_library.as_ref().unwrap();
    let one = stick.one_library.as_ref().unwrap();

    let expected = StickModel {
        tracks: strip_one_library_extras(&written),
        menu: pdb.menu.clone(),
        ..written.clone()
    };
    assert_eq!(pdb, &expected);
    assert_eq!(comparable(pdb), comparable(one));
    assert_eq!(stick.primary_kind(), Some(LibraryKind::DeviceLibrary));

    let first = pdb.track_by_path("/Contents/Alpha/one.wav").unwrap();
    assert_eq!(pdb.artist_name(first.artist_id), "Alpha");
    assert_eq!(pdb.key_name(first.key_id), "Db");
    assert_eq!(first.comment.len(), 140);
    assert_eq!(pdb.track(2).unwrap().title, "Zweiter Wind \u{00fc} \u{65e5}\u{672c} \u{0416}");
    assert_eq!(pdb.artist_name(pdb.track(2).unwrap().remixer_id), "Gamma");
    assert_ne!(pdb.track(3).unwrap().artist_id, first.artist_id);

    let root: Vec<&str> = pdb.children(0).iter().map(|node| node.name.as_str()).collect();
    assert_eq!(root, ["Sets", "Peak"]);
    let warmup: Vec<u32> = pdb.playlist_tracks(2).iter().map(|track| track.id).collect();
    assert_eq!(warmup, [2, 1]);

    assert_eq!(one.track(1).unwrap().cues.len(), 3);
    assert_eq!(one.track(1).unwrap().cues[1].comment, "drop");
    assert_eq!(pdb.device.name, "FFTEST");
    assert_eq!(one.device.created_date, "2026-11-22");
}

fn strip_one_library_extras(model: &StickModel) -> Vec<pioneer_usb_writer::stick::StickTrack> {
    model
        .tracks
        .iter()
        .cloned()
        .map(|mut track| {
            track.cues.clear();
            track.analysed_bits = None;
            track
        })
        .collect()
}

#[test]
fn v2_export_writes_analysis_and_artwork_files() {
    let library = library();
    let (usb, _) = exported_v2(&library, "2026-11-22");
    let stick = Stick::read(usb.path()).unwrap();
    let model = stick.library().unwrap();
    for track in &model.tracks {
        assert!(stick.audio_path(track).is_file());
        let analysis = stick.read_analysis(track).unwrap();
        assert!(analysis.dat.is_some() && analysis.ext.is_some());
        assert_eq!(analysis.dat.as_ref().unwrap().path().as_deref(), Some(track.path.as_str()));
        assert_eq!(analysis.beats().len(), 8);
    }
    assert_eq!(stick.read_analysis(model.track(1).unwrap()).unwrap().cues().len(), 3);
    for name in ["a1.jpg", "a1_m.jpg", "b1.jpg", "b1_m.jpg"] {
        assert!(usb.path().join("PIONEER/Artwork/00001").join(name).is_file(), "{name}");
    }
}

#[test]
fn same_input_gives_the_same_export_pdb_bytes() {
    let library = library();
    let (first, _) = exported_v2(&library, "2026-11-22");
    let (second, _) = exported_v2(&library, "2026-11-22");
    let pdb = |usb: &TempDir| std::fs::read(usb.path().join("PIONEER/rekordbox/export.pdb")).unwrap();
    assert_eq!(pdb(&first), pdb(&second));
    assert_eq!(dump_stick(first.path(), RAW).unwrap(), dump_stick(second.path(), RAW).unwrap());
}

#[test]
fn only_masked_lines_differ_between_exports_on_different_days() {
    let library = library();
    let (first, _) = exported_v2(&library, "2026-11-22");
    let (second, _) = exported_v2(&library, "2026-11-23");
    let unmasked = DumpOptions { normalise: false, ..RAW };
    let noise = diff_dumps(&dump_stick(first.path(), unmasked).unwrap(), &dump_stick(second.path(), unmasked).unwrap());
    assert!(!noise.is_empty());
    assert!(noise.iter().all(|line| line.contains("2026-11-2") || line.contains(&hex_of("2026-11-2"))), "{noise:#?}");
    let dated_in_tracks = diff_dumps(&dump_stick(first.path(), RAW).unwrap(), &dump_stick(second.path(), RAW).unwrap());
    assert!(dated_in_tracks.iter().all(|line| line.contains(" track ") || line.contains(" raw tracks ") || line.contains("row content")), "{dated_in_tracks:#?}");
}

fn hex_of(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn a_stick_written_from_a_read_stick_dumps_the_same() {
    let library = library();
    let (first, _) = exported_v2(&library, "2026-11-22");
    let source = Stick::read(first.path()).unwrap();
    let second = TempDir::new().unwrap();
    copy_stick(&source, LibraryKind::DeviceLibrary, second.path(), true).unwrap();

    let without_one_library = |dump: String| -> String {
        dump.lines().filter(|line| !line.starts_with("onelib ")).collect::<Vec<_>>().join("\n")
    };
    let left = without_one_library(dump_stick(first.path(), RAW).unwrap());
    let right = without_one_library(dump_stick(second.path(), RAW).unwrap());
    assert_eq!(diff_dumps(&left, &right), Vec::<String>::new());
    assert_eq!(
        std::fs::read(first.path().join("PIONEER/rekordbox/export.pdb")).unwrap(),
        std::fs::read(second.path().join("PIONEER/rekordbox/export.pdb")).unwrap()
    );

    let third = TempDir::new().unwrap();
    copy_stick(&source, LibraryKind::OneLibrary, third.path(), false).unwrap();
    let one_library_only = |root: &Path| -> String {
        dump_stick(root, RAW).unwrap().lines().filter(|line| line.starts_with("onelib ")).collect::<Vec<_>>().join("\n")
    };
    assert_eq!(diff_dumps(&one_library_only(first.path()), &one_library_only(third.path())), Vec::<String>::new());
}

#[test]
fn a_stick_with_an_empty_one_library_reads_from_export_pdb() {
    let library = library();
    let (usb, model) = exported_v2(&library, "2026-11-22");
    let emptied = StickModel { tracks: Vec::new(), playlists: Vec::new(), ..model.clone() };
    pioneer_usb_writer::stick::one_library::write(&usb.path().join("PIONEER/rekordbox/exportLibrary.db"), &emptied)
        .unwrap();

    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.one_library.as_ref().unwrap().tracks.len(), 0);
    assert_eq!(stick.primary_kind(), Some(LibraryKind::DeviceLibrary));
    assert_eq!(stick.track_count(), 3);
    assert_eq!(stick.library().unwrap().playlists.len(), 3);
}

#[test]
fn a_stick_with_only_one_library_reads_from_it() {
    let library = library();
    let (usb, _) = exported_v2(&library, "2026-11-22");
    std::fs::remove_file(usb.path().join("PIONEER/rekordbox/export.pdb")).unwrap();
    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.primary_kind(), Some(LibraryKind::OneLibrary));
    assert_eq!(stick.track_count(), 3);
}

#[test]
fn a_blank_folder_has_no_library() {
    let blank = TempDir::new().unwrap();
    let stick = Stick::read(blank.path()).unwrap();
    assert!(!stick.has_library());
    assert_eq!(stick.track_count(), 0);
    assert_eq!(dump_stick(blank.path(), RAW).unwrap(), "stick no-library\n");
}

#[test]
fn reading_leaves_the_stick_untouched() {
    let library = library();
    let (usb, _) = exported_v2(&library, "2026-11-22");
    let listing = |root: &Path| -> Vec<(String, u64, std::time::SystemTime)> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let meta = entry.metadata().unwrap();
                if meta.is_dir() {
                    stack.push(entry.path());
                } else {
                    out.push((entry.path().to_string_lossy().into_owned(), meta.len(), meta.modified().unwrap()));
                }
            }
        }
        out.sort();
        out
    };
    let before = listing(usb.path());
    Stick::read(usb.path()).unwrap();
    dump_stick(usb.path(), RAW).unwrap();
    assert_eq!(listing(usb.path()), before);
}

#[test]
fn player_files_are_listed_as_not_compared() {
    let library = library();
    let (usb, _) = exported_v2(&library, "2026-11-22");
    std::fs::write(usb.path().join("PIONEER/rekordbox/RBFLTR.DAT"), b"player").unwrap();
    std::fs::write(usb.path().join("PIONEER/._hidden"), b"mac").unwrap();
    let dump = dump_stick(usb.path(), RAW).unwrap();
    assert!(dump.contains("not-compared rekordbox/RBFLTR.DAT"));
    assert!(dump.contains("not-compared ._hidden"));
    assert!(!dump.contains("file rekordbox/RBFLTR.DAT"));
}

#[test]
fn the_old_writer_stick_reads_through_the_new_reader() {
    let library = library();
    let usb = TempDir::new().unwrap();
    write_usb(usb.path(), &library.tracks, &library.analyses, &library.playlists).unwrap();
    let stick = Stick::read(usb.path()).unwrap();
    let pdb = stick.device_library.as_ref().unwrap();
    let one = stick.one_library.as_ref().unwrap();
    assert_eq!(pdb.tracks.len(), 3);
    assert_eq!(pdb.track(2).unwrap().title, "Zweiter Wind \u{00fc} \u{65e5}\u{672c} \u{0416}");
    assert_eq!(pdb.artist_name(pdb.track(1).unwrap().artist_id), "Alpha");
    assert_eq!(pdb.playlist_tracks(2).iter().map(|track| track.id).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(one.tracks.len(), 3);
    assert_eq!(one.track(1).unwrap().cues.len(), 3);
}

const KNOWN_ROW_DIFFERENCES_FROM_THE_OLD_WRITER: &[&str] =
    &["colors", "genres", "keys", "playlist_entries", "playlist_tree", "property", "sorts", "tracks"];

#[test]
fn v2_rows_match_the_cdj_proven_writer_except_for_the_listed_tables() {
    let library = library_with_third_artist("Alpha");
    let old = TempDir::new().unwrap();
    write_usb(old.path(), &library.tracks, &library.analyses, &library.playlists).unwrap();
    let (new, _) = exported_v2(&library, &today());

    let rows = |root: &Path| {
        let bytes = std::fs::read(root.join("PIONEER/rekordbox/export.pdb")).unwrap();
        raw_row_lines("pdb", &device_library::table_rows(&bytes).unwrap(), true).join("\n")
    };
    let lines = diff_dumps(&rows(old.path()), &rows(new.path()));
    let mut differing: Vec<String> = lines
        .iter()
        .filter_map(|line| line.split_whitespace().nth(2).map(str::to_owned))
        .collect();
    differing.sort();
    differing.dedup();
    assert_eq!(differing, KNOWN_ROW_DIFFERENCES_FROM_THE_OLD_WRITER, "tables whose rows differ");
    let changed = |table: u32| lines.iter().filter(|line| line.starts_with(&format!("+pdb raw {} ", table::name(table)))).count();
    assert_eq!(changed(table::TRACKS), 2, "only the two rows with long strings move, by their alignment");
    assert_eq!(changed(table::ARTISTS) + changed(table::ALBUMS) + changed(table::LABELS) + changed(table::ARTWORK), 0);
}

#[test]
fn write_databases_rejects_a_missing_creation_date() {
    let library = library();
    let (usb, model) = exported_v2(&library, "2026-11-22");
    let undated = StickModel { device: StickDevice::default(), ..model };
    let error = write_databases(usb.path(), &undated).unwrap_err();
    assert!(format!("{error:#}").contains("creation date"));
}
