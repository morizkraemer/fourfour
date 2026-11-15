//! Likely duplicates of tracks about to be imported. The library has no audio fingerprint,
//! so a match is the same path, the same file (name and size), or the same normalised
//! artist and title with a duration within [`DURATION_TOLERANCE_SECS`].

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;
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
    let collapsed = if text.is_ascii() { collapse_ascii(text) } else { collapse_unicode(text) };
    match collapsed.strip_suffix(NEUTRAL_TITLE_SUFFIX) {
        Some(stripped) if !stripped.is_empty() => stripped.to_string(),
        _ => collapsed,
    }
}

fn collapse_unicode(text: &str) -> String {
    let folded: String = text
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// [`collapse_unicode`] for ASCII text, where normalisation changes nothing and case folding
/// is `to_ascii_lowercase`.
fn collapse_ascii(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut gap = false;
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(byte.to_ascii_lowercase() as char);
        } else {
            gap = true;
        }
    }
    out
}

/// Comparison form of a file path (NFC, lowercase), as the library stores it in `tracks.path_key`.
pub fn path_key(path: &Path) -> String {
    lowercase_nfc(&path.to_string_lossy())
}

/// Comparison form of a path's file name, as the library stores it in `tracks.file_name_key`; empty when the
/// path has no file name.
pub fn file_name_key(path: &Path) -> String {
    path.file_name().map(|name| lowercase_nfc(&name.to_string_lossy())).unwrap_or_default()
}

fn file_key(name_key: String, file_size: u64) -> Option<(String, u64)> {
    (file_size > 0 && !name_key.is_empty()).then_some((name_key, file_size))
}

fn lowercase_nfc(text: &str) -> String {
    if text.is_ascii() { text.to_ascii_lowercase() } else { text.nfc().collect::<String>().to_lowercase() }
}

fn tags_key(artist: &str, title: &str) -> Option<(String, String)> {
    folded_tags_key(normalise_tag(artist), normalise_tag(title))
}

fn folded_tags_key(artist: String, title: String) -> Option<(String, String)> {
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
    /// [`normalise_tag`] of `artist` and `title`, as the library stores them (`tracks.artist_key`, `title_key`).
    pub artist_key: String,
    pub title_key: String,
    /// [`path_key`] and [`file_name_key`] of `source_path` (`tracks.path_key`, `file_name_key`).
    pub path_key: String,
    pub file_name_key: String,
}

impl DuplicateCandidate {
    /// The candidate as the library stores `track` under `id`: the path goes through the
    /// database as lossy UTF-8.
    pub fn stored(id: i64, track: &Track) -> Self {
        Self {
            id: id as u32,
            source_path: PathBuf::from(track.source_path.to_string_lossy().into_owned()),
            file_size: track.file_size,
            artist: track.artist.clone(),
            title: track.title.clone(),
            duration_secs: track.duration_secs,
            artist_key: normalise_tag(&track.artist),
            title_key: normalise_tag(&track.title),
            path_key: path_key(&track.source_path),
            file_name_key: file_name_key(&track.source_path),
        }
    }

    /// The candidate with its path (and the path keys) replaced.
    pub fn with_source_path(self, source_path: PathBuf) -> Self {
        Self { path_key: path_key(&source_path), file_name_key: file_name_key(&source_path), source_path, ..self }
    }
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
            artist_key: normalise_tag(&track.artist),
            title_key: normalise_tag(&track.title),
            path_key: path_key(&track.source_path),
            file_name_key: file_name_key(&track.source_path),
        }
    }
}

struct CandidateKeys {
    id: u32,
    path: String,
    file: Option<(String, u64)>,
    tags: Option<(String, String)>,
    duration_secs: f64,
}

impl CandidateKeys {
    fn of(candidate: &DuplicateCandidate) -> Self {
        Self {
            id: candidate.id,
            path: candidate.path_key.clone(),
            file: file_key(candidate.file_name_key.clone(), candidate.file_size),
            tags: folded_tags_key(candidate.artist_key.clone(), candidate.title_key.clone()),
            duration_secs: candidate.duration_secs,
        }
    }
}

const KEYS_PER_THREAD_MIN: usize = 1024;
const KEY_THREADS_MAX: usize = 8;

/// The keys of every candidate, in library order. Building them is cheap now that the folded
/// keys are stored; large libraries are still split over a few threads.
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

/// Entries by key, each key's lowest id in `first` and the rest, in id order, in `more`: nearly
/// every key has one id, and those cost no list of their own.
#[derive(Debug)]
struct Slots<K, V> {
    first: HashMap<K, (u32, V)>,
    more: HashMap<K, Vec<(u32, V)>>,
}

