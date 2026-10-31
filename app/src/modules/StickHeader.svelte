<script>
  import { Button, StatusDot } from '$ds';
  import { library, ejectVolume } from '../stores/library.svelte.ts';
  import { sticks, stickName, stickAllTrackIds, stickPlaylistIds, stickSpace } from '../stores/sticks.svelte.ts';
  import { stickSync, syncStick, isStickSyncBusy, syncStatusLine, dismissStickSync } from '../stores/stick-sync.svelte.ts';
  import { syncAge } from '../lib/stickDrop.ts';

  let { volumePath, showName = true } = $props();

  const name = $derived(stickName(volumePath));
  const space = $derived(stickSpace(sticks.capacity[volumePath]));
  const playlistCount = $derived(stickPlaylistIds(volumePath).length);
  const trackCount = $derived.by(() => {
    const ids = new Set(stickAllTrackIds(volumePath));
    for (const id of stickPlaylistIds(volumePath)) {
      library.playlists.find((p) => p.id === id)?.track_ids.forEach((t) => ids.add(t));
    }
    return ids.size;
  });
  const mine = $derived(stickSync.volumePath === volumePath && stickSync.stage !== 'idle' && stickSync.stage !== 'confirm');
  const busy = $derived(mine && isStickSyncBusy());
  const empty = $derived(trackCount === 0);
  const lastSync = $derived(library.activeVolume === volumePath ? library.usbLastSync : null);
  const status = $derived(mine ? syncStatusLine() : lastSync ? `Synced ${syncAge(lastSync.at)}` : '');
  const failed = $derived(mine && stickSync.stage === 'failed');
  const progressPct = $derived(
    mine && stickSync.stage === 'writing' && stickSync.progress && stickSync.progress.total > 0
      ? (stickSync.progress.done / stickSync.progress.total) * 100
      : null
  );
</script>

<header class="ff-stickhead" data-stick-header={volumePath}>
  <div class="ff-stickhead__grid">
    <div class="ff-stickhead__id">
      {#if showName}
        <StatusDot status="online" />
        <span class="ff-stickhead__name">{name}</span>
      {/if}
      {#if space}<span class="ff-stickhead__size">{space.total}</span>{/if}
      <span class="ff-stickhead__counts">
        {playlistCount} {playlistCount === 1 ? 'playlist' : 'playlists'} · {trackCount} {trackCount === 1 ? 'track' : 'tracks'}
      </span>
    </div>
    <div class="ff-stickhead__storage" data-stick-capacity>
      {#if space}
        <span class="ff-stickhead__bar" aria-hidden="true">
          {#if progressPct !== null}
            <i class="ff-stickhead__bar-progress" style:width="{progressPct}%"></i>
          {:else}
            <i style:width="{space.usedPct}%"></i>
          {/if}
        </span>
        <span class="ff-stickhead__nums">
          <span class="ff-stickhead__used">{`${space.used} used · `}</span><span>{space.free} free</span>
        </span>
      {:else}
        <span class="ff-stickhead__muted">Capacity unavailable</span>
      {/if}
    </div>
    <div
      class="ff-stickhead__status"
      class:ff-stickhead__status--error={failed}
      role="status"
      data-stick-status
      title={status}
    >
      <span class="ff-stickhead__status-text">{status}</span>
      {#if mine && !busy && (stickSync.stage === 'done' || stickSync.stage === 'failed')}
        <button type="button" class="ff-stickhead__x" aria-label="Dismiss" onclick={dismissStickSync}>×</button>
      {/if}
    </div>
    <div class="ff-stickhead__actions">
      <Button
        label={busy ? 'Syncing…' : 'Sync'}
        size="small"
        variant="primary"
        disabled={busy || empty || isStickSyncBusy()}
        title={empty ? 'Put tracks or a playlist on this stick first' : `Write ${name}`}
        onclick={() => void syncStick(volumePath)}
      />
      <Button
        icon="eject"
        label="Eject"
        size="small"
        disabled={busy}
        title="Eject {name}"
        onclick={() => void ejectVolume(volumePath)}
      />
    </div>
  </div>
</header>

<style>
  .ff-stickhead {
    container-type: inline-size;
    border-bottom: 1px solid var(--ff-border);
    font-size: var(--ff-type-caption);
    color: var(--ff-text-mid);
  }
  .ff-stickhead__grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    grid-template-areas:
      'id status actions'
      'storage storage storage';
    align-items: center;
    column-gap: var(--ff-space-5);
    row-gap: var(--ff-space-3);
    padding: var(--ff-space-4) var(--ff-space-6);
  }
  .ff-stickhead__id {
    grid-area: id;
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-width: 0;
    white-space: nowrap;
  }
  .ff-stickhead__name {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ff-text);
    font-size: var(--ff-type-h2);
    font-weight: 600;
  }
  .ff-stickhead__size {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-num);
    color: var(--ff-muted);
  }
  .ff-stickhead__counts {
    color: var(--ff-muted);
  }
  .ff-stickhead__storage {
    grid-area: storage;
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-width: 0;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .ff-stickhead__bar {
    position: relative;
    flex: none;
    width: 120px;
    height: 4px;
    border-radius: 2px;
    background: var(--ff-elev-hi);
    overflow: hidden;
  }
  .ff-stickhead__bar i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--ff-text-mid);
  }
  .ff-stickhead__bar .ff-stickhead__bar-progress {
    background: var(--ff-status-online);
  }
  .ff-stickhead__status {
    grid-area: status;
    display: flex;
    align-items: center;
    gap: var(--ff-space-2);
    min-width: 0;
    justify-content: flex-end;
  }
  .ff-stickhead__status-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-stickhead__status--error {
    color: var(--ff-danger);
  }
  .ff-stickhead__status--error .ff-stickhead__status-text {
    white-space: normal;
  }
  .ff-stickhead__actions {
    grid-area: actions;
    display: flex;
    gap: var(--ff-space-3);
  }
  .ff-stickhead__muted {
    color: var(--ff-muted);
  }
  .ff-stickhead__x {
    border: none;
    background: transparent;
    color: var(--ff-muted);
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    padding: 0 var(--ff-space-2);
  }
  .ff-stickhead__x:hover {
    color: var(--ff-text);
  }
  .ff-stickhead__size,
  .ff-stickhead__counts,
  .ff-stickhead__used {
    display: none;
  }

  @container (min-width: 380px) {
    .ff-stickhead__grid {
      grid-template-columns: auto minmax(0, 1fr) auto;
      grid-template-areas:
        'id id actions'
        'storage status status';
    }
    .ff-stickhead__size,
    .ff-stickhead__used {
      display: inline;
    }
  }

  @container (min-width: 700px) {
    .ff-stickhead__grid {
      grid-template-columns: auto auto minmax(0, 1fr) auto;
      grid-template-areas: 'id storage status actions';
      min-height: 52px;
      padding: var(--ff-space-3) var(--ff-space-8);
    }
    .ff-stickhead__size,
    .ff-stickhead__used {
      display: none;
    }
    .ff-stickhead__storage {
      margin-left: var(--ff-space-6);
    }
    .ff-stickhead__bar {
      width: 110px;
    }
  }

  @container (min-width: 1000px) {
    .ff-stickhead__size,
    .ff-stickhead__counts,
    .ff-stickhead__used {
      display: inline;
    }
    .ff-stickhead__bar {
      width: 200px;
    }
  }
</style>
