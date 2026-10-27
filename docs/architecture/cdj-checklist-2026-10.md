# CDJ checklist — USB output changes merged 2026-10-26

For Moriz. These writer changes landed on master before any hardware test (Merlin's call). Details: `waveforms-3band-2026-10.md` (Export, Q40), `writer-bugs-2026-10.md`.

## Before you start
- **USB export failed from 192 tracks** (PDB page split). Fixed on `wu-writer`, see the 2026-10-27 section below. Until that is on the branch you test, use a stick of ≤150 tracks.

## What changed on the stick
1. New `.2EX` file next to `.DAT`/`.EXT`: `PPTH, PWV7, PWV6, PWVC`. PWV7/PWV6 moved out of `.EXT`.
2. `PWV6` (3-band overview) is now measured from the audio (Rekordbox-style rolling mean per band), not derived from PWV7. Bytes go up to ~127 like Rekordbox's (not capped at 45).
3. `PWVC` writes measured per-band gains instead of fixed 85/134/135.
4. `PWV5` now real u16 colour (`rrrgggbbbhhhhh00`); before it wrote `[amp, 0x80]`.
5. `PWV2` added to `.DAT`; `PWAV` header word 4 = `0x00010000`.
6. Colour detail no longer overflows past 7:17 (u64 index math).
7. Band values clamped ≤127. Section order per the doc.

## Check on each player you have (CDJ-3000, 3000X if available, anything older)
- [ ] Stick mounts, library browses, all playlists present.
- [ ] Tracks load and play (mp3, wav, flac; one track > 8 min).
- [ ] 3-band waveform shows on the deck (3000/3000X): zoom + overview; colours sensible.
- [ ] Older players: colour/blue waveform still shows (they ignore `.2EX`).
- [ ] Long track (> 7:17): waveform correct to the end, no garbage.
- [ ] Beat grid lines up on a few tracks (mp3 from LAME 3.100 especially: we shift those by −1 frame).
- [ ] Hot cues / memory cues present.
- [ ] Re-sync the same stick after removing a track: removed track gone, nothing else lost.

## If something breaks
Each change is in its own commit on the merged branch history (`git log -- pioneer-usb-writer/src/waveform pioneer-usb-writer/src/writer/anlz.rs`); revert the specific one and tell Merlin which check failed.

# CDJ checklist: writer fixes on `wu-writer` (2026-10-27)

Software-verified only (round-trip tests). Every on-stick byte change is listed; none has been on a player.

## Byte changes
1. **PDB, libraries over ~190 tracks.** Row offsets are now rebased per page. Output for small libraries (one page per table) is byte-identical to before. Larger libraries produce multi-page tables that did not exist before (chained `next_page`, header page `last_page`/`empty_candidate` as for the existing overflow tables). The per-row `index_shift` (`row_index * 0x20`) now wraps at 16 bits instead of panicking in debug; it keeps counting across pages as before.
2. **ANLZ `PCPT` (`.DAT`, and the empty `.EXT` ones stay empty).**
   - `status` (0x10) is `1` (was `0`) for every cue.
   - Entry type byte (0x1C) is `2` for loops (was always `1`). Plain cues stay `1`.
3. **ANLZ `PCP2` (`.EXT`).**
   - Entry type byte (0x10) is `2` for loops.
   - Comment: `len_comment` at 0x28 and the cue name as NUL-terminated UTF-16BE at 0x2C. Entry length (0x08) grows by the comment size. Cues without a name are unchanged (88 bytes, `len_comment` 0).
   - Colour: byte right after the comment is the palette index 1 to 16, followed by R, G, B. Cues without a colour stay zero.
4. **OneLibrary `cue` table.** `colorTableIndex` is the palette index 1 to 16 (was always 0); `cueComment` is the cue name (was `''`).

Palette: fourfour's 16 colours in order (pink, red, orange, yellow, lime, green, mint, aqua, sky, blue, indigo, purple, magenta, rose, peach, white), index = position. `src/cue_color.rs`. The order and the RGB values are assumed to match Rekordbox's cue colour table, not read from a player.

## Check on each player
- [ ] Stick with more than 200 tracks (try 1000): mounts, browse by artist/album/track, every track loads, every playlist has all its tracks.
- [ ] Hot cue with a name and colour: name shows on the deck / in the cue list, pad lights in the set colour. If the colours are off (wrong hue, not just a shade), the palette index to colour mapping is the suspect: note which index showed which colour.
- [ ] Memory cue with a name: name shows in the memory list.
- [ ] A loop saved as a hot cue and as a memory cue: shows as a loop (not a point), jumps and loops. If it shows as a point, `PCP2`/`PCPT` type `2` is wrong.
- [ ] Cues with status `1`: still all listed. If a cue disappeared, `PCPT` `status` is the first thing to revert (`writer/anlz.rs`, `build_cue_section`).
- [ ] A non-ASCII cue name (umlaut, accent) displays.
- [ ] Older player (CDJ-2000NXS2 / XDJ): names and colours may not show at all (`.DAT` carries no names); the cues themselves must still be there.

## If something breaks
Commits are separate: `fix: write valid PDB pages for any library size` (PDB), `feat: write cue names, colours and loop type` (ANLZ and OneLibrary). Revert the one that matches the failed check.
