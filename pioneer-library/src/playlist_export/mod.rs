//! Playlist export to files other tools read: M3U8, CSV, plain text, Traktor NML and Rekordbox XML.
//! Lists only, no audio is copied.

mod lists;
mod rekordbox_xml;
mod traktor_nml;
mod xml;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::{Beat, Playlist, Track};

use crate::LocalLibrary;
use crate::rekordbox::ImportedCue;

const MISSING_EXAMPLES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    M3u8,
    Csv,
    Text,
    TraktorNml,
    RekordboxXml,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::M3u8 => "m3u8",
            Self::Csv => "csv",
            Self::Text => "txt",
            Self::TraktorNml => "nml",
            Self::RekordboxXml => "xml",
        }
    }

    pub fn supports_folders(self) -> bool {
        matches!(self, Self::TraktorNml | Self::RekordboxXml)
    }
}

/// A track with everything the formats carry, as the library holds it today.
#[derive(Debug, Clone)]
pub struct ExportTrack {
    pub id: i64,
    pub track: Track,
    pub bpm: f64,
    pub key: String,
    /// Absolute path of the audio file.
    pub path: String,
    pub missing: bool,
    pub grid: Vec<Beat>,
    pub cues: Vec<ImportedCue>,
    pub rating: Option<u8>,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ExportNode {
    Playlist { name: String, track_ids: Vec<i64> },
    Folder { name: String, children: Vec<ExportNode> },
}

impl ExportNode {
    pub fn name(&self) -> &str {
        match self {
            Self::Playlist { name, .. } | Self::Folder { name, .. } => name,
        }
    }
}

/// What gets exported: one playlist or a folder tree, with the tracks they use in first-seen order.
#[derive(Debug, Clone)]
pub struct ExportDocument {
    pub root: ExportNode,
    pub tracks: Vec<ExportTrack>,
}

impl ExportDocument {
    pub fn name(&self) -> &str {
        self.root.name()
    }

    pub fn track(&self, id: i64) -> Option<&ExportTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    fn list_tracks(&self) -> Result<Vec<&ExportTrack>> {
        match &self.root {
            ExportNode::Playlist { track_ids, .. } => {
                let by_id: HashMap<i64, &ExportTrack> = self.tracks.iter().map(|t| (t.id, t)).collect();
                Ok(track_ids.iter().filter_map(|id| by_id.get(id).copied()).collect())
            }
            ExportNode::Folder { name, .. } => {
                bail!("\u{201c}{name}\u{201d} is a folder; folders export as Rekordbox XML or Traktor NML")
            }
        }
    }

    pub fn render(&self, format: ExportFormat) -> Result<String> {
        Ok(match format {
            ExportFormat::M3u8 => lists::m3u8(&self.list_tracks()?),
            ExportFormat::Csv => lists::csv(&self.list_tracks()?),
            ExportFormat::Text => lists::text(&self.list_tracks()?),
            ExportFormat::TraktorNml => traktor_nml::render(self),
            ExportFormat::RekordboxXml => rekordbox_xml::render(self),
        })
    }

    pub fn report(&self) -> ExportReport {
        let missing: Vec<&ExportTrack> = self.tracks.iter().filter(|t| t.missing).collect();
        ExportReport {
            written: self.tracks.len() as u32,
            missing: missing.len() as u32,
            missing_examples: missing.iter().take(MISSING_EXAMPLES).map(|t| t.path.clone()).collect(),
        }
    }
}

/// `written` counts the tracks in the file (a folder counts each track once), `missing` those among them whose
/// audio file is not on disk; they are still listed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportReport {
    pub written: u32,
    pub missing: u32,
    pub missing_examples: Vec<String>,
}

/// `<playlist> <YYYY-MM-DD>.<ext>`, with characters file systems refuse replaced by `_`.
pub fn default_file_name(playlist_name: &str, date: NaiveDate, format: ExportFormat) -> String {
    let cleaned: String = playlist_name
        .chars()
        .map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    let stem = if cleaned.is_empty() { "Playlist" } else { cleaned };
    format!("{stem} {}.{}", date.format("%Y-%m-%d"), format.extension())
}

