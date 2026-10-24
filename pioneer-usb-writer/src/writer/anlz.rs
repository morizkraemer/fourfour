use anyhow::Result;
use std::io::Write;
use std::path::Path;

use crate::models::{AnalysisResult, CuePoint, Track};
use crate::waveform::{Derivation, OVERVIEW_COLUMNS, Tuning, Waveforms, resample_columns, COLUMNS_PER_SECOND, PWV7_MAX};

/// Magic bytes for ANLZ file header
const ANLZ_MAGIC: &[u8; 4] = b"PMAI";
/// Header length (always 28 bytes)
const HEADER_LEN: u32 = 0x1C;

/// Section tag types — .DAT file
const TAG_PATH: &[u8; 4] = b"PPTH";
const TAG_BEAT_GRID: &[u8; 4] = b"PQTZ";
const TAG_WAVEFORM_PREVIEW: &[u8; 4] = b"PWAV";
const TAG_WAVEFORM_TINY: &[u8; 4] = b"PWV2";
const TAG_VBR: &[u8; 4] = b"PVBR";
const TAG_CUE: &[u8; 4] = b"PCOB";

/// Section tag types — .EXT file
const TAG_COLOR_PREVIEW: &[u8; 4] = b"PWV3";
const TAG_COLOR_DETAIL: &[u8; 4] = b"PWV5";
const TAG_COLOR_WAVEFORM: &[u8; 4] = b"PWV4";
const TAG_COLOR_3BAND_DETAIL: &[u8; 4] = b"PWV7";
const TAG_COLOR_3BAND_OVERVIEW: &[u8; 4] = b"PWV6";
const TAG_COLOR_3BAND_GAIN: &[u8; 4] = b"PWVC";
const TAG_CUE_EXTENDED: &[u8; 4] = b"PCO2";
const TAG_BEAT_GRID_EXT: &[u8; 4] = b"PQT2";
const TAG_VBR_EXT: &[u8; 4] = b"PVB2";

/// Write an ANLZ `.DAT` file for a track to `output_path`.
///
/// The file contains the following sections in the order required by rekordbox:
/// `PPTH` (USB path, UTF-16BE) → `PVBR` (VBR seek table) → `PQTZ` (beat grid)
/// → `PWAV` (400-byte monochrome waveform preview) → `PWV2` (100-byte tiny preview)
/// → `PCOB` (hot cues) → `PCOB` (memory cues).
///
/// Parent directories are created automatically.
pub fn write_anlz_dat(
    output_path: &Path,
    track: &Track,
    analysis: &AnalysisResult,
) -> Result<()> {
    // Build all sections first so we know total file size
    // Section order must match rekordbox: PPTH → PVBR → PQTZ → PWAV → PWV2 → PCOB → PCOB
    let hot_cues: Vec<&CuePoint> = analysis.cue_points.iter().filter(|c| c.hot_cue_number > 0).collect();
    let memory_cues: Vec<&CuePoint> = analysis.cue_points.iter().filter(|c| c.hot_cue_number == 0).collect();

    let path_section = build_path_section(&track.usb_path);
    let vbr_section = build_vbr_section();
    let beat_section = build_beat_grid_section(analysis);
    let waves = export_waveforms(track.duration_secs, analysis);
    let waveform_section = build_waveform_preview_section(&waves);
    let tiny_section = build_waveform_tiny_section(&waves);
    let cue_section_1 = build_cue_section(1, &hot_cues);   // hot cues
    let cue_section_2 = build_cue_section(0, &memory_cues); // memory cues

    let total_len = HEADER_LEN as usize
        + path_section.len()
        + vbr_section.len()
        + beat_section.len()
        + waveform_section.len()
        + tiny_section.len()
        + cue_section_1.len()
        + cue_section_2.len();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(output_path)?;

    // File header (28 bytes)
    file.write_all(ANLZ_MAGIC)?;                        // 0x00: magic "PMAI"
    file.write_all(&HEADER_LEN.to_be_bytes())?;          // 0x04: header length
    file.write_all(&(total_len as u32).to_be_bytes())?;  // 0x08: total file size
    file.write_all(&1u32.to_be_bytes())?;                // 0x0C: unknown (1 in rekordbox exports)
    file.write_all(&0x0001_0000u32.to_be_bytes())?;      // 0x10: unknown (0x10000 in rekordbox)
    file.write_all(&0x0001_0000u32.to_be_bytes())?;      // 0x14: unknown (0x10000 in rekordbox)
    file.write_all(&[0u8; 4])?;                          // 0x18: padding

    // Sections (order must match rekordbox: PPTH → PVBR → PQTZ → PWAV → PWV2 → PCOB → PCOB)
    file.write_all(&path_section)?;
    file.write_all(&vbr_section)?;
    file.write_all(&beat_section)?;
    file.write_all(&waveform_section)?;
    file.write_all(&tiny_section)?;
    file.write_all(&cue_section_1)?;
    file.write_all(&cue_section_2)?;

    Ok(())
}

