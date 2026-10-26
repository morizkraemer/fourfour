<script>
  import {
    SidebarSection, SidebarRow, FilterInput, Kbd, ColumnHeader, TrackRow,
    DetailPane, PlayerCompact, StatusBar, LevelMeter, Icon,
  } from '../../../viewport/design-system/svelte.js';
  import { MOCK_TRACKS, MOCK_SIDEBAR } from './mock-data.js';

  const COLUMNS = [
    { key: 'fav', label: 'FAV', width: 20 },
    { key: 'index', label: '#', width: 28 },
    { key: 'title', label: 'TITLE', width: 180 },
    { key: 'artist', label: 'ARTIST', width: 180 },
    { key: 'label', label: 'LABEL', width: 130 },
    { key: 'bpm', label: 'BPM', width: 52, align: 'right' },
    { key: 'key', label: 'KEY', width: 36, align: 'right' },
    { key: 'time', label: 'TIME', width: 48, align: 'right' },
  ];

  let { tracks = MOCK_TRACKS, sidebar = MOCK_SIDEBAR, activeRow = 'All Tracks', selectedIndex = 1 } = $props();
  let selected = $derived(tracks.find((t) => t.index === selectedIndex));
</script>

<div class="mw">
  <div class="mw__main">
    <div class="mw__sidebar">
      <div class="mw__chrome ff-titlebar-band ff-sidebar__chrome"></div>
      <div class="mw__top">
        <SidebarSection label="FAVORITES">
          {#each sidebar.favorites as row}
            <SidebarRow {...row} state={row.label === activeRow ? 'active' : 'rest'} />
          {/each}
        </SidebarSection>
        <SidebarSection label="LIBRARY">
          {#each sidebar.library as row}
            <SidebarRow {...row} state={row.label === activeRow ? 'active' : 'rest'} />
          {/each}
        </SidebarSection>
        <SidebarSection label="PLAYLISTS" showAdd>
          {#each sidebar.playlists as row}
            <SidebarRow {...row} state={row.label === activeRow ? 'active' : 'rest'} />
          {/each}
        </SidebarSection>
      </div>
      <div class="mw__bottom">
        <SidebarSection label="USB DEVICES">
          {#each sidebar.usb as row}
            <SidebarRow {...row} />
          {/each}
        </SidebarSection>
      </div>
    </div>
    <div class="mw__body">
      <div class="ff-gheader ff-titlebar-band ff-titlebar-band--ruled mw__header">
        <FilterInput placeholder="Search tracks, artists, labels…" />
        <Kbd key="⌘F" />
      </div>
      <div class="mw__content">
        <div class="mw__table">
          <ColumnHeader columns={COLUMNS} />
          {#each tracks as track}
            <TrackRow {track} columns={COLUMNS} state={track.index === selectedIndex ? 'selected' : 'rest'} />
          {/each}
        </div>
        <div class="mw__detail">
          {#if selected}
            <DetailPane
              mode="single"
              title={selected.title}
              artist={selected.artist}
              meta={{ ALBUM: '—', LABEL: selected.label, BPM: selected.bpm, KEY: selected.key, DURATION: selected.time, ADDED: '2 days ago' }}
            />
          {:else}
            <DetailPane mode="empty" />
          {/if}
        </div>
      </div>
    </div>
  </div>
  <div class="mw__player">
    <PlayerCompact title={selected?.title ?? 'No Track Loaded'} artist={selected?.artist ?? '—'} currentTime="1:12" totalTime={selected?.time ?? '0:00'} progress={0.18} bpm={selected?.bpm} key={selected?.key} />
  </div>
  <div class="mw__status">
    <StatusBar message="Ready" count="{tracks.length} tracks">
      {#snippet rightExtras()}
        <LevelMeter left={0} right={0} />
        <Icon name="settings" size={13} />
      {/snippet}
    </StatusBar>
  </div>
</div>

<style>
  .mw { display: flex; flex-direction: column; width: 1440px; height: 900px; overflow: hidden; background: var(--ff-bg); color: var(--ff-text); font-family: var(--ff-font); }
  .mw__main { flex: 1 1 0; min-height: 0; display: flex; }
  .mw__sidebar { flex: none; width: 240px; display: flex; flex-direction: column; background: var(--ff-bg); border-right: 1px solid var(--ff-border); }
  .mw__chrome { padding-left: var(--ff-traffic-light-x); }
  .mw__top { flex: 1 1 0; overflow-y: auto; }
  .mw__bottom { border-top: 1px solid var(--ff-border); }
  .mw__body { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; }
  .mw__header { display: flex; align-items: center; justify-content: center; gap: var(--ff-space-4); }
  .mw__content { flex: 1 1 0; min-height: 0; display: flex; }
  .mw__table { flex: 1 1 0; min-width: 0; overflow: auto; background: var(--ff-bg); }
  .mw__detail { flex: none; width: 278px; position: relative; }
  .mw__player, .mw__status { flex: none; }
  .mw__status { border-top: 1px solid var(--ff-border); }
</style>
