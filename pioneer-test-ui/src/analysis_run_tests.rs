use super::*;
use crate::analysis_backend::PartialResultSink;
use fourfour_analysis::service::{Pass, PoolPlan};
use pioneer_usb_writer::models::{AnalysisResult, BeatGrid, Track, WaveformPreview};

#[derive(Debug, Clone, PartialEq)]
enum Event {
    Start(String, usize),
    Ran(String, Pass),
    Paused(String),
    Resumed(String),
    Cancelled(String),
    End(String),
    PartialStored(i64, f64),
    Progress(i64),
    Complete,
}

type EventLog = Arc<Mutex<Vec<Event>>>;

const CHUNKS: usize = 6;
const CHUNK: Duration = Duration::from_millis(40);
const FAST: Duration = Duration::from_millis(10);

struct FakeEngine {
    log: EventLog,
    workers: usize,
    max_threads: usize,
    hold_grid: AtomicBool,
    failing: Mutex<Vec<String>>,
}

fn finished_result() -> AnalysisResult {
    AnalysisResult {
        beat_grid: BeatGrid { beats: Vec::new() },
        waveform: WaveformPreview { data: [7; 400] },
        bpm: 128.0,
        key: "8A".to_string(),
        energy: None,
        cue_points: Vec::new(),
        color_waveform: None,
    }
}

impl TrackEngine for FakeEngine {
    fn plan(&self, jobs: usize) -> PoolPlan {
        let workers = self.workers.min(jobs.max(1));
        PoolPlan { workers, threads_per_track: self.max_threads / workers }
    }

    fn analyze(&self, path: &Path, control: &Control, threads: usize, pass: Pass, on_partial: PartialResultSink) -> Result<AnalysisResult, String> {
        let name = file_name_of(path);
        self.log.lock().unwrap().push(Event::Start(name.clone(), threads));
        self.log.lock().unwrap().push(Event::Ran(name.clone(), pass));
        if self.failing.lock().unwrap().contains(&name) {
            return Err("failed to decode any audio packets".to_string());
        }
        if let Some(emit) = on_partial {
            emit(AnalysisResult { bpm: 0.0, ..finished_result() });
        }
        if pass == Pass::Fast {
            std::thread::sleep(FAST);
            self.log.lock().unwrap().push(Event::End(name));
            return Ok(AnalysisResult { bpm: 0.0, ..finished_result() });
        }
        for _ in 0..CHUNKS {
            let mut paused = false;
            while control.pause.load(Ordering::Relaxed) && !control.cancel.load(Ordering::Relaxed) {
                if !paused {
                    self.log.lock().unwrap().push(Event::Paused(name.clone()));
                    paused = true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            if paused {
                self.log.lock().unwrap().push(Event::Resumed(name.clone()));
            }
            if control.cancel.load(Ordering::Relaxed) {
                self.log.lock().unwrap().push(Event::Cancelled(name));
                return Err("analysis cancelled".to_string());
            }
            while pass == Pass::Grid && self.hold_grid.load(Ordering::Relaxed) && !control.pause.load(Ordering::Relaxed) && !control.cancel.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
            }
            std::thread::sleep(CHUNK);
        }
        self.log.lock().unwrap().push(Event::End(name));
        let key = if pass == Pass::Grid { String::new() } else { "8A".to_string() };
        Ok(AnalysisResult { key, ..finished_result() })
    }
}

struct HeldGridPasses<'a> {
    engine: &'a FakeEngine,
}

impl Drop for HeldGridPasses<'_> {
    fn drop(&mut self) {
        self.engine.hold_grid.store(false, Ordering::Relaxed);
    }
}

const TRIGGER_TIMEOUT: Duration = Duration::from_secs(30);

