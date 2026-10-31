use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::{Beat, BeatGrid};

use crate::rekordbox::ImportedMeta;
use crate::{LocalLibrary, UserEdits, queries};

pub const MIN_BPM: f64 = 40.0;
pub const MAX_BPM: f64 = 250.0;
pub const MAX_RATING: u8 = 5;

struct GridAnchor {
    stored_ms: f64,
    bar_position: u8,
    exact_ms: f64,
}

fn rescaled_beats(anchor: &GridAnchor, tempo: u32, duration_ms: f64) -> Vec<Beat> {
    let beat_ms = 6_000_000.0 / tempo as f64;
    let first_index = -((anchor.stored_ms / beat_ms).floor() as i64);
    let last_index = ((duration_ms - anchor.stored_ms) / beat_ms).floor() as i64;
    (first_index..=last_index.max(0))
        .map(|k| Beat {
            bar_position: ((anchor.bar_position as i64 - 1 + k).rem_euclid(4) + 1) as u8,
            time_ms: (anchor.stored_ms + k as f64 * beat_ms).round() as u32,
            tempo,
        })
        .collect()
}

pub fn is_camelot(key: &str) -> bool {
    let Some(letter) = key.chars().last() else { return false };
    let number = &key[..key.len() - letter.len_utf8()];
    matches!(letter, 'A' | 'B')
        && !number.is_empty()
        && number.bytes().all(|b| b.is_ascii_digit())
        && !number.starts_with('0')
        && number.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n))
}

/// What the track list shows from a track's sidecar files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackLabels {
    /// The hand-set rating over the imported one, as [`LocalLibrary::effective_rating`] gives it.
    pub rating: Option<u8>,
    /// The colour label imported from Rekordbox.
    pub color: Option<String>,
}

/// A label that changed on disk since the labels cache was written; `None` when the track has none any more.
pub type TrackLabelsChange = (i64, Option<TrackLabels>);

pub const LABELS_CACHE_FILE: &str = "track-labels.json";

#[derive(Serialize, Deserialize)]
struct LabelsCache {
    anlz_modified_ns: u128,
    labels: HashMap<i64, TrackLabels>,
}

fn modified_ns(path: &Path) -> Option<u128> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_nanos())
}

fn read_track_labels(track_dir: &Path) -> Option<TrackLabels> {
    let imported = ImportedMeta::read(track_dir);
    let rating = UserEdits::read(track_dir)
        .and_then(|edits| edits.rating)
        .or(imported.as_ref().and_then(|meta| meta.rating))
        .filter(|&r| r > 0);
    let color = imported.and_then(|meta| meta.color);
    (rating.is_some() || color.is_some()).then_some(TrackLabels { rating, color })
}

/// Reads [`TrackLabels`] from the sidecar files alone, so it can run on another thread while the library is queried.
///
/// Scanning every track folder costs about 230 ms at 50k tracks, so the last scan is kept in [`LABELS_CACHE_FILE`]
/// next to the database. The cache is trusted only while the `anlz` folder is unchanged (no track folder added or
/// removed); [`refresh`](Self::refresh) scans again after the list is on screen and reports what changed on disk.
pub struct TrackLabelsReader {
    anlz_dir: Option<PathBuf>,
}

