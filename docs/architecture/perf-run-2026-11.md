# Perf run, November 2026

Dated sections from the "as fast as possible" workers, newest last. Q50 baseline: `perf-q50-2026-10-30.md`.

## R2 startup at 10k-50k (2026-10-31)

Branch `worktree-agent-afd8c66b477699041` from `ov-1028`. Linux `dev` box, headless Chromium, release `bench_startup`.

**Changed**

- Boot snapshot ("cache the last open list"): the first screen of All Tracks in the current sort (about 40 rows, display fields and row preview) is kept in `localStorage` (`fourfour.bootSnapshot.v1`), rewritten 1.5 s after the list changes. At boot it paints through the normal row pool while the real load runs; the table is `inert` until then. The snapshot is only used for the same library path and the same layout and sort; the real `load_state` answer replaces it completely, so edited, deleted or outside-changed tracks never outlive the load.
- `load_state` no longer carries row previews (8.5 MB to 3.2 MB at 10k, 43 to 16 MB at 50k). Rows on screen fetch theirs with `get_row_previews(trackIds)`, one batched call per task, once per track object.
- `load_state` starts from `main.ts` before the UI mounts instead of after two other IPC round trips.

**Backend** (`bench_startup run`, median of 3 warm / 1 cold after page-cache eviction, load 6-9)

| Tracks | `load_state` warm before / after | cold before / after | of which JSON before / after | payload before / after |
|---|---:|---:|---:|---:|
| 10k | 104 / 85 ms | 114 / 86 ms | 46 / 18 ms | 8.5 / 3.2 MB |
| 25k | 247 / 200 ms | 255 / 186 ms | 106 / 42 ms | 21.4 / 8.1 MB |
| 50k | 488 / 370 ms | 504 / 387 ms | 209 / 85 ms | 42.9 / 16.3 MB |

Process start (bench binary) 10-20 ms, DB open 1.5 ms warm / 8-12 ms cold. The rest of `load_state` is the list query and the sidecar label scan, run side by side (50k: 240 and 230 ms).

**Frontend** (`app/smoke/perf-startup.spec.ts`: the real `load_state` JSON served over HTTP and parsed in the page, the backend time above simulated; before = `ov-1028` app, after = this branch; median of 3; load about 9). "First launch" has no snapshot yet; "cold" is a fresh browser with the snapshot from a previous session; "warm" is a reload.

| Tracks | Case | First row before / after | Real rows before / after | Interactive before / after |
|---|---|---:|---:|---:|
| 10k | first launch | 827 / 712 ms | 827 / 712 ms | 885 / 879 ms |
| 10k | cold | 781 / 437 ms | 781 / 885 ms | 832 / 1,050 ms |
| 10k | warm | 554 / 264 ms | 554 / 616 ms | 616 / 678 ms |
| 25k | first launch | 1,102 / 926 ms | 1,102 / 926 ms | 1,187 / 1,276 ms |
| 25k | cold | 1,117 / 460 ms | 1,118 / 1,021 ms | 1,188 / 1,021 ms |
| 25k | warm | 884 / 248 ms | 884 / 728 ms | 942 / 766 ms |
| 50k | first launch | 1,852 / 1,660 ms | 1,852 / 1,660 ms | 1,909 / 1,758 ms |
| 50k | cold | 1,844 / 561 ms | 1,844 / 1,741 ms | 2,155 / 1,810 ms |
| 50k | warm | 1,556 / 320 ms | 1,556 / 1,288 ms | 1,665 / 1,338 ms |

Split at 50k warm, after: bundle eval to first IPC about 170 ms, `load_state` requested at 150-190 ms, answered at 770 ms, then mapping 330 ms (four accessor properties per track object put each track on V8's slow object path; a class would be 4x faster but would drop the per-row `$state` proxy), list set 35-90 ms, first real row at 1,290 ms.

**Read with care.** One quiet pair (load about 9); three more pairs ran at load 30-75 and swing 2-3x either way, so they are not in the table. The 10k cold/warm rows show the snapshot costs about 60-100 ms on the real rows (the page paints the snapshot first); the snapshot rows moved into the shared row pool after this pair to cut the swap, not re-measured quiet. Real-app IPC, WKWebView parse and window show are not measured here; r1's Mac script covers them.

**Not done / next**

- Map tracks without per-object accessors (about 300 ms at 50k), keeping row reactivity.
- List query without the preview blob column (about 15% of the query at 50k, tried, not kept).
- Sidecar label scan: 50k directory reads on every start (230 ms at 50k).
- Window shown on first frame: the window already opens on `#0a0a0a`, so no white flash; not changed.
- Google Fonts stylesheet in `index.html` can hold the first script until it loads; no effect measured here (fast network), could stall an offline Mac.
