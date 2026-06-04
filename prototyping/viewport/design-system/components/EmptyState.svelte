<script>
  import './empty-state.css';

  let { title = '', sub = '', children } = $props();
</script>

<div class="ff-empty-state">
  {#if title}
    <p class="ff-empty-state__title">{title}</p>
  {/if}
  {#if sub}
    <p class="ff-empty-state__sub">{sub}</p>
  {/if}
  {@render children?.()}
</div>