/// PPTH section: UTF-16BE encoded file path.
fn build_path_section(usb_path: &str) -> Vec<u8> {
    // Encode path as UTF-16 Big Endian with null terminator
    let mut utf16: Vec<u16> = usb_path.encode_utf16().collect();
    utf16.push(0); // null terminator — CDJ requires this for path matching
    let path_bytes: Vec<u8> = utf16.iter().flat_map(|c| c.to_be_bytes()).collect();

    let path_len = path_bytes.len() as u32;
    // Section: tag(4) + header_len(4) + file_len(4) + path_len(4) + path_data
    let section_header_len: u32 = 16;
    let section_total_len = section_header_len + path_len;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_PATH);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&path_len.to_be_bytes());
    buf.extend_from_slice(&path_bytes);

    buf
}

/// PQTZ section: beat grid with beat positions.
fn build_beat_grid_section(analysis: &AnalysisResult) -> Vec<u8> {
    let beats = &analysis.beat_grid.beats;
    let beat_count = beats.len() as u32;

    // Section header: tag(4) + header_len(4) + section_len(4) + unknown1(4) + unknown2(4) + beat_count(4) = 24
    // Each beat entry: 2 (beat num) + 2 (tempo) + 4 (time) = 8 bytes
    let section_header_len: u32 = 24;
    let beats_data_len = beat_count * 8;
    let section_total_len = section_header_len + beats_data_len;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_BEAT_GRID);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&[0u8; 4]); // unknown1
    buf.extend_from_slice(&0x0008_0000u32.to_be_bytes()); // unknown2 (beat entry size marker)
    buf.extend_from_slice(&beat_count.to_be_bytes());

    for beat in beats {
        buf.extend_from_slice(&(beat.bar_position as u16).to_be_bytes());
        buf.extend_from_slice(&(beat.tempo as u16).to_be_bytes());
        buf.extend_from_slice(&beat.time_ms.to_be_bytes());
    }

    buf
}

/// The waveform sections of one track, from the band data in `analysis`.
///
/// Without band data the colour sections come out neutral (every band equal), never tinted.
fn export_waveforms(duration_secs: f64, analysis: &AnalysisResult) -> Waveforms {
    let columns = (duration_secs * COLUMNS_PER_SECOND).round() as usize;
    match analysis.color_waveform.as_ref().filter(|cw| !cw.detail.is_empty()) {
        Some(cw) => {
            let mut detail = resample_columns(&cw.detail, columns);
            for band in detail.iter_mut() {
                *band = band.map(|v| v.min(PWV7_MAX));
            }
            Waveforms::from_bands(detail, &Tuning::default(), &Derivation::default())
        }
        None => Waveforms::from_mono_preview(&analysis.waveform.data, columns, &Tuning::default()),
    }
}

fn has_band_data(analysis: &AnalysisResult) -> bool {
    analysis.color_waveform.as_ref().is_some_and(|cw| !cw.detail.is_empty())
}

