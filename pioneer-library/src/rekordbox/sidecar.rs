use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::Beat;

use super::model::{ImportedCue, SourceKind};

pub const SIDECAR_FILE: &str = "imported.json";

/// Imported data that has no home in the library schema. Its presence also
/// marks the beat grid and cues as user data that re-analysis must not replace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedMeta {
    pub source: SourceKind,
    pub source_id: String,
    pub grid_locked: bool,
    pub cues_locked: bool,
    pub rating: Option<u8>,
    pub color: Option<String>,
    pub cues: Vec<ImportedCue>,
    /// Imported beat grid for tracks that have no analysis to hold it yet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grid: Vec<Beat>,
}

impl ImportedMeta {
    pub fn read(track_anlz_dir: &Path) -> Option<Self> {
        let bytes = std::fs::read(track_anlz_dir.join(SIDECAR_FILE)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn write(&self, track_anlz_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(track_anlz_dir)
            .with_context(|| format!("Failed to create {}", track_anlz_dir.display()))?;
        std::fs::write(
            track_anlz_dir.join(SIDECAR_FILE),
            serde_json::to_vec_pretty(self)?,
        )
        .context("Failed to write imported.json")
    }
}
