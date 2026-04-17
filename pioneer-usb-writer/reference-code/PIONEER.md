# Pioneer DeviceSQL & ANLZ Format — Reverse Engineering Notes

This document contains technical discoveries about Pioneer's proprietary DeviceSQL format (`export.pdb`) and ANLZ analysis files that are **not publicly documented**. These findings were obtained through binary analysis, disassembly, and hardware testing on a **CDJ-3000 (firmware 3.19)**.

Existing open-source documentation (Deep Symmetry, rekordcrate, Kaitai Struct specs) covers the broad structure. This document focuses on the **undocumented details that make or break hardware compatibility** — the things that cause "rekordbox database not found" or silent re-analysis on real players.

**Use this freely.** If you're building tools for Pioneer hardware, this will save you weeks of binary diffing.

---

## Table of Contents

1. [ANLZ Path Hash Algorithm](#1-anlz-path-hash-algorithm)
2. [ANLZ File Format — What the CDJ Actually Requires](#2-anlz-file-format--what-the-cdj-actually-requires)
3. [PDB Database — Critical Undocumented Fields](#3-pdb-database--critical-undocumented-fields)
4. [PDB Page Header Formulas](#4-pdb-page-header-formulas)
5. [PDB Row Group Structure](#5-pdb-row-group-structure)
6. [Track Row Layout](#6-track-row-layout)
7. [CDJ-3000 Hardware Behavior](#7-cdj-3000-hardware-behavior)
8. [Debugging Methodology](#8-debugging-methodology)
9. [References](#9-references)

---

## 1. ANLZ Path Hash Algorithm

**How we found this:** Disassembled `CreateAnlzFileFolderPath()` from the rekordbox macOS binary using radare2.

### The Problem

Pioneer CDJs compute ANLZ file paths independently from the audio file path. They **completely ignore** the `analyze_path` field stored in the PDB track row. If your ANLZ files aren't at the exact hash-computed path, the CDJ will never find them.

### Path Format

```
/PIONEER/USBANLZ/P{XXX}/{YYYYYYYY}/ANLZ0000.DAT
/PIONEER/USBANLZ/P{XXX}/{YYYYYYYY}/ANLZ0000.EXT
```

- `XXX` = 3 hex digits (P value, derived from scattered bits of the hash)
- `YYYYYYYY` = 8 hex digits (hash value mod 200003)

### Algorithm

```python
def compute_anlz_path(file_path: str) -> tuple[int, int]:
    """
    Args:
        file_path: Path relative to USB root, e.g. "/Contents/Artist/Track.flac"
    Returns:
        (p_value, hash_value) for path P{p_value:03X}/{hash_value:08X}
    """
    hash_val = 0

    for char in file_path:
        c = ord(char) & 0xFFFF  # UTF-16 code unit
        temp = (hash_val * 0x5BC9 + c) & 0xFFFFFFFF
        hash_val = (temp * 0x93B5 + c) & 0xFFFFFFFF

    hash_result = hash_val % 200003  # 0x30D43 (prime)

    # P value: bits extracted from non-contiguous positions
    p = 0
    p |= (hash_result >> 0)  & 0x01  # bit 0  -> bit 0
    p |= (hash_result >> 1)  & 0x02  # bit 2  -> bit 1
    p |= (hash_result >> 4)  & 0x04  # bit 6  -> bit 2
    p |= (hash_result >> 4)  & 0x08  # bit 7  -> bit 3
    p |= (hash_result >> 5)  & 0x10  # bit 9  -> bit 4
    p |= (hash_result >> 8)  & 0x20  # bit 13 -> bit 5
    p |= (hash_result >> 10) & 0x40  # bit 16 -> bit 6

    return (p, hash_result)
```

### Verified Test Cases

| Input Path | P | Hash |
|---|---|---|
| `/Contents/Leo Portela/Bon Vibrant - Leo Portela.flac` | `0x00E` | `0x000281CE` |
| `/Contents/Daniela Cast/Jazzy - Daniela Cast.flac` | `0x00A` | `0x0000CC9C` |
| `/Contents/Huerta/Tatra Motokov - Huerta.flac` | `0x012` | `0x0000530C` |

All verified by comparing against rekordbox-exported ANLZ directory names.

---

## 2. ANLZ File Format — What the CDJ Actually Requires

The existing Kaitai Struct spec documents the section tags, but not the **validation requirements** that cause the CDJ to accept or reject the file.

### Critical Discovery: PPTH Null Terminator

The PPTH (path) section stores the audio file path as UTF-16 Big Endian. **The path MUST include a null terminator** (U+0000, 2 bytes).

Without it, the CDJ finds the ANLZ file but rejects it and creates its own `ANLZ0001.DAT` alongside yours. This was the hardest bug to find — the file is in the right place, the sections look correct, but the CDJ silently rejects it.

```
PPTH content:  /Contents/Artist/Track.flac\x00\x00   <- 2 zero bytes at end
path_len field: (string_length + 1) * 2              <- includes null terminator
```

**How we found this:** The CDJ created `ANLZ0001.DAT` next to our `ANLZ0000.DAT`. Comparing the CDJ-generated file against ours, the only difference in the PPTH section was 2 extra bytes (the null terminator). The CDJ's `path_len` was always 2 bytes larger than ours.

### .DAT File — Required Sections

Order matters. The CDJ reads sections sequentially.

```
PMAI  (28 bytes)     — File header
PPTH  (variable)     — File path (UTF-16BE, null-terminated)
PVBR  (1620 bytes)   — VBR seek table (all zeros for FLAC/WAV/AIFF)
PQTZ  (variable)     — Beat grid
PWAV  (420 bytes)    — Monochrome waveform preview (400 entries)
PCOB  (24 bytes)     — Cue points container (hot cues, count=1, no entries)
PCOB  (24 bytes)     — Cue points container (memory cues, count=0)
```

### .EXT File — Required by CDJ-3000

**The CDJ-3000 requires .EXT files.** Without them, it re-analyzes every track on load. This was not obvious because the .DAT file works fine on older hardware.

```
PMAI  (28 bytes)     — File header
PPTH  (variable)     — File path (same as .DAT)
PWV3  (51224 bytes)  — Color preview waveform (51200 entries, 1 byte each)
PCOB  (24 bytes)     — Cue points container (hot cues)
PCOB  (24 bytes)     — Cue points container (memory cues)
PCO2  (20 bytes)     — Extended cue container (hot cues)
PCO2  (20 bytes)     — Extended cue container (memory cues)
PQT2  (variable)     — Extended beat grid (2 bytes per beat)
PWV5  (variable)     — Detailed color waveform (51200 entries, 2 bytes each)
PWV4  (7224 bytes)   — Color waveform preview (1200 entries, 6 bytes each)
PVB2  (8032 bytes)   — Extended VBR info (all zeros for lossless)
```

### .2EX File — Optional (CDJ-3000)

Contains high-resolution waveforms. The CDJ works without it but displays lower-quality waveforms.

```
PMAI  (28 bytes)
PPTH  (variable)
PWV7  (variable)     — High-res color waveform (51200 entries, 3 bytes each)
PWV6  (variable)     — High-res color preview (1200 entries, 3 bytes each)
PWVC  (20 bytes)     — Waveform color config
```

### PMAI Header

```
Offset  Size  Value      Description
0x00    4     "PMAI"     Magic
0x04    4     0x1C       Header length (always 28)
0x08    4     (varies)   Total file size (all sections + header)
0x0C    4     0x01       Unknown (rekordbox uses 1, CDJ uses 0 — both work)
0x10    4     0x10000    Unknown (rekordbox uses 0x10000, CDJ uses 0)
0x14    4     0x10000    Unknown (same as above)
0x18    4     0x00       Padding
```

### PQTZ Section — Beat Grid

The header is **24 bytes, not 20**. Some documentation omits the `unknown2` field.

```
Offset  Size  Value       Description
0x00    4     "PQTZ"      Tag
0x04    4     0x18        Header length (24, NOT 20)
0x08    4     (varies)    Section length (header + beat_count * 8)
0x0C    4     0x00        Unknown1
0x10    4     0x00080000  Unknown2 (beat entry size marker — MUST be this value)
0x14    4     (varies)    Beat count
0x18    ...   beat data   Each beat: bar_position(u16be) + tempo(u16be) + time_ms(u32be) = 8 bytes
```

### PCOB Section — Cue Points

```
Offset  Size  Value       Description
0x00    4     "PCOB"      Tag
0x04    4     0x18        Header length (24)
0x08    4     0x18        Section length (24 when no cue entries)
0x0C    4     (varies)    Cue count (1 for hot cues container, 0 for memory)
0x10    4     0x00        Unknown
0x14    4     0xFFFFFFFF  Sentinel (rekordbox uses 0xFFFFFFFF)
```

### PCO2 Section — Extended Cues

```
Offset  Size  Value       Description
0x00    4     "PCO2"      Tag
0x04    4     0x14        Header length (20)
0x08    4     0x14        Section length
0x0C    4     (varies)    Count
0x10    4     0x00        Unknown
```

### PWV3 Section — Color Preview Waveform

```
Offset  Size  Value       Description
0x00    4     "PWV3"      Tag
0x04    4     0x18        Header length (24)
0x08    4     0xC818      Section length (51224)
0x0C    4     0x01        Unknown (possibly version)
0x10    4     0xC800      Data length (51200)
0x14    2     0x0096      Unknown (150 — possibly entries per beat?)
0x16    2     0x0000      Unknown
0x18    ...   data        51200 bytes, each byte: color(3 bits) | height(5 bits)
```

---

## 3. PDB Database — Critical Undocumented Fields

These are the fields that cause "rekordbox database not found" when wrong. All were found through binary bisection testing on CDJ-3000 hardware.

### Page 0 Header

```
Offset  Size  Field              Value/Formula
0x00    4     magic              0x00000000
0x04    4     page_size          4096 (0x1000)
0x08    4     num_tables         20 (0x14) — number of table types
0x0C    4     next_unused_page   52 — first page not used by any table
0x10    4     unknown            5
0x14    4     sequence           MUST be >= max sequence of all data pages + 1
0x18    4     padding            0
0x1C    ...   table_pointers     20 entries × 16 bytes each
```

**The sequence field** was our first "database not found" trap with more tracks. With 6 tracks, all data page sequences were below 53. With 11 tracks, playlist entries reached sequence 61. Hardcoding 53 broke everything.

### Table Pointer Format (16 bytes each)

```
Offset  Size  Field
0x00    4     table_type         Type ID (0x00=Tracks, 0x01=Genres, ...)
0x04    4     empty_candidate    Page index for empty/overflow chain
0x08    4     first_page         First header page for this table
0x0C    4     last_page          Last page in chain (header_page if no data)
```

### Columns Table (type 0x10) — Special Page Header

The Columns table uses a **completely different page header format** from all other tables. Getting this wrong causes "database not found."

```
Normal data page:     unknown5=0x0001, num_rows_large=(num_rows-1), unk4=num_groups
Columns data page:    unknown5=num_rows, num_rows_large=0, unk4=num_groups+1
Columns sequence:     Always 3 (regardless of row count)
```

### Tracks and History Header Pages — Special Values

Tracks (0x00) and History (0x13) header pages need two non-standard values:

```
unknown7 (offset 0x26):  1 (instead of 0 for all other tables)

Header page content at 0x28:
  +0x10: 0x1FFF0001  (instead of 0x1FFF0000 for other tables)
  +0x14: 0x00000010  (Tracks) or 0x00000140 (History)
```

### History Tables MUST Be Populated

Empty history data pages cause "database not found." Even though it makes no sense for a fresh export to have history, the CDJ requires it.

**Solution:** Embed binary blobs of known-good history pages from a working rekordbox export. The content doesn't need to match your tracks.

```
Page 36: HistoryPlaylists data (from reference export)
Page 38: HistoryEntries data (from reference export)
Page 40: History data (from reference export)
```

---

## 4. PDB Page Header Formulas

Every data page has a 40-byte header (0x28 bytes). Two fields use formulas based on row count that aren't documented anywhere.

### Page Header Layout

```
Offset  Size  Field          Formula / Value
0x00    4     gap            0x00000000
0x04    4     page_index     This page's index
0x08    4     table_type     Type ID
0x0C    4     next_page      Next page in chain (empty_candidate if last)
0x10    4     sequence       base + (total_rows - 1) * 5
0x14    4     unknown2       0x00000000
0x18    1     num_rows_small min(num_rows, 255)
0x19    1     unk3           (num_rows % 8) * 0x20
0x1A    1     unk4           ceil(num_rows / 16)  [+1 for Columns table]
0x1B    1     page_flags     0x64=header page, 0x24=data page
0x1C    2     free_size      Remaining bytes in page
0x1E    2     used_size      Bytes used by row data + row groups
0x20    2     unknown5       0x0001 (normal) / num_rows (Columns) / 0x1FFF (header)
0x22    2     num_rows_large (num_rows - 1) (normal) / 0 (Columns) / 0x1FFF (header)
0x24    2     unknown6       0x0000 (data) / 0x03EC (header)
0x26    2     unknown7       0 (normal) / 1 (Tracks + History header pages)
```

### Sequence Base Values

| Table | Base |
|---|---|
| Tracks (0x00) | 10 |
| Artists (0x02) | 7 |
| Albums (0x03) | 9 |
| Colors (0x06) | 8 |
| PlaylistTree (0x07) | 6 |
| PlaylistEntries (0x08) | 11 |
| Columns (0x10) | 3 (fixed, no formula) |

Formula: `sequence = base + (num_rows - 1) * 5`

For multi-page tables, sequence is cumulative across pages.

---

## 5. PDB Row Group Structure

Row offsets are stored in "row groups" at the end of each data page, growing downward from the page boundary.

```
Page layout:
  [0x00-0x27]  Page header (40 bytes)
  [0x28-...]   Row data (heap, growing upward)
  [...-0xFFF]  Row groups (growing downward from page end)
```

### Row Group Format

Groups hold up to 16 rows. Groups are stored in reverse order.

```
Full group (16 rows):   16 offsets(u16) + present_flags(u16) + padding(u16) = 36 bytes
Partial group (N rows):  N offsets(u16) + present_flags(u16) + padding(u16) = N*2 + 4 bytes
```

- `present_flags`: bitmask of used slots (0xFFFF for full group)
- `padding`: 0x0000 for full groups, implementation-dependent for partial

---

## 6. Track Row Layout

Track rows are variable-length with fixed fields followed by string offset pointers.

### Fixed Fields (first ~86 bytes)

| Offset | Size | Field |
|---|---|---|
| 0x00 | 2 | unknown (0x0024) |
| 0x02 | 2 | index_shift (0x0003) |
| 0x04 | 4 | bitmask (0x1FF803DE) |
| 0x08 | 4 | sample_rate |
| 0x0C | 4 | composer_id |
| 0x10 | 4 | file_size |
| 0x14 | 4 | unknown_2 (track_id + 20) |
| ... | ... | (more fixed fields) |
| 0x52 | 2 | sample_depth |
| 0x54 | 2 | duration_seconds |

### String Offset Fields (21 × u16)

These are offsets from the row start pointing to DeviceSQL-encoded strings within the row data:

1. isrc, 2. texter, 3-4. unknown, 5. unknown, 6. message,
7. kuvo_public, 8. autoload_hotcues, 9-10. unknown,
11. date_added, 12. release_date, 13. mix_name, 14. unknown,
15. analyze_path, 16. analyze_date, 17. comment, 18. title,
19. unknown, 20. filename, 21. file_path

---

## 7. CDJ-3000 Hardware Behavior

### USB Recognition Flow

1. CDJ reads `PIONEER/rekordbox/export.pdb`
2. Validates page 0 header, then walks table chains
3. **Any invalid page causes "rekordbox database not found"** — there is no partial failure

### Track Loading Flow

1. User selects track from browser
2. CDJ computes ANLZ hash from audio file path (ignoring PDB `analyze_path`)
3. Looks for `ANLZ0000.DAT` and `ANLZ0000.EXT` at computed hash path
4. **Validates PPTH path matches** (including null terminator)
5. If valid: uses waveform/beat data from files
6. If invalid: creates `ANLZ0001.DAT` with its own analysis (increments the number)

### What Triggers Re-Analysis

| Condition | Result |
|---|---|
| .EXT file missing | Re-analyzes (CDJ-3000 needs color waveforms) |
| .DAT file missing | Re-analyzes |
| PPTH path missing null terminator | Rejects file, creates ANLZ0001.DAT |
| Wrong hash directory | Files never found |
| Wrong PQTZ header size (20 instead of 24) | Unknown (may cause re-analysis) |

### What the CDJ Creates on USB

When analyzing, the CDJ writes:
- `ANLZ0001.DAT` — if `ANLZ0000.DAT` exists but was rejected
- `export.pdb.bak` — backup of the PDB on first use
- `RBFLTR.DAT` — filter/settings file (identifies player model and firmware)

### CDJ-Generated ANLZ vs Rekordbox ANLZ

| Field | CDJ-3000 | Rekordbox |
|---|---|---|
| PMAI unknown fields | All zeros | Non-zero (0x01, 0x10000, 0x10000) |
| PCOB sentinel | 0x00000000 | 0xFFFFFFFF |
| PPTH null terminator | Present | Present |
| PWV2 section (100-entry waveform) | Present | Not in .DAT |

Both formats are accepted by the CDJ. Rekordbox format is the reference standard.

---

## 8. Debugging Methodology

### Binary Bisection Testing

When the CDJ reports "database not found," the cause could be any single byte in a 168KB file. Binary bisection is the fastest way to isolate it:

1. Start with a known-working PDB (e.g., rekordbox export)
2. Copy your PDB's pages into the working one, one at a time
3. Test on CDJ after each page swap
4. When it breaks: that page is the problem
5. Within the broken page: swap individual fields to find the exact byte

This is how we found the Columns table, History table, and header page issues — each was a single page causing "database not found."

### ANLZ Validation

1. Check the hash-computed directory matches where your files are
2. Hex-compare your PPTH section against a CDJ-generated one (look for null terminator)
3. Compare section order and count against rekordbox-exported files
4. If the CDJ creates `ANLZ0001.DAT`, it found but rejected your `ANLZ0000.DAT` — diff them

### Tools

- `xxd` — hex dumps for binary comparison
- `python3 struct` — quick binary parsing scripts
- `dd` — extract individual pages from PDB files
- `dot_clean` — remove macOS resource forks from FAT32 USBs (they can confuse Linux-based players)

---

## 9. References

### Existing Documentation (starting points)

- [Deep Symmetry — PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry — ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate Library (Rust)](https://holzhaus.github.io/rekordcrate/)
- [Kaitai PDB Spec](https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_pdb.ksy)

### What This Document Adds

Things not covered by any of the above:
- ANLZ path hash algorithm (constants, modulo, P-value bit extraction)
- PPTH null terminator requirement
- Columns table special page header format
- History table population requirement
- Tracks/History header page unknown7 and content byte values
- Page 0 sequence must exceed all data page sequences
- CDJ-3000 .EXT file requirement
- PQTZ 24-byte header (unknown2 = 0x00080000)
- CDJ file rejection behavior (ANLZ0001.DAT creation)

---

*Tested on: CDJ-3000 firmware 3.19, April 2026*
*Source: [pioneer-usb-writer](https://github.com/) — Rust tool that writes CDJ-compatible USBs without rekordbox*
