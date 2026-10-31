<script>
  import { Icon, Switch } from '$ds';
  import { library, libraryTrackById } from '../stores/library.svelte.ts';
  import { ui } from '../stores/ui.svelte.ts';
  import { dnd, endDrag, stickTargetAt, updatePointer } from '../stores/dnd.svelte.ts';
  import { dropFlash, cardKey, dropPlaylistsOnStick, hoveredStickDropLabel } from '../stores/organize-drop.svelte.ts';
  import {
    stickName,
    stickAllTrackIds,
    stickPlaylistIds,
    removePlaylistFromStick,
    keepUpdatedSupported,
    keptAsIsSince,
    setKeepUpdated,
  } from '../stores/sticks.svelte.ts';
  import { formatBytes } from '../stores/stick-sync.svelte.ts';
  import { shortDate, syncAge } from '../lib/stickDrop.ts';
  import StickHeader from './StickHeader.svelte';

  let { volumePath } = $props();

  let openRow = $state(null);
  let suppressClick = false;

  $effect(() => {
    volumePath;
    openRow = null;
  });

  const name = $derived(stickName(volumePath));
  const allIds = $derived(stickAllTrackIds(volumePath));
  const playlists = $derived(
    stickPlaylistIds(volumePath).map((id) => library.playlists.find((p) => p.id === id)).filter(Boolean)
  );
  const onlyOnStick = $derived.by(() => {
    const names = new Set(library.playlists.map((p) => p.name));
    return library.activeVolume === volumePath ? library.usbPlaylists.filter((p) => !names.has(p.name)) : [];
  });
  const lastSync = $derived(library.activeVolume === volumePath ? library.usbLastSync : null);
  const canSwitch = $derived(keepUpdatedSupported(volumePath));
  const panelIsThisStick = $derived(
    ui.organize.expanded && ui.organize.target?.kind === 'usb' && ui.organize.target.id === volumePath
  );
  const stripShown = $derived(
    !panelIsThisStick &&
      ((dnd.kind === 'playlist' && dnd.playlistDragFromStick !== volumePath) ||
        (dnd.kind === 'tracks' && dnd.dragOriginSourceId !== volumePath))
  );
  const stripHot = $derived(stripShown && dnd.dropStickPath === volumePath && dnd.dropStickSurface === 'strip');
  const flash = $derived(dropFlash.key === cardKey({ kind: 'stick', volumePath }) ? dropFlash.text : '');

  function sizeOf(trackIds) {
    let bytes = 0;
    for (const id of trackIds) bytes += libraryTrackById(id)?.raw?.file_size ?? 0;
    return bytes > 0 ? formatBytes(bytes) : '—';
  }

  function tracksOf(trackIds) {
    return trackIds.map(libraryTrackById).filter(Boolean);
  }

  function rowNote(playlistId) {
    const since = keptAsIsSince(volumePath, playlistId);
    return since ? `as of ${shortDate(since)}` : 'follows your library';
  }

  const opened = $derived.by(() => {
    if (!openRow) return null;
    if (openRow.kind === 'all') {
      return { kind: 'all', name: 'All tracks', trackIds: allIds, count: allIds.length };
    }
    if (openRow.kind === 'playlist') {
      const playlist = playlists.find((p) => p.id === openRow.id);
      return playlist
        ? { kind: 'playlist', id: playlist.id, name: playlist.name, trackIds: playlist.track_ids, count: playlist.track_ids.length }
        : null;
    }
    const foreign = onlyOnStick.find((p) => p.id === openRow.id);
    return foreign ? { kind: 'foreign', name: foreign.name, trackIds: [], count: foreign.track_count } : null;
  });
  const openedKeptSince = $derived(opened?.kind === 'playlist' ? keptAsIsSince(volumePath, opened.id) : null);

  function open(row) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    openRow = row;
  }

  function onRowPointerDown(e, playlistId) {
    if (e.button !== 0 || e.target.closest('button')) return;
    let moved = false;
    function onMove(ev) {
      if (!moved && Math.hypot(ev.clientX - e.clientX, ev.clientY - e.clientY) > 6) {
        moved = true;
        dnd.kind = 'playlist';
        dnd.playlistDragIds = [playlistId];
        dnd.playlistDragFromStick = volumePath;
      }
      if (!moved) return;
      updatePointer(ev.clientX, ev.clientY);
      const target = stickTargetAt(ev.clientX, ev.clientY, volumePath);
      dnd.dropStickPath = target.path;
      dnd.dropStickSurface = target.surface;
    }
    async function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (!moved) return;
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
      const target = dnd.dropStickPath;
      endDrag();
      if (target) await dropPlaylistsOnStick(target, [playlistId], true);
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

<section class="ff-stickview" data-stick-view={volumePath} aria-label={name}>
  {#if opened}
    <header class="ff-stickview__plhead" data-stick-playlist-header>
      <div class="ff-stickview__plmain">
        <button type="button" class="ff-stickview__crumb" onclick={() => (openRow = null)}>{name}</button>
        <span class="ff-stickview__sep">/</span>
        <span class="ff-stickview__plname">{opened.name}</span>
        <span class="ff-stickview__plmeta">
          {opened.count} {opened.count === 1 ? 'track' : 'tracks'}{#if opened.kind !== 'foreign'} · {sizeOf(opened.trackIds)}{/if}
        </span>
      </div>
      {#if opened.kind === 'playlist'}
        <label class="ff-stickview__keep" data-keep-updated title={canSwitch ? '' : 'Always on in this version'}>
          <Switch
            on={!openedKeptSince}
            label="Keep updated"
            disabled={!canSwitch}
            onChange={(on) => void setKeepUpdated(volumePath, opened.id, on)}
          />
          <span>Keep updated</span>
          <small data-keep-note>
            {#if openedKeptSince}as of {shortDate(openedKeptSince)}{:else if lastSync}synced {syncAge(lastSync.at)}{/if}
          </small>
        </label>
      {/if}
    </header>
    {#if opened.kind === 'foreign'}
      <p class="ff-stickview__hint" data-stick-foreign-note>
        Made elsewhere. Sync never removes it.
      </p>
    {:else}
      <div class="ff-stickview__tracks" data-stick-tracks>
        <div class="ff-stickview__th ff-stickview__trow">
          <span>#</span><span>Title</span><span>Artist</span><span>BPM</span><span>Key</span>
        </div>
        {#each tracksOf(opened.trackIds) as track, i (track.id)}
          <div class="ff-stickview__trow">
            <span class="ff-stickview__num">{String(i + 1).padStart(2, '0')}</span>
            <span class="ff-stickview__t">{track.title}</span>
            <span class="ff-stickview__mid">{track.artist}</span>
            <span class="ff-stickview__num">{track.bpm}</span>
            <span class="ff-stickview__num">{track.key}</span>
          </div>
        {/each}
        {#if opened.trackIds.length === 0}
          <p class="ff-stickview__hint">No tracks yet.</p>
        {/if}
      </div>
    {/if}
  {:else}
    <StickHeader {volumePath} />

    {#if stripShown}
      <div
        class="ff-stickview__strip"
        class:ff-stickview__strip--hot={stripHot}
        data-put-strip
        data-drop-stick-path={volumePath}
        data-drop-surface="strip"
      >
        <Icon name="plus" size={13} />
        <span>{stripHot ? hoveredStickDropLabel() : `Put on ${name}`}</span>
      </div>
    {/if}
    {#if flash}
      <div class="ff-stickview__flash" role="status" data-stick-flash>{flash}</div>
    {/if}

    <div class="ff-stickview__list" data-stick-list>
      <div class="ff-stickview__th ff-stickview__row">
        <span>Playlist</span><span>Tracks</span><span>Size</span><span>Synced</span><span>Note</span><span></span>
      </div>
      <button type="button" class="ff-stickview__row" data-stick-row="all" onclick={() => open({ kind: 'all' })}>
        <span class="ff-stickview__t">All tracks</span>
        <span class="ff-stickview__num">{allIds.length}</span>
        <span class="ff-stickview__num">{sizeOf(allIds)}</span>
        <span></span>
        <span class="ff-stickview__muted">tracks dropped on the stick</span>
        <span></span>
      </button>
      {#each playlists as pl (pl.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="ff-stickview__row"
          role="button"
          tabindex="0"
          data-stick-row="playlist"
          data-playlist-id={pl.id}
          onpointerdown={(e) => onRowPointerDown(e, pl.id)}
          onclick={() => open({ kind: 'playlist', id: pl.id })}
          onkeydown={(e) => e.key === 'Enter' && open({ kind: 'playlist', id: pl.id })}
        >
          <span class="ff-stickview__t">{pl.name}</span>
          <span class="ff-stickview__num">{pl.track_ids.length}</span>
          <span class="ff-stickview__num">{sizeOf(pl.track_ids)}</span>
          <span>
            {#if !keptAsIsSince(volumePath, pl.id)}
              <span class="ff-stickview__synced" data-synced-mark>
                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M13 6.5A5 5 0 0 0 4 5" /><path d="M4 2.5v2.7h2.7" /><path d="M3 9.5A5 5 0 0 0 12 11" /><path d="M12 13.5v-2.7H9.3" /></svg>
                Synced
              </span>
            {/if}
          </span>
          <span class="ff-stickview__muted">{rowNote(pl.id)}</span>
          <button
            type="button"
            class="ff-stickview__x"
            aria-label="Take {pl.name} off this stick"
            title="Take off this stick"
            onclick={(e) => {
              e.stopPropagation();
              void removePlaylistFromStick(volumePath, pl.id);
            }}
          >
            <Icon name="x" size={12} />
          </button>
        </div>
      {/each}
      {#if playlists.length === 0}
        <p class="ff-stickview__hint">Drag a playlist from the sidebar to put it on this stick.</p>
      {/if}
      {#if onlyOnStick.length > 0}
        <div class="ff-stickview__group" data-only-on-stick>
          <span>Only on this stick</span>
          <span class="ff-stickview__group-note">Sync never removes these</span>
        </div>
        {#each onlyOnStick as pl (pl.id)}
          <button
            type="button"
            class="ff-stickview__row"
            data-stick-row="foreign"
            onclick={() => open({ kind: 'foreign', id: pl.id })}
          >
            <span class="ff-stickview__t">{pl.name}</span>
            <span class="ff-stickview__num">{pl.track_count}</span>
            <span class="ff-stickview__num">—</span>
            <span></span>
            <span class="ff-stickview__muted">made elsewhere</span>
            <span></span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</section>

<style>
  .ff-stickview {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    height: 100%;
    min-height: 0;
    overflow: auto;
    color: var(--ff-text-mid);
    font-size: var(--ff-type-row);
  }
  .ff-stickview__strip {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--ff-space-4);
    margin: var(--ff-space-4) var(--ff-space-8) 0;
    height: 40px;
    border: 1px dashed var(--ff-border-hi);
    border-radius: 6px;
    color: var(--ff-text-mid);
  }
  .ff-stickview__strip--hot {
    border: 1px solid rgba(255, 255, 255, 0.4);
    background: var(--ff-hover);
    color: var(--ff-text);
    font-weight: 500;
  }
  .ff-stickview__flash {
    margin: var(--ff-space-4) var(--ff-space-8) 0;
    color: var(--ff-text);
  }
  .ff-stickview__list {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    padding: var(--ff-space-3) 0 var(--ff-space-8);
  }
  .ff-stickview__row {
    display: grid;
    grid-template-columns: minmax(160px, 1.8fr) 64px 80px 90px minmax(120px, 1fr) 28px;
    align-items: center;
    gap: var(--ff-space-4);
    height: 34px;
    padding: 0 var(--ff-space-8);
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .ff-stickview__row:hover {
    background: var(--ff-hover);
  }
  .ff-stickview__th,
  .ff-stickview__th:hover {
    height: 26px;
    background: transparent;
    cursor: default;
    color: var(--ff-faint);
    font-size: var(--ff-type-label);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-stickview__t {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ff-text);
    font-size: var(--ff-type-body);
  }
  .ff-stickview__num {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-num);
  }
  .ff-stickview__muted,
  .ff-stickview__hint {
    color: var(--ff-muted);
  }
  .ff-stickview__hint {
    margin: var(--ff-space-5) var(--ff-space-8);
  }
  .ff-stickview__synced {
    display: inline-flex;
    align-items: center;
    gap: var(--ff-space-2);
    font-size: var(--ff-type-small);
  }
  .ff-stickview__synced svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ff-stickview__x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--ff-muted);
    cursor: pointer;
    opacity: 0;
  }
  .ff-stickview__row:hover .ff-stickview__x,
  .ff-stickview__x:focus-visible {
    opacity: 1;
  }
  .ff-stickview__x:hover {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-stickview__group {
    display: flex;
    justify-content: space-between;
    padding: var(--ff-space-6) var(--ff-space-8) var(--ff-space-3);
    color: var(--ff-muted);
    font-size: var(--ff-type-label);
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }
  .ff-stickview__group-note {
    letter-spacing: 0;
    text-transform: none;
    font-size: var(--ff-type-small);
  }
  .ff-stickview__plhead {
    container-type: inline-size;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--ff-space-3) var(--ff-space-6);
    min-height: 52px;
    padding: var(--ff-space-4) var(--ff-space-8);
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-stickview__plmain {
    display: flex;
    align-items: baseline;
    gap: var(--ff-space-3);
    min-width: 0;
    flex: 1;
    white-space: nowrap;
  }
  .ff-stickview__crumb {
    border: none;
    background: transparent;
    padding: 0;
    color: var(--ff-text-mid);
    font: inherit;
    cursor: pointer;
  }
  .ff-stickview__crumb:hover {
    color: var(--ff-text);
    text-decoration: underline;
  }
  .ff-stickview__sep {
    color: var(--ff-faint);
  }
  .ff-stickview__plname {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ff-text);
    font-size: var(--ff-type-h2);
    font-weight: 600;
  }
  .ff-stickview__plmeta {
    color: var(--ff-muted);
    font-size: var(--ff-type-caption);
  }
  .ff-stickview__keep {
    display: inline-flex;
    align-items: center;
    gap: var(--ff-space-4);
    color: var(--ff-text);
    font-size: var(--ff-type-compact);
    white-space: nowrap;
  }
  .ff-stickview__keep small {
    color: var(--ff-text-mid);
    font-size: var(--ff-type-caption);
  }
  .ff-stickview__tracks {
    display: flex;
    flex-direction: column;
    padding-bottom: var(--ff-space-8);
  }
  .ff-stickview__trow {
    display: grid;
    grid-template-columns: 34px minmax(150px, 1.7fr) minmax(110px, 1.2fr) 56px 48px;
    align-items: center;
    gap: var(--ff-space-4);
    height: 28px;
    padding: 0 var(--ff-space-8);
  }
  @container (max-width: 640px) {
    .ff-stickview__row {
      grid-template-columns: minmax(120px, 1fr) 52px 80px 28px;
    }
    .ff-stickview__row > :nth-child(3),
    .ff-stickview__row > :nth-child(5) {
      display: none;
    }
  }
  .ff-stickview__mid {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
