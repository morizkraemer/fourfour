<script lang="ts">
  import type { LazyView } from './lazy.svelte.ts';

  let { view, needed = true, props = {} }: { view: LazyView; needed?: boolean; props?: Record<string, unknown> } = $props();

  $effect(() => {
    if (needed) void view.load();
  });
</script>

{#if view.component}
  {@const View = view.component}
  <View {...props} />
{/if}