fn grid_passes(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Ran(name, Pass::Grid) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn track_number(name: &str) -> i64 {
    name.trim_start_matches('t').trim_end_matches(".wav").parse().expect("harness track name")
}

type SinkHook = Box<dyn Fn(i64) + Send + Sync>;

struct TestSink {
    log: EventLog,
    shared: SharedLibrary,
    on_partial: Mutex<Option<SinkHook>>,
    on_complete: Mutex<Option<SinkHook>>,
}

impl AnalysisSink for TestSink {
    fn log(&self, _level: LogLevel, _message: String) {}

    fn progress(&self, _current: u32, _total: u32, _message: String, track_id: i64) {
        self.log.lock().unwrap().push(Event::Progress(track_id));
    }

    fn partial(&self, track_id: i64) {
        let bpm = self.shared.lock().unwrap().get_analysis(track_id).unwrap().map_or(-1.0, |a| a.bpm);
        self.log.lock().unwrap().push(Event::PartialStored(track_id, bpm));
        if let Some(hook) = self.on_partial.lock().unwrap().as_ref() {
            hook(track_id);
        }
    }

    fn complete(&self) {
        self.log.lock().unwrap().push(Event::Complete);
        if let Some(hook) = self.on_complete.lock().unwrap().as_ref() {
            hook(0);
        }
    }
}

impl TestSink {
    fn log_of_completions(&self) -> usize {
        self.log.lock().unwrap().iter().filter(|e| **e == Event::Complete).count()
    }
}

struct Harness {
    shared: SharedLibrary,
    coordinator: AnalysisCoordinator,
    engine: Arc<FakeEngine>,
    sink: Arc<TestSink>,
    items: Vec<AnalysisWorkItem>,
}

fn track(index: usize) -> Track {
    Track {
        id: 0,
        source_path: PathBuf::from(format!("/music/t{index}.wav")),
        usb_path: format!("/Contents/t{index}.wav"),
        title: format!("t{index}"),
        artist: String::new(),
        album: String::new(),
        genre: String::new(),
        label: String::new(),
        remixer: String::new(),
        comment: String::new(),
        year: 2024,
        disc_number: 1,
        track_number: 1,
        tempo: 0,
        key: String::new(),
        duration_secs: 300.0,
        sample_rate: 44100,
        bitrate: 320,
        file_size: 1,
        artwork: None,
    }
}

fn harness(tracks: usize, workers: usize, max_threads: usize) -> Harness {
    let library = LocalLibrary::open_in_memory().unwrap();
    let items = (1..=tracks)
        .map(|n| {
            let t = track(n);
            (library.add_track(&t).unwrap(), t.source_path)
        })
        .collect();
    let shared: SharedLibrary = Arc::new(Mutex::new(library));
    let log: EventLog = Arc::new(Mutex::new(Vec::new()));
    Harness {
        shared: shared.clone(),
        coordinator: AnalysisCoordinator::default(),
        engine: Arc::new(FakeEngine { log: log.clone(), workers, max_threads, hold_grid: AtomicBool::new(false), failing: Mutex::new(Vec::new()) }),
        sink: Arc::new(TestSink { log, shared, on_partial: Mutex::new(None), on_complete: Mutex::new(None) }),
        items,
    }
}

/// A harness whose runs take every fast pass before the model starts, for the now-lane scenarios built on
/// grid passes.
fn fast_first_harness(tracks: usize, workers: usize, max_threads: usize) -> Harness {
    let mut h = harness(tracks, workers, max_threads);
    h.coordinator = AnalysisCoordinator::default().with_whole_jobs(0);
    h
}

impl Harness {
    fn runner(&self) -> Runner {
        Runner { engine: self.engine.clone() }
    }

    fn events(&self) -> Vec<Event> {
        self.engine.log.lock().unwrap().clone()
    }

    fn position(&self, wanted: &Event) -> usize {
        self.events().iter().position(|e| e == wanted).unwrap_or_else(|| panic!("{wanted:?} missing in {:?}", self.events()))
    }

    fn count(&self, wanted: &Event) -> usize {
        self.events().iter().filter(|e| *e == wanted).count()
    }

    fn unanalyzed(&self) -> Vec<i64> {
        self.shared.lock().unwrap().get_unanalyzed_track_ids().unwrap()
    }

    fn hold_grid_passes(&self) -> HeldGridPasses<'_> {
        self.engine.hold_grid.store(true, Ordering::Relaxed);
        HeldGridPasses { engine: &self.engine }
    }

    async fn wait_for<T>(&self, found: impl Fn(&[Event]) -> Option<T>) -> T {
        tokio::time::timeout(TRIGGER_TIMEOUT, async {
            loop {
                if let Some(value) = found(&self.events()) {
                    return value;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("trigger never happened in {:?}", self.events()))
    }

    async fn run_and_prioritize_when(
        &self,
        queued: Vec<AnalysisWorkItem>,
        pick: impl Fn(&[Event]) -> Option<i64>,
    ) -> (RunOutcome, String) {
        let run = run_with_runner(self.shared.clone(), self.coordinator.clone(), queued, self.sink.clone(), BackendEnv::default(), Some(self.runner()));
        let prioritize = async {
            let held = self.engine.hold_grid.load(Ordering::Relaxed).then(|| self.hold_grid_passes());
            let track_id = self.wait_for(&pick).await;
            let priority = self.items.iter().find(|(id, _)| *id == track_id).expect("picked a harness track").clone();
            prioritize_with_runner(self.shared.clone(), self.coordinator.clone(), priority, self.sink.clone(), BackendEnv::default(), self.runner())
                .await
                .unwrap();
            drop(held);
            format!("t{track_id}.wav")
        };
        let (outcome, priority) = tokio::join!(run, prioritize);
        (outcome.unwrap(), priority)
    }

    async fn run_all(&self, queued: Vec<AnalysisWorkItem>) -> RunOutcome {
        run_with_runner(self.shared.clone(), self.coordinator.clone(), queued, self.sink.clone(), BackendEnv::default(), Some(self.runner()))
            .await
            .unwrap()
    }
}

fn ran(name: &str, pass: Pass) -> Event {
    Event::Ran(name.to_string(), pass)
}

fn has(events: &[Event], wanted: &Event) -> bool {
    events.contains(wanted)
}

fn start(name: &str, threads: usize) -> Event {
    Event::Start(name.to_string(), threads)
}

fn named(kind: fn(String) -> Event, name: &str) -> Event {
    kind(name.to_string())
}

#[tokio::test]
async fn re_analysis_keeps_hand_edited_tags() {
    let h = harness(1, 1, 8);
    {
        let lib = h.shared.lock().unwrap();
        let mut edited = lib.get_track(1).unwrap().unwrap();
        edited.title = "Hand Title".into();
        edited.genre = "Hand Genre".into();
        lib.update_track(1, &edited).unwrap();
    }
    h.run_all(h.items.clone()).await;
    h.run_all(h.items.clone()).await;

    let stored = h.shared.lock().unwrap().get_track(1).unwrap().unwrap();
    assert_eq!((stored.title.as_str(), stored.genre.as_str()), ("Hand Title", "Hand Genre"));
    assert_eq!((stored.tempo, stored.key.as_str()), (12800, "8A"));
}

#[tokio::test]
async fn the_fast_pass_of_every_track_comes_before_any_grid_pass_and_stores_waveform_and_key() {
    let h = fast_first_harness(5, 2, 8);
    let outcome = h.run_all(h.items.clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 5 }));
    let events = h.events();
    let first_grid = events.iter().position(|e| matches!(e, Event::Ran(_, Pass::Grid))).unwrap();
    let fast_ended_before = events[..first_grid].iter().filter(|e| matches!(e, Event::End(_))).count();
    assert!(fast_ended_before >= 5 - 1, "a grid pass may only share the two workers with the last fast pass: {events:?}");
    for n in 1..=5 {
        let name = format!("t{n}.wav");
        assert!(h.count(&Event::PartialStored(n, 0.0)) >= 1);
        assert_eq!(h.count(&ran(&name, Pass::Fast)), 1, "{events:?}");
        assert_eq!(h.count(&ran(&name, Pass::Grid)), 1);
        let stored = h.shared.lock().unwrap().get_analysis(n).unwrap().unwrap();
        assert_eq!((stored.bpm, stored.key.as_str()), (128.0, "8A"), "grid result keeps the key the fast pass found");
        assert_eq!(h.shared.lock().unwrap().analysis_stages(n).unwrap(), Some(analysis_stage::ALL));
    }
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn the_model_starts_on_the_first_track_while_the_others_take_their_fast_pass() {
    let h = harness(5, 2, 8);
    let outcome = h.run_all(h.items.clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 5 }));
    let events = h.events();
    assert_eq!(h.count(&ran("t1.wav", Pass::All)), 1, "{events:?}");
    let t1_end = events.iter().position(|e| *e == named(Event::End, "t1.wav")).unwrap();
    assert!(events[..t1_end].contains(&Event::PartialStored(1, 0.0)), "the whole pass stores the waveform first: {events:?}");
    let first_grid = events.iter().position(|e| matches!(e, Event::Ran(_, Pass::Grid))).unwrap();
    let last_fast = events.iter().rposition(|e| matches!(e, Event::Ran(_, Pass::Fast))).unwrap();
    assert!(last_fast < first_grid, "grid passes wait until every track had its fast or whole pass: {events:?}");
    for n in 1..=5 {
        let name = format!("t{n}.wav");
        let (fast, grid, all) = (h.count(&ran(&name, Pass::Fast)), h.count(&ran(&name, Pass::Grid)), h.count(&ran(&name, Pass::All)));
        assert!((fast, grid, all) == (1, 1, 0) || (fast, grid, all) == (0, 0, 1), "{name}: {events:?}");
        assert_eq!(h.shared.lock().unwrap().analysis_stages(n).unwrap(), Some(analysis_stage::ALL));
        let stored = h.shared.lock().unwrap().get_analysis(n).unwrap().unwrap();
        assert_eq!((stored.bpm, stored.key.as_str()), (128.0, "8A"));
    }
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn between_the_passes_a_track_is_waveform_and_key_only() {
    let h = fast_first_harness(3, 1, 8);
    let run = h.run_all(h.items.clone());
    let watch = async {
        let _held = h.hold_grid_passes();
        h.wait_for(|e| has(e, &ran("t1.wav", Pass::Grid)).then_some(())).await;
        let lib = h.shared.lock().unwrap();
        assert_eq!(lib.analysis_stages(3).unwrap(), Some(analysis_stage::WAVEFORM | analysis_stage::KEY));
        assert_eq!(lib.get_unanalyzed_track_ids().unwrap(), vec![1, 2, 3]);
        assert_eq!(lib.get_track(3).unwrap().unwrap().key, "8A");
    };
    tokio::join!(run, watch);
}

#[tokio::test]
async fn a_track_with_waveform_and_key_stored_goes_straight_to_the_grid_pass() {
    let h = harness(2, 1, 8);
    h.shared.lock().unwrap().set_analysis_stages(1, &finished_result(), analysis_stage::WAVEFORM | analysis_stage::KEY).unwrap();
    h.run_all(h.items.clone()).await;

    assert_eq!(h.count(&ran("t1.wav", Pass::Fast)), 0);
    assert_eq!(h.count(&ran("t1.wav", Pass::Grid)), 1);
    assert_eq!(h.count(&ran("t2.wav", Pass::All)), 1, "the model is free again, so the new track runs whole: {:?}", h.events());
    assert_eq!(h.count(&ran("t2.wav", Pass::Fast)), 0);
}

#[tokio::test]
async fn with_staging_off_every_track_is_analysed_in_one_piece() {
    let mut h = harness(3, 2, 8);
    h.coordinator = AnalysisCoordinator::default().with_staging(false);
    h.run_all(h.items.clone()).await;

    assert!(h.events().iter().all(|e| !matches!(e, Event::Ran(_, Pass::Fast | Pass::Grid))));
    assert_eq!(h.events().iter().filter(|e| matches!(e, Event::Ran(_, Pass::All))).count(), 3);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn a_queued_track_jumps_to_the_now_lane_and_running_grid_passes_yield() {
    let h = fast_first_harness(6, 2, 8);
    h.engine.hold_grid.store(true, Ordering::Relaxed);
    let (outcome, priority) = h
        .run_and_prioritize_when(h.items.clone(), |e| {
            let running = grid_passes(e);
            (running.len() == 2).then(|| (1..=6).find(|n| !running.contains(&format!("t{n}.wav"))).unwrap())
        })
        .await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 6 }));
    assert_eq!(h.count(&start(&priority, 8)), 1, "{:?}", h.events());
    assert_eq!(h.count(&ran(&priority, Pass::All)), 1);
    assert_eq!(h.count(&ran(&priority, Pass::Grid)), 0, "its queued grid pass was replaced");
    let end_priority = h.events().iter().rposition(|e| *e == named(Event::End, &priority)).unwrap();
    let grids = grid_passes(&h.events());
    assert_eq!(grids.len(), 5, "{:?}", h.events());
    for yielded in &grids[..2] {
        assert!(h.position(&named(Event::Paused, yielded)) < end_priority);
        assert!(h.position(&named(Event::Resumed, yielded)) > end_priority);
    }
    for queued in &grids[2..] {
        assert!(h.position(&ran(queued, Pass::Grid)) > end_priority, "queued grid passes wait for the now lane");
    }
    assert!(h.unanalyzed().is_empty());
    assert_eq!(h.count(&Event::Complete), 1);
    assert_eq!(h.events().last(), Some(&Event::Complete));
}

