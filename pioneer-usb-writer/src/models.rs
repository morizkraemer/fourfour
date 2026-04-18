/// Shared types used across the application.

use serde::{Deserialize, Serialize};

/// Metadata and analysis results for a single audio track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Original file path on disk
    pub source_path: std::path::PathBuf,
    /// USB-relative path used by the CDJ, e.g. `/Contents/Artist/track.mp3`
    pub usb_path: String,
    /// Track title from tags
    pub title: String,
    /// Artist name from tags
    pub artist: String,
    /// Album name from tags
    pub album: String,
    /// Genre from tags
    pub genre: String,
    /// Record label from tags
    pub label: String,
    /// Remixer from tags
    pub remixer: String,
    /// Comment from tags
    pub comment: String,
    /// Year of release
    pub year: u16,
    /// Disc number
    pub disc_number: u16,
    /// Track number on disc
    pub track_number: u32,
    /// BPM (beats per minute × 100, as Pioneer stores it)
    pub tempo: u32,
    /// Musical key (e.g. "1A", "5B")
    pub key: String,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Bitrate in kbps
    pub bitrate: u32,
    /// File size in bytes
    pub file_size: u64,
    /// Unique track ID (1-based, assigned during processing)
    pub id: u32,
    /// Raw cover art image bytes (JPEG or PNG), if available
    pub artwork: Option<Vec<u8>>,
}

/// Beat grid: list of beat positions with timing info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatGrid {
    /// Ordered list of every detected beat in the track.
    pub beats: Vec<Beat>,
}

/// A single beat position within a track's beat grid.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Beat {
    /// Beat number within the bar (1-4)
    pub bar_position: u8,
    /// Time in milliseconds from track start
    pub time_ms: u32,
    /// Tempo at this beat (BPM × 100)
    pub tempo: u32,
}

/// 400-byte monochrome waveform preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformPreview {
    /// 400 bytes, each encoding height (5 low bits) and whiteness (3 high bits)
    #[serde(with = "waveform_data")]
    pub data: [u8; 400],
}

mod waveform_data {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &[u8; 400], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(data)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 400], D::Error> {
        let v: Vec<u8> = Deserialize::deserialize(d)?;
        v.try_into()
            .map_err(|v: Vec<u8>| serde::de::Error::custom(format!("expected 400 bytes, got {}", v.len())))
    }
}

/// A cue or memory point within a track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuePoint {
    /// 0 = memory cue, 1 = hot cue A, 2 = hot cue B, etc.
    pub hot_cue_number: u32,
    /// Position in milliseconds from track start
    pub time_ms: u32,
    /// Loop end position in ms, or `None` if not a loop
    pub loop_time_ms: Option<u32>,
}

/// Full analysis result for a track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Beat grid written to the ANLZ BEAT tag.
    pub beat_grid: BeatGrid,
    /// Monochrome waveform preview written to the ANLZ PWAV tag.
    pub waveform: WaveformPreview,
    /// Detected tempo in BPM (floating-point, not scaled).
    pub bpm: f64,
    /// Detected musical key, e.g. `"1A"` or `"5B"` (Camelot notation).
    pub key: String,
    /// Memory cues and hot cues written to the ANLZ PCOB tag.
    pub cue_points: Vec<CuePoint>,
}

/// A playlist containing a subset of tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    /// 1-based playlist ID stored in the PDB playlists table.
    pub id: u32,
    pub name: String,
    /// Track IDs belonging to this playlist (references `Track::id`).
    pub track_ids: Vec<u32>,
}

/// A track read back from an existing USB's OneLibrary database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingTrack {
    /// 1-based track ID from the PDB tracks table.
    pub id: u32,
    /// USB-relative path, e.g. `/Contents/Artist/track.mp3`.
    pub usb_path: String,
    pub title: String,
    pub artist: String,
    pub remixer: String,
    pub album: String,
    pub genre: String,
    pub label: String,
    /// Musical key in Camelot notation, e.g. `"1A"`.
    pub key: String,
    pub comment: String,
    pub year: u16,
    pub track_number: u32,
    pub disc_number: u16,
    /// Tempo as BPM × 100 (e.g. 12800 = 128.00 BPM).
    pub tempo: u32,
    /// Track length in seconds.
    pub duration_secs: f64,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Bitrate in kbps.
    pub bitrate: u32,
    /// File size in bytes.
    pub file_size: u64,
    /// Whether artwork is stored on the USB for this track.
    pub has_artwork: bool,
}

/// A playlist read back from an existing USB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingPlaylist {
    /// 1-based playlist ID from the PDB playlists table.
    pub id: u32,
    pub name: String,
    /// Track IDs belonging to this playlist (references `ExistingTrack::id`).
    pub track_ids: Vec<u32>,
}

/// Full state read from an existing USB's OneLibrary database.
#[derive(Debug, Clone)]
pub struct ExistingUsbState {
    /// All tracks found in the existing database.
    pub tracks: Vec<ExistingTrack>,
    /// All playlists found in the existing database.
    pub playlists: Vec<ExistingPlaylist>,
    /// Next available track ID to use when appending new tracks (max existing ID + 1).
    pub next_track_id: u32,
    /// Next available playlist ID to use when appending new playlists (max existing ID + 1).
    pub next_playlist_id: u32,
}
