use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pioneer_library::{DuplicateIndex, DuplicateReason, LocalLibrary};
use pioneer_usb_writer::models::Track;
use pioneer_usb_writer::scanner::AUDIO_EXTENSIONS;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DuplicateRow {
    pub new_path: String,
    pub file_name: String,
    pub existing_id: u32,
    pub title: String,
    pub artist: String,
    pub duration_secs: f64,
    pub reason: DuplicateReason,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImportPlan {
    pub token: u64,
    pub new_count: usize,
    pub already_in_library: usize,
    pub duplicates: Vec<DuplicateRow>,
    pub skipped: SkippedFiles,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct SkippedFiles {
    pub unsupported: usize,
    pub unsupported_exts: Vec<String>,
    pub unreadable: usize,
}

#[derive(Debug, Default)]
pub struct FileSurvey {
    audio: HashSet<PathBuf>,
    unsupported: usize,
    unsupported_exts: BTreeSet<String>,
}

pub fn survey_files(paths: &[String]) -> FileSurvey {
    let mut survey = FileSurvey::default();
    for p in paths {
        survey.visit(Path::new(p));
    }
    survey
}

impl FileSurvey {
    fn visit(&mut self, path: &Path) {
        if path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) {
            return;
        }
        if path.is_dir() {
            let Ok(entries) = std::fs::read_dir(path) else { return };
            for entry in entries.flatten() {
                let is_symlinked_dir = entry.file_type().is_ok_and(|t| t.is_symlink()) && entry.path().is_dir();
                if !is_symlinked_dir {
                    self.visit(&entry.path());
                }
            }
            return;
        }
        if !path.is_file() {
            return;
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            self.audio.insert(path.to_path_buf());
        } else {
            self.unsupported += 1;
            if !ext.is_empty() {
                self.unsupported_exts.insert(format!(".{ext}"));
            }
        }
    }
}

#[derive(Debug)]
pub struct PendingImport {
    token: u64,
    fresh: Vec<Track>,
    duplicate_of: Vec<Option<u32>>,
}

impl PendingImport {
    /// The tracks to add and the existing ids of the duplicates left out.
    pub fn resolve(self, skip_duplicates: bool) -> (Vec<Track>, Vec<u32>) {
        if !skip_duplicates {
            return (self.fresh, Vec::new());
        }
        let mut add = Vec::new();
        let mut skipped = Vec::new();
        for (track, dup) in self.fresh.into_iter().zip(self.duplicate_of) {
            match dup {
                Some(id) => skipped.push(id),
                None => add.push(track),
            }
        }
        (add, skipped)
    }
}

/// The import waiting on the duplicates question; a newer plan replaces it.
#[derive(Debug, Default)]
pub struct ImportPlans(Mutex<(u64, Option<PendingImport>)>);

impl ImportPlans {
    pub fn store(&self, build: impl FnOnce(u64) -> (PendingImport, ImportPlan)) -> Result<ImportPlan, String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        state.0 += 1;
        let (pending, plan) = build(state.0);
        state.1 = Some(pending);
        Ok(plan)
    }

    pub fn take(&self, token: u64) -> Result<PendingImport, String> {
        let mut state = self.0.lock().map_err(|e| e.to_string())?;
        match state.1.take() {
            Some(pending) if pending.token == token => Ok(pending),
            other => {
                state.1 = other;
                Err("This import is no longer waiting. Import the files again.".into())
            }
        }
    }
}

/// Splits scanned tracks into ones already in the library (same path, dropped silently),
/// likely duplicates (asked about) and new tracks.
pub fn plan_import(token: u64, library: &[Track], scanned: Vec<Track>, survey: &FileSurvey) -> (PendingImport, ImportPlan) {
    let index = DuplicateIndex::new(library);
    let by_id: HashMap<u32, &Track> = library.iter().map(|t| (t.id, t)).collect();
    let mut seen = HashSet::new();
    let mut already_in_library = 0;
    let mut fresh = Vec::new();
    let mut duplicate_of = Vec::new();
    let mut duplicates = Vec::new();
    for track in scanned {
        if !seen.insert(track.source_path.clone()) {
            continue;
        }
        let found = index.find(&track);
        match found {
            Some(hit) if hit.reason == DuplicateReason::SamePath => {
                already_in_library += 1;
                continue;
            }
            Some(hit) => {
                let existing_id = hit.existing_id as u32;
                let existing = by_id.get(&existing_id);
                duplicates.push(DuplicateRow {
                    new_path: track.source_path.to_string_lossy().into_owned(),
                    file_name: track.source_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    existing_id,
                    title: existing.map(|t| t.title.clone()).unwrap_or_default(),
                    artist: existing.map(|t| t.artist.clone()).unwrap_or_default(),
                    duration_secs: existing.map(|t| t.duration_secs).unwrap_or(track.duration_secs),
                    reason: hit.reason,
                });
                duplicate_of.push(Some(existing_id));
            }
            None => duplicate_of.push(None),
        }
        fresh.push(track);
    }
    let skipped = SkippedFiles {
        unsupported: survey.unsupported,
        unsupported_exts: survey.unsupported_exts.iter().cloned().collect(),
        unreadable: survey.audio.len().saturating_sub(seen.len()),
    };
    let plan = ImportPlan { token, new_count: fresh.len() - duplicates.len(), already_in_library, duplicates, skipped };
    (PendingImport { token, fresh, duplicate_of }, plan)
}

