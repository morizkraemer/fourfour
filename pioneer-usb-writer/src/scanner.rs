use anyhow::{Context, Result};
use lofty::prelude::*;
use lofty::probe::Probe;
use std::path::Path;
use walkdir::WalkDir;

use crate::models::Track;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "aiff", "aif", "m4a", "aac"];

/// Scan a directory for audio files and read their metadata.
pub fn scan_directory(dir: &Path) -> Result<Vec<Track>> {
    let mut tracks = Vec::new();
    let mut id = 1u32;

    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        match read_track_metadata(path, id) {
            Ok(track) => {
                tracks.push(track);
                id += 1;
            }
            Err(e) => {
                eprintln!("Warning: skipping {}: {}", path.display(), e);
            }
        }
    }

    tracks.sort_by(|a, b| a.title.cmp(&b.title));

    // Reassign IDs after sorting
    for (i, track) in tracks.iter_mut().enumerate() {
        track.id = (i + 1) as u32;
    }

    Ok(tracks)
}

fn read_track_metadata(path: &Path, id: u32) -> Result<Track> {
    let tagged_file = Probe::open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?
        .read()
        .with_context(|| format!("Failed to read tags from {}", path.display()))?;

    let properties = tagged_file.properties();
    let duration = properties.duration();
    let sample_rate = properties.sample_rate().unwrap_or(44100);
    let bitrate = properties.overall_bitrate().unwrap_or(320);

    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let title = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    let artist = tag
        .and_then(|t| t.artist().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = tag
        .and_then(|t| t.album().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown Album".to_string());

    let genre = tag
        .and_then(|t| t.genre().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown".to_string());

    let file_size = std::fs::metadata(path)?.len();

    // Extract cover art
    let artwork = tag
        .and_then(|t| {
            t.pictures()
                .iter()
                .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
                .or_else(|| t.pictures().first())
                .map(|p| p.data().to_vec())
        });

    // Build USB path: Contents/<Artist>/<filename>
    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown.mp3");
    let safe_artist = sanitize_path_component(&artist);
    let usb_path = format!("/Contents/{}/{}", safe_artist, filename);

    Ok(Track {
        source_path: path.to_path_buf(),
        usb_path,
        title,
        artist,
        album,
        genre,
        tempo: 0, // filled in by analyzer
        key: String::new(),
        duration_secs: duration.as_secs_f64(),
        sample_rate,
        bitrate,
        file_size,
        id,
        artwork,
    })
}

/// Replace filesystem-unsafe characters.
fn sanitize_path_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