#[tokio::test]
async fn the_now_lane_stores_the_waveform_before_the_full_result() {
    let h = fast_first_harness(3, 1, 8);
    h.engine.hold_grid.store(true, Ordering::Relaxed);
    h.run_and_prioritize_when(h.items.clone(), |e| has(e, &ran("t1.wav", Pass::Grid)).then_some(3)).await;

    let events = h.events();
    let all_start = events.iter().position(|e| *e == ran("t3.wav", Pass::All)).unwrap();
    let end = events.iter().rposition(|e| *e == named(Event::End, "t3.wav")).unwrap();
    assert!(events[all_start..end].contains(&Event::PartialStored(3, 0.0)), "{events:?}");
    assert_eq!(h.shared.lock().unwrap().get_analysis(3).unwrap().unwrap().bpm, 128.0);
}

#[tokio::test]
async fn a_narrow_grid_job_is_preempted_and_restarted_wide() {
    let h = fast_first_harness(6, 2, 8);
    h.engine.hold_grid.store(true, Ordering::Relaxed);
    let (outcome, priority) = h
        .run_and_prioritize_when(h.items.clone(), |e| grid_passes(e).first().map(|name| track_number(name)))
        .await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 6 }));
    assert_eq!(h.count(&named(Event::Cancelled, &priority)), 1, "{:?}", h.events());
    assert_eq!(h.count(&ran(&priority, Pass::All)), 1);
    assert_eq!(h.count(&Event::Progress(track_number(&priority))), 1);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn a_track_missing_from_the_queue_joins_the_run_and_its_total() {
    let h = harness(4, 2, 8);
    let (outcome, _) = h.run_and_prioritize_when(h.items[..3].to_vec(), |e| (!e.is_empty()).then_some(4)).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    assert_eq!(h.count(&start("t4.wav", 8)), 1);
    assert_eq!(h.count(&Event::Progress(4)), 1);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn a_job_that_already_has_every_thread_is_left_alone() {
    let h = fast_first_harness(2, 1, 8);
    h.engine.hold_grid.store(true, Ordering::Relaxed);
    let (outcome, _) = h
        .run_and_prioritize_when(h.items[..1].to_vec(), |e| has(e, &ran("t1.wav", Pass::Grid)).then_some(1))
        .await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 1 }));
    assert_eq!(h.count(&ran("t1.wav", Pass::Grid)), 1);
    assert_eq!(h.count(&named(Event::Cancelled, "t1.wav")), 0);
    assert_eq!(h.count(&ran("t1.wav", Pass::All)), 0);
}

#[tokio::test]
async fn prioritizing_while_idle_runs_the_track_alone_with_every_thread() {
    let h = harness(2, 4, 8);
    prioritize_with_runner(h.shared.clone(), h.coordinator.clone(), h.items[1].clone(), h.sink.clone(), BackendEnv::default(), h.runner())
        .await
        .unwrap();

    assert_eq!(h.count(&start("t2.wav", 8)), 1, "a lone track runs whole with every thread: {:?}", h.events());
    assert_eq!(h.count(&ran("t2.wav", Pass::All)), 1);
    assert!(h.position(&Event::PartialStored(2, 0.0)) < h.position(&named(Event::End, "t2.wav")));
    assert_eq!(h.count(&Event::Complete), 1);
    assert_eq!(h.unanalyzed(), vec![1]);
    assert!(!h.coordinator.is_active());
}

#[test]
fn now_claims_need_an_active_run() {
    let coordinator = AnalysisCoordinator::default();
    assert!(matches!(coordinator.claim_now(1, 8), NowClaim::NoRun));
}

#[test]
fn background_launches_wait_while_the_now_lane_is_busy() {
    let coordinator = AnalysisCoordinator::default();
    assert!(coordinator.try_begin());
    let queue: SharedAnalysisQueue = Arc::new(Mutex::new(VecDeque::from([(1, PathBuf::from("/a.wav")), (2, PathBuf::from("/b.wav"))])));
    coordinator.register_run(queue, 2);

    assert!(matches!(coordinator.claim_now(2, 8), NowClaim::Granted { .. }));
    assert!(coordinator.background_paused());
    assert_eq!(coordinator.queue_len(), 1);
    assert!(matches!(coordinator.claim_now(2, 8), NowClaim::AlreadyRunning));
    coordinator.release_now(2);
    assert!(!coordinator.background_paused());
}

fn active_coordinator() -> AnalysisCoordinator {
    let coordinator = AnalysisCoordinator::default();
    assert!(coordinator.try_begin());
    let queue: SharedAnalysisQueue = Arc::new(Mutex::new(VecDeque::from([(1, PathBuf::from("/a.wav")), (2, PathBuf::from("/b.wav"))])));
    coordinator.register_run(queue, 2);
    coordinator
}

