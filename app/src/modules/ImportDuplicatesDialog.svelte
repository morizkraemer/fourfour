<script>
  import { Dialog, Button } from '$ds';
  import { duplicatesPrompt, answerDuplicates } from '../stores/import-duplicates.svelte.ts';
  import { duplicatesTitle, trackTime } from '../lib/importDuplicates.ts';

  let rows = $derived(duplicatesPrompt.rows);
  let scrollEl = $state(null);
  let overflowing = $state(false);

  function measure() {
    overflowing = !!scrollEl && scrollEl.scrollHeight > scrollEl.clientHeight + 1;
  }

  $effect(() => {
    rows;
    scrollEl;
    measure();
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  });
</script>

<Dialog
  open={duplicatesPrompt.open}
  variant="wide"
  title={duplicatesTitle(rows.length)}
  onClose={() => answerDuplicates(true)}
>
  {#snippet body()}
    <div class="ff-dupes" data-duplicates-dialog>
      <div class="ff-dupes__row ff-dupes__row--head">
        <span>New file</span><span>Your track</span><span>Artist</span><span class="ff-dupes__num">Time</span>
      </div>
      <div class="ff-dupes__scroll" class:ff-dupes__scroll--more={overflowing} bind:this={scrollEl}>
        {#each rows as row (row.new_path)}
          <div class="ff-dupes__row" data-duplicate-row title={row.new_path}>
            <span class="ff-dupes__file">{row.file_name}</span>
            <span class="ff-dupes__title">{row.title}</span>
            <span>{row.artist}</span>
            <span class="ff-dupes__num">{trackTime(row.duration_secs)}</span>
          </div>
        {/each}
      </div>
    </div>
  {/snippet}
  {#snippet actions()}
    <Button label="Import anyway" variant="ghost" onclick={() => answerDuplicates(false)} />
    <Button label="Skip duplicates" variant="primary" onclick={() => answerDuplicates(true)} />
  {/snippet}
</Dialog>

<style>
  .ff-dupes {
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
    overflow: hidden;
  }
  .ff-dupes__scroll {
    max-height: 60vh;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .ff-dupes__scroll::-webkit-scrollbar {
    display: none;
  }
  .ff-dupes__scroll--more {
    -webkit-mask-image: linear-gradient(to bottom, #000 calc(100% - var(--ff-space-8)), transparent);
    mask-image: linear-gradient(to bottom, #000 calc(100% - var(--ff-space-8)), transparent);
  }
  .ff-dupes__row {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr) minmax(0, 1fr) 40px;
    align-items: center;
    gap: var(--ff-space-4);
    height: 28px;
    padding: 0 var(--ff-space-4);
    font-size: var(--ff-type-compact);
    color: var(--ff-text-mid);
  }
  .ff-dupes__scroll .ff-dupes__row + .ff-dupes__row {
    border-top: 1px solid var(--ff-border);
  }
  .ff-dupes__row--head {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ff-muted);
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-dupes__row span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dupes__file {
    color: var(--ff-muted);
  }
  .ff-dupes__title {
    color: var(--ff-text);
  }
  .ff-dupes__num {
    text-align: right;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-num);
  }
</style>
