<script>
  import './sidebar-row.css';
  import { nativeListener } from './nativeListener.js';
  import Icon from './Icon.svelte';
  import TagBadge from './TagBadge.svelte';
  import StatusDot from './StatusDot.svelte';
  import { isKeyboardFocused } from '../utils/focus.js';

  let {
    kind = 'leaf',
    label,
    icon,
    count,
    digit,
    status,
    action,
    state: rowState = 'rest',
    depth = 0,
    expanded,
    splitAction = true,
    renaming = false,
    renameValue = $bindable(''),
    onOpenInPanel,
    onclick,
    ondblclick,
    onRenameCommit,
    onRenameCancel,
  } = $props();

  let selected = $derived(rowState === 'active' || rowState === 'selected');
  /** @type {HTMLInputElement | null} */
  let renameInputEl = $state(null);
  let skipRenameBlurCommit = false;

  $effect(() => {
    if (renaming && renameInputEl) {
      renameInputEl.focus();
      renameInputEl.select();
    }
  });

  function openInPanel(e) {
    e.stopPropagation();
    onOpenInPanel?.();
  }

  function onRowKeydown(e) {
    if (e.target !== e.currentTarget || e.altKey) return;
    if (e.key === 'Enter' || (e.key === ' ' && isKeyboardFocused(e.currentTarget))) {
      e.preventDefault();
      onclick?.(e);
    } else if (e.key === 'F2' && ondblclick) {
      e.preventDefault();
      ondblclick(e);
    }
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
  <div
    class="ff-sbrow ff-sbrow--{rowState}"
    role="button"
    tabindex="0"
    data-focus-first={rowState === 'active' ? '' : undefined}
    aria-current={rowState === 'active' ? 'page' : undefined}
    aria-expanded={expanded != null ? expanded : undefined}
    style:padding-left={depth > 0 ? `calc(var(--ff-space-6) + ${depth * 14}px)` : undefined}
    {onclick}
    {ondblclick}
    onkeydown={onRowKeydown}
  >
    {#if expanded != null}
      <Icon name={expanded ? 'chevron-down' : 'chevron-right'} size={11} class="ff-sbrow__chevron" />
    {/if}
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
        aria-label="Name"
        bind:value={renameValue}
        onkeydown={handleRenameKeydown}
        onblur={handleRenameBlur}
        {@attach nativeListener('click', (e) => e.stopPropagation())}
        {@attach nativeListener('dblclick', (e) => e.stopPropagation())}
      />
    {:else}
      <span class="ff-sbrow__label">{label}</span>
    {/if}

    {#if count != null}<span class="ff-sbrow__count">{count}</span>{/if}
    {#if onOpenInPanel}
      <button class="ff-sbrow__action" title="Open in side panel" {@attach nativeListener('click', openInPanel)}>
        <Icon name="columns-2" size={13} />
      </button>
    {/if}
  </div>
{/if}