impl<K: Hash + Eq, V: PartialEq> PartialEq for Slots<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.first == other.first && self.more == other.more
    }
}

impl<K, V> Default for Slots<K, V> {
    fn default() -> Self {
        Self { first: HashMap::new(), more: HashMap::new() }
    }
}

impl<K: Hash + Eq + Clone, V: Copy> Slots<K, V> {
    fn insert(&mut self, key: K, entry: (u32, V)) {
        let slot = match self.first.entry(key) {
            Entry::Vacant(vacant) => {
                vacant.insert(entry);
                return;
            }
            Entry::Occupied(slot) => slot,
        };
        let (lower, higher) = if entry.0 < slot.get().0 { (entry, *slot.get()) } else { (*slot.get(), entry) };
        let key = slot.key().clone();
        *slot.into_mut() = lower;
        let rest = self.more.entry(key).or_default();
        let at = rest.partition_point(|(id, _)| *id < higher.0);
        rest.insert(at, higher);
    }

    fn remove(&mut self, key: &K, id: u32) -> bool {
        let Some(first) = self.first.get(key) else { return false };
        if first.0 != id {
            let Some(rest) = self.more.get_mut(key) else { return false };
            let Ok(at) = rest.binary_search_by_key(&id, |(id, _)| *id) else { return false };
            rest.remove(at);
            if rest.is_empty() {
                self.more.remove(key);
            }
            return true;
        }
        match self.more.get_mut(key).map(|rest| rest.remove(0)) {
            Some(next) => {
                if let Some(slot) = self.first.get_mut(key) {
                    *slot = next;
                }
                if self.more.get(key).is_some_and(Vec::is_empty) {
                    self.more.remove(key);
                }
            }
            None => {
                self.first.remove(key);
            }
        }
        true
    }

    fn lowest(&self, key: &K) -> Option<&(u32, V)> {
        self.first.get(key)
    }

    fn all(&self, key: &K) -> impl Iterator<Item = &(u32, V)> {
        self.first.get(key).into_iter().chain(self.more.get(key).into_iter().flatten())
    }
}

/// The library's tracks, indexed for duplicate lookups. Ids are the library ids (`Track::id`).
/// A key shared by several tracks answers with the lowest id, as a fresh index over the
/// library in id order would. It can be kept current with [`Self::insert_all`] and
/// [`Self::remove`] instead of being rebuilt.
#[derive(Debug, Default, PartialEq)]
pub struct DuplicateIndex {
    count: usize,
    by_path: Slots<String, ()>,
    by_file: Slots<(String, u64), ()>,
    by_tags: Slots<(String, String), f64>,
}

