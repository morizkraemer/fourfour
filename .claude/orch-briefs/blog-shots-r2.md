ROUND 2 for the blog screenshot pack (you made round 1 on this branch: drain/Q68-20261122, card blog-shots). Merlin's notes on the card, verbatim:
"not quite sure whats going on with the waveforms etc it looks weird. also you can just use actual songs from the demo folder"
"also crop stuff when not the full screen is of interest. also i hat ethe chart we just do text thats enough. and add the app icon as well"
And in chat: "use this as demo folder but also add more dummy playlists and folders etc (can be empty) to fill the sidebar".

DO:
1. Real songs: 75 MP3s copied read-only to ~/dev/data/blog-demo-2026-11-22 (Merlin's own DJ folder; real titles/artists may appear in public shots, he asked for it; do NOT commit the audio). Run the app's real Rust analysis on them headless on dev (the dev-bins / analysis path the app uses: BPM, grid, key, colour waveform overview + detail, cover art from tags) and feed the results into the browser mock so the library, player waveform, beat grid and inspector show real data. Commit only derived mock data needed for the shots if small (< 5 MB total); otherwise generate it into a gitignored cache and have the spec regenerate it. Use ~/.cache/blogshots-target as CARGO_TARGET_DIR (never /tmp), delete at the end; nice 10; at most 4 analysis processes.
2. Waveforms must look like the real app on a real track (round 1 looked weird: fake data). Hot cues/memory cues: add a few believable ones on 2-3 tracks.
3. Sidebar: add more playlists and folders (can be empty) with believable DJ names (e.g. "Warm-up", "Peak time", "Sunday Organic", folders "Gigs 2026", "Radio", "Ideas"), so the sidebar looks lived-in.
4. No chart. Drop shot 07; the numbers go in the text.
5. Crop: when only part of the screen matters (player, inspector, analysis popover, library check dialog), deliver a cropped version focused on it (with a little context), plus the full-window version for the library.
6. App icon: add a clean PNG of the app icon (pioneer-test-ui/icons/icon.png or the bundle icon, largest size) to the pack.
7. Critique your own shots at least 3 rounds (waveform look, alignment, truncated text, empty areas, debug UI) before posting.
DELIVER: replace the shots in docs/blog/shots/ (PNG @2x + 1440-wide JPEG), update the regenerate spec, and update card blog-shots to round 2 (status "open", new media, question "Round 2: which shots go in the post?"). Card files live untracked in the main checkout's lab/data/cards (the :4220 server reads them live). Confirm with `curl -s http://127.0.0.1:4220/__hub/cards | jq '.cards[] | select(.id=="blog-shots") | .round'`.
VALIDATION: vitest, svelte-check 0 errors, normal smoke green on a free port (check `ss -ltn`).
Rules unchanged: no code comments, no app behaviour changes, no Tauri window/audio/USB/Mac, no new dependencies, no push. Commits end with "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>".
REPORT: RESULT: head SHA, shot list (full/cropped), how waveforms were produced, card round.
