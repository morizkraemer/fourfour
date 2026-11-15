use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use pioneer_library::{AnalysisFailure, LocalLibrary};
use serde::{Deserialize, Serialize};

use crate::analysis_run::AnalysisWorkItem;
use crate::command_error::ErrorKind;

const SIDE_FILE: &str = "analysis-failures.json";
const DECODE_WORDS: [&str; 5] = ["decod", "audio", "codec", "no suitable format", "unsupported"];

/// Failed attempts on an unchanged file after which resume and "analyse all" leave the track out. One failure
/// can be a hiccup (a file still being copied, a timeout on a loaded machine); the second one, in a later run,
/// is the file.
pub(crate) const SKIP_AFTER_ATTEMPTS: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct FailureEntry {
    pub(crate) path: PathBuf,
    pub(crate) size: Option<u64>,
    pub(crate) modified_ms: Option<u64>,
    pub(crate) kind: ErrorKind,
    pub(crate) attempts: u32,
    pub(crate) last_attempt_ms: u64,
}

impl FailureEntry {
    fn describes(&self, path: &Path) -> bool {
        self.path == path && (self.size, self.modified_ms) == fingerprint(&fs::metadata(path))
    }

    fn of(stored: AnalysisFailure) -> Option<Self> {
        Some(Self {
            path: stored.source_path,
            size: stored.file_size,
            modified_ms: stored.modified_ms,
            kind: serde_json::from_value(serde_json::Value::String(stored.kind)).ok()?,
            attempts: stored.attempts,
            last_attempt_ms: stored.failed_at_ms,
        })
    }

    fn stored(&self, track_id: i64, analyzer_version: &str) -> AnalysisFailure {
        AnalysisFailure {
            track_id,
            source_path: self.path.clone(),
            file_size: self.size,
            modified_ms: self.modified_ms,
            kind: serde_json::to_value(self.kind).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
            attempts: self.attempts,
            analyzer_version: analyzer_version.to_string(),
            failed_at_ms: self.last_attempt_ms,
        }
    }
}

/// The side file earlier builds kept in the app data folder.
#[derive(Deserialize)]
struct SideFile {
    app_version: String,
    entries: BTreeMap<i64, FailureEntry>,
}

struct Memory {
    analyzer_version: String,
    side_file: Mutex<Option<PathBuf>>,
}

/// Remembers which tracks failed analysis, in the library's `analysis_failures` table, so a file that cannot be
/// analysed is not retried at every launch. A record made by another analyzer version counts as none: a new
/// analyzer retries every failed track. Without a version it remembers nothing.
#[derive(Clone, Default)]
pub(crate) struct FailureMemo {
    inner: Option<Arc<Memory>>,
}

