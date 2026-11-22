<script>
  import { Button, Icon, SettingRow, StatusDot } from '$ds';
  import { library } from '../stores/library.svelte.ts';
  import { fileStatus } from '../stores/file-status.svelte.ts';
  import {
    reconnectState,
    folderRelocation,
    relocateMissing,
    findMovedFiles,
    chooseReconnect,
    openFolderRelocation,
    closeFolderRelocation,
    previewFolderRelocation,
    chooseNewRoot,
    applyFolderRelocation,
    undoFolderRelocation,
  } from '../stores/file-health.svelte.ts';
  import { candidateLabel, canApplyRemap, choiceReasonText, reconnectSummary, relocationText, remapPreviewText } from '../lib/reconnect.ts';

  const MAX_CHOICES = 5;

  let plan = $derived(reconnectState.plan);
  let choices = $derived(plan ? plan.needs_choice.filter((c) => fileStatus.byId[c.track_id] === 'missing') : []);
  let shownChoices = $derived(choices.slice(0, MAX_CHOICES));
  let busy = $derived(reconnectState.busy || folderRelocation.busy);

  function trackName(id) {
    const t = library.tracks.find((track) => track.id === id);
    if (!t) return `Track ${id}`;
    return t.artist && t.artist !== '—' ? `${t.artist} – ${t.title}` : t.title;
  }

  function onOldRootKeydown(event) {
    event.stopPropagation();
    if (event.key === 'Enter') void previewFolderRelocation();
  }
</script>

<div class="ff-reconnect" data-reconnect-panel>
  {#if fileStatus.missing > 0}
    <div class="ff-reconnect__options" data-reconnect-options>
      <SettingRow title="Files were moved or renamed" description="Matched by their audio, even if renamed.">
        {#snippet control()}
          <Button label="Find moved files…" size="small" variant="primary" disabled={busy} onclick={() => void findMovedFiles()} />
        {/snippet}
      </SettingRow>
      <SettingRow title="A whole folder moved" description="Points its tracks to the new place.">
        {#snippet control()}
          <Button label="Relocate folder…" size="small" disabled={busy} onclick={() => void openFolderRelocation()} />
        {/snippet}
      </SettingRow>
      <SettingRow title="Files kept their names" description="Searches one folder by file name.">
        {#snippet control()}
          <Button label="Relocate missing…" size="small" disabled={busy || fileStatus.relocating} onclick={() => void relocateMissing()} />
        {/snippet}
      </SettingRow>
    </div>
  {/if}

  {#if plan}
    <section class="ff-reconnect__section" data-reconnect-result>
      <h3>Find moved files</h3>
      <p>{reconnectSummary(plan)}</p>
      {#each shownChoices as choice (choice.track_id)}
        <div class="ff-reconnect__choice" data-reconnect-choice={choice.track_id}>
          <div class="ff-reconnect__line">
            <StatusDot status="warn" />
            <span class="ff-reconnect__text">{trackName(choice.track_id)} <em>{choiceReasonText(choice.reason)}</em></span>
          </div>
          <div class="ff-reconnect__candidates">
            {#each choice.candidates as path (path)}
              <Button label={candidateLabel(path)} title={path} size="small" variant="ghost" disabled={busy} onclick={() => void chooseReconnect(choice.track_id, path)} />
            {/each}
          </div>
        </div>
      {/each}
      {#if choices.length > shownChoices.length}
        <p class="ff-reconnect__note">and {choices.length - shownChoices.length} more</p>
      {/if}
    </section>
  {/if}
  {#if reconnectState.error}
    <p class="ff-reconnect__error">{reconnectState.error}</p>
  {/if}

  {#if folderRelocation.open}
    <section class="ff-reconnect__section" data-relocate-folder>
      <h3>Relocate folder</h3>
      <div class="ff-reconnect__row">
        <input
          class="ff-reconnect__from"
          type="text"
          spellcheck="false"
          autocomplete="off"
          placeholder="Old folder"
          title={folderRelocation.oldRoot}
          bind:value={folderRelocation.oldRoot}
          disabled={busy}
          onkeydown={onOldRootKeydown}
          onchange={() => void previewFolderRelocation()}
        />
        <Icon name="chevron-right" size={12} />
        <span class="ff-reconnect__path" class:ff-reconnect__path--empty={!folderRelocation.newRoot} title={folderRelocation.newRoot}>
          {folderRelocation.newRoot || 'New folder'}
        </span>
        <Button label={folderRelocation.newRoot ? 'Change…' : 'Choose…'} size="small" disabled={busy} onclick={() => void chooseNewRoot()} />
      </div>
      {#if folderRelocation.preview}
        <p class="ff-reconnect__stat" data-relocate-preview>{remapPreviewText(folderRelocation.preview)}</p>
      {/if}
      <div class="ff-reconnect__buttons">
        <Button label="Relocate" size="small" variant="primary" disabled={busy || !canApplyRemap(folderRelocation.preview)} onclick={() => void applyFolderRelocation()} />
        <Button label="Cancel" size="small" variant="ghost" onclick={closeFolderRelocation} />
      </div>
    </section>
  {/if}
  {#if folderRelocation.last}
    <div class="ff-reconnect__line" data-relocate-last>
      <StatusDot status="online" />
      <span class="ff-reconnect__text" title={relocationText(folderRelocation.last.old_root, folderRelocation.last.new_root, folderRelocation.last.tracks)}>
        {relocationText(folderRelocation.last.old_root, folderRelocation.last.new_root, folderRelocation.last.tracks)}
      </span>
      <Button label="Undo" size="small" variant="ghost" disabled={busy} onclick={() => void undoFolderRelocation()} />
    </div>
  {/if}
  {#if folderRelocation.error}
    <p class="ff-reconnect__error">{folderRelocation.error}</p>
  {/if}
</div>

<style>
  .ff-reconnect {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }
  .ff-reconnect p,
  .ff-reconnect h3 {
    margin: 0;
  }
  .ff-reconnect h3 {
    font-size: inherit;
    font-weight: 600;
  }
  .ff-reconnect__buttons,
  .ff-reconnect__candidates {
    display: flex;
    flex-wrap: wrap;
    gap: var(--ff-space-2);
  }
  .ff-reconnect__options {
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    background: var(--ff-bg);
  }
  .ff-reconnect__options :global(.ff-setting-row) {
    padding: var(--ff-space-4) var(--ff-space-5);
  }
  .ff-reconnect__section {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-2);
  }
  .ff-reconnect__choice {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-1);
  }
  .ff-reconnect__line,
  .ff-reconnect__row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 28px;
  }
  .ff-reconnect__text,
  .ff-reconnect__path {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-reconnect__text em {
    font-style: normal;
    color: var(--ff-muted);
  }
  .ff-reconnect__path {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-caption);
  }
  .ff-reconnect__path--empty {
    color: var(--ff-muted);
  }
  .ff-reconnect__from {
    flex: 1 1 0;
    min-width: 0;
    height: 32px;
    box-sizing: border-box;
    padding: 0 10px;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-caption);
    color: var(--ff-text);
    background: var(--ff-bg);
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-md);
    outline: none;
  }
  .ff-reconnect__from:focus {
    border-color: rgba(255, 255, 255, 0.3);
  }
  .ff-reconnect__stat,
  .ff-reconnect__note {
    color: var(--ff-text-mid);
  }
  .ff-reconnect__error {
    color: var(--ff-danger);
  }
</style>