#[test]
fn the_ui_holds_background_jobs_until_it_was_idle_for_a_moment() {
    let coordinator = active_coordinator();
    let started = Instant::now();
    coordinator.ui_busy();
    assert!(coordinator.background_paused());
    std::thread::sleep(UI_IDLE_BEFORE_RESUME / 2);
    coordinator.ui_busy();
    std::thread::sleep(UI_IDLE_BEFORE_RESUME * 3 / 4);
    assert!(coordinator.background_paused(), "the second signal moved the resume out");
    while coordinator.background_paused() {
        assert!(started.elapsed() < UI_IDLE_BEFORE_RESUME * 4, "never resumed");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(started.elapsed() >= UI_IDLE_BEFORE_RESUME * 3 / 2);
    let (pauses, held) = coordinator.ui_pause_totals();
    assert_eq!(pauses, 1);
    assert!(held >= UI_IDLE_BEFORE_RESUME * 3 / 2, "{held:?}");
}

#[test]
fn the_ui_and_the_now_lane_hold_background_jobs_independently() {
    let coordinator = active_coordinator();
    assert!(matches!(coordinator.claim_now(2, 8), NowClaim::Granted { .. }));
    coordinator.ui_busy();
    std::thread::sleep(UI_IDLE_BEFORE_RESUME * 2);
    assert!(coordinator.background_paused(), "the now lane still runs");
    coordinator.ui_busy();
    coordinator.release_now(2);
    assert!(coordinator.background_paused(), "the UI is still busy");
    std::thread::sleep(UI_IDLE_BEFORE_RESUME * 2);
    assert!(!coordinator.background_paused());
}

#[test]
fn ui_signals_outside_a_run_hold_nothing() {
    let coordinator = AnalysisCoordinator::default();
    coordinator.ui_busy();
    assert!(!coordinator.background_paused());
}

#[tokio::test]
async fn model_jobs_wait_while_the_ui_is_busy_and_the_run_still_finishes() {
    let h = harness(3, 2, 8);
    let run = h.run_all(h.items.clone());
    let busy = async {
        h.wait_for(|e| e.iter().any(|ev| matches!(ev, Event::Ran(_, Pass::All) | Event::Ran(_, Pass::Grid))).then_some(())).await;
        for _ in 0..20 {
            h.coordinator.ui_busy();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let (outcome, ()) = tokio::join!(run, busy);
    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 3 }));
    assert!(h.events().iter().any(|e| matches!(e, Event::Paused(_))), "{:?}", h.events());
    assert!(h.unanalyzed().is_empty());
    assert_eq!(h.coordinator.ui_pause_totals().0, 1);
}

#[test]
fn cancel_reaches_every_job() {
    let coordinator = AnalysisCoordinator::default();
    assert!(coordinator.try_begin());
    let queue: SharedAnalysisQueue = Arc::new(Mutex::new(VecDeque::from([(1, PathBuf::from("/a.wav")), (2, PathBuf::from("/b.wav"))])));
    coordinator.register_run(queue, 2);
    let background = coordinator.take_next(true, |_| 0, ModelJobs { max: usize::MAX, whole: 0 }, |_| 1).unwrap();
    let NowClaim::Granted { cancel: now } = coordinator.claim_now(2, 8) else { panic!("claim refused") };

    coordinator.cancel();

    assert!(background.cancel.load(Ordering::Relaxed) && now.load(Ordering::Relaxed));
    assert_eq!(coordinator.queue_len(), 0);
}

fn resume_ids(work: &[AnalysisWorkItem]) -> Vec<i64> {
    work.iter().map(|(id, _)| *id).collect()
}

#[test]
fn resume_work_lists_partial_tracks_first_and_new_ones_only_when_the_setting_is_on() {
    let h = harness(4, 1, 8);
    let lib = h.shared.lock().unwrap();
    lib.set_analysis(3, &finished_result()).unwrap();
    lib.set_analysis_stages(2, &finished_result(), analysis_stage::WAVEFORM).unwrap();
    lib.set_analysis_stages(4, &finished_result(), analysis_stage::WAVEFORM | analysis_stage::KEY).unwrap();

    assert_eq!(resume_ids(&resume_work(&lib, true).unwrap()), vec![2, 4, 1]);
    assert_eq!(resume_ids(&resume_work(&lib, false).unwrap()), vec![2, 4]);
}

#[test]
fn resume_work_is_empty_when_everything_is_analysed() {
    let h = harness(2, 1, 8);
    let lib = h.shared.lock().unwrap();
    lib.set_analysis(1, &finished_result()).unwrap();
    lib.set_analysis(2, &finished_result()).unwrap();
    assert!(resume_work(&lib, true).unwrap().is_empty());
}

#[tokio::test]
async fn a_resumed_run_finishes_missing_stages_only() {
    let h = harness(3, 2, 8);
    h.shared
        .lock()
        .unwrap()
        .set_analysis_stages(1, &finished_result(), analysis_stage::WAVEFORM | analysis_stage::KEY)
        .unwrap();
    let work = resume_work(&h.shared.lock().unwrap(), true).unwrap();
    let outcome = h.run_all(work).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 3 }));
    assert_eq!(h.count(&ran("t1.wav", Pass::Fast)), 0);
    assert_eq!(h.count(&ran("t1.wav", Pass::Grid)), 1);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn resuming_while_a_run_is_active_queues_no_track_twice() {
    let h = harness(4, 2, 8);
    let run = h.run_all(h.items.clone());
    let resume = async {
        h.wait_for(|e| (!e.is_empty()).then_some(())).await;
        let work = resume_work(&h.shared.lock().unwrap(), true).unwrap();
        assert_eq!(work.len(), 4);
        merge_into_analysis_queue(&h.coordinator, work, false)
    };
    let (outcome, added) = tokio::join!(run, resume);

    assert_eq!(added, 0);
    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    for n in 1..=4 {
        let name = format!("t{n}.wav");
        let passes = (h.count(&ran(&name, Pass::Fast)), h.count(&ran(&name, Pass::Grid)), h.count(&ran(&name, Pass::All)));
        assert!(passes == (1, 1, 0) || passes == (0, 0, 1), "{name}: {:?}", h.events());
    }
}

#[tokio::test]
async fn resumed_work_yields_to_a_played_track() {
    let h = harness(4, 1, 8);
    let work = resume_work(&h.shared.lock().unwrap(), true).unwrap();
    let played = work[3].0;
    let (outcome, _) = h.run_and_prioritize_when(work[..3].to_vec(), |e| (!e.is_empty()).then_some(played)).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    assert_eq!(h.count(&ran("t4.wav", Pass::All)), 1, "{:?}", h.events());
    assert!(h.unanalyzed().is_empty());
}

fn name_of(id: i64) -> String {
    format!("t{id}.wav")
}

#[tokio::test]
async fn cancelling_mid_run_keeps_finished_tracks_and_a_resume_runs_only_the_rest() {
    let h = harness(8, 2, 8);
    let run = h.run_all(h.items.clone());
    let quit = async {
        h.wait_for(|e| (e.iter().filter(|event| matches!(event, Event::Progress(_))).count() >= 3).then_some(())).await;
        h.coordinator.cancel();
    };
    tokio::join!(run, quit);

    let left = h.unanalyzed();
    let finished: Vec<i64> = (1..=8).filter(|id| !left.contains(id)).collect();
    assert!(!finished.is_empty() && !left.is_empty(), "{:?}", h.events());
    let events_before = h.events().len();
    let work = resume_work(&h.shared.lock().unwrap(), true).unwrap();
    assert_eq!(resume_ids(&work), left);

    let outcome = h.run_all(work).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, .. }));
    let after = h.events()[events_before..].to_vec();
    for id in finished {
        assert!(after.iter().all(|e| !matches!(e, Event::Ran(name, _) if *name == name_of(id))), "t{id} ran again: {after:?}");
    }
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn work_merged_while_a_fast_pass_is_stored_does_not_run_the_track_twice() {
    let h = fast_first_harness(3, 2, 8);
    let (coordinator, items) = (h.coordinator.clone(), h.items.clone());
    *h.sink.on_partial.lock().unwrap() = Some(Box::new(move |id| {
        merge_into_analysis_queue(&coordinator, items.iter().filter(|(item, _)| *item == id).cloned().collect(), false);
    }));

    let outcome = h.run_all(h.items.clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, .. }));
    for id in 1..=3 {
        let passes = (h.count(&ran(&name_of(id), Pass::Fast)), h.count(&ran(&name_of(id), Pass::Grid)));
        assert_eq!(passes, (1, 1), "t{id}: {:?}", h.events());
    }
}

