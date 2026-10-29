<script>
  import { Button } from '$ds';
  import { library } from '../stores/library.svelte.ts';
  import { fileStatus, dismissBlockedLoad } from '../stores/file-status.svelte.ts';
  import { locateTrack } from '../stores/file-health.svelte.ts';

  let trackId = $derived(fileStatus.blockedTrackId);
  let track = $derived(trackId == null ? null : library.tracks.find((t) => t.id === trackId));
  let reason = $derived(trackId != null && fileStatus.byId[trackId] === 'error' ? "can't be read" : 'is missing');

  function locate() {
    const id = trackId;
    dismissBlockedLoad();
    if (id != null) void locateTrack(id);
  }
</script>

{#if trackId != null}
  <div class="ff-file-tip" role="status" data-file-blocked-tip>
    <span class="ff-file-tip__text">“{track?.title ?? `Track ${trackId}`}” {reason}. It can't be loaded.</span>
    <Button label="Locate…" size="small" onclick={locate} />
  </div>
{/if}

<style>
  .ff-file-tip {
    position: fixed;
    left: 50%;
    bottom: var(--ff-space-10);
    transform: translateX(-50%);
    z-index: 10040;
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    max-width: min(560px, calc(100vw - var(--ff-space-9)));
    padding: var(--ff-space-4) var(--ff-space-4) var(--ff-space-4) var(--ff-space-7);
    background: var(--ff-elev);
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-xl);
    box-shadow: var(--ff-shadow-menu);
    font-size: var(--ff-type-caption);
    color: var(--ff-text-mid);
  }
  .ff-file-tip__text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
