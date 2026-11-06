import { writeFileSync } from 'node:fs';
import { test } from './app.ts';
import type { SmokeWindow } from './tauri-mock.ts';

const RUN = process.env.FF_PERF_SCROLLFILL === '1';
const OUT = process.env.FF_PERF_OUT;
const IPC_MS = Number(process.env.FF_PERF_IPC_MS ?? 10);
const FRAMES = 120;
const SPEEDS = [84, 280];

interface Part {
  meanPct: number;
  p95Pct: number;
  worstPct: number;
  framesAffected: number;
}

interface FillStats {
  speedPxPerFrame: number;
  frames: number;
  rowsMissing: Part;
  previewsEmpty: Part;
  coversEmpty: Part;
  settledAfterMs: number;
  scrollMs: number;
  coverRequests: number;
  previewRequests: number;
  coverSeries: number[];
}

test('scroll fill: rows, previews and covers during a scripted fast scroll', async ({ smoke: { page } }) => {
  test.skip(!RUN || !process.env.FF_PERF_TRACKS || !process.env.FF_PERF_ROW_BANDS || !process.env.FF_SOAK_ARTWORK, 'set FF_PERF_SCROLLFILL=1 FF_PERF_TRACKS=10000 FF_PERF_ROW_BANDS=1 FF_SOAK_ARTWORK=1');
  test.setTimeout(240_000);
  await page.evaluate((ms) => {
    const w = window as unknown as SmokeWindow;
    w.__smokeArtwork = true;
    w.__smokeIpcMs = ms;
  }, IPC_MS);
  await page.waitForTimeout(1500);

  const results: FillStats[] = [];
  let startTop = 60_000;
  for (const speed of SPEEDS) {
    const stats = await page.evaluate(
      ({ speed, frames, startTop }) =>
        new Promise<FillStats>((resolve) => {
          const el = document.querySelector('.ff-table__scroll') as HTMLElement;
          const header = (el.querySelector('.ff-colh') as HTMLElement | null)?.offsetHeight ?? 0;
          const ROW = 28;
          const missing: number[] = [];
          const noWave: number[] = [];
          const noCover: number[] = [];
          const scratch = document.createElement('canvas');
          const scratchCtx = scratch.getContext('2d', { willReadFrequently: true })!;

          const hasPreview = (row: Element) => {
            const canvas = row.querySelector('.ff-trow__cell--wave canvas') as HTMLCanvasElement | null;
            if (!canvas) return true;
            scratch.width = canvas.width;
            scratch.height = canvas.height;
            scratchCtx.clearRect(0, 0, canvas.width, canvas.height);
            scratchCtx.drawImage(canvas, 0, 0);
            const { data, width, height } = scratchCtx.getImageData(0, 0, canvas.width, canvas.height);
            let touched = 0;
            for (let y = 0; y < height; y++) {
              for (let x = 0; x < width; x++) {
                if (data[(y * width + x) * 4 + 3] > 0) {
                  touched++;
                  break;
                }
              }
            }
            return touched > 4 * (window.devicePixelRatio || 1);
          };

          const hasCover = (row: Element) => {
            const cover = row.querySelector('.ff-trow__cover') as HTMLElement | null;
            return !cover || !!cover.style.backgroundImage;
          };

          const sample = () => {
            const top = el.scrollTop;
            const first = Math.max(0, Math.floor((top - header) / ROW) + 1);
            const last = Math.floor((top + el.clientHeight - header) / ROW) - 1;
            const present = new Map<number, Element>();
            for (const row of el.querySelectorAll('[data-track-row]')) present.set(Number((row as HTMLElement).dataset.rowIndex), row);
            let wanted = 0;
            let gone = 0;
            let waveless = 0;
            let coverless = 0;
            for (let i = first; i <= last; i++) {
              wanted++;
              const row = present.get(i);
              if (!row) {
                gone++;
                continue;
              }
              if (!hasPreview(row)) waveless++;
              if (!hasCover(row)) coverless++;
            }
            missing.push((100 * gone) / wanted);
            noWave.push((100 * (gone + waveless)) / wanted);
            noCover.push((100 * (gone + coverless)) / wanted);
          };

          const part = (series: number[]) => {
            const body = series.slice(0, frames);
            const sorted = [...body].sort((a, b) => a - b);
            return {
              meanPct: Math.round((10 * body.reduce((a, b) => a + b, 0)) / body.length) / 10,
              p95Pct: Math.round(sorted[Math.floor(sorted.length * 0.95)]),
              worstPct: Math.round(sorted.at(-1) ?? 0),
              framesAffected: body.filter((v) => v > 0).length,
            };
          };

          const asked = (cmd: string) => (window as unknown as SmokeWindow).__smokeInvokes.filter((i) => i.cmd === cmd).length;
          const coversAtStart = asked('get_track_artwork');
          const previewsAtStart = asked('get_row_previews');
          const startedAt = performance.now();
          el.scrollTop = startTop;
          let frame = 0;
          const tick = () => {
            sample();
            el.scrollTop = startTop + speed * ++frame;
            if (frame < frames) {
              requestAnimationFrame(tick);
              return;
            }
            const stopped = performance.now();
            const scrollMs = Math.round(stopped - startedAt);
            const settle = () => {
              sample();
              if ((noWave.at(-1) === 0 && noCover.at(-1) === 0) || performance.now() - stopped > 4000) {
                resolve({
                  speedPxPerFrame: speed,
                  frames,
                  rowsMissing: part(missing),
                  previewsEmpty: part(noWave),
                  coversEmpty: part(noCover),
                  settledAfterMs: Math.round(performance.now() - stopped),
                  scrollMs,
                  coverRequests: asked('get_track_artwork') - coversAtStart,
                  previewRequests: asked('get_row_previews') - previewsAtStart,
                  coverSeries: noCover.filter((_, i) => i % 6 === 0).map(Math.round),
                });
                return;
              }
              requestAnimationFrame(settle);
            };
            requestAnimationFrame(settle);
          };
          requestAnimationFrame(tick);
        }),
      { speed, frames: FRAMES, startTop },
    );
    results.push(stats);
    startTop += 30_000;
    await page.waitForTimeout(800);
  }
  console.log(JSON.stringify(results, null, 1));
  if (OUT) writeFileSync(OUT, JSON.stringify(results, null, 2));
});
