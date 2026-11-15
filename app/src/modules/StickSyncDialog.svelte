<script>
  import { Dialog, Button } from '$ds';
  import { stickSync, confirmStickSync, cancelStickSync, formatBytes } from '../stores/stick-sync.svelte.ts';
  import { analyzeTrackIds } from '../stores/library.svelte.ts';

  const MAX_NAMES = 5;

  const plan = $derived(stickSync.stage === 'confirm' ? stickSync.plan : null);
  const wontFit = $derived(plan?.fits === false);

  function names(tracks) {
    const shown = tracks.slice(0, MAX_NAMES).map((t) => (t.artist ? `${t.artist} – ${t.title}` : t.title));
    const more = tracks.length - shown.length;
    return more > 0 ? [...shown, `and ${more} more`] : shown;
  }

  function countText(n, one, many) {
    return `${n} ${n === 1 ? one : many}`;
  }

  async function analyzeFirst() {
    const ids = plan?.skipped_unanalyzed.map((t) => t.track_id).filter((id) => id != null) ?? [];
    cancelStickSync();
    if (ids.length > 0) await analyzeTrackIds(ids);
  }
</script>

<Dialog
  open={plan !== null}
  title={wontFit ? `Won't fit on ${plan?.volume}` : `Sync ${plan?.volume}?`}
  onClose={cancelStickSync}
>
  {#snippet body()}
    {#if plan}
      <div class="ff-stick-plan" data-stick-plan>
        {#if wontFit}
          <p data-plan-wontfit>
            Needs {formatBytes(plan.bytes_needed)}, {formatBytes(plan.free_bytes ?? 0)} free. Take a playlist off the
            stick or free up space. Nothing was written.
          </p>
        {:else}
          <p class="ff-stick-plan__summary">
            {countText(plan.add.length, 'track', 'tracks')} to add · {plan.unchanged_count + plan.update_count} already there
          </p>
        {/if}
        {#if plan.remove.length > 0}
          <section data-plan-remove>
            <h3>{countText(plan.remove.length, 'track comes', 'tracks come')} off the stick</h3>
            <ul>{#each names(plan.remove) as name}<li>{name}</li>{/each}</ul>
          </section>
        {/if}
        {#if plan.foreign_kept_count > 0}
          <section data-plan-foreign>
            <h3>{countText(plan.foreign_kept_count, 'track', 'tracks')} put there by other apps</h3>
            <p>They drop out of the stick's library. Their files stay on the stick.</p>
          </section>
        {/if}
        {#if plan.skipped_unanalyzed.length > 0}
          <section data-plan-unanalyzed>
            <h3>{countText(plan.skipped_unanalyzed.length, 'track is', 'tracks are')} not analyzed and won't be written</h3>
            <ul>{#each names(plan.skipped_unanalyzed) as name}<li>{name}</li>{/each}</ul>
          </section>
        {/if}
      </div>
    {/if}
  {/snippet}
  {#snippet actions()}
    {#if wontFit}
      <Button label="OK" variant="primary" onclick={cancelStickSync} />
    {:else}
      <Button label="Cancel" variant="ghost" onclick={cancelStickSync} />
      {#if (plan?.skipped_unanalyzed.length ?? 0) > 0}
        <Button label="Analyze first" onclick={() => void analyzeFirst()} />
      {/if}
      <Button label="Sync" variant="primary" onclick={() => void confirmStickSync()} />
    {/if}
  {/snippet}
</Dialog>

<style>
  .ff-stick-plan {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }
  .ff-stick-plan p,
  .ff-stick-plan ul {
    margin: 0;
  }
  .ff-stick-plan h3 {
    margin: 0 0 var(--ff-space-2);
    font-size: inherit;
    font-weight: 600;
  }
  .ff-stick-plan ul {
    padding-left: var(--ff-space-6);
    color: var(--ff-text-mid);
  }
</style>
