<script>
  import { CUE_COLORS } from './data.js';
  let { selected = 0, size = 14, withNone = true } = $props();
</script>

<span class="sw">
  {#if withNone}<span class="sw__dot sw__dot--none" class:sw__on={selected === -1} style="width:{size}px;height:{size}px"></span>{/if}
  {#each CUE_COLORS as c, i}
    <span class="sw__dot" class:sw__on={selected === i + 1} style="background:{c};width:{size}px;height:{size}px"></span>
  {/each}
</span>

<style>
  .sw { display: inline-flex; gap: 6px; align-items: center; }
  .sw__dot { border-radius: 3px; display: block; box-sizing: border-box; }
  .sw__dot--none { border: 1px dashed var(--ff-faint); background: transparent; }
  .sw__on { outline: 1.5px solid var(--ff-accent); outline-offset: 2px; }
</style>
