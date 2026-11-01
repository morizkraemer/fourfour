use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::analysis_run::AnalysisWorkItem;

const ORDER_FILE: &str = "analysis-order.json";
const WRITE_INTERVAL: Duration = Duration::from_secs(3);

fn order_file(dir: &Path) -> PathBuf {
    dir.join(ORDER_FILE)
}

pub(crate) fn save(dir: &Path, ids: &[i64]) -> io::Result<()> {
    if ids.is_empty() {
        return clear(dir);
    }
    fs::create_dir_all(dir)?;
    let file = order_file(dir);
    let partial = file.with_extension(format!("{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(ids).map_err(io::Error::other)?;
    fs::write(&partial, bytes)?;
    fs::rename(&partial, &file)
}

pub(crate) fn load(dir: &Path) -> Vec<i64> {
    fs::read(order_file(dir)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub(crate) fn clear(dir: &Path) -> io::Result<()> {
    match fs::remove_file(order_file(dir)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Puts the tracks the saved order lists first, in that order; the others follow in their given order.
pub(crate) fn apply(work: Vec<AnalysisWorkItem>, saved: &[i64]) -> Vec<AnalysisWorkItem> {
    let mut rank: HashMap<i64, usize> = HashMap::with_capacity(saved.len());
    for (position, id) in saved.iter().enumerate() {
        rank.entry(*id).or_insert(position);
    }
    let (mut listed, rest): (Vec<_>, Vec<_>) = work.into_iter().partition(|(id, _)| rank.contains_key(id));
    listed.sort_by_key(|(id, _)| rank[id]);
    listed.extend(rest);
    listed
}

/// Keeps the side file in step with a run's queue: at most one write per interval, and none when the order
/// did not change since the last one.
pub(crate) struct OrderJournal {
    dir: Option<PathBuf>,
    interval: Duration,
    last_write: Option<Instant>,
    last_ids: Vec<i64>,
}

impl OrderJournal {
    pub(crate) fn new(dir: Option<PathBuf>) -> Self {
        Self::with_interval(dir, WRITE_INTERVAL)
    }

    pub(crate) fn with_interval(dir: Option<PathBuf>, interval: Duration) -> Self {
        Self { dir, interval, last_write: None, last_ids: Vec::new() }
    }

    pub(crate) fn tick(&mut self, snapshot: impl FnOnce() -> Vec<i64>) -> io::Result<()> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        if self.last_write.is_some_and(|at| at.elapsed() < self.interval) {
            return Ok(());
        }
        self.last_write = Some(Instant::now());
        let ids = snapshot();
        if ids == self.last_ids {
            return Ok(());
        }
        save(dir, &ids)?;
        self.last_ids = ids;
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> io::Result<()> {
        self.last_ids.clear();
        self.dir.as_deref().map_or(Ok(()), clear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ff-order-{name}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn work(ids: &[i64]) -> Vec<AnalysisWorkItem> {
        ids.iter().map(|id| (*id, PathBuf::from(format!("/music/t{id}.wav")))).collect()
    }

    fn ids_of(work: &[AnalysisWorkItem]) -> Vec<i64> {
        work.iter().map(|(id, _)| *id).collect()
    }

    #[test]
    fn a_saved_order_reads_back_and_leaves_no_temp_file() {
        let dir = temp_dir("roundtrip");
        save(&dir, &[5, 2, 9]).unwrap();
        let names: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        let loaded = load(&dir);
        fs::remove_dir_all(&dir).ok();

        assert_eq!(loaded, vec![5, 2, 9]);
        assert_eq!(names, [ORDER_FILE]);
    }

    #[test]
    fn a_missing_or_corrupt_file_reads_as_no_order() {
        let dir = temp_dir("corrupt");
        assert!(load(&dir).is_empty());
        fs::write(order_file(&dir), b"[3, 1, 2").unwrap();
        assert!(load(&dir).is_empty());
        fs::write(order_file(&dir), b"{\"ids\": \"nope\"}").unwrap();
        assert!(load(&dir).is_empty());
        fs::write(order_file(&dir), [0xff, 0xfe, 0x00]).unwrap();
        assert!(load(&dir).is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn listed_tracks_come_first_in_saved_order_and_unknown_ids_are_skipped() {
        let ordered = apply(work(&[1, 2, 3, 4, 5]), &[4, 99, 2, 4]);
        assert_eq!(ids_of(&ordered), vec![4, 2, 1, 3, 5]);
    }

    #[test]
    fn an_empty_saved_order_keeps_the_given_order() {
        assert_eq!(ids_of(&apply(work(&[3, 1, 2]), &[])), vec![3, 1, 2]);
    }

    #[test]
    fn the_journal_writes_the_first_snapshot_then_at_most_once_per_interval() {
        let dir = temp_dir("coalesce");
        let mut journal = OrderJournal::with_interval(Some(dir.clone()), Duration::from_secs(3600));
        journal.tick(|| vec![1, 2, 3]).unwrap();
        journal.tick(|| panic!("a tick inside the interval must not even take a snapshot")).unwrap();
        let loaded = load(&dir);
        fs::remove_dir_all(&dir).ok();

        assert_eq!(loaded, vec![1, 2, 3]);
    }

    #[test]
    fn the_journal_skips_a_write_when_the_order_did_not_change() {
        let dir = temp_dir("unchanged");
        let mut journal = OrderJournal::with_interval(Some(dir.clone()), Duration::ZERO);
        journal.tick(|| vec![1, 2]).unwrap();
        fs::remove_file(order_file(&dir)).unwrap();
        journal.tick(|| vec![1, 2]).unwrap();
        let rewritten_same = order_file(&dir).exists();
        journal.tick(|| vec![2]).unwrap();
        let loaded = load(&dir);
        fs::remove_dir_all(&dir).ok();

        assert!(!rewritten_same);
        assert_eq!(loaded, vec![2]);
    }

    #[test]
    fn finishing_removes_the_file_and_a_journal_without_a_dir_does_nothing() {
        let dir = temp_dir("finish");
        let mut journal = OrderJournal::with_interval(Some(dir.clone()), Duration::ZERO);
        journal.tick(|| vec![1]).unwrap();
        assert!(order_file(&dir).exists());
        journal.finish().unwrap();
        let left = order_file(&dir).exists();
        fs::remove_dir_all(&dir).ok();
        assert!(!left);

        let mut none = OrderJournal::new(None);
        none.tick(|| panic!("no dir, no snapshot")).unwrap();
        none.finish().unwrap();
    }
}
