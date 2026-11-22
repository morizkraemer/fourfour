# USB hardware check, 2026-11-22

A headless check of USB export on the Mac with two real cards: a real Rekordbox DJ stick (FAT32, kept read-only) and an SD card in the built-in reader (FAT32, used for test exports next to its existing photos). Code: master `569c8c2`. No app window, no audio, nothing written to the DJ stick. Both cards were backed up first, and the backup was checked against the source (file count and bytes).

## How it was run

The repo has no headless export CLI, and no real `export.pdb` reader (only a small parser inside `pioneer-usb-writer/tests/export_roundtrip.rs`). So the check used a throwaway example binary, not committed, built against master. It drives the same calls the app makes:

- **Export:** `LocalLibrary` → `prepare_multi_sync(...).plan()` / `.run()` with `SyncSelection::Playlists`. Analysis was stored the way `analysis_run.rs` stores it: BPM and key copied into the track row, and colour waveforms measured from the audio with `waveform::Meter`. The beat grid is synthetic (124 BPM).
- **Read back:** `reader::read_usb_state`, `rekordbox::load_usb` + `load_analyses` (the importer), `reader::anlz::read_anlz` / `read_pwv6_overview`, and a page walk of the PDB modelled on the test parser (table pointers, data pages, row presence bits).
- **Checks:** a generic ANLZ tag walk, and our ANLZ path hash (`anlz_dir_for_path`) compared with each file's PPTH.
- **Volumes:** the app's removable rule (`volumes.rs`, copied into the tool) on every `/Volumes` entry, and `volume_capacity` compared with `df`.

Test audio: 8 clips of 30 s cut from test tracks already on the Mac. 6 WAV (one with an uppercase `.WAV`), 1 MP3, 1 AIFF. Names cover unicode (Latin accents, CJK, Cyrillic, an en dash), the FAT-illegal characters `? : * /`, a 225-character file name, two artist tags that differ only in case, and an artist tag ending in a dot.

## 1. The real Rekordbox stick (metadata from the backup)

Copied to dev (private, read-only): `PIONEER/rekordbox/*` and `PIONEER/USBANLZ/**`, 3799 files, 473 MB, no audio.

| What | Result |
|---|---|
| Layout | `Contents/<artist>/<file>`, `PIONEER/{Artwork, CDJ, DEVSETTING.DAT, djprofile.nxs, log, MPJ, rekordbox, TrashBox, USBANLZ}`, plus `PIONEER REC/` and `PRINT/` at the root. `rekordbox/` holds `export.pdb`, `exportExt.pdb`, `exportLibrary.db` (+ `-shm`, `-wal`), `RBFLTR.DAT`. `USBANLZ/USBMNG.DAT` (magic `PMNG`). |
| export.pdb | 20 tables, 501 pages; header sequence 8967 > highest data page sequence 8965. Tracks 1264 present (1385 rows incl. deleted). Artists 921, albums 851, labels 360, keys 47, genres 77, colours 8, playlist tree 45, playlist entries 519, history playlists 32, history entries 204, artwork 1106. Tables 16/17/18 (27/22/17 rows) match ours exactly. |
| exportExt.pdb | 9 tables; table 3 has 28 present rows (My Tags), table 7 one row. Our writer has no `exportExt.pdb`. |
| exportLibrary.db (OneLibrary) | **Empty.** 22 tables, but content, artist, album, playlist, cue and the rest all have 0 rows. `property`: dbVersion 10000, numberOfContents 0, created 2025-12-31. Category 22, menuItem 27, sort 17, myTag 28 (ours: 21/21/10/0, dbVersion 1000). Re-checked on a writable copy after a WAL replay: still 0. |
| ANLZ | 1264 tracks with `.DAT`/`.EXT`/`.2EX`. 4 folders also hold `ANLZ0001.*`, 1 also `ANLZ0002.*`. |
| ANLZ tags | DAT always `PPTH,PVBR,PQTZ,PWAV,PWV2,PCOB,PCOB`; 2EX always `PPTH,PWV7,PWV6,PWVC`. EXT varies: always PPTH, PWV3, PCOB x2, PCO2 x2, PWV5, PWV4; PQT2 in 1234/1264; **PSSI in 433/1264; PVB2 in only 82/1264**. |
| Our readers | `read_anlz` 1264/1264 OK (1.08 M beats, colour waveform on all), `read_pwv6_overview` 1264/1264, path hash = PPTH folder 1264/1264. The only "failure" is `USBMNG.DAT`, which is not an ANLZ file. `read_usb_state` / `load_usb` see **0 tracks** (empty OneLibrary; see issue below). |
| Rekordbox version | Not stored as text. OneLibrary dbVersion 10000 and PSSI phrase tags mean a recent Rekordbox (6/7); no exact number. |

## 2. Export round trip on the SD card

The SD card is MS-DOS FAT32, 16 KB clusters, protocol "Secure Digital", device location "Internal" (built-in reader), removable media yes.