pub fn plan_for_library(
    plans: &ImportPlans,
    lib: &LocalLibrary,
    scanned: Vec<Track>,
    survey: &FileSurvey,
) -> Result<ImportPlan, String> {
    let library = lib.get_all_tracks().map_err(|e| e.to_string())?;
    plans.store(|token| plan_import(token, &library, scanned, survey))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn track(id: u32, path: &str, artist: &str, title: &str, secs: f64) -> Track {
        Track {
            id,
            source_path: PathBuf::from(path),
            usb_path: String::new(),
            title: title.into(),
            artist: artist.into(),
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
            duration_secs: secs,
            sample_rate: 44100,
            bitrate: 320,
            file_size: 1000 + id as u64,
            artwork: None,
        }
    }

    fn sample() -> (PendingImport, ImportPlan) {
        let library = vec![track(7, "/Music/Glass Harbor.aiff", "Ode & Ash", "Glass Harbor", 425.0)];
        let scanned = vec![
            track(0, "/Music/Glass Harbor.aiff", "Ode & Ash", "Glass Harbor", 425.0),
            track(0, "/New/glass_harbor.mp3", "Ode & Ash", "Glass Harbor (Original Mix)", 426.0),
            track(0, "/New/fresh.mp3", "Hale", "Copper Lines", 380.0),
            track(0, "/New/fresh.mp3", "Hale", "Copper Lines", 380.0),
        ];
        plan_import(3, &library, scanned, &FileSurvey::default())
    }

    #[test]
    fn plan_drops_same_path_and_lists_likely_duplicates_with_your_track() {
        let (_, plan) = sample();
        assert_eq!(plan.token, 3);
        assert_eq!(plan.already_in_library, 1);
        assert_eq!(plan.new_count, 1);
        assert_eq!(
            plan.duplicates,
            vec![DuplicateRow {
                new_path: "/New/glass_harbor.mp3".into(),
                file_name: "glass_harbor.mp3".into(),
                existing_id: 7,
                title: "Glass Harbor".into(),
                artist: "Ode & Ash".into(),
                duration_secs: 425.0,
                reason: DuplicateReason::SameTags,
            }]
        );
    }

    #[test]
    fn skip_duplicates_adds_only_new_tracks_and_reports_the_existing_ids() {
        let (pending, _) = sample();
        let (add, skipped) = pending.resolve(true);
        assert_eq!(add.iter().map(|t| t.source_path.to_str().unwrap()).collect::<Vec<_>>(), ["/New/fresh.mp3"]);
        assert_eq!(skipped, [7]);
    }

    #[test]
    fn import_anyway_adds_duplicates_too() {
        let (pending, _) = sample();
        let (add, skipped) = pending.resolve(false);
        assert_eq!(add.len(), 2);
        assert!(skipped.is_empty());
    }

    #[test]
    fn only_the_latest_plan_can_be_committed() {
        let plans = ImportPlans::default();
        let first = plans.store(|token| plan_import(token, &[], vec![], &FileSurvey::default())).unwrap();
        let second = plans.store(|token| plan_import(token, &[], vec![], &FileSurvey::default())).unwrap();
        assert!(plans.take(first.token).is_err());
        assert!(plans.take(second.token).is_ok());
        assert!(plans.take(second.token).is_err());
    }

    #[test]
    fn survey_counts_unsupported_and_unreadable_files_and_ignores_hidden_ones() {
        let dir = std::env::temp_dir().join(format!("ff-survey-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for name in ["a.mp3", "b.MP3", "notes.txt", "sub/c.m4p", "sub/d.txt", ".DS_Store", "README"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let survey = survey_files(&[dir.to_string_lossy().into_owned()]);
        let scanned = vec![track(0, &dir.join("a.mp3").to_string_lossy(), "", "a", 1.0)];
        let (_, plan) = plan_import(1, &[], scanned, &survey);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            plan.skipped,
            SkippedFiles { unsupported: 4, unsupported_exts: vec![".m4p".into(), ".txt".into()], unreadable: 1 }
        );
    }
}
