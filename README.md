# fourfour

A lightweight, free and open source DJ library app for the management side of
rekordbox: scan your music folders, analyze the tracks, organize playlists and
sync them to a USB stick that plays on Pioneer CDJs. No decks, no performance
features, no rekordbox needed.

"fourfour" is a working title. Built by two brothers,
[@merlinkraemer](https://github.com/merlinkraemer) and
[@morizkraemer](https://github.com/morizkraemer).

## Status

Early alpha, macOS first. Builds are unsigned and only shared as drafts.

- **Library and player:** folder scan, playlists and folders, waveform player with
  beat grid and hot cues, inspector. Duplicate cleanup, moved-file reconnect and
  playlist export (M3U8, CSV, Traktor NML, rekordbox XML) are on the `rc` branch,
  in the next build.
- **Analysis:** fully local, in Rust (Beat This! ONNX model for beats and
  downbeats, own key model). On 698 Beatport previews, scored on the same tracks
  against Beatport's labels: key 58% exact vs rekordbox 51% on the held-out
  split, BPM on par (details: `docs/analysis/h2h-rekordbox-2026-11-22.md`, on
  branch `h2h-rekordbox`). About 5x faster than rekordbox analysing a 512-track folder (6:44 vs 34:19)
  on an M5 Pro. Energy exists but is off by default; phrase detection is in progress on a branch.
- **USB sync:** writes export.pdb, exportLibrary.db (OneLibrary) and ANLZ
  analysis files. Early exports were checked on a CDJ-3000. A new writer (v2:
  keeps rekordbox's own content on a stick, staged sync with recovery) is on a
  branch and not yet tested on hardware.
- **Platforms:** macOS (Apple Silicon) is the target. Windows and Linux build in
  CI, untested by hand.

## Run

```bash
cargo build                      # Rust workspace
cd pioneer-test-ui && ./dev.sh   # desktop app (Tauri), fast rebuild + relaunch
cd app && npm run dev            # frontend alone (Vite, :5200)
scripts/mac/build-dmg.sh         # unsigned macOS dmg (on a Mac)
```

## Project structure

```
fourfour/
├── app/                  Svelte 5: the desktop UI
├── pioneer-test-ui/      Rust: Tauri v2 shell and app commands
├── pioneer-library/      Rust: SQLite library, import, USB sync engine
├── pioneer-usb-writer/   Rust: Pioneer formats (PDB, OneLibrary, ANLZ, artwork), scanner, waveforms
├── fourfour-analysis-rs/ Rust: analysis (BPM, beat grid, key, energy)
├── fourfour-stretch/     Rust: time stretching for the player
├── analysis/             Python: older analysis prototype and benchmarks (not shipped)
├── benchmark/            scripts, manifests, baselines (datasets are local)
├── scripts/              build, perf and Mac scripts
├── lab/                  review cards for UI decisions
├── prototyping/          design-system playground
└── docs/                 architecture, UI decisions, analysis research
```

## Docs

- [AGENTS.md](AGENTS.md): contributor and agent entry point (architecture, build, format gotchas)
- [docs/README.md](docs/README.md): full documentation index
- `pioneer-usb-writer/reference-code/PIONEER.md`: Pioneer format notes

## Licence

GPL-3.0-or-later, see [LICENSE](LICENSE). The analysis and stretch crates are
MIT / Apache-2.0. Model weights carry their own licences (see the bundled
licence files). Pioneer, rekordbox and CDJ are trademarks of AlphaTheta
Corporation; fourfour is not affiliated with them and names them only to
describe compatibility.
