<script lang="ts">
  // Smoke test for the Path-A lift: import shared design-system primitives from
  // the single source ($ds) and render them inside this standalone Svelte app.
  // If this builds and paints, primitives lift cleanly — independent of the
  // viewport prototyping engine. The real Browse screen replaces this next.
  import {
    Button,
    ColumnHeader,
    TrackRow,
    TRACK_COLUMNS,
    SidebarSection,
    SidebarRow,
    DetailPane,
    PlayerCompact,
    StatusBar,
  } from '$ds';

  const columns = [
    { key: 'index', label: '#', width: 26 },
    { key: 'title', label: 'TITLE', flex: true },
    { key: 'artist', label: 'ARTIST', width: 180 },
    { key: 'bpm', label: 'BPM', width: 52, sort: 'desc' },
    { key: 'key', label: 'KEY', width: 48 },
    { key: 'time', label: 'TIME', width: 48 },
  ];

  const rows = [
    { state: 'rest', track: { index: '01', title: 'Hyperion', artist: 'Solomun', bpm: '124', key: '8A', time: '7:42' } },
    { state: 'selected', track: { index: '02', title: 'Voidwalker', artist: 'Tale Of Us', bpm: '122', key: '5A', time: '8:14' } },
    { state: 'rest', track: { index: '03', title: 'Drift', artist: 'Bicep', bpm: '128', key: '11B', time: '6:55' } },
  ];
</script>

<main>
  <h1>fourfour — design-system lift smoke test</h1>

  <section class="row">
    <Button label="Curate" variant="primary" />
    <Button label="Import" />
    <Button label="Filter" variant="ghost" icon="search" />
    <Button label="Delete" variant="destructive" />
  </section>

  <section class="panes">
    <div class="sidebar">
      <SidebarSection label="FAVORITES" showAdd>
        <SidebarRow kind="favorite" digit={1} label="Peak Time" count={24} />
        <SidebarRow kind="favorite" digit={2} label="Warmup" count={18} state="selected" />
      </SidebarSection>
      <SidebarSection label="USB DEVICES">
        <SidebarRow kind="usb" label="SanDisk 64GB" status="online" />
        <SidebarRow kind="usb" label="Tour Stick" status="offline" />
      </SidebarSection>
    </div>

    <div class="table">
      <ColumnHeader {columns} />
      {#each rows as r}
        <TrackRow track={r.track} state={r.state} {columns} />
      {/each}
    </div>

    <div class="detail">
      <DetailPane
        mode="single"
        title="Voidwalker"
        artist="Tale Of Us"
        meta={{ ALBUM: 'Endless Run', LABEL: 'Afterlife', BPM: '122', KEY: '5A', DURATION: '8:14', ADDED: '2 days ago' }}
        cues={[{ name: 'Intro', position: '0:32' }, { name: 'Drop', position: '2:48' }]} />
    </div>
  </section>

  <PlayerCompact
    title="Voidwalker"
    artist="Tale Of Us"
    currentTime="2:54"
    totalTime="8:14"
    progress={0.35}
    bpm="122.00"
    key="5A"
    cues={[{ position: 0.1, color: '#ec4899' }, { position: 0.55, color: '#38bdf8' }]} />

  <StatusBar message="Ready" context="3 tracks" count={3} />
</main>

<style>
  :global(body) {
    margin: 0;
    background: var(--ff-bg);
    color: var(--ff-text);
    font-family: var(--ff-font);
  }
  main {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-6);
    padding: var(--ff-space-7);
  }
  h1 {
    font-size: var(--ff-type-h2);
    color: var(--ff-text-mid);
    margin: 0;
  }
  .row {
    display: flex;
    gap: var(--ff-space-5);
    align-items: center;
  }
  .panes {
    display: flex;
    gap: var(--ff-space-6);
    align-items: flex-start;
  }
  .sidebar {
    width: 240px;
    background: var(--ff-bg);
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    padding: var(--ff-space-3) 0;
  }
  .table {
    flex: 1;
    background: var(--ff-surface);
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    overflow: hidden;
  }
  .detail {
    width: 278px;
    background: var(--ff-surface);
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
  }
</style>
