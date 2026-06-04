<!--
  App.svelte — fourfour Browse screen.
  Composes the six interactive modules into the "main layout v3" window frame:
    [ sidebar 240 | body[ header / (table fill | detail 278) ] ]
    / player (full width)
    / status bar (full width)
-->
<script>
  // Side-effect import: loads tokens.css + base.css globally
  import '$ds';

  // Modules
  import Sidebar       from './modules/Sidebar.svelte';
  import GlobalHeader  from './modules/GlobalHeader.svelte';
  import TrackTable    from './modules/TrackTable.svelte';
  import Detail        from './modules/Detail.svelte';
  import Player        from './modules/Player.svelte';
  import AppStatusBar  from './modules/AppStatusBar.svelte';

  import { ui } from './stores/ui.svelte.ts';
</script>

<div class="ff-browse">
  <div class="ff-browse__main">
    <div class="ff-browse__slot ff-browse__slot--sidebar">
      <Sidebar />
    </div>
    <div class="ff-browse__body">
      <div class="ff-browse__slot ff-browse__slot--header">
        <GlobalHeader />
      </div>
      <div class="ff-browse__content">
        <div class="ff-browse__slot ff-browse__slot--table">
          <TrackTable />
        </div>
        {#if ui.detailPaneOpen}
          <div class="ff-browse__slot ff-browse__slot--detail">
            <Detail />
          </div>
        {/if}
      </div>
    </div>
  </div>
  <div class="ff-browse__slot ff-browse__slot--player">
    <Player />
  </div>
  <div class="ff-browse__slot ff-browse__slot--status">
    <AppStatusBar />
  </div>
</div>

<style>
  /* ── Browse screen layout ──────────────────────────────── */
  :global(body) {
    margin: 0;
    overflow: hidden;
    background: var(--ff-bg);
    color: var(--ff-text);
    font-family: var(--ff-font);
  }

  .ff-browse {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    background: var(--ff-bg);
  }

  /* main area: [ sidebar | body ], fills space above player + status */
  .ff-browse__main {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    align-items: stretch;
  }

  /* body column: global header on top, table+detail row below */
  .ff-browse__body {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .ff-browse__content {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    align-items: stretch;
  }

  /* bare slots — no spacing; modules own their own padding/width (rule 4) */
  .ff-browse__slot {
    display: flex;
    min-width: 0;
  }
  .ff-browse__slot--sidebar { flex: none; }
  .ff-browse__slot--header  { flex: none; }
  .ff-browse__slot--table   { flex: 1 1 0; }
  .ff-browse__slot--detail  { flex: none; }
  .ff-browse__slot--player,
  .ff-browse__slot--status  { flex: none; }
</style>
