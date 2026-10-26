<script>
  let { value = 0, size = 14, hover = 0 } = $props();
</script>

<span class="stars" style="gap:{size > 12 ? 3 : 2}px">
  {#each [1, 2, 3, 4, 5] as n}
    <svg viewBox="0 0 24 24" width={size} height={size} class:on={n <= (hover || value)} class:ghost={hover && n > value && n <= hover}>
      <path d="M12 2l3.1 6.3 6.9 1-5 4.9 1.2 6.8L12 17.8 5.8 21l1.2-6.8-5-4.9 6.9-1z" />
    </svg>
  {/each}
</span>

<style>
  .stars { display: inline-flex; }
  svg { fill: none; stroke: var(--ff-faint); stroke-width: 1.6; stroke-linejoin: round; }
  svg.on { fill: var(--ff-text); stroke: var(--ff-text); }
  svg.ghost { fill: var(--ff-text-mid); stroke: var(--ff-text-mid); }
</style>
