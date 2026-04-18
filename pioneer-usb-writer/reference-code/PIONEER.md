# Pioneer DeviceSQL & ANLZ Format — Reverse Engineering Notes

This document contains technical discoveries about Pioneer's proprietary DeviceSQL format (`export.pdb`), ANLZ analysis files, and the newer **OneLibrary format** (`exportLibrary.db`) that are **not publicly documented**. These findings were obtained through binary analysis, disassembly, and hardware testing on a **CDJ-3000 (firmware 3.19)**.

Existing open-source documentation (Deep Symmetry, rekordcrate, Kaitai Struct specs) covers the broad structure. This document focuses on the **undocumented details that make or break hardware compatibility** — the things that cause "rekordbox database not found" or silent re-analysis on real players.

### Dual-Format Requirement

As of rekordbox 7.x, USB exports contain **both** the legacy format and the OneLibrary format side by side. Both must be written for full device compatibility:

- **Legacy** (`export.pdb` + `.DAT` + `.EXT`): Required for CDJ-3000, XDJ-XZ, and all older players.
- **OneLibrary** (`exportLibrary.db` + `.2EX` + `exportExt.pdb`): Required for CDJ-3000X, XDJ-AZ, OPUS-QUAD, OMNIS-DUO. Also read by djay Pro and Traktor.

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
9. [OneLibrary Format (exportLibrary.db)](#9-onelibrary-format-exportlibrarydb)
10. [Rekordbox Master Database (master.db)](#10-rekordbox-master-database-masterdb)
11. [References](#11-references)

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

### .2EX File — OneLibrary Waveforms

Contains the 3-band frequency waveforms used by OneLibrary. Required for CDJ-3000X/XDJ-AZ/OPUS-QUAD and readable by djay Pro and Traktor. The CDJ-3000 (non-X) works without it but displays lower-quality waveforms.

```
PMAI  (28 bytes)
PPTH  (variable)     — File path (same as .DAT/.EXT)
PWV7  (variable)     — 3-band waveform, full resolution (same entry count as PWV5, 3 bytes each)
PWV6  (variable)     — 3-band waveform, overview (same entry count as PWV4, 3 bytes each)
```

### PWV7 Section — Full-Resolution 3-Band Waveform

Entry count matches PWV3/PWV5 (varies by track length, e.g. 53734 for a ~6min track).

```
Offset  Size  Value       Description
0x00    4     "PWV7"      Tag
0x04    4     0x18        Header length (24)
0x08    4     (varies)    Section length (header + entry_count * 3)
0x0C    4     0x03        Unknown (version or type — always 3)
0x10    4     (varies)    Entry count (same as PWV3/PWV5 for this track)
0x14    2     0x0096      Unknown (150 — matches PWV3)
0x16    2     0x0000      Unknown
0x18    ...   data        3 bytes per entry: [low, mid, high] frequency amplitudes
```

Each entry is 3 bytes representing amplitude levels for frequency bands:
- Byte 0: **Low** (bass) — values typically 0x00–0x60
- Byte 1: **Mid** — values typically 0x00–0x40
- Byte 2: **High** (treble) — values typically 0x00–0x30

### PWV6 Section — Overview 3-Band Waveform

Entry count matches PWV4 (1200 entries for standard overview).

```
Offset  Size  Value       Description
0x00    4     "PWV6"      Tag
0x04    4     0x14        Header length (20)
0x08    4     (varies)    Section length (header + entry_count * 3)
0x0C    4     0x03        Unknown (always 3)
0x10    4     (varies)    Entry count (same as PWV4, typically 1200)
0x14    ...   data        3 bytes per entry: [low, mid, high] frequency amplitudes
```

Same 3-byte [low, mid, high] encoding as PWV7. This is the simplified/interoperable counterpart to PWV4 (which uses 6 bytes per entry with Pioneer-specific color encoding).

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

## 9. OneLibrary Format (exportLibrary.db)

OneLibrary (formerly "Device Library Plus") is a new export format introduced in rekordbox 6.8+ and expanded in rekordbox 7.x. It replaces the binary DeviceSQL format (`export.pdb`) with an encrypted SQLite database for newer hardware. The format was developed by AlphaTheta and adopted by Algoriddim (djay Pro) and Native Instruments (Traktor) for cross-platform DJ library interoperability.

### Files on USB

OneLibrary adds three files alongside the legacy format:

```
PIONEER/rekordbox/exportLibrary.db       — SQLCipher-encrypted SQLite database
PIONEER/rekordbox/exportLibrary.db-wal   — SQLite write-ahead log
PIONEER/rekordbox/exportLibrary.db-shm   — SQLite shared memory
PIONEER/rekordbox/exportExt.pdb          — Extended PDB (page-based, menu/sort config)
PIONEER/USBANLZ/P{XXX}/{HASH}/ANLZ0000.2EX  — 3-band waveforms (see section 2)
```

### Database Encryption

The database is encrypted with **SQLCipher** (SQLite encryption extension). The key is different from the master.db key.

**Decryption key:** `r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls`

The key is derived from an obfuscated constant in the rekordbox binary:

```python
import base64, zlib

BLOB_KEY = b"657f48f84c437cc1"
BLOB = b"PN_1dH8$oLJY)16j_RvM6qphWw`476>;C1cWmI#se(PG`j}~xAjlufj?`#0i{;=glh(SkW)y0>n?YEiD`l%t("

data = base64.b85decode(BLOB)
xored = bytes(b ^ BLOB_KEY[i % len(BLOB_KEY)] for i, b in enumerate(data))
key = zlib.decompress(xored).decode("utf-8")
# key = "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls"
```

**Opening the database:**

```python
import sqlcipher3

conn = sqlcipher3.connect("exportLibrary.db")
conn.execute("PRAGMA key='r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls'")
conn.execute("SELECT * FROM content")  # works
```

Default SQLCipher4 parameters (no custom PRAGMAs needed).

### Database Schema — 22 Tables

```sql
-- Core content
CREATE TABLE content(
    content_id integer primary key,
    title varchar,
    titleForSearch varchar,
    subtitle varchar,
    bpmx100 integer,              -- BPM * 100 (e.g. 12990 = 129.90 BPM)
    length integer,               -- duration in seconds
    trackNo integer,
    discNo integer,
    artist_id_artist integer,     -- FK -> artist.artist_id
    artist_id_remixer integer,
    artist_id_originalArtist integer,
    artist_id_composer integer,
    artist_id_lyricist integer,
    album_id integer,             -- FK -> album.album_id
    genre_id integer,             -- FK -> genre.genre_id
    label_id integer,             -- FK -> label.label_id
    key_id integer,               -- FK -> key.key_id (musical key)
    color_id integer,             -- FK -> color.color_id
    image_id integer,             -- FK -> image.image_id
    djComment varchar,
    rating integer,
    releaseYear integer,
    releaseDate varchar,
    dateCreated varchar,          -- "YYYY-MM-DD"
    dateAdded varchar,
    path varchar,                 -- USB-relative path, e.g. "/Contents/Artist/Album/File.flac"
    fileName varchar,
    fileSize integer,
    fileType integer,             -- 1=MP3, 4=M4A, 5=FLAC, 11=WAV, 12=AIFF
    bitrate integer,
    bitDepth integer,
    samplingRate integer,
    isrc varchar,
    djPlayCount integer,
    isHotCueAutoLoadOn integer,
    isKuvoDeliverStatusOn integer,
    kuvoDeliveryComment varchar,
    masterDbId integer,           -- rekordbox master DB identifier
    masterContentId integer,
    analysisDataFilePath varchar, -- USB-relative ANLZ path, e.g. "/PIONEER/USBANLZ/P00C/000289D2/ANLZ0000.DAT"
    analysedBits integer,
    contentLink integer,
    hasModified integer,
    cueUpdateCount integer,
    analysisDataUpdateCount integer,
    informationUpdateCount integer
);

-- Lookup tables
CREATE TABLE artist(artist_id integer primary key, name varchar, nameForSearch varchar);
CREATE TABLE album(album_id integer primary key, name varchar, artist_id integer, image_id integer, isComplation integer, nameForSearch varchar);
CREATE TABLE genre(genre_id integer primary key, name varchar);
CREATE TABLE label(label_id integer primary key, name varchar);
CREATE TABLE key(key_id integer primary key, name varchar);
CREATE TABLE color(color_id integer primary key, name varchar);
CREATE TABLE image(image_id integer primary key, path varchar);  -- e.g. "/PIONEER/Artwork/00001/b1.jpg"

-- Cue points
CREATE TABLE cue(
    cue_id integer primary key,
    content_id integer,
    kind integer,                 -- cue type
    colorTableIndex integer,
    cueComment varchar,
    isActiveLoop integer,
    beatLoopNumerator integer,
    beatLoopDenominator integer,
    inUsec integer,               -- microsecond precision
    outUsec integer,
    in150FramePerSec integer,
    out150FramePerSec integer,
    inMpegFrameNumber integer,
    outMpegFrameNumber integer,
    inMpegAbs integer,
    outMpegAbs integer,
    inDecodingStartFramePosition integer,
    outDecodingStartFramePosition integer,
    inFileOffsetInBlock integer,
    OutFileOffsetInBlock integer,
    inNumberOfSampleInBlock integer,
    outNumberOfSampleInBlock integer
);

-- Playlists (self-referential tree: folders + playlists)
CREATE TABLE playlist(playlist_id integer primary key, sequenceNo integer, name varchar, image_id integer, attribute integer, playlist_id_parent integer);
CREATE TABLE playlist_content(playlist_id integer, content_id integer, sequenceNo integer);

-- History
CREATE TABLE history(history_id integer primary key, sequenceNo integer, name varchar, attribute integer, history_id_parent integer);
CREATE TABLE history_content(history_id integer, content_id integer, sequenceNo integer);

-- Hot cue banks
CREATE TABLE hotCueBankList(hotCueBankList_id integer primary key, sequenceNo integer, name varchar, image_id integer, attribute integer, hotCueBankList_id_parent integer);
CREATE TABLE hotCueBankList_cue(hotCueBankList_id integer, cue_id integer, sequenceNo integer);

-- My Tags (rekordbox custom tagging system)
CREATE TABLE myTag(myTag_id integer primary key, sequenceNo integer, name varchar, attribute integer, myTag_id_parent integer);
CREATE TABLE myTag_content(myTag_id integer, content_id integer);

-- CDJ browse menu configuration
CREATE TABLE menuItem(menuItem_id integer primary key, kind integer, name varchar);  -- names use \ufffa...\ufffb delimiters
CREATE TABLE category(category_id integer primary key, menuItem_id integer, sequenceNo integer, isVisible integer);
CREATE TABLE sort(sort_id integer primary key, menuItem_id integer, sequenceNo integer, isVisible integer, isSelectedAsSubColumn integer);

-- Metadata
CREATE TABLE property(deviceName varchar, dbVersion varchar, numberOfContents integer, createdDate varchar, backGroundColorType integer, myTagMasterDBID integer);
CREATE TABLE recommendedLike(content_id_1 integer, content_id_2 integer, rating integer, createdDate integer);
```

### Key Observations

- **DB version** is `"1000"` (in `property.dbVersion`).
- **IDs are plain integers**, unlike the master.db which uses VARCHAR UUIDs. Much simpler.
- **Paths** use forward slashes and are relative to USB root (e.g. `/Contents/Artist/Album/File.flac`).
- **BPM** is stored as integer × 100 (e.g. 12990 = 129.90 BPM).
- **fileType values**: 1=MP3, 4=M4A, 5=FLAC, 11=WAV, 12=AIFF (same as master.db).
- **menuItem names** use special delimiters: `\ufffa` before and `\ufffb` after (e.g. `\ufffaGENRE\ufffb`).
- **color table** has 8 fixed entries: Pink, Red, Orange, Yellow, Green, Aqua, Blue, Purple.
- **`analysisDataFilePath`** points to the ANLZ path on the USB — same hash-computed path used by the legacy format.
- **`contentLink`** appears to be a hash or identifier — possibly related to how the CDJ cross-references between `export.pdb` and `exportLibrary.db`.

### exportExt.pdb

A page-based file (same 4096-byte page size as `export.pdb`) but with a different structure. Contains 18 pages for a 6-track export. Appears to store extended menu/sort/category configuration data. Page 0 is an index page, data pages use `0x64` page flags. Further reverse engineering needed.

---

## 10. Rekordbox Master Database (master.db)

The Rekordbox master database is the local library store on the user's computer — the source of truth that Rekordbox syncs to USB. Unlike the USB export format (which uses both DeviceSQL and OneLibrary), the master database is a single **SQLCipher-encrypted SQLite** file.

### Location

**macOS:** `~/Library/Pioneer/rekordbox/master.db`

**Windows:** `%APPDATA%\Pioneer\rekordbox\master.db`

Backup copies are rotated automatically:
- `master.backup.db` (most recent)
- `master.backup2.db`
- `master.backup3.db`

### Supporting Files in Same Directory

```
master.db                  — Main library database (SQLCipher encrypted)
product.db                 — Product/device info (same encryption)
datafile.edb               — Genre/tag/color configuration (Pioneer proprietary binary)
ExtData.edb                — Extended tag data (Pioneer proprietary binary)
networkAnalyze6.db         — Network analysis cache (unencrypted SQLite)
networkRecommend.db        — Network recommendation cache (unencrypted SQLite)
masterPlaylists6.xml       — Playlist tree structure (plain XML)
automixPlaylist6.xml       — Automix settings (plain XML)
share/PIONEER/Artwork/     — Cached artwork thumbnails
Exceptions/ExceptioinInfo  — Exception/crash data
```

### Encryption

The database uses **SQLCipher** with default v4 parameters (no custom KDF iterations, page size, etc.).

**Decryption key:** `402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497`

This key is **static across all Rekordbox 6.x and 7.x installations** — it is not per-device.

### How the Key Was Found

1. **Find the device password (dp).** Rekordbox stores a base64-encoded "device password" in the agent configuration:
   ```
   ~/Library/Application Support/Pioneer/rekordboxAgent/storage/options.json
   ```
   The `dp` field contains a base64-encoded 64-byte value. This is used for device pairing, **not** directly as the database key.

2. **Find the obfuscated key.** The open-source project `pyrekordbox` ships a hardcoded blob that contains the actual SQLCipher key, obfuscated with base85 encoding + XOR + zlib compression:

   ```python
   from pyrekordbox.db6.database import BLOB
   from pyrekordbox.utils import deobfuscate

   key = deobfuscate(BLOB)
   # key = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497"
   ```

   The deobfuscation uses:
   - XOR key: `b"657f48f84c437cc1"` (16 bytes, repeated cyclically)
   - Base85 decode the blob
   - XOR with the key
   - Zlib decompress

3. **Open the database.** With `rusqlite` (Rust, `bundled-sqlcipher` feature):
   ```rust
   let conn = Connection::open("master.db")?;
   conn.execute_batch("PRAGMA key='402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497'")?;
   conn.execute_batch("SELECT count(*) FROM sqlite_master")?; // verifies decryption
   ```

   Or with Python (`sqlcipher3`):
   ```python
   import sqlcipher3
   conn = sqlcipher3.connect("master.db")
   conn.execute("PRAGMA key='402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497'")
   conn.execute("SELECT * FROM djmdContent")
   ```

### Database Schema — 47 Tables

The master database uses a significantly different schema from the USB export format. Notable differences:
- All IDs are **VARCHAR UUIDs** (not sequential integers like the USB export)
- Tables are prefixed with `djmd` (DJ Music Database)
- Extensive cloud sync fields (`rb_data_status`, `rb_local_synced`, `usn`, etc.)
- `contentFile` and `imageFile` tables track file paths and hashes for cloud sync

#### Core Tables

| Table | Purpose | Key Columns |
|---|---|---|
| `djmdContent` | All tracks (649 in our library) | Title, ArtistID, AlbumID, GenreID, BPM, Length, TrackNo, BitRate, KeyID, ColorID, Rating, FolderPath, FileNameL, ImagePath, AnalysisDataPath |
| `djmdArtist` | Artists (590) | Name, SearchStr |
| `djmdAlbum` | Albums (600) | Name, AlbumArtistID, ImagePath, Compilation |
| `djmdGenre` | Genres (24) | Name |
| `djmdLabel` | Labels (382) | Name |
| `djmdKey` | Musical keys (24) | ScaleName, Seq |
| `djmdColor` | Cue colors (8) | ColorCode, SortKey, Commnt |
| `djmdPlaylist` | Playlists/tree (28) | Name, Attribute, ParentID, SmartList |
| `djmdSongPlaylist` | Playlist entries (642) | PlaylistID, ContentID, TrackNo |
| `djmdHistory` | Play history | Name, Attribute, ParentID, DateCreated |
| `djmdCue` | Cue points | ContentID, InMsec, OutMsec, Kind, Color, Comment, ActiveLoop |
| `djmdMixerParam` | Gain/peak settings | ContentID, GainHigh, GainLow, PeakHigh, PeakLow |

#### File Management Tables

| Table | Purpose |
|---|---|
| `contentFile` (2602) | Maps content to file paths, hashes, sizes — tracks cloud sync state |
| `imageFile` | Artwork file paths and hashes |
| `settingFile` | Settings file references |
| `contentCue` | Cue point file data (cloud sync) |
| `hotCueBanklistCue` | Hot cue bank file data |

#### System Tables

| Table | Purpose |
|---|---|
| `djmdProperty` | Database metadata (DBID, DBVersion, BaseDBDrive, DeviceID) |
| `djmdDevice` | Device registration |
| `agentRegistry` | Agent configuration key-value store |
| `uuidIDMap` | UUID to numeric ID mapping |
| `djmdMenuItems` / `djmdCategory` / `djmdSort` | Browser menu structure |

#### djmdContent — Full Column List

```sql
CREATE TABLE djmdContent(
    ID VARCHAR(255) PRIMARY KEY,
    FolderPath VARCHAR(255),       -- Absolute path on disk
    FileNameL VARCHAR(255),        -- Long filename
    FileNameS VARCHAR(255),        -- Short filename
    Title VARCHAR(255),
    ArtistID VARCHAR(255),         -- FK -> djmdArtist
    AlbumID VARCHAR(255),          -- FK -> djmdAlbum
    GenreID VARCHAR(255),          -- FK -> djmdGenre
    BPM INTEGER,                   -- BPM × 100 (same as USB export, e.g. 12990 = 129.90 BPM)
    Length INTEGER,                -- Duration in seconds
    TrackNo INTEGER,
    BitRate INTEGER,
    BitDepth INTEGER,
    Commnt TEXT,
    FileType INTEGER,              -- 1=MP3, 4=M4A, 5=FLAC, 11=WAV, 12=AIFF
    Rating INTEGER,                -- 0-5 stars
    ReleaseYear INTEGER,
    RemixerID VARCHAR(255),
    LabelID VARCHAR(255),
    OrgArtistID VARCHAR(255),
    KeyID VARCHAR(255),
    StockDate VARCHAR(255),
    ColorID VARCHAR(255),
    DJPlayCount INTEGER,
    ImagePath VARCHAR(255),
    MasterDBID VARCHAR(255),
    MasterSongID VARCHAR(255),
    AnalysisDataPath VARCHAR(255), -- Absolute path to ANLZ analysis
    SearchStr VARCHAR(255),
    FileSize INTEGER,
    DiscNo INTEGER,
    ComposerID VARCHAR(255),
    Subtitle VARCHAR(255),
    SampleRate INTEGER,
    DisableQuantize INTEGER,
    Analysed INTEGER,
    ReleaseDate VARCHAR(255),
    DateCreated VARCHAR(255),
    ContentLink INTEGER,
    Tag VARCHAR(255),
    ModifiedByRBM VARCHAR(255),
    HotCueAutoLoad VARCHAR(255),
    CueUpdated VARCHAR(255),
    AnalysisUpdated VARCHAR(255),
    TrackInfoUpdated VARCHAR(255),
    Lyricist VARCHAR(255),
    ISRC VARCHAR(255),
    SamplerTrackInfo INTEGER,
    SamplerPlayOffset INTEGER,
    SamplerGain FLOAT,
    VideoAssociate VARCHAR(255),
    LyricStatus INTEGER,
    ServiceID INTEGER,
    OrgFolderPath VARCHAR(255),
    -- Cloud sync fields
    UUID VARCHAR(255),
    rb_data_status INTEGER,
    rb_local_data_status INTEGER,
    rb_local_deleted TINYINT(1),
    rb_local_synced TINYINT(1),
    usn BIGINT,
    rb_local_usn BIGINT,
    created_at DATETIME,
    updated_at DATETIME
);
```

### Key Differences from USB Export Format

| Aspect | master.db | USB export (OneLibrary) |
|---|---|---|
| IDs | VARCHAR UUID strings | Sequential integers |
| BPM | Integer × 100 (e.g. 12990 = 129.90 BPM) — same as USB export | Integer × 100 (e.g. 13000) |
| Paths | Absolute local paths | USB-relative paths |
| Encryption | Key `402fd...08497` | Key `r8gdd...yqls` |
| Schema | 47 tables with cloud sync | 22 tables, export-only |
| Artwork | Referenced by absolute path | Embedded in `/PIONEER/Artwork/` |
| Playlists | Self-referential tree with `ParentID` | Flat with `playlist_id_parent` |

### Building a Tool That Reads master.db

The minimal approach to read a Rekordbox library without Rekordbox running:

```rust
use rusqlite::{Connection, params};

const MASTER_DB_KEY: &str = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";

fn open_master_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(&format!("PRAGMA key='{}'", MASTER_DB_KEY))?;
    conn.execute_batch("SELECT count(*) FROM sqlite_master")?; // verify decryption
    Ok(conn)
}

fn get_all_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut stmt = conn.prepare(
        "SELECT c.ID, c.Title, c.FolderPath, c.FileNameL, c.BPM, c.Length,
                c.TrackNo, c.BitRate, c.FileSize, c.SampleRate, c.FileType,
                c.Rating, c.KeyID, c.GenreID, c.ArtistID, c.AlbumID,
                ar.Name as Artist, al.Name as Album, g.Name as Genre,
                k.ScaleName as Key
         FROM djmdContent c
         LEFT JOIN djmdArtist ar ON c.ArtistID = ar.ID
         LEFT JOIN djmdAlbum al ON c.AlbumID = al.ID
         LEFT JOIN djmdGenre g ON c.GenreID = g.ID
         LEFT JOIN djmdKey k ON c.KeyID = k.ID
         ORDER BY c.ID"
    )?;
    // ... map rows to your track struct
}
```

### Will the Key Change?

No. The key has been stable across all Rekordbox 6.x and 7.x releases (2020–2026). Pioneer cannot rotate it without breaking every existing user's library. The only scenario where it would change is a major version migration (like the RB5→RB6 transition from XML to SQLCipher).

### Caveats

- **Do not open master.db while Rekordbox is running.** The database uses WAL mode and concurrent access can corrupt it.
- **The `dp` value in `options.json` is NOT the database key.** It's a device pairing token for the rekordboxAgent.
- **`product.db` uses the same encryption key** as `master.db`.
- **Cloud sync fields** (`rb_data_status`, `rb_local_synced`, `usn`) should not be modified if you want Rekordbox to continue syncing.

---

## 11. References

### Existing Documentation (starting points)

- [Deep Symmetry — PDB Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html)
- [Deep Symmetry — ANLZ Format](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html)
- [rekordcrate Library (Rust)](https://holzhaus.github.io/rekordcrate/)
- [Kaitai PDB Spec](https://github.com/Deep-Symmetry/crate-digger/blob/main/src/main/kaitai/rekordbox_pdb.ksy)
- [pyrekordbox — Device Library Plus support](https://github.com/dylanljones/pyrekordbox)
- [rbox — Rust/Python library for OneLibrary](https://docs.rs/crate/rbox/latest)
- [Pioneer database encryption research](https://github.com/liamcottle/pioneer-rekordbox-database-encryption)
- [Rekordbox OneLibrary announcement](https://rekordbox.com/en/2025/10/dj-brands-unite-to-launch-onelibrary/)

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
- OneLibrary exportLibrary.db decryption key and full schema (22 tables)
- ANLZ .2EX file structure: PWV7 (full-res 3-band) and PWV6 (overview 3-band) tags
- Dual-format requirement for supporting both legacy and new devices
- Rekordbox master.db location, encryption key, and full schema (47 tables)
- Key derivation from pyrekordbox obfuscated blob
- master.db vs USB export format differences (UUIDs vs integers, BPM scaling, paths)

---

*Tested on: CDJ-3000 firmware 3.19, April 2026*
*Source: [pioneer-usb-writer](https://github.com/) — Rust tool that writes CDJ-compatible USBs without rekordbox*
