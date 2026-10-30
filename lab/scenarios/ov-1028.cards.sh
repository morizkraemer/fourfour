#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

C=.capture
KEEP="K=Keep,C=Change (note)"

npm run -s hub:card -- --id ov1028-status-chip --title "Top-bar status chip" \
  --question "Status chip in the top bar: keep as is?" \
  --media "idle - track count=$C/ov1028-status-chip/1-idle-track-count.png" \
  --media "analysing=$C/ov1028-status-chip/2-analysing.png" \
  --media "importing=$C/ov1028-status-chip/3-importing.png" \
  --media "panel open while idle=$C/ov1028-status-chip/4-panel-open-idle.png" \
  --options "$KEEP"

npm run -s hub:card -- --id ov1028-sidebar --title "Right sidebar under full-width top bar" \
  --question "Right sidebar below the full-width top bar: keep?" \
  --media "closed=$C/ov1028-sidebar/1-closed.png" \
  --media "open=$C/ov1028-sidebar/2-open.png" \
  --media "open at max width=$C/ov1028-sidebar/3-open-max-width.png" \
  --options "$KEEP"

npm run -s hub:card -- --id ov1028-skeleton --title "Skeleton rows on start" \
  --question "Skeleton rows while the library loads, instead of the empty state: keep?" \
  --media "skeleton on start=$C/ov1028-skeleton/1-skeleton-rows-on-start.png" \
  --media "loaded=$C/ov1028-skeleton/2-loaded.png" \
  --options "$KEEP"

npm run -s hub:card -- --id ov1028-scroll-analysing --title "Scrolling while tracks analyse" \
  --question "Scrolling a BPM-sorted list while tracks analyse: stable?" \
  --media "scroll + BPM sort + live analysis=$C/ov1028-scroll-analysing/scroll-while-analysing.mp4" \
  --options "S=Stable,J=Jumpy (note)"
