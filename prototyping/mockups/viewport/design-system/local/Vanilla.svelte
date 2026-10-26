<script>
  let { make } = $props();
  let host = $state(null);

  $effect(() => {
    if (!host) return;
    host.replaceChildren(make().element);
  });
</script>

<span bind:this={host} class="vn"></span>

<style>
  .vn { display: inline-flex; align-items: center; }
</style>