| Step | Result |
|---|---|
| First export, 1 playlist, 8 tracks | Plan add 8, 38.2 MB to add, 56.1 MB needed (safety margin), fits. Done in 5.4 s. |
| Read back | PDB: 20 tables, 8 tracks, 5 artists, 4 albums, 1 playlist, 8 entries; header sequence 48 > data 46. OneLibrary: 8 tracks, 1 playlist, 8 entries (`load_usb` the same). ANLZ: 24 files; all read; colour waveform on all 8; path hash = PPTH 8/8. Titles, unicode names, durations, bitrates and sizes match the library. |
| Second export (2nd playlist, same tracks) | Incremental: 0 added, 7 unchanged, 1 updated; playlists 2, entries 11. |
| Third export, nothing changed | Again 7 unchanged, **1 updated** (see issue 42 below). |
| Our layout vs Rekordbox | Same `Contents/<artist>/<file>` and USBANLZ `P###/########/ANLZ0000.*` scheme; DAT and 2EX tag order identical. Ours lacks `exportExt.pdb`, `DEVSETTING.DAT`, `RBFLTR.DAT`, `USBMNG.DAT`, `djprofile.nxs`, `CDJ/`, `MPJ/`, `log/`, `TrashBox/`. Ours adds `PIONEER/fourfour-manifest.json`. Our EXT always carries PVB2 and never PSSI (Rekordbox: the reverse is common). OneLibrary dbVersion 1000 vs 10000. History tables are empty in ours (fine). |

## 3. Edge checks

| Check | Result |
|---|---|
| Removable detection | App rule: DJ stick (USB) and SD card (built-in Secure Digital, "Internal") both removable; a mounted `.dmg` correctly not removable; boot disk not removable. |
| Free space vs `df` | `volume_capacity` = `df` to the byte (total 30 681 350 144, free 29 206 659 072). |
| Filesystem | Both cards FAT32 (`msdos`). No exFAT card was available, so exFAT is untested. |
| Unicode names | Written and read back fine (Latin accents, CJK, Cyrillic, en dash). |
| FAT-illegal characters | `? : * /` become `_` in folder and file names; the original title is kept in the DB. |
| Long names | A 225-character file name works on FAT32 (limit 255). Our path cleanup has no length limit, so longer names would fail at write time (not tried). |
| Case | `test artist` and `Test Artist` share one folder on FAT (case-insensitive); the PDB keeps the lowercase path. Read-back by path still works. |
| Trailing dot | Artist `Artist Name.` makes a folder with a trailing dot. macOS FAT accepts it. Windows treats such names as invalid, and the CDJ is unknown. Not cleaned by the scanner. |
| AppleDouble | A source file with an xattr got a `._` sidecar on the card. The real stick has 258 such files, so this is normal for Mac-made sticks. |
| Uppercase `.WAV` | Written and read fine (#39 tracks the writer side). |
| AIFF tags | The AIFF clip came in as "Unknown Artist" with the right title. Possibly how ffmpeg writes AIFF tags rather than our scanner; unverified. |
| Stick full | 12 MB FAT image (`hdiutil -nobrowse`): plan says won't fit; run refuses with "Not enough space on FFTINY: needs 56.1 MB, 12.5 MB free. Nothing was written."; image left empty. A disk full mid-write (plan fits, write doesn't) was not tested. |

## Issues filed

- [#41](https://github.com/morizkraemer/fourfour/issues/41): a Rekordbox stick with an empty `exportLibrary.db` but a full `export.pdb` (like the real stick here) reports 0 foreign tracks. The UI warning and the "don't write straight away" gate never fire, and the first sync replaces `export.pdb` with only fourfour's tracks. Reproduced on dev against a copy of the stick's metadata: 1264 tracks in the PDB became 3. Import of the same stick finds 0 tracks with no error.
- [#42](https://github.com/morizkraemer/fourfour/issues/42): artist tags that differ only in case are merged by the OneLibrary writer but compared case-sensitively by the sync diff, so that track is "updated" on every sync.

## Clean-up

- SD card: only the new top-level `Contents/` and `PIONEER/` were removed (the clean-up refused anything that existed before). The listing is identical to before. DCIM is 224 files / 1 457 341 276 bytes, the same as the backup.
- DJ stick: only read (`diskutil info`, plus a `find`/`stat` count for the backup check). Mounted `noatime`; never written, renamed or ejected.
- The disk image was detached and deleted.

## Checklist for Merlin in the app (Mac)

1. Plug in the DJ stick with the app open: it should appear as a stick in the sidebar, with a capacity close to Finder's.
2. Open it and see what the app says is on it. Because of #41 it may look empty: its library is only in the old `export.pdb`.
3. **Do not press Sync on the real DJ stick until #41 is fixed**: it would hide its Rekordbox library from CDJs that read `export.pdb`.
4. Use the SD card or a spare stick instead: drag a playlist from the sidebar onto the stick card.
5. Press Sync: check the plan (track counts, "won't fit", "Analyze first" if anything is unanalysed) and the capacity bar against Finder.
6. Let it write: watch the progress line, then check the result counts.
7. Press Sync again without changes: it should have nothing to do (an artist spelled in two cases still shows 1 update, #42).
8. Take a playlist off with × and sync: its tracks should leave the stick; files you put there yourself stay.
9. Press Eject in the stick header: the card should leave Finder with no error.
10. Settings > Library > Import from Rekordbox > USB stick, pointed at the DJ stick (read-only): expect 0 tracks today (#41).