/// Section with the preview layout shared by `PWAV` and `PWV2`: `len_data, 0x00010000, data`.
fn build_preview_section(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let section_header_len: u32 = 20;
    let section_total_len = section_header_len + data.len() as u32;
    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(tag);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
    buf.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    buf.extend_from_slice(data);
    buf
}

/// PWAV section: 400-byte monochrome waveform preview (3-bit whiteness, 5-bit height).
fn build_waveform_preview_section(waves: &Waveforms) -> Vec<u8> {
    build_preview_section(TAG_WAVEFORM_PREVIEW, &waves.pwav)
}

/// PWV2 section: 100-byte tiny waveform preview (4-bit height).
fn build_waveform_tiny_section(waves: &Waveforms) -> Vec<u8> {
    build_preview_section(TAG_WAVEFORM_TINY, &waves.pwv2)
}

/// Write an ANLZ `.EXT` file for a track to `output_path`.
///
/// The CDJ-3000 requires this file to consider a track fully analyzed. It contains
/// the following sections in the order required by rekordbox:
/// `PPTH` → `PWV3` (color preview waveform) → `PCOB` × 2 (empty in EXT)
/// → `PCO2` × 2 (extended hot/memory cues) → `PQT2` (extended beat grid)
/// → `PWV5` (detailed color waveform) → `PWV4` (color waveform, 1200 entries)
/// → `PVB2` (extended VBR info).
///
/// `PWV3`, `PWV5` and `PWV4` come from the same band data as the `.2EX` file
/// (see [`write_anlz_2ex`]); without band data they are neutral, not tinted.
///
/// Parent directories are created automatically.
pub fn write_anlz_ext(
    output_path: &Path,
    track: &Track,
    analysis: &AnalysisResult,
) -> Result<()> {
    let hot_cues: Vec<&CuePoint> = analysis.cue_points.iter().filter(|c| c.hot_cue_number > 0).collect();
    let memory_cues: Vec<&CuePoint> = analysis.cue_points.iter().filter(|c| c.hot_cue_number == 0).collect();

    // Section order must match rekordbox: PPTH → PWV3 → PCOB → PCOB → PCO2 → PCO2 → PQT2 → PWV5 → PWV4 → PVB2
    let duration_secs = track.duration_secs;
    let path_section = build_path_section(&track.usb_path);
    let waves = export_waveforms(duration_secs, analysis);
    let pwv3_section = build_color_preview_section(&waves);
    let cue_section_1 = build_cue_section(1, &[]);             // EXT PCOBs are always empty
    let cue_section_2 = build_cue_section(0, &[]);             // actual cues go in PCO2
    let cue_ext_1 = build_cue_extended_section(1, &hot_cues);
    let cue_ext_2 = build_cue_extended_section(0, &memory_cues);
    let pqt2_section = build_beat_grid_ext_section(analysis, duration_secs);
    let pwv5_section = build_color_detail_section(&waves);
    let pwv4_section = build_color_waveform_section(&waves);
    let pvb2_section = build_vbr_ext_section();

    let total_len = HEADER_LEN as usize
        + path_section.len()
        + pwv3_section.len()
        + cue_section_1.len()
        + cue_section_2.len()
        + cue_ext_1.len()
        + cue_ext_2.len()
        + pqt2_section.len()
        + pwv5_section.len()
        + pwv4_section.len()
        + pvb2_section.len();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(output_path)?;

    // File header (28 bytes) — same as .DAT
    file.write_all(ANLZ_MAGIC)?;
    file.write_all(&HEADER_LEN.to_be_bytes())?;
    file.write_all(&(total_len as u32).to_be_bytes())?;
    file.write_all(&1u32.to_be_bytes())?;
    file.write_all(&0x0001_0000u32.to_be_bytes())?;
    file.write_all(&0x0001_0000u32.to_be_bytes())?;
    file.write_all(&[0u8; 4])?;

    // Sections
    file.write_all(&path_section)?;
    file.write_all(&pwv3_section)?;
    file.write_all(&cue_section_1)?;
    file.write_all(&cue_section_2)?;
    file.write_all(&cue_ext_1)?;
    file.write_all(&cue_ext_2)?;
    file.write_all(&pqt2_section)?;
    file.write_all(&pwv5_section)?;
    file.write_all(&pwv4_section)?;
    file.write_all(&pvb2_section)?;

    Ok(())
}

