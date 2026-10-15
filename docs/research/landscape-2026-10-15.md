# Landscape research: open-source and commercial "Rekordbox replacement / Pioneer USB writer" tools

Research date: 2026-10-15. Stars and "last push" values come from the GitHub API or the repo page fetched on that date. "Unverified" means I did not confirm it in a primary source or a source disagrees. Where I only have a README claim, I say "README claim".

Scope note: fourfour itself is public (github.com/morizkraemer/fourfour, 4 stars, Rust, no licence detected by GitHub, no LICENSE file in the local tree). Other projects already cite it: Booth's `docs/onelibrary.md` lists "fourfour (CDJ-3000, firmware 3.19): a dual-format drive worked; the writer's author says the CDJ-3000 still needs the legacy .EXT" (https://raw.githubusercontent.com/jwilkins/booth/main/docs/onelibrary.md). A gist on OneLibrary also links fourfour's PIONEER.md (https://gist.github.com/0xdevalias/b803476793b56f7c45e6361799168eb0).

---

## Executive findings (read this first)

1. **The space is crowded as of Oct 2026, and almost all of it is new (2026).** At least 8 other projects write a Pioneer USB without rekordbox: Booth, cratedrop, dj-usb-tkit, djbooth, vynull (Linux), DjManager, Lexicon (closed), MIXO (closed), VirtualDJ (closed), Ora DJ (closed), baken/expressport, rbxport, Export Studio. Most are one-person, AI-assisted, 0-130 stars, and hardware-verified on one or two players at most.
2. **Closest competitors to fourfour's exact scope** (own analysis incl. phrases/colour waveforms + writes both DBs + analysis files): **Booth** (jwilkins, Rust), **cratedrop** (maurycyw, Python/MIT), **djbooth** (gakonst, TS/Rust), **dj-usb-tkit** (haivala, Rust+Tauri v2, AGPL). All four are very recent (created Jun-Aug 2026).
3. **Nobody has Deep-Symmetry-grade verification.** Only hardware results on record: fourfour/CDJ-3000 fw 3.19; Booth on CDJ-3000X fw 1.40 + CDJ-1500X; cratedrop on XDJ-AZ (OneLibrary) and XDJ-XZ (legacy); dj-usb-tkit on CDJ-3000X fw 1.31 (per Booth doc). baken/openboxxx/onelibrary(v1vendi) state **no player has read their output yet**.
4. **OneLibrary encryption is a solved technical problem and an open legal/ethical one.** Fixed key, identical on every drive, SQLCipher 4 defaults. Projects differ on whether they ship the key (rekordcrate PR #269 bundles it; cratedrop fetches it at runtime; pyrekordbox ships a downloader).
5. **Format knowledge is still incomplete** (unknown fields in `export.pdb` track rows, `PQT2`, `PWVC`, `contentLink`, `analysedBits`, `fileType`), and every writer finds new hardware pitfalls (row slots, playlist numbering, ANLZ path hash, section order).
6. **Licence trap for fourfour's analysis stack:** DeepRhythm is AGPL-3.0 and Essentia is AGPL-3.0. Beat This! is MIT, librosa ISC. See section (c).
7. **AlphaTheta has never taken anything down** that I could find (no DMCA/C&D evidence), but the rekordbox EULA bans reverse engineering and AlphaTheta publishes no OneLibrary spec.

---

## 1. Open-source projects and libraries

### 1.1 Format libraries and documentation (readers, parsers, specs)

| Project | Link | Lang | Licence | Stars / last push | Formats | R/W | Hardware verified | Own analysis | Notes / limits |
|---|---|---|---|---|---|---|---|---|---|
| Deep Symmetry **crate-digger** | https://github.com/Deep-Symmetry/crate-digger | Java | EPL-2.0 (README: with MPL-2.0 or LGPL-3 secondary options; GitHub shows NOASSERTION) | 235 / 2026-10-07; latest release v0.2.1 2025-07-25 | export.pdb, exportExt.pdb, ANLZ DAT/EXT via Kaitai `.ksy` | Read only (fetches over NFS from players) | Reads from real players over Pro DJ Link; models not listed on the repo page | No | Some tables undocumented; loads whole file, cannot use DB indices |
| Deep Symmetry **djl-analysis** (format docs) | https://djl-analysis.deepsymmetry.org/ ; export.pdb page https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/exports.html ; ANLZ page https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html | docs (Antora) | Site UI MPL-2.0; doc text licence not stated | updated after Dec 2025 (page says so) | export.pdb, exportExt.pdb, ANLZ | n/a | n/a | n/a | Credits Henry Betts, Fabian Lesniak, Dominik Stolz, Robin McCorkell, Swiftb0y, evilfred. `columns` (0x10) and `history` (0x13) tables unconfirmed; many unknown row/page fields |
| Deep Symmetry **beat-link** | https://github.com/Deep-Symmetry/beat-link | Java | EPL-2.0 | 294 / 2026-10-08; v8.0.0 2025-07-22 | Pro DJ Link network, uses crate-digger | Read | Many players (network side) | No | Not a USB writer |
| Deep Symmetry **dysentery** | https://github.com/Deep-Symmetry/dysentery | Clojure | EPL-1.0 | 267 / 2026-10-12 | Pro DJ Link protocol analysis | n/a | n/a | n/a | Protocol research basis for docs |
| **Kaitai specs** `rekordbox_pdb.ksy`, `rekordbox_anlz.ksy` | in crate-digger: https://github.com/Deep-Symmetry/crate-digger/tree/main/src/main/kaitai ; also kaitai_struct_formats https://github.com/kaitai-io/kaitai_struct_formats | Kaitai | Under crate-digger's licence election; kaitai_struct_formats shows no licence via API (unverified what headers say) | formats repo 797 stars | PDB, ANLZ | Read (Kaitai serialization only for Java/Python per Mixxx maintainer) | n/a | n/a | Mixxx uses them for import. Holzhaus doubted Kaitai could cope with writing (Mixxx #10321) |
| **pyrekordbox** | https://github.com/dylanljones/pyrekordbox ; PyPI https://pypi.org/project/pyrekordbox/ | Python | MIT | 495 / 2026-10-14; PyPI 0.5.0 uploaded 2026-10-14 | master.db (SQLCipher), OneLibrary/DLP `exportLibrary.db`, ANLZ DAT/EXT/2EX, rekordbox.xml, MySettings. export.pdb not mentioned on README (a search snippet lists USB .pdb as a feature, so unverified) | master.db: edit existing + add/delete in some tables; XML R/W; MySettings R/W; ANLZ write "planned" | Tested with rekordbox 5.8.6, 6.7.7, 7.0.9 (software only); no player claims | No | PWV6, PWV7, PWVC unsupported; DEVSETTING.DAT unsupported; CI can't test rekordbox-dependent features. Key is downloaded rather than shipped (see subbox issue below) |
| **rekordcrate** | https://github.com/Holzhaus/rekordcrate | Rust | MPL-2.0 | 213 / 2026-10-05 | export.pdb dump, ANLZ DAT/EXT/2EX dump, *SETTING.DAT | Read; PDB serialization still open since 2022 (issue #68, last activity Aug 2023: https://github.com/Holzhaus/rekordcrate/issues/68) | n/a | No | "Heavy development, breaking API". Open PR #269 adds read-only Device Library Plus (bundled SQLCipher + key + encrypted fixture), opened 2026-08-29, no reviews yet: https://github.com/Holzhaus/rekordcrate/pull/269 |
| **rbox** (Rust crate, also PyPI `rbox`) | https://crates.io/crates/rbox (repo https://github.com/dylanljones/rbox); PyPI https://pypi.org/project/rbox/ | Rust (+Python via maturin) | **GPL-3.0-only for 0.1.8**; older versions MIT or MIT OR Apache-2.0 (crates.io API) | 2,375 crate downloads; updated 2026-10-11 | master.db R/W, One Library R/W, ANLZ R/W (all three), XML R/W, MySettings R/W; export.pdb not mentioned | Read+write | None stated | No | Licence changed to GPL in the latest version, so pin an older version only if you need permissive terms (check licence of the exact version you pull) |
| **evanpurkhiser/rekordbox-parser** | https://github.com/evanpurkhiser/rekordbox-parser | TS | not identified on page | 15 / 2026-10-05 | PDB + ANLZ via crate-digger Kaitai | Read | n/a | No | "Types completely lacking" |
| **alphatheta-connect** (fork of prolink-connect) | https://github.com/chrisle/alphatheta-connect | TS | MIT | 28 / 2026-10-04 | Pro DJ Link, exportLibrary.db read, ANLZ (waveforms, cues, PSSI, vocal) | Read | Targets Opus Quad, XDJ-RX3, XDJ-XZ, CDJ-3000/3000X (testing not listed) | No | |
| **onelibrary-connect** | https://github.com/chrisle/onelibrary-connect | TS | MIT | 10 / 2026-09-26 | exportLibrary.db | Read only, built-in key | n/a | No | Uses better-sqlite3-multiple-ciphers |
| **rekordbox-pdb** (fragmede) | https://github.com/fragmede/rekordbox-pdb | Python | MIT | 2 / 2026-07-06 | export.pdb, exportExt.pdb | **Read+write**, incremental PdbEditor | "validated by opening edited sticks in rekordbox" (software, no player named) | No (does not create ANLZ) | Notes a DB entry alone does not make a CDJ play a track |
| **rekordbox-pdb-rs** | https://github.com/fragmede/rekordbox-pdb-rs | Rust | MIT | 2 / 2026-07-06 | same | R/W, byte-identical with the Python version | as above | No | std-only; a viable MIT PDB writer reference |
| **kuwa72/rekordbox-pdb**, **cdjplay** | https://github.com/kuwa72/rekordbox-pdb , https://github.com/kuwa72/cdjplay | Python / Rust | MIT (rekordbox-pdb) | 0 | PDB | R/W | none stated | No | Appears to be a copy/variant of fragmede's work (unverified) |
| **rekordbox-usb-write-guide** (coderDJing) | https://github.com/coderDJing/rekordbox-usb-write-guide | docs | MIT | 1 / 2026-07-22 | Layout doc for audio, ANLZ, artwork, export.pdb, exportLibrary.db | n/a | Defines "hardware verified" labels; reports none | n/a | Key invariant: PPTH == track path in both DBs; `analyze_path` == `analysisDataFilePath` |
| **Booth `docs/onelibrary.md`** | https://raw.githubusercontent.com/jwilkins/booth/main/docs/onelibrary.md | docs | Booth Public Source License (not OSI) | | OneLibrary schema, ANLZ section order, player matrix | n/a | see Booth | n/a | Best single OneLibrary write-up found. Contains corrections of earlier misclaims |
| **0xdevalias gist** | https://gist.github.com/0xdevalias/b803476793b56f7c45e6361799168eb0 | notes | n/a | | key obfuscation chain, links | n/a | CDJ-3000 fw 3.19 (from fourfour notes) | n/a | Author flags unverified/AI-generated parts |
| **kimtore/rex** | https://github.com/kimtore/rex | Go | none | 71 / 2025-08-13 | export.pdb from Mixxx | Write | "a few Pioneer devices with varying results", Denon Prime 4 showed no library | No | No waveforms, grids, cues. "Do not use on a live gig." Stalled since Aug 2023 per issue |

### 1.2 USB writers and library managers (the direct competitors)

| Project | Link | Lang | Licence | Stars / created / pushed | Writes | Hardware verified | Own analysis | Approach and known limits |
|---|---|---|---|---|---|---|---|---|
| **Booth** (jwilkins) | https://github.com/jwilkins/booth | Rust | Booth Public Source License (source-available, non-commercial/OSS redistribution OK, proprietary embedding needs licence; GitHub: NOASSERTION) | 13 / 2026-08-01 / 2026-10-11 | Both DBs (legacy + OneLibrary), ANLZ DAT/EXT/2EX | CDJ-3000X fw 1.40 (browse, play, key search, colour waveform, beat grid, hot cues after fixes); CDJ-1500X fw 1.10 browses; **CDJ-3000 fw 2.05 refused the drive (cause unknown)** | Tempo/grid, key, phrases, cues, colour waveforms, EBU R128, stems (demucs/Mel-Band-RoFormer variants), fingerprint ID | Spec doc `docs/rekordbox-replacement-spec.md`: autocorrelation + Ellis DP beat tracking, Sha'ath key profiles, Foote novelty for phrases. "No measured accuracy figures yet". Read-back verifier shares no code with writer |
| **cratedrop** (maurycyw) | https://github.com/maurycyw/cratedrop ; PyPI https://pypi.org/project/cratedrop/ | Python | MIT | 0 / 2026-08-19 / 2026-10-10 | export.pdb + ANLZ + exportLibrary.db | **XDJ-AZ (OneLibrary)**, **XDJ-XZ (legacy, 180+ tracks)** | BPM+grid via **Beat This!** (low-confidence flagged), key via essentia with keyfinder-cli fallback, hot cues A-D, waveforms, phrases, artwork | Constant/piecewise-constant grids only. macOS-first. OneLibrary key fetched at runtime from public docs or env var (not shipped) |
| **dj-usb-tkit** (haivala) | https://github.com/haivala/dj-usb-tkit | Rust + Tauri v2 + vanilla JS | **AGPL-3.0-or-later** | 40 / 2026-06-30 / 2026-10-14 | rekordbox PDB/eDB (README; OneLibrary not mentioned in README, but Booth doc says dj-usb-tkit hand-wrote dual-database sticks that passed 4 scenarios on CDJ-3000X fw 1.31), Engine DJ library (off by default) | CDJ-3000X fw 1.31 per Booth doc (not on its own README) | `stratum` (default) or `essentia` engines: BPM, key, waveform, artwork | Mirror/additive sync modes, backups, preview-first USB repair, diagnostics. **Same stack as fourfour** (Rust + Tauri v2). Linux AppImage/deb/rpm |
| **djbooth** (gakonst) | https://github.com/gakonst/djbooth | TS + Rust + SwiftUI | none stated | 34 / 2026-07-12 / 2026-10-12 | Legacy + OneLibrary, DAT/EXT/2EX, `link` for Pro DJ Link, play-history import | Names CDJ-2000NXS2, CDJ-3000, XDJ-XZ, XDJ-AZ "as examples"; no test matrix | Native `djbooth-rs analyzer`: BPM, key, grid, phrases, waveforms; stems via MLX | Claims "1:1 compatibility at the hardware boundary" with differential fixtures (README claim) |
| **vynull** (vynulldev) | https://github.com/vynulldev/vynull | Go (+Python helper for master.db) | GPL-3.0 | 28 / 2026-07-01 / 2026-10-15 | Virtual CDJ/rekordbox source over Pro DJ Link (no USB needed) AND `--generate` writes PDB+ANLZ+settings USB | Linux x86_64 tested; players not listed | BPM, key, colour waveforms, beat grids, cues | **Linux-native**; NFS server to decks; imports rekordbox XML/master.db/zip and Traktor NML |
| **DjManager** (Radexito) | https://github.com/Radexito/DjManager | Electron/React | MIT | 27 / 2026-01-24 / 2026-10-13 | PDB, ANLZ, settings, hot cues A-H, waveforms; checks FAT32/exFAT | none listed | Mixxx analyzer BPM, key, LUFS, intro/outro, beatgrid, FFmpeg waveforms | Issue #272 says CDJ-3000 needs smart playlist entries in PDB (claim, unverified in the issue) |
| **baken `expressport`** (M-Igashi) | https://github.com/M-Igashi/baken | Rust | MIT | 58 / 2026-10-12 | export.pdb, USBANLZ, My Settings; `--onelibrary` optional (PR #244) | Per README: CDJ-3000 fw 3.22, CDJ-2000NXS2 fw 1.85/1.87 played baken sticks. **OneLibrary: no player has read it yet** | No: copies rekordbox's local analysis cache, `--generate-analysis` for the rest | Needs a rekordbox collection.xml + rekordbox cache. Byte-compared with real rekordbox 7.2 export (586/587 ANLZ hashes; DAT/EXT/2EX regenerated files match for 304/294/309 of 325 tracks). Issue #208: stale OneLibrary WAL/SHM files after rewriting export.pdb caused rekordbox inconsistency warnings |
| **rbxport** (chrisle, TRIODE) | https://github.com/chrisle/rbxport ; site https://rbxport.com/ | Rust + Tauri 2 + React | **GPL-2.0-or-later** (README); GitHub: NOASSERTION; site says licence not stated | 126 / 2026-10-15; 1,437 commits | Legacy + OneLibrary USB, Link Export | CDJ-3000 and XDJ-RX3 for Link Export; **CDJ-900NXS froze on a stick it wrote** (issue #182, fixed in PR #245: row slots + playlist numbering; cause unconfirmed) | Beatgrid/key tuned for electronic music (site); uses existing rekordbox library as source | Free (donations). Linux AppImage/.deb. "Fewer features" philosophy. Still needs a rekordbox library to import |
| **Export Studio** (claw1200) | https://github.com/claw1200/export-studio | TS | GPL-2.0 | 3 / 2026-10-06 / 2026-10-14 | DeviceLibrary + OneLibrary, non-destructive to existing stick content | none listed | Tempo/grid/key estimation, dynamic grid planned, phrase analysis planned | Alpha; library stays in rekordbox DB |
| **openboxxx** (sparkly-quasar) | https://github.com/sparkly-quasar/openboxxx | C++ (lib) + Python verify | GPLv2 (builds on Mixxx) | 3 / 2026-07-29 / 2026-10-14 | export.pdb (20 tables + 8-colour palette), ANLZ beat grids and cues; OneLibrary is a goal | **None**, not even rekordbox desktop | Mixxx-derived grids/cues; no waveforms | Mixxx fork with an "Export to rekordbox USB" button; proposal linked in Mixxx #9463 |
| **v1vendi/onelibrary** | https://github.com/v1vendi/onelibrary | Python + JS | MIT | 7 / 2026-08-21 | Reads legacy; reads and writes OneLibrary (CLI); browser viewer decrypts with WebCrypto | None; developed without a OneLibrary player; "pre-alpha" | No | Strongest verification = round trip through rekordbox |
| **rb2engine** (jrgutier) | https://github.com/jrgutier/rb2engine | Python | MIT | 3 / 2026-07-25 | rekordbox USB to Engine DJ `m.db` (schema 3.0.1/3.0.2), in place | Not stated | No waveforms (Engine builds them) | Credits crate-digger and libdjinterop; ~680 tests; 8 hot-cue pad limit |
| **OneLibraryCompanion** (hdelplan) | https://github.com/hdelplan/OneLibraryCompanion | Rust (inferred) | GPL-3.0 | 13 / 2026-10-07 | Reads OneLibrary USBs, loads tracks via Pro DJ Link; does not state writing | Mac / Raspberry Pi testing incomplete | No | Companion, not a writer |
| **freedj-3000 / OpenDeck** | https://github.com/audiodestrukt/freedj-3000 | Rust | GPL-2.0-or-later | n/a | Reads PDB/ANLZ (vendored, patched rekordcrate) | n/a | No | Clean-room open deck; relevant as a consumer of the format |
| **cratemover**, **crate_doctor**, **rekordbox-library-fixer**, **rekordfix**, **rbMigrate**, **rekordbox-mcp**, **cdj-usb-fixer**, **transcrate** | e.g. https://github.com/koraysels/rekordbox-library-fixer (70 stars), https://github.com/mchernauskas/crate_doctor | various | various | small | Repair/audit/migrate/transcode helpers | | | Peripheral; transcrate (https://github.com/hiroaki222/transcrate) checks audio against CDJ specs |
| **rx3-toolkit** (Tratosca) | https://github.com/Tratosca/rx3-toolkit | Python | MPL-2.0 | 135 / 2026-10-01 | Mods XDJ-RX3 from USB (stems, key shift). Needs normal rekordbox export | Firmware 1.19/1.20 only | No | Not a library writer. README: key fetched from manufacturer's published source package after accepting terms; LEGAL.md not read |
| **cdj-nxs2-qemu / cdj2000-emulator / cdj3k-emu-windows** | https://github.com/Stijn-Jacobs/cdj-nxs2-qemu (67 stars), https://github.com/cdj2k-revival/cdj2000-emulator (14) | C | n/a | | Emulate player firmware | | | Potentially valuable for CI testing of USBs without hardware (not verified how complete). Existence of firmware emulators is notable for legal risk |

### 1.3 Mixxx

- **Import:** Mixxx 2.3+ imports rekordbox databases (read-only) via Kaitai; HFS+ `.PIONEER` detection fix merged into 2.5 on 2026-08-20 (https://github.com/mixxxdj/mixxx/pull/16895). Partial DLP code reportedly exists in `RekordboxFeature` (claim by contributor spchamp, unverified: https://github.com/mixxxdj/mixxx/issues/15556).
- **Export:** Mixxx 2.4 exports to Engine DJ USB (via libdjinterop; forum report: https://community.enginedj.com/t/mixxx-now-supports-exporting-to-engine/54743). **No Pioneer export is merged.**
- **Rekordbox export tracking issues:** #10321 (open since 2021; Dec 2025: AnnoyingTechnology reported a PDB writer that worked on an XDJ but with no waveforms/artwork/cues, then full compat incl. waveform `anlz_path` hash workaround; "killer feature" thumbs-up; https://github.com/mixxxdj/mixxx/issues/10321) and #9463 (https://github.com/mixxxdj/mixxx/issues/9463). Maintainer position (Swiftb0y, 2023): problem is "not really that we can't write the database" but not knowing the format well enough to always write a valid one. Holzhaus (rekordcrate maintainer): rekordcrate PR #71 prerequisite, then replace Kaitai.
- **OneLibrary request** #15556 (open since 2025-10-31, no PRs) and libdjinterop #177 (https://github.com/xsco/libdjinterop/issues/177): maintainers suggest building a low-level DLP API in libdjinterop first.
- **GSoC:** I found no Mixxx GSoC project on rekordbox/OneLibrary export (2012 page only notes there was no USB export; 2022-2024 pages list other ideas). The 2025/2026 pages were not checked: unverified.
- **"Mixxx declined to write OneLibrary on anti-circumvention grounds"** is stated in Booth's doc but I could not find it in the Mixxx issue thread: unverified.
- **libdjinterop** (https://github.com/xsco/libdjinterop): C++, LGPL-3.0, 82 stars, pushed 2026-10-14. Engine DJ format library. Not Pioneer.
- **AnnoyingTechnology/rhythmbox-to-pioneer-xdj-exporter:** repo archived 2026-05-30 and shows as empty (https://github.com/AnnoyingTechnology/rhythmbox-to-pioneer-xdj-exporter). Its PIONEER.md is referenced by Mixxx #10321 but I could not read it.

### 1.4 OneLibrary / master.db SQLCipher handling

- `exportLibrary.db` = SQLCipher 4 default settings, one fixed passphrase identical on every drive, recoverable from the rekordbox binary (Booth doc; gist; pyrekordbox says key shared across all DLP DBs and not tied to machine/licence).
- Obfuscation chain per onelibrary-connect `encryption.ts` (gist summary): base85 decode, XOR with fixed ASCII key, zlib inflate, UTF-8, prefix check.
- `master.db` uses a different key (gist). pyrekordbox auto-loads or downloads it; changelog says it downloads from projects that hard-coded it (search result summary, unverified wording). rekordlocksmith (https://github.com/Bide-UK/rekordlocksmith, MIT, 10 stars, last push 2023-11) and liamcottle's repo (https://github.com/liamcottle/pioneer-rekordbox-database-encryption, 36 stars, last push 2020) are older key-extraction tools.
- **WAL trap:** a fresh OneLibrary export holds most data in the `-wal`; opening only the main file shows a near-empty library with no error. Writers must checkpoint and delete `-wal`/`-shm` (Booth doc; baken #209 removes stale WAL/SHM).
- Who writes OneLibrary: rekordbox (>= 6.8.1 writes both DBs, Dec 2023 per Booth doc), djay Pro, Traktor (announced; shipment unconfirmed), VirtualDJ (official, Sept 2026), Lexicon, MIXO (unconfirmed which DB), plus the open-source writers above.

### 1.5 Library converters

| Tool | Link | Notes |
|---|---|---|
| Lexicon DJ | below | converts all major libraries |
| DJ Conversion Utility (ATGR) | https://sellfy.com/p/emUY | Mac only, one-off about EUR 24.50 on store (a DDJT review still says EUR 19.50), rekordbox/Serato/Traktor/VirtualDJ; Engine handled by separate Denon Conversion Utility. Writes libraries, not CDJ USB (unverified for 2026 versions) |
| Rekord Buddy | https://wearecrossfader.co.uk/blog/rekord-buddy-2-review/ | 2020 review: $49.99 first year then $29.99/yr; reads rekordbox only via XML; USB writing not stated; a comment says it went freeware (unverified). Current status/OneLibrary: unverified, I could not load a product page |
| rb2engine, rbMigrate, cratebridge | links in 1.2 | Niche |
| Mixed In Key | https://mixedinkey.com/workflows/export-cue-points-to-rekordbox/ | Analysis plus tags; cue export via rekordbox XML; no USB export found |

---

## 2. Commercial and closed tools

| Tool | Price | Platforms | Writes CDJ USB directly? | OneLibrary / hardware | User praise / complaints |
|---|---|---|---|---|---|
| **Lexicon DJ** (Rekordcloud, https://www.lexicondj.com/) | Free tier includes "Convert to Pioneer USB"; Essential and Ultimate are subscriptions; page shows no amounts and says prices rose Sept 8 (2026); an old cached page listed $10.49/mo, $249 lifetime Essential, $20.99/mo Ultimate (stale). | Win 10/11, macOS (Apple Silicon ok), iOS/Android app on paid | **Yes**, no rekordbox needed. Copies audio into `Contents`, generates waveforms, "CDJ Compatibility" scanner/auto-fix. | Writes **OneLibrary and DeviceLibrary** (manual: https://www.lexicondj.com/manual/sync-to-pioneer-usb). 1.11 blog calls USB sync a beta: https://www.lexicondj.com/blog/lexicon-1-11-highlights . Examples: CDJ-3000, XDJ-AZ, CDJ-2000, CDJ-900, XDJ-XZ. User settings only copied if drive first initialised in rekordbox | Praise: homepage testimonial "happy I don't have to use the useless Rekordbox" (vendor-hosted). Engine forum: transfers easiest in Lexicon (https://community.enginedj.com/t/what-are-your-experiences-with-lexicon-and-does-syncing-work-well-in-all-directions/43622). Complaints: price hard to justify; sync-to-Engine failures with album art; cue points not carried when syncing to USB from rekordbox library (https://discuss.lexicondj.com/t/cue-point-both-hot-cues-and-memory-cues-are-synced-to-rekordbox-but-from-there-they-are-not-being-synced-when-syncing-to-usb/2382). Vendor says always carry a rekordbox-made USB |
| **MIXO** (https://www.mixo.dj) | $7/month (DDJT June 2026 and DJTT 2023) | Win, Mac, **Linux**, iOS, Android | Yes (added 2026): "straight to Rekordbox USB" and Engine USB (https://www.digitaldjtips.com/mixo-now-plays-perfectly-with-pioneer-dj-alphatheta-gear/) | Whether it writes OneLibrary: not stated. Praise is from DDJT students (tutor with a stake). Roadmap item "Complete" with no release note, last reply 2 years ago (https://www.mixo.dj/roadmap/3237-cdj-compatibility) | Users wanted it to avoid rekordbox; Linux/Mixxx users asked for it. Cloud-library dependence is a trade-off |
| **Ora DJ** (https://www.ora-dj.com) | Free desktop; Ora+ cloud sync (price not listed) | macOS; Windows closed beta; **no Linux** | Yes: USB for CDJs, also Traktor and rekordbox XML | Specific formats not stated. Has key, energy, vocal detection, phrase map | Vendor comparison blog (marketing) |
| **VirtualDJ** | paid licence (price not checked) | Win/Mac | Yes since 2021 "CDJ export" (https://djmag.com/news/new-virtualdj-adds-cdj-export-including-playlists-and-stems, 403 to fetch; headline via search) | **Official OneLibrary support Sept 2026**: CDJ-3000X, 1500X, XDJ-AZ, AN, OPUS-QUAD, OMNIS-DUO (https://sonicstate.com/news/2026/09/15/-virtualdj-officially-supports-onelibrary/) | DDJT: change is "official sanction" more than a new capability |
| **djay Pro** (Algoriddim) | subscription (not checked) | Win/Mac (iOS read-only) | Yes | OneLibrary export since Oct 2025 launch (https://www.digitaldjtips.com/onelibrary/); fixes through May 2026 release notes | Android not supported |
| **Traktor** (NI) | not checked | Win/Mac | Announced for Pro 4/Play | Shipment unconfirmed | |
| **rekordbox 7** (AlphaTheta) | not checked | Win/Mac | Yes (reference) | Writes both libs from 7.2.11; 7.2.12 withdrawn; 7.2.5 renamed DLP to OneLibrary (rekordbox.com/en/2025/10/rekordbox-v725-release-information/) | Cloud Library slow (own FAQ); CDJ-3000X won't browse Device-Library-only USB; converting overwrites existing OneLibrary |
| **Engine DJ** | free (unverified) | Win/Mac | Own Engine format only; Denon players read rekordbox USBs and make an Engine library on the stick (forum anecdote) | **No OneLibrary support found** (DDJT, Substack) | |
| **Rekord Buddy** | see 1.5 | Mac (older review) | Unclear | Unknown | |
| **DJCU** (ATGR) | about EUR 24.50 one-off | Mac | Library converter (XML/DB); I found no evidence it writes USB | Works with rekordbox 6 and 7 per store page | One DJTT user: crashes with large track counts, so convert selected playlists |
| **Mixed In Key** | prices not on page | Mac/PC | No | n/a | Best key accuracy in third-party tests (section 4) |
| **GigSafe** (Gumroad, https://alexeneas.gumroad.com/l/GigSafe) | not loaded | | Works alongside rekordbox, per search summary (unverified) | | |

---

## 3. Format and hardware

### 3.1 OneLibrary timeline
- Device Library Plus (DLP) appears in rekordbox 6.8.1 (Dec 2023 per Booth doc; Lexicon blog Dec 21, 2023 per gist). Renamed **OneLibrary** in rekordbox 7.2.5 (Oct 21, 2025) and announced with Algoriddim + Native Instruments (https://alphatheta.com/en/information/dj-brands-unite-to-launch-onelibrary/).
- AlphaTheta FAQ: OneLibrary is "essentially" the DLP rebrand; spec not published; "full compatibility between software isn't guaranteed" (https://reallychrism.substack.com/p/why-im-not-cheering-onelibrary ; https://www.rekordbox.com/en/support/faq/onelibrary-7/).
- **Players (AlphaTheta notice dated 2026-03-18, https://alphatheta.com/en/information/important-notice-for-customers-using-usb-devices-with-our-dj-equipment/):**
  - OneLibrary: CDJ-3000X, OPUS-QUAD, OMNIS-DUO, XDJ-AZ (the rekordbox FAQ also lists CDJ-1500X and XDJ-AN).
  - Device Library: CDJ-3000, XDJ-XZ, XDJ-RX3, XDJ-RR, XDJ-1000MK2, XDJ-700, CDJ-900NXS2, CDJ-2000NXS2 and older.
  - CDJ-3000 firmware 3.30 prioritised OneLibrary; 3.22 listed as latest. Booth says 3.30 was withdrawn within weeks and that firmware 3.30 read OneLibrary first "even when stale or empty" (Booth doc; the notice confirms the 3.30 vs 3.22 split). Playlists in Device Library only can show on a CDJ-3000 but not a CDJ-3000X.
  - CDJ-3000X: "This unit only supports OneLibrary" (CDJ-3000X manual, quoted in Booth doc).
- Both DBs use **separate ids**; rekordbox keeps them in sync only from 7.1.4 (Booth doc). Always write the same collection to both.

### 3.2 Reverse-engineering state
- **export.pdb:** table types, page/row layout mostly known; unknown: `columns`, `history`, many unnamed fields, page flags, track-row bitmask, ISRC string form (Deep Symmetry exports page). Newer findings from implementers: track row word `+0x04` is `0x000c0700` in rekordbox, words `+0x14..0x1b` non-zero, meaning unknown; `+0x04` "CONTENTLINK" derivation unknown (rbxport PR #245 and #383 context). Row slot word (slot x 32 per page) and per-parent 0-based playlist `sort_order` must match rekordbox (rbxport PR #245; cause of CDJ-900NXS freeze unconfirmed).
- **ANLZ unknowns** (djl-analysis ANLZ page): PQTZ/PCOB/PCO2/PVBR/PWAV/PWV2-7/PSSI header unknowns; PWV7 non-linear scaling not matched; PSSI `k1-k3` flags and "Up 3" beats unclear; page is inconsistent about which of EXT/2EX holds PWV7.
- **Section order matters** and wrong sections silently break the rest of the file (Booth doc): `.DAT`: PPTH, PVBR, PQTZ, PWAV, PWV2, PCOB x2. `.EXT`: PPTH, PWV3, PCOB x2, PCO2 x2, PQT2, PWV5, PWV4, PSSI (phrases are in EXT, not 2EX). `.2EX`: PWV6, PWV7, PWVC. `PQT2`, short `PWVC` undocumented. PPTH path must be NUL-terminated and counted in length. A CDJ-3000 with no `.EXT` re-analyses on load.
- **ANLZ directory hash:** `P` + 3 hex + 8 hex, reportedly hash of drive-relative audio path mod 200003 (probable, not confirmed). CDJ-3000X fw 1.40 ignored `analysisDataFilePath` and recomputed it; Mixxx #10321 contributor also needed the `anlz_path` hash for waveforms. Collisions handled by `ANLZ0000`, `ANLZ0001`.
- **Unresolved OneLibrary fields:** `fileType` codes, `analysedBits`, `contentLink`, `myTagMasterDBID`, whether any player needs `exportExt.pdb`, whether `.2EX`/`PWVC` are required, why CDJ-3000 fw 2.05 refuses a legacy DB (Booth doc).
- **Schema:** 22 tables, no FKs, seeded static rows; `cue` table empty in rekordbox exports (cues come from ANLZ); `property.dbVersion` text `'1000'` (Booth doc).

### 3.3 Real hardware breakages (with source)
| Breakage | Source |
|---|---|
| CDJ-900NXS froze on playlist screen; USB LED blinking | https://github.com/chrisle/rbxport/pull/245 |
| CDJ-3000 (fw 2.05) did not show library at all | Booth doc |
| CDJ-3000X ignored `analysisDataFilePath`; misnamed analysis dirs = no waveforms/grid/cues; wrong `PQTZ` in `.EXT` hid colour waveforms | Booth doc |
| fw 3.30 empty-screen on previously working sticks (Oct 2025) | Booth spec + AlphaTheta notice |
| Stale OneLibrary WAL/SHM after rewriting PDB: rekordbox inconsistency warning, stale tracks pointing at deleted files | https://github.com/M-Igashi/baken/issues/208 |
| rekordbox 7.2.12 exported tracks that failed to load; withdrawn | search summary of AlphaTheta pages (https://djgear2k.com/rekordbox-device-library-plus-format-guide/), secondary source |
| E-8305 "Unsupported File Format" gives no detail; unsupported: OGG, WMA, ALAC, DRM AAC; FLAC only on newer players; paths over 255 chars fail; WAV_EXTENSIBLE, 32-bit float, AIFF-C, >96 kHz issues | https://www.lexicondj.com/manual/sync-to-pioneer-usb ; Booth spec |
| Unanalysed tracks load slowly or not at all | Pioneer forum threads, e.g. https://forums.pioneerdj.com/hc/en-us/community/posts/203106319-CDJ-2000-s-not-showing-BPM-or-Waveform-when-using-USB-sticks |

### 3.4 Filesystem, artwork, paths, limits
- **Filesystems:** CDJ-3000X: FAT16, FAT32, exFAT, HFS+, not NTFS (Booth doc). exFAT only on the CDJ-3000 line per Booth spec; other guides disagree on exact models (https://startingtodj.com/best-usb-format-for-rekordbox-and-cdjs-fat32-vs-exfat-vs-hfs/). Recommend FAT32 for max compat; MBR partition scheme. HFS+ drives store `.PIONEER` instead of `PIONEER` (Mixxx PR #16895).
- **Limits:** official AlphaTheta help says up to 10,000 files (https://support.alphatheta.com/en-us/articles/4410621344153 returned 403; figure via search snippet, treat as unverified). Booth spec: 8 folder levels, 10,000 entries per folder, paths under 256 chars, SD cards limited to 32 GB. Forum posts: 1,000 tracks per playlist (older), 1,000 playlists/folders (https://forums.pioneerdj.com/hc/en-us/community/posts/23196640976153), 100 lists and folders per USB (https://forums.pioneerdj.com/hc/en-us/community/posts/360000698506); these conflict, so unverified for current firmware.
- **Artwork:** `PIONEER/Artwork/00001/aN.jpg` (80x80) and `aN_m.jpg` (240x240); `bN` copies identical; ~20 images per folder; whether players read `b` files unknown (https://github.com/M-Igashi/baken/issues/235 via search). JPEG only, max 800x800 embedded (https://www.blisshq.com/music-library-management-blog/2020/03/03/pioneer-cdj-album-art/, third-party).
- **Path encoding:** PDB strings: short ASCII and UTF-16LE long forms (Deep Symmetry). Paths in OneLibrary are drive-relative with forward slashes (Booth doc).

### 3.5 Legal stance
- **No takedown or C&D found** against pyrekordbox, crate-digger, rekordcrate, or any writer (searched; GitHub's DMCA repo not checked, so absence is not proven).
- **EULA:** rekordbox EULA (updated 2025-03-04) section 2.2(g) prohibits reverse engineering, with a narrow carve-out where law permits and written notice is given (https://rekordbox.com/en/license-agreement/).
- **Key circumvention argument:** one developer's issue states shipping the DB key is "circumvention of an access control (DMCA 1201 and EU equivalents)" and notes pyrekordbox ships a downloader rather than the value (https://github.com/laker-93/subbox-app/issues/139). Opinion, not legal advice.
- **Spec sharing:** AlphaTheta shared OneLibrary privately with partners, per Mixxx #15556 comments; no public SDK (Booth doc).
- AlphaTheta "acquired Serato" claim appears in some commentary; Booth doc says a 2024 regulatory decision contradicts it. Unverified either way.

---

## 4. Analysis approaches and accuracy

| Task | What others use | Source |
|---|---|---|
| BPM + grid | Beat This! (cratedrop); autocorrelation + Ellis DP (Booth); Mixxx analyzer (DjManager); "stratum" or Essentia (dj-usb-tkit); Lexicon proprietary (no dynamic BPM yet) | repos above; https://www.lexicondj.com/manual/all |
| Key | Essentia + keyfinder-cli (cratedrop); Sha'ath profiles (Booth); Lexicon built-in or OpenKeyScan (free, Rekordcloud, https://rekord.cloud/, licence unverified) | |
| Colour waveforms (PWV4/5/6/7) | Booth generates all five packings, formulas not documented, acceptance by byte-comparison with rekordbox output; baken copies rekordbox's cache; Lexicon generates; djbooth/dj-usb-tkit native | Booth spec; baken README |
| Phrases (PSSI) | Foote self-similarity novelty snapped to 4-bar boundaries (Booth); Ora DJ "phrase map"; cratedrop and djbooth claim phrase analysis (README claims) | |
| Cues | Memory cue at first downbeat + up to 8 hot cues at phrase/vocal entries (Booth); cratedrop A-D; Mixed In Key up to 8 auto cues (https://djtechtools.com/2021/05/06/mixed-in-key-10-improved-key-detection-cue-point-exporting-a-ui-update/) | |
| Accuracy vs rekordbox | **No published beat-grid or BPM benchmark vs rekordbox found.** Key: DubSpot test of 200 tracks (2026): Mixed In Key 89%, KeyFinder 90% on 50 dance tracks, rekordbox 7 69%, Beatport last (https://blog.dubspot.com/dubspot-lab-report-mixed-in-key-vs-beatport); Crossfader: 38% of rekordbox keys differ from MIK (https://wearecrossfader.co.uk/blog/mixed-in-key-11/). Both are single-source blog tests. Academic beat tracking: Beat This! and madmom lead, SMC is hard (https://arxiv.org/pdf/2407.21658). DeepRhythm self-reports 95.91% Acc1 on 953 songs (author claim) | |

---

## 5. Community demand

- **Mixxx users:** #10321 and #9463 have run since 2018/2021; "killer feature", "+10000" (Mar 2026); Linux user on rekordcrate #68: "preparing USB keys for CDJs currently requires proprietary systems" (https://github.com/Holzhaus/rekordcrate/issues/68). HN thread: users offer bounties; rekordbox "runs poorly on Linux" (https://news.ycombinator.com/item?id=42770170).
- **Wine is blocked:** rekordbox detects Wine and blocks USB export (Change.org petition text, https://www.change.org/p/pioneer-dj-s-rekordbox-for-linux, secondary).
- **MIXO roadmap:** users say USB export is the only reason they stay on rekordbox or would make the subscription worthwhile (https://www.mixo.dj/roadmap/3237-cdj-compatibility).
- **OneLibrary pain:** Substack critique: "essential" data only, no MyTags/Engine/Serato, older CDJs still need rekordbox, personal CDJ settings not saved without rekordbox, format closed (https://reallychrism.substack.com/p/why-im-not-cheering-onelibrary).
- **rekordbox pain:** Cloud Library sync slowness (own FAQ), CDJ-3000X won't browse Device-Library-only USBs, slow USB export; the cause is often drive speed (https://www.beatmind.io/blog/export-rekordbox-usb-mac-fast).
- **Unmet requests seen:** smart playlists without the 1,000-track cap (Booth spec; Lexicon forum https://discuss.lexicondj.com/t/cdj-friendly-export-to-rekordbox-by-splitting-playlists-that-contain-more-than-1000-tracks/2433), dynamic BPM grids (Lexicon lacks), settings files, history import, FLAC handling on older CDJs, verification before the gig (Booth spec), Linux support.
- **What Linux DJs use today:** Mixxx for playing; a Mac/Windows machine or VM for rekordbox/Lexicon USB prep; rbxport (needs an existing rekordbox library; Linux AppImage/.deb); MIXO (Linux desktop); vynull (Linux, Pro DJ Link); dj-usb-tkit (Linux packages). Ora DJ and Lexicon have no Linux build. (Synthesised from the cited pages; I did not find Reddit threads, so Reddit sentiment is unverified.)

---

## (a) Master comparison table (open + closed, USB writers only)

| Tool | Licence | Lang | Writes legacy PDB | Writes OneLibrary | ANLZ DAT/EXT/2EX | Own analysis | Colour/3-band WF | Phrases | Hardware shown | Linux | Maturity |
|---|---|---|---|---|---|---|---|---|---|---|---|
| fourfour (owner) | none | Rust/Tauri/Svelte + Python | yes | yes | yes | DeepRhythm, Essentia, librosa | yes | yes | CDJ-3000 fw 3.19 | yes | proven on 1 player |
| Booth | BPSL | Rust | yes | yes | yes | yes | yes | yes | 3000X fw1.40, 1500X; 3000 fw 2.05 refused | not stated (macOS bundle) | young, honest docs |
| cratedrop | MIT | Python | yes | yes | yes | Beat This!, essentia | yes | yes | XDJ-AZ, XDJ-XZ | not tested (macOS) | young |
| dj-usb-tkit | AGPL-3 | Rust/Tauri v2 | yes | dual per Booth doc | likely | stratum/essentia | not stated | no | 3000X fw1.31 (via Booth) | yes | most similar stack |
| djbooth | none | TS/Rust/Swift | yes | yes | yes | yes | yes | yes | names 4 models | unknown | README-level claims |
| vynull | GPL-3 | Go | yes | no | yes | yes | yes | no | not listed | yes | hobby |
| DjManager | MIT | Electron | yes | no | yes | Mixxx analyzer | waveforms | intro/outro | none | likely | alpha |
| baken expressport | MIT | Rust | yes | opt-in, untested | yes (copies rb cache) | no | via cache | via cache | 3000 fw3.22, 2000NXS2 | yes | beta; needs rekordbox cache |
| rbxport | GPL-2+ | Rust/Tauri | yes | yes | yes | grid/key | yes | no | 3000, RX3; 900NXS froze | yes | active, 126 stars |
| Export Studio | GPL-2 | TS | yes | yes | n/a | grid/key | | planned | none | likely | alpha |
| openboxxx | GPLv2 | C++ | yes | goal | grid/cues only | no | no | no | none | yes | experimental |
| rex | none | Go | yes | no | no | no | no | no | mixed | yes | stalled |
| rekordbox-pdb(-rs) | MIT | Py / Rust | yes (R/W) | no | no | no | no | no | rekordbox desktop only | yes | library |
| Lexicon | closed | | yes | yes | yes | yes | yes | no | broad (vendor) | no | production, "beta" USB |
| MIXO | closed | | yes | unknown | likely | yes | | | broad (student claims) | yes | production |
| Ora DJ | closed | | yes | unknown | | yes | | phrase map | | no | production |
| VirtualDJ / djay | closed | | yes | yes | | yes | | | official | no | production |

## (b) Top 10 to study in depth

1. **Booth docs** (onelibrary.md, rekordbox-replacement-spec.md): ANLZ section order, WAL trap, hash naming, hardware matrix, read-back verifier concept. Read-only study because of the licence.
2. **Deep Symmetry djl-analysis + crate-digger Kaitai specs**: canonical field maps and unknowns; use as ground truth for tests.
3. **rbxport PR #245 and issues #182/#342/#383**: concrete byte-level PDB deltas vs rekordbox (row slots, sort order, track-row words), tested on CDJ-900NXS/XDJ-700. Check that fourfour's PDB matches these.
4. **baken expressport**: byte-diff harness against real rekordbox 7 exports, stale WAL/SHM handling, dry-run `plan`/`export` API, MIT so reusable ideas.
5. **cratedrop**: closest scope, MIT, Beat This! integration, hardware on XDJ-AZ and XDJ-XZ, key fetched at runtime pattern.
6. **dj-usb-tkit**: same stack (Rust/Tauri v2), mirror/additive sync, backups, repair, diagnostics; AGPL so study only.
7. **pyrekordbox DLP docs and tests**: schema reference, key handling, MIT. Docs URL I tried returned 404; browse the repo docs.
8. **rekordcrate PR #269**: Diesel schema + encrypted fixture; fixture is valuable for tests.
9. **rb2engine**: swap-in atomic write pattern, verify command, report JSON; adaptable for fourfour's incremental sync checks.
10. **Lexicon manual**: CDJ compatibility scanner rules (header patching, sample rates, path length, FLAC support) that fourfour can reimplement.
Honourable mentions: djbooth (Pro DJ Link + history import), vynull (Linux, NFS), coderDJing guide (validation levels), fragmede rekordbox-pdb-rs (MIT PDB writer).

## (c) Reusable components and licence compatibility

fourfour currently has no LICENSE file (GitHub detects none). Decide that first; the table assumes you want MIT/Apache.

| Component | Licence | Use in MIT/Apache app? |
|---|---|---|
| pyrekordbox | MIT | Yes |
| rbox crate **0.1.8** | GPL-3.0-only (older: MIT or MIT/Apache) | **No** at 0.1.8 unless the app becomes GPL; older versions OK |
| rekordcrate | MPL-2.0 | Yes as dependency (file-level copyleft on MPL files you modify) |
| crate-digger / Kaitai ksy from it | EPL-2.0 (+ MPL-2.0/LGPL-3 options) | Specs/generated parsers usable under MPL-2.0 option; keep notices |
| kaitai_struct_formats | no licence detected via API | Check file headers before copying (unverified) |
| rekordbox-pdb, rekordbox-pdb-rs, cratedrop, baken, DjManager, rb2engine, onelibrary (v1vendi), alphatheta-connect, onelibrary-connect, rekordbox-explorer | MIT | Yes |
| libdjinterop | LGPL-3.0 | Dynamic link OK; irrelevant to Pioneer |
| rbxport | GPL-2.0-or-later | No (study only) |
| dj-usb-tkit | AGPL-3.0 | No |
| vynull, OneLibraryCompanion | GPL-3.0 | No |
| openboxxx, Export Studio | GPL-2 | No |
| Booth | BPSL (non-OSI) | No for embedding in a permissive project |
| rex, djbooth, fourfour | no licence | All rights reserved by default; cannot reuse code |
| **Essentia** | **AGPL-3.0** (https://github.com/MTG/essentia) | Distributing it makes the app AGPL; running it as a separate CLI process still has distribution implications for bundling. Commercial licence available from MTG (not verified here) |
| **DeepRhythm** | **AGPL-3.0** (PyPI summary pages) | Same issue |
| Beat This! | MIT (code); model weights licence not checked | Yes (check weights) |
| librosa | ISC | Yes |
| madmom | NOASSERTION (code BSD-like, models CC BY-NC-SA, widely known; verify) | Models are non-commercial: unverified |

Implication: fourfour's Python analysis CLI (DeepRhythm + Essentia) already forces AGPL if bundled and redistributed. Either license fourfour AGPL/GPL-compatible, or swap to Beat This! + librosa + custom key (Sha'ath profiles as in Booth) to stay MIT/Apache.

## (d) "Who solved what" matrix

Legend: Y = implemented, claimed; H = hardware-shown by source; ? = unknown/unverified; - = not done.

| Hard problem | fourfour | Booth | cratedrop | dj-usb-tkit | djbooth | baken | rbxport | Lexicon | Others |
|---|---|---|---|---|---|---|---|---|---|
| OneLibrary writing | Y (CDJ-3000 dual) | Y, H (3000X) | Y, H (AZ) | Y, H (3000X via Booth) | Y ? | opt-in, no player yet | Y ? | Y (vendor) | v1vendi, Export Studio, onelibrary CLI: unverified/no hw |
| Incremental sync | Y | backups, read-back verify | flash/doctor | mirror/additive | USB cmd + history | prune, plan/dry-run | sync, backups | Full/Playlist/Modified sync | Export Studio non-destructive; fragmede PdbEditor incremental PDB |
| Colour/3-band waveforms | Y | Y, H (colour on 3000X after fixes) | Y ? | Y ? | Y ? | copies rekordbox cache | ? | Y | openboxxx: none; rex: none |
| Phrase analysis (PSSI) | Y | Y (phrases untested on HW) | Y ? | - | Y ? | via cache | - | - | Ora: phrase map (app-side) |
| Hot-cue colours | Y | hot cues Y (HW) | A-D | ? | ? | via cache | ? | Y | rb2engine maps colours to Engine; CueMirror (djay hot cues 9-16 to memory cues) |
| Artwork | Y | not written in OneLibrary (image_id=0) | Y | Y | ? | issue #235 open | ? | Lexicon: album art in Engine sync | PDB artwork + `Artwork/` layout documented via baken issue |
| Large libraries | Y (incremental) | smart playlists with no 1,000 cap (spec) | 180+ tracks tested | ? | ? | 586-track fixture | 40+ tracks, nested folders tested on 900NXS | vendor | rb2engine: 3,665-track stick; DJCU crashes on large lists (anecdote); rekordbox-pdb 713-track test |

## (e) Gaps and opportunities

1. **Verification across players is the missing moat.** Only fourfour (3000), Booth (3000X, 1500X), cratedrop (AZ, XZ) and baken (3000, 2000NXS2) cite hardware. A published, versioned compatibility matrix (player + firmware + feature) following the coderDJing "Observed / Structural / rekordbox / Hardware verified" levels would be unique.
2. **Fixture corpus + differential tests vs real rekordbox exports** (baken does ANLZ hashes; Booth does read-back). A shared open corpus of real exports would help everyone; rekordcrate PR #269 shows the community values fixtures.
3. **CDJ-3000 firmware 3.30 incident** shows drives must carry both DBs identical; a "gig-ready check" (both DBs consistent, WAL checkpointed, ANLZ hash dirs resolve, paths under 255) is cheap and differentiating. Lexicon has the audio compatibility part only.
4. **Linux-first, rekordbox-free, MIT/Apache** is open: rbxport is GPL and needs a rekordbox library, dj-usb-tkit is AGPL, cratedrop is macOS-first, Lexicon/Ora have no Linux.
5. **Dynamic beat grids**: Lexicon has none, cratedrop/Booth are constant or piecewise constant. 
6. **Settings files** (MYSETTING, DEVSETTING): pyrekordbox parses MySettings, no DEVSETTING; Lexicon says user settings can only come from rekordbox.
7. **Open accuracy benchmark vs rekordbox** (BPM/grid/key/phrases): none exists; Booth admits no figures.
8. **Smart playlists** on CDJ-3000 (DjManager issue #272 unresolved).
9. **History playlists and play-count import back** (djbooth, cratedrop harvest history).
10. **Mixxx integration**: Mixxx has repeatedly asked for a vetted writer; a clean MIT/Apache Rust writer crate with a stable API could become the upstream path (rekordcrate serialization still open).

## (f) Risks, with evidence

| Risk | Evidence |
|---|---|
| Firmware changes break exports | CDJ-3000 fw 3.30 read OneLibrary first and was withdrawn; notice Mar 2026 (AlphaTheta URL above); rekordbox 7.2.12 withdrawn; AlphaTheta preparing CDX-3000X loading changes (notice) |
| Silent hardware failures on untested players | CDJ-900NXS freeze with rbxport (PR #245); CDJ-3000 fw 2.05 refused Booth drive; Denon Prime 4 showed no library with rex |
| Undocumented fields | Deep Symmetry unknown list; rbxport `0x04`/`0x14..1b` unknowns; `analysedBits`, `contentLink`, `fileType` |
| Licence contamination | Essentia/DeepRhythm AGPL; rbox 0.1.8 GPL-3; Booth non-OSI |
| Legal / EULA | EULA 2.2(g); subbox issue on DMCA 1201 argument; AlphaTheta shares spec only with partners |
| Crowding / AI-generated clones | Many 2026 projects state AI assistance (e.g. rekordcrate PR #269 "models AI-generated", Mixxx #10321 Opus 4.5 credit); bugs may be copied between them (Booth doc corrects errors in rbox DDL and pyrekordbox model) |
| User data loss | Every project warns: carry a rekordbox-made backup stick (Lexicon, openboxxx, rex, dj-usb-tkit) |
| Key rotation | subbox issue: "Pioneer could rotate the key or change schema in any release" (opinion); a fixed key identical across drives today |

## (g) Link list

Format and docs
- https://djl-analysis.deepsymmetry.org/ , .../rekordbox-export-analysis/exports.html , .../anlz.html
- https://github.com/Deep-Symmetry/crate-digger , https://github.com/Deep-Symmetry/beat-link , https://github.com/Deep-Symmetry/dysentery
- https://github.com/kaitai-io/kaitai_struct_formats
- https://github.com/Holzhaus/rekordcrate (+ issues #68, PR #269)
- https://github.com/dylanljones/pyrekordbox , https://pypi.org/project/pyrekordbox/ , https://github.com/dylanljones/rbox , https://crates.io/crates/rbox , https://pypi.org/project/rbox/
- https://github.com/fragmede/rekordbox-pdb , https://github.com/fragmede/rekordbox-pdb-rs
- https://github.com/chrisle/onelibrary-connect , https://github.com/chrisle/alphatheta-connect , https://github.com/evanpurkhiser/rekordbox-parser
- https://gist.github.com/0xdevalias/b803476793b56f7c45e6361799168eb0
- https://raw.githubusercontent.com/jwilkins/booth/main/docs/onelibrary.md , https://raw.githubusercontent.com/jwilkins/booth/main/docs/rekordbox-replacement-spec.md
- https://github.com/coderDJing/rekordbox-usb-write-guide

Writers and managers
- https://github.com/jwilkins/booth , https://github.com/maurycyw/cratedrop , https://github.com/haivala/dj-usb-tkit , https://github.com/gakonst/djbooth , https://github.com/vynulldev/vynull , https://github.com/Radexito/DjManager
- https://github.com/M-Igashi/baken (PR #128, #244, issues #115, #139, #208, #235)
- https://github.com/chrisle/rbxport , https://rbxport.com/ , https://github.com/chrisle/rbxport/pull/245
- https://github.com/claw1200/export-studio , https://github.com/sparkly-quasar/openboxxx , https://github.com/v1vendi/onelibrary , https://github.com/jrgutier/rb2engine , https://github.com/hdelplan/OneLibraryCompanion , https://github.com/kimtore/rex , https://github.com/audiodestrukt/freedj-3000 , https://github.com/Tratosca/rx3-toolkit
- https://github.com/murtaza64/manadj (issue #91 read layer) , https://github.com/morizkraemer/fourfour

Mixxx
- https://github.com/mixxxdj/mixxx/issues/10321 , /issues/9463 , /issues/15556 , /pull/16895
- https://github.com/xsco/libdjinterop , .../issues/177
- https://community.enginedj.com/t/mixxx-now-supports-exporting-to-engine/54743
- https://news.ycombinator.com/item?id=42770170

Commercial and news
- https://www.lexicondj.com/ , https://www.lexicondj.com/manual/sync-to-pioneer-usb , https://www.lexicondj.com/blog/lexicon-1-11-highlights , https://rekord.cloud/
- https://www.mixo.dj/roadmap/3237-cdj-compatibility , https://www.digitaldjtips.com/mixo-now-plays-perfectly-with-pioneer-dj-alphatheta-gear/
- https://www.ora-dj.com/blog/best-rekordbox-alternatives
- https://sonicstate.com/news/2026/09/15/-virtualdj-officially-supports-onelibrary/ , https://www.digitaldjtips.com/onelibrary/
- https://alphatheta.com/en/onelibrary/ , https://alphatheta.com/en/information/dj-brands-unite-to-launch-onelibrary/ , https://alphatheta.com/en/information/important-notice-for-customers-using-usb-devices-with-our-dj-equipment/ , https://www.rekordbox.com/en/support/faq/onelibrary-7/ , https://rekordbox.com/en/license-agreement/
- https://reallychrism.substack.com/p/why-im-not-cheering-onelibrary
- https://wearecrossfader.co.uk/blog/rekord-buddy-2-review/ , https://sellfy.com/p/emUY (DJCU)
- https://blog.dubspot.com/dubspot-lab-report-mixed-in-key-vs-beatport , https://wearecrossfader.co.uk/blog/mixed-in-key-11/ , https://mixedinkey.com/workflows/export-cue-points-to-rekordbox/
- https://github.com/laker-93/subbox-app/issues/139 (legal argument)
- https://github.com/MTG/essentia , https://github.com/CPJKU/beat_this , https://pypi.org/project/deeprhythm/

## Things I could not verify (explicitly)
- Rekord Buddy's current price, platform, rekordbox 7/OneLibrary support, and whether it writes USBs.
- MIXO's price for the USB export tier and which DB it writes; GigSafe, DJ Conversion Utility's USB behaviour in 2026.
- Engine DJ's price and any OneLibrary plans.
- Traktor OneLibrary shipment.
- AnnoyingTechnology's PIONEER.md contents (repo archived and empty).
- Mixxx GSoC 2025/2026 pages; Mixxx "declined on anti-circumvention grounds".
- Reddit threads (not reachable via the search tool).
- GitHub DMCA repo search for rekordbox-related takedowns.
- Hardware claims in README-level projects (djbooth, DjManager, vynull, dj-usb-tkit) beyond what they state.
- Kaitai formats licence headers; madmom model licence.
