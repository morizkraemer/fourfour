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

```
fourfour/
├── pioneer-usb-writer/     # Rust library — scan metadata, write Pioneer USB formats
│   ├── src/
│   │   ├── models.rs       # Track, AnalysisResult, Playlist types
│   │   ├── scanner.rs      # Reads tags via lofty
│   │   ├── writer/
│   │   │   ├── filesystem.rs  # Orchestrator: copy audio, artwork, call sub-writers
│   │   │   ├── pdb.rs         # export.pdb (legacy DeviceSQL)
│   │   │   ├── anlz.rs       # ANLZ0000.DAT/.EXT (beat grids, waveforms, cues)
│   │   │   ├── onelibrary.rs  # exportLibrary.db (SQLCipher OneLibrary)
│   │   │   └── sync.rs       # Incremental USB sync (read-back + merge)
│   │   └── reader/
│   │       ├── usb.rs         # Read existing USB state
│   │       ├── masterdb.rs    # Read Rekordbox master.db
│   │       └── anlz.rs        # Parse ANLZ files
│   └── reference-code/     # Reference binaries + format docs
│
├── pioneer-test-ui/        # Tauri v2 test harness (vanilla HTML/JS frontend)
│   ├── src/
│   │   ├── main.rs         # Tauri commands (scan, analyze, write, read USB state)
│   │   └── dto.rs          # Frontend DTOs
│   ├── frontend/           # index.html + app.js + style.css
│   └── tauri.conf.json
│
├── analysis/               # Python audio analysis (Lexicon DSP stack)
│   └── src/fourfour_analysis/
│       ├── cli.py          # fourfour-analyze / fourfour-benchmark entrypoints
│       ├── analyze.py      # Full analysis pipeline
│       ├── bpm.py          # BPM detection (Lexicon)
│       ├── key.py          # Key detection (Lexicon)
│       ├── waveform.py     # Pioneer-compatible waveform generation
│       └── energy.py       # Energy/segmentation
│
├── pioneer-library/        # Rust crate — read Rekordbox master.db (SQLCipher)
│
├── benchmark/              # Benchmark datasets, manifests, results, logs
│
├── mockup/                 # Static HTML UI mockup
│
└── docs/                   # Architecture notes, plans, findings
```
