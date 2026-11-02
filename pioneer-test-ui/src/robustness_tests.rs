use std::collections::{BTreeMap, HashSet};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use fourfour_analysis::analyze::Control;
use fourfour_analysis::decode::decode_mono;
use fourfour_analysis::service::{locate_models, waveform_only_result, Analyzer, Pass, PoolOptions, PoolPlan};
use pioneer_library::{analysis_stage, file_status, FileStatus, LocalLibrary};
use pioneer_usb_writer::models::AnalysisResult;

use crate::analysis_backend::{BackendEnv, PartialResultSink, Runner, TrackEngine};
use crate::analysis_run::{run_with_runner, AnalysisCoordinator, AnalysisSink, LogLevel, RunOutcome, SharedLibrary};
use crate::import_plan::{guarded_read, plan_for_library, scan_paths, store_pending_covers, survey_files, ImportPlan, ImportPlans};
use crate::messy_fixtures::*;
use crate::track_infos::{build_track_infos, list_track_infos};

const PIPELINE_BUDGET: Duration = Duration::from_secs(180);

struct DecodeEngine;

impl TrackEngine for DecodeEngine {
    fn plan(&self, jobs: usize) -> PoolPlan {
        PoolPlan { workers: 2.min(jobs.max(1)), threads_per_track: 1 }
    }

    fn analyze(&self, path: &Path, _control: &Control, _threads: usize, pass: Pass, _on_partial: PartialResultSink) -> Result<AnalysisResult, String> {
        let mono = decode_mono(path).map_err(|e| e.to_string())?;
        let waves = fourfour_analysis::waveform::compute(&mono.samples, mono.rate);
        let mut result = waveform_only_result(&waves);
        if pass != Pass::Fast {
            result.bpm = 120.0;
            result.key = "1A".into();
        }
        Ok(result)
    }
}

fn engine() -> (Arc<dyn TrackEngine>, &'static str) {
    static REAL: OnceLock<Option<Arc<Analyzer>>> = OnceLock::new();
    let real = REAL.get_or_init(|| locate_models(None).and_then(|dir| Analyzer::load(&dir, PoolOptions { max_threads: 2, max_workers: Some(2), ..PoolOptions::default() }).ok().map(Arc::new)));
    match real {
        Some(analyzer) => (analyzer.clone(), "Analyzer"),
        None => (Arc::new(DecodeEngine), "DecodeEngine"),
    }
}

struct Sink {
    lines: Mutex<Vec<String>>,
}

impl AnalysisSink for Sink {
    fn log(&self, level: LogLevel, message: String) {
        self.lines.lock().unwrap().push(format!("{}: {message}", level.as_str()));
    }

    fn progress(&self, _current: u32, _total: u32, _message: String, _track_id: i64) {}

    fn partial(&self, _track_id: i64) {}

    fn complete(&self) {}
}

#[derive(Debug)]
struct Row {
    name: String,
    path: PathBuf,
    id: i64,
    stages: Option<u8>,
}

impl Row {
    fn analysed(&self) -> bool {
        self.stages == Some(analysis_stage::ALL)
    }
}

struct Run {
    plan: ImportPlan,
    rows: Vec<Row>,
    finished: Option<(u32, u32)>,
    log: Vec<String>,
    engine: &'static str,
    library: SharedLibrary,
    _db: Arc<Scratch>,
}

impl Run {
    fn row(&self, name: &str) -> &Row {
        self.rows.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("{name} not in the library; library has {:?}", self.names()))
    }

    fn has(&self, name: &str) -> bool {
        self.rows.iter().any(|r| r.name == name)
    }

    fn names(&self) -> Vec<&str> {
        self.rows.iter().map(|r| r.name.as_str()).collect()
    }

    fn assert_analysed(&self, names: &[&str]) {
        for name in names {
            assert!(self.row(name).analysed(), "{name} must be imported and analysed: {:?}\n{}", self.row(name), self.log.join("\n"));
        }
    }

    fn assert_library_healthy(&self) {
        let lib = self.library.lock().unwrap();
        let infos = build_track_infos(&lib).expect("track list builds");
        assert_eq!(infos.len(), self.rows.len());
        assert_eq!(list_track_infos(&lib).expect("plain list builds").len(), self.rows.len());
        lib.get_all_tracks().expect("tracks load");
        lib.duplicate_candidates().expect("duplicate candidates load");
        lib.track_files().expect("track files load");
        for row in &self.rows {
            let track = lib.get_track(row.id).unwrap().expect("track row loads");
            assert_eq!(track.source_path, row.path);
            if row.analysed() {
                lib.get_analysis(row.id).unwrap().unwrap_or_else(|| panic!("analysis of {} reads back", row.name));
            }
        }
    }

    fn report(&self, case: &str) {
        let mut by_state: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for row in &self.rows {
            let state = if row.analysed() { "analysed" } else if row.stages.is_some() { "partial" } else { "imported-not-analysed" };
            by_state.entry(state).or_default().push(&row.name);
        }
        let short = |names: &Vec<&str>| names.iter().map(|n| n.chars().take(40).collect::<String>()).collect::<Vec<_>>().join(", ");
        eprintln!(
            "CASE {case} [{}] new={} known={} dup={} unsupported={} unreadable={} finished={:?}",
            self.engine, self.plan.new_count, self.plan.already_in_library, self.plan.duplicates.len(), self.plan.skipped.unsupported, self.plan.skipped.unreadable, self.finished
        );
        for (state, names) in by_state {
            eprintln!("  {state}: {}", short(&names));
        }
    }
}

