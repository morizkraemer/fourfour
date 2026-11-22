<script>
  import { ContextMenu, Icon, Switch } from '$ds';
  import { nativeListener } from '$ds/components/nativeListener.js';
  import { library, libraryTrackById } from '../stores/library.svelte.ts';
  import { ui } from '../stores/ui.svelte.ts';
  import { dnd, endDrag, stickTargetAt, updatePointer } from '../stores/dnd.svelte.ts';
  import { dropFlash, cardKey, dropPlaylistsOnStick, hoveredStickDropLabel } from '../stores/organize-drop.svelte.ts';
  import {
    stickName,
    stickAllTrackIds,
    stickPlaylistIds,
    stickPlaylistTrackIds,
    removePlaylistFromStick,
    removeTracksFromStickAll,
    setStickPlaylistTracks,
    keepUpdatedSupported,
    keptAsIsSince,
    setKeepUpdated,
  } from '../stores/sticks.svelte.ts';
  import { playStickTrack } from '../stores/player.svelte.ts';
  import { formatBytes } from '../stores/stick-sync.svelte.ts';
  import { shortDate, syncAge } from '../lib/stickDrop.ts';
  import { moveTracks, sameOrder, withoutTracks } from '../lib/stickEdits.ts';
  import { buildStickPlaylistMenuItems, buildStickTrackMenuItems } from '../menus/context-menus.ts';
  import { goToSource, jumpToTrack } from '../shortcuts/navigation.ts';

  let { volumePath, embedded = false } = $props();

  const DRAG_START_PX = 5;
  const KEPT_NOTICE = 'This playlist now stays as it is on the stick. Your library did not change. Turn Keep updated on to follow it again.';

  let openRow = $state(null);
  let suppressClick = false;
  let selected = $state(new Set());
  let anchorId = null;
  let menu = $state({ open: false, x: 0, y: 0, items: [] });
  let notice = $state('');
  let reorder = $state(null);
  let tracksEl = $state(null);

  $effect(() => {
    volumePath;
    openRow = null;
  });

  $effect(() => {
    openRow;
    selected = new Set();
    anchorId = null;
    notice = '';
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
    !embedded &&
      !panelIsThisStick &&
      ((dnd.kind === 'playlist' && dnd.playlistDragFromStick !== volumePath) ||
        (dnd.kind === 'tracks' && dnd.dragOriginSourceId !== volumePath))
  );
  const stripHot = $derived(stripShown && dnd.dropStickPath === volumePath && dnd.dropStickSurface === 'strip');
  const flash = $derived(!embedded && dropFlash.key === cardKey({ kind: 'stick', volumePath }) ? dropFlash.text : '');

  function tracksOnStick(playlist) {
    return stickPlaylistTrackIds(volumePath, playlist.id, playlist.track_ids);
  }

  function sizeOf(trackIds) {
    let bytes = 0;
    for (const id of trackIds) bytes += libraryTrackById(id)?.raw?.file_size ?? 0;
    return bytes > 0 ? formatBytes(bytes) : '-';
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
      if (!playlist) return null;
      const trackIds = tracksOnStick(playlist);
      return { kind: 'playlist', id: playlist.id, name: playlist.name, trackIds, count: trackIds.length };
    }
    const foreign = onlyOnStick.find((p) => p.id === openRow.id);
    return foreign ? { kind: 'foreign', name: foreign.name, trackIds: [], count: foreign.track_count } : null;
  });
  const openedKeptSince = $derived(opened?.kind === 'playlist' ? keptAsIsSince(volumePath, opened.id) : null);
  const openedTracks = $derived(opened ? opened.trackIds.map(libraryTrackById).filter(Boolean) : []);
  const canReorder = $derived(opened?.kind === 'playlist' && canSwitch);
  const movingIds = $derived(reorder ? selected : null);

  function open(row) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    openRow = row;
  }

  function swallowNextClick() {
    suppressClick = true;
    setTimeout(() => (suppressClick = false), 0);
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
      swallowNextClick();
      const target = dnd.dropStickPath;
      endDrag();
      if (target) await dropPlaylistsOnStick(target, [playlistId], true);
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  function openPlaylistMenu(e, playlist) {
    e.preventDefault();
    menu = {
      open: true,
      x: e.clientX,
      y: e.clientY,
      items: buildStickPlaylistMenuItems({
        onOpen: () => (openRow = { kind: 'playlist', id: playlist.id }),
        onShowInLibrary: () => void goToSource(playlist.name),
        onTakeOff: () => void removePlaylistFromStick(volumePath, playlist.id),
      }),
    };
  }

  function selectTrack(e, track) {
    if (suppressClick) {
      suppressClick = false;
      return;
    }
    const ids = openedTracks.map((t) => t.id);
    if (e.shiftKey && anchorId !== null && ids.includes(anchorId)) {
      const [from, to] = [ids.indexOf(anchorId), ids.indexOf(track.id)].sort((a, b) => a - b);
      selected = new Set(ids.slice(from, to + 1));
      return;
    }
    if (e.metaKey || e.ctrlKey) {
      const next = new Set(selected);
      if (!next.delete(track.id)) next.add(track.id);
      selected = next;
    } else {
      selected = new Set([track.id]);
    }
    anchorId = track.id;
  }

  function selectedInOrder() {
    return openedTracks.map((t) => t.id).filter((id) => selected.has(id));
  }

  async function changePlaylistTracks(next) {
    if (opened?.kind !== 'playlist' || sameOrder(next, opened.trackIds)) return;
    const wasFollowing = !openedKeptSince;
    await setStickPlaylistTracks(volumePath, opened.id, next);
    if (wasFollowing) notice = KEPT_NOTICE;
  }

  async function removeTracks(ids) {
    if (!opened || ids.length === 0) return;
    if (opened.kind === 'all') await removeTracksFromStickAll(volumePath, ids);
    else await changePlaylistTracks(withoutTracks(opened.trackIds, ids));
    selected = new Set();
  }

  function openTrackMenu(e, track) {
    if (!opened || opened.kind === 'foreign') return;
    e.preventDefault();
    if (!selected.has(track.id)) {
      selected = new Set([track.id]);
      anchorId = track.id;
    }
    const ids = selectedInOrder();
    menu = {
      open: true,
      x: e.clientX,
      y: e.clientY,
      items: buildStickTrackMenuItems({
        scope: opened.kind,
        count: ids.length,
        canEditPlaylist: canSwitch,
        onRemove: () => void removeTracks(ids),
        onShowInLibrary: () => void jumpToTrack(ids[0]),
      }),
    };
  }

  function dropSlotAt(clientY) {
    const rows = [...(tracksEl?.querySelectorAll('[data-stick-track]') ?? [])];
    let index = rows.length;
    for (let i = 0; i < rows.length; i++) {
      const rect = rows[i].getBoundingClientRect();
      if (clientY < rect.top + rect.height / 2) {
        index = i;
        break;
      }
    }
    const last = rows[rows.length - 1];
    const top = index < rows.length ? rows[index].offsetTop : last ? last.offsetTop + last.offsetHeight : 0;
    return { index, top };
  }

  function onTrackPointerDown(e, track) {
    if (e.button !== 0 || !canReorder) return;
    let dragging = false;
    function onMove(ev) {
      if (!dragging) {
        if (Math.hypot(ev.clientX - e.clientX, ev.clientY - e.clientY) <= DRAG_START_PX) return;
        dragging = true;
        if (!selected.has(track.id)) {
          selected = new Set([track.id]);
          anchorId = track.id;
        }
      }
      reorder = { ...dropSlotAt(ev.clientY), x: ev.clientX, y: ev.clientY };
    }
    async function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      if (!dragging) return;
      swallowNextClick();
      const slot = reorder;
      reorder = null;
      if (!slot || !opened) return;
      const beforeId = openedTracks[slot.index]?.id;
      const before = beforeId === undefined ? opened.trackIds.length : opened.trackIds.indexOf(beforeId);
      await changePlaylistTracks(moveTracks(opened.trackIds, selectedInOrder(), before));
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

<section class="ff-stickview" class:ff-stickview--embedded={embedded} data-stick-view={volumePath} aria-label={name}>
  {#if opened}
    <header class="ff-stickview__plhead" data-stick-playlist-header>
      <div class="ff-stickview__plmain">
        <button type="button" class="ff-stickview__crumb" onclick={() => (openRow = null)}>
          {#if embedded}<Icon name="chevron-left" size={12} />On this stick{:else}{name}{/if}
        </button>
        <span class="ff-stickview__sep">/</span>
        <span class="ff-stickview__plname">{opened.name}</span>
        <span class="ff-stickview__plmeta">
          {opened.count} {opened.count === 1 ? 'track' : 'tracks'}{opened.kind === 'foreign' ? '' : ` · ${sizeOf(opened.trackIds)}`}
        </span>
      </div>
      {#if opened.kind === 'playlist' && canSwitch}
        <label class="ff-stickview__keep" data-keep-updated>
          <Switch
            on={!openedKeptSince}
            label="Keep updated"
            onChange={(on) => {
              notice = '';
              void setKeepUpdated(volumePath, opened.id, on);
            }}
          />
          <span>Keep updated</span>
          <small data-keep-note>
            {#if openedKeptSince}as of {shortDate(openedKeptSince)}{:else if lastSync}synced {syncAge(lastSync.at)}{/if}
          </small>
        </label>
      {:else if opened.kind === 'playlist'}
        <span class="ff-stickview__keep ff-stickview__keep--fixed" data-keep-updated-fixed>Follows your library</span>
      {/if}
    </header>
    {#if notice}
      <div class="ff-stickview__notice" role="status" data-stick-notice>
        <span>{notice}</span>
        <button type="button" class="ff-stickview__x ff-stickview__x--shown" aria-label="Dismiss" onclick={() => (notice = '')}>
          <Icon name="x" size={12} />
        </button>
      </div>
    {/if}
    {#if opened.kind === 'foreign'}
      <p class="ff-stickview__hint" data-stick-foreign-note>
        Made elsewhere. Sync never removes it.
      </p>
    {:else}
      <div class="ff-stickview__tracks" class:ff-stickview__tracks--reordering={reorder} data-stick-tracks bind:this={tracksEl}>
        <div class="ff-stickview__th ff-stickview__trow">
          <span>#</span><span>Title</span><span>Artist</span><span>BPM</span><span>Key</span>
        </div>
        {#each openedTracks as track, i (track.id)}
          <div
            class="ff-stickview__trow ff-stickview__track"
            class:ff-stickview__track--selected={selected.has(track.id)}
            class:ff-stickview__track--moving={movingIds?.has(track.id)}
            role="option"
            tabindex="0"
            data-stick-track={track.id}
            aria-selected={selected.has(track.id)}
            onpointerdown={(e) => onTrackPointerDown(e, track)}
            onclick={(e) => selectTrack(e, track)}
            ondblclick={() => void playStickTrack(volumePath, track)}
            onkeydown={(e) => e.key === 'Enter' && void playStickTrack(volumePath, track)}
            oncontextmenu={(e) => openTrackMenu(e, track)}
          >
            <span class="ff-stickview__num ff-stickview__idx">{String(i + 1).padStart(2, '0')}</span>
            <span class="ff-stickview__t">{track.title}</span>
            <span class="ff-stickview__mid">{track.artist}</span>
            <span class="ff-stickview__num">{track.bpm}</span>
            <span class="ff-stickview__num">{track.key}</span>
          </div>
        {/each}
        {#if reorder}
          <div class="ff-stickview__dropline" data-stick-dropline style:top="{reorder.top}px"></div>
        {/if}
        {#if opened.trackIds.length === 0}
          <p class="ff-stickview__hint">No tracks yet.</p>
        {:else}
          <p class="ff-stickview__foot" data-stick-edit-hint>
            {#if opened.kind === 'all'}
              Removing a track here takes it off the stick on the next sync. It stays in your library.
            {:else if canReorder}
              Reorder or remove tracks here to change what the next sync writes. Your library stays as it is.
            {/if}
          </p>
        {/if}
      </div>
    {/if}
  {:else}
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
        {@const trackIds = tracksOnStick(pl)}
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
          oncontextmenu={(e) => openPlaylistMenu(e, pl)}
        >
          <span class="ff-stickview__t">{pl.name}</span>
          <span class="ff-stickview__num">{trackIds.length}</span>
          <span class="ff-stickview__num">{sizeOf(trackIds)}</span>
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
            {@attach nativeListener('click', (e) => {
              e.stopPropagation();
              void removePlaylistFromStick(volumePath, pl.id);
            })}
          >
            <Icon name="x" size={12} />
          </button>
        </div>
      {/each}
      {#if playlists.length === 0}
        <p class="ff-stickview__hint">
          {embedded ? 'Drag a playlist here to put it on the stick.' : 'Drag a playlist from the sidebar to put it on this stick.'}
        </p>
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
            <span class="ff-stickview__num">-</span>
            <span></span>
            <span class="ff-stickview__muted">made elsewhere</span>
            <span></span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</section>

{#if reorder}
  <div class="ff-stickview__ghost" data-stick-reorder-ghost style:left="{reorder.x + 14}px" style:top="{reorder.y + 14}px">
    Move {selected.size === 1 ? '1 track' : `${selected.size} tracks`}
  </div>
{/if}

{#if menu.open}
  <ContextMenu open x={menu.x} y={menu.y} items={menu.items} onClose={() => (menu = { ...menu, open: false })} />
{/if}

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
    --ff-stickview-gutter: var(--ff-space-8);
  }
  .ff-stickview--embedded {
    --ff-stickview-gutter: var(--ff-space-6);
  }
  .ff-stickview__strip {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--ff-space-4);
    margin: var(--ff-space-4) var(--ff-stickview-gutter) 0;
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
    margin: var(--ff-space-4) var(--ff-stickview-gutter) 0;
    color: var(--ff-text);
  }
  .ff-stickview__list {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    padding: 0 0 var(--ff-space-8);
  }
  .ff-stickview__row {
    display: grid;
    grid-template-columns: minmax(160px, 1.8fr) 64px 80px 90px minmax(120px, 1fr) 28px;
    align-items: center;
    gap: var(--ff-space-4);
    height: 34px;
    padding: 0 var(--ff-stickview-gutter);
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
    height: 32px;
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
  .ff-stickview__muted {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-stickview--embedded .ff-stickview__plmain {
    flex-basis: 100%;
  }
  .ff-stickview--embedded .ff-stickview__plhead {
    padding-top: var(--ff-space-4);
    padding-bottom: var(--ff-space-4);
  }
  .ff-stickview__hint {
    margin: var(--ff-space-5) var(--ff-stickview-gutter);
  }
  .ff-stickview__foot {
    margin: var(--ff-space-6) var(--ff-stickview-gutter) 0;
    color: var(--ff-faint);
    font-size: var(--ff-type-small);
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
    flex: none;
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
  .ff-stickview__x:focus-visible,
  .ff-stickview__x--shown {
    opacity: 1;
  }
  .ff-stickview__x:hover {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-stickview__group {
    display: flex;
    justify-content: space-between;
    padding: var(--ff-space-6) var(--ff-stickview-gutter) var(--ff-space-3);
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
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--ff-space-3) var(--ff-space-6);
    min-height: 44px;
    padding: var(--ff-space-3) var(--ff-stickview-gutter);
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
    display: inline-flex;
    align-items: center;
    gap: var(--ff-space-2);
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
    font-size: var(--ff-type-body);
    font-weight: 500;
  }
  .ff-stickview__plmeta {
    flex: none;
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
  .ff-stickview__keep--fixed {
    color: var(--ff-muted);
    font-size: var(--ff-type-caption);
  }
  .ff-stickview__notice {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    padding: var(--ff-space-3) var(--ff-stickview-gutter);
    border-bottom: 1px solid var(--ff-border);
    color: var(--ff-text-mid);
    font-size: var(--ff-type-caption);
  }
  .ff-stickview__notice > span {
    flex: 1;
    min-width: 0;
  }
  .ff-stickview__tracks {
    container-type: inline-size;
    position: relative;
    display: flex;
    flex-direction: column;
    padding-bottom: var(--ff-space-8);
    user-select: none;
  }
  .ff-stickview__trow {
    display: grid;
    grid-template-columns: 34px minmax(120px, 1.7fr) minmax(90px, 1.2fr) 56px 48px;
    align-items: center;
    gap: var(--ff-space-4);
    height: 28px;
    padding: 0 var(--ff-stickview-gutter);
  }
  .ff-stickview__trow.ff-stickview__th {
    height: 32px;
  }
  .ff-stickview__idx {
    color: var(--ff-faint);
  }
  .ff-stickview__track:hover:not(.ff-stickview__track--selected) {
    background: var(--ff-hover);
  }
  .ff-stickview__track--selected {
    background: var(--ff-select);
  }
  .ff-stickview__track--selected .ff-stickview__t {
    color: var(--ff-accent);
    font-weight: 500;
  }
  .ff-stickview__track--moving {
    opacity: 0.5;
  }
  .ff-stickview__tracks--reordering,
  .ff-stickview__tracks--reordering .ff-stickview__track {
    cursor: grabbing;
  }
  .ff-stickview__dropline {
    position: absolute;
    left: var(--ff-stickview-gutter);
    right: var(--ff-stickview-gutter);
    height: 2px;
    margin-top: -1px;
    border-radius: 1px;
    background: var(--ff-accent);
    pointer-events: none;
  }
  .ff-stickview__ghost {
    position: fixed;
    z-index: 1000;
    padding: 5px 11px;
    border-radius: 5px;
    background: var(--ff-accent);
    color: var(--ff-bg);
    font-size: var(--ff-type-compact);
    font-weight: 500;
    white-space: nowrap;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    pointer-events: none;
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
  @container (max-width: 460px) {
    .ff-stickview__trow {
      grid-template-columns: 26px minmax(90px, 1.6fr) minmax(70px, 1fr) 40px;
    }
    .ff-stickview__trow > :nth-child(4) {
      display: none;
    }
  }
  .ff-stickview__mid {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
