use std::collections::BTreeMap;
use std::path::Path;
use std::time::SystemTime;

use pioneer_usb_writer::models::{AnalysisResult, BeatGrid, Playlist, Track, WaveformPreview};
use pioneer_usb_writer::stick::dump::{dump_stick, DumpOptions};
use pioneer_usb_writer::stick::emit::{copy_stick, write_usb_v2};
use pioneer_usb_writer::stick::plan::{apply, plan, Ownership, Wanted};
use pioneer_usb_writer::stick::read::SETTINGS_FILES;
use pioneer_usb_writer::stick::{LibraryKind, Stick, StickDevice};
use tempfile::TempDir;

const PLAYER_AND_REKORDBOX_FILES: [&str; 9] = [
    "PIONEER/DEVSETTING.DAT",
    "PIONEER/MYSETTING.DAT",
    "PIONEER/MYSETTING2.DAT",
    "PIONEER/DJMMYSETTING.DAT",
    "PIONEER/djprofile.nxs",
    "PIONEER/USBANLZ/USBMNG.DAT",
    "PIONEER/rekordbox/RBFLTR.DAT",
    "PIONEER/rekordbox/playlists3.sync",
    "PIONEER REC/REC0001.WAV",
];
const EMPTY_FOLDERS: [&str; 5] = ["PIONEER/CDJ", "PIONEER/MPJ", "PIONEER/log", "PIONEER/TrashBox", "PRINT"];

fn track(source_dir: &Path, id: u32, name: &str) -> Track {
    let source_path = source_dir.join(format!("{name}.wav"));
    std::fs::write(&source_path, vec![id as u8; 2048]).unwrap();
    Track {
        usb_path: format!("/Contents/{name} artist/{name}.wav"),
        source_path,
        title: name.into(),
        artist: format!("{name} artist"),
        album: String::new(),
        genre: String::new(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 0,
        disc_number: 0,
        track_number: 0,
        tempo: 12800,
        key: "8A".into(),
        duration_secs: 1.0,
        sample_rate: 44100,
        bitrate: 1411,
        file_size: 2048,
        id,
        artwork: None,
    }
}

fn analysis() -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid { beats: Vec::new() },
        waveform: WaveformPreview { data: [0; 400] },
        bpm: 128.0,
        key: "8A".into(),
        energy: None,
        cue_points: Vec::new(),
        color_waveform: None,
    }
}

fn device() -> StickDevice {
    StickDevice { name: "RB".into(), created_date: "2026-11-22".into(), ..StickDevice::default() }
}

fn rekordbox_like_stick(source: &Path) -> TempDir {
    let usb = TempDir::new().unwrap();
    let theirs = vec![track(source, 1, "their one"), track(source, 2, "their two")];
    let playlists = vec![Playlist { id: 1, name: "Their Set".into(), track_ids: vec![1, 2], ..Default::default() }];
    write_usb_v2(usb.path(), &theirs, &[analysis(), analysis()], &playlists, device()).unwrap();
    for (index, file) in PLAYER_AND_REKORDBOX_FILES.iter().enumerate() {
        let path = usb.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0x40 + index as u8; 140 + index]).unwrap();
    }
    for folder in EMPTY_FOLDERS {
        std::fs::create_dir_all(usb.path().join(folder)).unwrap();
    }
    usb
}

fn stamps(root: &Path) -> BTreeMap<&'static str, (Vec<u8>, SystemTime)> {
    PLAYER_AND_REKORDBOX_FILES
        .iter()
        .map(|file| {
            let path = root.join(file);
            (*file, (std::fs::read(&path).unwrap(), std::fs::metadata(&path).unwrap().modified().unwrap()))
        })
        .collect()
}

#[test]
fn settings_and_player_files_survive_adding_updating_and_removing_our_tracks() {
    let source = TempDir::new().unwrap();
    let usb = rekordbox_like_stick(source.path());
    let before = stamps(usb.path());

    let mut ours = vec![track(source.path(), 10, "ours a"), track(source.path(), 11, "ours b")];
    let mut analyses = vec![analysis(), analysis()];
    let playlists = vec![Playlist { id: 1, name: "Ours".into(), track_ids: vec![10, 11], ..Default::default() }];
    let mut owned = Ownership::default();
    for round in 0..3 {
        match round {
            1 => {
                ours[0].title = "ours a (edit)".into();
                analyses[1].bpm = 130.0;
            }
            2 => {
                ours.clear();
                analyses.clear();
            }
            _ => {}
        }
        let lists = if ours.is_empty() { Vec::new() } else { playlists.clone() };
        let wanted = Wanted { tracks: &ours, analyses: &analyses, playlists: &lists, labels: &[] };
        let planned = plan(&Stick::read(usb.path()).unwrap(), wanted, &owned, device()).unwrap();
        assert!(!planned.is_noop(), "round {round}");
        apply(usb.path(), &planned, wanted).unwrap();
        owned = planned.ownership;

        assert_eq!(stamps(usb.path()), before, "round {round}");
        for folder in EMPTY_FOLDERS {
            assert!(usb.path().join(folder).is_dir(), "{folder} in round {round}");
        }
    }
    let stick = Stick::read(usb.path()).unwrap();
    assert_eq!(stick.track_count(), 2);
    assert_eq!(stick.settings_files().len(), SETTINGS_FILES.len());
    assert!(!usb.path().join("Contents/ours a artist").exists());
}

#[test]
fn a_copied_stick_gets_the_settings_files_and_the_dump_lists_them() {
    let source = TempDir::new().unwrap();
    let usb = rekordbox_like_stick(source.path());
    let copy = TempDir::new().unwrap();
    copy_stick(&Stick::read(usb.path()).unwrap(), LibraryKind::DeviceLibrary, copy.path(), true).unwrap();

    for name in SETTINGS_FILES {
        assert_eq!(
            std::fs::read(copy.path().join("PIONEER").join(name)).unwrap(),
            std::fs::read(usb.path().join("PIONEER").join(name)).unwrap(),
            "{name}"
        );
    }
    let settings_lines = |root: &Path| -> Vec<String> {
        dump_stick(root, DumpOptions::default())
            .unwrap()
            .lines()
            .filter(|line| SETTINGS_FILES.iter().any(|name| line.starts_with(&format!("file {name} "))))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(settings_lines(copy.path()).len(), SETTINGS_FILES.len());
    assert_eq!(settings_lines(copy.path()), settings_lines(usb.path()));
    assert!(dump_stick(usb.path(), DumpOptions::default()).unwrap().contains("not-compared rekordbox/RBFLTR.DAT"));
}

#[test]
fn a_stick_without_settings_files_gets_none() {
    let source = TempDir::new().unwrap();
    let usb = TempDir::new().unwrap();
    let ours = vec![track(source.path(), 1, "solo")];
    let analyses = vec![analysis()];
    let wanted = Wanted { tracks: &ours, analyses: &analyses, playlists: &[], labels: &[] };
    let planned = plan(&Stick::read(usb.path()).unwrap(), wanted, &Ownership::default(), device()).unwrap();
    apply(usb.path(), &planned, wanted).unwrap();
    let stick = Stick::read(usb.path()).unwrap();
    assert!(stick.settings_files().is_empty());
    for name in SETTINGS_FILES {
        assert!(!usb.path().join("PIONEER").join(name).exists());
    }
}
