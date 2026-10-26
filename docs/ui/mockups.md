# Mockups (viewport workflow)

Mockup boards for design rounds live in `prototyping/mockups/`, rendered by the
standalone `viewport` canvas (`~/dev/projects/viewport`). Same workflow as Merlin's
audio_tools/konsole UI work.

## How audio_tools does it

- `viewport` is a devDependency (`github:merlinkraemer/viewport`); `npm run viewport` starts it.
  Project layout: `viewport/artboards/<page>/<name>.artboard.js`, `viewport/design-system/`,
  `viewport.config.js`, skill at `.claude/skills/viewport/SKILL.md`.
- One page per focus area (top-level artboard folder); the artboard id is its path
  (`device-view/dv8-slots`). Merlin pastes ids as feedback; comments land in `viewport/comments.json`.
- A design round = one "now" board (current state) + 3 option boards + a Decisions board.
  Only the current round stays on the page; decided rounds are deleted (git keeps them).
- Picked pieces are "locked" by moving them into `design-system/`; later boards import them.
  Tokens live in one `tokens.css`. Picks are recorded in a doc (`docs/ui-picks`).
- Dev server on a fixed port, review build (`viewport build` + `viewport preview`) on a
  second port, both rows in lab-infra `ports.tsv`. Headless check script (`viewport:check`).

## fourfour setup

- Project: `prototyping/mockups/` (own `viewport.config.js`, `viewport.vite.js`, `viewport/`).
  It reuses `prototyping/node_modules`; no extra install.
- Real components: `mockups/viewport/design-system/index.js` re-exports the app's design system
  (`prototyping/viewport/design-system/svelte.js`, the same code the app imports as `$ds`),
  plus `host()` to mount Svelte, `mock-data.js`, and `MainWindow.svelte` (today's layout).
- Run: `cd prototyping && npm install && npm run mockups` then open
  `http://127.0.0.1:5230/#main-window` (ports.tsv row `fourfour mockups 5230`).
- The older `npm run dev` (port 5180) canvas stays as is; the app's `$ds` paths are unchanged.
- App modules (`app/src/modules/*`) are not imported: they need Tauri and live stores.
  Mock them in `MainWindow.svelte` from DS components instead.
