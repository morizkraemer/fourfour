use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::Track;

pub fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn serialize_preview<S: serde::Serializer>(
    preview: &Option<Vec<u8>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match preview {
        Some(bytes) => serializer.serialize_str(&base64_encode(bytes)),
        None => serializer.serialize_none(),
    }
}

/// Lightweight track info sent over the IPC boundary.
/// Omits artwork bytes and other heavy fields.
#[derive(Serialize, Clone)]
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
    /// True when an analysis row exists in the DB index (authoritative, not `tempo > 0`).
    pub has_analysis: bool,
    /// 400-byte mono waveform preview as base64 (a JSON number array is ~2.6x
    /// larger), present only when a real (non-placeholder) preview exists. Lets
    /// the track list paint mini waveforms without an N+1 fetch.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_preview")]
    pub waveform_preview: Option<Vec<u8>>,
    /// 3-band row preview (`ROW_BAND_COLUMNS` columns of low, mid, high bytes, 0..127) as base64.
    /// When present it replaces `waveform_preview`, which then stays unset.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_preview")]
    pub waveform_bands: Option<Vec<u8>>,
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
            has_analysis: false,
            waveform_preview: None,
            waveform_bands: None,
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
    pub track_id: Option<u32>,
}

/// State returned by `load_state` so the frontend can restore everything.
#[derive(Serialize, Clone)]
pub struct LoadedState {
    pub tracks: Vec<TrackInfo>,
    pub playlists: Vec<PlaylistInput>,
}

#[cfg(test)]
mod tests {
    use super::base64_encode;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(&[0xFB, 0xFF, 0xFE]), "+//+");
    }
}
