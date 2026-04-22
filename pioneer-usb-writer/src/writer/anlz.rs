use anyhow::Result;
use std::io::Write;
use std::path::Path;

use crate::models::{AnalysisResult, CuePoint, Track};

/// Magic bytes for ANLZ file header
const ANLZ_MAGIC: &[u8; 4] = b"PMAI";
/// Header length (always 28 bytes)
const HEADER_LEN: u32 = 0x1C;

/// Section tag types — .DAT file
const TAG_PATH: &[u8; 4] = b"PPTH";
const TAG_BEAT_GRID: &[u8; 4] = b"PQTZ";
const TAG_WAVEFORM_PREVIEW: &[u8; 4] = b"PWAV";
const TAG_VBR: &[u8; 4] = b"PVBR";
const TAG_CUE: &[u8; 4] = b"PCOB";

/// Section tag types — .EXT file
const TAG_COLOR_PREVIEW: &[u8; 4] = b"PWV3";
const TAG_COLOR_DETAIL: &[u8; 4] = b"PWV5";
const TAG_COLOR_WAVEFORM: &[u8; 4] = b"PWV4";
const TAG_CUE_EXTENDED: &[u8; 4] = b"PCO2";
const TAG_BEAT_GRID_EXT: &[u8; 4] = b"PQT2";
const TAG_VBR_EXT: &[u8; 4] = b"PVB2";

/// Write an ANLZ `.DAT` file for a track to `output_path`.
///
/// The file contains the following sections in the order required by rekordbox:
/// `PPTH` (USB path, UTF-16BE) → `PVBR` (VBR seek table) → `PQTZ` (beat grid)
/// → `PWAV` (400-byte monochrome waveform preview) → `PCOB` (hot cues)
/// → `PCOB` (memory cues).
///
/// Parent directories are created automatically.
pub fn write_anlz_dat(
    output_path: &Path,
    track: &Track,
    analysis: &AnalysisResult,
) -> Result<()> {
    // Build all sections first so we know total file size
    // Section order must match rekordbox: PPTH → PVBR → PQTZ → PWAV → PCOB → PCOB
    let hot_cues: Vec<&CuePoint> = analysis.cue_points.iter().filter(|c| c.hot_cue_number > 0).collect();
    let memory_cues: Vec<&CuePoint> = analysis.cue_points.iter().filter(|c| c.hot_cue_number == 0).collect();

    let path_section = build_path_section(&track.usb_path);
    let vbr_section = build_vbr_section();
    let beat_section = build_beat_grid_section(analysis);
    let waveform_section = build_waveform_preview_section(analysis);
    let cue_section_1 = build_cue_section(1, &hot_cues);   // hot cues
    let cue_section_2 = build_cue_section(0, &memory_cues); // memory cues

    let total_len = HEADER_LEN as usize
        + path_section.len()
        + vbr_section.len()
        + beat_section.len()
        + waveform_section.len()
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

    // Sections (order must match rekordbox: PPTH → PVBR → PQTZ → PWAV → PCOB → PCOB)
    file.write_all(&path_section)?;
    file.write_all(&vbr_section)?;
    file.write_all(&beat_section)?;
    file.write_all(&waveform_section)?;
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

/// PWAV section: 400-byte monochrome waveform preview.
fn build_waveform_preview_section(analysis: &AnalysisResult) -> Vec<u8> {
    // Section header: tag(4) + header_len(4) + file_len(4) + entry_count(4) + pad(4) = 20
    let section_header_len: u32 = 20;
    let entry_count: u32 = 400;
    let section_total_len = section_header_len + entry_count;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_WAVEFORM_PREVIEW);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&entry_count.to_be_bytes());
    buf.extend_from_slice(&[0u8; 4]); // padding
    buf.extend_from_slice(&analysis.waveform.data);

    buf
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
/// Color waveform data is currently derived from the monochrome PWAV values
/// (faked green/white tint); spectral analysis is not yet implemented.
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
    let pwv3_section = build_color_preview_section(analysis, duration_secs);
    let cue_section_1 = build_cue_section(1, &[]);             // EXT PCOBs are always empty
    let cue_section_2 = build_cue_section(0, &[]);             // actual cues go in PCO2
    let cue_ext_1 = build_cue_extended_section(1, &hot_cues);
    let cue_ext_2 = build_cue_extended_section(0, &memory_cues);
    let pqt2_section = build_beat_grid_ext_section(analysis, duration_secs);
    let pwv5_section = build_color_detail_section(analysis, duration_secs);
    let pwv4_section = build_color_waveform_section(analysis);
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

