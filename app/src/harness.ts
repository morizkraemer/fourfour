import { mount } from 'svelte';
import { PlayerCompact } from '$ds';
import '$ds/base.css';

async function main() {
  const data = await (await fetch('/wf.json')).json();
  let beats = data.beats;
  if (!beats || beats.length === 0) {
    beats = [];
    let t = 500, n = 0;
    while (t < data.durationMs) { beats.push({ time_ms: t, bar_position: (n % 4) + 1 }); t += 1850; n++; }
  }
  mount(PlayerCompact, {
    target: document.getElementById('h')!,
    props: {
      cover: '',
      title: 'Brust in to Flame',
      artist: 'Some Artist — Original Mix',
      playing: true,
      progress: 0.5,
      currentTime: '3:32',
      currentTimeMs: '.4',
      totalTime: '7:04',
      bpm: '126.00',
      key: '8A',
      colorData: data.colorData,
      previewBytes: null,
      beats,
      durationMs: data.durationMs,
      cues: [],
      trackKey: 1,
      followPlayhead: false,
    },
  });
}
main();
