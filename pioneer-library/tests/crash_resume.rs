use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use pioneer_library::{analysis_stage, remove_stale_staging, LocalLibrary};
use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, Track, WaveformPreview};

const TRACKS: i64 = 160;
const ROUNDS: u64 = 6;
const CHILD_DIR: &str = "FF_CRASH_CHILD_DIR";
const CHILD_LOG: &str = "FF_CRASH_CHILD_LOG";
const CHILD_PACE: Duration = Duration::from_millis(8);
const FAST: u8 = analysis_stage::WAVEFORM | analysis_stage::KEY;

fn track(index: i64) -> Track {
    Track {
        source_path: PathBuf::from(format!("/music/crash-{index}.wav")),
        usb_path: format!("/Contents/Unknown Artist/crash-{index}.wav"),
        title: format!("t{index}"),
        artist: "a".into(),
        album: String::new(),
        genre: String::new(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 0,
        disc_number: 0,
        track_number: 0,
        tempo: 0,
        key: String::new(),
        duration_secs: 240.0,
        sample_rate: 44_100,
        bitrate: 320,
        file_size: 1,
        id: 0,
        artwork: None,
    }
}

fn analysis(id: i64, fast: bool) -> AnalysisResult {
    let bpm = 100.0 + (id % 60) as f64;
    let beats = (0..600u32)
        .map(|n| Beat { bar_position: (n % 4) as u8 + 1, time_ms: 120 + n * 450 + (id as u32 % 7), tempo: (bpm * 100.0) as u32 })
        .collect();
    AnalysisResult {
        beat_grid: BeatGrid { beats: if fast { Vec::new() } else { beats } },
        waveform: WaveformPreview { data: [(id % 31) as u8 + 1; 400] },
        bpm: if fast { 0.0 } else { bpm },
        key: format!("{}A", id % 12 + 1),
        energy: None,
        cue_points: Vec::new(),
        color_waveform: None,
    }
}

fn store(lib: &LocalLibrary, id: i64, analysis: &AnalysisResult, stages: u8) {
    let staged = lib.begin_analysis(id, analysis, stages).unwrap().stage().unwrap();
    assert!(lib.commit_analysis(staged, None).unwrap());
}

fn analyse_pending(lib: &LocalLibrary, mut note: impl FnMut(i64, &str)) {
    for id in lib.get_unanalyzed_track_ids().unwrap() {
        let stages = lib.analysis_stages(id).unwrap().unwrap_or(0);
        if stages & FAST != FAST {
            note(id, "fast");
            store(lib, id, &analysis(id, true), FAST);
        }
        note(id, "grid");
        store(lib, id, &analysis(id, false), analysis_stage::ALL);
    }
}

fn library_with_tracks(dir: &Path) -> LocalLibrary {
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    for index in 1..=TRACKS {
        lib.add_track(&track(index)).unwrap();
    }
    lib
}

fn integrity(dir: &Path) {
    let conn = rusqlite::Connection::open(dir.join("library.db")).unwrap();
    let verdict: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(verdict, "ok");
}

fn file_bytes(dir: &Path, id: i64) -> Vec<Option<Vec<u8>>> {
    ["ANLZ0000.DAT", "ANLZ0000.EXT", "ANLZ0000.2EX"]
        .iter()
        .map(|name| std::fs::read(dir.join("anlz").join(id.to_string()).join(name)).ok())
        .collect()
}

fn rows(lib: &LocalLibrary) -> Vec<(i64, Option<u8>, Option<(u64, String)>)> {
    (1..=TRACKS)
        .map(|id| (id, lib.analysis_stages(id).unwrap(), lib.get_analysis(id).unwrap().map(|a| (a.bpm.to_bits(), a.key))))
        .collect()
}

fn staging_folders(dir: &Path) -> usize {
    let Ok(tracks) = std::fs::read_dir(dir.join("anlz")) else { return 0 };
    tracks
        .flatten()
        .flat_map(|track| std::fs::read_dir(track.path()).into_iter().flatten().flatten())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".staging-"))
        .count()
}