#[test]
fn a_run_that_is_closing_takes_no_more_work() {
    let coordinator = active_coordinator();
    coordinator.with_queue(|queue| queue.clear());
    assert!(coordinator.seal_if_idle());

    assert_eq!(merge_into_analysis_queue(&coordinator, vec![(3, PathBuf::from("/c.wav"))], false), 0);
    assert_eq!(coordinator.queue_len(), 0);
}

#[test]
fn a_run_with_queued_work_is_not_sealed() {
    let coordinator = active_coordinator();
    assert!(!coordinator.seal_if_idle());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn work_arriving_as_a_run_closes_is_not_lost() {
    let h = harness(3, 2, 8);
    let late = h.items[2].clone();
    let handle = tokio::runtime::Handle::current();
    let (shared, coordinator, sink, runner) = (h.shared.clone(), h.coordinator.clone(), h.sink.clone(), h.runner());
    let late_run: Arc<Mutex<Option<std::thread::JoinHandle<()>>>> = Arc::default();
    let slot = late_run.clone();
    let fired = AtomicBool::new(false);
    *h.sink.on_complete.lock().unwrap() = Some(Box::new(move |_| {
        if fired.swap(true, Ordering::SeqCst) {
            return;
        }
        let (shared, coordinator, sink, runner, late, handle) =
            (shared.clone(), coordinator.clone(), sink.clone(), runner.clone(), late.clone(), handle.clone());
        let join = std::thread::spawn(move || {
            handle.block_on(prioritize_with_runner(shared, coordinator, late, sink, BackendEnv::default(), runner)).unwrap();
        });
        std::thread::sleep(Duration::from_millis(300));
        *slot.lock().unwrap() = Some(join);
    }));

    h.run_all(h.items[..2].to_vec()).await;
    let join = late_run.lock().unwrap().take().expect("the late request was made while the run closed");
    tokio::task::spawn_blocking(move || join.join().unwrap()).await.unwrap();

    assert_eq!(h.unanalyzed(), Vec::<i64>::new(), "{:?}", h.events());
    assert_eq!(h.count(&ran("t3.wav", Pass::All)), 1);
    assert!(!h.coordinator.is_active());
}

#[tokio::test]
async fn a_run_that_cannot_start_its_analyzer_still_tells_the_ui_it_is_over() {
    let h = harness(2, 1, 8);

    let outcome = run_with_resolver(h.shared.clone(), h.coordinator.clone(), h.items.clone(), h.sink.clone(), None, || Err("no analyzer".to_string())).await;

    assert_eq!(outcome.err().as_deref(), Some("no analyzer"));
    assert_eq!(h.sink.log_of_completions(), 1);
    assert!(!h.coordinator.is_active());
}

#[tokio::test]
async fn back_to_back_runs_never_analyse_a_track_twice_at_once() {
    let h = harness(6, 2, 8);
    for _ in 0..4 {
        let run = h.run_all(h.items.clone());
        let stop = async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            h.coordinator.cancel();
        };
        tokio::time::timeout(Duration::from_secs(10), async { tokio::join!(run, stop) })
            .await
            .unwrap_or_else(|_| panic!("a cancelled run never ended: {:?}", h.events()));
        assert!(!h.coordinator.is_active());
    }
    let work = resume_work(&h.shared.lock().unwrap(), true).unwrap();
    h.run_all(work).await;

    assert!(h.unanalyzed().is_empty());
    let mut open: HashSet<String> = HashSet::new();
    for event in h.events() {
        match event {
            Event::Start(name, _) => assert!(open.insert(name.clone()), "{name} started while still running: {:?}", h.events()),
            Event::End(name) | Event::Cancelled(name) => {
                open.remove(&name);
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn a_track_removed_while_queued_is_dropped_and_leaves_nothing_behind() {
    let h = harness(3, 1, 8);
    h.shared.lock().unwrap().remove_track(2).unwrap();

    let outcome = h.run_all(h.items.clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { total: 3, .. }));
    let lib = h.shared.lock().unwrap();
    assert!(lib.get_track(2).unwrap().is_none());
    assert_eq!(lib.analysis_stages(2).unwrap(), None);
    assert_eq!(lib.complete_analysis_track_ids().unwrap(), vec![1, 3]);
}

#[tokio::test]
async fn a_result_for_a_file_the_track_no_longer_points_at_is_not_stored() {
    let h = harness(2, 1, 8);
    let moved = std::env::temp_dir().join(format!("ff-relink-{}", std::process::id())).join("t1.wav");
    std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
    write_tone_wav(&moved, 0.1, 440.0);
    h.shared.lock().unwrap().relink_track(1, &moved).unwrap();

    h.run_all(h.items.clone()).await;

    let lib = h.shared.lock().unwrap();
    assert_eq!(lib.analysis_stages(1).unwrap(), None);
    assert_eq!(lib.analysis_stages(2).unwrap(), Some(analysis_stage::ALL));
    let resumed = resume_work(&lib, true).unwrap();
    assert_eq!(resumed, vec![(1, moved.clone())]);
    std::fs::remove_dir_all(moved.parent().unwrap()).ok();
}

#[tokio::test]
async fn a_priority_request_that_cannot_start_its_analyzer_still_tells_the_ui_it_is_over() {
    let h = harness(2, 1, 8);

    let outcome = prioritize_with_resolver(h.shared.clone(), h.coordinator.clone(), h.items[0].clone(), h.sink.clone(), BackendEnv::default(), || {
        Err("no analyzer".to_string())
    })
    .await;

    assert_eq!(outcome.err().as_deref(), Some("no analyzer"));
    assert_eq!(h.sink.log_of_completions(), 1);
}

#[tokio::test]
async fn runs_started_at_the_same_moment_analyse_each_track_once() {
    let h = harness(6, 2, 8);

    let (a, b, c) = tokio::join!(h.run_all(h.items.clone()), h.run_all(h.items.clone()), h.run_all(h.items.clone()));

    let outcomes = [a, b, c];
    assert_eq!(outcomes.iter().filter(|o| matches!(o, RunOutcome::Finished { .. })).count(), 1);
    assert!(h.unanalyzed().is_empty());
    for id in 1..=6 {
        let name = name_of(id);
        let passes = (h.count(&ran(&name, Pass::Fast)), h.count(&ran(&name, Pass::Grid)), h.count(&ran(&name, Pass::All)));
        assert!(passes == (1, 1, 0) || passes == (0, 0, 1), "{name}: {:?}", h.events());
    }
}

#[tokio::test]
async fn a_track_stopped_by_cancelling_is_not_counted_as_a_failure() {
    let h = harness(1, 1, 8);
    let run = h.run_all(h.items.clone());
    let quit = async {
        h.wait_for(|e| has(e, &ran("t1.wav", Pass::All)).then_some(())).await;
        h.coordinator.cancel();
    };

    let (outcome, ()) = tokio::join!(run, quit);

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, .. }), "{:?}", h.events());
    assert_eq!(h.count(&named(Event::Cancelled, "t1.wav")), 1);
}

