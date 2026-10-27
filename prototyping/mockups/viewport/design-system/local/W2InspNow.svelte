<script>
  import { DetailPane } from '../../../../viewport/design-system/svelte.js';
  import { T, ALL_CUES, CUE_PALETTE, BANDS_STRIP, FILE_PATH } from './w2-data.js';

  let { width = 340 } = $props();
  const noop = () => {};

  const [minutes, seconds] = T.time.split(':').map(Number);
  const durationMs = (minutes * 60 + seconds) * 1000;
  const markers = ALL_CUES.map((c) => ({
    key: c.key,
    position: c.timeMs / durationMs,
    color: c.color,
    label: c.slot === 0 ? '' : c.letter,
  }));
  const fields = [
    { key: 'album', label: 'Album', value: T.album, readOnly: true },
    { key: 'genre', label: 'Genre', value: T.genre, readOnly: true },
  ];
</script>

<div class="now" style="width:{width}px">
  <DetailPane
    mode="single"
    title={T.title}
    artist={T.artist}
    cover={T.cover}
    {fields}
    duration={T.time}
    bpm={125.5}
    musicalKey="8A"
    onBpmChange={noop}
    onKeyChange={noop}
    bands={BANDS_STRIP}
    {markers}
    cues={ALL_CUES}
    cuePalette={CUE_PALETTE}
    showCues
    filePath={FILE_PATH}
  />
</div>

<style>
  .now { flex: none; height: 100%; position: relative; overflow: hidden; background: var(--ff-surface); border-left: 1px solid var(--ff-border); }
  .now :global(.ff-inspector) { width: 100%; height: 100%; border-left: none; overflow-y: auto; }
</style>
