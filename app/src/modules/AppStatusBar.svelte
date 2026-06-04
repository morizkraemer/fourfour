<!--
  AppStatusBar.svelte — Bottom status bar module.
  Wraps the $ds StatusBar primitive with library store data.
-->
<script>
  import { StatusBar } from '$ds';
  import { trackCount } from '../stores/library.svelte.ts';

  // Static for now — Phase 6 wires real analyzer state
  const message = 'Ready';
</script>

<div class="ff-statusbar-module">
  <StatusBar
    {message}
    context="{trackCount} tracks"
    count="1,284 tracks · 6.4 GB"
  />
</div>

<style>
  .ff-statusbar-module {
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }
</style>
