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
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

  // Modules
  import Sidebar        from './modules/Sidebar.svelte';
  import GlobalHeader   from './modules/GlobalHeader.svelte';
  import TrackTable     from './modules/TrackTable.svelte';
  import Detail         from './modules/Detail.svelte';
  import Player         from './modules/Player.svelte';
  import AppStatusBar   from './modules/AppStatusBar.svelte';
  import Settings       from './modules/Settings.svelte';
  import OrganizePanel  from './modules/OrganizePanel.svelte';
  import CommandPalette from './modules/CommandPalette.svelte';
  import StickSyncDialog from './modules/StickSyncDialog.svelte';
  import StickView from './modules/StickView.svelte';
  import StickDropTip from './modules/StickDropTip.svelte';
  import LibraryHealthDialog from './modules/LibraryHealthDialog.svelte';
  import ImportDuplicatesDialog from './modules/ImportDuplicatesDialog.svelte';
  import FileBlockedTip from './modules/FileBlockedTip.svelte';
  import { announcer } from './stores/announce.svelte.ts';
  import { initFileHealth } from './stores/file-health.svelte.ts';
  import { initImportActivity } from './stores/import-activity.svelte.ts';
  import { unlistenWhenReady } from './lib/lateUnlisten.ts';
  import { isEditableTarget } from './lib/editableTarget.ts';
  import { isKeyboardFocused, trackFocusModality } from '$ds/utils/focus.js';

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
  
  let browseMainEl = $state(null);
  let analyzeAlways = $state(false);

  $effect(() => {
    if (!library.showAnalysisPrompt) analyzeAlways = false;
  });


  let stickOnLeft = $derived(library.volumes.includes(ui.activeSidebarRow) ? ui.activeSidebarRow : null);
  let inspectorVisible = $derived(ui.detailPaneOpen && selectionCount() > 0 && !ui.organize.expanded);

  function yieldsToFocusedControl(e) {
    if (e.ctrlKey || e.metaKey || e.altKey) return false;
    if (e.key === 'Tab') return isKeyboardFocused(e.target);
    if (e.key === ' ') return isKeyboardFocused(e.target) && !e.target.closest('[role="listbox"], [data-wave-slider]');
    return false;
  }

  function onGlobalKeyDown(e) {
    if (e.key === 'Escape') clearFileDropHover();
    if (e.defaultPrevented) return;
    if (yieldsToFocusedControl(e)) return;
    shortcuts.dispatch(e, { typing: isEditableTarget(e.target) });
  }

  onMount(() => {
    initLibrary().then(() => {
      reconcileOrganizeTarget(library.playlists, library.volumes);
      void initFileHealth();
    });
    initOrganize();
    initImportActivity();
    void syncPlaybackAudioConfig();
    window.addEventListener('keydown', onGlobalKeyDown);
    const stopFocusModality = trackFocusModality();
    const teardownWindowChrome = installMacWindowChrome();

    let stopDragDrop = () => {};
    if (getIsTauri()) {
      try {
        const appWindow = getCurrentWebviewWindow();
        stopDragDrop = unlistenWhenReady(appWindow.onDragDropEvent((event) => {
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
        }), (err) => console.warn('File drag-drop unavailable:', err));
      } catch (err) {
        console.warn('File drag-drop unavailable:', err);
      }
    }

    return () => {
      window.removeEventListener('keydown', onGlobalKeyDown);
      stopFocusModality();
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
            <StickView volumePath={stickOnLeft} />
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
            <Detail />
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

<div class="ff-sr-only" role="status" aria-live="polite" aria-atomic="true" data-live-region>{#key announcer.nonce}{announcer.message}{/key}</div>

<Settings />
<CommandPalette />
<StickSyncDialog />
<StickDropTip />
<LibraryHealthDialog />
<ImportDuplicatesDialog />
<FileBlockedTip />

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
  .ff-sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    padding: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    border: 0;
  }

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
