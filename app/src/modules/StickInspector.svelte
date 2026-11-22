<script>
  import '$ds/components/detail-pane.css';
  import '$ds/components/inspector.css';
  import { Button, Icon } from '$ds';
  import { library, ejectVolume } from '../stores/library.svelte.ts';
  import { closeDetailPane } from '../stores/ui.svelte.ts';
  import { sticks, stickName, stickAllTrackIds, stickPlaylistIds, stickPlaylistTrackIds, stickSpace } from '../stores/sticks.svelte.ts';
  import {
    stickSync,
    stickPending,
    statusOfStick,
    syncStick,
    isStickSyncBusy,
    dismissStickSync,
    formatBytes,
  } from '../stores/stick-sync.svelte.ts';
  import { syncAge } from '../lib/stickDrop.ts';
  import StickDot from './StickDot.svelte';

  let { volumePath } = $props();

  const name = $derived(stickName(volumePath));
  const space = $derived(stickSpace(sticks.capacity[volumePath]));
  const playlistCount = $derived(stickPlaylistIds(volumePath).length);
  const trackCount = $derived.by(() => {
    const ids = new Set(stickAllTrackIds(volumePath));
    for (const id of stickPlaylistIds(volumePath)) {
      const playlist = library.playlists.find((p) => p.id === id);
      if (playlist) stickPlaylistTrackIds(volumePath, id, playlist.track_ids).forEach((t) => ids.add(t));
    }
    return ids.size;
  });
  const status = $derived(statusOfStick(volumePath));
  const pending = $derived(stickPending.byPath[volumePath]);
  const mine = $derived(stickSync.volumePath === volumePath);
  const busy = $derived(mine && isStickSyncBusy());
  const finished = $derived(mine && stickSync.stage === 'done');
  const lastSync = $derived(library.activeVolume === volumePath ? library.usbLastSync : null);
  const progress = $derived(mine && stickSync.stage === 'writing' ? stickSync.progress : null);
  const progressPct = $derived(progress && progress.total > 0 ? (progress.done / progress.total) * 100 : null);

  const headline = $derived.by(() => {
    if (status.kind === 'syncing') return progress && progress.total > 0 ? `Syncing ${progress.done} of ${progress.total}` : 'Syncing';
    if (status.kind === 'error') return 'Sync failed';
    if (status.kind === 'wont-fit') return "Won't fit";
    if (status.kind === 'pending') return status.label;
    return finished ? 'Synced. Safe to eject.' : 'In sync';
  });

  const detail = $derived.by(() => {
    if (status.kind === 'syncing') return progress?.message ?? 'Checking what changes';
    if (status.kind === 'error') return stickSync.error || 'The sync stopped. Check the stick, then sync again.';
    if (status.kind === 'wont-fit' && pending) {
      return `Needs ${formatBytes(pending.bytesNeeded)}, ${formatBytes(pending.freeBytes ?? 0)} free. Take a playlist off the stick or free up space.`;
    }
    if (status.kind === 'pending' && pending) {
      const parts = [];
      if (pending.adds > 0) parts.push(`${pending.adds} to add`);
      if (pending.removes > 0) parts.push(`${pending.removes} to remove`);
      const other = pending.changes - pending.adds - pending.removes;
      if (other > 0) parts.push(`${other} to update`);
      return `${parts.join(', ')}. Written on the next sync.`;
    }
    if (finished && stickSync.report?.report) {
      const r = stickSync.report.report;
      const parts = [`${r.tracks_added} added`];
      if (r.tracks_removed > 0) parts.push(`${r.tracks_removed} removed`);
      if (r.tracks_updated + r.tracks_replaced > 0) parts.push(`${r.tracks_updated + r.tracks_replaced} updated`);
      return parts.join(', ');
    }
    if (trackCount === 0) return 'Drag tracks or a playlist onto this stick.';
    return lastSync ? `Last synced ${syncAge(lastSync.at)}` : 'Nothing waiting.';
  });
</script>

