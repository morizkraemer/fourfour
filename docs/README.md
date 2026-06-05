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
| [`tech-stack.md`](architecture/tech-stack.md) | reference | Survey of open-source DJ analysis tools (Essentia, madmom, CLAP, Demucs, …) — options and pain points per layer. |

## ui/ — UI specs (current phase)

| Doc | | Purpose |
|---|---|---|
| [`vision.md`](ui/vision.md) | current | The product/UX vision for the app. |
| [`components.md`](ui/components.md) | current | Component & mockup checklist. |
| [`design-system.md`](ui/design-system.md) | current | The fourfour design system — tokens, primitives, patterns. |
| [`player-strip.md`](ui/player-strip.md) | current | Player strip implementation plan. |
| [`organize-sidebar.md`](ui/organize-sidebar.md) | current | Organize right-sidebar spec (rail, picker, dock, drop-to-playlist). |
| [`waveform-dynamics.md`](ui/waveform-dynamics.md) | current | Waveform display dynamics — playhead, scroll, band rendering. |

## analysis/ — analysis pipeline (research & plans)

Benchmarking was done externally (samplebase project); no benchmark code lives
in this repo. These capture the research, plans, and reverse-engineering.

| Doc | | Purpose |
|---|---|---|
| [`pipeline-handoff.md`](analysis/pipeline-handoff.md) | reference | **Authoritative.** Library picks, accuracy numbers, code samples for every analysis layer. |
| [`experimentation-path.md`](analysis/experimentation-path.md) | reference | 6-phase experimentation plan (BPM/key accuracy → waveforms → scale → phrases/embeddings/stems). |
| [`cli-build-plan.md`](analysis/cli-build-plan.md) | historical | Plan for the Python analysis CLI. |
| [`benchmark-plan.md`](analysis/benchmark-plan.md) | historical | Original in-repo benchmark harness plan (benchmarking happened externally instead). |
| [`lexicon-benchmark-plan.md`](analysis/lexicon-benchmark-plan.md) | historical | Lexicon vs Python analysis benchmark plan. |
| [`key-detection-findings.md`](analysis/key-detection-findings.md) | reference | Key-detection benchmark findings. |
| [`lexicon-reverse-engineering.md`](analysis/lexicon-reverse-engineering.md) | reference | Lexicon DJ v1.10.7 — complete reverse engineering. |
| [`lexicon-deep-dive.md`](analysis/lexicon-deep-dive.md) | reference | Lexicon DJ — technical deep dive. |

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
