<!--
  OrganizePicker.svelte — Destination picker rendered in the OrganizePanel body
  when no target is docked (ui.organize.target === null).

  Three groups:
    1. ＋ New playlist — creates immediately, docks, focuses inline rename.
    2. Playlist search — text input filtering playlists by name, ≤4 results;
       empty query shows ≤4 recents (falls back to first few playlists).
    3. USB devices — lists library.volumes.

  Read-only library views (All Tracks, Recently Added) are intentionally omitted.
-->
<script>
  import { Icon } from '$ds';
  import {
    library,
    addPlaylist,
    defaultPlaylistName,
    isFavoritePlaylistName,
  } from '../stores/library.svelte.ts';
  import { dockOrganizeTarget } from '../stores/ui.svelte.ts';

  const MAX_RESULTS = 4;

  /**
   * Called after a new playlist is created and docked, passing the initial
   * playlist name. OrganizePanel uses this to trigger inline header rename.
   */
  let { onNewPlaylistDocked = null } = $props();

  async function handleNewPlaylist() {
    const name = defaultPlaylistName();
    const newId = await addPlaylist(name);
    // Dock immediately — OrganizePanel body will switch to TrackTable
    dockOrganizeTarget({ kind: 'playlist', id: name, label: name, playlistId: newId });
    // Signal parent to start inline header rename
    onNewPlaylistDocked?.(name);
  }

  // ── Playlist search / recents ───────────────────────────────────────────────
  let query = $state('');

  /** Playlists eligible as destinations: exclude Favorites N and read-only views. */
  let eligiblePlaylists = $derived(
    library.playlists.filter((p) => !isFavoritePlaylistName(p.name))
  );

  /** Recents: resolve recent playlist ids to playlist objects in order. */
  let recentPlaylists = $derived(
    ui.organize.recentPlaylistIds
      .map((id) => eligiblePlaylists.find((p) => p.id === id))
      .filter(Boolean)
      .slice(0, MAX_RESULTS)
  );

  /** Fallback when no recents: first N eligible playlists. */
  let fallbackPlaylists = $derived(eligiblePlaylists.slice(0, MAX_RESULTS));

  /** Search results when query is non-empty. */
  let searchResults = $derived(
    query.trim()
      ? eligiblePlaylists
          .filter((p) => p.name.toLowerCase().includes(query.trim().toLowerCase()))
          .slice(0, MAX_RESULTS)
      : []
  );

  /** The playlist rows currently visible. */
  let visiblePlaylists = $derived(
    query.trim()
      ? searchResults
      : recentPlaylists.length > 0
        ? recentPlaylists
        : fallbackPlaylists
  );

  let emptySearchState = $derived(query.trim() && searchResults.length === 0);

  function pickPlaylist(pl) {
    dockOrganizeTarget({ kind: 'playlist', id: pl.name, label: pl.name, playlistId: pl.id });
  }

  // ── USB devices ─────────────────────────────────────────────────────────────
  function volumeLabel(path) {
    return path.replace(/\\/g, '/').split('/').pop() || path;
  }

  function pickVolume(path) {
    dockOrganizeTarget({ kind: 'usb', id: path, label: volumeLabel(path) });
  }
</script>

