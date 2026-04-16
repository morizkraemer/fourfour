use anyhow::{bail, Result};
use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use crate::models::{Playlist, Track};
use crate::writer::anlz;

const PAGE_SIZE: u32 = 4096;
const HEAP_START: usize = 0x28;

// Reference history data pages — CDJ requires populated history tables for database recognition
const REFERENCE_HISTORY_PLAYLISTS: &[u8; 4096] = include_bytes!("reference_history_p36.bin");
const REFERENCE_HISTORY_ENTRIES: &[u8; 4096] = include_bytes!("reference_history_p38.bin");
const REFERENCE_HISTORY: &[u8; 4096] = include_bytes!("reference_history_p40.bin");

// ── DeviceSQL String Encoding ──────────────────────────────────────

fn encode_string(s: &str) -> Vec<u8> {
    if s.is_empty() {
        return vec![0x03]; // empty string marker
    }

    let is_ascii = s.bytes().all(|b| b < 128);

    if !is_ascii {
        // UTF-16LE encoding
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let byte_len = utf16.len() * 2;
        let total_len = (byte_len + 4) as u16;
        let mut out = Vec::with_capacity(4 + byte_len);
        out.push(0x90);
        out.extend_from_slice(&total_len.to_le_bytes());
        out.push(0x00);
        for unit in utf16 {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        return out;
    }

    let bytes = s.as_bytes();
    let len = bytes.len();

    if len <= 126 {
        // Short ASCII: header = ((len + 1) << 1) | 1
        let header = (((len + 1) << 1) as u8) | 1;
        let mut v = Vec::with_capacity(1 + len);
        v.push(header);
        v.extend_from_slice(bytes);
        v
    } else {
        // Long ASCII: 0x40, u16 LE (content_len + 4), 0x00, content
        let total = (len + 4) as u16;
        let mut v = Vec::with_capacity(4 + len);
        v.push(0x40);
        v.extend_from_slice(&total.to_le_bytes());
        v.push(0x00);
        v.extend_from_slice(bytes);
        v
    }
}

// ── Page Layout ────────────────────────────────────────────────────
// Fixed page assignments matching rekordbox CDJ-3000 exports.
// Verified by binary comparison against a rekordbox-generated export.pdb.
// Key insight: pages 1-40 contain the actual data. Page 0 has table pointers.
// The `last_page` in table pointers controls chain traversal — must be exact.

struct TableLayout {
    table_type: u32,
    header_page: u32,
    data_page: u32,      // 0 = no data page written
    empty_candidate: u32,
    last_page: u32,       // for table pointer in page 0 (controls CDJ chain traversal)
}

const LAYOUTS: &[TableLayout] = &[
    TableLayout { table_type: 0x00, header_page: 1,  data_page: 2,  empty_candidate: 49, last_page: 2  }, // tracks
    TableLayout { table_type: 0x01, header_page: 3,  data_page: 4,  empty_candidate: 4,  last_page: 3  }, // genres (ec=data slot, last=header)
    TableLayout { table_type: 0x02, header_page: 5,  data_page: 6,  empty_candidate: 47, last_page: 6  }, // artists
    TableLayout { table_type: 0x03, header_page: 7,  data_page: 8,  empty_candidate: 48, last_page: 8  }, // albums
    TableLayout { table_type: 0x04, header_page: 9,  data_page: 0,  empty_candidate: 10, last_page: 9  }, // labels
    TableLayout { table_type: 0x05, header_page: 11, data_page: 12, empty_candidate: 12, last_page: 11 }, // keys (ec=data slot, last=header)
    TableLayout { table_type: 0x06, header_page: 13, data_page: 14, empty_candidate: 42, last_page: 14 }, // colors
    TableLayout { table_type: 0x07, header_page: 15, data_page: 16, empty_candidate: 46, last_page: 16 }, // playlist_tree
    TableLayout { table_type: 0x08, header_page: 17, data_page: 18, empty_candidate: 51, last_page: 18 }, // playlist_entries
    TableLayout { table_type: 0x09, header_page: 19, data_page: 0,  empty_candidate: 20, last_page: 19 }, // unknown09
    TableLayout { table_type: 0x0A, header_page: 21, data_page: 0,  empty_candidate: 22, last_page: 21 }, // unknown0a
    TableLayout { table_type: 0x0B, header_page: 23, data_page: 0,  empty_candidate: 24, last_page: 23 }, // unknown0b
    TableLayout { table_type: 0x0C, header_page: 25, data_page: 0,  empty_candidate: 26, last_page: 25 }, // unknown0c
    TableLayout { table_type: 0x0D, header_page: 27, data_page: 28, empty_candidate: 50, last_page: 28 }, // artwork (empty data page)
    TableLayout { table_type: 0x0E, header_page: 29, data_page: 0,  empty_candidate: 30, last_page: 29 }, // unknown0e
    TableLayout { table_type: 0x0F, header_page: 31, data_page: 0,  empty_candidate: 32, last_page: 31 }, // unknown0f
    TableLayout { table_type: 0x10, header_page: 33, data_page: 34, empty_candidate: 43, last_page: 34 }, // columns
    TableLayout { table_type: 0x11, header_page: 35, data_page: 36, empty_candidate: 44, last_page: 36 }, // history_playlists
    TableLayout { table_type: 0x12, header_page: 37, data_page: 38, empty_candidate: 45, last_page: 38 }, // history_entries
    TableLayout { table_type: 0x13, header_page: 39, data_page: 40, empty_candidate: 41, last_page: 40 }, // history
];

// ── Helpers ────────────────────────────────────────────────────────

fn align_to_4(heap: &mut Vec<u8>) {
    let r = heap.len() % 4;
    if r != 0 {
        heap.extend(std::iter::repeat(0u8).take(4 - r));
    }
}

fn row_group_bytes(num_rows: usize) -> usize {
    let full = num_rows / 16;
    let partial = num_rows % 16;
    full * 36 + if partial > 0 { partial * 2 + 4 } else { 0 }
}

fn seek_to_page(file: &mut std::fs::File, page: u32) -> Result<u64> {
    let offset = page as u64 * PAGE_SIZE as u64;
    file.seek(SeekFrom::Start(offset))?;
    Ok(offset)
}

// ── Page Writing ───────────────────────────────────────────────────

fn write_page_header(
    file: &mut std::fs::File,
    page_index: u32,
    table_type: u32,
    next_page: u32,
    num_rows: usize,
    page_flags: u8,
    sequence: u32,
    unknown7: u16,
) -> Result<()> {
    let is_header = page_flags == 0x64;

    let num_rows_small = num_rows.min(255) as u8;
    let unk3 = ((num_rows % 8) * 0x20) as u8;
    let unk4 = if num_rows >= 10 {
        let num_groups = ((num_rows + 15) / 16) as u8;
        if table_type == 0x10 { num_groups + 1 } else { num_groups }
    } else {
        0
    };

    // Header pages use sentinel values 0x1fff; data pages use actual row counts
    // Columns table (0x10) uses a different format: unknown5=num_rows, num_rows_large=0
    let (num_rows_large, unknown5, unknown6): (u16, u16, u16) = if is_header {
        (0x1fff, 0x1fff, 0x03ec)
    } else if table_type == 0x10 {
        // Columns page: CDJ requires unknown5=num_rows, num_rows_large=0
        (0, num_rows as u16, 0x0000)
    } else {
        let nrl = if num_rows == 0 { 0 } else { (num_rows - 1) as u16 };
        (nrl, 0x0001, 0x0000)
    };

    file.write_all(&[0u8; 4])?;                          // 0x00: gap
    file.write_all(&page_index.to_le_bytes())?;           // 0x04: page_index
    file.write_all(&table_type.to_le_bytes())?;           // 0x08: type
    file.write_all(&next_page.to_le_bytes())?;            // 0x0C: next_page
    file.write_all(&sequence.to_le_bytes())?;             // 0x10: sequence/unknown1
    file.write_all(&[0u8; 4])?;                          // 0x14: unknown2
    file.write_all(&[num_rows_small, unk3, unk4])?;       // 0x18-0x1A
    file.write_all(&[page_flags])?;                       // 0x1B: flags
    file.write_all(&[0u8; 2])?;                          // 0x1C: free_size (patched later)
    file.write_all(&[0u8; 2])?;                          // 0x1E: used_size (patched later)
    file.write_all(&unknown5.to_le_bytes())?;             // 0x20: unknown5
    file.write_all(&num_rows_large.to_le_bytes())?;       // 0x22: num_rows_large
    file.write_all(&unknown6.to_le_bytes())?;             // 0x24: unknown6
    file.write_all(&unknown7.to_le_bytes())?;             // 0x26: unknown7
    Ok(())
}

fn patch_page_usage(file: &mut std::fs::File, page_start: u64, free: u16, used: u16) -> Result<()> {
    file.seek(SeekFrom::Start(page_start + 0x1C))?;
    file.write_all(&free.to_le_bytes())?;
    file.write_all(&used.to_le_bytes())?;
    file.seek(SeekFrom::Start(page_start + PAGE_SIZE as u64))?;
    Ok(())
}

fn write_row_groups(file: &mut std::fs::File, num_rows: usize, offsets: &[u16]) -> Result<()> {
    let num_groups = (num_rows + 15) / 16;
    for group_idx in (0..num_groups).rev() {
        let start = group_idx * 16;
        let end = (start + 16).min(num_rows);
        let rows_in_group = end - start;

        let mut flags: u16 = 0;
        for i in 0..rows_in_group {
            flags |= 1 << i;
        }

        // Write offsets in reverse
        for i in (start..end).rev() {
            file.write_all(&offsets[i].to_le_bytes())?;
        }

        // Write flags
        file.write_all(&flags.to_le_bytes())?;

        // Unknown: 0 for full groups, highest bit for partial
        let unknown = if flags == 0xFFFF || flags == 0 {
            0u16
        } else {
            let leading = flags.leading_zeros() as u16;
            1u16 << (15 - leading)
        };
        file.write_all(&unknown.to_le_bytes())?;
    }
    Ok(())
}

fn write_header_page(
    file: &mut std::fs::File,
    page_index: u32,
    table_type: u32,
    next_page: u32,
    first_data_page: Option<u32>,
) -> Result<()> {
    let page_start = seek_to_page(file, page_index)?;

    // Tracks (0x00) and History (0x13) headers have unknown7=1 in rekordbox exports
    let (header_seq, unk7) = match table_type {
        0x00 => (44u32, 1u16), // Tracks
        0x13 => (17u32, 1u16), // History
        _ => (1u32, 0u16),
    };

    // Page header with flags=0x64 (header page)
    write_page_header(file, page_index, table_type, next_page, 0, 0x64, header_seq, unk7)?;

    // Header page content at 0x28
    file.write_all(&page_index.to_le_bytes())?;           // +0x00
    let fdp = first_data_page.unwrap_or(0x03FF_FFFF);
    file.write_all(&fdp.to_le_bytes())?;                   // +0x04
    file.write_all(&0x03FF_FFFFu32.to_le_bytes())?;        // +0x08: sentinel
    file.write_all(&0u32.to_le_bytes())?;                  // +0x0C

    // Tracks and History have content+0x10 = 0x1FFF0001 and a special value at +0x14
    match table_type {
        0x00 => {
            // Tracks: content+0x10 = 0x1FFF0001, content+0x14 = 0x00000010
            file.write_all(&0x1FFF_0001u32.to_le_bytes())?;
            file.write_all(&0x0000_0010u32.to_le_bytes())?;
            let pattern: [u8; 4] = [0xF8, 0xFF, 0xFF, 0x1F];
            let bytes_written = 0x28 + 24;
            let remaining = PAGE_SIZE as usize - bytes_written - 20;
            let pattern_count = remaining / 4;
            for _ in 0..pattern_count {
                file.write_all(&pattern)?;
            }
            file.write_all(&[0u8; 20])?;
        }
        0x13 => {
            // History: content+0x10 = 0x1FFF0001, content+0x14 = 0x00000140
            file.write_all(&0x1FFF_0001u32.to_le_bytes())?;
            file.write_all(&0x0000_0140u32.to_le_bytes())?;
            let pattern: [u8; 4] = [0xF8, 0xFF, 0xFF, 0x1F];
            let bytes_written = 0x28 + 24;
            let remaining = PAGE_SIZE as usize - bytes_written - 20;
            let pattern_count = remaining / 4;
            for _ in 0..pattern_count {
                file.write_all(&pattern)?;
            }
            file.write_all(&[0u8; 20])?;
        }
        _ => {
            // All other tables: content+0x10 = 0x1FFF0000, then fill pattern
            file.write_all(&0x1FFF_0000u32.to_le_bytes())?;
            let pattern: [u8; 4] = [0xF8, 0xFF, 0xFF, 0x1F];
            let bytes_written = 0x28 + 20;
            let remaining = PAGE_SIZE as usize - bytes_written - 20;
            let pattern_count = remaining / 4;
            for _ in 0..pattern_count {
                file.write_all(&pattern)?;
            }
            file.write_all(&[0u8; 20])?;
        }
    }

    // Patch free/used to 0 for header pages
    patch_page_usage(file, page_start, 0, 0)?;
    Ok(())
}

fn write_data_page(
    file: &mut std::fs::File,
    page_index: u32,
    table_type: u32,
    next_page: u32,
    heap: &[u8],
    row_offsets: &[u16],
    sequence: u32,
) -> Result<()> {
    let num_rows = row_offsets.len();
    let rg_bytes = row_group_bytes(num_rows);
    let padding = PAGE_SIZE as usize - HEAP_START - heap.len() - rg_bytes;

    let page_start = seek_to_page(file, page_index)?;
    write_page_header(file, page_index, table_type, next_page, num_rows, 0x24, sequence, 0)?;

    // Write heap data
    file.write_all(heap)?;

    // Write padding between heap and row groups
    if padding > 0 {
        file.write_all(&vec![0u8; padding])?;
    }

    // Write row groups
    write_row_groups(file, num_rows, row_offsets)?;

    // Patch free/used
    let free = padding as u16;
    let used = heap.len() as u16;
    patch_page_usage(file, page_start, free, used)?;

    Ok(())
}

/// Write a blank data page with proper page header (not all zeros).
/// All-zero pages are dangerous because next_page=0 leads the CDJ to
/// follow the chain into the file header.
fn write_blank_data_page(
    file: &mut std::fs::File,
    page_index: u32,
    table_type: u32,
    next_page: u32,
) -> Result<()> {
    let page_start = seek_to_page(file, page_index)?;
    // Write page header with 0 rows, flags=0x24 (data page)
    write_page_header(file, page_index, table_type, next_page, 0, 0x24, 1, 0)?;
    // Pad rest of page with zeros
    let remaining = PAGE_SIZE as usize - HEAP_START;
    file.write_all(&vec![0u8; remaining])?;
    // Patch free/used: all space is free, nothing used
    let free = (PAGE_SIZE as usize - HEAP_START) as u16;
    patch_page_usage(file, page_start, free, 0)?;
    Ok(())
}

// ── Table Building ─────────────────────────────────────────────────

fn build_genre_rows(genres: &[String]) -> (Vec<u8>, Vec<u16>) {
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for (i, g) in genres.iter().enumerate() {
        offsets.push(heap.len() as u16);
        heap.extend_from_slice(&((i + 1) as u32).to_le_bytes());
        heap.extend_from_slice(&encode_string(g));
        align_to_4(&mut heap);
    }
    (heap, offsets)
}

fn build_artist_rows(artists: &[String]) -> (Vec<u8>, Vec<u16>) {
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for (i, a) in artists.iter().enumerate() {
        let row_start = heap.len();
        offsets.push(row_start as u16);
        heap.extend_from_slice(&0x0060u16.to_le_bytes()); // subtype
        heap.extend_from_slice(&((i as u16) * 0x20).to_le_bytes()); // index_shift
        heap.extend_from_slice(&((i + 1) as u32).to_le_bytes()); // id
        heap.push(0x03); // empty string marker
        heap.push(0x0A); // ofs_name_near = 10
        heap.extend_from_slice(&encode_string(a));
        // Pad to min 28 bytes
        let row_size = heap.len() - row_start;
        if row_size < 28 {
            heap.extend(std::iter::repeat(0u8).take(28 - row_size));
        }
    }
    (heap, offsets)
}

fn build_album_rows(albums: &[String], album_artist_map: &HashMap<String, u32>) -> (Vec<u8>, Vec<u16>) {
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for (i, a) in albums.iter().enumerate() {
        let row_start = heap.len();
        offsets.push(row_start as u16);
        let artist_id = *album_artist_map.get(a).unwrap_or(&0);
        heap.extend_from_slice(&0x0080u16.to_le_bytes()); // subtype
        heap.extend_from_slice(&((i as u16) * 0x20).to_le_bytes()); // index_shift
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown
        heap.extend_from_slice(&artist_id.to_le_bytes()); // artist_id
        heap.extend_from_slice(&((i + 1) as u32).to_le_bytes()); // album id
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown
        heap.push(0x03); // empty string marker
        heap.push(0x16); // ofs_name_near = 22
        heap.extend_from_slice(&encode_string(a));
        let row_size = heap.len() - row_start;
        if row_size < 40 {
            heap.extend(std::iter::repeat(0u8).take(40 - row_size));
        }
    }
    (heap, offsets)
}

fn build_key_rows() -> (Vec<u8>, Vec<u16>) {
    // All 24 musical keys
    let key_names = [
        "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
        "Cm", "Dbm", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "Abm", "Am", "Bbm", "Bm",
    ];
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for (i, name) in key_names.iter().enumerate() {
        offsets.push(heap.len() as u16);
        let id = (i + 1) as u32;
        heap.extend_from_slice(&id.to_le_bytes()); // id
        heap.extend_from_slice(&id.to_le_bytes()); // id2
        heap.extend_from_slice(&encode_string(name));
        align_to_4(&mut heap);
    }
    (heap, offsets)
}

fn build_color_rows() -> (Vec<u8>, Vec<u16>) {
    let colors = [
        (1, "Pink"), (2, "Red"), (3, "Orange"), (4, "Yellow"),
        (5, "Green"), (6, "Aqua"), (7, "Blue"), (8, "Purple"),
    ];
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for &(idx, name) in &colors {
        offsets.push(heap.len() as u16);
        heap.extend_from_slice(&0u32.to_le_bytes()); // unknown1
        heap.push(0x00); // unknown2
        heap.push(idx as u8); // color_index
        heap.extend_from_slice(&0u16.to_le_bytes()); // unknown3
        heap.extend_from_slice(&encode_string(name));
        align_to_4(&mut heap);
    }
    (heap, offsets)
}

fn build_track_rows(
    tracks: &[Track],
    artist_map: &HashMap<String, u32>,
    album_map: &HashMap<String, u32>,
    genre_map: &HashMap<String, u32>,
) -> (Vec<u8>, Vec<u16>) {
    let mut heap = Vec::new();
    let mut offsets = Vec::new();

    for (idx, track) in tracks.iter().enumerate() {
        let row_start = heap.len();
        offsets.push(row_start as u16);

        let artist_id = *artist_map.get(&track.artist).unwrap_or(&0);
        let album_id = *album_map.get(&track.album).unwrap_or(&0);
        let genre_id = *genre_map.get(&track.genre).unwrap_or(&1);
        let key_id = key_name_to_id(&track.key);

        // --- Fixed header (94 bytes = 0x5E) ---
        heap.extend_from_slice(&0x0024u16.to_le_bytes()); // 0x00: subtype
        heap.extend_from_slice(&((idx as u16) * 0x20).to_le_bytes()); // 0x02: index_shift
        heap.extend_from_slice(&0x0700u32.to_le_bytes()); // 0x04: bitmask
        heap.extend_from_slice(&track.sample_rate.to_le_bytes()); // 0x08: sample_rate
        heap.extend_from_slice(&0u32.to_le_bytes()); // 0x0C: composer_id
        heap.extend_from_slice(&(track.file_size as u32).to_le_bytes()); // 0x10: file_size
        let u2 = (track.id + 5) | 0x100;
        heap.extend_from_slice(&u2.to_le_bytes()); // 0x14: analysis flags
        heap.extend_from_slice(&0xE5B6u16.to_le_bytes()); // 0x18: u3
        heap.extend_from_slice(&0x6A76u16.to_le_bytes()); // 0x1A: u4
        heap.extend_from_slice(&0u32.to_le_bytes()); // 0x1C: artwork_id
        heap.extend_from_slice(&key_id.to_le_bytes()); // 0x20: key_id
        heap.extend_from_slice(&0u32.to_le_bytes()); // 0x24: original_artist_id
        heap.extend_from_slice(&0u32.to_le_bytes()); // 0x28: label_id
        heap.extend_from_slice(&0u32.to_le_bytes()); // 0x2C: remixer_id
        heap.extend_from_slice(&(track.bitrate as u32).to_le_bytes()); // 0x30: bitrate
        heap.extend_from_slice(&0u32.to_le_bytes()); // 0x34: track_number
        heap.extend_from_slice(&track.tempo.to_le_bytes()); // 0x38: tempo
        heap.extend_from_slice(&genre_id.to_le_bytes()); // 0x3C: genre_id
        heap.extend_from_slice(&album_id.to_le_bytes()); // 0x40: album_id
        heap.extend_from_slice(&artist_id.to_le_bytes()); // 0x44: artist_id
        heap.extend_from_slice(&track.id.to_le_bytes()); // 0x48: id
        heap.extend_from_slice(&0u16.to_le_bytes()); // 0x4C: disc_number
        heap.extend_from_slice(&0u16.to_le_bytes()); // 0x4E: play_count
        heap.extend_from_slice(&0u16.to_le_bytes()); // 0x50: year
        heap.extend_from_slice(&16u16.to_le_bytes()); // 0x52: sample_depth
        heap.extend_from_slice(&(track.duration_secs as u16).to_le_bytes()); // 0x54: duration
        heap.extend_from_slice(&0x0029u16.to_le_bytes()); // 0x56: u5
        heap.push(0); // 0x58: color_id
        heap.push(0); // 0x59: rating
        let file_type: u16 = match track.source_path.extension().and_then(|e| e.to_str()) {
            Some("mp3") => 0x01,
            Some("m4a") | Some("mp4") | Some("aac") => 0x04,
            Some("flac") => 0x05,
            Some("wav") => 0x0b,
            Some("aiff") | Some("aif") => 0x0c,
            _ => 0x00,
        };
        heap.extend_from_slice(&file_type.to_le_bytes()); // 0x5A: file_type
        heap.extend_from_slice(&0x0003u16.to_le_bytes()); // 0x5C: u7

        debug_assert_eq!(heap.len() - row_start, 0x5E, "Track header must be 94 bytes");

        // --- String offset array (21 x u16 = 42 bytes, offsets relative to row_start) ---
        let string_data_start = 0x5E + 21 * 2; // = 0x88 = 136
        let mut string_data = Vec::new();
        let mut string_offsets = [0u16; 21];

        let mut add = |idx: usize, data: &[u8]| {
            string_offsets[idx] = (string_data_start + string_data.len()) as u16;
            string_data.extend_from_slice(data);
        };

        add(0, &[0x03]); // isrc
        add(1, &[0x03]); // lyricist
        add(2, &encode_string("3")); // unknown2
        add(3, &[0x05, 0x01]); // unknown3 flag
        add(4, &[0x03]); // unknown4
        add(5, &[0x03]); // message
        add(6, &[0x03]); // kuvo_public
        add(7, &encode_string("ON")); // autoload_hotcues
        add(8, &[0x03]); // unknown8
        add(9, &[0x03]); // unknown9
        add(10, &encode_string("2026-04-15")); // date_added
        add(11, &[0x03]); // release_date
        add(12, &[0x03]); // mix_name
        add(13, &[0x03]); // unknown13
        let anlz_path = anlz::anlz_path_for_pdb(track);
        add(14, &encode_string(&anlz_path)); // analyze_path
        add(15, &encode_string("2026-04-15")); // analyze_date
        add(16, &[0x03]); // comment
        add(17, &encode_string(&track.title)); // title
        add(18, &[0x03]); // unknown18
        let filename = track.source_path.file_name()
            .and_then(|f| f.to_str()).unwrap_or("");
        add(19, &encode_string(filename)); // filename
        add(20, &encode_string(&track.usb_path)); // file_path

        // Write string offset table
        for &ofs in &string_offsets {
            heap.extend_from_slice(&ofs.to_le_bytes());
        }

        debug_assert_eq!(heap.len() - row_start, 0x88, "String offsets must end at 0x88");

        // Write string data
        heap.extend_from_slice(&string_data);
        align_to_4(&mut heap);

        // Pad to minimum 344 bytes per track row
        let row_size = heap.len() - row_start;
        if row_size < 344 {
            heap.extend(std::iter::repeat(0u8).take(344 - row_size));
        }
    }

    (heap, offsets)
}

/// Build the 27 standard browse column definitions for Pioneer CDJ browser.
/// Column names use UTF-16LE with Unicode annotation delimiters ￺ (U+FFFA) and ￻ (U+FFFB).
/// Row format: u16 id, u16 column_type, encoded name string.
fn build_columns_rows() -> (Vec<u8>, Vec<u16>) {
    // 27 columns matching rekordbox CDJ-3000 export (verified against BLAU reference)
    let columns: &[(u16, u16, &str)] = &[
        (1,  0x0080, "\u{FFFA}GENRE\u{FFFB}"),
        (2,  0x0081, "\u{FFFA}ARTIST\u{FFFB}"),
        (3,  0x0082, "\u{FFFA}ALBUM\u{FFFB}"),
        (4,  0x0083, "\u{FFFA}TRACK\u{FFFB}"),
        (5,  0x0085, "\u{FFFA}BPM\u{FFFB}"),
        (6,  0x0086, "\u{FFFA}RATING\u{FFFB}"),
        (7,  0x0087, "\u{FFFA}YEAR\u{FFFB}"),
        (8,  0x0088, "\u{FFFA}REMIXER\u{FFFB}"),
        (9,  0x0089, "\u{FFFA}LABEL\u{FFFB}"),
        (10, 0x008a, "\u{FFFA}ORIGINAL ARTIST\u{FFFB}"),
        (11, 0x008b, "\u{FFFA}KEY\u{FFFB}"),
        (12, 0x008d, "\u{FFFA}CUE\u{FFFB}"),
        (13, 0x008e, "\u{FFFA}COLOR\u{FFFB}"),
        (14, 0x0092, "\u{FFFA}TIME\u{FFFB}"),
        (15, 0x0093, "\u{FFFA}BITRATE\u{FFFB}"),
        (16, 0x0094, "\u{FFFA}FILE NAME\u{FFFB}"),
        (17, 0x0084, "\u{FFFA}PLAYLIST\u{FFFB}"),
        (18, 0x0098, "\u{FFFA}HOT CUE BANK\u{FFFB}"),
        (19, 0x0095, "\u{FFFA}HISTORY\u{FFFB}"),
        (20, 0x0091, "\u{FFFA}SEARCH\u{FFFB}"),
        (21, 0x0096, "\u{FFFA}COMMENTS\u{FFFB}"),
        (22, 0x008c, "\u{FFFA}DATE ADDED\u{FFFB}"),
        (23, 0x0097, "\u{FFFA}DJ PLAY COUNT\u{FFFB}"),
        (24, 0x0090, "\u{FFFA}FOLDER\u{FFFB}"),
        (25, 0x00a1, "\u{FFFA}DEFAULT\u{FFFB}"),
        (26, 0x00a2, "\u{FFFA}ALPHABET\u{FFFB}"),
        (27, 0x00aa, "\u{FFFA}MATCHING\u{FFFB}"),
    ];
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for &(id, col_type, name) in columns {
        offsets.push(heap.len() as u16);
        heap.extend_from_slice(&id.to_le_bytes());
        heap.extend_from_slice(&col_type.to_le_bytes());
        heap.extend_from_slice(&encode_string(name)); // will use UTF-16LE due to non-ASCII chars
        align_to_4(&mut heap);
    }
    (heap, offsets)
}

fn build_playlist_tree_rows(playlists: &[(u32, u32, u32, &str, bool)]) -> (Vec<u8>, Vec<u16>) {
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for &(id, parent_id, sort, name, is_folder) in playlists {
        offsets.push(heap.len() as u16);
        heap.extend_from_slice(&parent_id.to_le_bytes());
        heap.extend_from_slice(&0u32.to_le_bytes());
        heap.extend_from_slice(&sort.to_le_bytes());
        heap.extend_from_slice(&id.to_le_bytes());
        heap.extend_from_slice(&(if is_folder { 1u32 } else { 0u32 }).to_le_bytes());
        heap.extend_from_slice(&encode_string(name));
        align_to_4(&mut heap);
    }
    (heap, offsets)
}

fn build_playlist_entry_rows(entries: &[(u32, u32, u32)]) -> (Vec<u8>, Vec<u16>) {
    let mut heap = Vec::new();
    let mut offsets = Vec::new();
    for &(entry_idx, track_id, playlist_id) in entries {
        offsets.push(heap.len() as u16);
        heap.extend_from_slice(&entry_idx.to_le_bytes());
        heap.extend_from_slice(&track_id.to_le_bytes());
        heap.extend_from_slice(&playlist_id.to_le_bytes());
    }
    (heap, offsets)
}

/// Map key name (e.g. "1A", "5B") to rekordbox key ID (1-24).
fn key_name_to_id(key: &str) -> u32 {
    // key_id 1-12 = C,Db,D,Eb,E,F,F#,G,Ab,A,Bb,B (major)
    // key_id 13-24 = Cm,Dbm,...,Bm (minor)
    // DJ notation: 1A=C(1), 2A=G(8), 3A=D(3), 4A=A(10), 5A=E(5), 6A=B(12),
    //              7A=F#(7), 8A=Db(2), 9A=Ab(9), 10A=Eb(4), 11A=Bb(11), 12A=F(6)
    // Minor: 1B=Am(22), 2B=Em(17), etc.
    let major_map: &[u32] = &[1, 8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6]; // 1A..12A
    let minor_map: &[u32] = &[22, 17, 24, 19, 14, 21, 16, 23, 18, 13, 20, 15]; // 1B..12B

    if key.is_empty() {
        return 1; // default C major
    }

    // Parse "1A" through "12B" format
    if key.ends_with('A') || key.ends_with('B') {
        let is_minor = key.ends_with('B');
        if let Ok(num) = key[..key.len() - 1].parse::<usize>() {
            if num >= 1 && num <= 12 {
                return if is_minor { minor_map[num - 1] } else { major_map[num - 1] };
            }
        }
    }
    1 // default
}

// ── Main PDB Writer ────────────────────────────────────────────────

pub fn write_pdb(output_path: &Path, tracks: &[Track], playlists: &[Playlist]) -> Result<()> {
    // Collect unique values
    let mut artists: Vec<String> = tracks.iter().map(|t| t.artist.clone()).collect();
    artists.sort();
    artists.dedup();
    let artist_map: HashMap<String, u32> = artists.iter().enumerate()
        .map(|(i, a)| (a.clone(), (i + 1) as u32)).collect();

    let mut albums: Vec<String> = tracks.iter().map(|t| t.album.clone()).collect();
    albums.sort();
    albums.dedup();
    let album_map: HashMap<String, u32> = albums.iter().enumerate()
        .map(|(i, a)| (a.clone(), (i + 1) as u32)).collect();
    let mut album_artist_map: HashMap<String, u32> = HashMap::new();
    for t in tracks {
        if let Some(&aid) = artist_map.get(&t.artist) {
            album_artist_map.entry(t.album.clone()).or_insert(aid);
        }
    }

    let mut genres: Vec<String> = tracks.iter().map(|t| t.genre.clone()).collect();
    genres.sort();
    genres.dedup();
    let genre_map: HashMap<String, u32> = genres.iter().enumerate()
        .map(|(i, g)| (g.clone(), (i + 1) as u32)).collect();

    // Build row data for each table
    let (track_heap, track_offsets) = build_track_rows(tracks, &artist_map, &album_map, &genre_map);
    let (genre_heap, genre_offsets) = build_genre_rows(&genres);
    let (artist_heap, artist_offsets) = build_artist_rows(&artists);
    let (album_heap, album_offsets) = build_album_rows(&albums, &album_artist_map);
    let (key_heap, key_offsets) = build_key_rows();
    let (color_heap, color_offsets) = build_color_rows();
    let (columns_heap, columns_offsets) = build_columns_rows();

    // Build playlist tree and entries from the provided playlists
    let playlist_tree_data: Vec<(u32, u32, u32, &str, bool)> = playlists.iter()
        .enumerate()
        .map(|(i, p)| (p.id, 0u32, i as u32, p.name.as_str(), false))
        .collect();
    let (pt_heap, pt_offsets) = build_playlist_tree_rows(&playlist_tree_data);

    let mut playlist_entries: Vec<(u32, u32, u32)> = Vec::new();
    let mut entry_idx = 1u32;
    for playlist in playlists {
        for &track_id in &playlist.track_ids {
            playlist_entries.push((entry_idx, track_id, playlist.id));
            entry_idx += 1;
        }
    }
    let (pe_heap, pe_offsets) = build_playlist_entry_rows(&playlist_entries);

    // Check that tables fit in single pages
    for (name, heap_len, num_rows) in [
        ("tracks", track_heap.len(), track_offsets.len()),
        ("genres", genre_heap.len(), genre_offsets.len()),
        ("artists", artist_heap.len(), artist_offsets.len()),
        ("albums", album_heap.len(), album_offsets.len()),
        ("keys", key_heap.len(), key_offsets.len()),
        ("colors", color_heap.len(), color_offsets.len()),
        ("columns", columns_heap.len(), columns_offsets.len()),
        ("playlist_tree", pt_heap.len(), pt_offsets.len()),
        ("playlist_entries", pe_heap.len(), pe_offsets.len()),
    ] {
        let total = heap_len + row_group_bytes(num_rows);
        let capacity = PAGE_SIZE as usize - HEAP_START;
        if total > capacity {
            bail!("{} table overflow: {} bytes needed, {} available. Too many tracks for single-page POC.", name, total, capacity);
        }
    }

    // Create file
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(output_path)?;

    // Pre-allocate 41 pages (0-40) matching rekordbox exports.
    // Empty_candidate pages beyond 40 don't need to exist in the file.
    let file_size = 41 * PAGE_SIZE;
    file.write_all(&vec![0u8; file_size as usize])?;
    file.seek(SeekFrom::Start(0))?;

    // Compute max sequence across all data pages so page 0 sequence exceeds them all.
    // Each data page sequence = base + (num_rows - 1) * 5.
    let data_sequences: Vec<u32> = vec![
        10 + track_offsets.len().saturating_sub(1) as u32 * 5,   // tracks
        7 + artist_offsets.len().saturating_sub(1) as u32 * 5,   // artists
        9 + album_offsets.len().saturating_sub(1) as u32 * 5,    // albums
        8 + 7 * 5,                                                // colors (always 8 rows)
        6 + pt_offsets.len().saturating_sub(1) as u32 * 5,       // playlist_tree
        11 + pe_offsets.len().saturating_sub(1) as u32 * 5,      // playlist_entries
        3,                                                        // columns (always 3)
    ];
    let max_seq = data_sequences.iter().copied().max().unwrap_or(0);
    let header_sequence = max_seq + 2; // page 0 sequence must exceed all data page sequences

    // --- Page 0: File header ---
    file.write_all(&[0u8; 4])?; // 0x00: magic
    file.write_all(&PAGE_SIZE.to_le_bytes())?; // 0x04: len_page
    file.write_all(&(LAYOUTS.len() as u32).to_le_bytes())?; // 0x08: num_tables
    file.write_all(&52u32.to_le_bytes())?; // 0x0C: next_unused_page
    file.write_all(&5u32.to_le_bytes())?; // 0x10: unknown
    file.write_all(&header_sequence.to_le_bytes())?; // 0x14: sequence
    file.write_all(&[0u8; 4])?; // 0x18: gap

    // Table pointers at 0x1C (type, empty_candidate, first_page, last_page)
    for layout in LAYOUTS {
        file.write_all(&layout.table_type.to_le_bytes())?;
        file.write_all(&layout.empty_candidate.to_le_bytes())?;
        file.write_all(&layout.header_page.to_le_bytes())?;
        file.write_all(&layout.last_page.to_le_bytes())?;
    }

    // --- Write table pages ---
    for layout in LAYOUTS {
        let has_data = layout.data_page != 0;
        // Tables where last_page == header_page are "empty" from CDJ perspective:
        // the CDJ stops at the header and never reads the data page.
        // Don't reference the data page from the header to avoid self-loops.
        let cdj_reads_data = has_data && layout.last_page != layout.header_page;

        let next_for_header = if cdj_reads_data {
            layout.data_page
        } else {
            layout.empty_candidate
        };
        let first_data = if cdj_reads_data { Some(layout.data_page) } else { None };

        // Header page
        write_header_page(&mut file, layout.header_page, layout.table_type, next_for_header, first_data)?;

        // Data page — only write if CDJ will actually read it
        if has_data && cdj_reads_data {
            let (heap, offsets, seq) = match layout.table_type {
                0x00 => { let s = 10 + (track_offsets.len().saturating_sub(1) as u32) * 5; (&track_heap, &track_offsets, s) },
                0x02 => { let s = 7 + (artist_offsets.len().saturating_sub(1) as u32) * 5; (&artist_heap, &artist_offsets, s) },
                0x03 => { let s = 9 + (album_offsets.len().saturating_sub(1) as u32) * 5; (&album_heap, &album_offsets, s) },
                0x06 => (&color_heap, &color_offsets, 8 + 7 * 5),
                0x07 => (&pt_heap, &pt_offsets, 6 + (pt_offsets.len().saturating_sub(1) as u32) * 5),
                0x08 => { let s = 11 + (pe_offsets.len().saturating_sub(1) as u32) * 5; (&pe_heap, &pe_offsets, s) },
                0x10 => (&columns_heap, &columns_offsets, 3u32), // columns sequence is always 3
                // History tables: CDJ requires populated history data pages
                0x11 => {
                    seek_to_page(&mut file, layout.data_page)?;
                    file.write_all(REFERENCE_HISTORY_PLAYLISTS)?;
                    continue;
                }
                0x12 => {
                    seek_to_page(&mut file, layout.data_page)?;
                    file.write_all(REFERENCE_HISTORY_ENTRIES)?;
                    continue;
                }
                0x13 => {
                    seek_to_page(&mut file, layout.data_page)?;
                    file.write_all(REFERENCE_HISTORY)?;
                    continue;
                }
                _ => {
                    // artwork and other tables — write blank data page with valid header
                    write_blank_data_page(
                        &mut file,
                        layout.data_page,
                        layout.table_type,
                        layout.empty_candidate,
                    )?;
                    continue;
                }
            };

            if offsets.is_empty() {
                // Leave as zeros (pre-allocated)
                continue;
            }

            write_data_page(
                &mut file,
                layout.data_page,
                layout.table_type,
                layout.empty_candidate, // next_page = empty_candidate (terminates chain)
                heap,
                offsets,
                seq,
            )?;
        }
    }

    Ok(())
}
