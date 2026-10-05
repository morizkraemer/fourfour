# fourfour

## Agent entry

- Format library: `pioneer-usb-writer/`. Library database and USB sync: `pioneer-library/`. Tauri shell: `pioneer-test-ui/`.
- Desktop UI: `app/` (Svelte, `:5200`). Analysis CLI: `analysis/` (`fourfour-analyze`, `fourfour-benchmark`).
- In-repo benchmark scripts: `benchmark/scripts/`. Waveform harness: `ui/waveform/` and `./waveform-dev`.
- Agent rules: [AGENTS.md](AGENTS.md). Doc index: [docs/README.md](docs/README.md).
- Static mockups in `mockup/` and Pencil files in `designs/` are not the app.

Write Pioneer CDJ-compatible USB drives without Rekordbox. Scan your music,
analyze it, export a drive that plays on real CDJ hardware.

> Status: the format library is proven on a CDJ-3000. The desktop UI (`app/`)
> and the analysis pipeline (`analysis/`) are under active development.

## Run

```bash
cargo build                      # Rust workspace
cd pioneer-test-ui && ./dev.sh   # desktop app (Tauri) — fast rebuild + relaunch
cd app && npm run dev            # frontend alone (Vite, :5200)
```

> `cargo tauri dev` runs as a raw binary (no `.app` bundle), so AeroSpace can't
> match it by `bundle-id` — target `if.app-name-regex-substring = 'pioneer-test-ui'`.

## CLI Tools

| Command | Crate | Use |
|---------|-------|-----|
| `fourfour-analyze` | `analysis/` | Analyze audio (BPM, key, waveform, energy, cues). Default backend is `deeprhythm_essentia`. `lexicon_port` is one of the backends. |
| `fourfour-benchmark` | `analysis/` | Run analysis against ground-truth datasets. |

## Project Structure

```
fourfour/
├── pioneer-usb-writer/   Rust  — format library (PDB / OneLibrary / ANLZ / artwork)
├── pioneer-library/      Rust  — persistent SQLite library + USB export & sync
├── pioneer-test-ui/      Rust  — Tauri v2 shell (hosts app/, bundles reference analyzer)
├── app/                  Svelte — the desktop UI (fourfour-app)
├── prototyping/          Svelte — design-system playground (viewport)
├── analysis/             Python — analysis CLI (fourfour-analyze / -benchmark)
├── benchmark/                   — scripts, manifests, baselines (datasets and results are local)
├── ui/waveform/                 — waveform dev harness (opened by ./waveform-dev)
├── mockup/                      — static HTML brand mockups
├── designs/                     — Pencil design file
└── docs/                        — architecture, UI specs, analysis research (see docs/README.md)
```

## Docs

- **[AGENTS.md](AGENTS.md)** — agent/contributor entrypoint (architecture, build, gotchas)
- **[docs/README.md](docs/README.md)** — full documentation index
- **`pioneer-usb-writer/reference-code/PIONEER.md`** — reverse-engineered Pioneer format notes
