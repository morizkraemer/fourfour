<script>
  import { FilterInput, Kbd } from '../../../../viewport/design-system/svelte.js';
  import Ico from './Ico.svelte';

  let { boardOn = false } = $props();
</script>

<div class="ff-gheader ff-titlebar-band ff-titlebar-band--ruled w2t">
  <FilterInput placeholder="Search tracks, artists, labels…" />
  <Kbd key="⌘F" />
  <button class="w2t__dock" class:w2t__dock--on={boardOn} aria-label="Toggle organize sidebar" aria-pressed={boardOn}><Ico name="panel" size={15} /></button>
</div>

<style>
  .w2t { position: relative; display: flex; align-items: center; justify-content: center; gap: var(--ff-space-4); }
  .w2t__dock { position: absolute; right: 14px; width: 28px; height: 26px; border: 1px solid transparent; background: none; color: var(--ff-text-mid); border-radius: var(--ff-radius-md); display: flex; align-items: center; justify-content: center; }
  .w2t__dock--on { background: var(--ff-select); color: var(--ff-text); border-color: var(--ff-border); }
</style>
