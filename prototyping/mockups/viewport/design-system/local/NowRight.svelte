<script>
  import { DetailPane, Icon, SidebarRow, FilterInput, Button } from '../../../../viewport/design-system/svelte.js';
  import BoardList from './BoardList.svelte';
  import { SELECTED, DETAIL_BANDS, CUES, TRACKS } from './data.js';

  /** mode: 'inspector' (inspector + cover rail) | 'drawer-empty' | 'drawer-filled' */
  let { mode = 'inspector', drawerWidth = 420 } = $props();
  const t = SELECTED;
  const rail = [TRACKS[2], TRACKS[6], TRACKS[10], TRACKS[3]];
</script>

{#if mode === 'inspector'}
  <div class="nr nr--insp">
    <DetailPane
      mode="single"
      title={t.title}
      artist={t.artist}
      cover={t.cover}
      bands={DETAIL_BANDS}
      meta={{ ALBUM: t.album, LABEL: t.label, BPM: t.bpm, KEY: t.key, DURATION: t.time, ADDED: '2 days ago' }}
      cues={CUES.slice(0, 3).map((c) => ({ name: 'Hot ' + c.id, position: c.position, color: c.color }))}
      filePath="/Users/merlin/Music/Kiln/Afterimage EP/01 Afterimage (Extended Mix).flac"
    />
  </div>
  <div class="nr nr--rail">
    <Icon name="chevron-left" size={16} />
    {#each rail as r}<span class="nr__tile" style="background-image:url({r.rowCover})"></span>{/each}
  </div>
{:else}
  <div class="nr nr--drawer" style="width:{drawerWidth}px">
    <div class="nr__dh">
      <span class="nr__dt">{mode === 'drawer-filled' ? 'PEAK TIME' : 'ORGANIZE'}</span>
      <button class="nr__ib" aria-label="Collapse to rail"><Icon name="panel-right" size={15} /></button>
      <button class="nr__ib" aria-label="Close"><Icon name="x" size={15} /></button>
    </div>
    {#if mode === 'drawer-empty'}
      <div class="nr__void"></div>
      <div class="nr__picker">
        <button class="nr__new"><Icon name="plus" size={15} /> New playlist</button>
        <div class="nr__search"><Icon name="search" size={13} /> Search playlists…</div>
        <SidebarRow kind="playlist" label="Peak Time" count={148} />
        <SidebarRow kind="playlist" label="Warm Up" count={96} />
        <div class="nr__sec">USB DEVICES</div>
        <SidebarRow kind="usb" label="SANDISK_64" status="online" />
      </div>
    {:else}
      <BoardList dropAfter={3} />
    {/if}
  </div>
{/if}

<style>
  .nr { flex: none; height: 100%; }
  .nr--insp { width: 320px; position: relative; overflow: hidden; border-left: 1px solid var(--ff-border); }
  .nr--insp :global(.ff-detail-pane) { width: 100%; height: 100%; }
  .nr--rail { width: 46px; display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 10px 0; color: var(--ff-text-mid); background: var(--ff-bg); border-left: 1px solid var(--ff-border); }
  .nr__tile { width: 30px; height: 30px; border-radius: 6px; background-size: cover; border: 1px solid var(--ff-border-hi); }
  .nr--drawer { display: flex; flex-direction: column; background: var(--ff-surface); border-left: 1px solid var(--ff-border); font-family: var(--ff-font); overflow: hidden; }
  .nr__dh { flex: none; height: 40px; display: flex; align-items: center; gap: 6px; padding: 0 12px 0 14px; border-bottom: 1px solid var(--ff-border); }
  .nr__dt { flex: 1; font-size: 13px; font-weight: 500; letter-spacing: 0.04em; color: var(--ff-muted); }
  .nr__ib { border: none; background: none; color: var(--ff-text-mid); display: flex; padding: 5px; border-radius: 3px; }
  .nr__void { flex: 1 1 0; }
  .nr__picker { flex: none; padding: 8px 10px 40px; display: flex; flex-direction: column; gap: 4px; }
  .nr__new { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 8px; background: none; border: none; color: var(--ff-text); font-size: 13px; font-family: var(--ff-font); border-bottom: 1px solid var(--ff-border); margin-bottom: 6px; }
  .nr__search { display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 8px; border: 1px solid var(--ff-border-hi); border-radius: 4px; color: var(--ff-faint); font-size: 12.5px; margin-bottom: 4px; }
  .nr__sec { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); padding: 12px 8px 4px; border-top: 1px solid var(--ff-border); margin-top: 8px; }
</style>
