<script>
  import { Button } from '../../../../viewport/design-system/svelte.js';

  let { cols = 1, groups = null, filter = '', showConflict = true, compact = false } = $props();

  const ALL = [
    { name: 'Playback', rows: [
      { a: 'Play / pause', k: ['Space'] },
      { a: 'Toggle big / small player', k: ['Tab'] },
      { a: 'Close settings, clear search, collapse player', k: ['Esc'] },
    ] },
    { name: 'Library', rows: [
      { a: 'Command palette', k: ['⌘', 'K'] },
      { a: 'Search tracks', k: ['⌘', 'F'] },
      { a: 'New playlist', k: ['⌘', '⇧', 'N'] },
      { a: 'Import files', k: ['⌘', 'O'] },
      { a: 'Toggle favorite 1 to 9 on selection', k: ['1', '…', '9'] },
      { a: 'Analyze selection', k: null },
    ] },
    { name: 'Layout', rows: [
      { a: 'Toggle organize drawer', k: ['⇧', 'Tab'] },
      { a: 'Focus next region', k: ['F6'] },
      { a: 'Focus previous region', k: ['⇧', 'F6'] },
      { a: 'Toggle inspector', k: null },
      { a: 'Open settings', k: ['⌘', ','] },
    ] },
    { name: 'USB', rows: [
      { a: 'Sync stick', k: null },
    ] },
  ];
  let list = $derived(groups ? ALL.filter((g) => groups.includes(g.name)) : ALL);
</script>

<div class="st" class:st--c={compact} style="column-count:{cols}">
  {#each list as g}
    <div class="st__g">
      <div class="st__gh">{g.name}</div>
      {#each g.rows as r}
        {@const capture = showConflict && r.a === 'Search tracks'}
        {@const conflict = showConflict && r.a === 'New playlist'}
        <div class="st__r" class:st__r--cap={capture}>
          <span class="st__a">{r.a}</span>
          {#if capture}
            <span class="st__k st__k--cap">Press keys…</span>
          {:else if r.k}
            <span class="st__ks">{#each r.k as key}<kbd>{key}</kbd>{/each}</span>
          {:else}
            <span class="st__un">Unassigned</span>
          {/if}
        </div>
        {#if conflict}
          <div class="st__conf">
            <span>⌘⇧N is used by <b>Toggle favorite 9</b>.</span>
            <Button label="Replace" size="compact" />
            <Button label="Cancel" size="compact" variant="ghost" />
          </div>
        {/if}
      {/each}
    </div>
  {/each}
</div>

<style>
  .st { column-gap: 36px; font-family: var(--ff-font); }
  .st__g { break-inside: avoid; margin-bottom: 14px; }
  .st__gh { font-family: var(--ff-font-mono); font-size: var(--ff-type-label-sm); letter-spacing: 0.06em; text-transform: uppercase; color: var(--ff-faint); padding: 4px 0 6px; border-bottom: 1px solid var(--ff-border); }
  .st__r { display: flex; align-items: center; justify-content: space-between; gap: 12px; height: 32px; border-bottom: 1px solid var(--ff-border); font-size: 12.5px; color: var(--ff-text-mid); }
  .st--c .st__r { height: 28px; }
  .st__r--cap { background: var(--ff-select); margin: 0 -8px; padding: 0 8px; border-radius: 3px; border-bottom-color: transparent; color: var(--ff-text); }
  .st__a { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .st__ks { display: inline-flex; gap: 3px; }
  kbd { min-width: 22px; height: 22px; padding: 0 6px; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--ff-border-hi); border-bottom-width: 2px; border-radius: 4px; font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-text); background: var(--ff-elev); }
  .st__un { font-size: 11.5px; color: var(--ff-faint); font-style: italic; }
  .st__k--cap { padding: 3px 10px; border: 1px dashed rgba(255, 255, 255, 0.4); border-radius: 4px; font-size: 11.5px; color: var(--ff-text); }
  .st__conf { display: flex; align-items: center; gap: 8px; padding: 7px 8px; margin: 0 -8px; font-size: 11.5px; color: var(--ff-danger); background: var(--ff-danger-bg); border-radius: 3px; }
  .st__conf span { flex: 1; }
  .st__conf b { font-weight: 500; }
</style>
