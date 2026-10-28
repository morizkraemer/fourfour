use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::{Beat, Track};

use super::remap::PathRemap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    MasterDb,
    Usb,
    Xml,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedCue {
    /// 0 = memory cue, 1 = hot cue A, 2 = hot cue B, ...
    pub hot_cue_number: u32,
    pub time_ms: u32,
    pub loop_time_ms: Option<u32>,
    pub color: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct SourceTrack {
    pub source_id: String,
    /// Path exactly as the source stores it.
    pub path: String,
    pub track: Track,
    pub rating: Option<u8>,
    pub color: Option<String>,
    pub cues: Vec<ImportedCue>,
    pub anlz_dat: Option<PathBuf>,
    pub grid: Option<Vec<Beat>>,
    pub artwork: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SourcePlaylist {
    pub source_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub is_folder: bool,
    pub sequence: i64,
    pub track_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SourceLibrary {
    pub kind: SourceKind,
    pub tracks: Vec<SourceTrack>,
    pub playlists: Vec<SourcePlaylist>,
    /// Remap implied by the source itself (USB root, Rekordbox drive swap).
    pub implied_remap: Option<PathRemap>,
    pub skipped_smart_playlists: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportOptions {
    /// "Old location -> new folder" rules; the first matching rule wins. An empty
    /// `from_prefix` stands for the detected common root. Empty list keeps the
    /// source's own paths (or its implied remap).
    #[serde(default)]
    pub remaps: Vec<PathRemap>,
    /// Import tracks whose audio file cannot be found.
    pub include_missing: bool,
}

/// How many tracks one remap rule caught and how many of their files exist.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemapStat {
    pub from_prefix: String,
    pub to_prefix: String,
    pub tracks: u32,
    pub files_found: u32,
    pub files_missing: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub source: SourceKind,
    pub tracks: u32,
    pub playlists: u32,
    pub folders: u32,
    pub skipped_smart_playlists: u32,
    pub hot_cues: u32,
    pub memory_cues: u32,
    pub tracks_with_grid: u32,
    pub detected_root: String,
    /// One entry per remap rule, in rule order.
    pub remap_stats: Vec<RemapStat>,
    /// Tracks no rule matched (they keep their path or the source's implied remap).
    pub unmapped: RemapStat,
    pub files_found: u32,
    pub files_missing: u32,
    pub missing_examples: Vec<String>,
    /// Tracks that match an existing library track by path or fingerprint.
    pub conflicts: u32,
    pub new_tracks: u32,
    pub playlist_conflicts: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RekordboxImportReport {
    pub tracks_imported: u32,
    pub tracks_merged: u32,
    pub tracks_skipped_missing: u32,
    pub files_found: u32,
    pub files_missing: u32,
    pub playlists_imported: u32,
    pub playlists_merged: u32,
    pub grids_imported: u32,
    pub hot_cues_imported: u32,
    pub memory_cues_imported: u32,
    pub cues_without_analysis: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ImportProgress {
    pub stage: &'static str,
    pub done: u32,
    pub total: u32,
}
