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

  let values = $state({ title: T.title, artist: T.artist, album: T.album, genre: T.genre });
  let fields = $derived([
    { key: 'album', label: 'Album', value: values.album },
    { key: 'genre', label: 'Genre', value: values.genre },
  ]);
</script>

<div class="a" style="width:{width}px">
  <DetailPane
    mode="single"
    title={values.title}
    artist={values.artist}
    cover={T.cover}
    {fields}
    onFieldChange={(key, value) => (values = { ...values, [key]: value })}
    duration={T.time}
    bpm={125.5}
    musicalKey="8A"
    bpmLocked
    keyLocked
    onBpmChange={noop}
    onKeyChange={noop}
    onReleaseLocks={noop}
    bands={BANDS_STRIP}
    {markers}
    onWaveformSeek={noop}
    onMarkerClick={noop}
    cues={ALL_CUES}
    cuePalette={CUE_PALETTE}
    showCues
    canSetCues
    canJumpToCues
    onCueAdd={noop}
    onCueJump={noop}
    onCueRename={noop}
    onCueRecolor={noop}
    onCueDelete={noop}
    onCueConvert={noop}
    filePath={FILE_PATH}
  />
</div>

<style>
  .a { flex: none; height: 100%; position: relative; overflow: hidden; background: var(--ff-surface); border-left: 1px solid var(--ff-border); }
  .a :global(.ff-inspector) { width: 100%; height: 100%; border-left: none; overflow-y: auto; }
</style>
