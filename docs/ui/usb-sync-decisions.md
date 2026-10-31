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

- 2026-10-27 (Merlin): a track dropped on a stick goes into that stick's "All tracks" collection (no playlist picker). Engine impact for the brother: loose tracks on a stick need to be owned/synced like playlist tracks.

## Round 3 built (2026-10-31, branch `b10-sync-ui-r3`, from `ov-1028`)
Built on the recommendations for STATE "Waiting on Merlin" 1-3 (not overruled):
- Panel closed: a sidebar playlist drag shows a "Put on {stick}" strip at the top of the stick view; only the strip takes the drop, the rest of the view does not. Hovered, it reads "Put on {stick} · Keep updated".
- Copying a playlist row from one stick onto another (sidebar row or docked board) puts it on with Keep updated on ("Copy to {stick} · Keep updated").
- Stick view on the left: stick header that adapts to its width (full: one row with size, counts, bar, used and free, status, Sync, Eject; half: free space only; panel: two lines; narrow: one row and a short bar), then the playlists on the stick (Tracks, Size, a quiet "Synced" mark, note) and an "Only on this stick" group for playlists made elsewhere. Clicking a row opens the stick playlist: crumb, name, counts, Keep updated switch with the sync age, or "as of {date}" when off.
- Drop targets name the action in one line: the strip, the docked board, and a small label next to the pointer over a sidebar stick row.
- Kept from ui-sync-flow: the stick header (name, capacity, last sync, Sync, Eject). The rest of that branch is superseded.
- Needs engine: per-playlist Keep updated off (`stick_set_keep_updated`, `kept_as_is` on `get_stick`; until then the switch stays on and is disabled), `last_sync` in `read_usb_state`, importing or copying a stick-only playlist. Plug in = sync is still a Sync button.
