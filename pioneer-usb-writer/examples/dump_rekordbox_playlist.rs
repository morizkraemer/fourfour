use anyhow::{Context, Result, bail};
use pioneer_usb_writer::reader::masterdb::read_masterdb;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        bail!(
            "usage: {} <master.db> <playlist-name> <source-prefix>",
            args.first().map(String::as_str).unwrap_or("dump_rekordbox_playlist")
        );
    }

    let db_path = Path::new(&args[1]);
    let playlist_name = &args[2];
    let source_prefix = Path::new(&args[3]);

    let import = read_masterdb(db_path)
        .with_context(|| format!("failed to read {}", db_path.display()))?;

    let playlist = import
        .playlists
        .iter()
        .find(|p| p.name == *playlist_name)
        .with_context(|| format!("playlist not found: {playlist_name}"))?;

    let tracks_by_id: HashMap<u32, _> = import.tracks.iter().map(|t| (t.id, t)).collect();
    let mut rows = Vec::new();

    for (position, track_id) in playlist.track_ids.iter().enumerate() {
        let track = tracks_by_id
            .get(track_id)
            .with_context(|| format!("playlist references missing track id {track_id}"))?;

        if !path_is_under(&track.source_path, source_prefix) {
            continue;
        }

        let bpm = if track.tempo > 0 {
            Some(track.tempo as f64 / 100.0)
        } else {
            None
        };

        rows.push(json!({
            "playlist_position": position + 1,
            "rekordbox_track_id": track.id,
            "path": track.source_path,
            "title": track.title,
            "artist": track.artist,
            "album": track.album,
            "genre": track.genre,
            "label": track.label,
            "remixer": track.remixer,
            "comment": track.comment,
            "bpm": bpm,
            "bpm_x100": track.tempo,
            "key_raw": track.key,
            "key_camelot_standard": rekordbox_key_to_camelot(&track.key),
            "duration_seconds": track.duration_secs,
            "sample_rate": track.sample_rate,
            "bitrate": track.bitrate,
            "file_size": track.file_size,
        }));
    }

    let output = json!({
        "source": "rekordbox-masterdb",
        "master_db": db_path,
        "playlist": playlist_name,
        "source_prefix": source_prefix,
        "playlist_tracks_total": playlist.track_ids.len(),
        "tracks_after_prefix_filter": rows.len(),
        "tracks": rows,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn path_is_under(path: &Path, prefix: &Path) -> bool {
    normalize_path(path).starts_with(normalize_path(prefix))
}

fn normalize_path(path: &Path) -> PathBuf {
    path.components().collect()
}

fn rekordbox_key_to_camelot(raw: &str) -> Option<&'static str> {
    let mut key = raw.trim().replace('♯', "#").replace('♭', "b");
    if key.is_empty() {
        return None;
    }

    let is_minor = key.ends_with('m') || key.ends_with("min") || key.ends_with("minor");
    if key.ends_with("minor") {
        key.truncate(key.len() - "minor".len());
    } else if key.ends_with("min") {
        key.truncate(key.len() - "min".len());
    } else if key.ends_with('m') {
        key.pop();
    }

    let note = key.trim().to_ascii_uppercase();
    match (note.as_str(), is_minor) {
        ("AB" | "G#", true) => Some("1A"),
        ("EB" | "D#", true) => Some("2A"),
        ("BB" | "A#", true) => Some("3A"),
        ("F", true) => Some("4A"),
        ("C", true) => Some("5A"),
        ("G", true) => Some("6A"),
        ("D", true) => Some("7A"),
        ("A", true) => Some("8A"),
        ("E", true) => Some("9A"),
        ("B", true) => Some("10A"),
        ("F#" | "GB", true) => Some("11A"),
        ("C#" | "DB", true) => Some("12A"),
        ("B", false) => Some("1B"),
        ("F#" | "GB", false) => Some("2B"),
        ("C#" | "DB", false) => Some("3B"),
        ("AB" | "G#", false) => Some("4B"),
        ("EB" | "D#", false) => Some("5B"),
        ("BB" | "A#", false) => Some("6B"),
        ("F", false) => Some("7B"),
        ("C", false) => Some("8B"),
        ("G", false) => Some("9B"),
        ("D", false) => Some("10B"),
        ("A", false) => Some("11B"),
        ("E", false) => Some("12B"),
        _ => None,
    }
}