fn with_deadline<T: Send + 'static>(what: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        sender.send(outcome).ok();
    });
    match receiver.recv_timeout(PIPELINE_BUDGET) {
        Ok(Ok(value)) => value,
        Ok(Err(panic)) => {
            let message = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            panic!("{what}: PANIC {message}")
        }
        Err(RecvTimeoutError::Timeout) => panic!("{what}: HANG, no result after {PIPELINE_BUDGET:?}"),
        Err(RecvTimeoutError::Disconnected) => panic!("{what}: worker vanished"),
    }
}

struct Pipeline {
    db_dir: Arc<Scratch>,
    library: SharedLibrary,
    plans: ImportPlans,
}

impl Pipeline {
    fn new(name: &str) -> Pipeline {
        let db_dir = Scratch::new(&format!("lib-{name}"));
        let library = LocalLibrary::open(&db_dir.path().join("library.db")).unwrap();
        Pipeline { db_dir: Arc::new(db_dir), library: Arc::new(Mutex::new(library)), plans: ImportPlans::default() }
    }

    fn import(&self, paths: &[String], on_file: &mut dyn FnMut()) -> ImportPlan {
        let known = self.library.lock().unwrap().track_paths().unwrap();
        let survey = survey_files(paths);
        let scanned = scan_paths(&survey, &known, on_file);
        let plan = plan_for_library(&self.plans, &self.library.lock().unwrap(), scanned, &survey).unwrap();
        let (tracks, _) = self.plans.take(plan.token).unwrap().resolve(false);
        crate::add_scanned_tracks(&self.library.lock().unwrap(), tracks).unwrap();
        store_pending_covers(&self.library, None, &mut |_| {}).unwrap();
        plan
    }

    fn analyse(&self) -> (Option<(u32, u32)>, Vec<String>, &'static str) {
        let (runner, name) = engine();
        let work = crate::analysis_run::resume_work(&self.library.lock().unwrap(), true).unwrap();
        let sink = Arc::new(Sink { lines: Mutex::new(Vec::new()) });
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let outcome = runtime
            .block_on(run_with_runner(self.library.clone(), AnalysisCoordinator::default(), work, sink.clone(), BackendEnv::default(), Some(Runner::Rust { engine: runner })))
            .expect("the analysis run ends without an error");
        let finished = match outcome {
            RunOutcome::Finished { errors, total } => Some((errors, total)),
            _ => None,
        };
        let lines = sink.lines.lock().unwrap().clone();
        (finished, lines, name)
    }

    fn snapshot(&self, plan: ImportPlan, finished: Option<(u32, u32)>, log: Vec<String>, engine: &'static str) -> Run {
        let lib = self.library.lock().unwrap();
        let mut rows: Vec<Row> = lib
            .track_files()
            .unwrap()
            .into_iter()
            .map(|file| Row {
                name: file.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                id: file.track_id,
                stages: lib.analysis_stages(file.track_id).unwrap(),
                path: file.path,
            })
            .collect();
        rows.sort_by(|a, b| a.path.cmp(&b.path));
        drop(lib);
        Run { plan, rows, finished, log, engine, library: self.library.clone(), _db: self.db_dir.clone() }
    }

    fn run(&self, paths: &[String]) -> Run {
        let plan = self.import(paths, &mut || {});
        let (finished, log, engine) = self.analyse();
        self.snapshot(plan, finished, log, engine)
    }
}

fn drop_and_analyse(case: &'static str, paths: Vec<String>) -> Run {
    with_deadline(case, move || {
        let pipeline = Pipeline::new(case);
        let run = pipeline.run(&paths);
        run.assert_library_healthy();
        run.report(case);
        run
    })
}

fn drop_and_import(case: &'static str, paths: Vec<String>) -> Run {
    with_deadline(case, move || {
        let pipeline = Pipeline::new(case);
        let plan = pipeline.import(&paths, &mut || {});
        let run = pipeline.snapshot(plan, None, Vec::new(), "import only");
        run.assert_library_healthy();
        run.report(case);
        run
    })
}

fn folder_of(scratch: &Scratch, name: &str) -> PathBuf {
    let dir = scratch.path().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn good_files(dir: &Path, encoded: &Encoded) -> Vec<String> {
    let mut names = Vec::new();
    for ext in FORMATS {
        if let Some(bytes) = encoded.by_ext(ext) {
            let name = format!("good-{ext}.{ext}");
            std::fs::write(dir.join(&name), bytes).unwrap();
            names.push(name);
        }
    }
    names
}

fn as_strs(names: &[String]) -> Vec<&str> {
    names.iter().map(String::as_str).collect()
}

fn paths_of(dirs: &[&Path]) -> Vec<String> {
    dirs.iter().map(|d| d.to_string_lossy().into_owned()).collect()
}

struct Fixture {
    scratch: Scratch,
    encoded: Encoded,
}

fn fixture(name: &str) -> Fixture {
    let scratch = Scratch::new(name);
    let work = folder_of(&scratch, "_work");
    let encoded = Encoded::make(&work);
    Fixture { scratch, encoded }
}

fn is_root() -> bool {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() == 0 }
}

