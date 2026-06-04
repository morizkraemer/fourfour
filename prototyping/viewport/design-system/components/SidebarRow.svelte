<script>
  // Sidebar row (Svelte port of sidebar-row.js). One component covers every row
  // kind in the nav.
  //   kind: 'section' | 'favorite' | 'leaf' | 'playlist' | 'usb'
  //   state: 'rest' | 'hover' | 'active' | 'selected'  (items only)
  import './sidebar-row.css';
  import Icon from './Icon.svelte';
  import TagBadge from './TagBadge.svelte';
  import StatusDot from './StatusDot.svelte';

  let {
    kind = 'leaf',
    label,
    icon,
    count,
    digit,
    status,
    action,
    state = 'rest',
    splitAction = true,
    renaming = false,
    renameValue = $bindable(''),
    onSplit,
    onclick,
    ondblclick,
    onRenameCommit,
    onRenameCancel,
  } = $props();

  let selected = $derived(state === 'active' || state === 'selected');
  /** @type {HTMLInputElement | null} */
  let renameInputEl = null;
  let skipRenameBlurCommit = false;

  $effect(() => {
    if (renaming && renameInputEl) {
      renameInputEl.focus();
      renameInputEl.select();
    }
  });

  function split(e) {
    e.stopPropagation();
    onSplit?.();
  }

  function handleRenameKeydown(e) {
    if (e.key === 'Enter') {
      e.preventDefault();
      skipRenameBlurCommit = true;
      onRenameCommit?.();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      skipRenameBlurCommit = true;
      onRenameCancel?.();
    }
  }

  function handleRenameBlur() {
    if (skipRenameBlurCommit) {
      skipRenameBlurCommit = false;
      return;
    }
    onRenameCommit?.();
  }
</script>

{#if kind === 'section'}
  <div class="ff-sbrow__section{action ? ' ff-sbrow__section--with-action' : ''}">
    <span class="ff-sbrow__section-label">{label}</span>
    {#if action}<span class="ff-sbrow__section-action">{action}</span>{/if}
  </div>
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ff-sbrow ff-sbrow--{state}" {onclick} {ondblclick}>
    {#if kind === 'favorite'}
      <TagBadge value={digit} variant={selected ? 'filled' : 'outline'} size="md" />
    {:else if kind === 'usb'}
      <StatusDot status={status ?? 'offline'} />
    {:else if icon}
      <Icon name={icon} size={13} />
    {/if}

    {#if renaming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:this={renameInputEl}
        class="ff-sbrow__label ff-sbrow__label-input"
        type="text"
        bind:value={renameValue}
        onkeydown={handleRenameKeydown}
        onblur={handleRenameBlur}
        onclick={(e) => e.stopPropagation()}
        ondblclick={(e) => e.stopPropagation()}
      />
    {:else}
      <span class="ff-sbrow__label">{label}</span>
    {/if}

    {#if count != null}<span class="ff-sbrow__count">{count}</span>{/if}
    {#if splitAction}
      <button class="ff-sbrow__action" title="Open in side panel" onclick={split}>
        <Icon name="columns-2" size={13} />
      </button>
    {/if}
  </div>
{/if}