#[test]
#[ignore = "child process of kill_nine_rounds_then_resume_matches_an_uninterrupted_run"]
fn crash_child() {
    let dir = PathBuf::from(std::env::var(CHILD_DIR).expect(CHILD_DIR));
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(std::env::var(CHILD_LOG).expect(CHILD_LOG)).unwrap();
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    analyse_pending(&lib, |id, pass| {
        writeln!(log, "{id} {pass}").unwrap();
        log.flush().unwrap();
        std::thread::sleep(CHILD_PACE);
    });
}

fn run_child(dir: &Path, log: &Path, kill_after: Option<Duration>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child", "--ignored", "--nocapture", "--test-threads=1"])
        .env(CHILD_DIR, dir)
        .env(CHILD_LOG, log)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    match kill_after {
        Some(after) => {
            std::thread::sleep(after);
            child.kill().unwrap();
            child.wait().unwrap();
        }
        None => assert!(child.wait().unwrap().success()),
    }
}

fn logged(log: &Path) -> Vec<(i64, String)> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(id, pass)| (id.parse().unwrap(), pass.to_string()))
        .collect()
}

#[test]
fn kill_nine_rounds_then_resume_matches_an_uninterrupted_run() {
    let reference_dir = tempfile::tempdir().unwrap();
    let reference = library_with_tracks(reference_dir.path());
    analyse_pending(&reference, |_, _| {});

    let dir = tempfile::tempdir().unwrap();
    drop(library_with_tracks(dir.path()));
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut complete_before: BTreeSet<i64> = BTreeSet::new();
    let mut killed_mid_work = 0;

    for round in 0..ROUNDS {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let log = dir.path().join(format!("round-{round}.log"));
        run_child(dir.path(), &log, Some(Duration::from_millis(150 + (seed >> 33) % 450)));

        let removed = remove_stale_staging(&dir.path().join("library.db"));
        killed_mid_work += removed;
        assert_eq!(staging_folders(dir.path()), 0);
        integrity(dir.path());
        let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();

        let redone: Vec<_> = logged(&log).into_iter().filter(|(id, _)| complete_before.contains(id)).collect();
        assert!(redone.is_empty(), "round {round} redid finished tracks: {redone:?}");

        for id in 1..=TRACKS {
            let stages = lib.analysis_stages(id).unwrap();
            if stages == Some(analysis_stage::ALL) {
                complete_before.insert(id);
                assert_eq!(file_bytes(dir.path(), id), file_bytes(reference_dir.path(), id), "track {id} files");
                assert_eq!(lib.get_analysis(id).unwrap().unwrap().bpm, analysis(id, false).bpm, "track {id} bpm");
            }
            if stages.is_some() {
                lib.get_analysis(id).unwrap_or_else(|e| panic!("track {id} analysis unreadable after kill: {e}"));
            }
        }
    }
    assert!(!complete_before.is_empty() && complete_before.len() < TRACKS as usize, "kills landed outside the run: {} complete", complete_before.len());
    eprintln!("{} of {TRACKS} tracks complete after {ROUNDS} kills, {killed_mid_work} staging folders left behind", complete_before.len());

    let last = dir.path().join("last.log");
    run_child(dir.path(), &last, None);
    let redone: Vec<_> = logged(&last).into_iter().filter(|(id, _)| complete_before.contains(id)).collect();
    assert!(redone.is_empty(), "final resume redid finished tracks: {redone:?}");

    integrity(dir.path());
    let lib = LocalLibrary::open(&dir.path().join("library.db")).unwrap();
    assert!(lib.get_unanalyzed_track_ids().unwrap().is_empty());
    assert_eq!(rows(&lib), rows(&reference));
    for id in 1..=TRACKS {
        assert_eq!(file_bytes(dir.path(), id), file_bytes(reference_dir.path(), id), "track {id} files");
    }
}
