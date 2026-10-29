//! Likely duplicates of tracks about to be imported. The library has no audio fingerprint,
//! so a match is the same path, the same file (name and size), or the same normalised
//! artist and title with a duration within [`DURATION_TOLERANCE_SECS`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

fn tags_key(artist: &str, title: &str) -> Option<(String, String)> {
    let artist = normalise_tag(artist);
    let title = normalise_tag(title);
    (!artist.is_empty() && artist != PLACEHOLDER_ARTIST && !title.is_empty()).then_some((artist, title))
}

pub fn durations_match(a: f64, b: f64) -> bool {
    a > 0.0 && b > 0.0 && (a - b).abs() <= DURATION_TOLERANCE_SECS
}

/// The fields of a library track that duplicate detection compares and the import dialog shows.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateCandidate {
    pub id: u32,
    pub source_path: PathBuf,
    pub file_size: u64,
    pub artist: String,
    pub title: String,
    pub duration_secs: f64,
}

impl From<&Track> for DuplicateCandidate {
    fn from(track: &Track) -> Self {
        Self {
            id: track.id,
            source_path: track.source_path.clone(),
            file_size: track.file_size,
            artist: track.artist.clone(),
            title: track.title.clone(),
            duration_secs: track.duration_secs,
        }
    }
}

struct CandidateKeys {
    id: i64,
    path: String,
    file: Option<(String, u64)>,
    tags: Option<(String, String)>,
    duration_secs: f64,
}

impl CandidateKeys {
    fn of(candidate: &DuplicateCandidate) -> Self {
        Self {
            id: candidate.id as i64,
            path: path_key(&candidate.source_path),
            file: file_key(&candidate.source_path, candidate.file_size),
            tags: tags_key(&candidate.artist, &candidate.title),
            duration_secs: candidate.duration_secs,
        }
    }
}

const KEYS_PER_THREAD_MIN: usize = 1024;
const KEY_THREADS_MAX: usize = 8;

/// The keys of every candidate, in library order. The Unicode folding is the cost, so large
/// libraries are split over a few threads.
fn candidate_keys(library: &[DuplicateCandidate]) -> Vec<CandidateKeys> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(KEY_THREADS_MAX);
    let chunk = library.len().div_ceil(threads).max(KEYS_PER_THREAD_MIN);
    if library.len() <= chunk {
        return library.iter().map(CandidateKeys::of).collect();
    }
    std::thread::scope(|scope| {
        let parts: Vec<_> = library
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(CandidateKeys::of).collect::<Vec<_>>()))
            .collect();
        parts.into_iter().flat_map(|part| part.join().expect("duplicate key thread panicked")).collect()
    })
}

/// The library's tracks, indexed for duplicate lookups. Ids are the library ids (`Track::id`).
#[derive(Debug, Default)]
pub struct DuplicateIndex {
    by_path: HashMap<String, i64>,
    by_file: HashMap<(String, u64), i64>,
    by_tags: HashMap<(String, String), Vec<(i64, f64)>>,
}

impl DuplicateIndex {
    pub fn new(library: &[DuplicateCandidate]) -> Self {
        let mut index = Self::default();
        for keys in candidate_keys(library) {
            index.by_path.entry(keys.path).or_insert(keys.id);
            if let Some(key) = keys.file {
                index.by_file.entry(key).or_insert(keys.id);
            }
            if let Some(key) = keys.tags {
                index.by_tags.entry(key).or_default().push((keys.id, keys.duration_secs));
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
        let candidates = tags_key(&track.artist, &track.title).and_then(|k| self.by_tags.get(&k))?;
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

    fn index(tracks: &[Track]) -> DuplicateIndex {
        DuplicateIndex::new(&tracks.iter().map(DuplicateCandidate::from).collect::<Vec<_>>())
    }

    fn library() -> DuplicateIndex {
        index(&[
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

    #[test]
    fn a_library_split_over_threads_keeps_the_first_track_of_each_key() {
        let mut tracks: Vec<Track> = (1..=5000)
            .map(|i| track(i, &format!("/Music/{i}.mp3"), "Artist", &format!("Title {i}"), 300.0, 10_000 + i as u64))
            .collect();
        tracks.push(track(5001, "/MUSIC/1.mp3", "Other", "Other", 10.0, 1));
        tracks.push(track(5002, "/Copies/2.mp3", "", "", 0.0, 10_002));
        tracks.push(track(5003, "/Copies/t3.mp3", "Artist", "Title 3", 300.5, 2));
        let index = index(&tracks);
        let probe = |path: &str, artist: &str, title: &str, secs: f64, size: u64| index.find(&track(0, path, artist, title, secs, size));
        let found = |existing_id, reason| Some(DuplicateOf { existing_id, reason });
        assert_eq!(probe("/music/1.mp3", "", "", 0.0, 3), found(1, DuplicateReason::SamePath));
        assert_eq!(probe("/New/2.mp3", "", "", 0.0, 10_002), found(2, DuplicateReason::SameFile));
        assert_eq!(probe("/New/t3.wav", "artist", "title 3", 300.2, 7), found(3, DuplicateReason::SameTags));
        assert_eq!(probe("/New/t5000.wav", "Artist", "Title 5000", 301.0, 7), found(5000, DuplicateReason::SameTags));
        assert_eq!(probe("/New/none.wav", "Artist", "Title 5001", 300.0, 7), None);
    }
}
