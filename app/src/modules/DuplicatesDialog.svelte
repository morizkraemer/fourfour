<script>
  import { Dialog, Button, SegmentedControl } from '$ds';
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
    memberDetail,
    reasonOf,
  } from '../lib/duplicateCleanup.ts';

  let exactLeft = $derived(exactCount(duplicates.groups, duplicates.skipped));
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
          <div class="ff-dedupe__intro">
            <p class="ff-dedupe__count">{groupsTitle(duplicates.groups.length)}</p>
            <p class="ff-dedupe__note">Merging keeps one entry. Playlists, cues, ratings, colours and comments move to it. No file is deleted.</p>
          </div>
          <div class="ff-dedupe__list">
            {#each duplicates.groups as group (group.key)}
              {@const keeperId = keeperOf(group, duplicates.choices)}
              {@const skipped = duplicates.skipped.has(group.key)}
              <section class="ff-dedupe__group" class:ff-dedupe__group--skipped={skipped} data-duplicate-group={group.key} data-confidence={group.confidence}>
                <header class="ff-dedupe__head">
                  <span class="ff-dedupe__title">{groupHeading(group)}</span>
                  <span class="ff-dedupe__kind">{skipped ? 'Skipped' : confidenceLabel(group)}</span>
                  <div class="ff-dedupe__actions">
                    <Button label={skipped ? 'Undo skip' : 'Skip'} size="small" variant="ghost" disabled={duplicates.busy} onclick={() => toggleSkip(group)} />
                    <Button label="Merge" size="small" variant="ghost" disabled={skipped || duplicates.busy} onclick={() => void mergeGroup(group)} />
                  </div>
                </header>
                <div class="ff-dedupe__members" role="radiogroup" aria-label="Copy to keep">
                  {#each group.members as member (member.id)}
                    {@const keeper = member.id === keeperId}
                    <button
                      type="button"
                      class="ff-dedupe__member"
                      class:ff-dedupe__member--keeper={keeper}
                      role="radio"
                      aria-checked={keeper}
                      disabled={skipped || duplicates.busy}
                      data-duplicate-member={member.id}
                      title={keeper ? member.path : `Keep this one instead: ${member.path}`}
                      onclick={() => chooseKeeper(group, member.id)}
                    >
                      <span class="ff-dedupe__dot" aria-hidden="true"></span>
                      <span class="ff-dedupe__detail">{memberDetail(member)}</span>
                      <span class="ff-dedupe__path">{member.path}</span>
                      <span class="ff-dedupe__tag">{keeper ? 'Keep' : 'Keep this one'}</span>
                    </button>
                  {/each}
                </div>
                <p class="ff-dedupe__reason" data-duplicate-reason>Keeper: {reasonOf(group, duplicates.choices)}</p>
              </section>
            {/each}
          </div>
        {/if}
      {:else if duplicates.archive.length === 0}
        <p class="ff-dedupe__note" data-archive-empty>Nothing merged yet.</p>
      {:else}
        <p class="ff-dedupe__note">Merged copies are out of your library, but their files are still on disk. Restore puts them back with their playlists and cues.</p>
        <div class="ff-dedupe__list">
          {#each duplicates.archive as merge (merge.merge_id)}
            <div class="ff-dedupe__archived" data-archived-merge={merge.merge_id} title={merge.tracks.map((t) => t.path).join('\n')}>
              <span class="ff-dedupe__title">{archiveHeading(merge)}</span>
              <span class="ff-dedupe__meta">{archiveDate(merge)}</span>
              <span class="ff-dedupe__meta">{archiveCopies(merge)}</span>
              <div class="ff-dedupe__actions">
                <Button label="Restore" size="small" variant="ghost" disabled={duplicates.busy} onclick={() => void restoreMerge(merge.merge_id)} />
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/snippet}
  {#snippet actions()}
    {@const canMergeAll = duplicates.tab === 'groups' && exactLeft > 0}
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
  .ff-dedupe__intro {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-1);
  }
  .ff-dedupe__count {
    font-size: var(--ff-type-h2);
    color: var(--ff-text);
  }
  .ff-dedupe__note {
    font-size: var(--ff-type-compact);
    color: var(--ff-muted);
  }
  .ff-dedupe__message {
    color: var(--ff-text);
  }
  .ff-dedupe__error {
    color: var(--ff-danger, var(--ff-text));
  }
  .ff-dedupe__list {
    display: flex;
    flex-direction: column;
    max-height: 56vh;
    overflow-y: auto;
  }
  .ff-dedupe__group {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-1);
    padding: var(--ff-space-5) 0;
    border-top: 1px solid var(--ff-border);
  }
  .ff-dedupe__group:first-child {
    border-top: none;
    padding-top: 0;
  }
  .ff-dedupe__group--skipped .ff-dedupe__title,
  .ff-dedupe__group--skipped .ff-dedupe__members,
  .ff-dedupe__group--skipped .ff-dedupe__reason {
    opacity: 0.4;
  }
  .ff-dedupe__head,
  .ff-dedupe__archived {
    display: flex;
    align-items: center;
    gap: var(--ff-space-5);
    min-height: 28px;
  }
  .ff-dedupe__title {
    flex: 1 1 0;
    min-width: 0;
    font-size: var(--ff-type-body);
    color: var(--ff-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__kind,
  .ff-dedupe__meta {
    flex: none;
    font-size: var(--ff-type-caption);
    color: var(--ff-muted);
  }
  .ff-dedupe__meta {
    font-variant-numeric: tabular-nums;
  }
  .ff-dedupe__actions {
    flex: none;
    display: flex;
    gap: var(--ff-space-1);
    opacity: 0.5;
    transition: opacity var(--ff-motion-fast) var(--ff-easing);
  }
  .ff-dedupe__group:hover .ff-dedupe__actions,
  .ff-dedupe__group:focus-within .ff-dedupe__actions,
  .ff-dedupe__group--skipped .ff-dedupe__actions,
  .ff-dedupe__archived:hover .ff-dedupe__actions,
  .ff-dedupe__archived:focus-within .ff-dedupe__actions {
    opacity: 1;
  }
  .ff-dedupe__archived {
    padding: var(--ff-space-3) 0;
    border-top: 1px solid var(--ff-border);
  }
  .ff-dedupe__archived:first-child {
    border-top: none;
  }
  .ff-dedupe__members {
    display: flex;
    flex-direction: column;
  }
  .ff-dedupe__member {
    display: grid;
    grid-template-columns: 14px minmax(0, 1.2fr) minmax(0, 1fr) 80px;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 26px;
    margin: 0 calc(-1 * var(--ff-space-3));
    padding: 0 var(--ff-space-3);
    border: none;
    border-radius: var(--ff-radius-md);
    background: none;
    font: inherit;
    font-size: var(--ff-type-compact);
    color: var(--ff-text-mid);
    text-align: left;
    cursor: pointer;
  }
  .ff-dedupe__member:disabled {
    cursor: default;
  }
  .ff-dedupe__member:not(:disabled):hover {
    background: var(--ff-hover);
  }
  .ff-dedupe__member:focus-visible {
    outline: 1px solid var(--ff-border-hover);
    outline-offset: -1px;
  }
  .ff-dedupe__member--keeper {
    color: var(--ff-text);
    cursor: default;
  }
  .ff-dedupe__dot {
    width: 8px;
    height: 8px;
    box-sizing: border-box;
    border: 1px solid var(--ff-muted);
    border-radius: 50%;
  }
  .ff-dedupe__member--keeper .ff-dedupe__dot {
    border-color: var(--ff-text);
    background: var(--ff-text);
  }
  .ff-dedupe__detail,
  .ff-dedupe__path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__path {
    font-size: var(--ff-type-caption);
    color: var(--ff-muted);
  }
  .ff-dedupe__tag {
    justify-self: end;
    font-size: var(--ff-type-caption);
    color: var(--ff-muted);
    opacity: 0;
  }
  .ff-dedupe__member--keeper .ff-dedupe__tag {
    opacity: 1;
  }
  .ff-dedupe__member:not(:disabled):hover .ff-dedupe__tag,
  .ff-dedupe__member:focus-visible .ff-dedupe__tag {
    opacity: 1;
    color: var(--ff-text-mid);
  }
  .ff-dedupe__reason {
    padding-left: calc(14px + var(--ff-space-4));
    font-size: var(--ff-type-caption);
    color: var(--ff-muted);
  }
</style>
