You are a worker on fourfour, a Tauri v2 + Svelte 5 + Rust DJ library app. Read AGENTS.md first (rules, "## Unattended", lanes: sync/format code is ours now, Mo tests on CDJs). You run in your own git worktree; branch `stick-play-tab` off master.

Merlin's feedback from testing on his Mac (2026-11-22), verbatim:
"2. cant play tracks from the card in the app - we also need full backwards compability where i can open a usb stick i synced with rekordbox itself and play the tracks and pull playlists into fourfour etc
5. sometimes tab still does focuisng elements not opening player (if no track is loaded in player for example"

TASKS
A. Item 5 (bug): find the Tab key handling (Tab should open/focus the player per the app's keyboard design; see the r174 focus work and docs on keyboard). Reproduce in vitest/Playwright: with no track loaded, Tab moves browser focus instead. Fix the root cause so Tab behaves the same with or without a loaded track (decide what "open player" means with no track: show the empty player). Test it.
B. Item 2a (bug or gap): tracks on a fourfour-synced stick cannot be played in the app. Find why (stick track rows not wired to the player, path mapping to /Volumes, missing command). Make stick tracks playable from the stick view, using the stick's file path. Tests.
C. Item 2b (investigate + plan, do not build): opening a stick written by Rekordbox itself: list its playlists and tracks, play tracks, pull playlists into fourfour. Facts: real Rekordbox sticks can have an EMPTY exportLibrary.db and keep everything in export.pdb (issue #41); our readers read OneLibrary and ANLZ but there is no export.pdb reader in the app; `pioneer_library::rekordbox::load_usb` rejects export.pdb-only sticks. A metadata-only copy of a real Rekordbox stick is at ~/dev/data/rbx-real-stick-2026-11-22 (read only; no audio; never commit its track names). A writer v2 plan (docs/plans/writer-v2-2026-11-22.md on branch writer-v2-plan, `git show writer-v2-plan:docs/plans/writer-v2-2026-11-22.md`) proposes vendoring rbxport's rbl-pdb (GPL) as the PDB reader, pending a GPL decision. Write a short plan: what the app needs (read export.pdb or OneLibrary, map to a stick model, play, import selected playlists into the library with cues/grid from ANLZ), which pieces overlap with the writer v2 plan, effort, and what can be built before the GPL decision.

VALIDATION: in app/: `npx vitest run`, `npx svelte-check` 0 errors, `npm run build`, Playwright smoke; `cargo test --workspace`; `cargo check -p pioneer-test-ui --features dev-bins --bins`.

MAY WRITE: app/ keyboard/focus handling and player wiring, stick view row play wiring only (another worker, stick-ui-r1, is redesigning the stick UI layout, status dot, inspector placement and the stick track list interactions: touch StickView/sticks store only for the minimal play wiring, keep the diff small and say exactly what you touched), pioneer-test-ui commands, pioneer-library stick reading if needed for play, tests, docs/plans/stick-read-2026-11-22.md (git add -f, docs/plans is gitignored). DO NOT TOUCH: USB writer format code, prototyping/, datasets, master.

RULES: no code comments; CARGO_TARGET_DIR=~/.cache/stick-play-target (never /tmp), delete at the end; nice 10; no Tauri window, audio output, USB writes or Mac access; no new dependencies; no push, no merge. Conventional commits ending with "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>".

REPORT: RESULT: head SHA, root causes for A and B, what changed, validation last lines, the 2b plan summary (5 lines). Or BLOCKED.
