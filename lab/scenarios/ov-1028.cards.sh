#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

KEEP="K=Keep,C=Change (note)"

npm run -s hub:card -- --id ov1028-status-chip --title "Top-bar status chip" --url /lab.html \
  --question "Status chip in the top bar: keep as is?" \
  --media "idle - track count=/data/cards/ov1028-status-chip/media/1-idle-track-count.png" \
  --media "analysing=/data/cards/ov1028-status-chip/media/2-analysing.png" \
  --media "importing=/data/cards/ov1028-status-chip/media/3-importing.png" \
  --media "panel open while idle=/data/cards/ov1028-status-chip/media/4-panel-open-idle.png" \
  --options "$KEEP"

npm run -s hub:card -- --id ov1028-sidebar --title "Right sidebar under full-width top bar" --url /lab.html \
  --question "Right sidebar below the full-width top bar: keep?" \
  --media "closed=/data/cards/ov1028-sidebar/media/1-closed.png" \
  --media "open=/data/cards/ov1028-sidebar/media/2-open.png" \
  --media "open at max width=/data/cards/ov1028-sidebar/media/3-open-max-width.png" \
  --options "$KEEP"

npm run -s hub:card -- --id ov1028-skeleton --title "Skeleton rows on start" --url /lab.html \
  --question "Skeleton rows while the library loads, instead of the empty state: keep?" \
  --media "skeleton on start=/data/cards/ov1028-skeleton/media/1-skeleton-rows-on-start.png" \
  --media "loaded=/data/cards/ov1028-skeleton/media/2-loaded.png" \
  --options "$KEEP"

npm run -s hub:card -- --id ov1028-scroll-analysing --title "Scrolling while tracks analyse" --url /lab.html \
  --question "Scrolling a BPM-sorted list while tracks analyse: stable?" \
  --media "scroll + BPM sort + live analysis=/data/cards/ov1028-scroll-analysing/media/scroll-while-analysing.mp4" \
  --options "S=Stable,J=Jumpy (note)"
