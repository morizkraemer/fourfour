use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models;

use crate::dto::base64_encode;
use crate::track_infos::effective_preview;

/// Max RGB waveform samples sent over IPC for the compact strip / table. Full detail is ~150/sec.
const PLAYER_COLOR_WAVEFORM_MAX: usize = 2048;

/// Safety cap for expanded-player IPC (~55 min at 150 cols/sec). Peak-hold
/// downsampling only applies beyond native length; typical tracks send full detail.
const PLAYER_COLOR_DETAIL_MAX: usize = 500_000;

/// Fixed-width overview bar for the expanded player (Pioneer PWV6 length).
const PLAYER_COLOR_OVERVIEW_LEN: usize = 1200;

fn downsample_3band(source: &[[u8; 3]], max_samples: usize) -> Vec<[u8; 3]> {
    if source.len() <= max_samples {
        return source.to_vec();
    }
    let n = source.len();
    (0..max_samples)
        .map(|i| {
            let start = i * n / max_samples;
            let end = ((i + 1) * n / max_samples).max(start + 1).min(n);
            source[start..end].iter().copied().fold([0u8; 3], |[l, m, h], [l2, m2, h2]| {
                [l.max(l2), m.max(m2), h.max(h2)]
            })
        })
        .collect()
}

fn color_detail_for_player(cw: &models::ColorWaveform) -> Vec<[u8; 3]> {
    let src = if !cw.detail.is_empty() {
        &cw.detail
    } else {
        &cw.overview
    };
    downsample_3band(src, PLAYER_COLOR_DETAIL_MAX)
}

fn color_waveform_for_player(cw: &models::ColorWaveform) -> Vec<[u8; 3]> {
    // Prefer full detail downsampled to the IPC cap. Overview (~1200 cols) is the
    // Rekordbox-native width but looks soft on our ~150 col/sec analyzer output.
    if !cw.detail.is_empty() {
        return downsample_3band(&cw.detail, PLAYER_COLOR_WAVEFORM_MAX);
    }
    if !cw.overview.is_empty() {
        return downsample_3band(&cw.overview, PLAYER_COLOR_WAVEFORM_MAX);
    }
    Vec::new()
}

/// Scale per-column band amps so the track peak matches PWAV (sample-peak based,
/// absolute 0 dBFS = 1.0). Band values stay per-column for smooth detail; only
/// the global calibration comes from the coarse preview.
fn pwav_band_calibration(entries: &[[u8; 3]], preview: &[u8]) -> f64 {
    if !preview.iter().any(|b| b & 0x1f != 0) {
        return 1.0;
    }
    let pwav_peak = preview
        .iter()
        .map(|b| (b & 0x1f) as f64 / 31.0)
        .fold(0.0_f64, f64::max);
    let band_peak = entries
        .iter()
        .map(|[l, m, h]| (*l).max(*m).max(*h) as f64 / 127.0)
        .fold(0.0_f64, f64::max);
    if band_peak > 1e-6 {
        pwav_peak / band_peak
    } else {
        1.0
    }
}

/// Band bytes as base64 plus the PWAV calibration; the frontend rebuilds
/// `{amp, r, g, b}` per sample from them (see `decodeColorWaveform`).
fn color_payload(entries: &[[u8; 3]], preview: Option<&[u8]>) -> serde_json::Value {
    let calibration = preview
        .map(|p| pwav_band_calibration(entries, p))
        .unwrap_or(1.0);
    let bands: Vec<u8> = entries.iter().flatten().copied().collect();
    serde_json::json!({
        "len": entries.len(),
        "calibration": calibration,
        "bands": base64_encode(&bands),
    })
}

/// Stored analysis data for a track, shaped for the frontend.
pub fn build_analysis_json(
    lib: &LocalLibrary,
    track_id: i64,
) -> Result<serde_json::Value, String> {
    let analysis = lib.get_analysis(track_id).map_err(|e| e.to_string())?;

    match analysis {
        Some(a) => {
            // Get track duration for beat grid display
            let duration_ms = lib
                .get_track(track_id)
                .map_err(|e| e.to_string())?
                .map(|t| (t.duration_secs * 1000.0).round() as u64)
                .unwrap_or(0);

            let preview = effective_preview(&a);
            let preview_slice = preview.as_deref();

            let waveform_color = a
                .color_waveform
                .as_ref()
                .map(|cw| color_payload(&color_waveform_for_player(cw), preview_slice))
                .unwrap_or(serde_json::Value::Null);

            // Higher-resolution detail + fixed overview for the expanded player's
            // dual-view renderer. Compact strip keeps using `waveform_color` above.
            let waveform_color_detail = a
                .color_waveform
                .as_ref()
                .map(|cw| color_payload(&color_detail_for_player(cw), preview_slice))
                .unwrap_or_default();
            let waveform_overview = a
                .color_waveform
                .as_ref()
                .map(|cw| {
                    let src = if cw.overview.is_empty() { &cw.detail } else { &cw.overview };
                    color_payload(&downsample_3band(src, PLAYER_COLOR_OVERVIEW_LEN), preview_slice)
                })
                .unwrap_or_default();

            // Serialize beat grid for frontend waveform rendering
            let beats: Vec<serde_json::Value> = a.beat_grid.beats.iter().map(|b| {
                serde_json::json!({
                    "time_ms": b.time_ms,
                    "bar_position": b.bar_position,
                })
            }).collect();

            // Serialize cue points
            let cue_points: Vec<serde_json::Value> = a.cue_points.iter().map(|c| {
                serde_json::json!({
                    "hot_cue_number": c.hot_cue_number,
                    "time_ms": c.time_ms,
                    "loop_time_ms": c.loop_time_ms,
                })
            }).collect();

            Ok(serde_json::json!({
                "waveform_preview": preview.unwrap_or_default(),
                "waveform_color": waveform_color,
                "waveform_color_detail": waveform_color_detail,
                "waveform_overview": waveform_overview,
                "waveform_peaks": serde_json::Value::Array(vec![]),
                "bpm": a.bpm,
                "key": a.key,
                "energy": a.energy,
                "beats": beats,
                "cue_points": cue_points,
                "duration_ms": duration_ms,
            }))
        }
        None => Err("No analysis data for this track".to_string()),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_payload_carries_band_bytes_and_calibration() {
        let entries = [[10u8, 20, 30], [127, 0, 5]];
        let preview = [0x1f_u8; 4];
        let payload = color_payload(&entries, Some(&preview));

        assert_eq!(payload["len"], 2);
        assert_eq!(payload["bands"], base64_encode(&[10, 20, 30, 127, 0, 5]));
        let calibration = payload["calibration"].as_f64().unwrap();
        assert!((calibration - pwav_band_calibration(&entries, &preview)).abs() < 1e-12);
    }

    #[test]
    fn color_payload_without_preview_is_uncalibrated() {
        let payload = color_payload(&[[1, 2, 3]], None);
        assert_eq!(payload["calibration"], 1.0);
    }

    #[test]
    fn downsample_keeps_peak_per_window() {
        let source: Vec<[u8; 3]> = (0..10u8).map(|n| [n, 0, 0]).collect();
        let out = downsample_3band(&source, 5);
        assert_eq!(out, vec![[1, 0, 0], [3, 0, 0], [5, 0, 0], [7, 0, 0], [9, 0, 0]]);
    }
}
