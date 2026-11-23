mod support;

use std::path::{Path, PathBuf};

use pioneer_library::playlist_export::ExportFormat;
use pioneer_library::rekordbox::{self, ImportOptions, ImportedCue, ImportedMeta, SourceKind};
use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, Track, WaveformPreview};
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    library: LocalLibrary,
    audio: PathBuf,
    present_id: i64,
    playlist: i64,
    empty: i64,
}

fn track(path: &Path, title: &str, artist: &str) -> Track {
    Track {
        source_path: path.to_path_buf(),
        usb_path: String::new(),
        title: title.into(),
        artist: artist.into(),
        album: String::new(),
        genre: "House".into(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2020,
        disc_number: 0,
        track_number: 3,
        tempo: 12000,
        key: String::new(),
        duration_secs: 4.0,
        sample_rate: 44100,
        bitrate: 320,
        file_size: 4096,
        id: 0,
        artwork: None,
    }
}

fn analysis(bpm: f64, key: &str) -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid {
            beats: (0..8u32)
                .map(|i| Beat { bar_position: (i % 4 + 1) as u8, time_ms: 100 + i * 500, tempo: (bpm * 100.0) as u32 })
                .collect(),
        },
        waveform: WaveformPreview { data: [0u8; 400] },
        bpm,
        key: key.into(),
        energy: None,
        cue_points: Vec::new(),
        color_waveform: None,
    }
}

fn cue(slot: u32, at: u32, end: Option<u32>, name: &str, color: Option<&str>) -> ImportedCue {
    ImportedCue { hot_cue_number: slot, time_ms: at, loop_time_ms: end, color: color.map(Into::into), name: name.into() }
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let library = LocalLibrary::open(&root.join("library.db")).unwrap();
    let audio = root.join("My Music").join("Tom & Jerry #1 \u{fc}.wav");
    support::write_wav(&audio, 440.0, 0.1);
    let gone = root.join("My Music").join("gone.mp3");

    let mut present = track(&audio, "Tom & Jerry <Remix>", "Cat, \"The\" Mouse");
    present.album = "Cartoons".into();
    let present_id = library.add_track(&present).unwrap();
    library.set_analysis(present_id, &analysis(120.0, "8A")).unwrap();
    library
        .set_cues(
            present_id,
            &[
                cue(1, 1000, None, "Drop", Some("#28e214")),
                cue(3, 2000, Some(3000), "Loop", Some("#010203")),
                cue(0, 500, None, "", None),
            ],
        )
        .unwrap();
    library.set_user_rating(present_id, 4).unwrap();
    ImportedMeta {
        source: SourceKind::Xml,
        source_id: "1".into(),
        grid_locked: false,
        cues_locked: false,
        rating: None,
        color: Some("#ff0000".into()),
        cues: Vec::new(),
        grid: Vec::new(),
    }
    .write(&root.join("anlz").join(present_id.to_string()))
    .unwrap();

    let missing_id = library.add_track(&track(&gone, "Second", "Other")).unwrap();
    let playlist = library.create_playlist("Friday: Warmup").unwrap();
    library.set_playlist_tracks(playlist, &[missing_id, present_id]).unwrap();
    let empty = library.create_playlist("Empty").unwrap();
    Fixture { _dir: dir, root, library, audio, present_id, playlist, empty }
}

fn render(fx: &Fixture, id: i64, format: ExportFormat) -> String {
    fx.library.export_document(id).unwrap().render(format).unwrap()
}

