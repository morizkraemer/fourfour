<script>
  import { Dialog, Button, SegmentedControl } from '$ds';
  import '$ds/components/checkbox.css';
  import {
    duplicates,
    closeDuplicates,
    showTab,
    chooseKeeper,
    toggleSkip,
    mergeGroup,
    mergeAllExact,
    restoreMerge,
  } from '../stores/duplicate-cleanup.svelte.ts';
  import {
    archiveCopies,
    archiveDate,
    archiveHeading,
    confidenceLabel,
    exactCount,
    groupHeading,
    groupsTitle,
    keeperOf,
    reasonOf,
  } from '../lib/duplicateCleanup.ts';
  import { trackTime } from '../lib/importDuplicates.ts';

  let exactLeft = $derived(exactCount(duplicates.groups, duplicates.skipped));
  let canMergeAll = $derived(duplicates.tab === 'groups' && exactLeft > 0);
  let tabs = $derived(['Duplicates', duplicates.archive.length > 0 ? `Archive (${duplicates.archive.length})` : 'Archive']);
</script>

<Dialog open={duplicates.open} variant="wide" title="Duplicates" onClose={closeDuplicates}>
  {#snippet body()}
    <div class="ff-dedupe" data-duplicates-view>
      <SegmentedControl items={tabs} activeIndex={duplicates.tab === 'groups' ? 0 : 1} onChange={(i) => showTab(i === 0 ? 'groups' : 'archive')} />
      {#if duplicates.error}
        <p class="ff-dedupe__error" data-duplicates-error>{duplicates.error}</p>
      {/if}
      {#if duplicates.message}
        <p class="ff-dedupe__message" data-duplicates-message>{duplicates.message}</p>
      {/if}

      {#if duplicates.tab === 'groups'}
        {#if duplicates.loading}
          <p class="ff-dedupe__note">Looking for copies…</p>
        {:else if duplicates.groups.length === 0}
          <p class="ff-dedupe__note" data-duplicates-empty>No copies found. Nothing to clean up.</p>
        {:else}
          <p class="ff-dedupe__note">
            {groupsTitle(duplicates.groups.length)}. Pick the copy to keep. Merging moves playlists, cues and ratings to it; no file is deleted.
          </p>
          <div class="ff-dedupe__table">
            <div class="ff-dedupe__cols ff-dedupe__cols--head" aria-hidden="true">
              <span>Keep</span><span>Format</span><span class="ff-dedupe__num">Bitrate</span><span class="ff-dedupe__num">Time</span><span class="ff-dedupe__num">Playlists</span><span class="ff-dedupe__num">Cues</span><span>Location</span>
            </div>
            <div class="ff-dedupe__scroll">
              {#each duplicates.groups as group (group.key)}
                {@const keeperId = keeperOf(group, duplicates.choices)}
                {@const skipped = duplicates.skipped.has(group.key)}
                <section class="ff-dedupe__group" class:ff-dedupe__group--skipped={skipped} data-duplicate-group={group.key} data-confidence={group.confidence}>
                  <header class="ff-dedupe__head">
                    <div class="ff-dedupe__texts">
                      <span class="ff-dedupe__title">{groupHeading(group)}</span>
                      <span class="ff-dedupe__sub">
                        {skipped ? 'Skipped' : confidenceLabel(group)} · <span data-duplicate-reason>Keeper: {reasonOf(group, duplicates.choices)}</span>
                      </span>
                    </div>
                    <Button label={skipped ? 'Undo skip' : 'Skip'} size="small" variant="ghost" disabled={duplicates.busy} onclick={() => toggleSkip(group)} />
                    <Button label="Merge" size="small" disabled={skipped || duplicates.busy} onclick={() => void mergeGroup(group)} />
                  </header>
                  <div role="radiogroup" aria-label={`Copy of ${groupHeading(group)} to keep`}>
                    {#each group.members as member (member.id)}
                      {@const keeper = member.id === keeperId}
                      <button
                        type="button"
                        class="ff-dedupe__cols ff-dedupe__member"
                        class:ff-dedupe__member--keeper={keeper}
                        role="radio"
                        aria-checked={keeper}
                        disabled={skipped || duplicates.busy}
                        data-duplicate-member={member.id}
                        title={member.path}
                        onclick={() => chooseKeeper(group, member.id)}
                      >
                        <span class="ff-radio" class:ff-radio--on={keeper} aria-hidden="true">
                          <span class="ff-radio__circle"><span class="ff-radio__dot"></span></span>
                        </span>
                        <span class="ff-dedupe__format">{member.format || 'file'}</span>
                        <span class="ff-dedupe__num">{member.bitrate > 0 ? `${member.bitrate} kbps` : '—'}</span>
                        <span class="ff-dedupe__num">{trackTime(member.duration_secs)}</span>
                        <span class="ff-dedupe__num">{member.playlists}</span>
                        <span class="ff-dedupe__num">{member.cues}</span>
                        <span class="ff-dedupe__path">{member.file_present ? member.path : `File missing: ${member.path}`}</span>
                      </button>
                    {/each}
                  </div>
                </section>
              {/each}
            </div>
          </div>
        {/if}
      {:else if duplicates.archive.length === 0}
        <p class="ff-dedupe__note" data-archive-empty>Nothing merged yet.</p>
      {:else}
        <p class="ff-dedupe__note">Merged copies are out of your library, but their files are still on disk. Restore puts them back with their playlists and cues.</p>
        <div class="ff-dedupe__table">
          <div class="ff-dedupe__archived ff-dedupe__archived--head" aria-hidden="true">
            <span>Song</span><span>Merged</span><span>Copies</span><span></span>
          </div>
          <div class="ff-dedupe__scroll">
            {#each duplicates.archive as merge (merge.merge_id)}
              <div class="ff-dedupe__archived" data-archived-merge={merge.merge_id} title={merge.tracks.map((t) => t.path).join('\n')}>
                <span class="ff-dedupe__title">{archiveHeading(merge)}</span>
                <span class="ff-dedupe__num-text">{archiveDate(merge)}</span>
                <span class="ff-dedupe__num-text">{archiveCopies(merge)}</span>
                <Button label="Restore" size="small" disabled={duplicates.busy} onclick={() => void restoreMerge(merge.merge_id)} />
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/snippet}
  {#snippet actions()}
    <Button label="Done" variant={canMergeAll ? 'ghost' : 'primary'} onclick={closeDuplicates} />
    {#if canMergeAll}
      <Button label={exactLeft === 1 ? 'Merge 1 exact match' : `Merge ${exactLeft} exact matches`} variant="primary" disabled={duplicates.busy} onclick={() => void mergeAllExact()} />
    {/if}
  {/snippet}
</Dialog>

<style>
  .ff-dedupe {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-5);
  }
  .ff-dedupe p {
    margin: 0;
  }
  .ff-dedupe__note {
    font-size: var(--ff-type-compact);
    line-height: 1.45;
    color: var(--ff-text-mid);
  }
  .ff-dedupe__message {
    color: var(--ff-text);
  }
  .ff-dedupe__error {
    color: var(--ff-danger, var(--ff-text));
  }
  .ff-dedupe__table {
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
    overflow: hidden;
  }
  .ff-dedupe__scroll {
    max-height: 52vh;
    overflow-y: auto;
  }
  .ff-dedupe__cols {
    display: grid;
    grid-template-columns: 32px 56px 76px 44px 64px 40px minmax(0, 1fr);
    align-items: center;
    gap: var(--ff-space-4);
    padding: 0 var(--ff-space-4);
  }
  .ff-dedupe__cols--head,
  .ff-dedupe__archived--head {
    height: 28px;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ff-muted);
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-dedupe__group + .ff-dedupe__group {
    border-top: 1px solid var(--ff-border-hi);
  }
  .ff-dedupe__head {
    display: flex;
    align-items: center;
    gap: var(--ff-space-2);
    padding: var(--ff-space-4) var(--ff-space-4) var(--ff-space-3);
  }
  .ff-dedupe__texts {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-1);
  }
  .ff-dedupe__title {
    min-width: 0;
    font-size: var(--ff-type-body);
    font-weight: 500;
    color: var(--ff-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__sub {
    font-size: var(--ff-type-caption);
    color: var(--ff-text-mid);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__member {
    width: 100%;
    height: 30px;
    border: none;
    border-top: 1px solid var(--ff-border);
    background: none;
    font: inherit;
    font-size: var(--ff-type-compact);
    color: var(--ff-text-mid);
    text-align: left;
    cursor: pointer;
  }
  .ff-dedupe__member:not(:disabled):hover {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-dedupe__member:focus-visible {
    outline: 1px solid var(--ff-border-hover);
    outline-offset: -1px;
  }
  .ff-dedupe__member--keeper,
  .ff-dedupe__member--keeper:not(:disabled):hover {
    background: var(--ff-select);
    color: var(--ff-text);
    cursor: default;
  }
  .ff-dedupe__member:disabled {
    cursor: default;
  }
  .ff-dedupe__group--skipped .ff-dedupe__member {
    opacity: 0.45;
  }
  .ff-dedupe__member .ff-radio {
    justify-self: center;
  }
  .ff-dedupe__format {
    text-transform: uppercase;
  }
  .ff-dedupe__num {
    text-align: right;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-num);
  }
  .ff-dedupe__cols--head .ff-dedupe__num {
    font-size: var(--ff-type-label);
  }
  .ff-dedupe__path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ff-text-mid);
  }
  .ff-dedupe__member--keeper .ff-dedupe__path {
    color: var(--ff-text);
  }
  .ff-dedupe__archived {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 88px 64px 72px;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 40px;
    padding: 0 var(--ff-space-4);
    font-size: var(--ff-type-compact);
  }
  .ff-dedupe__archived--head {
    min-height: 28px;
    font-size: var(--ff-type-label);
  }
  .ff-dedupe__archived :global(.ff-btn) {
    justify-self: end;
  }
  .ff-dedupe__scroll .ff-dedupe__archived + .ff-dedupe__archived {
    border-top: 1px solid var(--ff-border);
  }
  .ff-dedupe__num-text {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-num);
    color: var(--ff-text-mid);
  }
</style>
