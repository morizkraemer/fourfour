# Landscape synthesis, 2026-10-15

Merges `landscape-2026-10-15.md` (Claude subagent, primary sources) with a Perplexity Deep Research run Merlin pasted the same day (not stored verbatim). Unverified items are marked.

## Who else builds this

Open source, writes a Pioneer USB without rekordbox:

| Project | Lang | Licence | Both DBs | Own analysis | Hardware evidence | Source |
|---|---|---|---|---|---|---|
| Booth | Rust | Booth Public Source (not OSI) | yes | yes | CDJ-3000X fw 1.40, CDJ-1500X; CDJ-3000 fw 2.05 refused | Claude |
| cratedrop | Python | MIT | yes | yes | XDJ-AZ (OL), XDJ-XZ (legacy) | Claude |
| djbooth | TS/Rust/Swift | none stated | yes | yes | unverified | Claude |
| dj-usb-tkit | Rust + Tauri v2 | AGPL-3.0+ | OL not demonstrated (Perplexity) / per Booth doc on CDJ-3000X | Stratum, optional Essentia | partial | both |
| rbxport | Rust + Tauri + React | GPL-2.0+ (README) | yes | evolving | CDJ-900NXS froze (PR #245) | both |
| rekordlib | Zig | MPL-2.0 | yes (create/write) | no | library only | Perplexity |
| DJ Manager | Electron/JS | MIT | PDB only | Mixxx analyzer + FFmpeg | none named | both |
| Vynull | Go | GPL-3.0 | classic + Pro DJ Link | yes, incl. PSSI phrases | hobby-grade per README | both |
| baken expressport | Rust | MIT | OL optional | no | per README | Claude |
| rekordbox-pdb-rs | Rust | MIT | PDB, surgical incremental edits | no | byte-identical differential tests | both |
| rex | Go | unclear | PDB | no | "varying degrees", dormant | Perplexity |

Readers / specs: crate-digger + Kaitai (EPL/MPL/LGPL), pyrekordbox (MIT, v0.5.0 adds OneLibrary), rekordcrate (MPL-2.0), onelibrary-connect.
Commercial direct dual writers: Lexicon, MIXO (beta), djay Pro (official partner), VirtualDJ (official, Sept 2026), Alpha-DJ-Engine (claim only). Via Rekordbox: DJCU2, Mixed In Key.
Mixxx: reads Pioneer USBs, no export (#9463, #10321 open).

## Agreed facts

- Both DBs must carry the same library: CDJ-3000X is OneLibrary-only; CDJ-2000NXS2 / XDJ-XZ / CDJ-3000 (fw 3.22) read export.pdb.
- OneLibrary encryption is solved (fixed SQLCipher 4 key); the hard part is schema, ids, paths and parity with the PDB.
- No public accuracy benchmark of OSS grid/key analysis vs Rekordbox; key-detection tools agree poorly with each other.
- No AlphaTheta takedown found. The EULA bans reverse engineering; AlphaTheta warns against third-party library tools.
- Nobody publishes a player × firmware compatibility matrix or runs hardware CI.

## Conflicts / unverified

- CDJ-3000 fw 3.30 read OneLibrary first and was withdrawn (Booth doc) vs fw 3.22 "no OneLibrary" (AlphaTheta page). Both may be true; track per firmware.
- rbxport licence: GPL-2.0+ in README, NOASSERTION on GitHub.
- DeepRhythm licence: AGPL-3.0 (Claude) vs "check" (Perplexity).

## Checks against fourfour's code

- WAL trap: handled (`onelibrary.rs:195` checkpoints, stale `-wal`/`-shm` removed).
- `.2EX` not written; PWV6/PWV7 sit in `.EXT` (`anlz.rs:177`). Booth: they belong in `.2EX`, and wrong `.EXT` sections hid colour waveforms on a CDJ-3000X. Risk for 3000X.
- No PSSI (phrases). Only Vynull does this in OSS.
- ANLZ dir hash: Booth reports the CDJ-3000X ignores `analysisDataFilePath` and recomputes the path hash. Not checked in fourfour.
- Licence: Essentia + DeepRhythm AGPL in the analysis path; repo has no LICENSE.

## Where fourfour can stand out

1. Dual PDB + OneLibrary from one snapshot, by default (already the design).
2. Published compatibility ledger: model, firmware, filesystem, fixture hash, per-feature pass/fail.
3. Loss-safe, manifest-based incremental sync (temp + rename, delete last, decode-and-diff after write).
4. Parity inspector / gig-readiness checker: decode both DBs into one IR, report differences and broken refs.
5. Analysis with visible confidence (BPM alternatives, grid/key confidence, review badges).
6. Permissive Rust core + separate analysis process with a clear licence boundary.

## Patterns worth borrowing

- rb2engine: source → IR → write → decode → diff; atomic DB replacement; conversion report.
- rekordbox-pdb-rs: differential tests for byte-identical PDB.
- Rekordbox Compatibility Converter: rollback, free-space and path-containment checks.
- Booth `docs/onelibrary.md`: section order, WAL trap, path-hash dirs, players matrix.
- Test corpus: 1,000 playlists, >1,000-track playlist, deep folders, duplicates, Unicode/FAT-invalid paths, multi-page PDBs.