fn assert_well_formed(xml: &str) {
    let mut stack: Vec<&str> = Vec::new();
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        let close = {
            let mut quote = None;
            let mut end = None;
            for (i, c) in after.char_indices() {
                match (quote, c) {
                    (None, '"') => quote = Some('"'),
                    (Some('"'), '"') => quote = None,
                    (None, '>') => {
                        end = Some(i);
                        break;
                    }
                    (Some(_), '<') => panic!("raw < inside an attribute value"),
                    _ => {}
                }
            }
            end.expect("tag is never closed")
        };
        let tag = &after[..close];
        rest = &after[close + 1..];
        if tag.starts_with('?') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop(), Some(name), "mismatched closing tag {name}");
        } else if !tag.ends_with('/') {
            stack.push(tag.split_whitespace().next().unwrap());
        }
    }
    assert!(stack.is_empty(), "unclosed tags {stack:?}");
    for (at, _) in xml.match_indices('&') {
        let entity = &xml[at..];
        let end = entity.find(';').expect("bare ampersand");
        assert!(
            ["&amp;", "&lt;", "&gt;", "&quot;", "&apos;"].contains(&&entity[..=end]) || entity[1..].starts_with('#'),
            "unknown entity {}",
            &entity[..=end]
        );
    }
}

#[test]
fn text_lists_artist_and_title_in_playlist_order() {
    let fx = fixture();
    assert_eq!(render(&fx, fx.playlist, ExportFormat::Text), "Other - Second\nCat, \"The\" Mouse - Tom & Jerry <Remix>\n");
    assert_eq!(fx.library.playlist_tracklist(fx.playlist).unwrap(), render(&fx, fx.playlist, ExportFormat::Text));
}

#[test]
fn m3u8_keeps_missing_files_and_absolute_paths() {
    let fx = fixture();
    let gone = std::path::absolute(fx.root.join("My Music").join("gone.mp3")).unwrap();
    let expected = format!(
        "#EXTM3U\n#EXTINF:4,Other - Second\n{}\n#EXTINF:4,Cat, \"The\" Mouse - Tom & Jerry <Remix>\n{}\n",
        gone.display(),
        fx.audio.display()
    );
    assert_eq!(render(&fx, fx.playlist, ExportFormat::M3u8), expected);
}

#[test]
fn csv_has_the_eight_columns_and_quotes_special_characters() {
    let fx = fixture();
    let gone = std::path::absolute(fx.root.join("My Music").join("gone.mp3")).unwrap();
    let expected = format!(
        "position,title,artist,album,bpm,key,duration,path\r\n\
         1,Second,Other,,120.00,,0:04,{}\r\n\
         2,Tom & Jerry <Remix>,\"Cat, \"\"The\"\" Mouse\",Cartoons,120.00,8A,0:04,{}\r\n",
        gone.display(),
        fx.audio.display()
    );
    assert_eq!(render(&fx, fx.playlist, ExportFormat::Csv), expected);
}

#[test]
fn empty_playlist_exports_empty_files_in_every_format() {
    let fx = fixture();
    assert_eq!(render(&fx, fx.empty, ExportFormat::M3u8), "#EXTM3U\n");
    assert_eq!(render(&fx, fx.empty, ExportFormat::Text), "");
    assert_eq!(render(&fx, fx.empty, ExportFormat::Csv), "position,title,artist,album,bpm,key,duration,path\r\n");
    for format in [ExportFormat::RekordboxXml, ExportFormat::TraktorNml] {
        assert_well_formed(&render(&fx, fx.empty, format));
    }
    let report = fx.library.export_document(fx.empty).unwrap().report();
    assert_eq!((report.written, report.missing), (0, 0));
}

#[test]
fn report_counts_written_and_missing_tracks() {
    let fx = fixture();
    let dest = fx.root.join("out.m3u8");
    let report = fx.library.export_playlist(fx.playlist, ExportFormat::M3u8, &dest).unwrap();
    assert_eq!((report.written, report.missing), (2, 1));
    assert!(report.missing_examples[0].ends_with("gone.mp3"));
    assert!(dest.exists());
}

#[test]
fn files_are_utf8_without_a_byte_order_mark() {
    let fx = fixture();
    for format in [
        ExportFormat::M3u8,
        ExportFormat::Csv,
        ExportFormat::Text,
        ExportFormat::TraktorNml,
        ExportFormat::RekordboxXml,
    ] {
        let dest = fx.root.join(format!("out.{}", format.extension()));
        fx.library.export_playlist(fx.playlist, format, &dest).unwrap();
        let bytes = std::fs::read(&dest).unwrap();
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(String::from_utf8(bytes).is_ok());
    }
}

