<script>
  // GhostField: a text value that looks like plain text until touched. Hover shows
  // a pencil, focus turns it into an input. Enter or blur commits, Esc reverts.
  //   mixed: several tracks selected with different values (shows "Mixed")
  //   suggest: autocomplete rows shown under the field while editing
  //   state: 'rest' | 'hover' | 'edit' (showcase)
  import './ghost-field.css';
  import Icon from './Icon.svelte';

  let {
    value = '',
    placeholder = '',
    mono = false,
    title = false,
    align = 'left',
    multiline = false,
    mixed = false,
    suggest = null,
    state: look = 'rest',
    label = '',
    onCommit = undefined,
  } = $props();

  let draft = $state(null);
  let shown = $derived(draft ?? (mixed ? '' : value));

  function commit() {
    const next = draft;
    draft = null;
    if (next !== null && next !== value) onCommit?.(next);
  }

  function onKeyDown(event) {
    event.stopPropagation();
    if (event.key === 'Enter' && !(multiline && event.shiftKey)) {
      event.preventDefault();
      commit();
      event.currentTarget.blur();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      draft = null;
      event.currentTarget.blur();
    }
  }
</script>

<div
  class="ff-ghost{mono ? ' ff-ghost--mono' : ''}{title ? ' ff-ghost--title' : ''}{align === 'right' ? ' ff-ghost--right' : ''}{multiline ? ' ff-ghost--multi' : ''}{look === 'hover' ? ' ff-ghost--hover' : ''}{look === 'edit' ? ' ff-ghost--edit' : ''}">
  {#if multiline}
    <textarea
      class="ff-ghost__input"
      rows="2"
      aria-label={label || placeholder}
      placeholder={mixed ? 'Mixed' : placeholder}
      spellcheck="false"
      value={shown}
      oninput={(e) => (draft = e.currentTarget.value)}
      onkeydown={onKeyDown}
      onblur={commit}></textarea>
  {:else}
    <input
      class="ff-ghost__input"
      type="text"
      aria-label={label || placeholder}
      placeholder={mixed ? 'Mixed' : placeholder}
      spellcheck="false"
      value={shown}
      oninput={(e) => (draft = e.currentTarget.value)}
      onkeydown={onKeyDown}
      onblur={commit} />
  {/if}
  <span class="ff-ghost__pen"><Icon name="pencil" size={11} /></span>
  {#if suggest && look === 'edit'}
    <div class="ff-ghost__suggest">
      {#each suggest as option, i}
        <div class="ff-ghost__option{i === 0 ? ' ff-ghost__option--on' : ''}">{option}</div>
      {/each}
    </div>
  {/if}
</div>
