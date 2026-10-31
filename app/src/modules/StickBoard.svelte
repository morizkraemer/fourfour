<script>
  import { Icon } from '$ds';
  import { library } from '../stores/library.svelte.ts';
  import { dnd } from '../stores/dnd.svelte.ts';
  import { dropFlash, cardKey, hoveredStickDropLabel } from '../stores/organize-drop.svelte.ts';
  import { stickAllTrackIds, stickPlaylistIds, removePlaylistFromStick, keptAsIsSince } from '../stores/sticks.svelte.ts';
  import { shortDate } from '../lib/stickDrop.ts';
  import StickHeader from './StickHeader.svelte';

  let { volumePath } = $props();

  const allCount = $derived(stickAllTrackIds(volumePath).length);
  const playlists = $derived(
    stickPlaylistIds(volumePath).map((id) => library.playlists.find((p) => p.id === id)).filter(Boolean)
  );
  const dropHover = $derived(
    (dnd.kind === 'playlist' || dnd.kind === 'tracks') && dnd.dropStickPath === volumePath && dnd.dropStickSurface === 'board'
  );
  const flash = $derived(dropFlash.key === cardKey({ kind: 'stick', volumePath }) ? dropFlash.text : '');
</script>

<section
  class="ff-stickboard"
  class:ff-stickboard--drop={dropHover}
  data-stick-board
  data-drop-stick-path={volumePath}
  data-drop-surface="board"
  aria-label="On this stick"
>
  <StickHeader {volumePath} showName={false} />

  {#if dropHover}
    <div class="ff-stickboard__droplabel" data-drop-label>{hoveredStickDropLabel()}</div>
  {/if}

  <div class="ff-stickboard__inner">
    <div class="ff-stickboard__label">ON THIS STICK</div>
    <ul class="ff-stickboard__list" data-stick-list>
      <li class="ff-stickboard__row" data-stick-row="all">
        <Icon name="list" size={13} />
        <span class="ff-stickboard__name">All tracks</span>
        <span class="ff-stickboard__count">{allCount}</span>
      </li>
      {#each playlists as pl (pl.id)}
        {@const since = keptAsIsSince(volumePath, pl.id)}
        <li class="ff-stickboard__row" data-stick-row="playlist" data-playlist-id={pl.id}>
          <Icon name="list" size={13} />
          <span class="ff-stickboard__name">{pl.name}</span>
          <span class="ff-stickboard__mark" data-synced-mark={since ? undefined : ''}>{since ? `as of ${shortDate(since)}` : 'Synced'}</span>
          <span class="ff-stickboard__count">{pl.track_ids.length}</span>
          <button
            type="button"
            class="ff-stickboard__x"
            aria-label="Take {pl.name} off this stick"
            title="Take off this stick"
            onclick={() => void removePlaylistFromStick(volumePath, pl.id)}
          >
            <Icon name="x" size={12} />
          </button>
        </li>
      {/each}
    </ul>
    {#if playlists.length === 0}
      <div class="ff-stickboard__hint">{flash || 'Drag a playlist here to put it on the stick.'}</div>
    {:else if flash}
      <div class="ff-stickboard__hint" role="status">{flash}</div>
    {/if}
  </div>
</section>

<style>
  .ff-stickboard {
    position: relative;
    display: flex;
    flex-direction: column;
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-stickboard__inner {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-3);
    padding: var(--ff-space-4) var(--ff-space-5);
  }
  .ff-stickboard__droplabel {
    position: absolute;
    z-index: 2;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
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
  .ff-stickboard__mark {
    color: var(--ff-muted);
    font-size: var(--ff-type-small);
  }
  .ff-stickboard--drop {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
    background: var(--ff-hover);
  }
  .ff-stickboard__hint {
    color: var(--ff-muted);
  }
  .ff-stickboard__label {
    color: var(--ff-muted);
    font-size: 10px;
    letter-spacing: 0.08em;
  }
  .ff-stickboard__list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .ff-stickboard__row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
    min-height: 26px;
    color: var(--ff-text);
  }
  .ff-stickboard__name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-stickboard__count {
    color: var(--ff-muted);
    font-variant-numeric: tabular-nums;
  }
  .ff-stickboard__x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--ff-muted);
    cursor: pointer;
  }
  .ff-stickboard__x:hover {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
</style>
