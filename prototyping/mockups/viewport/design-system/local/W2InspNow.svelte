<script>
  import { DetailPane } from '../../../../viewport/design-system/svelte.js';
  import { T, HOT, MEMORY, CUE_PALETTE, BANDS_STRIP, FILE_PATH } from './w2-data.js';

  let { width = 278 } = $props();
  const noop = () => {};
</script>

<div class="now" style="width:{width}px">
  <DetailPane
    mode="single"
    title={T.title}
    artist={T.artist}
    cover={T.cover}
    bands={BANDS_STRIP}
    meta={{ ALBUM: T.album, LABEL: T.label, BPM: '125.5', KEY: T.key, DURATION: T.time, ADDED: '2 days ago' }}
    bpm={125.5}
    musicalKey="8A"
    onBpmChange={noop}
    onKeyChange={noop}
    cues={[...HOT, ...MEMORY]}
    cuePalette={CUE_PALETTE}
    showCues
    canSetCues={false}
    canJumpToCues={false}
    filePath={FILE_PATH}
  />
</div>

<style>
  .now { flex: none; height: 100%; position: relative; overflow: hidden; background: var(--ff-surface); border-left: 1px solid var(--ff-border); }
  .now :global(.ff-detail-pane) { width: 100%; height: 100%; border-left: none; overflow-y: auto; }
</style>
