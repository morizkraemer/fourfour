<!--
  App.svelte — fourfour Browse screen.
  Composes the six interactive modules into the "main layout v3" window frame:
    [ sidebar 240[ chrome + nav ] | body[ header / (table fill | detail 340 | organize) ] ]
    / player (full width)
    / status bar (full width)
-->
<script>
  import { onMount } from 'svelte';
  // Side-effect import: loads tokens.css + base.css globally
  import '$ds';
  import { Dialog, Button, ResizeHandle } from '$ds';

  // Modules
  import Sidebar        from './modules/Sidebar.svelte';
  import GlobalHeader   from './modules/GlobalHeader.svelte';
  import TrackTable     from './modules/TrackTable.svelte';
  import Player         from './modules/Player.svelte';
  import AppStatusBar   from './modules/AppStatusBar.svelte';
  import OrganizePanel  from './modules/OrganizePanel.svelte';
  import LazyMount from './lib/LazyMount.svelte';
  import { appDialogs, inspector, stickView } from './lib/lazy-views.ts';
  import { firstScreenShown, releaseAfterFallback } from './lib/afterFirstScreen.ts';
  import { initFileHealth } from './stores/file-health.svelte.ts';
  import { initImportActivity } from './stores/import-activity.svelte.ts';
  import { unlistenWhenReady } from './lib/lateUnlisten.ts';
  import { isEditableTarget } from './lib/editableTarget.ts';

  import {
    ui,
    initOrganize,
    reconcileOrganizeTarget,
    setPanelWidth,
    commitPanelWidth,
    resetPanelWidth,
  } from './stores/ui.svelte.ts';
  import { PANEL_WIDTH } from './lib/panelWidth.ts';
  import { shortcuts } from './shortcuts/index.ts';
  import {
    initLibrary,
    library,
    confirmAnalysisPrompt,
    dismissAnalysisPrompt,
  } from './stores/library.svelte.ts';
  import { fileDrop, hoverFileDrop, clearFileDropHover, importDroppedPaths } from './stores/file-drop.svelte.ts';
  import { selectionCount } from './stores/selection.svelte.ts';
  import { getIsTauri } from './services/tauri.svelte.ts';
  import { syncPlaybackAudioConfig } from './services/playback.svelte.ts';
  import { installMacWindowChrome } from './services/window-chrome.ts';
  import { currentWebviewWindow } from './services/webview-window.ts';
  
  let browseMainEl = $state(null);
  let analyzeAlways = $state(false);

  $effect(() => {
    if (!library.showAnalysisPrompt) analyzeAlways = false;
  });


  let stickOnLeft = $derived(library.volumes.includes(ui.activeSidebarRow) ? ui.activeSidebarRow : null);
  let inspectorVisible = $derived(ui.detailPaneOpen && selectionCount() > 0 && !ui.organize.expanded);

  function onGlobalKeyDown(e) {
    if (e.key === 'Escape') clearFileDropHover();
    if (e.defaultPrevented) return;
    shortcuts.dispatch(e, { typing: isEditableTarget(e.target) });
  }

  onMount(() => {
    releaseAfterFallback();
    initLibrary().then(() => {
      reconcileOrganizeTarget(library.playlists, library.volumes);
      void initFileHealth();
      firstScreenShown();
    });
    initOrganize();
    initImportActivity();
    void syncPlaybackAudioConfig();
    window.addEventListener('keydown', onGlobalKeyDown);
    const teardownWindowChrome = installMacWindowChrome();

    let stopDragDrop = () => {};
    if (getIsTauri()) {
      stopDragDrop = unlistenWhenReady(
        currentWebviewWindow().then((appWindow) => appWindow.onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === 'enter' || p.type === 'over') {
            hoverFileDrop(p.position);
          } else if (p.type === 'drop') {
            const target = hoverFileDrop(p.position);
            clearFileDropHover();
            if (p.paths?.length) void importDroppedPaths(p.paths, target);
          } else {
            clearFileDropHover();
          }
        })),
        (err) => console.warn('File drag-drop unavailable:', err),
      );
    }

    return () => {
      window.removeEventListener('keydown', onGlobalKeyDown);
      stopDragDrop();
      teardownWindowChrome();
    };
  });
</script>

