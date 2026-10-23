# USB sync — design decisions

Running log of Merlin's calls on the USB sync flow. Concepts: round 1 mockups (A sidebar tree, B right panel, C playlist chips), 2026-10-15.

## Model (decided)
- Only playlists placed on a stick go on it. Plug in = sync, no button, no plan on every run.
- Ask only when it matters: files would be removed, it won't fit, tracks aren't analyzed.
- Never delete files fourfour didn't write.
- Each playlist on a stick either follows the library or stays as it is. The concept is right; the word "Frozen" is not — needs a better term.

## Kept from round 1
- The storage design (capacity bar, eject).
- Sticks as expandable nodes in the left sidebar, with their playlists underneath (A). The duplication with library playlists is acceptable because a stick can hold playlists the app doesn't have.
- The stick docked in the right side panel (B). The side panel is a board you drag onto that changes with what's needed: another app playlist, a stick overview, or one playlist on a stick.
- "Won't fit" as a stop.

## Rejected
- Playlist chips (C): doesn't scale past ~5 sticks and adds nothing the tree + panel can't show.
- The drag tooltip with cost and the Live/Frozen popover as built. Where this lives is open: the playlist header, or a separate modal.
- Large notification cards with dead space.
- A full-width storage header for a little information.

## Round 2 calls (2026-10-15)
- Wording: one switch, "Keep updated". Only playlists that follow the library show a "Synced" mark; the others show nothing (no second label).
- Either side can show anything: library, app playlist, stick, playlist on a stick. Left = browse, right = put. The drop target decides the action.
- Layouts must cover: stick open on the left with the side panel closed (full width — needs its own header design), stick left + panel open, and the other combinations.

## Open
- Header layout: what shows where, when (sidebar row vs panel header vs playlist header), compact.
- Reverse direction: playlists on a stick that aren't in the library (made elsewhere, or by Rekordbox) — show, import, keep.
- Removal threshold, remembering unplugged sticks (needs a stable stick id), backup stick copy vs mirror.