/// PWV3 section: color preview waveform (variable entries, 1 byte each).
/// Entry count = duration_secs * 150 to match rekordbox's resolution.
/// Each byte: bits 7-5 = color (1=red/bass, 2=blue/high, 4=green/mid, 7=white), bits 4-0 = height.
fn build_color_preview_section(analysis: &AnalysisResult, duration_secs: f64) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let entry_count: u32 = (duration_secs * 150.0).round() as u32;
    let section_total_len = section_header_len + entry_count;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_PREVIEW);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&1u32.to_be_bytes());              // unknown (1 in rekordbox)
    buf.extend_from_slice(&entry_count.to_be_bytes());        // data length
    buf.extend_from_slice(&0x0096_0000u32.to_be_bytes());     // unknown fields

    if let Some(cw) = &analysis.color_waveform {
        let src_len = cw.detail.len() as u64;
        for i in 0..entry_count {
            // Use u64 to avoid overflow for long tracks (u32*u32 overflows at ~7 min).
            let src_idx = if src_len > 0 {
                (i as u64 * src_len / entry_count as u64) as usize
            } else {
                0
            };
            let [low, mid, high] = if src_idx < cw.detail.len() {
                cw.detail[src_idx]
            } else {
                [0, 0, 0]
            };
            let max_amp = low.max(mid).max(high);
            // Scale max amplitude (0-255) to 5-bit height (0-31)
            let height = (max_amp as u32 * 31 / 255) as u8;
            // Determine dominant band → color code
            let color_bits: u8 = if low >= mid && low >= high {
                1 // red — bass dominant
            } else if high >= low && high >= mid {
                2 // blue — treble dominant
            } else {
                4 // green — mid dominant
            };
            buf.push((color_bits << 5) | (height & 0x1F));
        }
    } else {
        // Fallback: interpolate from mono PWAV with white color
        for i in 0..entry_count {
            let pwav_idx = (i * 400 / entry_count) as usize;
            let pwav_byte = analysis.waveform.data[pwav_idx];
            let height = pwav_byte & 0x1F; // 5-bit height from PWAV
            let color: u8 = 7 << 5;        // 0xe0 = white/full-spectrum (matches rekordbox)
            buf.push(color | height);
        }
    }

    buf
}

