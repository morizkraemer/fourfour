use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use pioneer_library::{LibraryRevision, LocalLibrary, TrackLabels};

use crate::track_infos;

struct Precomputed {
    revision: LibraryRevision,
    labels: HashMap<i64, TrackLabels>,
    bytes: Vec<u8>,
}

enum Slot {
    Waiting,
    Ready(Precomputed),
    Served,
}

/// The `load_state` answer built while the webview starts, handed out once and only while it is still what
/// [`encoded_state`] would build: the library revision and the labels the answer was built from must both be
/// unchanged. Both functions run with the library locked, so no write can land between the check and the reply.
pub struct EarlyState {
    slot: Mutex<Slot>,
}

impl Default for EarlyState {
    fn default() -> Self {
        Self { slot: Mutex::new(Slot::Waiting) }
    }
}

impl EarlyState {
    /// Builds the answer unless one was already served (or is ready). The revision is read after the build, so a
    /// write the build itself makes (row previews filled in) is part of what it is compared with.
    pub fn prepare(&self, lib: &LocalLibrary) -> Result<(), String> {
        let mut slot = self.slot.lock().map_err(|e| e.to_string())?;
        if !matches!(*slot, Slot::Waiting) {
            return Ok(());
        }
        let (state, labels) = track_infos::loaded_state_with_labels(lib)?;
        let bytes = state.to_bytes()?;
        let revision = lib.revision().map_err(|e| e.to_string())?;
        *slot = Slot::Ready(Precomputed { revision, labels, bytes });
        Ok(())
    }

    fn take_current(&self, lib: &LocalLibrary) -> Result<Option<Vec<u8>>, String> {
        let mut slot = self.slot.lock().map_err(|e| e.to_string())?;
        match std::mem::replace(&mut *slot, Slot::Served) {
            Slot::Ready(early)
                if early.revision == lib.revision().map_err(|e| e.to_string())?
                    && early.labels == lib.track_labels_reader().read_cached() =>
            {
                Ok(Some(early.bytes))
            }
            _ => Ok(None),
        }
    }
}

pub type OpenedLibrary = (PathBuf, Arc<Mutex<LocalLibrary>>);

/// Opens the library and builds the `load_state` answer on a thread of its own, so both run while the webview
/// is created. The opened library comes back on the channel with its lock already held by the thread, so the
/// answer is built before anything else the setup starts can use the library. `ready` gets the build's result.
/// An open error is sent as it is and nothing is built.
pub fn open_and_prepare<E: Send + 'static>(
    open: impl FnOnce() -> Result<(PathBuf, LocalLibrary), E> + Send + 'static,
    early: Arc<EarlyState>,
    ready: impl FnOnce(Result<(), String>) + Send + 'static,
) -> mpsc::Receiver<Result<OpenedLibrary, E>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || match open() {
        Err(error) => {
            let _ = sender.send(Err(error));
        }
        Ok((path, library)) => {
            let library = Arc::new(Mutex::new(library));
            let Ok(locked) = library.lock() else { return };
            let _ = sender.send(Ok((path, library.clone())));
            ready(early.prepare(&locked));
        }
    });
    receiver
}

