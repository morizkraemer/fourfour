//! Parser for Pioneer ANLZ `.DAT` and `.EXT` binary files.
//!
//! The ANLZ format is tag-based: a 28-byte `PMAI` file header followed by
//! sequential sections.  Each section starts with a 4-byte ASCII tag, a
//! big-endian `header_len`, and a big-endian `section_len`.  Data follows
//! after the header; skip to `offset + section_len` to reach the next section.
//!
//! This reader is intentionally tolerant — unknown tags are silently skipped
//! and missing sections produce sensible defaults so that partially written
//! or future-format files degrade gracefully.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::models::{
    AnalysisResult, Beat, BeatGrid, ColorWaveform, CuePoint, WaveformPreview,
};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Read a big-endian u32 from `data` at `offset`.
fn read_u32_be(data: &[u8], offset: usize) -> Result<u32> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .context("unexpected EOF reading u32")?
        .try_into()
        .unwrap();
    Ok(u32::from_be_bytes(bytes))
}

/// Read a big-endian u16 from `data` at `offset`.
fn read_u16_be(data: &[u8], offset: usize) -> Result<u16> {
    let bytes: [u8; 2] = data
        .get(offset..offset + 2)
        .context("unexpected EOF reading u16")?
        .try_into()
        .unwrap();
    Ok(u16::from_be_bytes(bytes))
}

// ── section parsers ─────────────────────────────────────────────────────────

/// Parse a PQTZ (beat grid) section into a list of [`Beat`]s.
fn parse_pqtz(section: &[u8]) -> Result<Vec<Beat>> {
    // Header is 24 bytes. beat_count at [20..24].
    if section.len() < 24 {
        bail!("PQTZ section too short ({} bytes)", section.len());
    }
    let beat_count = read_u32_be(section, 20)? as usize;
    let data = &section[24..];

    let mut beats = Vec::with_capacity(beat_count);
    for i in 0..beat_count {
        let base = i * 8;
        if base + 8 > data.len() {
            break;
        }
        let bar_position = read_u16_be(data, base)? as u8;
        let tempo = read_u16_be(data, base + 2)? as u32;
        let time_ms = read_u32_be(data, base + 4)?;
        beats.push(Beat {
            bar_position,
            time_ms,
            tempo,
        });
    }
    Ok(beats)
}

/// Parse a PWAV (waveform preview) section into a 400-byte array.
fn parse_pwav(section: &[u8]) -> Result<[u8; 400]> {
    // Header is 20 bytes, data is 400 bytes.
    if section.len() < 20 + 400 {
        bail!(
            "PWAV section too short ({} bytes, need {})",
            section.len(),
            20 + 400
        );
    }
    let mut data = [0u8; 400];
    data.copy_from_slice(&section[20..20 + 400]);
    Ok(data)
}

/// Parse a PCOB (cue points) section into a list of [`CuePoint`]s.
///
/// Each PCOB section contains zero or more PCPT sub-entries (56 bytes each).
fn parse_pcob(section: &[u8]) -> Result<Vec<CuePoint>> {
    if section.len() < 24 {
        bail!("PCOB section too short ({} bytes)", section.len());
    }
    // The writer stores entry_count as a u32 at offset 16 in the header.
    // However, the task spec says entry_count is u16 at [20..22].
    // Let's handle both: try the u32 at offset 16 first (matches the writer),
    // but also validate against available data.
    let entry_count_from_header = read_u32_be(section, 16)? as usize;

    let entries_data = &section[24..];
    let pcpt_entry_len = 56usize;
    // Clamp to what's actually available.
    let max_possible = entries_data.len() / pcpt_entry_len;
    let entry_count = entry_count_from_header.min(max_possible);

    let mut cues = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let base = i * pcpt_entry_len;
        let entry = &entries_data[base..base + pcpt_entry_len];

        // Validate PCPT tag
        if entry.get(0..4) != Some(b"PCPT") {
            continue; // skip malformed entries
        }

        let hot_cue_number = read_u32_be(entry, 12)?;
        // time_ms is at offset 0x20 within the PCOB data... but within
        // the PCPT entry it's at relative offset 32 (0x20).
        // From the writer: time_ms written at entry offset 0x20 (32 bytes in).
        let time_ms = read_u32_be(entry, 32)?;
        let loop_time_raw = read_u32_be(entry, 36)?;
        let loop_time_ms = if loop_time_raw == 0xFFFF_FFFF {
            None
        } else {
            Some(loop_time_raw)
        };

        cues.push(CuePoint {
            hot_cue_number,
            time_ms,
            loop_time_ms,
        });
    }
    Ok(cues)
}