/// PWV5 section: detailed color waveform (variable entries, 2 bytes each).
/// Entry count matches PWV3 (duration_secs * 150).
fn build_color_detail_section(analysis: &AnalysisResult, duration_secs: f64) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let num_entries: u32 = (duration_secs * 150.0).round() as u32;
    let data_len = num_entries * 2;
    let section_total_len = section_header_len + data_len;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_DETAIL);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&2u32.to_be_bytes());               // bytes per entry
    buf.extend_from_slice(&num_entries.to_be_bytes());         // entry count
    buf.extend_from_slice(&0x0096_0305u32.to_be_bytes());      // unknown fields (confirmed from rekordbox)

    if let Some(cw) = &analysis.color_waveform {
        let src_len = cw.detail.len() as u64;
        for i in 0..num_entries {
            // Use u64 to avoid overflow for long tracks (u32*u32 overflows at ~7 min).
            let src_idx = if src_len > 0 {
                (i as u64 * src_len / num_entries as u64) as usize
            } else {
                0
            };
            let [low, mid, high] = if src_idx < cw.detail.len() {
                cw.detail[src_idx]
            } else {
                [0, 0, 0]
            };
            let max_amp = low.max(mid).max(high);
            // Scale amplitude (0-255) to 5-bit height (0-31) for bits 14:10.
            let height = (max_amp as u32 * 31 / 255) as u16;
            // Color indicator in bits 4:0 — must match parse_pwv5 ranges:
            // 0-2 neutral, 3-7 bass dominant, 8-12 treble dominant, 13+ mid dominant.
            let ch3: u16 = if max_amp == 0 {
                0 // silence
            } else if low >= mid && low >= high {
                5  // bass dominant
            } else if high >= low && high >= mid {
                10 // treble dominant
            } else {
                15 // mid dominant
            };
            let word: u16 = (height << 10) | ch3;
            buf.extend_from_slice(&word.to_le_bytes());
        }
    } else {
        // Fallback: derive from mono PWAV, neutral color.
        for i in 0..num_entries {
            let pwav_idx = (i * 400 / num_entries) as usize;
            let pwav_byte = analysis.waveform.data[pwav_idx];
            let height = (pwav_byte & 0x1F) as u16; // 5-bit height (0-31)
            let word: u16 = height << 10; // neutral color (ch3 = 0), height in bits 14:10
            buf.extend_from_slice(&word.to_le_bytes());
        }
    }

    buf
}

/// PWV4 section: color waveform preview (1200 entries, 6 bytes each).
fn build_color_waveform_section(analysis: &AnalysisResult) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let num_entries: u32 = 1200; // 0x04B0
    let bytes_per_entry: u32 = 6;
    let data_len = num_entries * bytes_per_entry;
    let section_total_len = section_header_len + data_len;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_WAVEFORM);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&bytes_per_entry.to_be_bytes());     // bytes per entry
    buf.extend_from_slice(&num_entries.to_be_bytes());         // entry count
    buf.extend_from_slice(&[0u8; 4]);                          // unknown

    if let Some(cw) = &analysis.color_waveform {
        // overview is expected to be exactly 1200 entries; use direct indexing,
        // falling back to interpolation if lengths differ.
        let src_len = cw.overview.len() as u32;
        for i in 0..1200u32 {
            let src_idx = if src_len > 0 {
                (i * src_len / 1200) as usize
            } else {
                0
            };
            let [low, mid, high] = if src_idx < cw.overview.len() {
                cw.overview[src_idx]
            } else {
                [0, 0, 0]
            };
            let max_height = low.max(mid).max(high);
            // 6 bytes per entry: [red(bass), green(mid), blue(high), max_height, high/2, whiteness]
            buf.push(low);           // red — bass channel
            buf.push(mid);           // green — mid channel
            buf.push(high);          // blue — treble channel
            buf.push(max_height);    // overall height
            buf.push(high / 2);      // secondary blue
            buf.push(0);             // whiteness
        }
    } else {
        // Fallback: derive color from mono PWAV (fake green tint)
        for i in 0..1200u32 {
            let pwav_idx = (i * 400 / 1200) as usize;
            let pwav_byte = analysis.waveform.data[pwav_idx];
            let height = pwav_byte & 0x1F;
            // 6 bytes per entry: [red, green, blue, height, blue2, white]
            buf.push(height / 2);    // red component
            buf.push(height);        // green component
            buf.push(height / 2);    // blue component
            buf.push(height);        // height
            buf.push(height / 3);    // blue2
            buf.push(0);             // whiteness
        }
    }

    buf
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

/// Return the ANLZ `.DAT` path as stored in the PDB `analyze_path` field.
///
/// Identical to [`anlz_path_for_track`] but prefixed with `/`. Note: the CDJ
/// recomputes this path from the hash algorithm and ignores the stored value —
/// this is kept for PDB completeness only.
pub fn anlz_path_for_pdb(track: &Track) -> String {
    format!("/{}", anlz_path_for_track(track))
}