fn collect_node(
    playlists: &[Playlist],
    playlist: &Playlist,
    seen: &mut HashSet<u32>,
    track_ids: &mut Vec<i64>,
) -> ExportNode {
    if !playlist.is_folder {
        let ids: Vec<i64> = playlist.track_ids.iter().map(|&id| id as i64).collect();
        for &id in &ids {
            if !track_ids.contains(&id) {
                track_ids.push(id);
            }
        }
        return ExportNode::Playlist { name: playlist.name.clone(), track_ids: ids };
    }
    seen.insert(playlist.id);
    let inside: Vec<&Playlist> =
        playlists.iter().filter(|p| p.parent_id == playlist.id && !seen.contains(&p.id)).collect();
    let children = inside.into_iter().map(|child| collect_node(playlists, child, seen, track_ids)).collect();
    ExportNode::Folder { name: playlist.name.clone(), children }
}

fn absolute_path(path: &Path) -> String {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()).to_string_lossy().into_owned()
}

impl LocalLibrary {
    /// Gathers a playlist, or a folder with its playlists, for export.
    pub fn export_document(&self, playlist_id: i64) -> Result<ExportDocument> {
        let playlists = self.get_all_playlists()?;
        let start = playlists
            .iter()
            .find(|p| p.id as i64 == playlist_id)
            .with_context(|| format!("Playlist {playlist_id} does not exist"))?;
        let mut track_ids = Vec::new();
        let root = collect_node(&playlists, start, &mut HashSet::new(), &mut track_ids);
        let mut tracks = Vec::with_capacity(track_ids.len());
        for id in track_ids {
            if let Some(track) = self.get_track(id)? {
                tracks.push(self.export_track(id, track)?);
            }
        }
        Ok(ExportDocument { root, tracks })
    }

    fn export_track(&self, id: i64, track: Track) -> Result<ExportTrack> {
        let analysis = self.get_analysis(id)?;
        let meta = self.imported_meta(id);
        let (bpm, key, mut grid, cues) = match &analysis {
            Some(a) => (a.bpm, a.key.clone(), a.beat_grid.beats.clone(), self.get_cues_with_analysis(id, a)),
            None => (0.0, String::new(), Vec::new(), self.get_cues(id)?),
        };
        if grid.is_empty() {
            grid = meta.as_ref().map(|m| m.grid.clone()).unwrap_or_default();
        }
        Ok(ExportTrack {
            id,
            bpm: if bpm > 0.0 { bpm } else { track.tempo as f64 / 100.0 },
            key: if key.is_empty() { track.key.clone() } else { key },
            path: absolute_path(&track.source_path),
            missing: !track.source_path.exists(),
            grid,
            cues,
            rating: self.effective_rating(id, meta.as_ref().and_then(|m| m.rating)),
            color: meta.and_then(|m| m.color),
            track,
        })
    }

    /// Writes the playlist (or, for Rekordbox XML and Traktor NML, the folder) to `dest`.
    pub fn export_playlist(&self, playlist_id: i64, format: ExportFormat, dest: &Path) -> Result<ExportReport> {
        let document = self.export_document(playlist_id)?;
        let text = document.render(format)?;
        std::fs::write(dest, text).with_context(|| format!("Failed to write {}", dest.display()))?;
        Ok(document.report())
    }

    /// The default name offered in the save dialog, dated today.
    pub fn export_file_name(&self, playlist_id: i64, format: ExportFormat) -> Result<String> {
        let playlist = self
            .get_playlist(playlist_id)?
            .with_context(|| format!("Playlist {playlist_id} does not exist"))?;
        Ok(default_file_name(&playlist.name, chrono::Local::now().date_naive(), format))
    }

    /// "Artist - Title" per line, for the clipboard.
    pub fn playlist_tracklist(&self, playlist_id: i64) -> Result<String> {
        self.export_document(playlist_id)?.render(ExportFormat::Text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_is_playlist_and_date_with_extension() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        assert_eq!(default_file_name("Friday Warmup", date, ExportFormat::M3u8), "Friday Warmup 2026-11-21.m3u8");
        assert_eq!(default_file_name("Set", date, ExportFormat::RekordboxXml), "Set 2026-11-21.xml");
    }

    #[test]
    fn file_name_replaces_characters_file_systems_refuse() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        assert_eq!(default_file_name("A/B: C?", date, ExportFormat::Csv), "A_B_ C_ 2026-01-02.csv");
        assert_eq!(default_file_name("  ..  ", date, ExportFormat::Text), "Playlist 2026-01-02.txt");
    }
}