fn write_tone_wav(path: &Path, seconds: f64, hz: f64) {
    let rate = 44_100u32;
    let mut data = Vec::new();
    for i in 0..(rate as f64 * seconds) as usize {
        let t = i as f64 / rate as f64;
        let left = ((2.0 * std::f64::consts::PI * hz * t).sin() * 12_000.0) as i16;
        let right = ((2.0 * std::f64::consts::PI * hz * 3.1 * t).sin() * 9_000.0) as i16;
        data.extend_from_slice(&left.to_le_bytes());
        data.extend_from_slice(&right.to_le_bytes());
    }
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(&data);
    std::fs::write(path, wav).unwrap();
}

#[tokio::test]
async fn every_analysed_track_leaves_the_detail_series_the_app_would_measure_in_the_cache() {
    let dir = std::env::temp_dir().join(format!("ff-run-detail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = harness(0, 2, 8);
    h.items = (1..=4)
        .map(|n| {
            let path = dir.join(format!("t{n}.wav"));
            write_tone_wav(&path, 1.0 + n as f64 * 0.25, 60.0 * n as f64);
            let t = Track { source_path: path.clone(), ..track(n) };
            (h.shared.lock().unwrap().add_track(&t).unwrap(), path)
        })
        .collect();
    let env = BackendEnv { app_data_dir: Some(dir.clone()), ..BackendEnv::default() };
    let cache = dir.join(DETAIL_CACHE_DIR);

    let outcome = run_with_runner(h.shared.clone(), h.coordinator.clone(), h.items.clone(), h.sink.clone(), env, Some(h.runner())).await.unwrap();
    let stored = std::fs::read_dir(&cache).unwrap().count();
    let pairs: Vec<_> = h
        .items
        .iter()
        .map(|(_, path)| {
            let cached = crate::waveform_detail::detail_bytes(&cache, path).unwrap();
            (cached, crate::band_analysis::measure_detail(path).unwrap().to_bytes())
        })
        .collect();
    let events = h.events();
    std::fs::remove_dir_all(&dir).ok();

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    assert!(events.iter().any(|e| matches!(e, Event::Ran(_, Pass::Fast))), "the run mixes fast and whole passes: {events:?}");
    assert_eq!(stored, 4);
    for (cached, measured) in pairs {
        assert_eq!(cached, measured);
    }
}

fn order_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ff-run-order-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn order_file_left(dir: &Path) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("analysis-order"))
}

#[tokio::test]
async fn a_run_keeps_its_pending_order_in_a_side_file_and_removes_it_when_it_finishes() {
    let dir = order_dir("run");
    let h = harness(5, 1, 8);
    let queued: Vec<AnalysisWorkItem> = [3, 1, 2, 5, 4].iter().map(|id| h.items[*id as usize - 1].clone()).collect();
    let seen: Arc<Mutex<Option<Vec<i64>>>> = Arc::default();
    let seen_in_hook = seen.clone();
    let hook_dir = dir.clone();
    *h.sink.on_partial.lock().unwrap() = Some(Box::new(move |_| {
        seen_in_hook.lock().unwrap().get_or_insert_with(|| crate::analysis_order::load(&hook_dir));
    }));
    let env = BackendEnv { app_data_dir: Some(dir.clone()), ..BackendEnv::default() };

    let outcome = run_with_runner(h.shared.clone(), h.coordinator.clone(), queued, h.sink.clone(), env, Some(h.runner())).await.unwrap();
    let file_left = order_file_left(&dir);
    std::fs::remove_dir_all(&dir).ok();

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 5 }));
    assert_eq!(seen.lock().unwrap().clone(), Some(vec![3, 1, 2, 5, 4]));
    assert!(!file_left);
}

#[tokio::test]
async fn a_cancelled_run_removes_the_side_file_too() {
    let dir = order_dir("cancel");
    let h = harness(4, 1, 8);
    let env = BackendEnv { app_data_dir: Some(dir.clone()), ..BackendEnv::default() };
    let run = run_with_runner(h.shared.clone(), h.coordinator.clone(), h.items.clone(), h.sink.clone(), env, Some(h.runner()));
    let cancel = async {
        h.wait_for(|e| (!e.is_empty()).then_some(())).await;
        h.coordinator.cancel();
    };
    let (outcome, ()) = tokio::join!(run, cancel);
    let file_left = order_file_left(&dir);
    std::fs::remove_dir_all(&dir).ok();

    assert!(matches!(outcome.unwrap(), RunOutcome::Finished { .. }));
    assert!(!file_left);
}

#[test]
fn resume_follows_the_saved_order_for_tracks_still_pending_and_skips_removed_and_finished_ones() {
    let dir = order_dir("resume");
    let h = harness(5, 1, 8);
    let lib = h.shared.lock().unwrap();
    lib.set_analysis(2, &finished_result()).unwrap();
    crate::analysis_order::save(&dir, &[3, 99, 2, 5]).unwrap();

    let work = resume_work_in_saved_order(&lib, true, Some(&dir)).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(resume_ids(&work), vec![3, 5, 1, 4]);
}

#[test]
fn resume_keeps_todays_order_without_a_usable_side_file() {
    let dir = order_dir("fallback");
    let h = harness(4, 1, 8);
    let lib = h.shared.lock().unwrap();
    lib.set_analysis_stages(4, &finished_result(), analysis_stage::WAVEFORM).unwrap();
    let todays = resume_ids(&resume_work(&lib, true).unwrap());

    let missing = resume_ids(&resume_work_in_saved_order(&lib, true, Some(&dir)).unwrap());
    std::fs::write(dir.join("analysis-order.json"), b"[3, 2,").unwrap();
    let corrupt = resume_ids(&resume_work_in_saved_order(&lib, true, Some(&dir)).unwrap());
    let no_dir = resume_ids(&resume_work_in_saved_order(&lib, true, None).unwrap());
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(todays, vec![4, 1, 2, 3]);
    assert_eq!(missing, todays);
    assert_eq!(corrupt, todays);
    assert_eq!(no_dir, todays);
}

fn failure_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ff-failmemo-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

impl Harness {
    fn remembering_failures(mut self, dir: &Path) -> Self {
        self.coordinator = AnalysisCoordinator::default().with_failure_memo(FailureMemo::in_dir(dir.to_path_buf(), "test"));
        self
    }

    fn runs_of(&self, name: &str) -> usize {
        self.events().iter().filter(|e| matches!(e, Event::Ran(n, _) if n == name)).count()
    }

    fn fail_on(&self, names: &[&str]) {
        *self.engine.failing.lock().unwrap() = names.iter().map(|n| n.to_string()).collect();
    }

    fn resume_without_known_failures(&self) -> (Vec<i64>, usize) {
        let lib = self.shared.lock().unwrap();
        let (work, skipped) = without_known_failures(&lib, &self.coordinator, resume_work(&lib, true).unwrap()).unwrap();
        (resume_ids(&work), skipped)
    }

    fn work_for(&self, ids: &[i64]) -> Vec<AnalysisWorkItem> {
        self.items.iter().filter(|(id, _)| ids.contains(id)).cloned().collect()
    }
}