/// PWV3 section: color preview waveform (150 entries per second, 1 byte each).
/// Each byte: bits 7-5 = whiteness, bits 4-0 = height.
fn build_color_preview_section(waves: &Waveforms) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let entry_count = waves.pwv3.len() as u32;
    let section_total_len = section_header_len + entry_count;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_PREVIEW);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&1u32.to_be_bytes());
    buf.extend_from_slice(&entry_count.to_be_bytes());
    buf.extend_from_slice(&0x0096_0000u32.to_be_bytes());
    buf.extend_from_slice(&waves.pwv3);
    buf
}

/// PWV5 section: detailed color waveform (150 entries per second, 2 bytes each).
/// Each entry is big-endian `rrrgggbbbhhhhh00`: 3-bit low, mid and high level, 5-bit height.
fn build_color_detail_section(waves: &Waveforms) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let num_entries = waves.pwv5.len() as u32;
    let section_total_len = section_header_len + num_entries * 2;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_DETAIL);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&2u32.to_be_bytes());
    buf.extend_from_slice(&num_entries.to_be_bytes());
    buf.extend_from_slice(&0x0096_0305u32.to_be_bytes());
    for entry in &waves.pwv5 {
        buf.extend_from_slice(&entry.to_be_bytes());
    }
    buf
}

/// PWV4 section: color waveform preview (1200 entries, 6 bytes each).
fn build_color_waveform_section(waves: &Waveforms) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let bytes_per_entry: u32 = 6;
    let num_entries = waves.pwv4.len() as u32;
    let section_total_len = section_header_len + num_entries * bytes_per_entry;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_WAVEFORM);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&bytes_per_entry.to_be_bytes());
    buf.extend_from_slice(&num_entries.to_be_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    for entry in &waves.pwv4 {
        buf.extend_from_slice(entry);
    }
    buf
}

/// PWV7 section: 3-band waveform (150 entries per second, low/mid/high bytes, at most 127).
fn build_pwv7_section(waves: &Waveforms) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let entry_count = waves.pwv7.len() as u32;
    let section_total_len = section_header_len + entry_count * 3;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_3BAND_DETAIL);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&3u32.to_be_bytes());
    buf.extend_from_slice(&entry_count.to_be_bytes());
    buf.extend_from_slice(&0x0096_0000u32.to_be_bytes());
    for entry in &waves.pwv7 {
        buf.extend_from_slice(entry);
    }
    buf
}

/// PWV6 section: 3-band overview (1200 entries, low/mid/high bytes).
fn build_pwv6_section(waves: &Waveforms) -> Vec<u8> {
    let section_header_len: u32 = 20;
    let entry_count = OVERVIEW_COLUMNS as u32;
    let section_total_len = section_header_len + entry_count * 3;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_3BAND_OVERVIEW);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&3u32.to_be_bytes());
    buf.extend_from_slice(&entry_count.to_be_bytes());
    for entry in &waves.pwv6 {
        buf.extend_from_slice(entry);
    }
    buf
}

/// PWVC section: the low, mid and high gain behind the 3-band bytes.
fn build_pwvc_section(waves: &Waveforms) -> Vec<u8> {
    let section_header_len: u32 = 14;
    let section_total_len: u32 = 20;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_3BAND_GAIN);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&0u16.to_be_bytes());
    for gain in waves.gains {
        buf.extend_from_slice(&gain.to_be_bytes());
    }
    buf
}

