---
name: labkit
description: Hand visual or hands-on results to Merlin as review cards in this project's lab Inbox (labkit), and act on his picks, notes and locks. Use when a change needs Merlin's eyes or ears, when preparing options for him to pick, at the start of every orchestrator turn (read notes), and when he says "keep this".
---

# labkit

Merlin reviews this project on his phone, in the lab Inbox. Every result he must look at, play or decide becomes ONE card there. He picks an option with one tap, writes a note, or marks it Done; you read that with `hub:notes` and act.

- Lab: `https://dev.tail87069a.ts.net:4220/lab.html` (Tailscale HTTPS, works on the phone). Never send localhost links.
- Data: `lab/data/cards/<id>.json`, media in `lab/data/cards/<id>/media/`, locks in `lab/data/locked/`.
- Scripts (run in `lab`): `npm run hub:card`, `npm run hub:notes`, `npm run hub:lock`.

## A card

```bash
npm run hub:card -- --id bar-height --title "Bar height" \
  --question "Which bar height? Rec: B, two rows fit at 390px." \
  --url "/app?exp=bar-height:b" \
  --media a=/hub/cards/bar-height/media/a.png --media b=/hub/cards/bar-height/media/b.png \
  --options a=Short,b=Tall \
  --check "Both rows readable" --check "Tap targets at least 44px" \
  --status backlog
```

- One card per decision. `--question` is one plain question with your recommendation.
- `--check`: 2 to 4 short bullets, what to look at or try.
- `--url` is the "play it" link on the review server. The card must be answerable without opening it.
- `--status backlog` from workers; the orchestrator ranks cards into the Inbox (at most ~7 open).
- Re-review: the SAME `--id` with `--round next` and only the new media/options. A new round replaces the old playlist and media. Never a new card per round, never the round in the title.

## Media rules

- Image-first: Merlin judges from the media, never by "go open the app".
- One tight crop or short clip PER OPTION (`--media <option>=<path>` matching `--options`). Never one wide compare grid: unreadable at 390px.
- Crop to the thing being decided; render at device scale 2+ so fullscreen stays sharp; add a scale reference when size matters.
- Motion (animation, transition, menu, flow) as a short looping MP4/WebM or small GIF, never described in words.
- Before / after = two items (`old=`, `new=`).
- A card whose media cannot show the difference is not ready.

Capture headless on your own port: crop with an element screenshot or clip rect; record motion and convert with `ffmpeg` (H.264 MP4 with `-movflags +faststart`, a few seconds, a few MB at most).

## Experiments

New ideas go behind a switch (`?exp=<name>`), off by default, on the review branch. No held branches, no extra ports. Prove default behaviour is unchanged with every switch off. The card's `--url` turns the switch on; `--check` says what it tests, what to try, what working feels like.

## Orchestrator: every turn

1. `npm run hub:notes` first (prints unread notes, picks, pins, Done; marks them read; `--peek` to only look).
2. Turn every note into something: a pick or change → work, next round on the same card; "keep this" / final pick / a rule → `hub:lock`; a question → answer on the card; nothing left → status `done`.
3. Keep at most ~7 cards open, ranked by what unblocks most.
4. Never judge visuals yourself; workers verify headless, Merlin judges.

## Locks

```bash
npm run hub:lock -- --kind decision --title "Two-row bar" --summary "B: tall bar, two rows" --source <note id> --link "/app?exp=bar-height:b"
```

Kinds: `mechanic`, `art`, `decision`, `ref`. Check the locks before changing anything visible. A locked thing changes only through a card that asks.

## Never

- Review items in chat ("Merlin should check X"). They are cards.
- localhost, 127.0.0.1 or LAN links for Merlin.
- Editing or deleting Merlin's notes, picks or Done marks.
- A card without media that answers its question.

Full rules: `lab/node_modules/labkit/docs/rules.md`; workflows: `lab/node_modules/labkit/docs/workflows.md`.
