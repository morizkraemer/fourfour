<script>
  import { DestinationCard, Icon } from '$ds';
  import {
    library,
    isFavoritePlaylistName,
    tracksForSidebarSource,
  } from '../stores/library.svelte.ts';
  import { dnd } from '../stores/dnd.svelte.ts';
  import { ui, dockOrganizeTarget } from '../stores/ui.svelte.ts';
  import { createPlaylistAndOpen } from '../stores/new-playlist.svelte.ts';
  import { dropFlash, cardKey } from '../stores/organize-drop.svelte.ts';
  import { stickAllTrackIds } from '../stores/sticks.svelte.ts';

  const MAX_RECENT = 4;

  let query = $state('');

  let eligiblePlaylists = $derived(library.playlists.filter((p) => !isFavoritePlaylistName(p.name)));

  let recentPlaylists = $derived.by(() => {
    const recent = ui.organize.recentPlaylistIds
      .map((id) => eligiblePlaylists.find((p) => p.id === id))
      .filter(Boolean);
    return (recent.length > 0 ? recent : eligiblePlaylists).slice(0, MAX_RECENT);
  });

  let needle = $derived(query.trim().toLowerCase());

  let visiblePlaylists = $derived(
    needle ? eligiblePlaylists.filter((p) => p.name.toLowerCase().includes(needle)) : recentPlaylists
  );

  let visibleSticks = $derived(
    needle
      ? library.volumes.filter((v) => volumeLabel(v).toLowerCase().includes(needle))
      : library.volumes
  );

  function volumeLabel(path) {
    return path.replace(/\\/g, '/').split('/').pop() || path;
  }

  function coversFor(playlist) {
    return tracksForSidebarSource(playlist.name)
      .map((t) => t.cover)
      .filter(Boolean)
      .slice(0, 4);
  }

  function trackCountLabel(count) {
    return `${count} ${count === 1 ? 'track' : 'tracks'}`;
  }

  function flashFor(target) {
    return dropFlash.key === cardKey(target) ? dropFlash.text : '';
  }

  function isDropTarget(active) {
    return dnd.kind === 'tracks' && active;
  }

  function pickPlaylist(pl) {
    dockOrganizeTarget({ kind: 'playlist', id: pl.name, label: pl.name, playlistId: pl.id });
  }

  function pickVolume(path) {
    dockOrganizeTarget({ kind: 'usb', id: path, label: volumeLabel(path) });
  }
</script>

<div class="ff-picker">
  <label class="ff-picker__search">
    <Icon name="search" size={13} />
    <input
      type="search"
      placeholder="Find playlist or stick…"
      bind:value={query}
      autocomplete="off"
      spellcheck={false}
    />
  </label>

  <div class="ff-picker__label">{needle ? 'PLAYLISTS' : 'RECENT'}</div>
  <div class="ff-picker__cards">
    {#if !needle}
      <DestinationCard
        variant="new"
        name="New playlist"
        meta="⌘⇧N"
        onclick={() => void createPlaylistAndOpen()}
      />
    {/if}
    {#each visiblePlaylists as pl (pl.id)}
      {@const target = { kind: 'playlist', playlistId: pl.id }}
      <DestinationCard
        name={pl.name}
        meta={trackCountLabel(pl.track_ids.length)}
        covers={coversFor(pl)}
        dropActive={isDropTarget(dnd.dropCardPlaylistId === pl.id)}
        flash={flashFor(target)}
        data-drop-playlist-id={pl.id}
        onclick={() => pickPlaylist(pl)}
      />
    {/each}
  </div>
  {#if needle && visiblePlaylists.length === 0}
    <div class="ff-picker__none">No playlists match</div>
  {/if}

  <div class="ff-picker__label">STICKS</div>
  <div class="ff-picker__sticks">
    {#each visibleSticks as vol (vol)}
      {@const target = { kind: 'stick', volumePath: vol }}
      <DestinationCard
        variant="stick"
        name={volumeLabel(vol)}
        meta={`All tracks · ${stickAllTrackIds(vol).length}`}
        dropActive={isDropTarget(dnd.dropStickPath === vol)}
        flash={flashFor(target)}
        data-drop-stick-path={vol}
        onclick={() => pickVolume(vol)}
      />
    {/each}
    {#if visibleSticks.length === 0}
      <div class="ff-picker__none">{needle ? 'No sticks match' : 'No stick plugged in'}</div>
    {/if}
  </div>

  <div class="ff-picker__hint">Drag tracks onto a card to put them there. Click a card to open it here.</div>
</div>

<style>
  .ff-picker {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    align-self: stretch;
    width: 100%;
    gap: var(--ff-space-5);
    padding: var(--ff-space-5);
    overflow-y: auto;
  }

  .ff-picker__search {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
    color: var(--ff-faint);
  }

  .ff-picker__search input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: none;
    color: var(--ff-text);
    font-family: var(--ff-font);
    font-size: var(--ff-type-row);
    appearance: none;
  }

  .ff-picker__search input::placeholder {
    color: var(--ff-faint);
  }

  .ff-picker__search input::-webkit-search-cancel-button {
    display: none;
  }

  .ff-picker__label {
    margin-top: var(--ff-space-3);
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label-sm);
    letter-spacing: 0.06em;
    color: var(--ff-faint);
  }

  .ff-picker__cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: var(--ff-space-4);
  }

  .ff-picker__sticks {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }

  .ff-picker__none {
    font-size: var(--ff-type-caption);
    color: var(--ff-faint);
  }

  .ff-picker__hint {
    font-size: var(--ff-type-caption);
    color: var(--ff-faint);
  }
</style>
