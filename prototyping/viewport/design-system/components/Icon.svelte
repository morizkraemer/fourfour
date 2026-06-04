<script>
  // Icon (Svelte port of icon.js). Inline lucide 24×24 stroke glyph; stroke is
  // currentColor so the parent's text color drives it. Glyphs from icon-glyphs.js.
  import './icon.css';
  import { ICONS } from './icon-glyphs.js';
  let { name, size = 14, strokeWidth = 2 } = $props();
</script>

<svg
  class="ff-icon"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round">
  {@html ICONS[name] ?? ''}
</svg>