#[test]
fn zero_byte_files_of_every_format_are_skipped_and_neighbours_import() {
    let f = fixture("zero");
    let dir = folder_of(&f.scratch, "music");
    let good = good_files(&dir, &f.encoded);
    for ext in ["mp3", "flac", "wav", "aiff", "aif", "m4a", "aac"] {
        std::fs::write(dir.join(format!("zero.{ext}")), b"").unwrap();
    }
    let run = drop_and_analyse("zero-byte", paths_of(&[&dir]));
    run.assert_analysed(&as_strs(&good));
    for ext in ["mp3", "flac", "wav", "aiff", "aif", "m4a", "aac"] {
        let name = format!("zero.{ext}");
        assert!(!run.has(&name) || !run.row(&name).analysed(), "{name} must never count as analysed");
    }
    assert_eq!(run.finished.map(|(_, total)| total as usize), Some(run.rows.len()));
}

#[test]
fn truncated_and_corrupted_audio_never_blocks_the_good_files() {
    let f = fixture("truncated");
    let dir = folder_of(&f.scratch, "music");
    let good = good_files(&dir, &f.encoded);
    for ext in FORMATS {
        let Some(bytes) = f.encoded.by_ext(ext) else { continue };
        std::fs::write(dir.join(format!("header-only.{ext}")), &bytes[..bytes.len().min(30)]).unwrap();
        std::fs::write(dir.join(format!("cut40.{ext}")), cut(bytes, 0.4)).unwrap();
        std::fs::write(dir.join(format!("cut90.{ext}")), cut(bytes, 0.9)).unwrap();
        std::fs::write(dir.join(format!("middle.{ext}")), flip_bytes(bytes, 0.3, 0.6)).unwrap();
        std::fs::write(dir.join(format!("start.{ext}")), flip_bytes(bytes, 0.0, 0.15)).unwrap();
        std::fs::write(dir.join(format!("random.{ext}")), random_bytes(100_000)).unwrap();
    }
    let run = drop_and_analyse("truncated-corrupt", paths_of(&[&dir]));
    run.assert_analysed(&as_strs(&good));
    let (errors, total) = run.finished.expect("run finished");
    assert_eq!(total as usize, run.rows.len(), "every imported row was attempted");
    assert_eq!(errors as usize, run.rows.iter().filter(|r| !r.analysed()).count(), "every failure counted, none silent");
    for ext in FORMATS {
        let name = format!("random.{ext}");
        assert!(!run.has(&name) || !run.row(&name).analysed(), "{name}: random bytes must not analyse");
    }
}

#[test]
fn wrong_extension_and_unsupported_files() {
    let f = fixture("wrongext");
    let dir = folder_of(&f.scratch, "music");
    let good = good_files(&dir, &f.encoded);
    std::fs::write(dir.join("wav-as.mp3"), &f.encoded.wav).unwrap();
    std::fs::write(dir.join("wav-as.flac"), &f.encoded.wav).unwrap();
    std::fs::write(dir.join("wav-as.m4a"), &f.encoded.wav).unwrap();
    std::fs::write(dir.join("aiff-as.wav"), &f.encoded.aiff).unwrap();
    std::fs::write(dir.join("text-as.mp3"), b"words, not music\n".repeat(300)).unwrap();
    std::fs::write(dir.join("text-as.wav"), b"words, not music\n".repeat(300)).unwrap();
    std::fs::write(dir.join("fake.ogg"), [b"OggS".as_slice(), &random_bytes(4000)].concat()).unwrap();
    std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
    std::fs::write(dir.join("cover.jpg"), random_bytes(900)).unwrap();
    std::fs::write(dir.join("no-extension"), random_bytes(900)).unwrap();
    std::fs::write(dir.join("track.opus"), random_bytes(900)).unwrap();
    std::fs::write(dir.join("track.wma"), random_bytes(900)).unwrap();
    std::fs::write(dir.join("song.mp3.txt"), b"x").unwrap();
    std::fs::write(dir.join("loud.MP3"), f.encoded.mp3.as_deref().unwrap_or(&f.encoded.wav)).unwrap();
    std::fs::write(dir.join("loud.FLAC"), f.encoded.flac.as_deref().unwrap_or(&f.encoded.wav)).unwrap();
    let run = drop_and_analyse("wrong-extension", paths_of(&[&dir]));
    run.assert_analysed(&as_strs(&good));
    run.assert_analysed(&["loud.MP3"]);
    for name in ["notes.txt", "cover.jpg", "no-extension", "track.opus", "track.wma", "song.mp3.txt", "fake.ogg"] {
        assert!(!run.has(name), "{name} is not an audio extension and must stay out of the library");
    }
    assert!(run.plan.skipped.unsupported >= 7, "{:?}", run.plan.skipped);
    for name in ["text-as.mp3", "text-as.wav"] {
        assert!(!run.has(name) || !run.row(name).analysed());
    }
}

