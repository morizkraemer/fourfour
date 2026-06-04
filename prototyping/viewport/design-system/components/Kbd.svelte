<script>
  // Keyboard hint (Svelte port of kbd.js). Label-only, never interactive.
  import './kbd.css';
  let { key } = $props();
</script>

<kbd class="ff-kbd">{key}</kbd>
