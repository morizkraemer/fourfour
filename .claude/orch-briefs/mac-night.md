You are a worker on fourfour, a Tauri v2 + Svelte 5 + Rust DJ library app. Read AGENTS.md first (rules, Mac access, mac-queue, run limits), scripts/mac/ (night3.sh, perf-night.sh, build-dmg.sh, check-dmg.sh) and docs/architecture/perf-run-2026-11.md. You run in your own git worktree on a drain branch. Do NOT run anything on the Mac: you only prepare.

TASK: one headless Mac night-run script, scripts/mac/night-2026-11-25.sh, plus a summary script, for a later night when Merlin is not using the Mac. It builds a given branch (default: the integration branch `rc` name passed as arg) in the build clone and measures/checks, each step logged with timings and a final summary file:
1. Startup speed (first rows) at 10k and 50k with the existing perf harness; compare to master.
2. Analysis speed on Merlin's 512-track test folder (path from the existing night scripts/docs; read-only) with the shipping settings; tracks/min, wall time, power state (pmset -g batt) noted. This is the number for the blog: run it twice.
3. DMG size and check-dmg.sh on the built dmg.
4. Q24 data migration: run the migration self-test on a COPY of the old app data folder made inside a temp dir under the build clone (never touch the real ~/Library/Application Support folders, never touch anything Pioneer/rekordbox).
5. Cold import bytes read (F_RDAHEAD) on a 10k folder: fs_usage or the existing harness counters, compare to master.
6. CoreAudio double open at startup: log-based check if headless-possible, else mark "needs Merlin".
Every step: taskpolicy background priority unless measuring speed (then note it), timeouts, continue on failure, pipefail. Nothing writes outside the build clone and a results dir. No USB, no Rekordbox, no app windows.
Also write docs/architecture/night-2026-11-25.md: how to start it (mac-queue command), runtime estimate, what Merlin must check by hand afterwards (preview swap look, playlist XML import into Rekordbox/Traktor: by Merlin only).
VALIDATION: shellcheck if available; `bash -n`; dry-run mode (--dry-run prints the plan) tested on dev.
MAY WRITE: scripts/mac/night-2026-11-25.sh, its summary script, the doc. DO NOT TOUCH: anything else, master. No push.
RULES: no code comments in code (shell scripts too, except a usage line printed by --help); plain hyphens only. Conventional commits ending with "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>".
REPORT: RESULT: head SHA, the exact mac-queue command, runtime estimate, steps that need Merlin.
