use std::collections::BTreeSet;
use std::path::Path;

use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, CuePoint, Playlist, Track, WaveformPreview};
use pioneer_usb_writer::stick::dump::{dump_stick, DumpOptions};
use pioneer_usb_writer::stick::plan::{apply, apply_with, plan, Ownership, Wanted};
use pioneer_usb_writer::stick::publish::{self, Recovery, Step, JOURNAL_FILE, PREVIOUS_DIR, STAGE_DIR};
use pioneer_usb_writer::stick::verify::verify_stick;
use pioneer_usb_writer::stick::{Stick, StickDevice};
use tempfile::TempDir;

const RAW: DumpOptions = DumpOptions { raw: true, normalise: false, pages: true };

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
    bytes.extend(std::iter::repeat_n(0u8, data_len as usize));
    std::fs::write(path, bytes).unwrap();
}

fn cover_png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(32, 32, image::Rgb([200, 30, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn track(source_dir: &Path, id: u32, file: &str, seconds: u32) -> Track {
    let source_path = source_dir.join(file);
    write_wav(&source_path, seconds);
    Track {
        usb_path: format!("/Contents/Artist {id}/{file}"),
        source_path: source_path.clone(),
        title: format!("Title {id}"),
        artist: format!("Artist {id}"),
        album: "Album".into(),
        genre: "House".into(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2020,
        disc_number: 1,
        track_number: 1,
        tempo: 12800,
        key: "8A".into(),
        duration_secs: seconds as f64,
        sample_rate: 8000,
        bitrate: 128,
        file_size: std::fs::metadata(&source_path).unwrap().len(),
        id,
        artwork: None,
    }
}

fn analysis(cue_ms: &[u32]) -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid { beats: (0..4).map(|i| Beat { bar_position: i as u8 + 1, time_ms: i * 468, tempo: 12800 }).collect() },
        waveform: WaveformPreview { data: [0xA4; 400] },
        bpm: 128.0,
        key: "8A".into(),
        energy: None,
        cue_points: cue_ms
            .iter()
            .enumerate()
            .map(|(index, &time_ms)| CuePoint { hot_cue_number: index as u32 + 1, time_ms, loop_time_ms: None, ..Default::default() })
            .collect(),
        color_waveform: None,
    }
}

struct Library {
    tracks: Vec<Track>,
    analyses: Vec<AnalysisResult>,
    playlists: Vec<Playlist>,
}

impl Library {
    fn wanted(&self) -> Wanted<'_> {
        Wanted { tracks: &self.tracks, analyses: &self.analyses, playlists: &self.playlists, labels: &[] }
    }
}

fn first_generation(source: &Path) -> Library {
    let mut covered = track(source, 2, "two.wav", 1);
    covered.artwork = Some(cover_png());
    Library {
        tracks: vec![track(source, 1, "one.wav", 1), covered, track(source, 3, "three.wav", 1)],
        analyses: vec![analysis(&[]), analysis(&[100]), analysis(&[])],
        playlists: vec![Playlist { id: 1, name: "Set".into(), track_ids: vec![1, 2, 3], ..Default::default() }],
    }
}

fn second_generation(source: &Path) -> Library {
    let first = first_generation(source);
    let longer = track(source, 3, "three.wav", 2);
    Library {
        tracks: vec![first.tracks[0].clone(), longer, track(source, 4, "four.wav", 1)],
        analyses: vec![analysis(&[250, 500]), analysis(&[]), analysis(&[])],
        playlists: vec![Playlist { id: 1, name: "Set".into(), track_ids: vec![4, 1, 3], ..Default::default() }],
    }
}

fn device() -> StickDevice {
    StickDevice { name: "FFTEST".into(), created_date: "2026-11-22".into(), ..StickDevice::default() }
}

fn stick_with_first_generation(source: &Path) -> (TempDir, Ownership) {
    let usb = TempDir::new().unwrap();
    let library = first_generation(source);
    let planned = plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &Ownership::default(), device()).unwrap();
    apply(usb.path(), &planned, library.wanted()).unwrap();
    (usb, planned.ownership)
}

