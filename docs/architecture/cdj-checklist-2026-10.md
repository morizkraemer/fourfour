# CDJ checklist — USB output changes merged 2026-10-26

For Moriz. These writer changes landed on master before any hardware test (Merlin's call). Details: `waveforms-3band-2026-10.md` (Export, Q40), `writer-bugs-2026-10.md`.

## Before you start
- **USB export still fails from 192 tracks** (PDB page split, `pdb.rs` `split_into_pages`, u16 offsets). Not fixed. Test with a stick of ≤150 tracks, or fix that first (repro tests in `pioneer-usb-writer/tests/writer_scale_repro.rs`, ignored).

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
