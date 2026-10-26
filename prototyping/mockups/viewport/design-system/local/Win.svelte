<script>
  import {
    SidebarSection, SidebarRow, ColumnHeader, TrackRow, PlayerCompact, StatusBar, LevelMeter, Icon, FilterInput, Kbd,
  } from '../../../../viewport/design-system/svelte.js';
  import { TRACKS, SIDEBAR, PLAYER_BANDS, SELECTED, CUES } from './data.js';

  const WIDE = [
    { key: 'fav', label: '', width: 20 },
    { key: 'index', label: '#', width: 28 },
    { key: 'wave', label: 'PREVIEW', width: 96 },
    { key: 'cover', label: '', width: 22 },
    { key: 'title', label: 'TITLE', flex: true },
    { key: 'artist', label: 'ARTIST', width: 170 },
    { key: 'label', label: 'LABEL', width: 110 },
    { key: 'bpm', label: 'BPM', width: 52, align: 'right' },
    { key: 'key', label: 'KEY', width: 36, align: 'right' },
    { key: 'time', label: 'TIME', width: 44, align: 'right' },
  ];
  const MID = WIDE.filter((c) => c.key !== 'label');
  const NARROW = MID.filter((c) => c.key !== 'wave').map((c) => (c.key === 'artist' ? { ...c, width: 130 } : c));

  let {
    top, right, below, overlay, rowExtra, sidebarBottom,
    cols = 'wide', selectedId = 1, expandedId = null, activeRow = 'All Tracks',
    rows = TRACKS, count = '30,412 tracks', status = 'Ready', statusExtra,
    sidebarWidth = 240, player = true, tableOverlay, tableHeader = true,
    playerTitle = SELECTED.title, playerArtist = SELECTED.artist,
  } = $props();

  let columns = $derived(cols === 'wide' ? WIDE : cols === 'mid' ? MID : NARROW);
</script>

<div class="win">
  <div class="win__main">
    <div class="win__sidebar" style="width:{sidebarWidth}px">
      <div class="win__chrome ff-titlebar-band ff-sidebar__chrome">
        <span class="win__dot" style="background:#ff5f57"></span>
        <span class="win__dot" style="background:#febc2e"></span>
        <span class="win__dot" style="background:#28c840"></span>
      </div>
      <div class="win__top">
        <SidebarSection label="FAVORITES">
          {#each SIDEBAR.favorites as row}
            <SidebarRow {...row} state={row.label === activeRow ? 'active' : 'rest'} />
          {/each}
        </SidebarSection>
        <SidebarSection label="LIBRARY">
          {#each SIDEBAR.library as row}
            <SidebarRow {...row} state={row.label === activeRow ? 'active' : 'rest'} />
          {/each}
        </SidebarSection>
        <SidebarSection label="PLAYLISTS" showAdd>
          {#each SIDEBAR.playlists as row}
            <SidebarRow {...row} state={row.label === activeRow ? 'active' : 'rest'} />
          {/each}
        </SidebarSection>
      </div>
      <div class="win__bottom">
        {#if sidebarBottom}
          {@render sidebarBottom()}
        {:else}
          <SidebarSection label="USB DEVICES">
            {#each SIDEBAR.usb as row}
              <SidebarRow {...row} />
            {/each}
          </SidebarSection>
        {/if}
      </div>
    </div>

    <div class="win__body">
      {#if top}
        {@render top()}
      {:else}
        <div class="ff-gheader ff-titlebar-band ff-titlebar-band--ruled win__header">
          <FilterInput placeholder="Search tracks, artists, labels…" />
          <Kbd key="⌘F" />
        </div>
      {/if}
      <div class="win__content">
        <div class="win__center">
          <div class="win__table">
            {#if tableHeader}<ColumnHeader columns={columns} />{/if}
            <div class="win__rows">
              {#each rows as track (track.id)}
                <TrackRow {track} {columns} state={track.id === selectedId ? 'selected' : 'rest'} />
                {#if rowExtra && track.id === expandedId}
                  {@render rowExtra(track)}
                {/if}
              {/each}
            </div>
            <div class="win__scroll"><div class="win__thumb"></div></div>
            {#if tableOverlay}{@render tableOverlay()}{/if}
          </div>
          {#if below}{@render below()}{/if}
        </div>
        {#if right}{@render right()}{/if}
      </div>
    </div>
  </div>
  {#if player}
    <div class="win__player">
      <PlayerCompact
        title={playerTitle}
        artist={playerArtist}
        cover={SELECTED.rowCover}
        currentTime="3:41"
        totalTime="8:11"
        progress={0.45}
        bands={PLAYER_BANDS}
        cues={CUES.map((c) => ({ position: c.frac, color: c.color }))}
        bpm="125.00"
        key="8A"
      />
    </div>
  {/if}
  <div class="win__status">
    <StatusBar message={status} count={count}>
      {#snippet rightExtras()}
        {#if statusExtra}{@render statusExtra()}{/if}
        <LevelMeter left={0} right={0} />
        <Icon name="settings" size={13} />
      {/snippet}
    </StatusBar>
  </div>
  {#if overlay}
    <div class="win__overlay">{@render overlay()}</div>
  {/if}
</div>

<style>
  .win { position: relative; display: flex; flex-direction: column; width: 1440px; height: 900px; overflow: hidden; background: var(--ff-bg); color: var(--ff-text); font-family: var(--ff-font); border: 1px solid var(--ff-border-hi); border-radius: 10px; }
  .win__main { flex: 1 1 0; min-height: 0; display: flex; }
  .win__sidebar { flex: none; display: flex; flex-direction: column; background: var(--ff-bg); border-right: 1px solid var(--ff-border); }
  .win__chrome { padding-left: var(--ff-traffic-light-x); display: flex; align-items: center; gap: 8px; }
  .win__dot { width: 12px; height: 12px; border-radius: 50%; }
  .win__top { flex: 1 1 0; overflow: hidden; }
  .win__bottom { border-top: 1px solid var(--ff-border); }
  .win__body { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; }
  .win__header { display: flex; align-items: center; justify-content: center; gap: var(--ff-space-4); }
  .win__content { flex: 1 1 0; min-height: 0; display: flex; }
  .win__center { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; }
  .win__table { position: relative; flex: 1 1 0; min-height: 0; min-width: 0; overflow: hidden; background: var(--ff-bg); }
  .win__rows { overflow: hidden; }
  .win__scroll { position: absolute; right: 2px; top: 30px; bottom: 4px; width: 4px; }
  .win__thumb { height: 22px; margin-top: 0; border-radius: 2px; background: var(--ff-border-hi); }
  .win__player { flex: none; border-top: 1px solid var(--ff-border); }
  .win__status { flex: none; border-top: 1px solid var(--ff-border); }
  .win__overlay { position: absolute; inset: 0; z-index: 20; }
</style>
