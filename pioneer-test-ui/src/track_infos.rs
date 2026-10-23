use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models;

use crate::dto::TrackInfo;

/// Build TrackInfo list from the library's tracks + flags.
///
/// Embeds the mono preview so the list paints mini waveforms without an N+1
/// fetch. Tracks without a real preview (the all-zero placeholder, missing ANLZ
/// files) get none, otherwise the UI would cache a flat waveform as if it were
/// real. When the DAT's PWAV section is degenerate but color data exists, the
/// preview is synthesized from the color overview so the list mini-waveform
/// stays consistent with the player strip — see #waveform-bug.
pub fn build_track_infos(lib: &LocalLibrary) -> Result<Vec<TrackInfo>, String> {
    let rows = lib.get_all_tracks_with_flags().map_err(|e| e.to_string())?;
    let analyses = lib.get_waveform_analyses().map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(track, has_artwork, has_analysis, has_cues)| {
            let waveform_preview = if has_analysis {
                analyses
                    .get(&i64::from(track.id))
                    .and_then(effective_preview)
            } else {
                None
            };
            TrackInfo {
                id: track.id,
                source_path: track.source_path.to_string_lossy().to_string(),
                title: track.title,
                artist: track.artist,
                album: track.album,
                genre: track.genre,
                tempo: track.tempo,
                key: track.key,
                duration_secs: track.duration_secs,
                bitrate: track.bitrate,
                file_size: track.file_size,
                has_artwork,
                has_cues,
                has_analysis,
                waveform_preview,
            }
        })
        .collect())
}

/// Build a 400-byte mono PWAV preview from a color waveform's per-sample band
/// peaks. Used when a track's stored PWAV section is degenerate but its color
/// data is intact, so the list mini-waveform matches what the player renders.
/// Byte layout matches PWAV: bits 0-4 height (0-31), bits 5-7 whiteness (0-7).
fn mono_preview_from_color(cw: &models::ColorWaveform) -> Option<[u8; 400]> {
    let src = if !cw.overview.is_empty() {
        &cw.overview
    } else if !cw.detail.is_empty() {
        &cw.detail
    } else {
        return None;
    };

    let amps: Vec<u8> = src.iter().map(|[l, m, h]| (*l).max(*m).max(*h)).collect();
    let max = amps.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return None;
    }

    let n = amps.len();
    let mut out = [0u8; 400];
    for (i, byte) in out.iter_mut().enumerate() {
        let start = i * n / 400;
        let end = ((i + 1) * n / 400).max(start + 1).min(n);
        let bucket = amps[start..end].iter().copied().max().unwrap_or(0) as u32;
        // Absolute scale: 127 = 0 dBFS reference (matches PWV7 band ceiling).
        let height = ((bucket * 31) / 127).min(31) as u8;
        let whiteness = ((bucket * 7) / 127).min(7) as u8;
        *byte = (whiteness << 5) | (height & 0x1F);
    }
    Some(out)
}

/// The mono PWAV preview to show, shared by the list and the player so both
/// surfaces stay in sync: the real DAT preview when it carries signal, else a
/// preview synthesized from the color waveform, else `None` (no waveform data).
pub fn effective_preview(a: &models::AnalysisResult) -> Option<Vec<u8>> {
    if a.waveform.data.iter().any(|&b| b & 0x1f != 0) {
        Some(a.waveform.data.to_vec())
    } else {
        a.color_waveform
            .as_ref()
            .and_then(mono_preview_from_color)
            .map(|d| d.to_vec())
    }
}
