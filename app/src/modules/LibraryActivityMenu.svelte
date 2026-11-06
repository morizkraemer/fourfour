<!--
  LibraryActivityMenu.svelte — Popover under the top-bar library activity button.
  Opens from the always-visible status chip. Lists the library summary, everything running (importing,
  syncing, analyzing, each with a progress bar), the last import (this session; after a folder import it
  offers to turn the folder into playlists), the missing-files actions while files are missing or
  unreadable, and the import entry points. Uses the shared menu surface (.ff-menu).
-->
<script>
  import { tick } from 'svelte';
  import { Button, ProgressBar } from '$ds';
  import { closeOnEscape } from '$ds/components/menuEscape.js';
  import { addTracksToPlaylist, isFavoritePlaylistName, library, unanalyzedTrackCount } from '../stores/library.svelte.ts';
  import { importActivity } from '../stores/import-activity.svelte.ts';
  import { fileStatus } from '../stores/file-status.svelte.ts';
  import { checkLibraryAgain, openHealthDialog, relocateMissing, showProblemTracks } from '../stores/file-health.svelte.ts';
  import { importDropTarget } from '../stores/ui.svelte.ts';
  import { importIntoSource, importFilesIntoSource } from '../menus/context-menus.ts';
  import { playlistsFromImportedFolder } from '../stores/file-drop.svelte.ts';
  import { analysisTimeLeft } from '../lib/libraryActivity.ts';

  let { open = false, x = 0, y = 0, hasProblems = false, onClose } = $props();

  let menuEl = $state(null);
  let importing = $derived(importActivity.running);
  let trackCount = $derived(library.tracks.length);
  let unanalyzedCount = $derived(open ? unanalyzedTrackCount() : 0);
  let analyzing = $derived(library.analyzing && library.analysisProgress.total > 0);
  let analysis = $derived(library.analysisProgress);
  let now = $state(Date.now());
  let timeLeft = $derived(analysisTimeLeft(library.analysisPace, analysis.current, analysis.total, now));
  let analyzingNow = $derived.by(() => {
    if (!open || !analyzing) return null;
    const id = library.analyzingTrackIds[0];
    const track = id === undefined ? null : library.tracks.find((t) => t.id === id);
    return track ? track.title || `Track ${track.id}` : null;
  });
  let toGo = $derived(Math.max(0, analysis.total - analysis.current));
  let importPercent = $derived(importActivity.total > 0 ? (importActivity.done / importActivity.total) * 100 : 0);
  let folder = $derived(importing ? null : importActivity.folder);
  let pickingPlaylist = $state(false);
  let placedIn = $state('');
  let playlistChoices = $derived(
    pickingPlaylist ? library.playlists.filter((p) => !p.is_folder && !isFavoritePlaylistName(p.name)) : [],
  );

  $effect(() => {
    if (!open || !analyzing) return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), 5000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (open) return;
    pickingPlaylist = false;
    placedIn = '';
  });

  async function addFolderTo(playlist) {
    if (!folder) return;
    pickingPlaylist = false;
    const added = await addTracksToPlaylist(playlist.id, folder.ids);
    placedIn = added === 0 ? `Already in ‘${playlist.name}’` : `Added ${added} to ‘${playlist.name}’`;
  }

  async function newPlaylistFromFolder() {
    if (!folder) return;
    const imported = folder;
    onClose?.();
    await playlistsFromImportedFolder(imported);
  }

  $effect(() => {
    if (!open) return;
    const returnFocusTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    void tick().then(() => menuEl?.querySelector('button:not([disabled])')?.focus());
    return () => {
      if (returnFocusTo?.isConnected && (document.activeElement === document.body || menuEl?.contains(document.activeElement))) returnFocusTo.focus();
    };
  });

  $effect(() => {
    if (!open) return;
    function onDoc(e) {
      if (menuEl && !menuEl.contains(e.target)) onClose?.();
    }
    function onKey(e) {
      closeOnEscape(e, onClose);
    }
    function onWindowBlur() {
      onClose?.();
    }
    const t = setTimeout(() => {
      document.addEventListener('pointerdown', onDoc, true);
      document.addEventListener('keydown', onKey);
      window.addEventListener('blur', onWindowBlur);
    }, 0);
    return () => {
      clearTimeout(t);
      document.removeEventListener('pointerdown', onDoc, true);
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('blur', onWindowBlur);
    };
  });

  function place(node) {
    const rect = node.getBoundingClientRect();
    node.style.left = `${Math.max(8, Math.min(x, window.innerWidth - rect.width - 8))}px`;
    node.style.top = `${Math.max(8, Math.min(y, window.innerHeight - rect.height - 8))}px`;
  }

  function run(action) {
    onClose?.();
    action();
  }
</script>

