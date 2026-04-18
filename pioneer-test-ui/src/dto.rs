use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::Track;

/// Lightweight track info sent over the IPC boundary.
/// Omits artwork bytes and other heavy fields.
#[derive(Serialize, Deserialize, Clone)]
pub struct TrackInfo {
    pub id: u32,
    pub source_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    /// BPM * 100, 0 = not yet analyzed
    pub tempo: u32,
    /// Empty string = not yet analyzed
    pub key: String,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub file_size: u64,
    pub has_artwork: bool,
    pub has_cues: bool,
}

impl From<&Track> for TrackInfo {
    fn from(t: &Track) -> Self {
        Self {
            id: t.id,
            source_path: t.source_path.to_string_lossy().to_string(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            genre: t.genre.clone(),
            tempo: t.tempo,
            key: t.key.clone(),
            duration_secs: t.duration_secs,
            bitrate: t.bitrate,
            file_size: t.file_size,
            has_artwork: t.artwork.is_some(),
            has_cues: false,
        }
    }
}

/// Input from the frontend describing a playlist to create on the USB.
#[derive(Serialize, Deserialize, Clone)]
pub struct PlaylistInput {
    pub id: u32,
    pub name: String,
    pub track_ids: Vec<u32>,
}

/// Payload emitted during analysis to report progress.
#[derive(Serialize, Clone)]
pub struct ProgressPayload {
    pub current: u32,
    pub total: u32,
    pub message: String,
}

/// State returned by `load_state` so the frontend can restore everything.
#[derive(Serialize, Deserialize, Clone)]
pub struct LoadedState {
    pub tracks: Vec<TrackInfo>,
    pub playlists: Vec<PlaylistInput>,
}
