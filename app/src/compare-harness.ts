import { mount, type ComponentProps } from 'svelte';
import { Waveform, WaveformView } from '$ds';
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

async function main() {
  const fixture = await (await fetch('/compare-fixture.json')).json();
  const root = document.getElementById('compare')!;
  root.style.cssText = 'width:1160px;padding:16px;box-sizing:border-box;';
  const title = document.createElement('div');
  title.textContent = fixture.name;
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
    section.append(caption, expanded, small);
    root.append(section);

    const analysis = side.analysis;
    const compact = colorBandsFromPayload(analysis.waveform_color);
    const detail = colorBandsFromPayload(analysis.waveform_color_detail) ?? compact;
    const overview = colorBandsFromPayload(analysis.waveform_overview) ?? compact;
    const rowBytes = decodeBandPreview(side.load_state_row ?? analysis.waveform_row_bands);
    const row = rowBandsFromBandBytes(rowBytes);

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
      props: { bands: row, width: 300, height: 36, variant: 'strip' } as unknown as ComponentProps<typeof Waveform>,
    });
  });
  document.body.dataset.ready = '1';
}

main();
