use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::rekordbox::ImportedCue;

pub const EDITS_FILE: &str = "edits.json";
pub const HOT_CUE_SLOTS: u32 = 8;

/// What the user changed by hand. A locked grid or cue list is never replaced by re-analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserEdits {
    pub grid_locked: bool,
    pub cues_locked: bool,
    pub cues: Vec<ImportedCue>,
    #[serde(default)]
    pub bpm_locked: bool,
    #[serde(default)]
    pub key_locked: bool,
}

impl UserEdits {
    pub fn read(track_anlz_dir: &Path) -> Option<Self> {
        let bytes = std::fs::read(track_anlz_dir.join(EDITS_FILE)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn write(&self, track_anlz_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(track_anlz_dir)
            .with_context(|| format!("Failed to create {}", track_anlz_dir.display()))?;
        std::fs::write(track_anlz_dir.join(EDITS_FILE), serde_json::to_vec_pretty(self)?)
            .context("Failed to write edits.json")
    }
}

/// Validated cue list in storage order: hot cues A to H first, then memory cues by time.
pub fn normalize_cues(cues: &[ImportedCue]) -> Result<Vec<ImportedCue>> {
    let mut out = cues.to_vec();
    for cue in &out {
        if cue.hot_cue_number > HOT_CUE_SLOTS {
            bail!(
                "Hot cue slot {} is out of range (0 is a memory cue, 1 to {HOT_CUE_SLOTS} are A to H)",
                cue.hot_cue_number
            );
        }
        if cue.loop_time_ms.is_some_and(|end| end <= cue.time_ms) {
            bail!("Loop of the cue at {} ms ends before it starts", cue.time_ms);
        }
    }
    out.sort_by_key(|c| (c.hot_cue_number == 0, c.hot_cue_number, c.time_ms));
    for pair in out.windows(2) {
        let same_slot = pair[0].hot_cue_number == pair[1].hot_cue_number;
        if same_slot && pair[0].hot_cue_number > 0 {
            bail!("Hot cue slot {} is used twice", pair[0].hot_cue_number);
        }
        if same_slot && pair[0].time_ms == pair[1].time_ms {
            bail!("Two memory cues at {} ms", pair[0].time_ms);
        }
    }
    Ok(out)
}
