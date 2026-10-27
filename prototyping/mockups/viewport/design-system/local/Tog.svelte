<script>
  let { on = true } = $props();
</script>

<span class="tog" class:tog--on={on} role="switch" aria-checked={on}><i></i></span>

<style>
  .tog { display: inline-block; width: 30px; height: 17px; border-radius: 4px; background: var(--ff-bg); border: 1px solid var(--ff-border-hi); position: relative; flex: none; vertical-align: middle; }
  .tog i { position: absolute; top: 2px; left: 2px; width: 11px; height: 11px; border-radius: 2px; background: var(--ff-muted); transition: left var(--ff-motion-default) var(--ff-easing); }
  .tog--on { background: var(--ff-accent); border-color: var(--ff-accent); }
  .tog--on i { left: 15px; background: #0a0a0a; }
</style>
