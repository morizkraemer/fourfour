//! Audio file health: does each track's file still exist where the library expects it,
//! and relinking moved files by searching a folder.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Ok,
    Missing,
    Error,
}

/// One stat call: a readable regular file is `Ok`, a path that is not there is `Missing`,
/// anything else (permission denied, a directory, an I/O error) is `Error`.
pub fn file_status(path: &Path) -> FileStatus {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => FileStatus::Ok,
        Ok(_) => FileStatus::Error,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileStatus::Missing,
        Err(_) => FileStatus::Error,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackFile {
    pub track_id: i64,
    pub path: PathBuf,
    pub file_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relinked {
    pub track_id: i64,
    pub source_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelocateReport {
    pub matched: Vec<Relinked>,
    pub ambiguous: Vec<i64>,
    pub not_found: Vec<i64>,
}

fn name_key(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().to_lowercase())
}

fn index_folder(root: &Path) -> HashMap<String, Vec<(PathBuf, u64)>> {
    let mut by_name: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if let Some(key) = name_key(&path) {
                    by_name.entry(key).or_default().push((path, size));
                }
            }
        }
    }
    by_name
}

/// Finds each missing track under `folder` by file name (case-insensitive). A known size
/// narrows several same-name files to the ones of that size. Exactly one candidate is a
/// match; a file two missing tracks both point at, or more than one candidate, is ambiguous.
/// Paths in `taken` already belong to other tracks and are never offered.
pub fn plan_relocation(
    missing: &[TrackFile],
    folder: &Path,
    taken: &HashSet<PathBuf>,
) -> (Vec<(i64, PathBuf)>, Vec<i64>, Vec<i64>) {
    let index = index_folder(folder);
    let mut picks: Vec<(i64, PathBuf)> = Vec::new();
    let mut ambiguous = Vec::new();
    let mut not_found = Vec::new();

    for track in missing {
        let candidates: Vec<&(PathBuf, u64)> = name_key(&track.path)
            .and_then(|key| index.get(&key))
            .map(|list| list.iter().filter(|(p, _)| !taken.contains(p)).collect())
            .unwrap_or_default();
        let same_size: Vec<&(PathBuf, u64)> = if track.file_size > 0 {
            candidates.iter().copied().filter(|(_, size)| *size == track.file_size).collect()
        } else {
            Vec::new()
        };
        let narrowed = if same_size.is_empty() { candidates } else { same_size };
        match narrowed.as_slice() {
            [] => not_found.push(track.track_id),
            [(path, _)] => picks.push((track.track_id, path.clone())),
            _ => ambiguous.push(track.track_id),
        }
    }

    let mut claims: HashMap<&PathBuf, usize> = HashMap::new();
    for (_, path) in &picks {
        *claims.entry(path).or_default() += 1;
    }
    let (matched, contested): (Vec<_>, Vec<_>) = picks
        .iter()
        .cloned()
        .partition(|(_, path)| claims.get(path).copied() == Some(1));
    ambiguous.extend(contested.into_iter().map(|(id, _)| id));
    (matched, ambiguous, not_found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; bytes]).unwrap();
    }

    fn gone(id: i64, name: &str, size: u64) -> TrackFile {
        TrackFile { track_id: id, path: PathBuf::from(format!("/Volumes/OLD/{name}")), file_size: size }
    }

    #[test]
    fn status_tells_ok_missing_and_error_apart() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.mp3");
        write(&file, 4);
        assert_eq!(file_status(&file), FileStatus::Ok);
        assert_eq!(file_status(&dir.path().join("nope.mp3")), FileStatus::Missing);
        assert_eq!(file_status(dir.path()), FileStatus::Error);
    }

    #[test]
    fn plan_reports_matched_ambiguous_and_not_found() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("deep/a/One.mp3"), 10);
        write(&dir.path().join("x/Two.mp3"), 20);
        write(&dir.path().join("y/Two.mp3"), 20);
        write(&dir.path().join("x/Three.mp3"), 30);
        write(&dir.path().join("y/Three.mp3"), 31);

        let missing = [gone(1, "one.MP3", 0), gone(2, "Two.mp3", 20), gone(3, "Three.mp3", 31), gone(4, "Four.mp3", 1)];
        let (matched, ambiguous, not_found) = plan_relocation(&missing, dir.path(), &HashSet::new());

        assert_eq!(
            matched,
            vec![(1, dir.path().join("deep/a/One.mp3")), (3, dir.path().join("y/Three.mp3"))]
        );
        assert_eq!(ambiguous, vec![2]);
        assert_eq!(not_found, vec![4]);
    }

    #[test]
    fn a_file_two_tracks_want_is_ambiguous_and_taken_paths_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let shared = dir.path().join("Same.mp3");
        let owned = dir.path().join("Owned.mp3");
        write(&shared, 5);
        write(&owned, 5);

        let missing = [gone(1, "Same.mp3", 0), gone(2, "Same.mp3", 0), gone(3, "Owned.mp3", 0)];
        let taken = HashSet::from([owned]);
        let (matched, ambiguous, not_found) = plan_relocation(&missing, dir.path(), &taken);

        assert!(matched.is_empty());
        assert_eq!(ambiguous, vec![1, 2]);
        assert_eq!(not_found, vec![3]);
    }
}
