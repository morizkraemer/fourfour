use anyhow::{Context, Result, bail};

use pioneer_usb_writer::models::{Beat, BeatGrid};

use crate::{LocalLibrary, UserEdits, queries};

pub const MIN_BPM: f64 = 40.0;
pub const MAX_BPM: f64 = 250.0;

pub fn is_camelot(key: &str) -> bool {
    let Some(letter) = key.chars().last() else { return false };
    let number = &key[..key.len() - letter.len_utf8()];
    matches!(letter, 'A' | 'B') && number.parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) && !number.starts_with('0')
}

impl LocalLibrary {
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

    /// Fit the beat grid to a new BPM: spacing is scaled around the first beat and every beat
    /// carries the new tempo. Separate from `set_user_bpm` so changing the BPM never moves the grid
    /// unasked. The BPM is stored as hand-set.
    pub fn rescale_beat_grid(&self, track_id: i64, new_bpm: f64) -> Result<()> {
        if !new_bpm.is_finite() || !(MIN_BPM..=MAX_BPM).contains(&new_bpm) {
            bail!("BPM {new_bpm} is outside {MIN_BPM} to {MAX_BPM}");
        }
        let mut analysis = self
            .get_analysis(track_id)?
            .context("Track has no analysis to rescale the beat grid of")?;
        let Some(first) = analysis.beat_grid.beats.first().copied() else {
            bail!("Track has no beat grid to rescale");
        };
        let new_tempo = (new_bpm * 100.0).round() as u32;
        let factor = first.tempo as f64 / new_tempo as f64;
        analysis.beat_grid = BeatGrid {
            beats: analysis
                .beat_grid
                .beats
                .iter()
                .map(|b| Beat {
                    bar_position: b.bar_position,
                    time_ms: first.time_ms + (b.time_ms.saturating_sub(first.time_ms) as f64 * factor).round() as u32,
                    tempo: new_tempo,
                })
                .collect(),
        };
        analysis.bpm = new_tempo as f64 / 100.0;
        let stages = queries::select_analysis_stages(&self.conn, track_id)?.unwrap_or(crate::analysis_stage::ALL);
        self.write_analysis(track_id, &analysis, stages)?;
        self.update_user_edits(track_id, |edits| {
            edits.bpm_locked = true;
            edits.grid_locked = true;
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