impl DuplicateIndex {
    pub fn new(library: &[DuplicateCandidate]) -> Self {
        let mut index = Self::default();
        index.insert_all(library);
        index
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Adds tracks that are not in the index yet.
    pub fn insert_all(&mut self, candidates: &[DuplicateCandidate]) {
        self.by_path.first.reserve(candidates.len());
        self.by_file.first.reserve(candidates.len());
        self.by_tags.first.reserve(candidates.len());
        for keys in candidate_keys(candidates) {
            let id = keys.id;
            self.by_path.insert(keys.path, (id, ()));
            if let Some(key) = keys.file {
                self.by_file.insert(key, (id, ()));
            }
            if let Some(key) = keys.tags {
                self.by_tags.insert(key, (id, keys.duration_secs));
            }
        }
        self.count += candidates.len();
    }

    /// Takes a track out, given the values it was indexed with. Returns whether it was there.
    pub fn remove(&mut self, candidate: &DuplicateCandidate) -> bool {
        let keys = CandidateKeys::of(candidate);
        let removed = self.by_path.remove(&keys.path, keys.id);
        if let Some(key) = keys.file {
            self.by_file.remove(&key, keys.id);
        }
        if let Some(key) = keys.tags {
            self.by_tags.remove(&key, keys.id);
        }
        if removed {
            self.count -= 1;
        }
        removed
    }

    pub fn find(&self, track: &Track) -> Option<DuplicateOf> {
        let hit = |existing_id: u32, reason| Some(DuplicateOf { existing_id: existing_id as i64, reason });
        if let Some(&(id, ())) = self.by_path.lowest(&path_key(&track.source_path)) {
            return hit(id, DuplicateReason::SamePath);
        }
        if let Some(&(id, ())) = file_key(file_name_key(&track.source_path), track.file_size).and_then(|k| self.by_file.lowest(&k)) {
            return hit(id, DuplicateReason::SameFile);
        }
        let key = tags_key(&track.artist, &track.title)?;
        self.by_tags
            .all(&key)
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

    fn lcg_strings(count: usize, alphabet: &[char]) -> Vec<String> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 33) as usize
        };
        (0..count).map(|_| (0..next() % 24).map(|_| alphabet[next() % alphabet.len()]).collect()).collect()
    }

    #[test]
    fn ascii_fast_paths_match_the_unicode_paths() {
        let alphabet: Vec<char> = (0u8..128).map(char::from).collect();
        let mut texts = lcg_strings(20_000, &alphabet);
        texts.extend((0u8..128).map(|b| char::from(b).to_string()));
        texts.extend(["", " ", "  a  ", "A-B", "x (Original Mix)", " Original Mix", "a\tb\nc", "Track_01.MP3"].map(String::from));
        for text in &texts {
            assert_eq!(collapse_ascii(text), collapse_unicode(text), "{text:?}");
            assert_eq!(lowercase_nfc(text), text.nfc().collect::<String>().to_lowercase(), "{text:?}");
        }
    }

    #[test]
    fn non_ascii_text_still_goes_through_unicode_folding() {
        assert_eq!(normalise_tag("Beyoncé (Original Mix)"), "beyonce");
        assert_eq!(lowercase_nfc("Cafe\u{301} Ö.MP3"), "caf\u{e9} ö.mp3");
        let mixed: Vec<char> = "aZ09 -_()&é\u{301}ÖÅß\u{ff21}\u{3042}".chars().collect();
        for text in lcg_strings(5_000, &mixed) {
            let expected = text.nfkd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_lowercase).map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>();
            let expected = expected.split_whitespace().collect::<Vec<_>>().join(" ");
            let expected = match expected.strip_suffix(NEUTRAL_TITLE_SUFFIX) {
                Some(stripped) if !stripped.is_empty() => stripped.to_string(),
                _ => expected,
            };
            assert_eq!(normalise_tag(&text), expected, "{text:?}");
        }
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

    #[test]
    fn removing_the_lowest_id_of_a_shared_key_promotes_the_next_one() {
        let tracks: Vec<Track> = [(1, "/Music/a.mp3", "Hale", 380.0), (2, "/music/A.mp3", "HALE", 381.0), (3, "/MUSIC/a.MP3", "hale", 382.0)]
            .map(|(id, path, artist, secs)| track(id, path, artist, "Copper Lines", secs, 10))
            .into();
        let candidate = |i: usize| DuplicateCandidate::from(&tracks[i]);
        let mut index = index(&tracks);
        let by = |index: &DuplicateIndex, secs: f64| index.find(&track(0, "/x/y.mp3", "Hale", "Copper Lines", secs, 1)).map(|h| h.existing_id);
        let same_path = |index: &DuplicateIndex| index.find(&track(0, "/music/a.mp3", "", "", 0.0, 1)).map(|h| h.existing_id);
        assert_eq!((same_path(&index), by(&index, 382.5)), (Some(1), Some(2)));
        assert!(index.remove(&candidate(1)));
        assert_eq!((same_path(&index), by(&index, 382.5)), (Some(1), Some(3)));
        assert!(index.remove(&candidate(0)));
        assert_eq!((same_path(&index), by(&index, 380.5), by(&index, 385.0)), (Some(3), Some(3), None));
        index.insert_all(&[candidate(0)]);
        assert_eq!(same_path(&index), Some(1));
        assert!(index.remove(&candidate(2)) && index.remove(&candidate(0)));
        assert!(!index.remove(&candidate(0)));
        assert_eq!(index, DuplicateIndex::default());
    }

    #[test]
    fn a_track_rewritten_by_remove_and_insert_is_found_under_its_new_values_only() {
        let old = DuplicateCandidate::from(&track(1, "/m/a.mp3", "Hale", "Copper Lines", 380.0, 10));
        let mut index = DuplicateIndex::new(&[old.clone()]);
        assert!(index.remove(&old));
        index.insert_all(&[DuplicateCandidate::from(&track(1, "/m/b.mp3", "Ode", "Glass Harbor", 300.0, 20))]);
        assert_eq!(index.len(), 1);
        assert_eq!(index.find(&track(0, "/m/a.mp3", "Hale", "Copper Lines", 380.0, 10)), None);
        let moved = track(0, "/x/b.mp3", "ode", "glass harbor", 300.0, 20);
        assert_eq!(index.find(&moved), Some(DuplicateOf { existing_id: 1, reason: DuplicateReason::SameFile }));
    }
}
