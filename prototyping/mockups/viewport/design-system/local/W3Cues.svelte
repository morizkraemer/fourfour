<script>
  import { GhostField } from '../../../../viewport/design-system/svelte.js';
  import { ALL_CUES } from './w2-data.js';

  let { hoverKey = 'h3' } = $props();
</script>

<div class="cl">
  {#each ALL_CUES as c (c.key)}
    <div class="cl__row">
      <span class="cl__l" style="color:{c.color}">{c.letter}</span>
      <GhostField value={c.name} placeholder="Unnamed" label="Cue name" state={c.key === hoverKey ? 'hover' : 'rest'} />
      <GhostField value={c.time} mono align="right" label="Cue time" />
    </div>
  {/each}
  <button class="cl__add">+ Add at playhead</button>
</div>

<style>
  .cl { display: flex; flex-direction: column; }
  .cl__row { display: grid; grid-template-columns: 16px minmax(0, 1fr) 56px; align-items: center; gap: 4px; }
  .cl__l { font-family: var(--ff-font-mono); font-size: 11px; font-weight: 600; text-align: center; }
  .cl__add { align-self: flex-start; margin: 4px 0 0 20px; padding: 0; border: none; background: none; color: var(--ff-muted); font-family: var(--ff-font); font-size: 11.5px; }
</style>