#[test]
fn default_file_name_is_playlist_name_and_today() {
    let fx = fixture();
    let name = fx.library.export_file_name(fx.playlist, ExportFormat::Csv).unwrap();
    let today = chrono::Local::now().format("%Y-%m-%d");
    assert_eq!(name, format!("Friday_ Warmup {today}.csv"));
}

#[test]
fn rekordbox_xml_is_well_formed_and_escapes_special_characters() {
    let fx = fixture();
    let xml = render(&fx, fx.playlist, ExportFormat::RekordboxXml);
    assert_well_formed(&xml);
    assert!(xml.contains("Name=\"Tom &amp; Jerry &lt;Remix&gt;\""));
    assert!(xml.contains("Artist=\"Cat, &quot;The&quot; Mouse\""));
    assert!(xml.contains("Location=\"file://localhost"));
    assert!(xml.contains("Tom%20%26%20Jerry%20%231%20%C3%BC.wav"));
    assert!(xml.contains("Rating=\"204\""));
    assert!(xml.contains("Colour=\"0xFF0000\""));
    assert!(xml.contains("Tonality=\"8A\""));
}

#[test]
fn rekordbox_xml_round_trips_through_our_importer() {
    let fx = fixture();
    let xml_path = fx.root.join("export.xml");
    let report = fx.library.export_playlist(fx.playlist, ExportFormat::RekordboxXml, &xml_path).unwrap();
    assert_eq!((report.written, report.missing), (2, 1));

    let source = rekordbox::load_xml(&xml_path).unwrap();
    assert_eq!(source.tracks.len(), 2);
    let target_dir = tempfile::tempdir().unwrap();
    let target = LocalLibrary::open(&target_dir.path().join("library.db")).unwrap();
    let options = ImportOptions { include_missing: true, ..ImportOptions::default() };
    let import = target.import_rekordbox_from(&source, &options, &|_| {}).unwrap();
    assert_eq!((import.tracks_imported, import.playlists_imported), (2, 1));

    let playlists = target.get_all_playlists().unwrap();
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].name, "Friday: Warmup");
    let titles: Vec<String> =
        playlists[0].track_ids.iter().map(|&id| target.get_track(id as i64).unwrap().unwrap().title).collect();
    assert_eq!(titles, ["Second", "Tom & Jerry <Remix>"]);

    let imported = target.get_track(playlists[0].track_ids[1] as i64).unwrap().unwrap();
    let id = imported.id as i64;
    assert_eq!(imported.artist, "Cat, \"The\" Mouse");
    assert_eq!(imported.album, "Cartoons");
    assert_eq!(imported.source_path, fx.audio);
    assert_eq!((imported.tempo, imported.key.as_str()), (12000, "8A"));

    let meta = target.imported_meta(id).unwrap();
    assert_eq!(meta.rating, Some(4));
    assert_eq!(meta.color.as_deref(), Some("#ff0000"));

    let original = fx.library.get_analysis(fx.present_id).unwrap().unwrap().beat_grid.beats;
    let grid: Vec<(u32, u8, u32)> = meta.grid.iter().map(|b| (b.time_ms, b.bar_position, b.tempo)).collect();
    let expected: Vec<(u32, u8, u32)> = original.iter().map(|b| (b.time_ms, b.bar_position, b.tempo)).collect();
    assert_eq!(grid, expected);

    let cues = target.get_cues(id).unwrap();
    let want = fx.library.get_cues(fx.present_id).unwrap();
    assert_eq!(cues.len(), 3);
    for cue in &want {
        assert!(cues.contains(cue), "cue {cue:?} lost in the round trip: {cues:?}");
    }
    let missing = target.get_track(playlists[0].track_ids[0] as i64).unwrap().unwrap();
    assert_eq!(missing.title, "Second");
}

