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
    onSplit,
  } = $props();

  let selected = $derived(state === 'active' || state === 'selected');

  function split(e) {
    e.stopPropagation();
    onSplit?.();
  }
</script>

{#if kind === 'section'}
  <div class="ff-sbrow__section{action ? ' ff-sbrow__section--with-action' : ''}">
    <span class="ff-sbrow__section-label">{label}</span>
    {#if action}<span class="ff-sbrow__section-action">{action}</span>{/if}
  </div>
{:else}
  <div class="ff-sbrow ff-sbrow--{state}">
    {#if kind === 'favorite'}
      <TagBadge value={digit} variant={selected ? 'filled' : 'outline'} size="md" />
    {:else if kind === 'usb'}
      <StatusDot status={status ?? 'offline'} />
    {:else if icon}
      <Icon name={icon} size={13} />
    {/if}

    <span class="ff-sbrow__label">{label}</span>

    {#if count != null}<span class="ff-sbrow__count">{count}</span>{/if}
    {#if splitAction}
      <button class="ff-sbrow__action" title="Open in side panel" onclick={split}>
        <Icon name="columns-2" size={13} />
      </button>
    {/if}
  </div>
{/if}
