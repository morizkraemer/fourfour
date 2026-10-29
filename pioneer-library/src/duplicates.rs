//! Likely duplicates of tracks about to be imported. The library has no audio fingerprint,
//! so a match is the same path, the same file (name and size), or the same normalised
//! artist and title with a duration within [`DURATION_TOLERANCE_SECS`].

use std::collections::HashMap;
use std::path::Path;

use pioneer_usb_writer::models::Track;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

pub const DURATION_TOLERANCE_SECS: f64 = 2.0;

const PLACEHOLDER_ARTIST: &str = "unknown artist";
const NEUTRAL_TITLE_SUFFIX: &str = " original mix";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateReason {
    SamePath,
    SameFile,
    SameTags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateOf {
    pub existing_id: i64,
    pub reason: DuplicateReason,
}

/// Comparison form of an artist or title: accents and case folded, punctuation dropped,
/// whitespace collapsed, and a trailing "(Original Mix)" removed.
pub fn normalise_tag(text: &str) -> String {
    let folded: String = text
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let collapsed = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.strip_suffix(NEUTRAL_TITLE_SUFFIX) {
        Some(stripped) if !stripped.is_empty() => stripped.to_string(),
        _ => collapsed,
    }
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().nfc().collect::<String>().to_lowercase()
}

fn file_key(path: &Path, file_size: u64) -> Option<(String, u64)> {
    let name = path.file_name()?.to_string_lossy().nfc().collect::<String>().to_lowercase();
    (file_size > 0).then_some((name, file_size))
}

fn tags_key(track: &Track) -> Option<(String, String)> {
    let artist = normalise_tag(&track.artist);
    let title = normalise_tag(&track.title);
    (!artist.is_empty() && artist != PLACEHOLDER_ARTIST && !title.is_empty()).then_some((artist, title))
}

pub fn durations_match(a: f64, b: f64) -> bool {
    a > 0.0 && b > 0.0 && (a - b).abs() <= DURATION_TOLERANCE_SECS
}

/// The library's tracks, indexed for duplicate lookups. Ids are the library ids (`Track::id`).
#[derive(Debug, Default)]
pub struct DuplicateIndex {
    by_path: HashMap<String, i64>,
    by_file: HashMap<(String, u64), i64>,
    by_tags: HashMap<(String, String), Vec<(i64, f64)>>,
}

impl DuplicateIndex {
    pub fn new(library: &[Track]) -> Self {
        let mut index = Self::default();
        for track in library {
            let id = track.id as i64;
            index.by_path.entry(path_key(&track.source_path)).or_insert(id);
            if let Some(key) = file_key(&track.source_path, track.file_size) {
                index.by_file.entry(key).or_insert(id);
            }
            if let Some(key) = tags_key(track) {
                index.by_tags.entry(key).or_default().push((id, track.duration_secs));
            }
        }
        index
    }

    pub fn find(&self, track: &Track) -> Option<DuplicateOf> {
        let hit = |existing_id, reason| Some(DuplicateOf { existing_id, reason });
        if let Some(&id) = self.by_path.get(&path_key(&track.source_path)) {
            return hit(id, DuplicateReason::SamePath);
        }
        if let Some(&id) = file_key(&track.source_path, track.file_size).and_then(|k| self.by_file.get(&k)) {
            return hit(id, DuplicateReason::SameFile);
        }
        let candidates = tags_key(track).and_then(|k| self.by_tags.get(&k))?;
        candidates
            .iter()
            .find(|(_, duration)| durations_match(*duration, track.duration_secs))
            .and_then(|&(id, _)| hit(id, DuplicateReason::SameTags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::make_track;

    fn track(id: u32, path: &str, artist: &str, title: &str, secs: f64, size: u64) -> Track {
        Track {
            id,
            artist: artist.into(),
            duration_secs: secs,
            file_size: size,
            ..make_track(title, path)
        }
    }

    fn library() -> DuplicateIndex {
        DuplicateIndex::new(&[
            track(1, "/Music/Ode & Ash - Glass Harbor.aiff", "Ode & Ash", "Glass Harbor", 425.0, 70_000_000),
            track(2, "/Music/Copper Lines.flac", "Hale", "Copper Lines", 380.0, 40_000_000),
            track(3, "/Music/Intro.mp3", "Unknown Artist", "Intro", 60.0, 1_000_000),
        ])
    }

    #[test]
    fn normalise_folds_case_accents_punctuation_and_original_mix() {
        assert_eq!(normalise_tag("  Glass   Harbor (Original Mix) "), "glass harbor");
        assert_eq!(normalise_tag("Beyoncé"), "beyonce");
        assert_eq!(normalise_tag("Ode & Ash"), "ode ash");
        assert_eq!(normalise_tag("ODE/ASH"), "ode ash");
        assert_eq!(normalise_tag("Original Mix"), "original mix");
        assert_eq!(normalise_tag("Glass Harbor (Extended Mix)"), "glass harbor extended mix");
    }

    #[test]
    fn same_tags_match_within_two_seconds_only() {
        let index = library();
        let close = track(0, "/Downloads/ode_ash-glass_harbor-320.mp3", "ODE & ASH", "Glass Harbor (Original Mix)", 427.0, 9_000_000);
        assert_eq!(index.find(&close), Some(DuplicateOf { existing_id: 1, reason: DuplicateReason::SameTags }));
        let far = Track { duration_secs: 427.5, ..close.clone() };
        assert_eq!(index.find(&far), None);
        let other_title = track(0, "/Downloads/x.mp3", "Ode & Ash", "Harbor Light", 425.0, 9_000_000);
        assert_eq!(index.find(&other_title), None);
    }

    #[test]
    fn unknown_or_empty_artist_and_zero_duration_never_match_on_tags() {
        let index = library();
        assert_eq!(index.find(&track(0, "/New/Intro.wav", "Unknown Artist", "Intro", 60.0, 5)), None);
        assert_eq!(index.find(&track(0, "/New/a.wav", "", "Copper Lines", 380.0, 5)), None);
        assert_eq!(index.find(&track(0, "/New/b.wav", "Hale", "Copper Lines", 0.0, 5)), None);
    }

    #[test]
    fn same_path_matches_ignoring_case() {
        let index = library();
        let again = track(0, "/music/copper lines.FLAC", "Someone", "Else", 1.0, 1);
        assert_eq!(index.find(&again), Some(DuplicateOf { existing_id: 2, reason: DuplicateReason::SamePath }));
    }

    #[test]
    fn same_file_name_and_size_match_in_another_folder() {
        let index = library();
        let copy = track(0, "/Backup/copper lines.flac", "", "", 0.0, 40_000_000);
        assert_eq!(index.find(&copy), Some(DuplicateOf { existing_id: 2, reason: DuplicateReason::SameFile }));
        let other_size = Track { file_size: 40_000_001, ..copy.clone() };
        assert_eq!(index.find(&other_size), None);
        let unknown_size = Track { file_size: 0, ..copy };
        assert_eq!(index.find(&unknown_size), None);
    }
}
