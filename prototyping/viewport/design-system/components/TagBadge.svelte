<script>
  // Tag / digit badge (Svelte port of tag-badge.js). The numbered pill in the
  // sidebar Favorites section (md, outline/filled) and on selected track rows
  // (sm, color fill).
  //   variant: 'outline' (rest) | 'filled' (active) | 'color' (CDJ color fill)
  //   size: 'md' (18px, sidebar) | 'sm' (14px, track row)
  //   color: CDJ index 1..7 or any hex (only used by variant 'color')
  import './tag-badge.css';

  let { value, variant = 'outline', color, size = 'md' } = $props();

  const SIZES = { md: 18, sm: 14 };
  let box = $derived(SIZES[size] ?? SIZES.md);
  let style = $derived(
    [
      `width:${box}px`,
      `height:${box}px`,
      variant === 'color'
        ? `background:${typeof color === 'number' ? `var(--ff-track-color-${color})` : color}`
        : '',
    ]
      .filter(Boolean)
      .join(';')
  );
</script>

<span class="ff-badge ff-badge--{variant} ff-badge--{size}" {style}>
  <span class="ff-badge__value">{value}</span>
</span>
