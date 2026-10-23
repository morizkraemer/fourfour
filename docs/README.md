# fourfour — Documentation

Index of everything under `docs/`. Agents: start at [`AGENTS.md`](../AGENTS.md)
(root), then come here for depth.

Each entry is tagged: **[current]** actively maintained · **[reference]** durable
knowledge · **[historical]** captures a past phase, still useful context ·
**[archived]** superseded, kept for the record.

## architecture/ — how the system is built

| Doc | | Purpose |
|---|---|---|
| [`repo-overview.md`](architecture/repo-overview.md) | reference | Crate-by-crate map, what works on hardware, data flow, line counts. *(Predates the Svelte `app/`; for current UI see `ui/`.)* |
| [`ui-architecture.md`](architecture/ui-architecture.md) | current | UI architecture for the desktop app — stores, modules, IPC. |
| [`performance-2026-10.md`](architecture/performance-2026-10.md) | reference | Startup and UI performance: before/after numbers at 1k/10k/30k tracks, method, what is left. |
| [`tech-stack.md`](architecture/tech-stack.md) | reference | Survey of open-source DJ analysis tools (Essentia, madmom, CLAP, Demucs, …) — options and pain points per layer. |

## ui/ — UI specs (current phase)

| Doc | | Purpose |
|---|---|---|
| [`vision.md`](ui/vision.md) | current | The product/UX vision for the app. |
| [`components.md`](ui/components.md) | current | Component & mockup checklist. |
| [`design-system.md`](ui/design-system.md) | current | The fourfour design system — tokens, primitives, patterns. |
| [`review-2026-10-15.md`](ui/review-2026-10-15.md) | reference | UX review, 2026-10-15. |
| [`usb-sync-spec.md`](ui/usb-sync-spec.md) | current | USB sync spec. |
| [`usb-sync-decisions.md`](ui/usb-sync-decisions.md) | current | USB sync design decisions. |
| [`player-strip.md`](ui/player-strip.md) | current | Player strip implementation plan. |
| [`organize-sidebar.md`](ui/organize-sidebar.md) | current | Organize right-sidebar spec (rail, picker, dock, drop-to-playlist). |
| [`waveform-dynamics.md`](ui/waveform-dynamics.md) | current | Waveform display dynamics — playhead, scroll, band rendering. |

## analysis/ — analysis pipeline (research & plans)

`benchmark/` in this repo has reusable scripts (`scripts/cli_batch_analyze.py`),
small manifests, and baselines. Generated results, logs, cache, and local audio
datasets are gitignored. Samplebase is a separate project. The files below are
the research, plans, and reverse-engineering notes.

| Doc | | Purpose |
|---|---|---|
| [`pipeline-handoff.md`](analysis/pipeline-handoff.md) | historical | Library picks, accuracy numbers, code samples (April 2026). Describes a stratum-dsp grid design that is not wired; see the current docs below. |
| [`packaging-decision.md`](analysis/packaging-decision.md) | current | How to ship analysis without a Python venv; measured Rust prototype (`prototyping/analysis-packaging`). |
| [`benchmark-2026-10.md`](analysis/benchmark-2026-10.md) | current | October 2026 accuracy benchmark. |
| [`bpm-grid-2026-10.md`](analysis/bpm-grid-2026-10.md) | current | BPM and beat-grid results. |
| [`key-detector-2026-10.md`](analysis/key-detector-2026-10.md) | current | Key detector results. |
| [`analyzer-backend-review.md`](analysis/analyzer-backend-review.md) | historical | Analyzer backend review (June 2026). |
| [`experimentation-path.md`](analysis/experimentation-path.md) | reference | 6-phase experimentation plan (BPM/key accuracy → waveforms → scale → phrases/embeddings/stems). |
| [`cli-build-plan.md`](analysis/cli-build-plan.md) | historical | Plan for the Python analysis CLI. |
| [`benchmark-plan.md`](analysis/benchmark-plan.md) | historical | Original in-repo benchmark harness plan (benchmarking happened externally instead). |
| [`lexicon-benchmark-plan.md`](analysis/lexicon-benchmark-plan.md) | historical | Lexicon vs Python analysis benchmark plan. |
| [`key-detection-findings.md`](analysis/key-detection-findings.md) | reference | Key-detection benchmark findings. |
| [`lexicon-reverse-engineering.md`](analysis/lexicon-reverse-engineering.md) | reference | Lexicon DJ v1.10.7 — complete reverse engineering. |
| [`lexicon-deep-dive.md`](analysis/lexicon-deep-dive.md) | reference | Lexicon DJ — technical deep dive. |

## research/ — project reviews and landscape

| Doc | | Purpose |
|---|---|---|
| [`review-2026-10-15.md`](research/review-2026-10-15.md) | reference | Whole-project review, including doc drift. |
| [`landscape-2026-10-15.md`](research/landscape-2026-10-15.md) | reference | Landscape of similar tools. |
| [`synthesis-2026-10-15.md`](research/synthesis-2026-10-15.md) | reference | Synthesis of landscape and review. |
| [`fourfour-map.html`](research/fourfour-map.html) | reference | Feature map (HTML). |

## process/ — working state

| Doc | | Purpose |
|---|---|---|
| [`todo.md`](process/todo.md) | current | Running task list. |
| [`lessons.md`](process/lessons.md) | current | Lessons captured after corrections — read at session start. |

## archive/ — superseded

| Doc | | Purpose |
|---|---|---|
| [`session-handoff.md`](archive/session-handoff.md) | archived | Last session handoff snapshot. |
| [`waveform-bug-handoff.md`](archive/waveform-bug-handoff.md) | archived | Waveform bug investigation handoff (fix landed on `master`). |

## Elsewhere in the repo

- [`pioneer-usb-writer/reference-code/PIONEER.md`](../pioneer-usb-writer/reference-code/PIONEER.md) — **[reference]** reverse-engineered Pioneer binary format notes (the canonical format spec).
- [`prototyping/README.md`](../prototyping/README.md) — design-system prototyping (viewport).
- Per-crate `README.md` in `pioneer-usb-writer/`, `pioneer-test-ui/`, `analysis/`, `benchmark/`.