<div class="ff-picker">
  <!-- ── New playlist ─────────────────────────────────────────────────── -->
  <div class="ff-picker__section">
    <button class="ff-picker__action-row" onclick={() => void handleNewPlaylist()}>
      <span class="ff-picker__action-icon"><Icon name="plus" size={14} /></span>
      <span class="ff-picker__action-label">New playlist</span>
    </button>
  </div>

  <!-- ── Playlist search ──────────────────────────────────────────────── -->
  <div class="ff-picker__section ff-picker__section--playlists">
    <div class="ff-picker__search-row">
      <span class="ff-picker__search-icon"><Icon name="search" size={12} /></span>
      <input
        class="ff-picker__search-input"
        type="search"
        placeholder="Search playlists…"
        bind:value={query}
        autocomplete="off"
        spellcheck={false}
      />
      {#if query}
        <button
          class="ff-picker__search-clear"
          aria-label="Clear search"
          onclick={() => (query = '')}
        >×</button>
      {/if}
    </div>

    <div class="ff-picker__list">
      {#if emptySearchState}
        <div class="ff-picker__empty">No playlists match "{query}"</div>
      {:else if visiblePlaylists.length === 0}
        <div class="ff-picker__empty">No playlists yet</div>
      {:else}
        {#each visiblePlaylists as pl (pl.id)}
          <button class="ff-picker__row" onclick={() => pickPlaylist(pl)}>
            <span class="ff-picker__row-icon"><Icon name="list" size={13} /></span>
            <span class="ff-picker__row-label">{pl.name}</span>
            <span class="ff-picker__row-count">{pl.track_ids.length}</span>
          </button>
        {/each}
      {/if}
    </div>
  </div>

  <!-- ── USB devices ──────────────────────────────────────────────────── -->
  {#if library.volumes.length > 0}
    <div class="ff-picker__section ff-picker__section--usb">
      <div class="ff-picker__section-label">USB Devices</div>
      {#each library.volumes as vol (vol)}
        <button class="ff-picker__row" onclick={() => pickVolume(vol)}>
          <span class="ff-picker__row-icon"><Icon name="disc" size={13} /></span>
          <span class="ff-picker__row-label">{volumeLabel(vol)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .ff-picker {
    display: flex;
    flex-direction: column;
    width: 100%;
    gap: 0;
    overflow-y: auto;
    padding: var(--ff-space-3) 0;
  }

  /* ── Sections ──────────────────────────────────────────────────────── */
  .ff-picker__section {
    display: flex;
    flex-direction: column;
    padding: 0 var(--ff-space-3);
  }

  .ff-picker__section + .ff-picker__section {
    margin-top: var(--ff-space-3);
    padding-top: var(--ff-space-3);
    border-top: 1px solid var(--ff-border);
  }

  .ff-picker__section-label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ff-faint);
    padding: 0 var(--ff-space-2);
    margin-bottom: var(--ff-space-2);
  }

  /* ── New playlist action row ───────────────────────────────────────── */
  .ff-picker__action-row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-2);
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    padding: var(--ff-space-2) var(--ff-space-2);
    border-radius: var(--ff-radius-sm, 4px);
    color: var(--ff-accent);
    font-size: var(--ff-text-sm, 12px);
    transition: background 0.1s ease;
    text-align: left;
  }

  .ff-picker__action-row:hover {
    background: var(--ff-hover);
  }

  .ff-picker__action-icon {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .ff-picker__action-label {
    font-weight: 500;
  }

  /* ── Search input row ──────────────────────────────────────────────── */
  .ff-picker__search-row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-1);
    background: var(--ff-input-bg, var(--ff-surface-hi));
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-sm, 4px);
    padding: 4px var(--ff-space-2);
    margin-bottom: var(--ff-space-2);
  }

  .ff-picker__search-icon {
    display: flex;
    align-items: center;
    color: var(--ff-faint);
    flex-shrink: 0;
  }

  .ff-picker__search-input {
    flex: 1 1 0;
    background: none;
    border: none;
    outline: none;
    color: var(--ff-text);
    font-size: var(--ff-text-sm, 12px);
    min-width: 0;
    appearance: none;
  }

  .ff-picker__search-input::placeholder {
    color: var(--ff-faint);
  }

  /* Hide the native search clear button (webkit) */
  .ff-picker__search-input::-webkit-search-cancel-button {
    display: none;
  }

  .ff-picker__search-clear {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--ff-faint);
    font-size: 14px;
    line-height: 1;
    padding: 0;
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .ff-picker__search-clear:hover {
    color: var(--ff-text);
  }

  /* ── List rows ─────────────────────────────────────────────────────── */
  .ff-picker__list {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .ff-picker__row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-2);
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    padding: var(--ff-space-2) var(--ff-space-2);
    border-radius: var(--ff-radius-sm, 4px);
    color: var(--ff-text);
    font-size: var(--ff-text-sm, 12px);
    transition: background 0.1s ease;
    text-align: left;
  }

  .ff-picker__row:hover {
    background: var(--ff-hover);
  }

  .ff-picker__row-icon {
    display: flex;
    align-items: center;
    color: var(--ff-faint);
    flex-shrink: 0;
  }

  .ff-picker__row-label {
    flex: 1 1 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ff-picker__row-count {
    color: var(--ff-faint);
    font-size: 11px;
    flex-shrink: 0;
  }

  /* ── Empty state ───────────────────────────────────────────────────── */
  .ff-picker__empty {
    font-size: var(--ff-text-sm, 12px);
    color: var(--ff-faint);
    padding: var(--ff-space-2) var(--ff-space-2);
    font-style: italic;
  }
</style>
