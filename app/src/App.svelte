<!--
  App.svelte — fourfour Browse screen.
  Composes the six interactive modules into the "main layout v3" window frame:
    [ sidebar 240[ chrome + nav ] | body[ header / (table fill | detail 278) ] ]
    / player (full width)
    / status bar (full width)
-->
<script>
  import { onMount } from 'svelte';
  // Side-effect import: loads tokens.css + base.css globally
  import '$ds';
  import { Dialog, Button, FileDropOverlay } from '$ds';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

  // Modules
  import Sidebar       from './modules/Sidebar.svelte';
  import GlobalHeader  from './modules/GlobalHeader.svelte';
  import TrackTable    from './modules/TrackTable.svelte';
  import ListPanel     from './modules/ListPanel.svelte';
  import Detail        from './modules/Detail.svelte';
  import Player        from './modules/Player.svelte';
  import AppStatusBar  from './modules/AppStatusBar.svelte';
  import Settings      from './modules/Settings.svelte';

  import {
    ui,
    closeSplitPanel,
    importDropTarget,
    openSettings,
    closeSettings,
    focusListFilter,
    clearListFilter,
    initSplitPaneRatio,
    resizeSplitPane,
    commitSplitPaneRatio,
  } from './stores/ui.svelte.ts';
  import { sidebarSourceLabel, playlistForSidebarSource } from './stores/library.svelte.ts';
  import { importIntoSource } from './menus/context-menus.ts';
  import {
    initLibrary,
    library,
    confirmAnalysisPrompt,
    dismissAnalysisPrompt,
    importFiles,
    toggleFavoriteSlot,
  } from './stores/library.svelte.ts';
  import { selection, selectionCount } from './stores/selection.svelte.ts';
  import { getIsTauri } from './services/tauri.svelte.ts';
  import { syncPlaybackAudioConfig } from './services/playback.svelte.ts';
  import { installMacWindowChrome } from './services/window-chrome.ts';
  import { player, togglePlay } from './stores/player.svelte.ts';

  let fileDropVisible = $state(false);
  let splitPanelsEl = $state(null);

  let dropTarget = $derived(importDropTarget());
  let dropTargetLabel = $derived(sidebarSourceLabel(dropTarget));
  let dropIntoPlaylist = $derived(playlistForSidebarSource(dropTarget) != null);
  let fileDropSub = $derived(
    dropIntoPlaylist
      ? `Files and folders will be added to “${dropTargetLabel}” and analyzed`
      : 'Files and folders will be added to your library and analyzed'
  );

  function isEditableTarget(target) {
    if (!(target instanceof Element)) return false;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return true;
    if (target.closest('[contenteditable="true"]')) return true;
    if (target.closest('.ff-settings-screen')) return true;
    return !!target.closest('.ff-dialog-wrapper');
  }

  function onGlobalKeyDown(e) {
    if (e.defaultPrevented) return;

    if (e.metaKey && e.key === ',') {
      e.preventDefault();
      if (ui.settingsOpen) closeSettings();
      else openSettings();
      return;
    }

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      focusListFilter();
      return;
    }

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'o') {
      e.preventDefault();
      if (isEditableTarget(e.target)) return;
      if (library.analyzing || library.syncing) return;
      void importIntoSource(importDropTarget());
      return;
    }

    if (e.key === 'Escape' && ui.settingsOpen) {
      closeSettings();
      e.preventDefault();
      return;
    }

    if (e.key === 'Escape' && library.showAnalysisPrompt) {
      return;
    }

    if (e.key === 'Escape' && ui.listFilter.trim()) {
      clearListFilter();
      e.preventDefault();
      return;
    }

    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (isEditableTarget(e.target)) return;

    if (e.key === ' ' || e.code === 'Space') {
      if (!player.track) return;
      e.preventDefault();
      void togglePlay();
      return;
    }

    const slot = /^[1-9]$/.test(e.key) ? parseInt(e.key, 10) : null;
    if (slot == null || selectionCount() === 0) return;
    e.preventDefault();
    toggleFavoriteSlot(slot, [...selection.ids]);
  }

  function startSplitResize(e) {
    if (e.button !== 0 || !splitPanelsEl) return;
    e.preventDefault();
    e.stopPropagation();
    const divider = e.currentTarget;
    const rect = splitPanelsEl.getBoundingClientRect();
    divider.setPointerCapture?.(e.pointerId);
    document.body.classList.add('ff-split--resizing');

    function onMove(ev) {
      resizeSplitPane(ev.clientX, rect.left, rect.width);
    }

    function onUp(ev) {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      try {
        divider.releasePointerCapture?.(ev.pointerId);
      } catch {
        /* already released */
      }
      document.body.classList.remove('ff-split--resizing');
      commitSplitPaneRatio();
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  onMount(() => {
    initLibrary();
    initSplitPaneRatio();
    void syncPlaybackAudioConfig();
    window.addEventListener('keydown', onGlobalKeyDown);
    const teardownWindowChrome = installMacWindowChrome();

    if (getIsTauri()) {
      try {
        const appWindow = getCurrentWebviewWindow();
        appWindow.onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === 'hover' || p.type === 'over') {
            fileDropVisible = true;
          } else if (p.type === 'drop') {
            fileDropVisible = false;
            const paths = p.paths;
            if (paths?.length) {
              void importFiles(paths, {
                targetSource: importDropTarget(),
              });
            }
          } else if (p.type === 'cancel' || p.type === 'leave') {
            fileDropVisible = false;
          }
        });
      } catch (err) {
        console.warn('File drag-drop unavailable:', err);
      }
    }

    return () => {
      window.removeEventListener('keydown', onGlobalKeyDown);
      teardownWindowChrome();
    };
  });
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
      <div class="ff-browse__content" class:ff-browse__content--split={ui.splitPanel != null}>
        {#if ui.splitPanel}
          <div
            class="ff-browse__panels"
            bind:this={splitPanelsEl}
            style:--split-ratio={ui.splitPaneRatio}
          >
            <div class="ff-browse__split-pane">
              <ListPanel
                sourceId={ui.activeSidebarRow}
                title={sidebarSourceLabel(ui.activeSidebarRow)}
              />
            </div>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="ff-browse__split-divider"
              role="separator"
              aria-orientation="vertical"
              aria-label="Resize side-by-side panels"
              onpointerdown={startSplitResize}
            ></div>
            <div class="ff-browse__split-pane ff-browse__split-pane--fill">
              <ListPanel
                sourceId={ui.splitPanel.sourceId}
                title={ui.splitPanel.label}
                splitCompanion
                showClose
                onClose={closeSplitPanel}
              />
            </div>
          </div>
        {:else}
          <div class="ff-browse__slot ff-browse__slot--table">
            <TrackTable />
          </div>
          {#if ui.detailPaneOpen && selectionCount() > 0}
            <div class="ff-browse__slot ff-browse__slot--detail">
              <Detail />
            </div>
          {/if}
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

<Settings />

<Dialog
  open={library.showAnalysisPrompt}
  title="Analyze imported tracks?"
  bodyText={library.pendingAnalyzeTrackIds.length === 1
    ? '1 track was added. Run BPM, key, and waveform analysis now?'
    : `${library.pendingAnalyzeTrackIds.length} tracks were added. Run BPM, key, and waveform analysis now?`}
  onClose={dismissAnalysisPrompt}
>
  {#snippet actions()}
    <Button label="Not now" variant="ghost" onclick={dismissAnalysisPrompt} />
    <Button label="Analyze" variant="primary" onclick={() => void confirmAnalysisPrompt()} />
  {/snippet}
</Dialog>

<FileDropOverlay
  visible={fileDropVisible}
  title="Drop audio files here"
  sub={fileDropSub}
/>

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

  /* main area: [ sidebar | body[ header / content ] ], fills space above player + status */
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

  .ff-browse__panels {
    flex: 1 1 0;
    min-width: 0;
    min-height: 0;
    display: flex;
    align-items: stretch;
  }

  .ff-browse__split-pane {
    flex: 0 0 calc((100% - 5px) * var(--split-ratio, 0.5));
    min-width: 0;
    min-height: 0;
    display: flex;
  }

  .ff-browse__split-pane--fill {
    flex: 1 1 0;
  }

  .ff-browse__split-divider {
    position: relative;
    flex: 0 0 5px;
    width: 5px;
    cursor: col-resize;
    touch-action: none;
    z-index: 3;
    /* Match panel header (40px surface) then table body (bg) — avoids a dark strip in the header band */
    background: linear-gradient(
      to bottom,
      var(--ff-surface) 0,
      var(--ff-surface) 40px,
      var(--ff-bg) 40px,
      var(--ff-bg) 100%
    );
  }

  .ff-browse__split-divider::after {
    content: '';
    position: absolute;
    left: 2px;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--ff-border-hi);
    opacity: 0.7;
    transition: opacity 0.12s ease, background 0.12s ease;
  }

  .ff-browse__split-divider:hover::after {
    opacity: 1;
    background: var(--ff-border-hover);
  }

  :global(body.ff-split--resizing) .ff-browse__split-divider::after {
    opacity: 1;
    background: var(--ff-accent);
  }

  :global(body.ff-split--resizing) {
    cursor: col-resize !important;
    user-select: none;
  }

  :global(body.ff-split--resizing) * {
    cursor: col-resize !important;
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