fn fingerprint(meta: &io::Result<fs::Metadata>) -> (Option<u64>, Option<u64>) {
    match meta {
        Ok(meta) => (Some(meta.len()), meta.modified().ok().map(millis)),
        Err(_) => (None, None),
    }
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn kind_of(meta: &io::Result<fs::Metadata>, message: &str) -> ErrorKind {
    match meta {
        Err(e) if e.kind() == io::ErrorKind::NotFound => ErrorKind::NotFound,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => ErrorKind::PermissionDenied,
        _ => {
            let message = message.to_lowercase();
            if DECODE_WORDS.iter().any(|word| message.contains(word)) {
                ErrorKind::DecodeFailed
            } else {
                ErrorKind::AnalysisFailed
            }
        }
    }
}

impl FailureMemo {
    /// Records under `analyzer_version`; the side file of earlier builds in `dir` is taken over by the first
    /// [`Self::skip_known`].
    pub(crate) fn in_dir(dir: PathBuf, analyzer_version: &str) -> Self {
        Self {
            inner: Some(Arc::new(Memory {
                analyzer_version: analyzer_version.to_string(),
                side_file: Mutex::new(Some(dir.join(SIDE_FILE))),
            })),
        }
    }

    fn stored(&self, lib: &LocalLibrary, track_id: i64) -> Result<Option<(AnalysisFailure, Option<FailureEntry>)>, String> {
        let Some(memory) = &self.inner else { return Ok(None) };
        let Some(stored) = lib.analysis_failure(track_id).map_err(|e| e.to_string())? else { return Ok(None) };
        let entry = (stored.analyzer_version == memory.analyzer_version).then(|| FailureEntry::of(stored.clone())).flatten();
        Ok(Some((stored, entry)))
    }

    #[cfg(test)]
    pub(crate) fn entry(&self, lib: &LocalLibrary, track_id: i64) -> Option<FailureEntry> {
        self.stored(lib, track_id).ok().flatten().and_then(|(_, entry)| entry)
    }

    /// One more failed attempt on the file as it is now; the count starts again when the path, the file or the
    /// analyzer changed.
    pub(crate) fn record_failure(&self, lib: &LocalLibrary, track_id: i64, path: &Path, message: &str) -> Result<(), String> {
        let Some(memory) = &self.inner else { return Ok(()) };
        let meta = fs::metadata(path);
        let (size, modified_ms) = fingerprint(&meta);
        let previous_attempts = self
            .stored(lib, track_id)?
            .and_then(|(_, entry)| entry)
            .filter(|entry| entry.path == path && (entry.size, entry.modified_ms) == (size, modified_ms))
            .map_or(0, |entry| entry.attempts);
        let entry = FailureEntry {
            path: path.to_path_buf(),
            size,
            modified_ms,
            kind: kind_of(&meta, message),
            attempts: previous_attempts + 1,
            last_attempt_ms: millis(SystemTime::now()),
        };
        lib.record_analysis_failure(&entry.stored(track_id, &memory.analyzer_version)).map_err(|e| e.to_string())
    }

    pub(crate) fn record_success(&self, lib: &LocalLibrary, track_id: i64) -> Result<(), String> {
        if self.inner.is_none() {
            return Ok(());
        }
        lib.clear_analysis_failures(&[track_id]).map(|_| ()).map_err(|e| e.to_string())
    }

    /// `work` without the tracks that failed `SKIP_AFTER_ATTEMPTS` times on this very file with this analyzer, and
    /// how many that were. Records whose file changed, whose track points elsewhere now or that another analyzer
    /// made are dropped (the track is retried); records of removed tracks went with the track.
    pub(crate) fn skip_known(&self, lib: &LocalLibrary, work: Vec<AnalysisWorkItem>) -> Result<(Vec<AnalysisWorkItem>, usize), String> {
        let Some(memory) = &self.inner else { return Ok((work, 0)) };
        self.adopt_side_file(lib, memory);
        let mut stale = Vec::new();
        let mut skipped = 0;
        let mut kept = Vec::with_capacity(work.len());
        for item in work {
            match self.stored(lib, item.0)? {
                Some((_, Some(entry))) if entry.describes(&item.1) => {
                    if entry.attempts >= SKIP_AFTER_ATTEMPTS {
                        skipped += 1;
                        continue;
                    }
                }
                Some(_) => stale.push(item.0),
                None => {}
            }
            kept.push(item);
        }
        if !stale.is_empty() {
            lib.clear_analysis_failures(&stale).map_err(|e| e.to_string())?;
        }
        Ok((kept, skipped))
    }

    /// Takes over the entries of the side file earlier builds wrote, for tracks that still point at the same file,
    /// and removes the file. Once per memo.
    fn adopt_side_file(&self, lib: &LocalLibrary, memory: &Memory) {
        let Some(file) = memory.side_file.lock().unwrap_or_else(|p| p.into_inner()).take() else { return };
        let Ok(bytes) = fs::read(&file) else { return };
        if let Ok(saved) = serde_json::from_slice::<SideFile>(&bytes)
            && saved.app_version == memory.analyzer_version
        {
            for (track_id, entry) in saved.entries {
                let same_file = lib.get_track(track_id).ok().flatten().is_some_and(|track| track.source_path == entry.path);
                if same_file && lib.analysis_failure(track_id).ok().flatten().is_none() {
                    let _ = lib.record_analysis_failure(&entry.stored(track_id, &memory.analyzer_version));
                }
            }
        }
        let _ = fs::remove_file(&file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};

    use pioneer_usb_writer::models::Track;

    const VERSION: &str = "1.0.0";

    struct Scratch {
        dir: PathBuf,
        lib: LocalLibrary,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ff-failures-{name}-{}", std::process::id()));
            fs::remove_dir_all(&dir).ok();
            fs::create_dir_all(dir.join("data")).unwrap();
            fs::create_dir_all(dir.join("music")).unwrap();
            let lib = LocalLibrary::open_in_memory().unwrap();
            for id in 1..=3 {
                let track = Track {
                    id: 0,
                    source_path: dir.join("music").join(format!("track{id}.mp3")),
                    usb_path: format!("/Contents/track{id}.mp3"),
                    title: format!("Track {id}"),
                    artist: String::new(),
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
                    duration_secs: 1.0,
                    sample_rate: 44_100,
                    bitrate: 0,
                    file_size: 0,
                    artwork: None,
                };
                assert_eq!(lib.add_track(&track).unwrap(), id);
            }
            Self { dir, lib }
        }

        fn data(&self) -> PathBuf {
            self.dir.join("data")
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.dir.join("music").join(name);
            fs::write(&path, bytes).unwrap();
            path
        }

        fn memo(&self) -> FailureMemo {
            FailureMemo::in_dir(self.data(), VERSION)
        }

        fn side_file(&self) -> PathBuf {
            self.data().join(SIDE_FILE)
        }

        fn fail(&self, memo: &FailureMemo, id: i64, path: &Path, times: u32) {
            for _ in 0..times {
                memo.record_failure(&self.lib, id, path, "failed to decode any audio packets").unwrap();
            }
        }

        fn kept(&self, memo: &FailureMemo, work: Vec<AnalysisWorkItem>) -> (Vec<i64>, usize) {
            let (work, skipped) = memo.skip_known(&self.lib, work).unwrap();
            (work.into_iter().map(|(id, _)| id).collect(), skipped)
        }

        fn entry(&self, memo: &FailureMemo, id: i64) -> Option<FailureEntry> {
            memo.entry(&self.lib, id)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn touch(path: &Path, seconds_after_epoch: u64) {
        let file = fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_after_epoch)).unwrap();
    }

    #[test]
    fn a_track_is_skipped_after_two_failed_attempts_on_the_same_file() {
        let s = Scratch::new("skip");
        let bad = s.file("bad.mp3", b"junk");
        let fine = s.file("fine.mp3", b"ok");
        let memo = s.memo();
        s.fail(&memo, 1, &bad, 2);

        assert_eq!(s.kept(&memo, vec![(1, bad), (2, fine)]), (vec![2], 1));
    }

    #[test]
    fn one_failed_attempt_is_not_enough() {
        let s = Scratch::new("one");
        let bad = s.file("bad.mp3", b"junk");
        let memo = s.memo();
        s.fail(&memo, 1, &bad, 1);

        assert_eq!(s.kept(&memo, vec![(1, bad)]), (vec![1], 0));
        assert_eq!(s.entry(&memo, 1).unwrap().attempts, 1);
    }

    #[test]
    fn a_changed_mtime_or_size_retries_and_clears_the_entry() {
        let s = Scratch::new("changed");
        let by_time = s.file("time.mp3", b"junk");
        let by_size = s.file("size.mp3", b"junk");
        touch(&by_time, 1_000);
        touch(&by_size, 1_000);
        let memo = s.memo();
        s.fail(&memo, 1, &by_time, 2);
        s.fail(&memo, 2, &by_size, 2);
        touch(&by_time, 2_000);
        fs::write(&by_size, b"junk, but longer").unwrap();
        touch(&by_size, 1_000);

        assert_eq!(s.kept(&memo, vec![(1, by_time), (2, by_size)]), (vec![1, 2], 0));
        assert_eq!((s.entry(&memo, 1), s.entry(&memo, 2)), (None, None));
        assert!(s.lib.analysis_failures().unwrap().is_empty());
    }

    #[test]
    fn a_failure_after_a_change_starts_counting_again() {
        let s = Scratch::new("recount");
        let bad = s.file("bad.mp3", b"junk");
        touch(&bad, 1_000);
        let memo = s.memo();
        s.fail(&memo, 1, &bad, 2);
        touch(&bad, 2_000);
        s.fail(&memo, 1, &bad, 1);

        assert_eq!(s.entry(&memo, 1).unwrap().attempts, 1);
        assert_eq!(s.kept(&memo, vec![(1, bad)]), (vec![1], 0));
    }

    #[test]
    fn a_missing_file_stays_skipped_until_it_appears() {
        let s = Scratch::new("missing");
        let path = s.dir.join("music").join("gone.mp3");
        let memo = s.memo();
        s.fail(&memo, 1, &path, 2);
        assert_eq!(s.entry(&memo, 1).unwrap().kind, ErrorKind::NotFound);
        assert_eq!(s.kept(&memo, vec![(1, path.clone())]), (vec![], 1));

        fs::write(&path, b"back").unwrap();
        assert_eq!(s.kept(&memo, vec![(1, path)]), (vec![1], 0));
    }

    #[test]
    fn a_relink_or_relocate_to_another_path_clears_the_entry() {
        let s = Scratch::new("relink");
        let old = s.file("old.mp3", b"junk");
        let moved = s.file("moved.mp3", b"junk");
        let memo = s.memo();
        s.fail(&memo, 1, &old, 2);

        assert_eq!(s.kept(&memo, vec![(1, moved)]), (vec![1], 0));
        assert_eq!(s.entry(&memo, 1), None);
    }

    #[test]
    fn removed_tracks_take_their_record_with_them() {
        let s = Scratch::new("removed");
        let a = s.file("a.mp3", b"junk");
        let b = s.file("b.mp3", b"junk");
        let memo = s.memo();
        s.fail(&memo, 1, &a, 2);
        s.fail(&memo, 2, &b, 2);

        s.lib.remove_track(1).unwrap();

        assert_eq!((s.entry(&memo, 1), s.entry(&memo, 2).map(|e| e.attempts)), (None, Some(2)));
        let reloaded = s.memo();
        assert_eq!(s.entry(&reloaded, 2).map(|e| e.attempts), Some(2));
    }

    #[test]
    fn a_success_clears_the_entry() {
        let s = Scratch::new("success");
        let bad = s.file("bad.mp3", b"junk");
        let memo = s.memo();
        s.fail(&memo, 1, &bad, 2);
        memo.record_success(&s.lib, 1).unwrap();

        assert_eq!(s.entry(&memo, 1), None);
        assert!(s.lib.analysis_failures().unwrap().is_empty());
        assert_eq!(s.kept(&memo, vec![(1, bad)]), (vec![1], 0));
    }

    #[test]
    fn entries_read_back_with_their_kind_and_write_no_side_file() {
        let s = Scratch::new("roundtrip");
        let bad = s.file("bad.mp3", b"junk");
        let slow = s.file("slow.mp3", b"junk");
        let memo = s.memo();
        memo.record_failure(&s.lib, 1, &bad, "failed to decode any audio packets").unwrap();
        memo.record_failure(&s.lib, 2, &slow, "TIMEOUT after 900s").unwrap();
        memo.record_failure(&s.lib, 3, &s.dir.join("music").join("gone.mp3"), "No such file or directory").unwrap();

        let reloaded = s.memo();
        let kinds: Vec<_> = (1..=3).map(|id| s.entry(&reloaded, id).unwrap().kind).collect();
        let first = s.entry(&reloaded, 1).unwrap();

        assert_eq!(kinds, vec![ErrorKind::DecodeFailed, ErrorKind::AnalysisFailed, ErrorKind::NotFound]);
        assert_eq!(fs::read_dir(s.data()).unwrap().count(), 0);
        assert_eq!((first.path, first.size, first.attempts), (bad, Some(4), 1));
        assert!(first.last_attempt_ms > 0);
        assert_eq!(s.lib.analysis_failures().unwrap()[1].kind, "analysis_failed");
    }

    #[test]
    fn records_of_another_analyzer_version_are_retried_and_dropped() {
        let s = Scratch::new("version");
        let bad = s.file("bad.mp3", b"junk");
        let old = FailureMemo::in_dir(s.data(), "0.9.0");
        s.fail(&old, 1, &bad, 2);
        assert_eq!(s.kept(&old, vec![(1, bad.clone())]), (vec![], 1));

        let new = s.memo();
        assert_eq!(s.entry(&new, 1), None);
        s.fail(&new, 1, &bad, 1);
        assert_eq!(s.entry(&new, 1).unwrap().attempts, 1, "the count starts again with the new analyzer");
        assert_eq!(s.kept(&new, vec![(1, bad.clone())]), (vec![1], 0));

        s.fail(&old, 1, &bad, 2);
        assert_eq!(s.kept(&new, vec![(1, bad)]), (vec![1], 0));
        assert!(s.lib.analysis_failures().unwrap().is_empty(), "the other version's record is dropped");
    }

    #[test]
    fn the_side_file_of_earlier_builds_is_taken_over_once_and_removed() {
        let s = Scratch::new("adopt");
        let path_1 = s.lib.get_track(1).unwrap().unwrap().source_path;
        fs::write(&path_1, b"junk").unwrap();
        let (size, modified_ms) = fingerprint(&fs::metadata(&path_1));
        let entry = |path: &Path| FailureEntry {
            path: path.to_path_buf(),
            size,
            modified_ms,
            kind: ErrorKind::DecodeFailed,
            attempts: 2,
            last_attempt_ms: 7,
        };
        let side = serde_json::json!({
            "app_version": VERSION,
            "entries": { "1": entry(&path_1), "2": entry(Path::new("/elsewhere/other.mp3")), "99": entry(&path_1) },
        });
        fs::write(s.side_file(), serde_json::to_vec(&side).unwrap()).unwrap();

        let memo = s.memo();
        assert_eq!(s.kept(&memo, vec![(1, path_1.clone())]), (vec![], 1));
        assert!(!s.side_file().exists());
        let ids: Vec<i64> = s.lib.analysis_failures().unwrap().iter().map(|f| f.track_id).collect();
        assert_eq!(ids, [1], "only the entry whose track still points at the file");

        fs::write(s.side_file(), b"{ not json").unwrap();
        assert_eq!(s.kept(&memo, vec![(1, path_1)]), (vec![], 1));
        assert!(s.side_file().exists(), "taken over once per memo");
        assert_eq!(s.kept(&s.memo(), Vec::new()), (vec![], 0));
        assert!(!s.side_file().exists(), "an unreadable side file is removed without changes");
    }

    #[test]
    fn a_memo_without_a_version_remembers_nothing() {
        let s = Scratch::new("none");
        let memo = FailureMemo::default();
        let path = PathBuf::from("/music/t1.wav");
        memo.record_failure(&s.lib, 1, &path, "x").unwrap();
        memo.record_failure(&s.lib, 1, &path, "x").unwrap();

        assert_eq!(s.entry(&memo, 1), None);
        assert_eq!(s.kept(&memo, vec![(1, path)]), (vec![1], 0));
    }
}
