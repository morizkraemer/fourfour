You are a worker on fourfour, a Tauri v2 + Svelte 5 + Rust DJ library app. Read AGENTS.md first (rules, "## Unattended", run limits). You run in your own git worktree on a drain branch.

TASK: build one integration branch with the finished, unlanded work so it can be tested on the Mac in one build. Nothing lands on master.
Merge into your branch (off master), in this order, resolving conflicts without changing behaviour:
1. perf-detail (supersedes drain/hc-L3-20261120; do NOT merge hc-L3)
2. hc-release-profile
3. perf-startup
4. perf-scan (already contains hc-fuzz)
5. drain/Q24-20261121 (bundle id app.ffdj.library + data migration)
6. review-q60-62 (playlist export, reconnect, duplicates), then ui-dupes-r5 (duplicates UI round 5 on top of it)
7. xplat (cross-platform fixes + CI)
If a merge needs a real product decision, skip that branch, note it, continue with the rest.

VALIDATION after all merges: `cargo test --workspace`; `cargo check -p pioneer-test-ui --features dev-bins --bins`; in app/: `npm ci`, `npx vitest run`, `npx svelte-check` (0 errors), `npm run build`, Playwright smoke suite; `scripts/perf.sh` if it runs on Linux (report numbers vs scripts/perf-budgets.json). Use `set -o pipefail` for every check piped through tee/tail.

MAY WRITE: merge results on your branch, minimal conflict fixes, test fixes caused by the merge (say each). DO NOT TOUCH: master, prototyping/, datasets. No push.
RULES: no code comments; CARGO_TARGET_DIR=~/.cache/rc-target (never /tmp), delete at the end; nice 10; no Tauri window, audio, USB writes or Mac access; no new dependencies. Conventional commits ending with "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>".
REPORT: RESULT: head SHA, merged branches + conflicts and how resolved, skipped branches and why, validation commands + last lines, perf numbers. Or BLOCKED.
