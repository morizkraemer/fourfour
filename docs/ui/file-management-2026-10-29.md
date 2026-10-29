# File management — decisions 2026-10-29

Source: Merlin's viewport review of the `files/*` and `topbar/*` boards (branch `fb-mockups`, archived boards under `prototyping/mockups/viewport/archive/files/`).

## Rule over everything

All new UI fits the existing app and is consistent across every popup, modal, dialog, icon and import flow: one dialog shell, one button set, one spacing scale (4 px unit, 4/8/16/24/32; controls 28 px), one icon set. Reuse existing components (the stick sync dialog, CommandPalette modal, FilterInput, Icon). New UI must not introduce a second look.

## Decided

| Area | Decision |
|---|---|
| Top bar | Option B: track count + library status button ("3 missing"), opening a popover with one action per line |
| Missing files, rows | Option A: calm status column (missing, relocated, error) |
| Relocate | Option A: search one folder and relink all matches; "Locate…" for a single file as fallback |
| Health check | Quiet check on every launch (file existence only), warning surfaces in the top-bar status button, results dialog on click; Settings → Library card with "Check now" |
| Duplicates on import | Approved 2026-10-29, no visible scrollbar (fade or overlay). Simplified (board `files/duplicates-simple`): one dialog before import, "N tracks look like ones you already have", compact list, "Skip duplicates" (default) / "Import anyway". No per-group choices, no Replace. Match = same audio fingerprint OR same artist + title + duration ±2 s |
| Backup | Option A: one Settings → Backup page: export library + analysis + settings as one compressed file, optional "include audio files" (keeps folder structure), restore with a confirm dialog |
| Import | Audio stays where it is (no copy); unchanged |

## Open

- Big player BPM/key edit: no separate strip. Must be simpler and live inside the existing header strip → round player-edit-5.
