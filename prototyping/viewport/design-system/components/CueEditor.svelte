<script>
  // Edit one cue: name, colour (palette swatches) and delete.
  let {
    chip,
    palette = [],
    onRename = undefined,
    onRecolor = undefined,
    onDelete = undefined,
    onClose = undefined,
  } = $props();

  let draft = $state('');
  let nameInput = $state(null);

  $effect(() => {
    nameInput?.focus();
    nameInput?.select();
  });
  let editingKey = null;

  $effect(() => {
    if (chip.key !== editingKey) {
      editingKey = chip.key;
      draft = chip.name;
    }
  });

  function commitName() {
    if (chip && draft.trim() !== chip.name) onRename?.(draft);
  }

  function onKey(e) {
    e.stopPropagation();
    if (e.key === 'Enter') {
      commitName();
      onClose?.();
    } else if (e.key === 'Escape') {
      draft = chip.name;
      onClose?.();
    }
  }
</script>

<div class="ff-cue-editor" role="group" aria-label="Edit cue {chip.label}">
  <div class="ff-cue-editor__row">
    <span class="ff-cue-editor__badge{chip.slot === 0 ? ' ff-cue-editor__badge--memory' : ''}" style="background-color:{chip.color}">{chip.letter}</span>
    <input
      class="ff-cue-editor__name"
      type="text"
      maxlength="32"
      placeholder="Name"
      aria-label="Cue name"
      bind:this={nameInput}
      bind:value={draft}
      onkeydown={onKey}
      onblur={commitName} />
    <span class="ff-cue-editor__time">{chip.time}</span>
  </div>
  {#if chip.slot !== 0}
  <div class="ff-cue-editor__palette" role="radiogroup" aria-label="Cue colour">
    {#each palette as swatch}
      <button
        type="button"
        class="ff-cue-editor__swatch{swatch.color === chip.color ? ' ff-cue-editor__swatch--on' : ''}"
        style="background-color:{swatch.color}"
        title={swatch.name}
        aria-label={swatch.name}
        role="radio"
        aria-checked={swatch.color === chip.color}
        onclick={() => onRecolor?.(swatch.color)}></button>
    {/each}
  </div>
  {/if}
  <div class="ff-cue-editor__actions">
    <button type="button" class="ff-cue-editor__btn ff-cue-editor__btn--danger" onclick={() => onDelete?.()}>Delete</button>
    <button type="button" class="ff-cue-editor__btn" onclick={() => onClose?.()}>Done</button>
  </div>
</div>

<style>
  .ff-cue-editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border-radius: var(--ff-radius-sm);
    background-color: var(--ff-elev);
    border: 1px solid var(--ff-border-hi);
    box-sizing: border-box;
    width: 100%;
  }
  .ff-cue-editor__row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ff-cue-editor__badge {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--ff-font);
    font-size: 11px;
    font-weight: 700;
    color: #0a0a0a;
  }
  .ff-cue-editor__badge--memory {
    background: none !important;
    border: 1px solid var(--ff-border-hi);
    color: var(--ff-text-mid);
    box-sizing: border-box;
  }
  .ff-cue-editor__name {
    flex: 1 1 0;
    min-width: 0;
    height: 24px;
    padding: 0 6px;
    border-radius: var(--ff-radius-sm);
    border: 1px solid var(--ff-border-hi);
    background: var(--ff-bg);
    color: var(--ff-text);
    font-family: var(--ff-font);
    font-size: 12px;
    box-sizing: border-box;
  }
  .ff-cue-editor__time {
    flex: none;
    font-family: var(--ff-font-mono);
    font-size: 11px;
    color: var(--ff-text-mid);
  }
  .ff-cue-editor__palette {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 4px;
  }
  .ff-cue-editor__swatch {
    aspect-ratio: 1;
    min-width: 0;
    border-radius: 3px;
    border: 1px solid transparent;
    padding: 0;
    cursor: pointer;
  }
  .ff-cue-editor__swatch--on {
    border-color: var(--ff-text);
    box-shadow: 0 0 0 1px var(--ff-bg) inset;
  }
  .ff-cue-editor__actions {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .ff-cue-editor__btn {
    height: 24px;
    padding: 0 10px;
    border-radius: var(--ff-radius-sm);
    border: 1px solid var(--ff-border-hi);
    background: transparent;
    color: var(--ff-text);
    font-family: var(--ff-font);
    font-size: 11px;
    cursor: pointer;
  }
  .ff-cue-editor__btn:hover {
    border-color: var(--ff-border-hover);
  }
  .ff-cue-editor__btn--danger {
    color: var(--ff-danger);
    border-color: var(--ff-danger-border);
  }
  .ff-cue-editor__btn--danger:hover {
    background: var(--ff-danger-bg);
    border-color: var(--ff-danger-border-hi);
  }
</style>
