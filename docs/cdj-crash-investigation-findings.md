# CDJ-3000 Crash Investigation — Complete Findings

**Date:** 2026-04-23
**Hardware:** CDJ-3000 (firmware 3.19)
**Problem:** Orange USB (our tool) crashes CDJ on track load. Blue USB (Rekordbox) works.

---

## Timeline of Fixes Applied Today

### v0.9.8 → v0.9.9
**Root cause:** PWV5 (color detail) format was changed in commit `d81d218` ("test") from the empirically-observed Rekordbox format `[amplitude, 0x80]` to a made-up `u16` bit-packed format `(height << 10) | ch3`. This caused the CDJ firmware to read garbage and crash.

**Fixes:**
- Reverted PWV5 writer to `[max_amplitude, 0x80]`
- Fixed `parse_pwv5` in reader to parse amplitude-only (greyscale)
- Changed reader preference from PWV5 to PWV3 (PWV3 has color, PWV5 is amplitude-only)
- PQT2 reverted to header-only (0 data entries) — writing bogus PQT2 data also crashes
- `analysedBits` changed from `105` to `41` (only bits we actually provide: waveform + beatgrid + color)
- Added `clean_dot_underscore()` to remove macOS `._*` AppleDouble files

**Result:** CDJ stopped crashing.

---

### v0.9.9 → v0.9.10
**Problem:** RGB waveform had "no amplitude" — looked like a spotty line. Also no 3-band waveform displayed.

**Root cause:** PWV4 bytes 0-2 (the actual color channels) were scaled down to 0-15 / 0-31 to match the old fake-green fallback. This crushed amplitude to near-zero.

**Also found:** PWV3/PWV5 entry counts were uncapped. For tracks > 5:41, `duration * 150` exceeds 51200. The CDJ firmware may allocate fixed 51200-byte buffers and overflow.

