<script>
  import { Dialog, Button, SegmentedControl, SettingRow, StatusDot } from '$ds';
  import '$ds/components/checkbox.css';
  import { duplicates, closeDuplicates, showTab, chooseKeeper, togglePick, mergePicked, restoreMerge } from '../stores/duplicate-cleanup.svelte.ts';
  import {
    archiveHeading,
    archiveNote,
    copyFacts,
    copyPlaces,
    groupHeading,
    groupNote,
    groupsTitle,
    keeperOf,
    mergeLabel,
    pickedCount,
  } from '../lib/duplicateCleanup.ts';

  let onGroups = $derived(duplicates.tab === 'groups');
  let toMerge = $derived(pickedCount(duplicates.groups, duplicates.picked));
  let canMerge = $derived(onGroups && duplicates.groups.length > 0);
  let tabs = $derived([
    duplicates.groups.length > 0 ? `Found (${duplicates.groups.length})` : 'Found',
    duplicates.archive.length > 0 ? `Archive (${duplicates.archive.length})` : 'Archive',
  ]);
</script>

<Dialog open={duplicates.open} variant="wide" title="Duplicates" onClose={closeDuplicates}>
  {#snippet body()}
    <div class="ff-dedupe" data-duplicates-view>
      <div class="ff-dedupe__tabs">
        <SegmentedControl items={tabs} activeIndex={onGroups ? 0 : 1} onChange={(i) => showTab(i === 0 ? 'groups' : 'archive')} />
      </div>

      {#if onGroups}
        {#if duplicates.loading}
          <p class="ff-dedupe__lead">Looking for copies…</p>
        {:else if duplicates.groups.length === 0}
          {#if !duplicates.message}
            <div class="ff-dedupe__line" data-duplicates-empty>
              <StatusDot status="online" />
              <span>No duplicates. Every song is in your library once.</span>
            </div>
          {/if}
        {:else}
          <p class="ff-dedupe__lead">{groupsTitle(duplicates.groups.length)}</p>
        {/if}
      {:else if duplicates.archive.length === 0}
        <p class="ff-dedupe__lead" data-archive-empty>Nothing merged yet. Merged copies show up here and can be restored.</p>
      {:else}
        <p class="ff-dedupe__lead">Merged copies are out of your library. Their files are still on disk.</p>
      {/if}

      {#if duplicates.message}
        <div class="ff-dedupe__line" data-duplicates-message>
          <StatusDot status="online" />
          <span>{duplicates.message}</span>
        </div>
      {/if}
      {#if duplicates.error}
        <p class="ff-dedupe__error" data-duplicates-error>{duplicates.error}</p>
      {/if}

      {#if onGroups && !duplicates.loading && duplicates.groups.length > 0}
        <div class="ff-dedupe__list">
          {#each duplicates.groups as group (group.key)}
            {@const keeperId = keeperOf(group, duplicates.choices)}
            {@const picked = duplicates.picked.has(group.key)}
            {@const places = copyPlaces(group.members.map((m) => m.path))}
            <section class="ff-dedupe__song" class:ff-dedupe__song--off={!picked} data-duplicate-group={group.key} data-confidence={group.confidence}>
              <label class="ff-checkbox ff-dedupe__pick" class:ff-checkbox--on={picked}>
                <input
                  class="ff-dedupe__input"
                  type="checkbox"
                  checked={picked}
                  disabled={duplicates.busy}
                  data-duplicate-pick={group.key}
                  onchange={() => togglePick(group)}
                />
                <span class="ff-checkbox__box">
                  <span class="ff-checkbox__icon">
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                      <polyline points="2,5.5 4.2,7.5 8,2.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                  </span>
                </span>
                <span class="ff-dedupe__texts">
                  <span class="ff-dedupe__title">{groupHeading(group)}</span>
                  <span class="ff-dedupe__note" data-duplicate-reason>{groupNote(group, duplicates.choices)}</span>
                </span>
              </label>
              <div class="ff-dedupe__copies" role="radiogroup" aria-label={`Copy of ${groupHeading(group)} to keep`}>
                {#each group.members as member, i (member.id)}
                  {@const keeper = member.id === keeperId}
                  <label class="ff-dedupe__copy" class:ff-dedupe__copy--keep={keeper} data-duplicate-member={member.id} title={member.path}>
                    <input
                      class="ff-dedupe__input"
                      type="radio"
                      name={`ff-dedupe-keeper-${group.key}`}
                      checked={keeper}
                      disabled={duplicates.busy}
                      onchange={() => chooseKeeper(group, member.id)}
                    />
                    <span class="ff-radio" class:ff-radio--on={keeper}>
                      <span class="ff-radio__circle"><span class="ff-radio__dot"></span></span>
                    </span>
                    <span class="ff-dedupe__fate">{picked ? (keeper ? 'Keep' : 'Merge') : ''}</span>
                    <span class="ff-dedupe__facts">
                      {copyFacts(member, group)}
                      {#if !member.file_present}<span class="ff-dedupe__missing"> · file missing</span>{/if}
                    </span>
                    <span class="ff-dedupe__place">{places[i]}</span>
                  </label>
                {/each}
              </div>
            </section>
          {/each}
        </div>
        <p class="ff-dedupe__foot">Playlists, cues and ratings move to the copy you keep. No file is deleted.</p>
      {:else if !onGroups && duplicates.archive.length > 0}
        <div class="ff-dedupe__list">
          {#each duplicates.archive as merge (merge.merge_id)}
            <div data-archived-merge={merge.merge_id} title={merge.tracks.map((t) => t.path).join('\n')}>
              <SettingRow title={archiveHeading(merge)} description={archiveNote(merge)}>
                {#snippet control()}
                  <Button label="Restore" size="small" disabled={duplicates.busy} onclick={() => void restoreMerge(merge.merge_id)} />
                {/snippet}
              </SettingRow>
            </div>
          {/each}
        </div>
        <p class="ff-dedupe__foot">Restore puts a copy back in your library with its playlists and cues.</p>
      {/if}
    </div>
  {/snippet}
  {#snippet actions()}
    <Button label="Done" variant={canMerge ? 'default' : 'primary'} onclick={closeDuplicates} />
    {#if canMerge}
      <Button label={mergeLabel(toMerge)} variant="primary" disabled={duplicates.busy || toMerge === 0} onclick={() => void mergePicked()} />
    {/if}
  {/snippet}
</Dialog>

<style>
  :global(.ff-dialog--wide:has(.ff-dedupe)) {
    min-width: 0;
    max-width: 620px;
    width: min(620px, calc(100vw - 48px));
    padding: 20px 22px;
  }
  .ff-dedupe {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-5);
    font-size: var(--ff-type-caption);
    line-height: 1.5;
    color: var(--ff-text-mid);
  }
  .ff-dedupe p {
    margin: 0;
  }
  .ff-dedupe__tabs {
    position: absolute;
    top: 17px;
    right: 22px;
  }
  .ff-dedupe__lead {
    color: var(--ff-text);
  }
  .ff-dedupe__foot {
    color: var(--ff-muted);
  }
  .ff-dedupe__error {
    color: var(--ff-danger);
  }
  .ff-dedupe__line {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 28px;
    color: var(--ff-text);
  }
  .ff-dedupe__list {
    max-height: 56vh;
    overflow-y: auto;
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
  }
  .ff-dedupe__list > * + * {
    border-top: 1px solid var(--ff-border);
  }
  .ff-dedupe__list :global(.ff-setting-row) {
    padding: var(--ff-space-4) var(--ff-space-5);
  }
  .ff-dedupe__song {
    padding: var(--ff-space-4) var(--ff-space-3) var(--ff-space-3);
  }
  .ff-dedupe__input {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: 0;
    opacity: 0;
    pointer-events: none;
  }
  .ff-dedupe__pick {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: var(--ff-space-4);
    padding: var(--ff-space-1) var(--ff-space-3);
    user-select: none;
  }
  .ff-dedupe__pick .ff-checkbox__box {
    margin-top: 3px;
  }
  .ff-dedupe__input:focus-visible + .ff-checkbox__box {
    box-shadow: 0 0 0 2px var(--ff-bg), 0 0 0 3px rgba(255, 255, 255, 0.6);
  }
  .ff-dedupe__texts {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-1);
  }
  .ff-dedupe__title {
    font-size: var(--ff-type-body);
    font-weight: 500;
    line-height: 1.4;
    color: var(--ff-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__song--off .ff-dedupe__title {
    font-weight: 400;
    color: var(--ff-text-mid);
  }
  .ff-dedupe__note {
    font-size: var(--ff-type-caption);
    line-height: 1.4;
    color: var(--ff-muted);
  }
  .ff-dedupe__copies {
    display: flex;
    flex-direction: column;
    margin-top: var(--ff-space-3);
    padding-left: 22px;
  }
  .ff-dedupe__copy {
    position: relative;
    display: grid;
    grid-template-columns: 14px 38px minmax(0, 1fr) minmax(0, auto);
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 26px;
    padding: 0 var(--ff-space-3);
    border-radius: var(--ff-radius-sm);
    font-size: var(--ff-type-compact);
    color: var(--ff-text-mid);
    cursor: pointer;
    user-select: none;
  }
  .ff-dedupe__copy:hover {
    background: var(--ff-hover);
  }
  .ff-dedupe__copy:has(.ff-dedupe__input:focus-visible) {
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.45);
  }
  .ff-dedupe__copy--keep {
    color: var(--ff-text);
  }
  .ff-dedupe__facts,
  .ff-dedupe__place {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-dedupe__fate {
    color: var(--ff-text-mid);
  }
  .ff-dedupe__copy--keep .ff-dedupe__fate {
    font-weight: 600;
    color: var(--ff-text);
  }
  .ff-dedupe__place {
    max-width: 240px;
    font-size: var(--ff-type-caption);
    color: var(--ff-text-mid);
  }
  .ff-dedupe__copy--keep .ff-dedupe__place {
    color: var(--ff-text);
  }
  .ff-dedupe__missing {
    color: var(--ff-danger);
  }
</style>
