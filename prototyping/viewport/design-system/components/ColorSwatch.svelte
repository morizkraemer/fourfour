<script>
  // CDJ color swatch (Svelte port of color-swatch.js). Rounded square color tag
  // on track rows.
  //   color: CDJ index 1..7 or any hex
  //   size: edge length in px (default 9, matching the track-row tag slot)
  import './color-swatch.css';

  let { color, size = 9 } = $props();
  let fill = $derived(typeof color === 'number' ? `var(--ff-track-color-${color})` : color);
</script>

<span class="ff-swatch" style="width:{size}px;height:{size}px;background:{fill}"></span>
