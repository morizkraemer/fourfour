import WaveformDisplay from '$ds/waveform-display.js';
import '$ds/base.css';
import { bandsFromDetailBytes, bandsFromStickBytes } from './services/waveform-bands.ts';

/**
 * Dev-only page (`npm run dev`, then /zoom-detail.html?track=NAME&win=SECONDS&at=SECONDS): the expanded
 * waveform of one measured track at a fixed zoom, for the before/after shots and the frame-time runs of
 * `app/perf/zoom-detail-shots.mjs`. `/fx/NAME.pwv7` and `/fx/NAME.ffwd` come from `waveform_detail_dump`.
 */
async function bytes(url: string): Promise<Uint8Array> {
  return new Uint8Array(await (await fetch(url)).arrayBuffer());
}

async function main() {
  const params = new URLSearchParams(location.search);
  const name = params.get('track')!;
  const win = Number(params.get('win') ?? 5);
  const at = Number(params.get('at') ?? 15);
  const pwv7 = await bytes(`/fx/${name}.pwv7`);
  const detail = await bytes(`/fx/${name}.ffwd`);
  const durationMs = (pwv7.length / 3 / 150) * 1000;
  const bands = bandsFromStickBytes(pwv7);
  const bandsZoom = bandsFromDetailBytes(detail.buffer as ArrayBuffer);

  const host = document.getElementById('host')!;
  host.className = 'ff-waveform-view';
  host.style.cssText = 'width:1128px;height:220px;';
  const display = new WaveformDisplay(host, { mode: 'expanded' });
  const frac = (at * 1000) / durationMs;
  display.setData({ bands, bandsOverview: bands, bandsZoom, beats: [], cues: [], duration_ms: durationMs, bpm: 0 } as any, frac as any);
  display._applyZoom((durationMs / 1000) / win);
  display._setClock(frac);

  (window as any).__bench = (frames: number, speed: number) => {
    const canvas = host.querySelector('.waveform-display__zoom') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d')!;
    const times: number[] = [];
    const stepFrac = (speed * (1000 / 60)) / durationMs;
    let f = frac;
    for (let i = 0; i < frames; i++) {
      f += stepFrac;
      const t0 = performance.now();
      display._setClock(f);
      ctx.getImageData(0, 0, 1, 1);
      times.push(performance.now() - t0);
    }
    times.sort((a, b) => a - b);
    const pct = (p: number) => times[Math.min(times.length - 1, Math.floor((p / 100) * times.length))];
    return { p50: pct(50), p95: pct(95), max: times[times.length - 1], mean: times.reduce((a, b) => a + b, 0) / times.length };
  };
  document.body.dataset.ready = '1';
}

main();
