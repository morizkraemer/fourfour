<script>
  import Ico from './Ico.svelte';
  let { label = 'BPM', value = '125.00', state = 'rest', delta = '', color = '', big = true, extras = true, sub = '' } = $props();
</script>

<div class="scr scr--{state}" class:scr--big={big}>
  <div class="scr__label"><span>{label}</span>{#if sub && state !== 'drag'}<span class="scr__sub">{sub}</span>{/if}</div>
  <div class="scr__box">
    <span class="scr__arr"><Ico name="chevron-left" size={12} /></span>
    <span class="scr__val" style={color ? `color:${color}` : ''}>{value}</span>
    <span class="scr__arr"><Ico name="chevron-right" size={12} /></span>
    {#if state === 'drag'}
      <span class="scr__delta">{delta}</span>
    {/if}
  </div>
  {#if state === 'drag'}
    <div class="scr__ruler">
      {#each Array(31) as _, i}<i class:major={i % 5 === 0}></i>{/each}
      <b></b>
    </div>
  {:else if extras}
    <div class="scr__hint">drag · ↑↓ · type</div>
  {/if}
</div>

<style>
  .scr { position: relative; display: flex; flex-direction: column; gap: 3px; min-width: 0; flex: 1 1 0; }
  .scr__label { display: flex; justify-content: space-between; font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; color: var(--ff-faint); }
  .scr__sub { color: var(--ff-muted); letter-spacing: 0; }
  .scr__box { position: relative; display: flex; align-items: center; justify-content: space-between; height: 32px; padding: 0 6px; border: 1px solid var(--ff-border); background: var(--ff-bg); border-radius: var(--ff-radius-md); cursor: ew-resize; }
  .scr--big .scr__box { height: 38px; }
  .scr__val { font-family: var(--ff-font-mono); font-size: 15px; font-weight: 500; color: var(--ff-text); }
  .scr--big .scr__val { font-size: 18px; }
  .scr__arr { color: var(--ff-faint); display: flex; opacity: 0.5; }
  .scr--hover .scr__box, .scr--drag .scr__box { border-color: var(--ff-border-hi); background: var(--ff-elev); }
  .scr--hover .scr__arr, .scr--drag .scr__arr { opacity: 1; color: var(--ff-text-mid); }
  .scr--drag .scr__box { border-color: rgba(255, 255, 255, 0.34); }
  .scr__delta { position: absolute; top: -26px; left: 50%; transform: translateX(-50%); padding: 3px 7px; border-radius: 4px; background: var(--ff-accent); color: #0a0a0a; font-family: var(--ff-font-mono); font-size: 11px; font-weight: 600; white-space: nowrap; z-index: 5; }
  .scr__hint { font-size: 10px; color: var(--ff-faint); text-align: center; }
  .scr__ruler { position: relative; height: 10px; display: flex; justify-content: space-between; align-items: flex-end; padding: 0 4px; }
  .scr__ruler i { width: 1px; height: 4px; background: var(--ff-faint); display: block; }
  .scr__ruler i.major { height: 8px; background: var(--ff-text-mid); }
  .scr__ruler b { position: absolute; left: 50%; bottom: 0; width: 2px; height: 10px; background: var(--ff-accent); margin-left: -1px; }
</style>
