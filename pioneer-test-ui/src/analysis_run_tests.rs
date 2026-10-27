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
        if let Some(emit) = on_partial {
            emit(AnalysisResult { bpm: 0.0, key: String::new(), ..finished_result() });
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

struct TestSink {
    log: EventLog,
    shared: SharedLibrary,
}

impl AnalysisSink for TestSink {
    fn log(&self, _level: LogLevel, _message: String) {}

    fn progress(&self, _current: u32, _total: u32, _message: String, track_id: i64) {
        self.log.lock().unwrap().push(Event::Progress(track_id));
    }

    fn partial(&self, track_id: i64) {
        let bpm = self.shared.lock().unwrap().get_analysis(track_id).unwrap().map_or(-1.0, |a| a.bpm);
        self.log.lock().unwrap().push(Event::PartialStored(track_id, bpm));
    }

    fn complete(&self) {
        self.log.lock().unwrap().push(Event::Complete);
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
        engine: Arc::new(FakeEngine { log: log.clone(), workers, max_threads, hold_grid: AtomicBool::new(false) }),
        sink: Arc::new(TestSink { log, shared }),
        items,
    }
}

impl Harness {
    fn runner(&self) -> Runner {
        Runner::Rust { engine: self.engine.clone() }
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

    async fn run_and_prioritize_when(
        &self,
        queued: Vec<AnalysisWorkItem>,
        priority: AnalysisWorkItem,
        trigger: impl Fn(&[Event]) -> bool,
    ) -> RunOutcome {
        let run = run_with_runner(self.shared.clone(), self.coordinator.clone(), queued, self.sink.clone(), BackendEnv::default(), Some(self.runner()));
        let prioritize = async {
            tokio::time::timeout(Duration::from_secs(10), async {
                while !trigger(&self.events()) {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            })
            .await
            .expect("trigger never happened");
            prioritize_with_runner(self.shared.clone(), self.coordinator.clone(), priority, self.sink.clone(), BackendEnv::default(), self.runner())
                .await
                .unwrap();
            self.engine.hold_grid.store(false, Ordering::Relaxed);
        };
        let (outcome, ()) = tokio::join!(run, prioritize);
        outcome.unwrap()
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
    let h = harness(5, 2, 8);
    let outcome = h.run_all(h.items.clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 5 }));
    let events = h.events();
    let last_fast = events.iter().rposition(|e| matches!(e, Event::Ran(_, Pass::Fast))).unwrap();
    let first_grid = events.iter().position(|e| matches!(e, Event::Ran(_, Pass::Grid))).unwrap();
    assert!(last_fast < first_grid, "{events:?}");
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
async fn between_the_passes_a_track_is_waveform_and_key_only() {
    let h = harness(3, 1, 8);
    let run = h.run_all(h.items.clone());
    let watch = async {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !has(&h.events(), &ran("t1.wav", Pass::Grid)) {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
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
    assert_eq!(h.count(&ran("t2.wav", Pass::Fast)), 1);
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
    let h = harness(6, 2, 8);
    h.engine.hold_grid.store(true, Ordering::Relaxed);
    let outcome = h
        .run_and_prioritize_when(h.items.clone(), h.items[5].clone(), |e| has(e, &ran("t1.wav", Pass::Grid)) && has(e, &ran("t2.wav", Pass::Grid)))
        .await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 6 }));
    assert_eq!(h.count(&start("t6.wav", 8)), 1, "{:?}", h.events());
    assert_eq!(h.count(&ran("t6.wav", Pass::All)), 1);
    assert_eq!(h.count(&ran("t6.wav", Pass::Grid)), 0, "its queued grid pass was replaced");
    let end_t6 = h.events().iter().rposition(|e| *e == named(Event::End, "t6.wav")).unwrap();
    assert!(h.position(&named(Event::Paused, "t1.wav")) < end_t6);
    assert!(h.position(&named(Event::Paused, "t2.wav")) < end_t6);
    assert!(h.position(&named(Event::Resumed, "t1.wav")) > end_t6);
    assert!(h.position(&ran("t3.wav", Pass::Grid)) > end_t6, "queued grid passes wait for the now lane");
    assert!(h.unanalyzed().is_empty());
    assert_eq!(h.count(&Event::Complete), 1);
    assert_eq!(h.events().last(), Some(&Event::Complete));
}

#[tokio::test]
async fn the_now_lane_stores_the_waveform_before_the_full_result() {
    let h = harness(3, 1, 8);
    h.run_and_prioritize_when(h.items.clone(), h.items[2].clone(), |e| has(e, &ran("t1.wav", Pass::Grid))).await;

    let events = h.events();
    let all_start = events.iter().position(|e| *e == ran("t3.wav", Pass::All)).unwrap();
    let end = events.iter().rposition(|e| *e == named(Event::End, "t3.wav")).unwrap();
    assert!(events[all_start..end].contains(&Event::PartialStored(3, 0.0)), "{events:?}");
    assert_eq!(h.shared.lock().unwrap().get_analysis(3).unwrap().unwrap().bpm, 128.0);
}

#[tokio::test]
async fn a_narrow_grid_job_is_preempted_and_restarted_wide() {
    let h = harness(6, 2, 8);
    let outcome = h
        .run_and_prioritize_when(h.items.clone(), h.items[0].clone(), |e| has(e, &ran("t1.wav", Pass::Grid)))
        .await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 6 }));
    assert_eq!(h.count(&named(Event::Cancelled, "t1.wav")), 1, "{:?}", h.events());
    assert_eq!(h.count(&ran("t1.wav", Pass::All)), 1);
    assert_eq!(h.count(&Event::Progress(1)), 1);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn a_track_missing_from_the_queue_joins_the_run_and_its_total() {
    let h = harness(4, 2, 8);
    let outcome = h
        .run_and_prioritize_when(h.items[..3].to_vec(), h.items[3].clone(), |e| !e.is_empty())
        .await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    assert_eq!(h.count(&start("t4.wav", 8)), 1);
    assert_eq!(h.count(&Event::Progress(4)), 1);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn a_job_that_already_has_every_thread_is_left_alone() {
    let h = harness(2, 1, 8);
    let outcome = h
        .run_and_prioritize_when(h.items[..1].to_vec(), h.items[0].clone(), |e| has(e, &ran("t1.wav", Pass::Grid)))
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

    assert_eq!(h.count(&start("t2.wav", 1)), 1, "the run starts with the fast pass");
    assert_eq!(h.count(&start("t2.wav", 8)), 1, "the grid pass has every thread");
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

#[test]
fn cancel_reaches_every_job() {
    let coordinator = AnalysisCoordinator::default();
    assert!(coordinator.try_begin());
    let queue: SharedAnalysisQueue = Arc::new(Mutex::new(VecDeque::from([(1, PathBuf::from("/a.wav")), (2, PathBuf::from("/b.wav"))])));
    coordinator.register_run(queue, 2);
    let background = coordinator.take_next(true, |_| 0, |_| 1).unwrap();
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
        tokio::time::timeout(Duration::from_secs(10), async {
            while h.events().is_empty() {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        let work = resume_work(&h.shared.lock().unwrap(), true).unwrap();
        assert_eq!(work.len(), 4);
        merge_into_analysis_queue(&h.coordinator, work, false)
    };
    let (outcome, added) = tokio::join!(run, resume);

    assert_eq!(added, 0);
    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    for n in 1..=4 {
        assert_eq!(h.count(&ran(&format!("t{n}.wav"), Pass::Fast)), 1, "{:?}", h.events());
        assert_eq!(h.count(&ran(&format!("t{n}.wav"), Pass::Grid)), 1);
    }
}

#[tokio::test]
async fn resumed_work_yields_to_a_played_track() {
    let h = harness(4, 1, 8);
    let work = resume_work(&h.shared.lock().unwrap(), true).unwrap();
    let outcome = h.run_and_prioritize_when(work[..3].to_vec(), work[3].clone(), |e| !e.is_empty()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    assert_eq!(h.count(&ran("t4.wav", Pass::All)), 1, "{:?}", h.events());
    assert!(h.unanalyzed().is_empty());
}