#[test]
fn odd_tags_import_safely_and_stay_bounded() {
    let f = fixture("tags");
    let dir = folder_of(&f.scratch, "music");
    let good = good_files(&dir, &f.encoded);
    let cases = tag_cases(&f.encoded);
    for case in &cases {
        std::fs::write(dir.join(format!("{}.{}", case.name, case.ext)), &case.bytes).unwrap();
    }
    let work = f.scratch.path().join("_work");
    for ext in ["flac", "m4a", "mp3"] {
        if let Some(bytes) = ffmpeg_tagged(&work, ext, &EMOJI_RTL_TITLE.repeat(40), &"x".repeat(5000)) {
            std::fs::write(dir.join(format!("ffmpeg-tagged.{ext}")), bytes).unwrap();
        }
    }
    let run = drop_and_analyse("odd-tags", paths_of(&[&dir]));
    run.assert_analysed(&as_strs(&good));
    let lib = run.library.lock().unwrap();
    let longest = lib
        .get_all_tracks()
        .unwrap()
        .iter()
        .flat_map(|t| [t.title.len(), t.artist.len(), t.album.len(), t.genre.len(), t.label.len(), t.remixer.len(), t.comment.len(), t.usb_path.len()])
        .max()
        .unwrap();
    assert!(longest <= 4096, "a tag of {longest} bytes reached the library");
    for track in lib.get_all_tracks().unwrap() {
        assert!(!track.title.contains('\0') && !track.artist.contains('\0') && !track.album.contains('\0'), "NUL in tags of {}", track.source_path.display());
        assert!(!track.title.is_empty(), "empty title for {}", track.source_path.display());
    }
}

#[test]
fn unicode_long_and_odd_paths_import_and_analyse() {
    let f = fixture("paths");
    let root = folder_of(&f.scratch, "music");
    let wav = &f.encoded.wav;
    let mut names: Vec<String> = Vec::new();
    let long_stem = "x".repeat(240);
    let odd = [
        "\u{1F3A7} emoji \u{1F525}.wav".to_string(),
        "\u{5E9}\u{5DC}\u{5D5}\u{5DD} rtl.wav".to_string(),
        "\u{645}\u{631}\u{62D}\u{628}\u{627}.wav".to_string(),
        "cafe\u{301} combining.wav".to_string(),
        "caf\u{E9} precomposed.wav".to_string(),
        "\u{65E5}\u{672C}\u{8A9E}\u{306E}\u{66F2}.wav".to_string(),
        "tab\there.wav".to_string(),
        "new\nline.wav".to_string(),
        "  leading and trailing  .wav".to_string(),
        "quote\"star*question?.wav".to_string(),
        "trailing dots....wav".to_string(),
        format!("{long_stem}.wav"),
        "100%.wav".to_string(),
        "semi;colon&amp;.wav".to_string(),
        "back\\slash.wav".to_string(),
        "\u{202E}rtl-override.wav".to_string(),
    ];
    for name in &odd {
        std::fs::write(root.join(name), wav).unwrap();
        names.push(name.clone());
    }
    let unicode_dir = root.join("\u{1F4C1} \u{5E9}\u{5DC}\u{5D5}\u{5DD} album");
    std::fs::create_dir_all(&unicode_dir).unwrap();
    std::fs::write(unicode_dir.join("inside.wav"), wav).unwrap();
    names.push("inside.wav".into());
    let mut deep = root.clone();
    while deep.as_os_str().len() < 3500 {
        deep = deep.join("d".repeat(100));
    }
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("deep.wav"), wav).unwrap();
    names.push("deep.wav".into());
    let raw_name = std::ffi::OsStr::from_bytes(b"latin1-\xe9\xff.wav");
    let has_raw = std::fs::write(root.join(raw_name), wav).is_ok();
    let run = drop_and_analyse("odd-paths", paths_of(&[&root]));
    run.assert_analysed(&as_strs(&names));
    if has_raw {
        for row in run.rows.iter().filter(|r| r.path.as_os_str().as_bytes().windows(2).any(|w| w == [0xe9, 0xff])) {
            panic!("a path that is not UTF-8 reached the library as {:?}", row.path);
        }
        assert!(run.plan.skipped.unreadable >= 1, "the non-UTF-8 name must be reported as skipped: {:?}", run.plan.skipped);
    }
    for row in &run.rows {
        assert_eq!(file_status(&row.path), FileStatus::Ok, "{:?} must stay reachable", row.path);
    }
}

