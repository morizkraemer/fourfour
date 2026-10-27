<script>
  import { Button, Icon } from '$ds';
  import { library } from '../stores/library.svelte.ts';
  import { dnd } from '../stores/dnd.svelte.ts';
  import { dropFlash, cardKey } from '../stores/organize-drop.svelte.ts';
  import {
    sticks,
    stickAllTrackIds,
    stickPlaylistIds,
    removePlaylistFromStick,
    capacityLabel,
  } from '../stores/sticks.svelte.ts';
  import { stickSync, syncStick, isStickSyncBusy, syncStatusLine, dismissStickSync } from '../stores/stick-sync.svelte.ts';

  let { volumePath } = $props();

  const allCount = $derived(stickAllTrackIds(volumePath).length);
  const playlists = $derived(
    stickPlaylistIds(volumePath).map((id) => library.playlists.find((p) => p.id === id)).filter(Boolean)
  );
  const capacity = $derived(sticks.capacity[volumePath] ?? null);
  const usedPct = $derived(
    capacity && capacity.total_bytes > 0
      ? Math.min(100, ((capacity.total_bytes - capacity.free_bytes) / capacity.total_bytes) * 100)
      : null
  );
  const mine = $derived(stickSync.volumePath === volumePath);
  const busy = $derived(mine && isStickSyncBusy());
  const status = $derived(mine ? syncStatusLine() : '');
  const progressPct = $derived(
    mine && stickSync.stage === 'writing' && stickSync.progress && stickSync.progress.total > 0
      ? (stickSync.progress.done / stickSync.progress.total) * 100
      : null
  );
  const empty = $derived(allCount === 0 && playlists.length === 0);
  const dropHover = $derived(dnd.kind === 'playlist' && dnd.dropStickPath === volumePath);
  const flash = $derived(dropFlash.key === cardKey({ kind: 'stick', volumePath }) ? dropFlash.text : '');
</script>

<section
  class="ff-stickboard"
  class:ff-stickboard--drop={dropHover}
  data-stick-board
  data-drop-stick-path={volumePath}
  aria-label="On this stick"
>
  <div class="ff-stickboard__bar">
    <div class="ff-stickboard__capacity" data-stick-capacity>
      {#if usedPct !== null}
        <span class="ff-stickboard__meter" aria-hidden="true"><i style:width="{usedPct}%"></i></span>
        <span>{capacityLabel(capacity)}</span>
      {:else}
        <span class="ff-stickboard__muted">Capacity unavailable</span>
      {/if}
    </div>
    <Button
      label={busy ? 'Syncing…' : 'Sync'}
      icon="usb"
      size="small"
      variant="primary"
      disabled={busy || empty || isStickSyncBusy()}
      title={empty ? 'Put tracks or a playlist on this stick first' : 'Write the stick'}
      onclick={() => void syncStick(volumePath)}
    />
  </div>

  {#if status}
    <div class="ff-stickboard__status" role="status" data-stick-status>
      {#if progressPct !== null}
        <span class="ff-stickboard__meter ff-stickboard__meter--progress" aria-hidden="true"><i style:width="{progressPct}%"></i></span>
      {/if}
      <span class="ff-stickboard__status-text" class:ff-stickboard__status-text--error={stickSync.stage === 'failed'}>{status}</span>
      {#if !busy && (stickSync.stage === 'done' || stickSync.stage === 'failed')}
        <button type="button" class="ff-stickboard__x" aria-label="Dismiss" onclick={dismissStickSync}>
          <Icon name="x" size={12} />
        </button>
      {/if}
    </div>
  {/if}

  <div class="ff-stickboard__label">ON THIS STICK</div>
  <ul class="ff-stickboard__list" data-stick-list>
    <li class="ff-stickboard__row" data-stick-row="all">
      <Icon name="list" size={13} />
      <span class="ff-stickboard__name">All tracks</span>
      <span class="ff-stickboard__count">{allCount}</span>
    </li>
    {#each playlists as pl (pl.id)}
      <li class="ff-stickboard__row" data-stick-row="playlist" data-playlist-id={pl.id}>
        <Icon name="list" size={13} />
        <span class="ff-stickboard__name">{pl.name}</span>
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
</section>

<style>
  .ff-stickboard {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-3);
    padding: var(--ff-space-4) var(--ff-space-5);
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-stickboard--drop {
    outline: 1px solid var(--ff-accent);
    outline-offset: -1px;
    background: var(--ff-hover);
  }
  .ff-stickboard__bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--ff-space-4);
  }
  .ff-stickboard__capacity {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
    color: var(--ff-text-mid);
    font-variant-numeric: tabular-nums;
  }
  .ff-stickboard__meter {
    position: relative;
    width: 64px;
    height: 4px;
    border-radius: 2px;
    background: var(--ff-elev-hi);
    overflow: hidden;
  }
  .ff-stickboard__meter i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--ff-text-mid);
  }
  .ff-stickboard__meter--progress {
    width: 100%;
    flex: 1;
  }
  .ff-stickboard__meter--progress i {
    background: var(--ff-accent);
  }
  .ff-stickboard__muted,
  .ff-stickboard__hint {
    color: var(--ff-muted);
  }
  .ff-stickboard__status {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
    color: var(--ff-text-mid);
  }
  .ff-stickboard__status-text {
    flex: none;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-stickboard__status-text--error {
    color: var(--ff-danger);
    white-space: normal;
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
