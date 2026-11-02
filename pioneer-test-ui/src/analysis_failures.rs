use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::analysis_run::AnalysisWorkItem;
use crate::command_error::ErrorKind;

const FAILURE_FILE: &str = "analysis-failures.json";
const WRITE_INTERVAL: Duration = Duration::from_secs(3);
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
}

#[derive(Serialize, Deserialize)]
struct SideFile {
    app_version: String,
    entries: BTreeMap<i64, FailureEntry>,
}

struct Memory {
    file: PathBuf,
    app_version: String,
    interval: Duration,
    entries: BTreeMap<i64, FailureEntry>,
    dirty: bool,
    last_write: Option<Instant>,
}

/// Remembers which tracks failed analysis, in `analysis-failures.json` next to `analysis-order.json`, so a file
/// that cannot be analysed is not retried at every launch. Without a directory it remembers nothing.
#[derive(Clone, Default)]
pub(crate) struct FailureMemo {
    inner: Arc<Mutex<Option<Memory>>>,
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

impl Memory {
    fn load(dir: &Path, app_version: &str) -> Self {
        let file = dir.join(FAILURE_FILE);
        let saved: Option<SideFile> = fs::read(&file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let entries = saved.filter(|saved| saved.app_version == app_version).map(|saved| saved.entries).unwrap_or_default();
        Self { file, app_version: app_version.to_string(), interval: WRITE_INTERVAL, entries, dirty: false, last_write: None }
    }

    fn write_if_due(&mut self, force: bool) -> io::Result<()> {
        if !self.dirty || (!force && self.last_write.is_some_and(|at| at.elapsed() < self.interval)) {
            return Ok(());
        }
        self.last_write = Some(Instant::now());
        self.write()?;
        self.dirty = false;
        Ok(())
    }

    fn write(&self) -> io::Result<()> {
        if self.entries.is_empty() {
            return match fs::remove_file(&self.file) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let partial = self.file.with_extension(format!("{}.tmp", std::process::id()));
        let saved = SideFile { app_version: self.app_version.clone(), entries: self.entries.clone() };
        fs::write(&partial, serde_json::to_vec(&saved).map_err(io::Error::other)?)?;
        fs::rename(&partial, &self.file)
    }
}

impl FailureMemo {
    pub(crate) fn in_dir(dir: PathBuf, app_version: &str) -> Self {
        Self { inner: Arc::new(Mutex::new(Some(Memory::load(&dir, app_version)))) }
    }

    pub(crate) fn with_interval(self, interval: Duration) -> Self {
        if let Some(memory) = self.lock().as_mut() {
            memory.interval = interval;
        }
        self
    }

    fn lock(&self) -> MutexGuard<'_, Option<Memory>> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn entry(&self, track_id: i64) -> Option<FailureEntry> {
        self.lock().as_ref()?.entries.get(&track_id).cloned()
    }

    /// One more failed attempt on the file as it is now; the count starts again when the path or the file changed.
    pub(crate) fn record_failure(&self, track_id: i64, path: &Path, message: &str) -> io::Result<()> {
        let mut guard = self.lock();
        let Some(memory) = guard.as_mut() else {
            return Ok(());
        };
        let meta = fs::metadata(path);
        let (size, modified_ms) = fingerprint(&meta);
        let previous_attempts = memory.entries.get(&track_id).filter(|entry| entry.path == path && (entry.size, entry.modified_ms) == (size, modified_ms)).map_or(0, |entry| entry.attempts);
        let entry = FailureEntry {
            path: path.to_path_buf(),
            size,
            modified_ms,
            kind: kind_of(&meta, message),
            attempts: previous_attempts + 1,
            last_attempt_ms: millis(SystemTime::now()),
        };
        memory.entries.insert(track_id, entry);
        memory.dirty = true;
        memory.write_if_due(false)
    }

    pub(crate) fn record_success(&self, track_id: i64) -> io::Result<()> {
        let mut guard = self.lock();
        let Some(memory) = guard.as_mut() else {
            return Ok(());
        };
        if memory.entries.remove(&track_id).is_some() {
            memory.dirty = true;
        }
        memory.write_if_due(false)
    }

    pub(crate) fn flush(&self) -> io::Result<()> {
        self.lock().as_mut().map_or(Ok(()), |memory| memory.write_if_due(true))
    }

    /// `work` without the tracks that failed `SKIP_AFTER_ATTEMPTS` times on this very file, and how many that
    /// were. Entries whose file changed or whose track points elsewhere now are dropped (the track is retried),
    /// and so are entries of tracks that are gone.
    pub(crate) fn skip_known(&self, work: Vec<AnalysisWorkItem>, is_track: impl Fn(i64) -> bool) -> io::Result<(Vec<AnalysisWorkItem>, usize)> {
        let mut guard = self.lock();
        let Some(memory) = guard.as_mut() else {
            return Ok((work, 0));
        };
        let before = memory.entries.len();
        memory.entries.retain(|id, _| is_track(*id));
        let mut skipped = 0;
        let mut kept = Vec::with_capacity(work.len());
        for item in work {
            match memory.entries.get(&item.0) {
                Some(entry) if entry.describes(&item.1) => {
                    if entry.attempts >= SKIP_AFTER_ATTEMPTS {
                        skipped += 1;
                        continue;
                    }
                }
                Some(_) => {
                    memory.entries.remove(&item.0);
                }
                None => {}
            }
            kept.push(item);
        }
        if memory.entries.len() != before {
            memory.dirty = true;
            memory.write_if_due(true)?;
        }
        Ok((kept, skipped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::SystemTime;

    const VERSION: &str = "1.0.0";

    struct Scratch {
        dir: PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ff-failures-{name}-{}", std::process::id()));
            fs::remove_dir_all(&dir).ok();
            fs::create_dir_all(dir.join("data")).unwrap();
            fs::create_dir_all(dir.join("music")).unwrap();
            Self { dir }
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
            FailureMemo::in_dir(self.data(), VERSION).with_interval(Duration::ZERO)
        }

        fn side_file(&self) -> PathBuf {
            self.data().join("analysis-failures.json")
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

    fn fail(memo: &FailureMemo, id: i64, path: &Path, times: u32) {
        for _ in 0..times {
            memo.record_failure(id, path, "failed to decode any audio packets").unwrap();
        }
    }

    fn kept(memo: &FailureMemo, work: Vec<AnalysisWorkItem>) -> (Vec<i64>, usize) {
        let (work, skipped) = memo.skip_known(work, |_| true).unwrap();
        (work.into_iter().map(|(id, _)| id).collect(), skipped)
    }

    #[test]
    fn a_track_is_skipped_after_two_failed_attempts_on_the_same_file() {
        let s = Scratch::new("skip");
        let bad = s.file("bad.mp3", b"junk");
        let fine = s.file("fine.mp3", b"ok");
        let memo = s.memo();
        fail(&memo, 1, &bad, 2);

        assert_eq!(kept(&memo, vec![(1, bad), (2, fine)]), (vec![2], 1));
    }

    #[test]
    fn one_failed_attempt_is_not_enough() {
        let s = Scratch::new("one");
        let bad = s.file("bad.mp3", b"junk");
        let memo = s.memo();
        fail(&memo, 1, &bad, 1);

        assert_eq!(kept(&memo, vec![(1, bad)]), (vec![1], 0));
        assert_eq!(memo.entry(1).unwrap().attempts, 1);
    }

    #[test]
    fn a_changed_mtime_or_size_retries_and_clears_the_entry() {
        let s = Scratch::new("changed");
        let by_time = s.file("time.mp3", b"junk");
        let by_size = s.file("size.mp3", b"junk");
        touch(&by_time, 1_000);
        touch(&by_size, 1_000);
        let memo = s.memo();
        fail(&memo, 1, &by_time, 2);
        fail(&memo, 2, &by_size, 2);
        touch(&by_time, 2_000);
        fs::write(&by_size, b"junk, but longer").unwrap();
        touch(&by_size, 1_000);

        assert_eq!(kept(&memo, vec![(1, by_time), (2, by_size)]), (vec![1, 2], 0));
        assert_eq!((memo.entry(1), memo.entry(2)), (None, None));
    }

    #[test]
    fn a_failure_after_a_change_starts_counting_again() {
        let s = Scratch::new("recount");
        let bad = s.file("bad.mp3", b"junk");
        touch(&bad, 1_000);
        let memo = s.memo();
        fail(&memo, 1, &bad, 2);
        touch(&bad, 2_000);
        fail(&memo, 1, &bad, 1);

        assert_eq!(memo.entry(1).unwrap().attempts, 1);
        assert_eq!(kept(&memo, vec![(1, bad)]), (vec![1], 0));
    }

    #[test]
    fn a_missing_file_stays_skipped_until_it_appears() {
        let s = Scratch::new("missing");
        let path = s.dir.join("music").join("gone.mp3");
        let memo = s.memo();
        fail(&memo, 1, &path, 2);
        assert_eq!(memo.entry(1).unwrap().kind, ErrorKind::NotFound);
        assert_eq!(kept(&memo, vec![(1, path.clone())]), (vec![], 1));

        fs::write(&path, b"back").unwrap();
        assert_eq!(kept(&memo, vec![(1, path)]), (vec![1], 0));
    }

    #[test]
    fn a_relink_or_relocate_to_another_path_clears_the_entry() {
        let s = Scratch::new("relink");
        let old = s.file("old.mp3", b"junk");
        let moved = s.file("moved.mp3", b"junk");
        let memo = s.memo();
        fail(&memo, 1, &old, 2);

        assert_eq!(kept(&memo, vec![(1, moved)]), (vec![1], 0));
        assert_eq!(memo.entry(1), None);
    }

    #[test]
    fn removed_tracks_drop_out_of_the_memo() {
        let s = Scratch::new("removed");
        let a = s.file("a.mp3", b"junk");
        let b = s.file("b.mp3", b"junk");
        let memo = s.memo();
        fail(&memo, 1, &a, 2);
        fail(&memo, 2, &b, 2);

        memo.skip_known(Vec::new(), |id| id == 2).unwrap();

        assert_eq!((memo.entry(1), memo.entry(2).map(|e| e.attempts)), (None, Some(2)));
        let reloaded = s.memo();
        assert_eq!((reloaded.entry(1), reloaded.entry(2).map(|e| e.attempts)), (None, Some(2)));
    }

    #[test]
    fn a_success_clears_the_entry() {
        let s = Scratch::new("success");
        let bad = s.file("bad.mp3", b"junk");
        let memo = s.memo();
        fail(&memo, 1, &bad, 2);
        memo.record_success(1).unwrap();

        assert_eq!(memo.entry(1), None);
        assert!(!s.side_file().exists());
        assert_eq!(kept(&memo, vec![(1, bad)]), (vec![1], 0));
    }

    #[test]
    fn entries_read_back_with_their_kind_and_leave_no_temp_file() {
        let s = Scratch::new("roundtrip");
        let bad = s.file("bad.mp3", b"junk");
        let slow = s.file("slow.mp3", b"junk");
        let memo = s.memo();
        memo.record_failure(1, &bad, "failed to decode any audio packets").unwrap();
        memo.record_failure(2, &slow, "TIMEOUT after 900s").unwrap();
        memo.record_failure(3, &s.dir.join("music").join("gone.mp3"), "No such file or directory").unwrap();

        let reloaded = s.memo();
        let kinds: Vec<_> = (1..=3).map(|id| reloaded.entry(id).unwrap().kind).collect();
        let names: Vec<_> = fs::read_dir(s.data()).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        let first = reloaded.entry(1).unwrap();

        assert_eq!(kinds, vec![ErrorKind::DecodeFailed, ErrorKind::AnalysisFailed, ErrorKind::NotFound]);
        assert_eq!(names, ["analysis-failures.json"]);
        assert_eq!((first.path, first.size, first.attempts), (bad, Some(4), 1));
        assert!(first.last_attempt_ms > 0);
    }

    #[test]
    fn a_missing_or_corrupt_side_file_skips_nothing() {
        let s = Scratch::new("corrupt");
        let bad = s.file("bad.mp3", b"junk");
        assert_eq!(kept(&s.memo(), vec![(1, bad.clone())]), (vec![1], 0));

        let good = {
            let memo = s.memo();
            fail(&memo, 1, &bad, 2);
            fs::read(s.side_file()).unwrap()
        };
        let broken: [&[u8]; 6] = [
            &good[..good.len() / 2],
            b"[]",
            b"{\"app_version\": 3, \"entries\": 7}",
            b"{\"app_version\": \"1.0.0\", \"entries\": {\"1\": {\"path\": 5}}}",
            b"{\"app_version\": \"1.0.0\", \"entries\": {\"x\": {}}}",
            &[0xff, 0xfe, 0x00],
        ];
        for bytes in broken {
            fs::write(s.side_file(), bytes).unwrap();
            assert_eq!(kept(&s.memo(), vec![(1, bad.clone())]), (vec![1], 0), "{}", String::from_utf8_lossy(bytes));
        }

        fs::write(s.side_file(), b"{ not json").unwrap();
        let memo = s.memo();
        fail(&memo, 1, &bad, 1);
        assert_eq!(memo.entry(1).unwrap().attempts, 1, "a corrupt file is replaced by the next write");
        assert_eq!(s.memo().entry(1).unwrap().attempts, 1);
    }

    #[test]
    fn entries_of_another_app_version_are_ignored() {
        let s = Scratch::new("version");
        let bad = s.file("bad.mp3", b"junk");
        let old = FailureMemo::in_dir(s.data(), "0.9.0").with_interval(Duration::ZERO);
        fail(&old, 1, &bad, 2);

        assert_eq!(kept(&s.memo(), vec![(1, bad.clone())]), (vec![1], 0));
        assert_eq!(kept(&old, vec![(1, bad)]), (vec![], 1));
    }

    #[test]
    fn writes_are_coalesced_and_a_flush_catches_up() {
        let s = Scratch::new("coalesce");
        let a = s.file("a.mp3", b"junk");
        let memo = FailureMemo::in_dir(s.data(), VERSION).with_interval(Duration::from_secs(3600));
        memo.record_failure(1, &a, "x").unwrap();
        memo.record_failure(2, &a, "x").unwrap();

        let after_two = FailureMemo::in_dir(s.data(), VERSION);
        assert_eq!((after_two.entry(1).is_some(), after_two.entry(2).is_some()), (true, false));

        memo.flush().unwrap();
        let after_flush = FailureMemo::in_dir(s.data(), VERSION);
        assert_eq!((after_flush.entry(1).is_some(), after_flush.entry(2).is_some()), (true, true));

        fs::remove_file(s.side_file()).unwrap();
        memo.flush().unwrap();
        assert!(!s.side_file().exists(), "nothing changed since the last write");
    }

    #[test]
    fn a_memo_without_a_dir_remembers_nothing() {
        let memo = FailureMemo::default();
        let path = PathBuf::from("/music/t1.wav");
        memo.record_failure(1, &path, "x").unwrap();
        memo.record_failure(1, &path, "x").unwrap();
        memo.flush().unwrap();

        assert_eq!(memo.entry(1), None);
        assert_eq!(kept(&memo, vec![(1, path)]), (vec![1], 0));
    }
}