#[test]
fn symlink_loops_and_dangling_links_end_the_walk() {
    let f = fixture("symlinks");
    let root = folder_of(&f.scratch, "music");
    let good = good_files(&root, &f.encoded);
    let a = folder_of(&f.scratch, "music/a");
    let b = folder_of(&f.scratch, "music/a/b");
    std::fs::write(a.join("in-a.wav"), &f.encoded.wav).unwrap();
    std::fs::write(b.join("in-b.wav"), &f.encoded.wav).unwrap();
    symlink("..", b.join("up")).unwrap();
    symlink("../..", b.join("up2")).unwrap();
    symlink(".", a.join("self")).unwrap();
    symlink(&root, a.join("to-root")).unwrap();
    symlink(&a, b.join("to-a")).unwrap();
    symlink("loop-b.wav", root.join("loop-a.wav")).unwrap();
    symlink("loop-a.wav", root.join("loop-b.wav")).unwrap();
    symlink("self.wav", root.join("self.wav")).unwrap();
    symlink("missing-target.wav", root.join("dangling.wav")).unwrap();
    symlink("missing-dir", root.join("dangling-dir")).unwrap();
    let mut previous = "good-wav.wav".to_string();
    for i in 0..45 {
        let name = format!("chain{i}.wav");
        symlink(&previous, root.join(&name)).unwrap();
        previous = name;
    }
    let outside = folder_of(&f.scratch, "outside");
    std::fs::write(outside.join("outside.wav"), &f.encoded.wav).unwrap();
    symlink(&outside, root.join("linked-outside")).unwrap();
    symlink(&outside, outside.join("outside-self")).unwrap();
    let run = drop_and_import("symlink-loops", paths_of(&[&root]));
    for name in good.iter().map(String::as_str).chain(["in-a.wav", "in-b.wav", "outside.wav"]) {
        assert!(run.has(name), "{name} must be imported: {:?}", run.names());
    }
    for name in ["loop-a.wav", "loop-b.wav", "self.wav", "dangling.wav"] {
        assert!(!run.has(name), "{name} points nowhere and must not be imported");
    }
    drop_and_import("symlink-dropped-loop",paths_of(&[&a.join("self"), &b.join("up")]));
}

#[test]
fn unreadable_files_and_folders_are_reported_not_fatal() {
    if is_root() {
        eprintln!("skipping: root ignores file permissions");
        return;
    }
    let f = fixture("perms");
    let root = folder_of(&f.scratch, "music");
    let good = good_files(&root, &f.encoded);
    let locked_file = root.join("locked.wav");
    std::fs::write(&locked_file, &f.encoded.wav).unwrap();
    std::fs::set_permissions(&locked_file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let locked_dir = folder_of(&f.scratch, "music/locked-folder");
    std::fs::write(locked_dir.join("hidden-inside.wav"), &f.encoded.wav).unwrap();
    std::fs::write(locked_dir.join("hidden-inside-2.wav"), &f.encoded.wav).unwrap();
    std::fs::set_permissions(&locked_dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    let no_list = folder_of(&f.scratch, "music/no-list-folder");
    std::fs::write(no_list.join("unlistable.wav"), &f.encoded.wav).unwrap();
    std::fs::set_permissions(&no_list, std::fs::Permissions::from_mode(0o111)).unwrap();
    let run = drop_and_analyse("unreadable", paths_of(&[&root]));
    run.assert_analysed(&as_strs(&good));
    assert!(!run.has("locked.wav"), "an unreadable file cannot be imported");
    assert!(run.plan.skipped.unreadable >= 2, "the locked file and the locked folders are reported as unreadable: {:?}", run.plan.skipped);
}

#[test]
fn read_only_folders_import_and_analyse_without_writing_into_them() {
    let f = fixture("readonly");
    let root = folder_of(&f.scratch, "music");
    let good = good_files(&root, &f.encoded);
    let sub = folder_of(&f.scratch, "music/album");
    std::fs::write(sub.join("track.wav"), &f.encoded.wav).unwrap();
    let before: HashSet<PathBuf> = walk(&root);
    for dir in [&sub, &root] {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_file() {
                std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o444)).unwrap();
            }
        }
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
    let run = drop_and_analyse("read-only", paths_of(&[&root]));
    run.assert_analysed(&as_strs(&good));
    run.assert_analysed(&["track.wav"]);
    assert_eq!(walk(&root), before, "nothing may be written into the source folders");
}

fn walk(root: &Path) -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            out.insert(entry.path());
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            }
        }
    }
    out
}

#[test]
fn files_deleted_between_scan_and_analysis_fail_alone() {
    let f = fixture("deleted");
    let root = folder_of(&f.scratch, "music");
    let good = good_files(&root, &f.encoded);
    for i in 0..4 {
        std::fs::write(root.join(format!("doomed{i}.wav")), &f.encoded.wav).unwrap();
    }
    let (root_s, doomed) = (paths_of(&[&root]), root.join("doomed2.wav"));
    let run = with_deadline("deleted-before-analysis", move || {
        let pipeline = Pipeline::new("deleted");
        let plan = pipeline.import(&root_s, &mut || {});
        for i in [0, 2] {
            std::fs::remove_file(root_s_path(&root_s).join(format!("doomed{i}.wav"))).unwrap();
        }
        std::fs::write(root_s_path(&root_s).join("doomed1.wav"), b"replaced by garbage").unwrap();
        let (finished, log, engine) = pipeline.analyse();
        let run = pipeline.snapshot(plan, finished, log, engine);
        run
    });
    run.report("deleted-before-analysis");
    run.assert_analysed(&as_strs(&good));
    run.assert_analysed(&["doomed3.wav"]);
    for name in ["doomed0.wav", "doomed1.wav", "doomed2.wav"] {
        assert!(!run.row(name).analysed(), "{name} cannot be analysed");
    }
    assert_eq!(run.finished.map(|(errors, _)| errors), Some(3), "{}", run.log.join("\n"));
    assert_eq!(file_status(&doomed), FileStatus::Missing);
    run.assert_library_healthy();
}

