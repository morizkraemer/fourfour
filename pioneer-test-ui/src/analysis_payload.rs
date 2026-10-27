use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models;

use crate::dto::base64_encode;
use crate::track_infos::{effective_preview, row_bands};

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

/// Band bytes as base64 plus the calibration the frontend applies to them.
///
/// `pwv7` marks bytes that are the stick's `PWV7` / `PWV6` values (the track has a stored
/// `.2EX`): they go over unscaled, `calibration` is 1 and the frontend draws `byte / 127`.
/// Other bytes (tracks stored before the 3-band export) keep the PWAV peak calibration.
fn color_payload(entries: &[[u8; 3]], preview: Option<&[u8]>, pwv7: bool) -> serde_json::Value {
    let calibration = match preview {
        Some(p) if !pwv7 => pwav_band_calibration(entries, p),
        _ => 1.0,
    };
    let bands: Vec<u8> = entries.iter().flatten().copied().collect();
    serde_json::json!({
        "len": entries.len(),
        "calibration": calibration,
        "pwv7": pwv7,
        "bands": base64_encode(&bands),
    })
}

/// The colour fields of `get_analysis_data`: the compact strip (peak-held to 2048 columns),
/// the expanded player's full detail and 1200-column overview, and the 3-band row preview.
/// `Null` without colour data.
pub fn waveform_fields(
    cw: Option<&models::ColorWaveform>,
    preview: Option<&[u8]>,
    pwv7: bool,
) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    let null = serde_json::Value::Null;
    let color = cw.map(|cw| color_payload(&color_waveform_for_player(cw), preview, pwv7));
    let detail = cw.map(|cw| color_payload(&color_detail_for_player(cw), preview, pwv7));
    let overview = cw.map(|cw| {
        let src = if cw.overview.is_empty() { &cw.detail } else { &cw.overview };
        color_payload(&downsample_3band(src, PLAYER_COLOR_OVERVIEW_LEN), preview, pwv7)
    });
    let row = cw.and_then(row_bands).map(|bytes| serde_json::Value::String(base64_encode(&bytes)));
    fields.insert("waveform_color".into(), color.unwrap_or_else(|| null.clone()));
    fields.insert("waveform_color_detail".into(), detail.unwrap_or_else(|| null.clone()));
    fields.insert("waveform_overview".into(), overview.unwrap_or_else(|| null.clone()));
    fields.insert("waveform_row_bands".into(), row.unwrap_or(null));
    fields
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

            let pwv7 = a.color_waveform.as_ref().is_some_and(|cw| cw.exact.is_some());
            let mut payload = waveform_fields(a.color_waveform.as_ref(), preview_slice, pwv7);

            // Serialize beat grid for frontend waveform rendering
            let beats: Vec<serde_json::Value> = a.beat_grid.beats.iter().map(|b| {
                serde_json::json!({
                    "time_ms": b.time_ms,
                    "bar_position": b.bar_position,
                })
            }).collect();

            let cue_points: Vec<serde_json::Value> = lib
                .get_cues(track_id)
                .map_err(|e| e.to_string())?
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "hot_cue_number": c.hot_cue_number,
                        "time_ms": c.time_ms,
                        "loop_time_ms": c.loop_time_ms,
                        "color": c.color,
                        "name": c.name,
                    })
                })
                .collect();

            payload.insert("waveform_preview".into(), serde_json::json!(preview.unwrap_or_default()));
            payload.insert("waveform_peaks".into(), serde_json::Value::Array(vec![]));
            payload.insert("bpm".into(), serde_json::json!(a.bpm));
            payload.insert("key".into(), serde_json::json!(a.key));
            let edits = lib.user_edits(track_id);
            payload.insert("bpm_locked".into(), serde_json::json!(edits.bpm_locked));
            payload.insert("key_locked".into(), serde_json::json!(edits.key_locked));
            payload.insert("energy".into(), serde_json::json!(a.energy));
            payload.insert("beats".into(), serde_json::Value::Array(beats));
            payload.insert("cue_points".into(), serde_json::Value::Array(cue_points));
            payload.insert("duration_ms".into(), serde_json::json!(duration_ms));
            let stages = lib.analysis_stages(track_id).map_err(|e| e.to_string())?;
            let pending = stages != Some(pioneer_library::analysis_stage::ALL);
            payload.insert("analysis_pending".into(), serde_json::json!(pending));
            payload.insert("analysis_stages".into(), serde_json::json!(stages.unwrap_or(0)));
            Ok(serde_json::Value::Object(payload))
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
        let payload = color_payload(&entries, Some(&preview), false);

        assert_eq!(payload["len"], 2);
        assert_eq!(payload["bands"], base64_encode(&[10, 20, 30, 127, 0, 5]));
        let calibration = payload["calibration"].as_f64().unwrap();
        assert!((calibration - pwav_band_calibration(&entries, &preview)).abs() < 1e-12);
    }

    #[test]
    fn color_payload_without_preview_is_uncalibrated() {
        let payload = color_payload(&[[1, 2, 3]], None, false);
        assert_eq!(payload["calibration"], 1.0);
    }

    #[test]
    fn downsample_keeps_peak_per_window() {
        let source: Vec<[u8; 3]> = (0..10u8).map(|n| [n, 0, 0]).collect();
        let out = downsample_3band(&source, 5);
        assert_eq!(out, vec![[1, 0, 0], [3, 0, 0], [5, 0, 0], [7, 0, 0], [9, 0, 0]]);
    }

    fn base64_decode(text: &str) -> Vec<u8> {
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut acc = 0u32;
        let mut bits = 0;
        for c in text.bytes().filter(|&c| c != b'=') {
            acc = (acc << 6) | ALPHABET.iter().position(|&a| a == c).expect("base64 character") as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        out
    }

    fn section_body<'a>(file: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
        let be = |at: usize| u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
        let mut offset = be(4);
        while offset + 12 <= file.len() {
            let (header_len, section_len) = (be(offset + 4), be(offset + 8));
            if &file[offset..offset + 4] == tag {
                return &file[offset + header_len..offset + section_len];
            }
            offset += section_len;
        }
        panic!("no {} section", String::from_utf8_lossy(tag));
    }

    fn test_track(seconds: f64) -> models::Track {
        models::Track {
            id: 0,
            source_path: std::path::PathBuf::from("/music/q36.mp3"),
            usb_path: "/Contents/q36.mp3".to_string(),
            title: "Q36".to_string(),
            artist: "Test".to_string(),
            album: String::new(),
            genre: String::new(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 2026,
            disc_number: 1,
            track_number: 1,
            tempo: 12_400,
            key: "8A".to_string(),
            duration_secs: seconds,
            sample_rate: 44_100,
            bitrate: 320,
            file_size: 1,
            artwork: None,
        }
    }

    fn measured_analysis(seconds: f64) -> models::AnalysisResult {
        let rate = 44_100u32;
        let frames = (rate as f64 * seconds) as usize;
        let samples: Vec<f32> = (0..frames)
            .flat_map(|i| {
                let t = i as f64 / rate as f64;
                let kick = (2.0 * std::f64::consts::PI * 55.0 * t).sin() * (1.0 - (t * 2.0).fract());
                let hat = (2.0 * std::f64::consts::PI * 7000.0 * t).sin() * (1.0 - (t * 4.0).fract()).powi(3);
                let v = (0.55 * kick + 0.25 * hat) as f32;
                [v, v]
            })
            .collect();
        let mut meter = pioneer_usb_writer::waveform::Meter::new(rate, 2);
        meter.push(&samples);
        let waves = pioneer_usb_writer::waveform::analyze(&meter.finish());
        models::AnalysisResult {
            beat_grid: models::BeatGrid { beats: Vec::new() },
            waveform: waves.preview().expect("400-column preview"),
            bpm: 124.0,
            key: "8A".to_string(),
            energy: None,
            cue_points: Vec::new(),
            color_waveform: Some(waves.color_waveform()),
        }
    }

    #[test]
    fn analysis_data_band_bytes_are_the_pwv7_bytes_written_to_the_2ex() {
        let dir = std::env::temp_dir().join(format!("ff-q36-payload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
        let seconds = 12.0;
        let id = lib.add_track(&test_track(seconds)).unwrap();
        let analysis = measured_analysis(seconds);
        lib.set_analysis(id, &analysis).unwrap();

        let payload = build_analysis_json(&lib, id).unwrap();
        let two_ex = std::fs::read(dir.join("anlz").join(id.to_string()).join("ANLZ0000.2EX")).unwrap();
        let pwv7 = section_body(&two_ex, b"PWV7");
        let pwv6 = section_body(&two_ex, b"PWV6");
        assert_eq!(pwv7.len(), 1800 * 3);

        let detail = &payload["waveform_color_detail"];
        assert_eq!(detail["pwv7"], true);
        assert_eq!(detail["calibration"], 1.0);
        assert_eq!(detail["len"], 1800);
        assert_eq!(base64_decode(detail["bands"].as_str().unwrap()), pwv7);
        assert_eq!(base64_decode(payload["waveform_overview"]["bands"].as_str().unwrap()), pwv6);

        let compact = base64_decode(payload["waveform_color"]["bands"].as_str().unwrap());
        assert_eq!(compact, pwv7, "1800 columns fit under the compact cap, so it is the PWV7 bytes too");

        let written = &analysis.color_waveform.as_ref().unwrap().detail;
        let flat: Vec<u8> = written.iter().flatten().copied().collect();
        assert_eq!(flat, pwv7, "the stick carries the bytes the analysis measured");

        let row = base64_decode(payload["waveform_row_bands"].as_str().unwrap());
        assert_eq!(row.len(), pioneer_library::ROW_BAND_COLUMNS * 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn analysis_data_carries_edited_cues_with_name_and_colour() {
        let dir = std::env::temp_dir().join(format!("ff-wd-cues-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
        let id = lib.add_track(&test_track(12.0)).unwrap();
        lib.set_analysis(id, &measured_analysis(12.0)).unwrap();
        let cue = |slot, time_ms, name: &str, color: Option<&str>| pioneer_library::rekordbox::ImportedCue {
            hot_cue_number: slot,
            time_ms,
            loop_time_ms: None,
            color: color.map(String::from),
            name: name.into(),
        };
        lib.set_cues(id, &[cue(2, 4_000, "Drop", Some("#ff2fa3")), cue(0, 1_000, "", None)]).unwrap();

        let payload = build_analysis_json(&lib, id).unwrap();
        let cues = payload["cue_points"].as_array().unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0]["hot_cue_number"], 2);
        assert_eq!(cues[0]["name"], "Drop");
        assert_eq!(cues[0]["color"], "#ff2fa3");
        assert_eq!(cues[1]["hot_cue_number"], 0);
        assert!(cues[1]["color"].is_null());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tracks_without_pwv7_keep_the_calibrated_fallback() {
        let payload = color_payload(&[[10, 20, 30]; 16], Some(&[0x10_u8; 4]), false);
        assert_eq!(payload["pwv7"], false);
        assert!(payload["calibration"].as_f64().unwrap() > 1.0);
    }
}
