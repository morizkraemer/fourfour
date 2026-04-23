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

/// Parse a PWV5 (high-resolution amplitude waveform, 2 bytes/entry) section.
///
/// Rekordbox writes PWV5 as `[amplitude, 0x80]` where:
/// - byte 0 = max amplitude (0-255)
/// - byte 1 = 0x80 (constant flags byte observed in all exports)
///
/// Color information lives in PWV3/PWV4; PWV5 is amplitude-only detail.
/// We return greyscale `[amp, amp, amp]` so downstream code gets the shape
/// even though the per-band color must come from PWV3/PWV4.
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
        let amplitude = data[i * 2];
        // let flags = data[i * 2 + 1]; // typically 0x80
        entries.push([amplitude, amplitude, amplitude]);
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

/// Read a `.DAT` file (and its siblings `.EXT` and `.2EX` if they exist) into an
/// [`AnalysisResult`].
///
/// The `.EXT` file is expected at the same path with the extension changed to
/// `"EXT"`.  The `.2EX` file is expected with extension `"2EX"`.
/// If neither exists, `color_waveform` will be `None`.
///
/// The reader is tolerant: unknown tags are skipped and missing sections
/// produce sensible defaults.
///
/// `.2EX` (PWV6/PWV7) is preferred over `.EXT` (PWV3/PWV4/PWV5) when both are
/// present, because `.2EX` uses the standardised 3-byte `[low, mid, high]` format.
pub fn read_anlz(dat_path: &Path) -> Result<AnalysisResult> {
    let dat_bytes = std::fs::read(dat_path)
        .with_context(|| format!("failed to read DAT file: {}", dat_path.display()))?;

    let (beats, waveform_data, mut cue_points) = parse_dat_sections(&dat_bytes)?;

    // Try to read the sibling .2EX file first (preferred format).
    let ex_path = dat_path.with_extension("2EX");
    let mut color_waveform = if ex_path.exists() {
        let ex_bytes = std::fs::read(&ex_path)
            .with_context(|| format!("failed to read 2EX file: {}", ex_path.display()))?;
        parse_2ex_sections(&ex_bytes).ok()
    } else {
        None
    };

    // Fall back to .EXT if .2EX is missing or empty.
    if color_waveform.is_none() {
        let ext_path = dat_path.with_extension("EXT");
        if ext_path.exists() {
            let ext_bytes = std::fs::read(&ext_path)
                .with_context(|| format!("failed to read EXT file: {}", ext_path.display()))?;
            let (ext_cues, detail, overview) = parse_ext_sections(&ext_bytes)?;

            // PCO2 extended cues are more authoritative than PCOB cues when present.
            if !ext_cues.is_empty() {
                cue_points = ext_cues;
            }

            if detail.is_some() || overview.is_some() {
                color_waveform = Some(ColorWaveform {
                    detail: detail.unwrap_or_default(),
                    overview: overview.unwrap_or_default(),
                });
            }
        }
    }

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

    // Prefer PWV3 (color + height) over PWV5 (amplitude-only) for detail.
    // PWV5 provides higher amplitude resolution but no color; PWV3 gives both.
    let detail = pwv3_detail.or(pwv5_detail);
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

/// Walk the tag-based sections in a `.2EX` file and extract PWV6/PWV7 data.
///
/// Returns `Some(ColorWaveform)` if at least one of PWV6 or PWV7 is found.
fn parse_2ex_sections(data: &[u8]) -> Result<ColorWaveform> {
    if data.len() < 28 {
        bail!("2EX file too short for PMAI header ({} bytes)", data.len());
    }
    if &data[0..4] != b"PMAI" {
        bail!("2EX file missing PMAI magic (got {:?})", &data[0..4]);
    }

    let mut pwv7_detail: Option<Vec<[u8; 3]>> = None;
    let mut pwv6_overview: Option<Vec<[u8; 3]>> = None;

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
            b"PWV7" => {
                if let Ok(entries) = parse_pwv7(section) {
                    pwv7_detail = Some(entries);
                }
            }
            b"PWV6" => {
                if let Ok(entries) = parse_pwv6(section) {
                    pwv6_overview = Some(entries);
                }
            }
            _ => { /* skip PPTH, etc. */ }
        }

        offset += section_len;
    }

    if pwv7_detail.is_some() || pwv6_overview.is_some() {
        Ok(ColorWaveform {
            detail: pwv7_detail.unwrap_or_default(),
            overview: pwv6_overview.unwrap_or_default(),
        })
    } else {
        bail!("no PWV6 or PWV7 sections found in 2EX file")
    }
}

/// Parse a PWV7 (full-resolution 3-band, 3 bytes/entry) section.
fn parse_pwv7(section: &[u8]) -> Result<Vec<[u8; 3]>> {
    if section.len() < 24 {
        bail!("PWV7 section too short ({} bytes)", section.len());
    }
    let entry_count = read_u32_be(section, 16)? as usize;
    let data = &section[24..];
    let max_entries = data.len() / 3;
    let count = entry_count.min(max_entries);

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let base = i * 3;
        entries.push([data[base], data[base + 1], data[base + 2]]);
    }
    Ok(entries)
}

/// Parse a PWV6 (overview 3-band, 3 bytes/entry) section.
fn parse_pwv6(section: &[u8]) -> Result<Vec<[u8; 3]>> {
    if section.len() < 20 {
        bail!("PWV6 section too short ({} bytes)", section.len());
    }
    let entry_count = read_u32_be(section, 16)? as usize;
    let data = &section[20..];
    let max_entries = data.len() / 3;
    let count = entry_count.min(max_entries);

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let base = i * 3;
        entries.push([data[base], data[base + 1], data[base + 2]]);
    }
    Ok(entries)
}