impl TrackLabelsReader {
    /// [`TrackLabels`] of every track that has a rating or a colour, read on several threads.
    pub fn read_all(&self) -> HashMap<i64, TrackLabels> {
        let Some(anlz_dir) = &self.anlz_dir else { return HashMap::new() };
        let Ok(entries) = std::fs::read_dir(anlz_dir) else { return HashMap::new() };
        let ids: Vec<i64> = entries.filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok()).collect();
        crate::read_track_dirs(anlz_dir, &ids, read_track_labels)
    }

    /// The cached labels while the cache matches the `anlz` folder, else [`read_all`](Self::read_all), cached.
    pub fn read_cached(&self) -> HashMap<i64, TrackLabels> {
        if let Some(cache) = self.load_cache().filter(|cache| Some(cache.anlz_modified_ns) == self.anlz_modified_ns()) {
            return cache.labels;
        }
        let labels = self.read_all();
        self.store(&labels);
        labels
    }

    /// Scans every track folder, caches the result and returns the labels that differ from the cache. Each change is
    /// read once more before it is reported, so a rating set during the scan is not undone.
    pub fn refresh(&self) -> Vec<TrackLabelsChange> {
        let Some(anlz_dir) = &self.anlz_dir else { return Vec::new() };
        let mut fresh = self.read_all();
        let cached = self.load_cache().map(|cache| cache.labels).unwrap_or_default();
        let changed: Vec<i64> = fresh
            .iter()
            .filter(|(id, labels)| cached.get(id) != Some(labels))
            .map(|(id, _)| *id)
            .chain(cached.keys().filter(|id| !fresh.contains_key(id)).copied())
            .collect();
        let mut changes = Vec::new();
        for id in changed {
            let now = read_track_labels(&anlz_dir.join(id.to_string()));
            match &now {
                Some(labels) => fresh.insert(id, labels.clone()),
                None => fresh.remove(&id),
            };
            if cached.get(&id) != now.as_ref() {
                changes.push((id, now));
            }
        }
        self.store(&fresh);
        changes
    }

    /// Re-reads one track's labels into the cache, when there is one.
    pub fn update(&self, track_id: i64) {
        let (Some(anlz_dir), Some(mut cache)) = (&self.anlz_dir, self.load_cache()) else { return };
        match read_track_labels(&anlz_dir.join(track_id.to_string())) {
            Some(labels) => cache.labels.insert(track_id, labels),
            None => cache.labels.remove(&track_id),
        };
        self.write_cache(&cache);
    }

    /// Drops the cache, so the next start scans every track folder.
    pub fn invalidate(&self) {
        if let Some(path) = self.cache_path() {
            let _ = std::fs::remove_file(path);
        }
    }

    fn cache_path(&self) -> Option<PathBuf> {
        Some(self.anlz_dir.as_ref()?.parent()?.join(LABELS_CACHE_FILE))
    }

    fn anlz_modified_ns(&self) -> Option<u128> {
        modified_ns(self.anlz_dir.as_ref()?)
    }

    fn load_cache(&self) -> Option<LabelsCache> {
        serde_json::from_slice(&std::fs::read(self.cache_path()?).ok()?).ok()
    }

    fn store(&self, labels: &HashMap<i64, TrackLabels>) {
        let Some(anlz_modified_ns) = self.anlz_modified_ns() else { return };
        self.write_cache(&LabelsCache { anlz_modified_ns, labels: labels.clone() });
    }

    fn write_cache(&self, cache: &LabelsCache) {
        let Some(path) = self.cache_path() else { return };
        let tmp = path.with_extension("json.tmp");
        let written = serde_json::to_vec(cache)
            .map_err(anyhow::Error::from)
            .and_then(|bytes| Ok(std::fs::write(&tmp, bytes)?))
            .and_then(|()| Ok(std::fs::rename(&tmp, &path)?));
        if let Err(err) = written {
            eprintln!("track labels cache not written: {err:#}");
        }
    }
}

impl LocalLibrary {
    pub fn track_labels_reader(&self) -> TrackLabelsReader {
        TrackLabelsReader { anlz_dir: self.anlz_dir() }
    }

    pub fn user_edits(&self, track_id: i64) -> UserEdits {
        self.track_anlz_dir(track_id).and_then(|dir| UserEdits::read(&dir)).unwrap_or_default()
    }

    fn update_user_edits(&self, track_id: i64, change: impl FnOnce(&mut UserEdits)) -> Result<()> {
        let Some(dir) = self.track_anlz_dir(track_id) else { return Ok(()) };
        let mut edits = UserEdits::read(&dir).unwrap_or_default();
        change(&mut edits);
        edits.write(&dir)
    }

    /// Set the BPM of an analyzed track by hand. The beat grid is not touched.
    pub fn set_user_bpm(&self, track_id: i64, bpm: f64) -> Result<()> {
        if !bpm.is_finite() || !(MIN_BPM..=MAX_BPM).contains(&bpm) {
            bail!("BPM {bpm} is outside {MIN_BPM} to {MAX_BPM}");
        }
        let bpm = (bpm * 100.0).round() / 100.0;
        if !queries::update_analysis_bpm(&self.conn, track_id, bpm)? {
            bail!("Track has no analysis to set the BPM of");
        }
        self.update_user_edits(track_id, |edits| edits.bpm_locked = true)
    }

    /// Set the Camelot key ("8A", "12B") of an analyzed track by hand.
    pub fn set_user_key(&self, track_id: i64, key: &str) -> Result<()> {
        if !is_camelot(key) {
            bail!("{key:?} is not a Camelot key");
        }
        if !queries::update_analysis_key(&self.conn, track_id, key)? {
            bail!("Track has no analysis to set the key of");
        }
        self.update_user_edits(track_id, |edits| edits.key_locked = true)
    }

    /// Let re-analysis replace the hand-set BPM and the beat grid again; the key and the cues stay as they are.
    pub fn release_user_bpm(&self, track_id: i64) -> Result<()> {
        self.update_user_edits(track_id, |edits| {
            edits.bpm_locked = false;
            edits.grid_locked = false;
            edits.grid_offset_ms = None;
            edits.grid_anchor_ms = None;
        })
    }