<div class="ff-stickinspector">
  <div class="ff-detail-pane ff-inspector ff-inspector--closable" data-stick-inspector={volumePath}>
    <button type="button" class="ff-inspector__close" aria-label="Close inspector" title="Close inspector" onclick={closeDetailPane}>
      <Icon name="x" size={14} />
    </button>

    <div class="ff-inspector__id">
      <div class="ff-inspector__cover ff-stickinspector__glyph"><Icon name="usb" size={28} /></div>
      <div class="ff-inspector__idf">
        <h2 class="ff-inspector__title" title={name}>{name}</h2>
        <p class="ff-inspector__artist">USB stick{space ? ` · ${space.total}` : ''}</p>
      </div>
    </div>

    <div class="ff-inspector__figs">
      <div class="ff-inspector__fig" data-fig="playlists">
        <span class="ff-inspector__fig-label">PLAYLISTS</span>
        <div class="ff-inspector__fig-val"><span class="ff-inspector__fig-static">{playlistCount}</span></div>
      </div>
      <div class="ff-inspector__fig" data-fig="tracks">
        <span class="ff-inspector__fig-label">TRACKS</span>
        <div class="ff-inspector__fig-val"><span class="ff-inspector__fig-static">{trackCount}</span></div>
      </div>
      <div class="ff-inspector__fig" data-fig="free">
        <span class="ff-inspector__fig-label">FREE</span>
        <div class="ff-inspector__fig-val"><span class="ff-inspector__fig-static">{space ? space.free : '-'}</span></div>
      </div>
    </div>

    <div class="ff-stickinspector__storage" data-stick-capacity>
      {#if space}
        <span class="ff-stickinspector__bar" aria-hidden="true"><i style:width="{space.usedPct}%"></i></span>
        <span class="ff-stickinspector__nums">{space.used} used of {space.total}</span>
      {:else}
        <span class="ff-stickinspector__nums">Capacity unavailable</span>
      {/if}
    </div>

    <section class="ff-stickinspector__sync" data-stick-status={status.kind}>
      <div class="ff-inspector__cues-head"><span>Sync</span></div>
      <div class="ff-stickinspector__state" role="status">
        <p class="ff-stickinspector__headline">
          <StickDot tone={status.tone} label={status.label} />
          <span class="ff-stickinspector__headline-text">{headline}</span>
          {#if mine && !busy && (stickSync.stage === 'done' || stickSync.stage === 'failed')}
            <button type="button" class="ff-stickinspector__dismiss" aria-label="Dismiss" title="Dismiss" onclick={dismissStickSync}>
              <Icon name="x" size={11} />
            </button>
          {/if}
        </p>
        {#if status.kind === 'syncing'}
          <span class="ff-stickinspector__bar ff-stickinspector__bar--progress" data-stick-progress aria-hidden="true">
            <i class:ff-stickinspector__bar-wait={progressPct === null} style:width={progressPct === null ? null : `${progressPct}%`}></i>
          </span>
        {/if}
        <p class="ff-stickinspector__detail" class:ff-stickinspector__detail--problem={status.tone === 'problem'} data-stick-detail>{detail}</p>
      </div>
      <div class="ff-stickinspector__actions">
        <Button
          label={busy ? 'Syncing' : 'Sync now'}
          size="small"
          variant={status.kind === 'pending' || status.kind === 'error' ? 'primary' : 'default'}
          disabled={trackCount === 0 || isStickSyncBusy()}
          title={trackCount === 0 ? 'Put tracks or a playlist on this stick first' : `Write the changes to ${name}`}
          onclick={() => void syncStick(volumePath)}
        />
        <Button icon="eject" label="Eject" size="small" disabled={busy} title="Eject {name}" onclick={() => void ejectVolume(volumePath)} />
      </div>
    </section>
  </div>
</div>

<style>
  .ff-stickinspector {
    width: 100%;
    height: 100%;
    background: var(--ff-surface);
    border-left: 1px solid var(--ff-border);
    overflow: hidden;
  }
  .ff-stickinspector > :global(.ff-detail-pane) {
    width: 100%;
    height: 100%;
    min-height: 0;
    border-left: none;
  }
  .ff-stickinspector__glyph {
    width: 56px;
    height: 56px;
  }
  .ff-stickinspector__storage {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-3);
    padding: 0 2px;
  }
  .ff-stickinspector__bar {
    position: relative;
    display: block;
    height: 4px;
    border-radius: 2px;
    background: var(--ff-elev-hi);
    overflow: hidden;
  }
  .ff-stickinspector__bar i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--ff-text-mid);
  }
  .ff-stickinspector__bar--progress i {
    background: var(--ff-status-warn);
    transition: width 0.2s ease;
  }
  .ff-stickinspector__bar-wait {
    width: 30%;
    animation: ff-stickinspector-wait 1.2s ease-in-out infinite alternate;
  }
  @keyframes ff-stickinspector-wait {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(233%);
    }
  }
  .ff-stickinspector__nums {
    font-size: var(--ff-type-small);
    color: var(--ff-muted);
    font-variant-numeric: tabular-nums;
  }
  .ff-stickinspector__sync {
    display: flex;
    flex-direction: column;
    margin-top: var(--ff-space-4);
  }
  .ff-stickinspector__state {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-3);
    padding: 0 2px;
  }
  .ff-stickinspector__headline {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 20px;
    margin: 0;
    font-size: var(--ff-type-body);
    font-weight: 500;
    color: var(--ff-text);
  }
  .ff-stickinspector__headline-text {
    flex: 1;
    min-width: 0;
  }
  .ff-stickinspector__detail {
    margin: 0;
    font-size: var(--ff-type-caption);
    line-height: 1.45;
    color: var(--ff-text-mid);
    overflow-wrap: anywhere;
  }
  .ff-stickinspector__detail--problem {
    color: var(--ff-danger);
  }
  .ff-stickinspector__dismiss {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    border-radius: var(--ff-radius-md);
    background: transparent;
    color: var(--ff-muted);
    cursor: pointer;
  }
  .ff-stickinspector__dismiss:hover {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-stickinspector__actions {
    display: flex;
    gap: var(--ff-space-3);
    margin-top: var(--ff-space-5);
    padding: 0 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .ff-stickinspector__bar-wait {
      animation: none;
    }
  }
</style>
