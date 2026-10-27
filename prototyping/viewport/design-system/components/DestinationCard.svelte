<script>
  import './destination-card.css';
  import Icon from './Icon.svelte';
  import StatusDot from './StatusDot.svelte';

  let {
    variant = 'playlist',
    name,
    meta = '',
    covers = [],
    online = true,
    dropActive = false,
    flash = '',
    onclick,
    ...rest
  } = $props();

  let tiles = $derived([0, 1, 2, 3].map((i) => covers[i] ?? null));
</script>

<button
  type="button"
  class="ff-dcard ff-dcard--{variant}"
  class:ff-dcard--drop={dropActive}
  class:ff-dcard--offline={variant === 'stick' && !online}
  {onclick}
  {...rest}
>
  {#if variant === 'new'}
    <Icon name="plus" size={20} />
    <span class="ff-dcard__name">{name}</span>
    {#if meta}<span class="ff-dcard__meta ff-dcard__meta--mono">{meta}</span>{/if}
  {:else if variant === 'stick'}
    <Icon name="usb" size={18} />
    <span class="ff-dcard__text">
      <span class="ff-dcard__name"><StatusDot status={online ? 'online' : 'offline'} />{name}</span>
      <span class="ff-dcard__meta">{meta}</span>
    </span>
  {:else}
    <span class="ff-dcard__mosaic">
      {#each tiles as cover}
        <i style:background-image={cover ? `url('${cover}')` : null}></i>
      {/each}
    </span>
    <span class="ff-dcard__name">{name}</span>
    <span class="ff-dcard__meta">{meta}</span>
  {/if}
  {#if dropActive}<span class="ff-dcard__drop">Drop to add</span>{/if}
  {#if flash}<span class="ff-dcard__flash" role="status">{flash}</span>{/if}
</button>