/// Write an ANLZ `.2EX` file for a track to `output_path`, if `analysis` carries band data.
///
/// Sections: `PPTH` → `PWV7` (3-band detail, 150 entries/s) → `PWV6` (3-band overview,
/// 1200 entries) → `PWVC` (gains). Rekordbox keeps these three in `.2EX`; players that
/// find them in `.EXT` ignore them.
///
/// Returns `Ok(false)` and writes nothing when there is no band data (a stale file at
/// `output_path` is removed), `Ok(true)` when the file was written.
pub fn write_anlz_2ex(
    output_path: &Path,
    track: &Track,
    analysis: &AnalysisResult,
) -> Result<bool> {
    if !has_band_data(analysis) {
        match std::fs::remove_file(output_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        return Ok(false);
    }

    let waves = export_waveforms(track.duration_secs, analysis);
    let path_section = build_path_section(&track.usb_path);
    let pwv7_section = build_pwv7_section(&waves);
    let pwv6_section = build_pwv6_section(&waves);
    let pwvc_section = build_pwvc_section(&waves);

    let total_len = HEADER_LEN as usize
        + path_section.len()
        + pwv7_section.len()
        + pwv6_section.len()
        + pwvc_section.len();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(output_path)?;
    file.write_all(ANLZ_MAGIC)?;
    file.write_all(&HEADER_LEN.to_be_bytes())?;
    file.write_all(&(total_len as u32).to_be_bytes())?;
    file.write_all(&1u32.to_be_bytes())?;
    file.write_all(&0x0001_0000u32.to_be_bytes())?;
    file.write_all(&0x0001_0000u32.to_be_bytes())?;
    file.write_all(&[0u8; 4])?;
    file.write_all(&path_section)?;
    file.write_all(&pwv7_section)?;
    file.write_all(&pwv6_section)?;
    file.write_all(&pwvc_section)?;

    Ok(true)
}

/// PCO2 section: extended cue/loop point container with PCP2 entries.
fn build_cue_extended_section(cue_type: u32, cues: &[&CuePoint]) -> Vec<u8> {
    let section_header_len: u32 = 20;
    let pcp2_entry_len: u32 = 88;
    let section_total_len: u32 = section_header_len + pcp2_entry_len * cues.len() as u32;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_CUE_EXTENDED);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&cue_type.to_be_bytes());                 // 0x0C: type (1=hot, 0=memory)
    buf.extend_from_slice(&(cues.len() as u16).to_be_bytes());     // 0x10: entry count (u16)
    buf.extend_from_slice(&[0u8; 2]);                               // 0x12: padding

    for cue in cues {
        let loop_time = cue.loop_time_ms.unwrap_or(0xFFFFFFFF);

        buf.extend_from_slice(b"PCP2");                             // 0x00: tag
        buf.extend_from_slice(&16u32.to_be_bytes());                // 0x04: header_len
        buf.extend_from_slice(&pcp2_entry_len.to_be_bytes());       // 0x08: entry_len
        buf.extend_from_slice(&cue.hot_cue_number.to_be_bytes());   // 0x0C: hot_cue_number
        buf.push(0x01);                                             // 0x10: type (1 = cue point)
        buf.push(0x00);                                             // 0x11: unknown
        buf.extend_from_slice(&0x03E8u16.to_be_bytes());            // 0x12: loop_denominator
        buf.extend_from_slice(&cue.time_ms.to_be_bytes());          // 0x14: time_ms
        buf.extend_from_slice(&loop_time.to_be_bytes());            // 0x18: loop_time
        buf.extend_from_slice(&0x0001u16.to_be_bytes());            // 0x1C: unknown
        buf.extend_from_slice(&[0u8; 58]);                          // 0x1E: remaining (color/label/padding, zeros)
    }

    buf
}