#[tokio::test]
async fn resume_stops_retrying_a_track_after_two_failed_runs() {
    let dir = failure_dir("resume");
    let h = harness(3, 1, 8).remembering_failures(&dir);
    h.fail_on(&["t2.wav"]);

    let first = h.run_all(h.items.clone()).await;
    assert!(matches!(first, RunOutcome::Finished { errors: 1, total: 3 }));
    assert_eq!(h.resume_without_known_failures(), (vec![2], 0));

    h.run_all(h.work_for(&[2])).await;

    assert_eq!(h.resume_without_known_failures(), (vec![], 1));
    assert_eq!(h.runs_of("t2.wav"), 2, "{:?}", h.events());
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_rust_analyzer_failure_is_recorded_as_a_failure_and_the_track_is_not_analysed_another_way() {
    let dir = failure_dir("no-fallback");
    let h = harness(2, 1, 8).remembering_failures(&dir);
    h.fail_on(&["t2.wav"]);

    let outcome = h.run_all(h.items.clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 1, total: 2 }));
    assert_eq!(h.runs_of("t2.wav"), 1, "{:?}", h.events());
    assert_eq!(h.unanalyzed(), vec![2]);
    assert_eq!(h.shared.lock().unwrap().analysis_failures().unwrap().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn an_explicit_request_for_a_skipped_track_runs_it_and_a_success_clears_the_entry() {
    let dir = failure_dir("explicit");
    let h = harness(2, 1, 8).remembering_failures(&dir);
    h.fail_on(&["t2.wav"]);
    h.run_all(h.items.clone()).await;
    h.run_all(h.work_for(&[2])).await;
    assert_eq!(h.resume_without_known_failures(), (vec![], 1));
    let runs_before = h.runs_of("t2.wav");

    let again = rerun_work(&h.shared.lock().unwrap(), &[2]).unwrap();
    h.run_all(again).await;
    assert_eq!(h.runs_of("t2.wav"), runs_before + 1, "the request ran although the track is skipped");
    assert_eq!(h.resume_without_known_failures(), (vec![], 1), "it failed again, so it stays skipped");

    h.fail_on(&[]);
    let again = rerun_work(&h.shared.lock().unwrap(), &[2]).unwrap();
    h.run_all(again).await;

    assert!(h.unanalyzed().is_empty());
    assert_eq!(h.resume_without_known_failures(), (vec![], 0));
    assert!(h.shared.lock().unwrap().analysis_failures().unwrap().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_priority_request_for_a_skipped_track_runs_it() {
    let dir = failure_dir("priority");
    let h = harness(2, 1, 8).remembering_failures(&dir);
    h.fail_on(&["t2.wav"]);
    h.run_all(h.items.clone()).await;
    h.run_all(h.work_for(&[2])).await;
    assert_eq!(h.resume_without_known_failures(), (vec![], 1));

    h.fail_on(&[]);
    prioritize_with_runner(h.shared.clone(), h.coordinator.clone(), h.items[1].clone(), h.sink.clone(), BackendEnv::default(), h.runner())
        .await
        .unwrap();

    assert!(h.unanalyzed().is_empty());
    assert_eq!(h.resume_without_known_failures(), (vec![], 0));
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_result_that_cannot_be_stored_is_not_a_file_failure() {
    let dir = failure_dir("not-stored");
    let h = harness(2, 1, 8).remembering_failures(&dir);
    let moved = dir.join("t1.wav");
    write_tone_wav(&moved, 0.1, 440.0);
    h.shared.lock().unwrap().relink_track(1, &moved).unwrap();

    h.run_all(h.items.clone()).await;
    h.run_all(h.work_for(&[1])).await;

    let lib = h.shared.lock().unwrap();
    assert_eq!(h.coordinator.failures().entry(&lib, 1), None);
    assert_eq!(h.coordinator.failures().entry(&lib, 2), None);
    drop(lib);
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_cancelled_analysis_is_not_a_file_failure() {
    let dir = failure_dir("cancelled");
    let h = harness(1, 1, 8).remembering_failures(&dir);
    let run = h.run_all(h.items.clone());
    let cancel = async {
        h.wait_for(|e| e.iter().any(|e| matches!(e, Event::Start(..))).then_some(())).await;
        h.coordinator.cancel();
    };
    tokio::join!(run, cancel);

    assert_eq!(h.coordinator.failures().entry(&h.shared.lock().unwrap(), 1), None);
    std::fs::remove_dir_all(&dir).ok();
}

fn coordinator_with_queue(ids: impl IntoIterator<Item = i64>) -> AnalysisCoordinator {
    let coordinator = AnalysisCoordinator::default();
    assert!(coordinator.try_begin());
    let queue: VecDeque<AnalysisWorkItem> = ids.into_iter().map(|id| (id, PathBuf::from(name_of(id)))).collect();
    let total = queue.len() as u32;
    coordinator.register_run(Arc::new(Mutex::new(queue)), total);
    coordinator
}

fn queued_ids(coordinator: &AnalysisCoordinator) -> Vec<i64> {
    coordinator.with_queue(|queue| queue.iter().map(|(id, _)| *id).collect()).unwrap()
}

fn grid_queued_ids(coordinator: &AnalysisCoordinator) -> Vec<i64> {
    coordinator.jobs.lock().unwrap().grid_queue.iter().map(|(id, _)| *id).collect()
}

const ROOMY: ModelJobs = ModelJobs { max: 3, whole: 1 };

fn take(coordinator: &AnalysisCoordinator, stored: &[i64]) -> Option<(i64, Stage)> {
    let stored: HashSet<i64> = stored.iter().copied().collect();
    coordinator
        .take_next(true, |id| if stored.contains(&id) { FAST_STAGES } else { 0 }, ROOMY, |_| 2)
        .map(|task| (task.track_id, task.stage))
}

#[test]
fn focused_tracks_move_to_the_front_in_focus_order_and_the_rest_keep_theirs() {
    let coordinator = coordinator_with_queue(1..=10);
    coordinator.focus(&[7, 3, 99, 9, 7]);
    assert_eq!(queued_ids(&coordinator), vec![7, 3, 9, 1, 2, 4, 5, 6, 8, 10]);
}

#[test]
fn a_new_focus_replaces_the_old_and_tracks_that_left_it_go_back_to_their_place() {
    let coordinator = coordinator_with_queue(1..=8);
    coordinator.focus(&[6, 7]);
    coordinator.focus(&[2]);
    assert_eq!(queued_ids(&coordinator), vec![2, 1, 3, 4, 5, 6, 7, 8]);
    coordinator.focus(&[]);
    assert_eq!(queued_ids(&coordinator), (1..=8).collect::<Vec<_>>());
}

#[test]
fn a_focus_set_before_the_run_starts_orders_its_queue() {
    let coordinator = AnalysisCoordinator::default();
    coordinator.focus(&[4, 2]);
    assert!(coordinator.try_begin());
    let queue: VecDeque<AnalysisWorkItem> = (1..=5).map(|id| (id, PathBuf::from(name_of(id)))).collect();
    coordinator.register_run(Arc::new(Mutex::new(queue)), 5);
    assert_eq!(queued_ids(&coordinator), vec![4, 2, 1, 3, 5]);
}

#[test]
fn merged_tracks_take_their_focus_place_and_unfocused_ones_queue_at_the_back() {
    let coordinator = coordinator_with_queue(1..=4);
    coordinator.focus(&[6, 2]);
    let work = |ids: &[i64]| ids.iter().map(|id| (*id, PathBuf::from(name_of(*id)))).collect::<Vec<_>>();
    assert_eq!(coordinator.merge_work(&work(&[5, 6]), false), Some(2));
    assert_eq!(queued_ids(&coordinator), vec![6, 2, 1, 3, 4, 5]);
    assert_eq!(coordinator.merge_work(&work(&[7, 4]), true), Some(1));
    assert_eq!(queued_ids(&coordinator), vec![6, 2, 4, 7, 1, 3, 5], "front merges lead the unfocused tracks");
}

#[test]
fn without_a_focus_merges_behave_as_before() {
    let coordinator = coordinator_with_queue(1..=3);
    let work = |ids: &[i64]| ids.iter().map(|id| (*id, PathBuf::from(name_of(*id)))).collect::<Vec<_>>();
    coordinator.merge_work(&work(&[4]), false);
    coordinator.merge_work(&work(&[5, 3]), true);
    assert_eq!(queued_ids(&coordinator), vec![3, 5, 1, 2, 4]);
}

#[test]
fn a_focus_names_at_most_the_band_limit() {
    let coordinator = coordinator_with_queue(1..=(MAX_FOCUS as i64 + 10));
    let wanted: Vec<i64> = (1..=(MAX_FOCUS as i64 + 10)).rev().collect();
    coordinator.focus(&wanted);
    let ids = queued_ids(&coordinator);
    assert_eq!(ids[..MAX_FOCUS], wanted[..MAX_FOCUS]);
    assert_eq!(ids[MAX_FOCUS..], (1..=10).collect::<Vec<_>>());
}

#[test]
fn a_focused_track_gets_the_model_while_unfocused_new_tracks_take_fast_passes() {
    let coordinator = coordinator_with_queue(1..=6);
    assert_eq!(take(&coordinator, &[]), Some((1, Stage::Whole)), "the first new track starts the model as before");
    assert_eq!(take(&coordinator, &[]), Some((2, Stage::Fast)));
    coordinator.focus(&[5, 4]);
    assert_eq!(take(&coordinator, &[]), Some((5, Stage::Whole)));
    assert_eq!(take(&coordinator, &[]), Some((4, Stage::Whole)));
    assert_eq!(take(&coordinator, &[]), Some((3, Stage::Fast)), "the model is full: three model jobs run");
}

#[test]
fn a_focused_track_waiting_for_its_grid_pass_goes_ahead_of_new_tracks() {
    let coordinator = coordinator_with_queue(1..=6);
    let first = take(&coordinator, &[]).unwrap();
    assert_eq!(first, (1, Stage::Whole));
    assert_eq!(take(&coordinator, &[]), Some((2, Stage::Fast)));
    assert_eq!(take(&coordinator, &[]), Some((3, Stage::Fast)));
    coordinator.hand_to_grid((3, PathBuf::from(name_of(3))));
    coordinator.hand_to_grid((2, PathBuf::from(name_of(2))));
    assert_eq!(grid_queued_ids(&coordinator), vec![3, 2], "without a focus grid passes queue as they come");
    coordinator.focus(&[2]);
    assert_eq!(take(&coordinator, &[2, 3]), Some((2, Stage::Grid)));
    assert_eq!(take(&coordinator, &[2, 3]), Some((4, Stage::Fast)), "unfocused grid passes still wait for the fast passes");
}

#[test]
fn a_focused_new_track_ranked_above_a_focused_grid_pass_goes_first() {
    let coordinator = coordinator_with_queue(1..=4);
    assert_eq!(take(&coordinator, &[]), Some((1, Stage::Whole)));
    assert_eq!(take(&coordinator, &[]), Some((2, Stage::Fast)));
    coordinator.hand_to_grid((2, PathBuf::from(name_of(2))));
    coordinator.focus(&[4, 2]);
    assert_eq!(take(&coordinator, &[2]), Some((4, Stage::Whole)));
    assert_eq!(take(&coordinator, &[2]), Some((2, Stage::Grid)));
}

#[tokio::test]
async fn every_track_is_analysed_exactly_once_while_the_focus_keeps_moving() {
    let h = harness(40, 3, 6);
    let run = h.run_all(h.items.clone());
    let scroll = async {
        let bands: [&[i64]; 6] = [&[20, 21, 22, 23], &[35, 36, 1], &[], &[10, 11, 12, 13, 14, 15], &[40, 39, 38], &[2, 30]];
        for band in bands {
            h.wait_for(|e| (!e.is_empty()).then_some(())).await;
            h.coordinator.focus(band);
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
    };
    let (outcome, ()) = tokio::join!(run, scroll);

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 40 }));
    assert!(h.unanalyzed().is_empty());
    for id in 1..=40 {
        let name = name_of(id);
        let passes = (h.count(&ran(&name, Pass::Fast)), h.count(&ran(&name, Pass::Grid)), h.count(&ran(&name, Pass::All)));
        assert!(passes == (1, 1, 0) || passes == (0, 0, 1), "{name}: {passes:?} in {:?}", h.events());
        assert_eq!(h.count(&Event::Progress(id)), 1, "{name}");
    }
}

#[tokio::test]
async fn rows_on_screen_finish_before_tracks_queued_ahead_of_them() {
    let h = harness(30, 3, 6);
    let visible: Vec<i64> = (16..=19).collect();
    let run = h.run_all(h.items.clone());
    let scroll = async {
        h.wait_for(|e| (!e.is_empty()).then_some(())).await;
        h.coordinator.focus(&visible);
    };
    let (outcome, ()) = tokio::join!(run, scroll);

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 30 }));
    let events = h.events();
    let done_at = |id: i64| events.iter().position(|e| *e == Event::Progress(id)).unwrap();
    let last_visible = visible.iter().map(|id| done_at(*id)).max().unwrap();
    let finished_ahead = (5..16).filter(|id| done_at(*id) < last_visible).count();
    assert_eq!(finished_ahead, 0, "tracks 5-15 finished before the rows on screen: {events:?}");
}