{#snippet entry(label, onclick, disabled = false)}
  <button type="button" class="ff-menu__item ff-menu__item--{disabled ? 'disabled' : 'default'} ff-activity__entry" {disabled} onclick={() => run(onclick)}>
    <span class="ff-menu__item-label">{label}</span>
  </button>
{/snippet}

{#if open}
  <div class="ff-menu ff-context-menu ff-activity" data-library-activity-menu bind:this={menuEl} use:place style="position: fixed; left: {x}px; top: {y}px; z-index: 10000;">
    <div class="ff-activity__section" data-library-summary>
      <span class="ff-activity__title">Library</span>
      <span class="ff-activity__headline">{trackCount.toLocaleString()} {trackCount === 1 ? 'track' : 'tracks'}</span>
      {#if trackCount > 0}
        <span class="ff-activity__line">
          {unanalyzedCount > 0 ? `${(trackCount - unanalyzedCount).toLocaleString()} analysed, ${unanalyzedCount.toLocaleString()} not analysed` : 'All analysed'}
        </span>
      {/if}
    </div>
    <div class="ff-menu__separator"></div>
    {#if importing}
      <div class="ff-activity__section" data-import-progress>
        <span class="ff-activity__row">
          <span class="ff-activity__title">Importing</span>
          {#if importActivity.total > 0}
            <span class="ff-activity__count">{importActivity.done} of {importActivity.total} files</span>
          {/if}
        </span>
        <ProgressBar variant={importActivity.total > 0 ? 'determinate' : 'indeterminate'} value={importPercent} />
        <span class="ff-activity__line">
          {importActivity.total === 0 ? 'Looking for audio files…' : importActivity.done < importActivity.total ? 'Reading tags…' : 'Adding to your library…'}
        </span>
      </div>
      <div class="ff-menu__separator"></div>
    {/if}
    {#if analyzing}
      <div class="ff-activity__section" data-analysis-progress>
        <span class="ff-activity__row">
          <span class="ff-activity__title">Analyzing</span>
          <span class="ff-activity__count">{analysis.current} of {analysis.total}</span>
        </span>
        <ProgressBar value={(analysis.current / analysis.total) * 100} />
        {#if analyzingNow}
          <span class="ff-activity__now" data-analysis-now><span class="ff-activity__muted">Now</span> {analyzingNow}</span>
        {/if}
        <span class="ff-activity__line" data-analysis-left>
          {toGo === 1 ? '1 track to go' : `${toGo.toLocaleString()} tracks to go`}{timeLeft ? ` · ${timeLeft.charAt(0).toLowerCase()}${timeLeft.slice(1)}` : ''}
        </span>
      </div>
      <div class="ff-menu__separator"></div>
    {/if}
    {#if library.syncing}
      <div class="ff-activity__section" data-sync-progress>
        <span class="ff-activity__title">Syncing</span>
        <span class="ff-activity__headline">{library.statusMessage}</span>
      </div>
      <div class="ff-menu__separator"></div>
    {/if}
    {#if !importing && importActivity.summary}
      <div class="ff-activity__section" data-last-import>
        <span class="ff-activity__title">Last import</span>
        <span class="ff-activity__headline">{importActivity.summary.headline}</span>
        {#each importActivity.summary.lines as line}
          <span class="ff-activity__line">{line}</span>
        {/each}
        {#if folder}
          <div class="ff-activity__actions" data-import-folder-actions>
            <Button size="small" variant="primary" label="New playlist from folder" title={`Create ‘${folder.name}’ from the imported folder`} onclick={() => void newPlaylistFromFolder()} />
            <Button size="small" label="Add to playlist…" onclick={() => (pickingPlaylist = !pickingPlaylist)} />
          </div>
          {#if placedIn}
            <span class="ff-activity__line" data-import-placed>{placedIn}</span>
          {/if}
        {/if}
      </div>
      {#if pickingPlaylist}
        <div class="ff-activity__picker" data-import-playlist-picker>
          {#each playlistChoices as playlist (playlist.id)}
            <button type="button" class="ff-menu__item ff-menu__item--default ff-activity__entry" onclick={() => void addFolderTo(playlist)}>
              <span class="ff-menu__item-label">{playlist.name}</span>
            </button>
          {:else}
            <span class="ff-activity__line ff-activity__empty">No playlists yet</span>
          {/each}
        </div>
      {/if}
      <div class="ff-menu__separator"></div>
    {/if}
    {#if hasProblems}
      {@render entry('Relocate missing…', relocateMissing, fileStatus.missing === 0 || fileStatus.relocating)}
      {#if fileStatus.onlyProblems}
        {@render entry('Show all tracks', () => showProblemTracks(false))}
      {:else}
        {@render entry('Show missing tracks', () => showProblemTracks(true))}
      {/if}
      {@render entry('Check again', checkLibraryAgain, fileStatus.running)}
      {@render entry('Library check results…', openHealthDialog)}
      <div class="ff-menu__separator"></div>
    {/if}
    {@render entry('Import folder…', () => importIntoSource(importDropTarget()), importing)}
    {@render entry('Import files…', () => importFilesIntoSource(importDropTarget()), importing)}
  </div>
{/if}

<style>
  .ff-activity {
    min-width: 240px;
    max-width: 320px;
  }
  .ff-activity__section {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-1);
    padding: var(--ff-space-2) var(--ff-space-4);
    font-size: var(--ff-type-compact);
  }
  .ff-activity__title {
    font-size: var(--ff-type-label);
    color: var(--ff-muted);
  }
  .ff-activity__headline {
    color: var(--ff-text);
  }
  .ff-activity__line {
    color: var(--ff-text-mid);
  }
  .ff-activity__row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--ff-space-4);
  }
  .ff-activity__count {
    font-family: var(--ff-font-mono);
    font-size: 11px;
    color: var(--ff-text);
  }
  .ff-activity__section :global(.ff-progress-bar) {
    margin: 2px 0;
  }
  .ff-activity__now {
    color: var(--ff-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ff-activity__muted {
    color: var(--ff-muted);
    margin-right: var(--ff-space-2);
  }
  .ff-activity__actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--ff-space-2);
    margin-top: var(--ff-space-2);
  }
  .ff-activity__picker {
    max-height: 196px;
    overflow-y: auto;
    padding-bottom: var(--ff-space-2);
  }
  .ff-activity__empty {
    display: block;
    padding: var(--ff-space-2) var(--ff-space-4);
  }
  .ff-activity__entry {
    width: 100%;
    height: 28px;
    padding-top: 0;
    padding-bottom: 0;
    border: none;
    background: none;
    text-align: left;
  }
  .ff-activity__entry:disabled {
    cursor: default;
  }
</style>
