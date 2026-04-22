# fourfour

nicht-trash-rekordbox klon

## Run

```bash
cargo tauri dev              
> `cargo tauri dev` runs as a raw binary (no `.app` bundle), so AeroSpace can't match it by bundle-id.
> target via if.app-name-regex-substring = 'pioneer-test-ui'` instead
```
## CLI Tools

| Command | Crate | Use |
|---------|-------|-----|
| `fourfour-analyze` | `analysis/` | Analyze audio files (BPM, key, waveform, energy, cues). Python, uses Lexicon DSP. |
| `fourfour-benchmark` | `analysis/` | Run benchmarks against ground-truth datasets. |

## Project Structure

```bash
├── analysis/
│   └── src/fourfour_analysis/
│       ├── analyze.py
│       ├── bpm.py
│       ├── cli.py
│       ├── energy.py
│       ├── key.py
│       └── waveform.py
├── benchmark/
├── docs/
├── pioneer-library/
│   └── src/
├── pioneer-test-ui/
│   ├── frontend/
│   └── src/
├── pioneer-usb-writer/
│   ├── reference-code/
│   └── src/
│       ├── reader/
│       ├── writer/
│       ├── models.rs
│       └── scanner.rs
└── mockup/
```