/// The `load_state` answer: the early one when it is still current, else built now. The library must be locked.
pub fn encoded_state(lib: &LocalLibrary, early: &EarlyState) -> Result<(Vec<u8>, bool), String> {
    if let Some(bytes) = early.take_current(lib)? {
        return Ok((bytes, true));
    }
    Ok((track_infos::loaded_state(lib)?.to_bytes()?, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_usb_writer::models::{AnalysisResult, Beat, BeatGrid, Track, WaveformPreview};
    use std::path::PathBuf;

    fn track(title: &str) -> Track {
        Track {
            id: 0,
            source_path: PathBuf::from(format!("/m/{title}.mp3")),
            usb_path: String::new(),
            title: title.into(),
            artist: "Artist".into(),
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
            duration_secs: 200.0,
            sample_rate: 44100,
            bitrate: 320,
            file_size: 1,
            artwork: None,
        }
    }

    fn analysis() -> AnalysisResult {
        AnalysisResult {
            beat_grid: BeatGrid { beats: vec![Beat { bar_position: 1, time_ms: 0, tempo: 12800 }] },
            waveform: WaveformPreview { data: [9u8; 400] },
            bpm: 128.0,
            key: "8A".into(),
            energy: None,
            cue_points: Vec::new(),
            color_waveform: None,
        }
    }

    fn file_library(name: &str) -> (LocalLibrary, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ff-early-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (LocalLibrary::open(&dir.join("library.db")).unwrap(), dir)
    }

    fn on_demand(lib: &LocalLibrary) -> Vec<u8> {
        track_infos::loaded_state(lib).unwrap().to_bytes().unwrap()
    }

    #[test]
    fn an_untouched_library_gets_the_early_answer_and_the_same_bytes_as_on_demand() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        for title in ["a", "b", "c"] {
            lib.add_track(&track(title)).unwrap();
        }
        let early = EarlyState::default();
        early.prepare(&lib).unwrap();
        let (bytes, was_early) = encoded_state(&lib, &early).unwrap();
        assert!(was_early);
        assert_eq!(bytes, on_demand(&lib));
    }

    #[test]
    fn the_early_answer_is_served_once() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        lib.add_track(&track("a")).unwrap();
        let early = EarlyState::default();
        early.prepare(&lib).unwrap();
        assert!(encoded_state(&lib, &early).unwrap().1);
        assert!(!encoded_state(&lib, &early).unwrap().1);
        early.prepare(&lib).unwrap();
        assert!(!encoded_state(&lib, &early).unwrap().1);
    }

    #[test]
    fn a_write_between_setup_and_the_command_returns_the_current_state() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let id = lib.add_track(&track("a")).unwrap();
        let writes: [(&str, &dyn Fn(&LocalLibrary)); 4] = [
            ("import", &|lib| {
                lib.add_track(&track("b")).unwrap();
            }),
            ("analysis result", &|lib| lib.set_analysis(id, &analysis()).unwrap()),
            ("playlist", &|lib| {
                let list = lib.create_playlist("Set").unwrap();
                lib.set_playlist_tracks(list, &[id]).unwrap();
            }),
            ("removal", &|lib| {
                lib.remove_track(id).unwrap();
            }),
        ];
        for (name, write) in writes {
            let early = EarlyState::default();
            early.prepare(&lib).unwrap();
            let stale = encoded_state(&lib, &EarlyState::default()).unwrap().0;
            write(&lib);
            let (bytes, was_early) = encoded_state(&lib, &early).unwrap();
            assert!(!was_early, "{name}: served a stale answer");
            assert_eq!(bytes, on_demand(&lib), "{name}");
            assert_ne!(bytes, stale, "{name}: the write changed nothing the state shows");
        }
    }

    #[test]
    fn a_write_by_another_connection_returns_the_current_state() {
        let (lib, dir) = file_library("other-connection");
        lib.add_track(&track("a")).unwrap();
        let early = EarlyState::default();
        early.prepare(&lib).unwrap();
        LocalLibrary::open(&dir.join("library.db")).unwrap().add_track(&track("b")).unwrap();
        let (bytes, was_early) = encoded_state(&lib, &early).unwrap();
        assert!(!was_early);
        assert_eq!(bytes, on_demand(&lib));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_rating_set_after_setup_returns_the_current_state() {
        let (lib, dir) = file_library("rating");
        let id = lib.add_track(&track("a")).unwrap();
        lib.set_analysis(id, &analysis()).unwrap();
        let early = EarlyState::default();
        early.prepare(&lib).unwrap();
        lib.set_user_rating(id, 4).unwrap();
        let (bytes, was_early) = encoded_state(&lib, &early).unwrap();
        assert!(!was_early);
        assert_eq!(bytes, on_demand(&lib));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_replaced_library_never_gets_the_old_answer() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        lib.add_track(&track("a")).unwrap();
        let early = EarlyState::default();
        early.prepare(&lib).unwrap();
        let replacement = LocalLibrary::open_in_memory().unwrap();
        replacement.add_track(&track("z")).unwrap();
        let (bytes, was_early) = encoded_state(&replacement, &early).unwrap();
        assert!(!was_early);
        assert_eq!(bytes, on_demand(&replacement));
    }
    #[test]
    fn the_thread_hands_back_the_opened_library_and_has_the_answer_ready_for_the_first_request() {
        let (seed, dir) = file_library("thread-ok");
        for title in ["a", "b", "c"] {
            seed.add_track(&track(title)).unwrap();
        }
        drop(seed);
        let db = dir.join("library.db");
        let early = Arc::new(EarlyState::default());
        let (built_tx, built_rx) = mpsc::channel();
        let opened = open_and_prepare(
            {
                let db = db.clone();
                move || LocalLibrary::open(&db).map(|lib| (db, lib)).map_err(|e| e.to_string())
            },
            early.clone(),
            move |built| built_tx.send(built).unwrap(),
        );
        let (path, library) = opened.recv().unwrap().unwrap();
        assert_eq!(path, db);
        let lib = library.lock().unwrap();
        let (bytes, was_early) = encoded_state(&lib, &early).unwrap();
        assert!(was_early);
        assert_eq!(bytes, on_demand(&lib));
        assert_eq!(built_rx.recv().unwrap(), Ok(()));
    }

    #[test]
    fn a_write_after_the_thread_built_the_answer_returns_the_new_state() {
        let (_seed, dir) = file_library("thread-write");
        let db = dir.join("library.db");
        let early = Arc::new(EarlyState::default());
        let opened = open_and_prepare(
            {
                let db = db.clone();
                move || LocalLibrary::open(&db).map(|lib| (db, lib)).map_err(|e| e.to_string())
            },
            early.clone(),
            |_| {},
        );
        let (_, library) = opened.recv().unwrap().unwrap();
        let lib = library.lock().unwrap();
        lib.add_track(&track("late")).unwrap();
        let (bytes, was_early) = encoded_state(&lib, &early).unwrap();
        assert!(!was_early);
        assert_eq!(bytes, on_demand(&lib));
    }

    #[test]
    fn an_open_error_is_handed_back_unchanged_and_nothing_is_built() {
        let early = Arc::new(EarlyState::default());
        let (built_tx, built_rx) = mpsc::channel::<Result<(), String>>();
        let opened = open_and_prepare(
            || Err::<(PathBuf, LocalLibrary), String>("cannot open".into()),
            early.clone(),
            move |built| built_tx.send(built).unwrap(),
        );
        assert_eq!(opened.recv().unwrap().err().unwrap(), "cannot open");
        assert!(built_rx.recv().is_err());
        assert!(matches!(*early.slot.lock().unwrap(), Slot::Waiting));
    }
}