/// PQT2 section: extended beat grid.
fn build_beat_grid_ext_section(analysis: &AnalysisResult, duration_secs: f64) -> Vec<u8> {
    let beats = &analysis.beat_grid.beats;
    let beat_count = beats.len() as u32;

    // PQT2 header: 56 bytes. Data: 2 bytes per beat.
    let section_header_len: u32 = 56;
    let data_len = beat_count * 2;
    let section_total_len = section_header_len + data_len;

    let track_duration_ms = (duration_secs * 1000.0).round() as u32;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_BEAT_GRID_EXT);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&[0u8; 4]);                          // unknown1
    buf.extend_from_slice(&0x0100_0002u32.to_be_bytes());      // flags/version
    buf.extend_from_slice(&[0u8; 4]);                          // unknown2
    buf.extend_from_slice(&[0u8; 4]);                          // timing field 1
    buf.extend_from_slice(&2u32.to_be_bytes());                // unknown3
    buf.extend_from_slice(&[0u8; 4]);                          // timing field 2
    buf.extend_from_slice(&track_duration_ms.to_be_bytes());   // track duration ms
    buf.extend_from_slice(&beat_count.to_be_bytes());          // beat count
    buf.extend_from_slice(&[0u8; 4]);                          // unknown4 (checksum?)
    buf.extend_from_slice(&[0u8; 4]);                          // unknown5
    buf.extend_from_slice(&[0u8; 4]);                          // unknown6

    // 2-byte entries per beat — simple encoding based on tempo
    for beat in beats {
        let tempo_encoded = (beat.tempo & 0x03FF) as u16;
        let beat_num = ((beat.bar_position as u16) & 0x0F) << 10;
        buf.extend_from_slice(&(beat_num | tempo_encoded).to_be_bytes());
    }

    buf
}

/// PVB2 section: extended VBR info (8032 bytes total, all zeros for FLAC).
fn build_vbr_ext_section() -> Vec<u8> {
    let section_header_len: u32 = 32;
    let section_total_len: u32 = 8032;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_VBR_EXT);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&[0u8; 20]); // header fields (all zeros for FLAC)
    buf.extend_from_slice(&vec![0u8; (section_total_len - section_header_len) as usize]);

    buf
}

/// PVBR section: VBR seek table (1620 bytes total, all zeros for non-VBR files like FLAC).
fn build_vbr_section() -> Vec<u8> {
    let section_header_len: u32 = 16;
    let section_total_len: u32 = 1620; // 0x654 — always this size in rekordbox exports
    let data_len = section_total_len - section_header_len;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_VBR);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&[0u8; 4]); // unknown (completes 16-byte header)
    buf.extend_from_slice(&vec![0u8; data_len as usize]); // all zeros for FLAC

    buf
}

/// PCOB section: cue/loop point container with PCPT entries.
fn build_cue_section(cue_type: u32, cues: &[&CuePoint]) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let pcpt_entry_len: u32 = 56;
    let section_total_len: u32 = section_header_len + pcpt_entry_len * cues.len() as u32;

    let sentinel: u32 = if cues.is_empty() {
        0xFFFFFFFF
    } else if cue_type == 1 {
        0xFFFFFFFF // hot cues always use 0xFFFFFFFF
    } else {
        0x00000000 // memory cues use 0 when entries present
    };

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_CUE);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&cue_type.to_be_bytes());                // type (1=hot, 0=memory)
    buf.extend_from_slice(&(cues.len() as u32).to_be_bytes());    // entry count
    buf.extend_from_slice(&sentinel.to_be_bytes());

    for cue in cues {
        let loop_time = cue.loop_time_ms.unwrap_or(0xFFFFFFFF);

        buf.extend_from_slice(b"PCPT");                             // 0x00: tag
        buf.extend_from_slice(&28u32.to_be_bytes());                // 0x04: header_len
        buf.extend_from_slice(&pcpt_entry_len.to_be_bytes());       // 0x08: entry_len
        buf.extend_from_slice(&cue.hot_cue_number.to_be_bytes());   // 0x0C: hot_cue_number
        buf.extend_from_slice(&[0u8; 4]);                           // 0x10: status
        buf.extend_from_slice(&0x0001u16.to_be_bytes());            // 0x14: unknown
        buf.extend_from_slice(&[0u8; 2]);                           // 0x16: unknown
        buf.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());       // 0x18: sentinel
        buf.push(0x01);                                             // 0x1C: type (1 = cue)
        buf.push(0x00);                                             // 0x1D: unknown
        buf.extend_from_slice(&0x03E8u16.to_be_bytes());            // 0x1E: loop_denominator
        buf.extend_from_slice(&cue.time_ms.to_be_bytes());          // 0x20: time_ms
        buf.extend_from_slice(&loop_time.to_be_bytes());            // 0x24: loop_time
        buf.extend_from_slice(&[0u8; 16]);                          // 0x28: padding
    }

    buf
}