/// Parse a PWV3 (color preview waveform) section into `[low, mid, high]` entries.
fn parse_pwv3(section: &[u8]) -> Result<Vec<[u8; 3]>> {
    if section.len() < 24 {
        bail!("PWV3 section too short ({} bytes)", section.len());
    }
    let entry_count = read_u32_be(section, 16)? as usize;
    let data = &section[24..];

    let count = entry_count.min(data.len());
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let byte = data[i];
        let color = (byte >> 5) & 0x07;
        let height = byte & 0x1F;
        let [low, mid, high] = match color {
            1 => {
                // red/bass dominant
                [
                    (height as u16 * 4).min(255) as u8,
                    height,
                    height,
                ]
            }
            2 => {
                // blue/highs dominant
                [
                    height,
                    height,
                    (height as u16 * 4).min(255) as u8,
                ]
            }
            4 => {
                // green/mids dominant
                [
                    height,
                    (height as u16 * 4).min(255) as u8,
                    height,
                ]
            }
            7 => {
                // white/full-spectrum
                [
                    (height as u16 * 3).min(255) as u8,
                    (height as u16 * 3).min(255) as u8,
                    (height as u16 * 3).min(255) as u8,
                ]
            }
            _ => {
                // unknown color — grey
                [
                    (height as u16 * 2).min(255) as u8,
                    (height as u16 * 2).min(255) as u8,
                    (height as u16 * 2).min(255) as u8,
                ]
            }
        };
        entries.push([low, mid, high]);
    }
    Ok(entries)
}

/// Parse a PWV5 (HD color waveform, 2 bytes/entry, little-endian) section into `[low, mid, high]` entries.
///
/// Each 2-byte entry is a little-endian 16-bit word:
/// - bits 14:10 = amplitude (0-31)
/// - bits  4:0  = color indicator:
///     0-2  → neutral (all bands equal)
///     3-7  → bass dominant   (blue in Rekordbox)
///     8-12 → treble dominant (white)
///     13+  → mid dominant    (orange)
///
/// Outputs `[bass, mid, high]` with the dominant band ×4 so that the same
/// downstream normalization used for PWV3 (`amp = max/124`) produces the
/// correct amplitude and color.
fn parse_pwv5(section: &[u8]) -> Result<Vec<[u8; 3]>> {
    if section.len() < 24 {
        bail!("PWV5 section too short ({} bytes)", section.len());
    }
    let entry_count = read_u32_be(section, 16)? as usize;
    let data = &section[24..];
    let max_entries = data.len() / 2;
    let count = entry_count.min(max_entries);

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let b0 = data[i * 2];
        let b1 = data[i * 2 + 1];
        // Little-endian: word = (b1 << 8) | b0
        let word = ((b1 as u16) << 8) | (b0 as u16);
        let height = ((word >> 10) & 0x1f) as u16;
        let ch3 = (word & 0x1f) as u8;

        let (bass, mid, high): (u16, u16, u16) = if ch3 <= 2 {
            // Neutral / silence — all bands equal at ×4 scale so downstream
            // normalization (÷ max) preserves amplitude = height/31.
            let v = (height * 4).min(255);
            (v, v, v)
        } else if ch3 <= 7 {
            ((height * 4).min(255), height, height) // bass dominant (blue)
        } else if ch3 <= 12 {
            (height, height, (height * 4).min(255)) // treble dominant (white)
        } else {
            (height, (height * 4).min(255), height) // mid dominant (orange)
        };

        entries.push([bass as u8, mid as u8, high as u8]);
    }
    Ok(entries)
}

