# Mockup boards with 3 variations

Mockups for design rounds go in `prototyping/mockups/` (see `docs/ui/mockups.md`).
Run `cd prototyping && npm run mockups` (port 5230). Do not edit the app or `prototyping/viewport/`
design system for a mockup; build on it.

1. Page = folder: `mockups/viewport/artboards/<area>/`. Id = path, e.g. `track-row/option-a`.
2. Board "now": copy `main-window/current.artboard.js`; render the real DS components with
   `host(Component, props)` from `#design-system/index.js` and data from `mock-data.js`.
3. Options: `option-a`, `option-b`, `option-c` artboards, three genuinely different ideas,
   each shown inside the full surrounding layout (add props to `MainWindow.svelte` such as
   `columns` or a snippet slot rather than copying it).
4. Add `decisions.artboard.js` listing each question, recommendation and the option ids.
5. Meta: `{ title, layer: 'composite', group: '<Round name>', order }`.
6. Verify headless: load `http://127.0.0.1:5230/#<area>`, assert no console errors, screenshot.
   Kill the server with `fuser -k 5230/tcp`.
7. When Merlin picks, move the winner into the app's design system and delete the other option boards.