**Fixes:**
- PWV4 bytes 0-2 reverted to full 0-255 raw `low/mid/high`
- PWV3/PWV5 entry counts capped at 51200
- PQT2 header beat-count field fixed to `0` (it's header-only)

**Result:** RGB waveform became visible again, but still not proper 3-band color.

---

### v0.9.10 → v0.9.11
**Problem:** Still no true 3-band waveform on CDJ-3000.

**Root cause:** PWV4 is a proprietary 6-byte Pioneer format that we don't fully understand. The CDJ-3000 (non-X) "works without .2EX but displays lower-quality waveforms."

**Fixes:**
- PWV4 reverted to original format (bytes 0-2 raw, bytes 3-5 as original)
- Added `.2EX` file generation with PWV6 (overview) and PWV7 (full-res)
- `.2EX` uses standard 3-byte `[low, mid, high]` format per PIONEER.md
- Reader updated to prefer `.2EX` over `.EXT`
- `filesystem.rs` now writes `.2EX` for every track

**Result:** CDJ crashed again. The `.2EX` addition reintroduced a crash vector — possibly the CDJ-3000 trying to parse `.2EX` and hitting an unknown validation failure, or conflicting with `analysedBits = 41`.

---

## Current Working Hypothesis

The CDJ-3000 crash is caused by **any single malformed byte in the ANLZ files**. The firmware has strict validation and no graceful degradation. We found these specific crash vectors:

| Issue | Commit Introduced | Fix Status |
|-------|------------------|------------|
| PWV5 format changed to bogus `(height << 10) \| ch3` | `d81d218` | Fixed in v0.9.9 |
| PQT2 writing data entries with wrong encoding | Before working tree | Fixed in working tree |
| PWV4 amplitude crushed to 0-15 | v0.9.10 | Fixed in v0.9.10 |
| PWV3/PWV5 entry count > 51200 | `55aa3b8` | Fixed in v0.9.10 |
| `analysedBits = 105` with bit 64 set | Original | Changed to 41 in working tree |
| macOS `._*` resource forks | N/A | Fixed in working tree |
| `.2EX` file presence | v0.9.11 | **Possibly crashes CDJ-3000** |

---

## What We Know About Formats

### PWV5 (2 bytes/entry in .EXT)
- **Format:** `[amplitude, 0x80]` where amplitude = `max(low, mid, high)` (0-255)
- **Evidence:** Old writer comment: "Byte 1: 0x80 (observed constant in rekordbox exports)"
- **Purpose:** High-resolution amplitude detail. Color comes from PWV3/PWV4.

### PWV3 (1 byte/entry in .EXT)
- **Format:** `color_bits << 5 \| height` where height = 0-31
- **Color mapping:** 1=red/bass, 2=blue/high, 4=green/mid, 7=white
- **Purpose:** Color preview with low-res amplitude

### PWV4 (6 bytes/entry in .EXT)
- **Format:** UNKNOWN / proprietary Pioneer encoding
- **Old fallback:** `[height/2, height, height/2, height, height/3, 0]`
- **Current code:** `[low, mid, high, max_height, high/2, 0]` (bytes 0-2 at 0-255)
- **Purpose:** Color waveform overview (1200 entries)
- **⚠️ Problem:** We do NOT know the exact byte layout Rekordbox uses

### PWV6/PWV7 (3 bytes/entry in .2EX)
- **Format:** `[low, mid, high]` — well documented, interoperable
- **PWV7:** Full resolution, same entry count as PWV3/PWV5
- **PWV6:** Overview, 1200 entries
- **Purpose:** OneLibrary 3-band waveforms

---

## The Beat Grid Mismatch

Our beat grid aligns correctly in the waveform dev tool but appears shifted on the CDJ. Two likely causes:

1. **Cached `ANLZ0001.DAT` files** — The CDJ creates these when it rejects `ANLZ0000.DAT`. Even after fixing our files, the cached `ANLZ0001.DAT` may still be used. **Solution: Wipe the USB completely before each export.**

2. **BPM algorithm differences** — DeepRhythm/Lexicon produces slightly different BPM and beat positions than Pioneer's internal engine. This is expected and unavoidable without reverse-engineering Pioneer's analysis code.

---

## Recommended Next Steps

### Immediate (to stop crashing)
1. **Revert `.2EX` generation** if it's causing crashes on CDJ-3000. The `.2EX` file may be validated more strictly than `.EXT`.
2. **Wipe USB completely** before every test to eliminate cached `ANLZ0001.DAT` files.
3. **Test with one track** that is ~3-5 minutes long (well under 51200 entries) to eliminate entry-count issues.

### Short term (to get 3-band working)
1. **Hex-dump Rekordbox PWV4** — Compare our PWV4 bytes with Rekordbox's PWV4 bytes for the same track. Look at the first 10-20 entries. This is the only way to reverse-engineer the proprietary 6-byte format.
2. **Test `.2EX` independently** — Try exporting with `.2EX` but without `.EXT`, or vice versa, to isolate which file the CDJ-3000 objects to.
3. **Verify `analysedBits`** — Try `41` vs `105` to see if bit 64 enables/disables 3-band display.

### Long term
1. **Implement .2EX properly** once we know it doesn't crash.
2. **Add a hex-diff tool** to the waveform dev tool that compares our ANLZ with Rekordbox's ANLZ byte-by-byte.

---

## Code State at v0.9.12

- `pioneer-usb-writer/src/writer/anlz.rs` — PWV5 fixed, PWV4 reverted, PQT2 header-only, entry counts capped, `.2EX` writer added
- `pioneer-usb-writer/src/reader/anlz.rs` — Reader prefers `.2EX`, parses PWV6/PWV7
- `pioneer-usb-writer/src/writer/filesystem.rs` — Writes `.DAT`, `.EXT`, `.2EX`
- `pioneer-usb-writer/src/writer/onelibrary.rs` — `analysedBits = 41`

---

## v0.9.19 — RESOLUTION: Full 3-Band Color via `.2EX`

**Date:** 2026-04-23 (evening)

### Breakthrough
After binary bisection comparing our ANLZ files against Rekordbox's byte-by-byte, we isolated the crash to **PWV4** in `.EXT`. Rekordbox's PWV4 uses a proprietary 6-byte encoding that does not match any assumed `[R,G,B,max,secondary,white]` layout. Writing real RGB data into PWV4 crashes the CDJ-3000 firmware.

However, Rekordbox also writes `.2EX` files containing **PWV6** (1200-entry 3-byte overview) and **PWV7** (full-res 3-byte detail) in a clean, interoperable format. PIONEER.md documents that `.2EX` is **preferred** over `.EXT` when both are present.

### Test Results
| Configuration | CDJ Crash? | Colors? |
|---------------|-----------|---------|
| Fake-green PWV4, no `.2EX` | **No** | Green only |
| Fake-green PWV4, `.2EX` enabled | **No** | **Full 3-band** |
| Real RGB PWV4, no `.2EX` | **Yes** | N/A |

### Conclusion
The CDJ-3000 (non-X, firmware 3.19) **reads `.2EX` and displays real 3-band colors** when the `.2EX` file is present. It falls back to `.EXT` PWV4 only when `.2EX` is missing.

**Final strategy:**
- `.EXT` PWV4: always use fake-green fallback (safe, avoids proprietary encoding)
- `.EXT` PWV3/PWV5: real color/amplitude data (safe formats)
- `.2EX` PWV6/PWV7: real 3-band RGB + PWVC config (safe, preferred by CDJ)
- `.2EX` generation is now **enabled by default** in `write_usb`

### Files Changed in v0.9.19
- `pioneer-usb-writer/src/writer/filesystem.rs` — `.2EX` enabled by default
- `pioneer-usb-writer/src/writer/anlz.rs` — PWVC section added to `.2EX`; removed `SKIP_PWV4` env var
- `pioneer-test-ui/src/bin/pioneer-cli.rs` — New CLI for rapid testing
- `pioneer-test-ui/src/bin/anlz-diff.rs` — Diagnostic tool for comparing ANLZ files

---

## Key Insight

> The CDJ-3000 firmware does not tolerate ANY deviation from Rekordbox's format in PWV4/PWV5/PQT2. However, `.2EX` provides a safe, well-documented path to full 3-band color waveforms. The firmware prefers `.2EX` over `.EXT` when both are present, making `.2EX` the correct interoperability layer for modern players.
