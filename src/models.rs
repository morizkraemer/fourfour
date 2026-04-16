/// Shared types used across the application.

/// Metadata and analysis results for a single audio track.
#[derive(Debug, Clone)]
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
}

/// Beat grid: list of beat positions with timing info.
#[derive(Debug, Clone)]
pub struct BeatGrid {
    pub beats: Vec<Beat>,
}

#[derive(Debug, Clone, Copy)]
pub struct Beat {
    /// Beat number within the bar (1-4)
    pub bar_position: u8,
    /// Time in milliseconds from track start
    pub time_ms: u32,
    /// Tempo at this beat (BPM × 100)
    pub tempo: u32,
}

/// 400-byte monochrome waveform preview.
#[derive(Debug, Clone)]
pub struct WaveformPreview {
    /// 400 bytes, each encoding height (5 low bits) and whiteness (3 high bits)
    pub data: [u8; 400],
}

/// Full analysis result for a track.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub beat_grid: BeatGrid,
    pub waveform: WaveformPreview,
    pub bpm: f64,
    pub key: String,
}

/// A playlist containing a subset of tracks.
#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: u32,
    pub name: String,
    /// Track IDs belonging to this playlist.
    pub track_ids: Vec<u32>,
}