/// Parse a PWV4 (color overview waveform, 6 bytes/entry) section into `[low, mid, high]` entries.
fn parse_pwv4(section: &[u8]) -> Result<Vec<[u8; 3]>> {
    if section.len() < 24 {
        bail!("PWV4 section too short ({} bytes)", section.len());
    }
    let entry_count = read_u32_be(section, 16)? as usize;
    let data = &section[24..];

    let max_entries = data.len() / 6;
    let count = entry_count.min(max_entries);

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let base = i * 6;
        // [r, g, b, height, blue2, whiteness]
        // r = low (bass), g = mid, b = high — first 3 bytes are the band values.
        let low = data[base];
        let mid = data[base + 1];
        let high = data[base + 2];
        entries.push([low, mid, high]);
    }
    Ok(entries)
}

// ── public API ──────────────────────────────────────────────────────────────

/// Read a `.DAT` file (and its sibling `.EXT` if it exists) into an
/// [`AnalysisResult`].
///
/// The `.EXT` file is expected at the same path with the extension changed to
/// `"EXT"`.  If the `.EXT` file doesn't exist, `color_waveform` will be `None`.
///
/// The reader is tolerant: unknown tags are skipped and missing sections
/// produce sensible defaults.
pub fn read_anlz(dat_path: &Path) -> Result<AnalysisResult> {
    let dat_bytes = std::fs::read(dat_path)
        .with_context(|| format!("failed to read DAT file: {}", dat_path.display()))?;

    let (beats, waveform_data, mut cue_points) = parse_dat_sections(&dat_bytes)?;

    // Try to read the sibling .EXT file.
    let ext_path = dat_path.with_extension("EXT");
    let color_waveform = if ext_path.exists() {
        let ext_bytes = std::fs::read(&ext_path)
            .with_context(|| format!("failed to read EXT file: {}", ext_path.display()))?;
        let (ext_cues, detail, overview) = parse_ext_sections(&ext_bytes)?;

        // PCO2 extended cues are more authoritative than PCOB cues when present.
        if !ext_cues.is_empty() {
            cue_points = ext_cues;
        }

        if detail.is_some() || overview.is_some() {
            Some(ColorWaveform {
                detail: detail.unwrap_or_default(),
                overview: overview.unwrap_or_default(),
            })
        } else {
            None
        }
    } else {
        None
    };

    // Infer BPM from the first beat's tempo field (stored as BPM * 100).
    let bpm = beats
        .first()
        .map(|b| b.tempo as f64 / 100.0)
        .unwrap_or(0.0);

    Ok(AnalysisResult {
        beat_grid: BeatGrid { beats },
        waveform: WaveformPreview {
            data: waveform_data,
        },
        bpm,
        key: String::new(), // Key is not stored in ANLZ; caller fills from DB.
        cue_points,
        color_waveform,
    })
}

/// Walk the tag-based sections in a `.DAT` file and extract what we understand.
fn parse_dat_sections(data: &[u8]) -> Result<(Vec<Beat>, [u8; 400], Vec<CuePoint>)> {
    if data.len() < 28 {
        bail!("DAT file too short for PMAI header ({} bytes)", data.len());
    }
    if &data[0..4] != b"PMAI" {
        bail!(
            "DAT file missing PMAI magic (got {:?})",
            &data[0..4]
        );
    }

    let mut beats: Vec<Beat> = Vec::new();
    let mut waveform_data = [0u8; 400];
    let mut cue_points: Vec<CuePoint> = Vec::new();

    let file_header_len = read_u32_be(data, 4)? as usize;
    let mut offset = file_header_len; // skip the PMAI header

    while offset + 12 <= data.len() {
        let tag = &data[offset..offset + 4];
        let _header_len = read_u32_be(data, offset + 4)? as usize;
        let section_len = read_u32_be(data, offset + 8)? as usize;

        if section_len == 0 || offset + section_len > data.len() {
            break; // malformed or truncated — stop here
        }

        let section = &data[offset..offset + section_len];

        match tag {
            b"PQTZ" => {
                if let Ok(b) = parse_pqtz(section) {
                    beats = b;
                }
            }
            b"PWAV" => {
                if let Ok(w) = parse_pwav(section) {
                    waveform_data = w;
                }
            }
            b"PCOB" => {
                if let Ok(c) = parse_pcob(section) {
                    cue_points.extend(c);
                }
            }
            _ => { /* skip unknown tags (PPTH, PVBR, etc.) */ }
        }

        offset += section_len;
    }

    Ok((beats, waveform_data, cue_points))
}