#[test]
fn folders_export_as_nested_xml_nodes_and_not_as_lists() {
    let fx = fixture();
    let folder = fx.library.create_folder("Sets", 0).unwrap();
    let inner = fx.library.create_playlist_in("Inner", folder as u32).unwrap();
    fx.library.set_playlist_tracks(inner, &[fx.present_id]).unwrap();
    fx.library.create_playlist_in("Nested empty", folder as u32).unwrap();

    let xml_path = fx.root.join("folder.xml");
    let report = fx.library.export_playlist(folder, ExportFormat::RekordboxXml, &xml_path).unwrap();
    assert_eq!((report.written, report.missing), (1, 0));
    let source = rekordbox::load_xml(&xml_path).unwrap();
    let sets = source.playlists.iter().find(|p| p.name == "Sets").unwrap();
    assert!(sets.is_folder);
    let inner = source.playlists.iter().find(|p| p.name == "Inner").unwrap();
    assert_eq!(inner.parent_id.as_deref(), Some(sets.source_id.as_str()));
    assert_eq!(inner.track_ids.len(), 1);

    assert_well_formed(&render(&fx, folder, ExportFormat::TraktorNml));
    let err = fx.library.export_playlist(folder, ExportFormat::M3u8, &fx.root.join("x.m3u8")).unwrap_err();
    assert!(err.to_string().contains("folder"));
}

#[test]
fn traktor_nml_has_collection_entries_cues_key_and_playlist_keys() {
    let fx = fixture();
    let nml = render(&fx, fx.playlist, ExportFormat::TraktorNml);
    assert_well_formed(&nml);
    assert!(nml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>"));
    assert!(nml.contains("<NML VERSION=\"19\">"));
    assert!(nml.contains("<COLLECTION ENTRIES=\"2\">"));
    assert!(nml.contains("TITLE=\"Tom &amp; Jerry &lt;Remix&gt;\" ARTIST=\"Cat, &quot;The&quot; Mouse\""));
    assert!(nml.contains("FILE=\"Tom &amp; Jerry #1 \u{fc}.wav\""));
    assert!(nml.contains("<TEMPO BPM=\"120.000000\" BPM_QUALITY=\"100\"/>"));
    assert!(nml.contains("<MUSICAL_KEY VALUE=\"21\"/>"));
    assert!(nml.contains("RANKING=\"204\""));
    assert!(nml.contains("NAME=\"AutoGrid\" DISPL_ORDER=\"0\" TYPE=\"4\" START=\"100.000000\""));
    assert!(nml.contains("NAME=\"Drop\" DISPL_ORDER=\"0\" TYPE=\"0\" START=\"1000.000000\" LEN=\"0.000000\" REPEATS=\"-1\" HOTCUE=\"0\""));
    assert!(nml.contains("NAME=\"Loop\" DISPL_ORDER=\"0\" TYPE=\"5\" START=\"2000.000000\" LEN=\"1000.000000\" REPEATS=\"-1\" HOTCUE=\"2\""));
    assert!(nml.contains("START=\"500.000000\" LEN=\"0.000000\" REPEATS=\"-1\" HOTCUE=\"-1\""));
    assert!(nml.contains("<NODE TYPE=\"PLAYLIST\" NAME=\"Friday: Warmup\">"));
    assert!(nml.contains("<PLAYLIST ENTRIES=\"2\" TYPE=\"LIST\" UUID=\""));
    let entries = nml.matches("<PRIMARYKEY TYPE=\"TRACK\" KEY=\"").count();
    assert_eq!(entries, 2);
    let location = nml.lines().find(|l| l.contains("<LOCATION") && l.contains("Jerry")).unwrap().to_string();
    let dir = location.split("DIR=\"").nth(1).unwrap().split('"').next().unwrap();
    let file = location.split("FILE=\"").nth(1).unwrap().split('"').next().unwrap();
    let volume = location.split("VOLUME=\"").nth(1).unwrap().split('"').next().unwrap();
    assert!(nml.contains(&format!("KEY=\"{volume}{dir}{file}\"")));
}
