# Design round 1: main window, inspector, settings, organize drawer

Date: 2026-10-26. Branch `wm-mockups`. Answers feedback F15, F16, F21, F22, F23 with mockup boards only (no app code).
Run `cd prototyping && npm run mockups`, open `http://127.0.0.1:5230/#<page>` (workflow in [mockups.md](mockups.md)).

Each page has a "now" board (today), three option boards and a Decisions board with the recommendation and open questions.
Boards use the real design-system components; board-local sketches live in `prototyping/mockups/viewport/design-system/local/`.

| Page | Feedback | Boards | Options | Recommendation |
|---|---|---|---|---|
| `main-window` | F23, F16 | `current`, `current-open`, `option-a..c`, `decisions` | A one right dock with two tabs; B track strip below + board right; C inline row editor + board right | B (A's single dock toggle, C's omnibox as top bar options) |
| `inspector` | F22 | `current`, `option-a..c`, `decisions` | A live form panel; B tabbed panel with cues on the waveform; C wide editor strip | C if main-window is B, otherwise B |
| `settings` | F21 | `current`, `option-a..c`, `decisions` | A preferences sheet; B one searchable page; C settings where they matter | B (open as a sheet if it fits) |
| `organize-sidebar` | F15, F16 | `current`, `option-a..c`, `decisions` | A destination cards; B drop to start; C set builder | A (+ B's drop-anywhere fallback) |

F16 is resolved the same way in every option: no "x". Two labelled verbs, Widen and Hide (chevron); clearing the target is a menu item.

Constraints kept: left = browse, right = put; stick header; Synced / Keep updated; simple row previews; 3-band players; ⌘K palette; resizable sidebars.
Open questions are listed on each Decisions board.