    /// Let re-analysis replace the hand-set key again; the BPM, the grid and the cues stay as they are.
    pub fn release_user_key(&self, track_id: i64) -> Result<()> {
        self.update_user_edits(track_id, |edits| edits.key_locked = false)
    }

    /// Set the star rating (0 clears it). Shown over the imported rating.
    pub fn set_user_rating(&self, track_id: i64, rating: u8) -> Result<()> {
        if rating > MAX_RATING {
            bail!("Rating {rating} is above {MAX_RATING} stars");
        }
        self.update_user_edits(track_id, |edits| edits.rating = Some(rating))?;
        self.track_labels_reader().update(track_id);
        Ok(())
    }

    /// The hand-set rating when there is one, else the imported one; `None` when unrated.
    pub fn effective_rating(&self, track_id: i64, imported: Option<u8>) -> Option<u8> {
        self.user_edits(track_id).rating.or(imported).filter(|&r| r > 0)
    }

    /// Re-space the beat grid for a new BPM from its anchor downbeat ([`UserEdits::grid_anchor_ms`]; without one, the
    /// first downbeat), which stays exactly where it is (its sub-millisecond place in `grid_offset_ms` is untouched). Beats
    /// are laid out over the whole track and carry the new tempo; the BPM and the grid are stored as hand-set. The anchor
    /// is kept, so a downbeat the fill added at the track start is never the anchor of a later rescale. A track with no
    /// beat grid only gets the BPM.
    pub fn rescale_beat_grid(&self, track_id: i64, new_bpm: f64) -> Result<()> {
        if !new_bpm.is_finite() || !(MIN_BPM..=MAX_BPM).contains(&new_bpm) {
            bail!("BPM {new_bpm} is outside {MIN_BPM} to {MAX_BPM}");
        }
        let mut analysis = self
            .get_analysis(track_id)?
            .context("Track has no analysis to rescale the beat grid of")?;
        let edits = self.user_edits(track_id);
        let offset_ms = edits.grid_offset_ms.unwrap_or(0.0);
        let beats = &analysis.beat_grid.beats;
        let anchor = match edits.grid_anchor_ms {
            Some(exact) => Some(GridAnchor { stored_ms: (exact - offset_ms).round(), bar_position: 1, exact_ms: exact }),
            None => beats.iter().find(|b| b.bar_position == 1).or(beats.first()).map(|b| GridAnchor {
                stored_ms: b.time_ms as f64,
                bar_position: b.bar_position,
                exact_ms: b.time_ms as f64 + offset_ms,
            }),
        };
        let Some(anchor) = anchor else {
            return self.set_user_bpm(track_id, new_bpm);
        };
        let duration_ms = self.get_track(track_id)?.context("Track not found")?.duration_secs * 1000.0;
        let new_tempo = (new_bpm * 100.0).round() as u32;
        analysis.beat_grid = BeatGrid { beats: rescaled_beats(&anchor, new_tempo, duration_ms) };
        analysis.bpm = new_tempo as f64 / 100.0;
        let stages = queries::select_analysis_stages(&self.conn, track_id)?.unwrap_or(crate::analysis_stage::ALL);
        self.write_analysis(track_id, &analysis, stages)?;
        self.update_user_edits(track_id, |edits| {
            edits.bpm_locked = true;
            edits.grid_locked = true;
            if edits.grid_anchor_ms.is_none() && anchor.bar_position == 1 {
                edits.grid_anchor_ms = Some(anchor.exact_ms);
            }
        })
    }

    /// Re-analysis keeps a hand-set BPM and key.
    pub(crate) fn keep_user_edits(
        &self,
        track_id: i64,
        analysis: &pioneer_usb_writer::models::AnalysisResult,
    ) -> Result<pioneer_usb_writer::models::AnalysisResult> {
        let mut kept = analysis.clone();
        let edits = self.user_edits(track_id);
        if !edits.bpm_locked && !edits.key_locked {
            return Ok(kept);
        }
        if let Some((bpm, key, _)) = queries::select_analysis_index(&self.conn, track_id)? {
            if edits.bpm_locked {
                kept.bpm = bpm;
            }
            if edits.key_locked {
                kept.key = key;
            }
        }
        Ok(kept)
    }
}

#[cfg(test)]
mod tests {
    use super::is_camelot;

    #[test]
    fn camelot_keys_are_one_to_twelve_then_a_or_b() {
        for key in ["1A", "8A", "12B"] {
            assert!(is_camelot(key), "{key}");
        }
        for key in ["", "A", "0A", "08A", "13B", "8C", "+8A", "8a", " 8A"] {
            assert!(!is_camelot(key), "{key}");
        }
    }
}
