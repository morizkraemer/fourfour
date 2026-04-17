/// Shared types used across the application.

use serde::{Deserialize, Serialize};

/// Metadata and analysis results for a single audio track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Original file path on disk
    pub source_path: std::path::PathBuf,
    /// Relative path on USB (e.g. "Contents/Artist/track.mp3")
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
    pub beats: Vec<Beat>,
}

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
    pub beat_grid: BeatGrid,
    pub waveform: WaveformPreview,
    pub bpm: f64,
    pub key: String,
    pub cue_points: Vec<CuePoint>,
}

/// A playlist containing a subset of tracks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u32,
    pub name: String,
    /// Track IDs belonging to this playlist.
    pub track_ids: Vec<u32>,
}