/// Pioneer's ANLZ path hash algorithm.
/// The CDJ computes this hash from the audio file path and uses it to locate
/// the ANLZ file — it ignores the analyze_path field in the PDB.
fn compute_anlz_path_hash(file_path: &str) -> (u16, u32) {
    let mut hash: u32 = 0;
    for c in file_path.chars() {
        let code_unit = (c as u32) & 0xFFFF;
        let temp = hash.wrapping_mul(0x5bc9).wrapping_add(code_unit);
        hash = temp.wrapping_mul(0x93b5).wrapping_add(code_unit);
    }
    let hash_result = hash % 0x30d43; // modulo 200003
    let mut p_value: u16 = 0;
    p_value |= ((hash_result >> 0) & 1) as u16;
    p_value |= ((hash_result >> 1) & 2) as u16;
    p_value |= ((hash_result >> 4) & 4) as u16;
    p_value |= ((hash_result >> 4) & 8) as u16;
    p_value |= ((hash_result >> 5) & 0x10) as u16;
    p_value |= ((hash_result >> 8) & 0x20) as u16;
    p_value |= ((hash_result >> 10) & 0x40) as u16;
    (p_value, hash_result)
}

/// Compute the ANLZ base directory path from a USB-relative audio path.
///
/// Public version for use by sync cleanup where only the path string is available.
pub fn anlz_dir_for_path(usb_path: &str) -> String {
    let (p_value, hash_value) = compute_anlz_path_hash(usb_path);
    format!("PIONEER/USBANLZ/P{:03X}/{:08X}", p_value, hash_value)
}

/// Compute the ANLZ base directory path for a track using Pioneer's hash algorithm.
fn anlz_dir_for_track(track: &Track) -> String {
    anlz_dir_for_path(&track.usb_path)
}

/// Return the USB-relative path to the `ANLZ0000.DAT` file for `track`
/// (e.g. `PIONEER/USBANLZ/P0A3/001F8B2C/ANLZ0000.DAT`).
///
/// The path is computed from the track's USB path using Pioneer's hash algorithm
/// (see [`compute_anlz_path_hash`]). No leading `/`.
pub fn anlz_path_for_track(track: &Track) -> String {
    format!("{}/ANLZ0000.DAT", anlz_dir_for_track(track))
}

/// Return the USB-relative path to the `ANLZ0000.EXT` file for `track`.
///
/// Same directory as [`anlz_path_for_track`], different file extension. No leading `/`.
pub fn anlz_ext_path_for_track(track: &Track) -> String {
    format!("{}/ANLZ0000.EXT", anlz_dir_for_track(track))
}

/// Return the USB-relative path to the `ANLZ0000.2EX` file for `track`.
///
/// Same directory as [`anlz_path_for_track`]; holds the 3-band waveform sections. No leading `/`.
pub fn anlz_2ex_path_for_track(track: &Track) -> String {
    format!("{}/ANLZ0000.2EX", anlz_dir_for_track(track))
}

/// Return the ANLZ `.DAT` path as stored in the PDB `analyze_path` field.
///
/// Identical to [`anlz_path_for_track`] but prefixed with `/`. Note: the CDJ
/// recomputes this path from the hash algorithm and ignores the stored value —
/// this is kept for PDB completeness only.
pub fn anlz_path_for_pdb(track: &Track) -> String {
    format!("/{}", anlz_path_for_track(track))
}
