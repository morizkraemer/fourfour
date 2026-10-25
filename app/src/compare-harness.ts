import { mount, type ComponentProps } from 'svelte';
import { Waveform, WaveformView } from '$ds';
import WaveformDisplay from '$ds/waveform-display.js';
import '$ds/base.css';
import { decodeBandPreview } from './services/analysis-data.ts';
import { colorBandsFromPayload, rowBandsFromBandBytes } from './services/waveform-bands.ts';

/**
 * Dev-only side-by-side page (`npm run dev`, then /compare.html): the app's waveform views for one
 * track, drawn twice by the same renderer. `/compare-fixture.json` is written by the
 * `waveform_fixtures` tool: `ours` and `rekordbox` each carry the `get_analysis_data` colour fields.
 */
interface Side {
  analysis: Record<string, any>;
  load_state_row?: string | null;
}

function asLegacy(color: any) {
  if (!color || typeof color !== 'object') return color;
  const bytes = Uint8Array.from(atob(color.bands), (c) => c.charCodeAt(0));
  let peak = 1;
  for (const b of bytes) if (b > peak) peak = b;
  return { ...color, pwv7: false, calibration: 127 / peak };
}

async function main() {
  const params = new URLSearchParams(location.search);
  const legacy = params.has('legacy');
  const fixture = await (await fetch('/compare-fixture.json')).json();
  const root = document.getElementById('compare')!;
  root.style.cssText = 'width:1160px;padding:16px;box-sizing:border-box;';
  const title = document.createElement('div');
  title.textContent = fixture.name + (legacy ? '  [legacy payload path: no pwv7 flag, track peak calibrated to full scale]' : '');
  title.style.cssText = 'margin-bottom:10px;color:#e8ebf0;font-size:13px;';
  root.append(title);

  const sides: Array<[string, Side]> = [
    ['OURS  (analysis measured by the shared module, bytes as the app receives them)', fixture.ours],
    ['REKORDBOX  (its own PWV7 / PWV6, same payload code, same renderer)', fixture.rekordbox],
  ];
  const trackKey = (n: number) => `${fixture.name}:${n}`;

  sides.forEach(([label, side], n) => {
    const section = document.createElement('div');
    section.style.cssText = 'margin-bottom:18px;';
    const caption = document.createElement('div');
    caption.textContent = label;
    caption.style.cssText = 'margin-bottom:6px;color:#8d939e;';
    const compactBox = document.createElement('div');
    compactBox.className = 'cmp-compact';
    compactBox.style.cssText = 'width:1128px;height:44px;margin-bottom:8px;';
    const expanded = document.createElement('div');
    expanded.className = 'cmp-expanded';
    expanded.style.cssText = 'width:1128px;height:220px;';
    const small = document.createElement('div');
    small.style.cssText = 'display:flex;gap:24px;align-items:center;margin-top:10px;';
    const rowBox = document.createElement('div');
    rowBox.className = 'cmp-row';
    const stripBox = document.createElement('div');
    stripBox.className = 'cmp-strip';
    small.append(rowBox, stripBox);
    section.append(caption, compactBox, expanded, small);
    root.append(section);

    const analysis = side.analysis;
    const pick = (color: any) => colorBandsFromPayload(legacy ? asLegacy(color) : color);
    const detail = pick(analysis.waveform_color_detail) ?? pick(analysis.waveform_color);
    const compact = detail;
    const overview = detail;
    const rowBytes = decodeBandPreview(side.load_state_row ?? analysis.waveform_row_bands);
    const row = rowBandsFromBandBytes(rowBytes);

    mount(WaveformView, {
      target: compactBox,
      props: {
        mode: 'compact',
        bands: compact,
        beats: fixture.beats,
        cues: [],
        durationMs: fixture.duration_ms,
        trackKey: trackKey(n) + ':c',
        progress: 0.37,
        playing: false,
      } as unknown as ComponentProps<typeof WaveformView>,
    });
    mount(WaveformView, {
      target: expanded,
      props: {
        mode: 'expanded',
        bands: compact,
        bandsDetail: detail,
        bandsOverview: overview,
        beats: fixture.beats,
        cues: [],
        bpm: 0,
        durationMs: fixture.duration_ms,
        trackKey: trackKey(n),
        progress: 0.37,
        playing: false,
      } as unknown as ComponentProps<typeof WaveformView>,
    });
    mount(Waveform, {
      target: rowBox,
      props: { bands: row, width: 120, height: 14, variant: 'mini' } as unknown as ComponentProps<typeof Waveform>,
    });
    mount(Waveform, {
      target: stripBox,
      props: { bands: row, width: 300, height: 36, variant: 'strip', look: 'bands' } as unknown as ComponentProps<typeof Waveform>,
    });
  });
  if (params.has('play')) {
    const host = document.createElement('div');
    host.className = 'cmp-play ff-waveform-view';
    host.style.cssText = 'width:1128px;height:220px;';
    root.append(host);
    const side = fixture.ours.analysis;
    const bands = colorBandsFromPayload(side.waveform_color_detail);
    const display = new WaveformDisplay(host, { mode: 'expanded' });
    display.setData(
      { bands, bandsOverview: bands, beats: fixture.beats, cues: [], duration_ms: fixture.duration_ms, bpm: 0 } as any,
      0.4 as any,
    );
    const startedAt = performance.now();
    const startFrac = 0.4;
    const tickMs = 50;
    setInterval(() => {
      const frac = startFrac + (performance.now() - startedAt) / fixture.duration_ms;
      display.setTransport(true, frac);
    }, tickMs);
    display.setTransport(true, startFrac);
  }
  document.body.dataset.ready = '1';
}

main();