fn root_s_path(paths: &[String]) -> PathBuf {
    PathBuf::from(&paths[0])
}

#[test]
fn files_deleted_between_the_survey_and_the_scan_are_reported_unreadable() {
    let f = fixture("vanish");
    let root = folder_of(&f.scratch, "music");
    let good = good_files(&root, &f.encoded);
    for i in 0..30 {
        std::fs::write(root.join(format!("vanishing{i:02}.wav")), &f.encoded.wav).unwrap();
    }
    let paths = paths_of(&[&root]);
    let run = with_deadline("vanish-between-survey-and-scan", move || {
        let pipeline = Pipeline::new("vanish");
        let survey = survey_files(&paths);
        for i in 0..30 {
            std::fs::remove_file(root_s_path(&paths).join(format!("vanishing{i:02}.wav"))).unwrap();
        }
        let scanned = scan_paths(&survey, &HashSet::new(), &mut || {});
        let plan = plan_for_library(&pipeline.plans, &pipeline.library.lock().unwrap(), scanned, &survey).unwrap();
        let (tracks, _) = pipeline.plans.take(plan.token).unwrap().resolve(false);
        crate::add_scanned_tracks(&pipeline.library.lock().unwrap(), tracks).unwrap();
        let (finished, log, engine) = pipeline.analyse();
        pipeline.snapshot(plan, finished, log, engine)
    });
    run.report("vanish-between-survey-and-scan");
    run.assert_analysed(&as_strs(&good));
    assert_eq!(run.rows.len(), good.len(), "deleted files never reach the library");
    assert_eq!(run.plan.skipped.unreadable, 30);
    run.assert_library_healthy();
}

#[test]
fn duplicate_files_are_listed_and_both_copies_analyse() {
    let f = fixture("dupes");
    let root = folder_of(&f.scratch, "music");
    let other = folder_of(&f.scratch, "music/copy");
    let good = good_files(&root, &f.encoded);
    std::fs::write(root.join("same-bytes-1.wav"), &f.encoded.wav).unwrap();
    std::fs::write(root.join("same-bytes-2.wav"), &f.encoded.wav).unwrap();
    std::fs::write(other.join("good-wav.wav"), &f.encoded.wav).unwrap();
    std::fs::hard_link(root.join("good-wav.wav"), root.join("hardlink.wav")).unwrap();
    symlink(root.join("good-wav.wav"), root.join("symlink-to-file.wav")).unwrap();
    let loose = root.join("good-wav.wav").to_string_lossy().into_owned();
    let run = drop_and_analyse("duplicates", vec![root.to_string_lossy().into_owned(), loose.clone(), loose]);
    run.assert_analysed(&as_strs(&good));
    run.assert_analysed(&["same-bytes-1.wav", "same-bytes-2.wav", "hardlink.wav"]);
    let wav_rows = run.rows.iter().filter(|r| r.path == root.join("good-wav.wav")).count();
    assert_eq!(wav_rows, 1, "one file dropped three times is one row");

    let elsewhere = folder_of(&f.scratch, "elsewhere");
    std::fs::write(elsewhere.join("good-wav.wav"), &f.encoded.wav).unwrap();
    let flagged = with_deadline("duplicate-flagging", {
        let (first, second) = (paths_of(&[&root]), paths_of(&[&elsewhere]));
        move || {
            let pipeline = Pipeline::new("dupes-flag");
            pipeline.import(&first, &mut || {});
            pipeline.import(&second, &mut || {}).duplicates.len()
        }
    });
    assert_eq!(flagged, 1, "a same-name same-size copy of a library track is flagged");

    let again = with_deadline("reimport", {
        let paths = paths_of(&[&root]);
        move || {
            let pipeline = Pipeline::new("reimport");
            let first = pipeline.import(&paths, &mut || {});
            let second = pipeline.import(&paths, &mut || {});
            let count = pipeline.library.lock().unwrap().track_count().unwrap();
                (first.new_count, second.new_count, second.already_in_library, count)
        }
    });
    assert_eq!(again.1, 0, "dropping the same folder again adds nothing: {again:?}");
    assert!(again.2 >= again.0, "{again:?}");
}

#[test]
fn tiny_empty_and_missing_drops() {
    let f = fixture("tiny");
    let single = folder_of(&f.scratch, "single");
    std::fs::write(single.join("only.wav"), &f.encoded.wav).unwrap();
    let run = drop_and_analyse("one-file-folder", paths_of(&[&single]));
    run.assert_analysed(&["only.wav"]);
    assert_eq!(run.rows.len(), 1);

    let no_audio = folder_of(&f.scratch, "no-audio");
    std::fs::write(no_audio.join("readme.txt"), b"x").unwrap();
    std::fs::write(no_audio.join("cover.jpg"), random_bytes(100)).unwrap();
    let run = drop_and_analyse("zero-audio-folder", paths_of(&[&no_audio]));
    assert!(run.rows.is_empty());
    assert_eq!(run.plan.new_count, 0);
    assert_eq!(run.plan.skipped.unsupported, 2);
    assert_eq!(run.finished, None, "no analysis run for nothing");

    let empty = folder_of(&f.scratch, "empty");
    let run = drop_and_analyse("empty-folder", paths_of(&[&empty]));
    assert!(run.rows.is_empty());

    let missing = f.scratch.path().join("never-existed");
    let hidden_only = folder_of(&f.scratch, ".hidden-root");
    std::fs::write(hidden_only.join("a.wav"), &f.encoded.wav).unwrap();
    let run = drop_and_analyse("missing-and-hidden", paths_of(&[&missing, &hidden_only, &no_audio.join("readme.txt")]));
    assert!(run.rows.is_empty());

    let run = drop_and_analyse("nothing-dropped", Vec::new());
    assert!(run.rows.is_empty());
    assert_eq!(run.plan.new_count, 0);
}

