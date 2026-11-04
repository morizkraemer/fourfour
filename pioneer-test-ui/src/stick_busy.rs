use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::command_error::{CommandError, ErrorKind};

#[derive(Clone, Default)]
pub struct BusySticks(Arc<Mutex<HashSet<PathBuf>>>);

pub struct BusyGuard {
    sticks: BusySticks,
    volume: PathBuf,
}

fn locked(set: &Mutex<HashSet<PathBuf>>) -> MutexGuard<'_, HashSet<PathBuf>> {
    set.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl BusySticks {
    pub fn acquire(&self, volume: &Path) -> Result<BusyGuard, CommandError> {
        let key = volume.canonicalize().unwrap_or_else(|_| volume.to_path_buf());
        if !locked(&self.0).insert(key.clone()) {
            return Err(CommandError::new(
                ErrorKind::StickBusy,
                "This stick is busy. Try again when the current sync finishes.",
            )
            .with_path(volume));
        }
        Ok(BusyGuard { sticks: self.clone(), volume: key })
    }

    pub fn acquire_all<'a>(&self, volumes: impl IntoIterator<Item = &'a Path>) -> Result<Vec<BusyGuard>, CommandError> {
        volumes.into_iter().map(|volume| self.acquire(volume)).collect()
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        locked(&self.sticks.0).remove(&self.volume);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ff-busy-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn second_acquire_is_refused_until_the_first_drops() {
        let sticks = BusySticks::default();
        let dir = temp_dir("second");
        let first = sticks.acquire(&dir).expect("first");
        let refused = sticks.acquire(&dir).err().expect("second refused");
        assert_eq!(refused.kind, ErrorKind::StickBusy);
        drop(first);
        sticks.acquire(&dir).expect("free again");
    }

    #[test]
    fn different_spellings_of_one_volume_collide() {
        let sticks = BusySticks::default();
        let dir = temp_dir("spelling");
        let _held = sticks.acquire(&dir).expect("first");
        let dotted = dir.join(".");
        assert!(sticks.acquire(&dotted).is_err());
    }

    #[test]
    fn different_volumes_do_not_block_each_other() {
        let sticks = BusySticks::default();
        let a = temp_dir("a");
        let b = temp_dir("b");
        let _a = sticks.acquire(&a).expect("a");
        let _b = sticks.acquire(&b).expect("b");
    }

    #[test]
    fn releases_when_the_operation_fails() {
        let sticks = BusySticks::default();
        let dir = temp_dir("error");
        let outcome: Result<(), CommandError> = (|| {
            let _guard = sticks.acquire(&dir)?;
            Err(CommandError::other("sync failed"))
        })();
        assert!(outcome.is_err());
        sticks.acquire(&dir).expect("released after error");
    }

    #[test]
    fn releases_when_the_operation_panics() {
        let sticks = BusySticks::default();
        let dir = temp_dir("panic");
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _guard = sticks.acquire(&dir).expect("first");
            panic!("writer blew up");
        }));
        assert!(outcome.is_err());
        sticks.acquire(&dir).expect("released after panic");
    }

    #[test]
    fn acquire_all_is_all_or_nothing() {
        let sticks = BusySticks::default();
        let a = temp_dir("all-a");
        let b = temp_dir("all-b");
        let held_b = sticks.acquire(&b).expect("b");
        assert!(sticks.acquire_all([a.as_path(), b.as_path()]).is_err());
        sticks.acquire(&a).expect("a was released by the failed batch");
        drop(held_b);
        assert_eq!(sticks.acquire_all([a.as_path(), a.as_path()]).err().map(|e| e.kind), Some(ErrorKind::StickBusy));
    }
}
