use anyhow::{Context, Result};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use std::path::Path;
use walkdir::WalkDir;

use crate::models::Track;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "aiff", "aif", "m4a", "aac"];

/// Recursively scan a directory for audio files and return their metadata as a list of [`Track`]s.
///
/// Walks `dir` (following symlinks), skips any file whose extension is not in the
/// supported set (`mp3`, `flac`, `wav`, `aiff`, `aif`, `m4a`, `aac`), and reads
/// tag metadata from each matching file via [`read_track_metadata`].
///
/// Files that cannot be opened or parsed emit a warning to stderr and are skipped.
///
/// The returned list is **sorted** by title → artist → source path, and track IDs
/// are reassigned in that sorted order starting at 1.
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

    tracks.sort_by(|a, b| {
        a.title.cmp(&b.title)
            .then_with(|| a.artist.cmp(&b.artist))
            .then_with(|| a.source_path.cmp(&b.source_path))
    });

    // Reassign IDs after sorting
    for (i, track) in tracks.iter_mut().enumerate() {
        track.id = (i + 1) as u32;
    }

    Ok(tracks)
}

/// Read metadata from a specific list of file paths and return them as [`Track`]s.
///
/// Unlike [`scan_directory`], this function does not recurse — it processes only
/// the exact paths provided. Non-audio files (unrecognised extension) are silently
/// skipped; files that fail to parse emit a warning and are skipped.
///
/// Tracks are returned **in the order the paths were supplied** (no sorting).
/// IDs are assigned sequentially starting at 1.
pub fn scan_files(paths: &[std::path::PathBuf]) -> Result<Vec<Track>> {
    let audio_extensions = ["mp3", "flac", "wav", "aiff", "aif", "m4a", "aac"];
    let mut tracks = Vec::new();
    let mut id = 1u32;

    for path in paths {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if audio_extensions.contains(&ext.to_lowercase().as_str()) {
                match read_track_metadata(path, id) {
                    Ok(track) => {
                        tracks.push(track);
                        id += 1;
                    }
                    Err(e) => eprintln!("Warning: failed to read {}: {}", path.display(), e),
                }
            }
        }
    }

    Ok(tracks)
}

/// Read tag metadata from a single audio file and return a [`Track`].
///
/// Extracts title, artist, album, genre, label, remixer, comment, year, disc number,
/// track number, sample rate, bitrate, file size, and cover art bytes via `lofty`.
///
/// **Defaults for missing tags:**
/// - `title` — filename stem (without extension)
/// - `artist` — `"Unknown Artist"`
/// - `album` — `"Unknown Album"`
/// - `genre` — `"Unknown"`
/// - `label`, `remixer`, `comment` — empty string
/// - `year`, `disc_number`, `track_number` — `0`
///
/// The `tempo` and `key` fields are left empty/zero; they must be filled in by
/// the analyzer before writing to USB.
///
/// The USB-relative audio path is derived as `/Contents/<artist>/<filename>`,
/// with path-unsafe characters in the artist and filename replaced by `_`.
pub fn read_track_metadata(path: &Path, id: u32) -> Result<Track> {
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

    let label = tag
        .and_then(|t| t.get_string(ItemKey::Publisher).map(|s| s.to_string()))
        .unwrap_or_default();

    let remixer = tag
        .and_then(|t| t.get_string(ItemKey::Remixer).map(|s| s.to_string()))
        .unwrap_or_default();

    let comment = tag
        .and_then(|t| t.get_string(ItemKey::Comment).map(|s| s.to_string()))
        .unwrap_or_default();

    let year = tag
        .and_then(|t| t.get_string(ItemKey::Year).and_then(|s| s.parse::<u16>().ok()))
        .unwrap_or(0);

    let disc_number = tag
        .and_then(|t| t.disk())
        .unwrap_or(0) as u16;

    let track_number = tag
        .and_then(|t| t.track())
        .unwrap_or(0);

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
    let raw_filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown.mp3");
    let filename = sanitize_path_component(raw_filename);
    let safe_artist = sanitize_path_component(&artist);
    let usb_path = format!("/Contents/{}/{}", safe_artist, filename);

    Ok(Track {
        source_path: path.to_path_buf(),
        usb_path,
        title,
        artist,
        album,
        genre,
        label,
        remixer,
        comment,
        year,
        disc_number,
        track_number,
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