#[test]
fn a_messy_library_imports_everything_good_and_survives_the_rest() {
    let f = fixture("everything");
    let root = folder_of(&f.scratch, "music");
    let good = good_files(&root, &f.encoded);
    let messy = folder_of(&f.scratch, "music/messy");
    for ext in FORMATS {
        let Some(bytes) = f.encoded.by_ext(ext) else { continue };
        std::fs::write(messy.join(format!("zero.{ext}")), b"").unwrap();
        std::fs::write(messy.join(format!("cut.{ext}")), cut(bytes, 0.5)).unwrap();
        std::fs::write(messy.join(format!("noise.{ext}")), random_bytes(50_000)).unwrap();
        std::fs::write(messy.join(format!("mangled.{ext}")), flip_bytes(bytes, 0.1, 0.9)).unwrap();
    }
    for case in tag_cases(&f.encoded) {
        std::fs::write(messy.join(format!("tag-{}.{}", case.name, case.ext)), &case.bytes).unwrap();
    }
    symlink("..", messy.join("loop")).unwrap();
    symlink("nowhere.wav", messy.join("dangling.wav")).unwrap();
    std::fs::write(messy.join("notes.txt"), b"x").unwrap();
    let run = drop_and_analyse("everything-at-once", paths_of(&[&root]));
    run.assert_analysed(&as_strs(&good));
    let (errors, total) = run.finished.expect("finished");
    assert_eq!(total as usize, run.rows.len());
    assert_eq!(errors as usize, run.rows.iter().filter(|r| !r.analysed()).count());
}

#[test]
fn huge_and_special_files_do_not_hang_the_import() {
    let f = fixture("special");
    let root = folder_of(&f.scratch, "music");
    let good = good_files(&root, &f.encoded);
    for ext in ["mp3", "flac", "wav", "m4a", "aiff"] {
        let file = std::fs::File::create(root.join(format!("sparse-zeros.{ext}"))).unwrap();
        file.set_len(256 * 1024 * 1024).unwrap();
    }
    let mut huge_junk = std::fs::File::create(root.join("sparse-junk-header.mp3")).unwrap();
    std::io::Write::write_all(&mut huge_junk, &f.encoded.mp3.clone().unwrap_or_else(synthetic_mp3)[..2000]).unwrap();
    huge_junk.set_len(256 * 1024 * 1024).unwrap();
    let fifo = root.join("pipe.mp3");
    let made = std::process::Command::new("mkfifo").arg(&fifo).status().map(|s| s.success()).unwrap_or(false);
    if made {
        symlink(&fifo, root.join("pipe-link.wav")).unwrap();
    }
    symlink("/dev/zero", root.join("zero-device.wav")).unwrap();
    symlink("/dev/null", root.join("null-device.mp3")).unwrap();
    let run = drop_and_analyse("huge-and-special", paths_of(&[&root]));
    run.assert_analysed(&as_strs(&good));
    for name in ["pipe.mp3", "pipe-link.wav", "zero-device.wav", "null-device.mp3"] {
        assert!(!run.has(name), "{name} is not a regular file");
    }
}

#[test]
fn a_panicking_tag_reader_costs_one_file_not_the_import() {
    let path = Path::new("/music/crashes-the-parser.mp3");
    assert!(guarded_read(path, || panic!("parser bug")).is_none());
    assert!(guarded_read(path, || Err(anyhow::anyhow!("bad tag"))).is_none());
}

#[test]
fn clamped_tags_keep_the_usb_path_in_step_with_the_artist() {
    let f = fixture("clamp");
    let dir = folder_of(&f.scratch, "music");
    let long_artist = "\u{1F3A7}".repeat(900);
    let (title, artist) = ("t".repeat(5000), long_artist.as_str());
    let Some(bytes) = ffmpeg_tagged(f.scratch.path().join("_work").as_path(), "flac", &title, artist) else {
        eprintln!("skipping: ffmpeg unavailable");
        return;
    };
    let file = dir.join("long-tags.flac");
    std::fs::write(&file, bytes).unwrap();
    let track = guarded_read(&file, || pioneer_usb_writer::scanner::read_track_metadata(&file, 0)).expect("readable");
    assert_eq!(track.title.chars().count(), 500);
    assert_eq!(track.artist.chars().count(), 500);
    let artist_dir = track.usb_path.split('/').nth(2).unwrap();
    assert_eq!(artist_dir.chars().count(), 500);
    assert!(track.usb_path.ends_with("/long-tags.flac"));
}

