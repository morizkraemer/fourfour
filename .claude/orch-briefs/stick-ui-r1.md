You are a UI designer-engineer for fourfour, a Tauri v2 + Svelte 5 + Rust DJ library app. Read AGENTS.md first (rules, Review with Merlin, lab-review), the global lab-review skill (~/.claude/skills/lab-review/SKILL.md), docs/ui/usb-sync-decisions.md and docs/ui/usb-sync-spec.md. Also look at branch ui-sync-flow (b510e70, earlier sync UI round 3 contract) for context; build on master's current UI, polish in place, no rewrite of what works. You run in your own git worktree; branch `stick-ui-r1` off master.

Merlin tested USB sync on his Mac today (2026-11-22). His screenshots: .claude/orch-briefs/img/69.png (sidebar USB DEVICES list, green dot), 70.png (Keep updated toggle), 71.png (stick panel header), 72.png (stick selected with the side panel open: header shown twice), 73.png (top bar "INTENSO +1 in sync" pill). His feedback verbatim (items for you: 1, 3, 4, 6, 7):

"1. they show up. the selected one has a green dot. that doesnt rteally make sense right? green means its active in some way i guess. but what does that mean? maybe yellow for syncing or pulsing or something idk what do your rec?
3. cant toggle the keep updated toggle?
4. layout of the sync usb info sidebar header thing is a bit weird. almost good tho icant quite put my finger on it - having the usb selected and sidebar open duplicates the sutff as well
6. currently no way of removeing or reordering or editing tracks on the stick itself at all (no context menu, no hover on the list itself etc
7. the in sync is a bit weird. dont need that when nothing is wrong i think or maybe just in the sidebar inspector thing for the stick. also i guess the stick should have the inspector panel not the ingformation int he siderbar, the sidebar is for dragging tracks around like a dual window setup, the inspector is for info"

DIRECTION (orchestrator rec, adjust if the code shows a better fit; say so):
- Item 1: the dot shows status, never selection (selection = the normal row highlight). No dot when idle and in sync; amber pulsing while syncing; amber steady when changes are waiting; red on error. Same colours everywhere the stick status appears.
- Item 3: find why the toggle does not toggle (bug) and fix it, with a test.
- Item 7: drop the top-bar "in sync" pill when everything is fine; show it only when something needs attention (syncing, changes waiting, error, won't fit). Stick info (capacity, sync state, Sync/Eject, status line) moves to the inspector panel, like a track's info does. The side panel becomes a plain dual-window track list for dragging (stick contents, playlists on it), no info header.
- Item 4: follows from 7: one header, no duplicate when the stick is selected and the side panel is open. Make the remaining stick header calm and aligned with the app's existing headers.
- Item 6: stick track lists get the same row interactions as library lists where they make sense: hover, context menu (Remove from stick / remove from this playlist on the stick, Show in library), drag to reorder inside a playlist on the stick. Edits change what the next sync writes (the stick is a sync target, the library stays the source); keep that clear in copy.

DESIGN PROCESS (required): study the app's existing visual language first (library rows, inspector, menus, Library check dialog). Use realistic mock data (believable artist/track names, not hashes like "4FB2B2C"). Take headless screenshots (Playwright with the smoke mock backend, 1440x900 and @2x) and critique your own shots at least 3 rounds before posting: alignment, duplicated info, heavy boxes, spacing, states. Never post shots you have not checked.

STATES TO SHOW: sidebar list (idle, syncing, changes waiting, error), the stick in the inspector (in sync, syncing with progress, changes waiting, won't fit), the side panel as a plain list, a stick track row hover + context menu, reorder drag, the top bar with and without a problem.

VALIDATION: in app/: `npx vitest run`, `npx svelte-check` (0 errors), `npm run build`, Playwright smoke suite; `cargo check -p pioneer-test-ui --features dev-bins --bins` if Rust changes. Add vitest for new logic (status mapping, edit-to-sync-plan).

REVIEW: post one lab Inbox card (id `stick-ui-r1`, status "open") with before/after shots and one question per item, then rebuild/restart the review preview so it shows at https://dev.tail87069a.ts.net:4220/lab.html (ports.tsv service `review`); confirm with `curl -s http://127.0.0.1:4220/__hub/cards | jq`.

MAY WRITE: app/ (stick/sync UI, stores, tests, smoke mocks), pioneer-test-ui commands only if item 3 or 6 needs a backend call (say so), lab/ card data. DO NOT TOUCH: prototyping/, datasets, master, the USB writer format code (another effort rebuilds it). Another worker fixes Tab/focus and play-from-stick in the player/keyboard code: stay out of keyboard focus handling and the player.

RULES: no code comments; plain hyphens only in copy; CARGO_TARGET_DIR=~/.cache/stick-ui-target (never /tmp), delete at the end; nice 10; no Tauri window, audio, USB writes or Mac access; no new dependencies; no push, no merge. Conventional commits ending with "Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>".

REPORT: RESULT: head SHA, what changed per item, the cause of the toggle bug, validation last lines, card id. Or BLOCKED: decision, 2-3 options, recommendation.
