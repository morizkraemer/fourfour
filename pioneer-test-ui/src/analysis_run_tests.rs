use super::*;
use crate::analysis_backend::PartialResultSink;
use fourfour_analysis::service::PoolPlan;
use pioneer_usb_writer::models::{AnalysisResult, BeatGrid, Track, WaveformPreview};

#[derive(Debug, Clone, PartialEq)]
enum Event {
    Start(String, usize),
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

struct FakeEngine {
    log: EventLog,
    workers: usize,
    max_threads: usize,
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

    fn analyze(&self, path: &Path, control: &Control, threads: usize, on_partial: PartialResultSink) -> Result<AnalysisResult, String> {
        let name = file_name_of(path);
        self.log.lock().unwrap().push(Event::Start(name.clone(), threads));
        if let Some(emit) = on_partial {
            emit(AnalysisResult { bpm: 0.0, key: String::new(), ..finished_result() });
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
            std::thread::sleep(CHUNK);
        }
        self.log.lock().unwrap().push(Event::End(name));
        Ok(finished_result())
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
        engine: Arc::new(FakeEngine { log: log.clone(), workers, max_threads }),
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

    async fn run_and_prioritize_later(&self, queued: Vec<AnalysisWorkItem>, priority: AnalysisWorkItem) -> RunOutcome {
        let run = run_with_runner(self.shared.clone(), self.coordinator.clone(), queued, self.sink.clone(), BackendEnv::default(), Some(self.runner()));
        let prioritize = async {
            tokio::time::sleep(Duration::from_millis(70)).await;
            prioritize_with_runner(self.shared.clone(), self.coordinator.clone(), priority, self.sink.clone(), BackendEnv::default(), self.runner())
                .await
                .unwrap();
        };
        let (outcome, ()) = tokio::join!(run, prioritize);
        outcome.unwrap()
    }
}

fn start(name: &str, threads: usize) -> Event {
    Event::Start(name.to_string(), threads)
}

fn named(kind: fn(String) -> Event, name: &str) -> Event {
    kind(name.to_string())
}

#[tokio::test]
async fn a_queued_track_jumps_to_the_now_lane_and_background_yields() {
    let h = harness(8, 2, 8);
    let outcome = h.run_and_prioritize_later(h.items.clone(), h.items[6].clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 8 }));
    assert_eq!(h.count(&start("t7.wav", 8)), 1, "{:?}", h.events());
    assert_eq!(h.count(&start("t7.wav", 4)), 0);
    assert!(h.position(&start("t7.wav", 8)) < h.position(&start("t3.wav", 4)), "t7 starts before the third queued track");
    let end_t7 = h.position(&named(Event::End, "t7.wav"));
    assert!(h.position(&named(Event::Paused, "t1.wav")) < end_t7);
    assert!(h.position(&named(Event::Paused, "t2.wav")) < end_t7);
    assert!(h.position(&named(Event::Resumed, "t1.wav")) > end_t7);
    assert!(h.unanalyzed().is_empty());
    assert_eq!(h.count(&Event::Complete), 1);
    assert_eq!(h.events().last(), Some(&Event::Complete));
}

#[tokio::test]
async fn the_waveform_is_stored_before_the_full_result() {
    let h = harness(3, 1, 8);
    h.run_and_prioritize_later(h.items.clone(), h.items[2].clone()).await;

    assert_eq!(h.count(&Event::PartialStored(3, 0.0)), 1, "{:?}", h.events());
    assert!(h.position(&Event::PartialStored(3, 0.0)) < h.position(&named(Event::End, "t3.wav")));
    assert_eq!(h.shared.lock().unwrap().get_analysis(3).unwrap().unwrap().bpm, 128.0);
}

#[tokio::test]
async fn a_narrow_in_flight_job_is_preempted_and_restarted_wide() {
    let h = harness(4, 2, 8);
    let outcome = h.run_and_prioritize_later(h.items.clone(), h.items[1].clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    assert_eq!(h.count(&start("t2.wav", 4)), 1);
    assert_eq!(h.count(&named(Event::Cancelled, "t2.wav")), 1);
    assert_eq!(h.count(&start("t2.wav", 8)), 1);
    assert_eq!(h.count(&named(Event::End, "t2.wav")), 1);
    assert_eq!(h.count(&Event::Progress(2)), 1);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn a_track_missing_from_the_queue_joins_the_run_and_its_total() {
    let h = harness(4, 2, 8);
    let outcome = h.run_and_prioritize_later(h.items[..3].to_vec(), h.items[3].clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 4 }));
    assert_eq!(h.count(&start("t4.wav", 8)), 1);
    assert_eq!(h.count(&Event::Progress(4)), 1);
    assert!(h.unanalyzed().is_empty());
}

#[tokio::test]
async fn a_job_that_already_has_every_thread_is_left_alone() {
    let h = harness(2, 1, 8);
    let outcome = h.run_and_prioritize_later(h.items[..1].to_vec(), h.items[0].clone()).await;

    assert!(matches!(outcome, RunOutcome::Finished { errors: 0, total: 1 }));
    assert_eq!(h.count(&start("t1.wav", 8)), 1);
    assert_eq!(h.count(&named(Event::Cancelled, "t1.wav")), 0);
    assert_eq!(h.count(&Event::Paused("t1.wav".to_string())), 0);
}

#[tokio::test]
async fn prioritizing_while_idle_runs_the_track_alone_with_every_thread() {
    let h = harness(2, 4, 8);
    prioritize_with_runner(h.shared.clone(), h.coordinator.clone(), h.items[1].clone(), h.sink.clone(), BackendEnv::default(), h.runner())
        .await
        .unwrap();

    assert_eq!(h.count(&start("t2.wav", 8)), 1);
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
    let background = coordinator.take_next(|_| 1).unwrap();
    let NowClaim::Granted { cancel: now } = coordinator.claim_now(2, 8) else { panic!("claim refused") };

    coordinator.cancel();

    assert!(background.cancel.load(Ordering::Relaxed) && now.load(Ordering::Relaxed));
    assert_eq!(coordinator.queue_len(), 0);
}