<div class="ff-browse">
  <div class="ff-browse__main" bind:this={browseMainEl}>
    <div class="ff-browse__slot ff-browse__slot--sidebar" data-focus-region="sidebar">
      <Sidebar />
    </div>
    <div class="ff-browse__body">
      <div class="ff-browse__slot ff-browse__slot--header" data-focus-region="search">
        <GlobalHeader />
      </div>
      <div class="ff-browse__content" class:ff-browse__content--file-drop={fileDrop.overView} data-file-drop-content>
        <div class="ff-browse__slot ff-browse__slot--table" data-focus-region="tracks">
          {#if stickOnLeft}
            <LazyMount view={stickView} props={{ volumePath: stickOnLeft }} />
          {:else}
            <TrackTable />
          {/if}
        </div>
        {#if inspectorVisible}
          <div class="ff-browse__slot ff-browse__slot--detail" style:width="{ui.detailWidth}px">
            <ResizeHandle
              edge="left"
              label="Resize inspector"
              value={ui.detailWidth}
              min={PANEL_WIDTH.inspector.min}
              max={PANEL_WIDTH.inspector.max}
              onResize={(px) => setPanelWidth('inspector', px)}
              onCommit={() => commitPanelWidth('inspector')}
              onReset={() => resetPanelWidth('inspector')}
            />
            <LazyMount view={inspector} />
          </div>
        {/if}
      <div
        class="ff-browse__organize-dock"
        class:ff-browse__organize-dock--inspector={inspectorVisible}
        data-focus-region="organize"
      >
        <OrganizePanel containerEl={browseMainEl} />
      </div>
      </div>
    </div>
  </div>
  <div class="ff-browse__slot ff-browse__slot--player" data-focus-region="player">
    <Player />
  </div>
  <div class="ff-browse__slot ff-browse__slot--status">
    <AppStatusBar />
  </div>
</div>

<LazyMount view={appDialogs} needed={ui.settingsOpen || ui.paletteOpen} />

<Dialog
  open={library.showAnalysisPrompt}
  title="Analyze imported tracks?"
  onClose={dismissAnalysisPrompt}
>
  {#snippet body()}
    <p class="ff-analyze-prompt__text">
      {library.pendingAnalyzeTrackIds.length === 1
        ? '1 track was added. Run BPM, key, and waveform analysis now?'
        : `${library.pendingAnalyzeTrackIds.length} tracks were added. Run BPM, key, and waveform analysis now?`}
    </p>
    <span
      class="ff-checkbox ff-analyze-prompt__always"
      class:ff-checkbox--on={analyzeAlways}
      role="checkbox"
      aria-checked={analyzeAlways}
      tabindex="0"
      data-analyze-always
      onclick={() => (analyzeAlways = !analyzeAlways)}
      onkeydown={(e) => {
        if (e.key === ' ') {
          e.preventDefault();
          analyzeAlways = !analyzeAlways;
        }
      }}
    >
      <span class="ff-checkbox__box">
        <svg class="ff-checkbox__icon" width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
          <polyline points="2,5.5 4.2,7.5 8,2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </span>
      <span class="ff-checkbox__label">Always analyze new tracks automatically</span>
    </span>
  {/snippet}
  {#snippet actions()}
    <Button label="Not now" variant="ghost" onclick={dismissAnalysisPrompt} />
    <Button label="Analyze" variant="primary" onclick={() => void confirmAnalysisPrompt(analyzeAlways)} />
  {/snippet}
</Dialog>


<style>
  .ff-analyze-prompt__text {
    margin: 0 0 var(--ff-space-5);
  }

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

  /* main area: [ sidebar | body[ header / content[ table | detail | organize ] ] ], fills space above player + status */
  .ff-browse__main {
    position: relative;
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
    position: relative;
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    align-items: stretch;
  }
  .ff-browse__content--file-drop::after {
    content: '';
    position: absolute;
    inset: 0;
    border: 1px solid var(--ff-accent);
    pointer-events: none;
    z-index: 5;
  }

  /* bare slots — no spacing; modules own their own padding/width (rule 4) */
  .ff-browse__slot {
    display: flex;
    min-width: 0;
  }
  .ff-browse__slot--sidebar { flex: none; }
  .ff-browse__slot--header  { flex: none; }
  .ff-browse__slot--table   { flex: 1 1 0; }
  .ff-browse__slot--detail  { flex: none; position: relative; }

  .ff-browse__organize-dock {
    display: flex;
    flex: none;
    min-width: 0;
  }
  .ff-browse__organize-dock--inspector {
    background: var(--ff-surface);
  }
  .ff-browse__slot--player,
  .ff-browse__slot--status  { flex: none; }
</style>
