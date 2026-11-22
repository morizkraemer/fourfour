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
          <p class="ff-dedupe__note">
            {groupsTitle(duplicates.groups.length)}. Merging keeps one entry; playlists, cues, ratings, colours and comments move to it. No file is deleted.
          </p>
          <div class="ff-dedupe__list">
            {#each duplicates.groups as group (group.key)}
              {@const keeperId = keeperOf(group, duplicates.choices)}
              {@const skipped = duplicates.skipped.has(group.key)}
              <section class="ff-dedupe__group" class:ff-dedupe__group--skipped={skipped} data-duplicate-group={group.key} data-confidence={group.confidence}>
                <header class="ff-dedupe__head">
                  <span class="ff-dedupe__title">{groupHeading(group)}</span>
                  <span class="ff-dedupe__kind">{confidenceLabel(group)}</span>
                </header>
                <p class="ff-dedupe__reason" data-duplicate-reason>Keeper: {reasonOf(group, duplicates.choices)}</p>
                {#each group.members as member (member.id)}
                  <div class="ff-dedupe__member" class:ff-dedupe__member--keeper={member.id === keeperId} data-duplicate-member={member.id} title={member.path}>
                    <span class="ff-dedupe__tag">{member.id === keeperId ? 'Keep' : 'Merge'}</span>
                    <span class="ff-dedupe__detail">{memberDetail(member)}</span>
                    <span class="ff-dedupe__path">{member.path}</span>
                    {#if member.id !== keeperId}
                      <Button label="Keep this one" size="small" variant="ghost" disabled={skipped || duplicates.busy} onclick={() => chooseKeeper(group, member.id)} />
                    {/if}
                  </div>
                {/each}
                <div class="ff-dedupe__actions">
                  <Button label={skipped ? 'Undo skip' : 'Skip'} size="small" variant="ghost" disabled={duplicates.busy} onclick={() => toggleSkip(group)} />
                  <Button label="Merge" size="small" variant="primary" disabled={skipped || duplicates.busy} onclick={() => void mergeGroup(group)} />
                </div>
              </section>
            {/each}
          </div>
        {/if}
      {:else if duplicates.archive.length === 0}
        <p class="ff-dedupe__note" data-archive-empty>Nothing merged yet.</p>
      {:else}
        <p class="ff-dedupe__note">Merged copies are out of your library, but their files are still on disk. Restore puts them back with their ids, playlists and cues.</p>
        <div class="ff-dedupe__list">
          {#each duplicates.archive as merge (merge.merge_id)}
            <section class="ff-dedupe__group" data-archived-merge={merge.merge_id}>
              <header class="ff-dedupe__head">
                <span class="ff-dedupe__title">{archiveHeading(merge)}</span>
                <span class="ff-dedupe__kind">{archiveDate(merge)}</span>
              </header>
              {#each merge.tracks as track (track.id)}
                <div class="ff-dedupe__member" title={track.path}>
                  <span class="ff-dedupe__tag">{track.format || 'file'}</span>
                  <span class="ff-dedupe__detail">{track.artist ? `${track.artist} - ${track.title}` : track.title}</span>
                  <span class="ff-dedupe__path">{track.path}</span>
                </div>
              {/each}
              <div class="ff-dedupe__actions">
                <Button label="Restore" size="small" variant="primary" disabled={duplicates.busy} onclick={() => void restoreMerge(merge.merge_id)} />
              </div>
            </section>
          {/each}
        </div>
      {/if}
    </div>
  {/snippet}
  {#snippet actions()}
    {#if duplicates.tab === 'groups'}
      <Button
        label={exactLeft > 0 ? `Accept all exact matches (${exactLeft})` : 'Accept all exact matches'}
        disabled={exactLeft === 0 || duplicates.busy}
        onclick={() => void mergeAllExact()}
      />
    {/if}
    <Button label="Done" variant="primary" onclick={closeDuplicates} />
  {/snippet}
</Dialog>

<style>
  .ff-dedupe {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }
  .ff-dedupe p {
    margin: 0;
  }
  .ff-dedupe__note {
    color: var(--ff-text-mid);
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
    gap: var(--ff-space-4);
    max-height: 56vh;
    overflow-y: auto;
  }
  .ff-dedupe__group {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
    padding: var(--ff-space-2) 0;
  }
  .ff-dedupe__group--skipped {
    opacity: 0.55;
  }
  .ff-dedupe__head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--ff-space-4);
    padding: 0 var(--ff-space-4);
    font-size: var(--ff-type-compact);
  }
  .ff-dedupe__title {
    color: var(--ff-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__kind {
    flex: none;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ff-muted);
  }
  .ff-dedupe__reason {
    padding: 0 var(--ff-space-4);
    font-size: var(--ff-type-compact);
    color: var(--ff-text-mid);
  }
  .ff-dedupe__member {
    display: grid;
    grid-template-columns: 48px minmax(0, 1.4fr) minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 28px;
    padding: 0 var(--ff-space-4);
    font-size: var(--ff-type-compact);
    color: var(--ff-text-mid);
  }
  .ff-dedupe__member--keeper {
    color: var(--ff-text);
  }
  .ff-dedupe__tag {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ff-muted);
  }
  .ff-dedupe__detail,
  .ff-dedupe__path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__path {
    color: var(--ff-muted);
  }
  .ff-dedupe__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--ff-space-2);
    padding: var(--ff-space-2) var(--ff-space-4) 0;
  }
</style>
