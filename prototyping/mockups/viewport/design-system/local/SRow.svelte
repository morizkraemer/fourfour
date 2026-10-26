<script>
  let { title = '', desc = '', dot = false, compact = false, control, wide = false } = $props();
</script>

<div class="sr" class:sr--c={compact}>
  <div class="sr__t">
    <span class="sr__title">{#if dot}<i class="sr__dot" title="Changed from default"></i>{/if}{title}</span>
    {#if desc}<span class="sr__desc">{desc}</span>{/if}
  </div>
  <div class="sr__ctl" class:sr__ctl--w={wide}>{#if dot}<button class="sr__reset" title="Reset to default (128 frames)">↺</button>{/if}{@render control?.()}</div>
</div>

<style>
  .sr { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 16px; align-items: center; padding: 12px 16px; border-bottom: 1px solid var(--ff-border); font-family: var(--ff-font); }
  .sr:last-child { border-bottom: none; }
  .sr--c { padding: 9px 16px; }
  .sr__t { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .sr__title { font-size: 13px; font-weight: 500; color: var(--ff-text); display: flex; align-items: center; gap: 6px; }
  .sr__dot { width: 5px; height: 5px; border-radius: 50%; background: var(--ff-text-mid); }
  .sr__desc { font-size: 11.5px; color: var(--ff-muted); line-height: 1.4; }
  .sr__ctl { justify-self: end; display: flex; align-items: center; gap: 8px; width: 220px; justify-content: flex-end; }
  .sr__ctl :global(.ff-dropdown) { width: 100%; }
  .sr__ctl--w { width: 280px; }
  .sr__reset { border: none; background: none; color: var(--ff-muted); font-size: 14px; padding: 2px 4px; }
</style>
