# fourfour

nicht-trash-rekordbox klon

## Run

```bash
cargo tauri dev              
> `cargo tauri dev` runs as a raw binary (no `.app` bundle), so AeroSpace can't match it by bundle-id.
> target via if.app-name-regex-substring = 'pioneer-test-ui'` instead

## CLI Tools

| Command | Crate | Use |
|---------|-------|-----|
| `fourfour-analyze` | `analysis/` | Analyze audio files (BPM, key, waveform, energy, cues). Python, uses Lexicon DSP. |
| `fourfour-benchmark` | `analysis/` | Run benchmarks against ground-truth datasets. |

## Project Structure

- **`pioneer-usb-writer/`** — Rust library. Scan metadata, write Pioneer USB formats (PDB, ANLZ, OneLibrary)
  - `models.rs` `scanner.rs` — Track types, tag reading via lofty
  - `writer/` — `filesystem.rs` (orchestrator), `pdb.rs`, `anlz.rs`, `onelibrary.rs`, `sync.rs`
  - `reader/` — `usb.rs` (read USB state), `masterdb.rs` (Rekordbox db), `anlz.rs`
  - `reference-code/` — Reference binaries + format docs
- **`pioneer-test-ui/`** — Tauri v2 test harness (vanilla HTML/JS frontend)
  - `src/main.rs` — Tauri commands (scan, analyze, write, read USB state)
  - `frontend/` — `index.html` + `app.js` + `style.css`
- **`analysis/`** — Python audio analysis CLI (`fourfour-analyze`, `fourfour-benchmark`)
  - `src/fourfour_analysis/` — `cli.py`, `analyze.py`, `bpm.py`, `key.py`, `waveform.py`, `energy.py`
- **`pioneer-library/`** — Rust crate, reads Rekordbox `master.db` (SQLCipher)
- **`benchmark/`** — Datasets, manifests, results, logs
- **`docs/`** — Architecture notes, plans, findings