/// Prints how long the rows in the middle of the list (and a played track) wait, with and without a focus.
/// `cargo test -p pioneer-test-ui time_until_rows_on_screen -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn time_until_rows_on_screen_are_analysed_with_and_without_a_focus() {
    const TRACKS: usize = 120;
    let visible: Vec<i64> = (55..=70).collect();
    for focused in [false, true, false, true] {
        let h = harness(TRACKS, 4, 8);
        let started = Instant::now();
        let run = h.run_all(h.items.clone());
        let watch = async {
            h.wait_for(|e| (!e.is_empty()).then_some(())).await;
            if focused {
                h.coordinator.focus(&visible);
            }
            h.wait_for(|e| visible.iter().all(|id| e.contains(&Event::Progress(*id))).then_some(())).await;
            started.elapsed()
        };
        let play = async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let played = h.items[95].clone();
            let asked = Instant::now();
            prioritize_with_runner(h.shared.clone(), h.coordinator.clone(), played, h.sink.clone(), BackendEnv::default(), h.runner())
                .await
                .unwrap();
            asked.elapsed()
        };
        let (_, visible_done, played_done) = tokio::join!(run, watch, play);
        println!(
            "R171 focus={focused} tracks={TRACKS} visible_rows_done={:.2}s played_track_done={:.2}s full_run={:.2}s",
            visible_done.as_secs_f64(),
            played_done.as_secs_f64(),
            started.elapsed().as_secs_f64()
        );
        assert!(h.unanalyzed().is_empty());
    }
}