#[test]
fn folders_that_link_to_each_other_do_not_multiply_the_walk() {
    let f = fixture("clique");
    let root = folder_of(&f.scratch, "music");
    let dirs: Vec<PathBuf> = (0..8).map(|i| folder_of(&f.scratch, &format!("music/d{i}"))).collect();
    for (i, dir) in dirs.iter().enumerate() {
        std::fs::write(dir.join(format!("only-{i}.wav")), &f.encoded.wav).unwrap();
        for (j, other) in dirs.iter().enumerate() {
            if i != j {
                symlink(other, dir.join(format!("see-also-{j}"))).unwrap();
            }
        }
    }
    let survey_started = std::time::Instant::now();
    let survey = survey_files(&paths_of(&[&root]));
    let found = survey.audio_count();
    eprintln!("CLIQUE survey found {found} paths in {:.1}s", survey_started.elapsed().as_secs_f64());
    assert!(found <= 8 * 3, "{found} paths for 8 files: the walk follows every ordering of the links");
}

#[test]
fn tracks_removed_while_queued_do_not_stop_the_run() {
    let f = fixture("removed");
    let root = folder_of(&f.scratch, "music");
    for i in 0..6 {
        std::fs::write(root.join(format!("t{i}.wav")), &f.encoded.wav).unwrap();
    }
    let paths = paths_of(&[&root]);
    let run = with_deadline("removed-while-queued", move || {
        let pipeline = Pipeline::new("removed");
        let plan = pipeline.import(&paths, &mut || {});
        let work = crate::analysis_run::resume_work(&pipeline.library.lock().unwrap(), true).unwrap();
        assert_eq!(work.len(), 6);
        pipeline.library.lock().unwrap().remove_tracks(&[work[1].0, work[4].0]).unwrap();
        let (runner, engine_name) = engine();
        let sink = Arc::new(Sink { lines: Mutex::new(Vec::new()) });
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let outcome = runtime.block_on(run_with_runner(pipeline.library.clone(), AnalysisCoordinator::default(), work, sink.clone(), BackendEnv::default(), Some(Runner::Rust { engine: runner })));
        let finished = match outcome {
            Ok(RunOutcome::Finished { errors, total }) => Some((errors, total)),
            Ok(_) => None,
            Err(e) => panic!("the run ended with an error: {e}\n{}", sink.lines.lock().unwrap().join("\n")),
        };
        let lines = sink.lines.lock().unwrap().clone();
        pipeline.snapshot(plan, finished, lines, engine_name)
    });
    run.report("removed-while-queued");
    assert_eq!(run.rows.len(), 4);
    run.assert_analysed(&run.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>());
    run.assert_library_healthy();
}

fn panic_text(panic: Box<dyn std::any::Any + Send>) -> String {
    panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

#[test]
fn the_waveform_decoders_survive_damaged_audio() {
    use crate::band_analysis::{measure_detail, measure_detail_window, measure_waveforms, measure_waveforms_and_detail};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let f = fixture("wavedecode");
    let dir = folder_of(&f.scratch, "damaged");
    let files = damaged_files(&dir, &f.encoded);
    let mut failures = Vec::new();
    for file in &files {
        let started = std::time::Instant::now();
        let results = [
            ("detail", catch_unwind(AssertUnwindSafe(|| measure_detail(&file.path).map(|_| ())))),
            ("waveforms", catch_unwind(AssertUnwindSafe(|| measure_waveforms(&file.path).map(|_| ())))),
            ("both", catch_unwind(AssertUnwindSafe(|| measure_waveforms_and_detail(&file.path).map(|_| ())))),
            ("window", catch_unwind(AssertUnwindSafe(|| measure_detail_window(&file.path, [256; 3], 0.0, 5.0, 800, &|| true).map(|_| ())))),
        ];
        for (what, result) in results {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("WAVE {what:<9} {:<34} err {e}", file.label),
                Err(panic) => failures.push(format!("{what} PANIC on {}: {}", file.label, panic_text(panic))),
            }
        }
        if started.elapsed() > Duration::from_secs(30) {
            failures.push(format!("{} took {:.0}s", file.label, started.elapsed().as_secs_f64()));
        }
    }
    assert!(failures.is_empty(), "{} problems in {} files:\n{}", failures.len(), files.len(), failures.join("\n"));
}

#[test]
fn damaged_cover_images_come_back_unchanged_instead_of_failing() {
    let mut png = Vec::new();
    image::RgbImage::from_fn(900, 900, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, 7]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let mut huge_header = png.clone();
    huge_header[16..24].copy_from_slice(&[0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff]);
    for (label, bytes) in [
        ("garbage", random_bytes(5000)),
        ("empty", Vec::new()),
        ("png cut in half", cut(&png, 0.5)),
        ("png with 65535 x 65535 header", huge_header),
        ("png header only", png[..30].to_vec()),
        ("png with a mangled middle", flip_bytes(&png, 0.3, 0.6)),
    ] {
        let thumb = std::panic::catch_unwind(|| crate::artwork_thumbnail(&bytes, 240)).unwrap_or_else(|_| panic!("{label}: the thumbnail code panicked"));
        assert!(!thumb.is_empty() || bytes.is_empty(), "{label}");
    }
}