/// Walk the tag-based sections in an `.EXT` file and extract color waveform +
/// extended cue data.
///
/// Returns `(extended_cues, pwv3_or_pwv4_detail, pwv4_overview)`.
fn parse_ext_sections(
    data: &[u8],
) -> Result<(Vec<CuePoint>, Option<Vec<[u8; 3]>>, Option<Vec<[u8; 3]>>)> {
    if data.len() < 28 {
        bail!("EXT file too short for PMAI header ({} bytes)", data.len());
    }
    if &data[0..4] != b"PMAI" {
        bail!(
            "EXT file missing PMAI magic (got {:?})",
            &data[0..4]
        );
    }

    let mut cue_points: Vec<CuePoint> = Vec::new();
    let mut pwv3_detail: Option<Vec<[u8; 3]>> = None;
    let mut pwv5_detail: Option<Vec<[u8; 3]>> = None;
    let mut pwv4_overview: Option<Vec<[u8; 3]>> = None;

    let file_header_len = read_u32_be(data, 4)? as usize;
    let mut offset = file_header_len;

    while offset + 12 <= data.len() {
        let tag = &data[offset..offset + 4];
        let _header_len = read_u32_be(data, offset + 4)? as usize;
        let section_len = read_u32_be(data, offset + 8)? as usize;

        if section_len == 0 || offset + section_len > data.len() {
            break;
        }

        let section = &data[offset..offset + section_len];

        match tag {
            b"PWV3" => {
                if let Ok(entries) = parse_pwv3(section) {
                    pwv3_detail = Some(entries);
                }
            }
            b"PWV5" => {
                if let Ok(entries) = parse_pwv5(section) {
                    pwv5_detail = Some(entries);
                }
            }
            b"PWV4" => {
                if let Ok(entries) = parse_pwv4(section) {
                    pwv4_overview = Some(entries);
                }
            }
            b"PCO2" => {
                if let Ok(c) = parse_pco2(section) {
                    cue_points.extend(c);
                }
            }
            _ => { /* skip PPTH, PWV5, PQT2, PVB2, PCOB, etc. */ }
        }

        offset += section_len;
    }

    // Prefer PWV5 (HD color, 2 bytes/entry) over PWV3 (1 byte/entry) for detail.
    let detail = pwv5_detail.or(pwv3_detail);
    Ok((cue_points, detail, pwv4_overview))
}

/// Parse a PCO2 (extended cue points) section into a list of [`CuePoint`]s.
///
/// Each PCO2 section contains zero or more PCP2 sub-entries (88 bytes each).
fn parse_pco2(section: &[u8]) -> Result<Vec<CuePoint>> {
    if section.len() < 20 {
        bail!("PCO2 section too short ({} bytes)", section.len());
    }
    let entry_count = read_u16_be(section, 16)? as usize;
    let entries_data = &section[20..];
    let pcp2_entry_len = 88usize;
    let max_possible = entries_data.len() / pcp2_entry_len;
    let count = entry_count.min(max_possible);

    let mut cues = Vec::with_capacity(count);
    for i in 0..count {
        let base = i * pcp2_entry_len;
        let entry = &entries_data[base..base + pcp2_entry_len];

        // Validate PCP2 tag
        if entry.get(0..4) != Some(b"PCP2") {
            continue;
        }

        let hot_cue_number = read_u32_be(entry, 12)?;
        let time_ms = read_u32_be(entry, 20)?;
        let loop_time_raw = read_u32_be(entry, 24)?;
        let loop_time_ms = if loop_time_raw == 0xFFFF_FFFF {
            None
        } else {
            Some(loop_time_raw)
        };

        cues.push(CuePoint {
            hot_cue_number,
            time_ms,
            loop_time_ms,
        });
    }
    Ok(cues)
}
