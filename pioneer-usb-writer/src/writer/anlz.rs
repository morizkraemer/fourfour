use anyhow::Result;
use std::io::Write;
use std::path::Path;

use crate::models::{AnalysisResult, Track};

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

/// Write an ANLZ .DAT file containing path, beat grid, and waveform preview.
pub fn write_anlz_dat(
    output_path: &Path,
    track: &Track,
    analysis: &AnalysisResult,
) -> Result<()> {
    // Build all sections first so we know total file size
    // Section order must match rekordbox: PPTH → PVBR → PQTZ → PWAV → PCOB → PCOB
    let path_section = build_path_section(&track.usb_path);
    let vbr_section = build_vbr_section();
    let beat_section = build_beat_grid_section(analysis);
    let waveform_section = build_waveform_preview_section(analysis);
    let cue_section_1 = build_cue_section(1); // hot cues container (count=1, no entries)
    let cue_section_2 = build_cue_section(0); // memory cues container (count=0)

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

/// Write an ANLZ .EXT file containing color waveforms and extended beat grid.
/// The CDJ-3000 requires this file to consider a track fully analyzed.
pub fn write_anlz_ext(
    output_path: &Path,
    track: &Track,
    analysis: &AnalysisResult,
) -> Result<()> {
    // Section order must match rekordbox: PPTH → PWV3 → PCOB → PCOB → PCO2 → PCO2 → PQT2 → PWV5 → PWV4 → PVB2
    let path_section = build_path_section(&track.usb_path);
    let pwv3_section = build_color_preview_section(analysis);
    let cue_section_1 = build_cue_section(1);
    let cue_section_2 = build_cue_section(0);
    let cue_ext_1 = build_cue_extended_section(1);
    let cue_ext_2 = build_cue_extended_section(0);
    let pqt2_section = build_beat_grid_ext_section(analysis);
    let pwv5_section = build_color_detail_section(analysis);
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

/// PWV3 section: color preview waveform (51200 entries, 1 byte each).
/// Each byte: bits 5-7 = color (frequency band), bits 0-4 = height.
fn build_color_preview_section(analysis: &AnalysisResult) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let entry_count: u32 = 51200; // 0xC800
    let section_total_len = section_header_len + entry_count;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_PREVIEW);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&1u32.to_be_bytes());              // unknown (1 in rekordbox)
    buf.extend_from_slice(&entry_count.to_be_bytes());        // data length
    buf.extend_from_slice(&0x0096_0000u32.to_be_bytes());     // unknown fields

    // Generate color waveform from PWAV data by interpolating 400 → 51200 entries
    for i in 0..51200u32 {
        let pwav_idx = (i * 400 / 51200) as usize;
        let pwav_byte = analysis.waveform.data[pwav_idx];
        let height = pwav_byte & 0x1F; // 5-bit height from PWAV
        let color: u8 = 3 << 5;        // green (mid-frequency) as default color
        buf.push(color | height);
    }

    buf
}

/// PWV5 section: detailed color waveform (51200 entries, 2 bytes each).
fn build_color_detail_section(analysis: &AnalysisResult) -> Vec<u8> {
    let section_header_len: u32 = 24;
    let num_entries: u32 = 51200;
    let data_len = num_entries * 2;
    let section_total_len = section_header_len + data_len;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_COLOR_DETAIL);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&2u32.to_be_bytes());               // bytes per entry
    buf.extend_from_slice(&num_entries.to_be_bytes());         // entry count
    buf.extend_from_slice(&0x0096_0305u32.to_be_bytes());      // unknown fields (from BLAU)

    // Generate 2-byte entries from PWAV data
    for i in 0..51200u32 {
        let pwav_idx = (i * 400 / 51200) as usize;
        let pwav_byte = analysis.waveform.data[pwav_idx];
        let height = pwav_byte & 0x1F;
        // Byte 0: height info, Byte 1: color/intensity info
        buf.push(height);
        buf.push(0x60 | (height >> 1)); // simple color encoding
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

    // Generate 6-byte entries from PWAV data
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

    buf
}

/// PCO2 section: extended cue/loop point container.
fn build_cue_extended_section(count: u32) -> Vec<u8> {
    let section_header_len: u32 = 20;
    let section_total_len: u32 = 20;

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_CUE_EXTENDED);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&count.to_be_bytes());
    buf.extend_from_slice(&[0u8; 4]); // unknown

    buf
}

/// PQT2 section: extended beat grid.
fn build_beat_grid_ext_section(analysis: &AnalysisResult) -> Vec<u8> {
    let beats = &analysis.beat_grid.beats;
    let beat_count = beats.len() as u32;

    // PQT2 header: 56 bytes. Data: 2 bytes per beat.
    let section_header_len: u32 = 56;
    let data_len = beat_count * 2;
    let section_total_len = section_header_len + data_len;

    // Compute track duration from last beat
    let track_duration_ms = beats.last().map(|b| b.time_ms).unwrap_or(0);

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

/// PCOB section: cue/loop point container.
fn build_cue_section(count: u32) -> Vec<u8> {
    // Section: tag(4) + header_len(4) + section_len(4) + count(4) + unknown(4) + sentinel(4) = 24
    let section_header_len: u32 = 24;
    let section_total_len: u32 = 24; // no cue entries, just the header

    let mut buf = Vec::with_capacity(section_total_len as usize);
    buf.extend_from_slice(TAG_CUE);
    buf.extend_from_slice(&section_header_len.to_be_bytes());
    buf.extend_from_slice(&section_total_len.to_be_bytes());
    buf.extend_from_slice(&count.to_be_bytes());
    buf.extend_from_slice(&[0u8; 4]); // unknown
    buf.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // sentinel

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

/// Compute the ANLZ base directory path for a track using Pioneer's hash algorithm.
fn anlz_dir_for_track(track: &Track) -> String {
    let (p_value, hash_value) = compute_anlz_path_hash(&track.usb_path);
    format!("PIONEER/USBANLZ/P{:03X}/{:08X}", p_value, hash_value)
}

/// Compute the ANLZ .DAT path for a track.
pub fn anlz_path_for_track(track: &Track) -> String {
    format!("{}/ANLZ0000.DAT", anlz_dir_for_track(track))
}

/// Compute the ANLZ .EXT path for a track.
pub fn anlz_ext_path_for_track(track: &Track) -> String {
    format!("{}/ANLZ0000.EXT", anlz_dir_for_track(track))
}

/// The path as stored in the PDB track row (with leading /).
pub fn anlz_path_for_pdb(track: &Track) -> String {
    format!("/{}", anlz_path_for_track(track))
}