fn leftovers(root: &Path) -> Vec<&'static str> {
    [STAGE_DIR, PREVIOUS_DIR, JOURNAL_FILE].into_iter().filter(|name| root.join(name).exists()).collect()
}

#[test]
fn a_sync_killed_at_any_step_leaves_the_old_or_the_new_stick_after_recovery() {
    let source = TempDir::new().unwrap();
    let (reference, owned) = stick_with_first_generation(source.path());
    let old_dump = dump_stick(reference.path(), RAW).unwrap();
    let library = second_generation(source.path());
    let planned = plan(&Stick::read(reference.path()).unwrap(), library.wanted(), &owned, device()).unwrap();
    apply(reference.path(), &planned, library.wanted()).unwrap();
    let new_dump = dump_stick(reference.path(), RAW).unwrap();
    assert_ne!(old_dump, new_dump);
    assert!(new_dump.contains("audio Contents/Artist 4/four.wav") && !new_dump.contains("Artist 2"));

    let mut outcomes = BTreeSet::new();
    let mut killed_steps = Vec::new();
    for kill_at in 0.. {
        let (usb, owned) = stick_with_first_generation(source.path());
        let library = second_generation(source.path());
        let planned = plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &owned, device()).unwrap();
        let mut seen = 0;
        let mut killed = None;
        let result = apply_with(usb.path(), &planned, library.wanted(), &mut |step: &Step| {
            if seen == kill_at {
                killed = Some(step.clone());
                anyhow::bail!("the stick was pulled");
            }
            seen += 1;
            Ok(())
        });
        let Some(step) = killed else {
            result.unwrap();
            assert_eq!(dump_stick(usb.path(), RAW).unwrap(), new_dump);
            break;
        };
        assert!(result.is_err());
        killed_steps.push(step.clone());

        let recovery = publish::recover(usb.path()).unwrap();
        assert_eq!(leftovers(usb.path()), Vec::<&str>::new(), "after {step:?}");
        let dump = dump_stick(usb.path(), RAW).unwrap();
        let outcome = if dump == old_dump {
            "old"
        } else if dump == new_dump {
            "new"
        } else {
            panic!("a mixed stick after being killed at {step:?} ({recovery:?})");
        };
        outcomes.insert(outcome);
        assert_eq!(outcome == "old", step == Step::WriteJournal, "{step:?}");
        let owned_now = if outcome == "new" { planned.ownership.clone() } else { owned };
        assert_eq!(verify_stick(usb.path(), &owned_now.paths).unwrap(), Vec::new(), "after {step:?}");
        let again = plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &owned_now, device()).unwrap();
        apply(usb.path(), &again, library.wanted()).unwrap();
        assert_eq!(dump_stick(usb.path(), RAW).unwrap(), new_dump, "after {step:?}");
        assert_eq!(publish::recover(usb.path()).unwrap(), Recovery::NothingToDo);
    }
    assert_eq!(outcomes, BTreeSet::from(["old", "new"]));
    assert!(killed_steps.iter().any(|step| matches!(step, Step::Move(_))));
    assert!(killed_steps.iter().any(|step| matches!(step, Step::Delete(_))));
    assert!(killed_steps.contains(&Step::RemoveJournal) && killed_steps.contains(&Step::CleanUp));
    assert!(killed_steps.len() > 15, "{}", killed_steps.len());
}

