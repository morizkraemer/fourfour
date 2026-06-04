// Standalone harness: drives the vendored WaveformDisplay with real calibrated
// data (app/public/wf-display.json) using the same 0–1 → 0–255 rgb mapping the
// ExpandedPlayer wrapper applies. Verifies the renderer + mapping in a browser.
import WaveformDisplay from './lib/WaveformDisplay.js';

const status = document.getElementById('status')!;
const errors: string[] = [];
window.addEventListener('error', (e) => {
  errors.push(String(e.message));
  status.textContent = 'ERROR: ' + errors.join(' | ');
});

function toDisplay(samples: any[]) {
  return samples.map((s) => ({ amp: s.amp, r: s.r * 255, g: s.g * 255, b: s.b * 255 }));
}

async function main() {
  const data = await (await fetch('/wf-display.json')).json();
  const display = new WaveformDisplay(document.getElementById('wave')!);
  display.onSeek = (frac: number) => {
    (window as any).__lastSeek = frac;
    status.textContent = `seek -> ${frac.toFixed(4)}  (detail=${data.colorDetail.length}, overview=${data.colorOverview.length}, beats=${data.beats.length}, bpm=${data.bpm}, key=${data.key})`;
  };
  display.setData({
    waveform_color: toDisplay(data.colorDetail),
    waveform_overview: toDisplay(data.colorOverview),
    waveform_preview: data.previewBytes,
    beats: data.beats,
    duration_ms: data.durationMs,
    bpm: data.bpm,
  });
  display.setPlayhead(0.35);
  (window as any).__display = display;
  if (errors.length === 0) {
    status.textContent = `OK — rendered detail=${data.colorDetail.length} overview=${data.colorOverview.length} beats=${data.beats.length} dur=${data.durationMs}ms bpm=${data.bpm} key=${data.key}. Playhead@0.35.`;
  }
}
main().catch((e) => {
  status.textContent = 'MAIN ERROR: ' + e;
});
