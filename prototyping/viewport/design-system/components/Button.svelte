<script>
  // Button (Svelte port of button.js). Variants/states 1:1 with button.css.
  //   variant: 'primary' | 'default' | 'ghost' | 'destructive'
  //   iconOnly: square icon button · icon: a name from icon-glyphs.js
  //   disabled: dims + drops pointer events · kbd: trailing shortcut hint
  import './button.css';
  import Icon from './Icon.svelte';
  import Kbd from './Kbd.svelte';

  let {
    label,
    variant = 'default',
    icon,
    iconOnly = false,
    disabled = false,
    size = 'default',
    kbd,
    onclick,
  } = $props();

  let iconSize = $derived(size === 'compact' ? 10 : size === 'small' ? 13 : 14);
  let cls = $derived(
    [
      'ff-btn',
      `ff-btn--${variant}`,
      iconOnly && 'ff-btn--icon',
      disabled && 'ff-btn--disabled',
      size === 'compact' && 'ff-btn--compact',
      size === 'small' && 'ff-btn--small',
    ]
      .filter(Boolean)
      .join(' ')
  );
</script>

<button class={cls} type="button" disabled={disabled || undefined} {onclick}>
  {#if icon}<Icon name={icon} size={iconSize} />{/if}
  {#if label && !iconOnly}<span class="ff-btn__label">{label}</span>{/if}
  {#if kbd}<Kbd key={kbd} />{/if}
</button>