#[test]
fn staging_left_without_a_journal_is_thrown_away() {
    let source = TempDir::new().unwrap();
    let (usb, _) = stick_with_first_generation(source.path());
    let before = dump_stick(usb.path(), RAW).unwrap();
    std::fs::create_dir_all(usb.path().join(STAGE_DIR).join("PIONEER/rekordbox")).unwrap();
    std::fs::write(usb.path().join(STAGE_DIR).join("PIONEER/rekordbox/export.pdb"), b"half written").unwrap();

    assert!(publish::pending(usb.path()));
    assert_eq!(publish::recover(usb.path()).unwrap(), Recovery::DiscardedUnfinishedStaging);
    assert_eq!(leftovers(usb.path()), Vec::<&str>::new());
    assert_eq!(dump_stick(usb.path(), RAW).unwrap(), before);
    assert_eq!(publish::recover(usb.path()).unwrap(), Recovery::NothingToDo);
}

#[test]
fn a_new_sync_does_not_start_over_an_interrupted_one() {
    let source = TempDir::new().unwrap();
    let (usb, owned) = stick_with_first_generation(source.path());
    let library = second_generation(source.path());
    let planned = plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &owned, device()).unwrap();
    let mut steps = 0;
    apply_with(usb.path(), &planned, library.wanted(), &mut |_| {
        steps += 1;
        anyhow::ensure!(steps < 4, "the stick was pulled");
        Ok(())
    })
    .unwrap_err();

    let error = apply(usb.path(), &planned, library.wanted()).unwrap_err();
    assert!(format!("{error:#}").contains("interrupted"), "{error:#}");
    assert_eq!(publish::recover(usb.path()).unwrap(), Recovery::FinishedInterruptedPublish);
    assert!(plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &planned.ownership, device()).unwrap().is_noop());
}

#[test]
fn a_missing_source_file_changes_nothing_on_the_stick() {
    let source = TempDir::new().unwrap();
    let (usb, owned) = stick_with_first_generation(source.path());
    let before = dump_stick(usb.path(), RAW).unwrap();
    let library = second_generation(source.path());
    std::fs::remove_file(&library.tracks[2].source_path).unwrap();
    let planned = plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &owned, device()).unwrap();

    let error = apply(usb.path(), &planned, library.wanted()).unwrap_err();
    assert!(format!("{error:#}").contains("Nothing was changed"), "{error:#}");
    assert_eq!(leftovers(usb.path()), Vec::<&str>::new());
    assert_eq!(dump_stick(usb.path(), RAW).unwrap(), before);
}

#[test]
fn a_plan_that_fails_its_check_changes_nothing_on_the_stick() {
    let source = TempDir::new().unwrap();
    let (usb, owned) = stick_with_first_generation(source.path());
    let before = dump_stick(usb.path(), RAW).unwrap();
    let library = second_generation(source.path());
    let mut planned = plan(&Stick::read(usb.path()).unwrap(), library.wanted(), &owned, device()).unwrap();
    planned.model.playlists[0].track_ids.push(999);

    let error = apply(usb.path(), &planned, library.wanted()).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("did not pass its check") && message.contains("999"), "{message}");
    assert_eq!(leftovers(usb.path()), Vec::<&str>::new());
    assert_eq!(dump_stick(usb.path(), RAW).unwrap(), before);
}

#[test]
fn verify_finds_missing_audio_and_a_foreign_analysis_file() {
    let source = TempDir::new().unwrap();
    let (usb, owned) = stick_with_first_generation(source.path());
    assert_eq!(verify_stick(usb.path(), &owned.paths).unwrap(), Vec::new());

    let stick = Stick::read(usb.path()).unwrap();
    let model = stick.library().unwrap();
    let first = model.track(1).unwrap();
    let third = model.track(3).unwrap();
    std::fs::remove_file(stick.audio_path(first)).unwrap();
    std::fs::copy(stick.analysis_path(first).unwrap(), stick.analysis_path(third).unwrap()).unwrap();

    let problems = verify_stick(usb.path(), &owned.paths).unwrap();
    let about = |path: &str, text: &str| problems.iter().any(|p| p.subject == path && p.message.contains(text));
    assert!(about(&first.path, "audio file is missing"), "{problems:?}");
    assert!(about(&third.path, "names another track"), "{problems:?}");
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(verify_stick(usb.path(), &BTreeSet::new()).unwrap(), Vec::new());
}
