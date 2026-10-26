import WaveformDisplay from '$ds/waveform-display.js';
import '$ds/tokens.css';
import '$ds/base.css';
import { bandsFromStickBytes } from './services/waveform-bands.ts';

/**
 * Dev-only page for the Rekordbox match (`/rbmatch.html`, driven by `perf/rbmatch.mjs`): draws one
 * expanded WaveformDisplay from raw PWV7 (and PWV6) bytes at an exact pixels-per-second and playhead
 * time, so a render can be compared pixel for pixel with a Rekordbox screenshot.
 */
interface ShowOptions {
  pwv7: number[];
  pwv6?: number[];
  durationMs: number;
  leftSec: number;
  pxPerSec: number;
  width: number;
  overviewHeight: number;
  zoomHeight: number;
  overviewFromPwv6?: boolean;
}

function show(o: ShowOptions) {
  const root = document.getElementById('stage')!;
  root.innerHTML = '';
  root.style.cssText = `width:${o.width}px;height:${o.overviewHeight + o.zoomHeight}px;position:relative;background:#000`;
  const host = document.createElement('div');
  host.style.cssText = 'width:100%;height:100%;';
  host.className = 'ff-waveform-view';
  root.append(host);
  const bands = bandsFromStickBytes(Uint8Array.from(o.pwv7));
  const overview = o.overviewFromPwv6 && o.pwv6 ? { ...bandsFromStickBytes(Uint8Array.from(o.pwv6)), overview: true } : bands;
  const display = new WaveformDisplay(host, { mode: 'expanded' });
  const style = document.createElement('style');
  style.textContent = `.ff-waveform-view .waveform-display{display:flex;flex-direction:column;width:100%;height:100%;overflow:hidden}
.ff-waveform-view .waveform-display__overview{display:block;width:100%;height:${o.overviewHeight}px;flex-shrink:0}
.ff-waveform-view .waveform-display__zoom{display:block;width:100%;height:100%}`;
  document.head.append(style);
  display.setData({ bands, bandsOverview: overview, beats: [], cues: [], duration_ms: o.durationMs, bpm: 0 } as any, 0 as any);
  const zoom = (o.pxPerSec * (o.durationMs / 1000)) / o.width;
  display.setViewport(zoom, o.leftSec / (o.durationMs / 1000));
  display.redraw();
  document.body.dataset.ready = String(Date.now());
}

(window as any).rbShow = show;
document.body.dataset.ready = '0';
