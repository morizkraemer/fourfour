<script>
  import { Button } from '../../../../viewport/design-system/svelte.js';
  import { SHORTCUT_GROUPS } from './w2-shortcuts.js';

  let { layout = 'now', groups = null, recordingId = null, conflictId = null, filter = '' } = $props();

  let list = $derived(
    (groups ? SHORTCUT_GROUPS.filter((g) => groups.includes(g.group)) : SHORTCUT_GROUPS)
      .map((g) => ({ ...g, actions: g.actions.filter((a) => !filter || a.label.toLowerCase().includes(filter.toLowerCase())) }))
      .filter((g) => g.actions.length)
  );
</script>

{#if layout === 'now'}
  <div class="sn">
    <div class="sn__bar">
      <p class="sn__note">
        Click a shortcut, then press the new keys. Esc cancels, Backspace clears.
        Tab and Shift+Tab keep their normal meaning in text fields, dialogs and Preferences.
        Everywhere else use F6 and Shift+F6 to move focus between the sidebar, search, tracks, drawer and player.
      </p>
      <Button label="Reset all" size="small" />
    </div>
    {#each list as g}
      <h3 class="sn__group">{g.group}</h3>
      <div class="ff-settings-page__fields-container">
        {#each g.actions as a}
          <div class="ff-settings-page__field">
            <div class="ff-settings-page__field-texts"><span class="ff-settings-page__field-label">{a.label}</span></div>
            <div class="ff-settings-page__field-control sn__ctl">
              {#if a.custom}<Button label="Reset" size="small" variant="ghost" />{/if}
              <Button label={a.binding ?? 'Unassigned'} size="small" />
            </div>
          </div>
        {/each}
      </div>
    {/each}
  </div>
{:else}
  <div class="sc">
    {#each list as g}
      <div class="sc__g">
        <div class="sc__gh">{g.group}</div>
        {#each g.actions as a (a.id)}
          <div class="sc__r" class:sc__r--rec={recordingId === a.id} class:sc__r--bad={conflictId === a.id}>
            <span class="sc__l">{a.label}</span>
            {#if conflictId === a.id}
              <span class="sc__warn">Already used by “Search tracks”</span>
              <button class="sc__b">Replace</button><button class="sc__b sc__b--g">Cancel</button>
            {:else}
              {#if a.custom}<button class="sc__undo" title="Reset to default">↺</button>{/if}
              <span class="sc__k" class:sc__k--none={!a.binding} class:sc__k--rec={recordingId === a.id}>{recordingId === a.id ? 'Press keys…' : (a.binding ?? 'Unassigned')}</span>
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  .sn { display: flex; flex-direction: column; gap: var(--ff-space-4); }
  .sn__bar { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--ff-space-7); }
  .sn__note { margin: 0; max-width: 520px; font-family: var(--ff-font); font-size: var(--ff-type-caption); line-height: 1.5; color: var(--ff-muted); }
  .sn__group { margin: var(--ff-space-6) 0 0; font-family: var(--ff-font-mono); font-size: var(--ff-type-label); font-weight: 500; letter-spacing: 0.06em; text-transform: uppercase; color: var(--ff-muted); }
  .sn__ctl { display: flex; align-items: center; gap: var(--ff-space-3); }

  .sc { font-family: var(--ff-font); }
  .sc__g { break-inside: avoid; margin-bottom: 14px; }
  .sc__gh { font-family: var(--ff-font-mono); font-size: var(--ff-type-label); letter-spacing: 0.06em; text-transform: uppercase; color: var(--ff-muted); padding: 0 0 5px; border-bottom: 1px solid var(--ff-border); margin-bottom: 2px; }
  .sc__r { display: flex; align-items: center; gap: 8px; height: 26px; font-size: 12px; color: var(--ff-text-mid); }
  .sc__l { flex: 1 1 0; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sc__k { flex: none; min-width: 44px; text-align: center; padding: 2px 8px; border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-md); background: var(--ff-bg); color: var(--ff-text); font-family: var(--ff-font-mono); font-size: 11px; }
  .sc__k--none { color: var(--ff-faint); border-style: dashed; font-family: var(--ff-font); }
  .sc__k--rec { border-color: rgba(255, 255, 255, 0.5); color: var(--ff-text); background: var(--ff-elev); }
  .sc__r--bad { height: auto; min-height: 26px; flex-wrap: wrap; padding: 4px 0; }
  .sc__warn { flex: none; color: var(--ff-danger); font-size: 11px; }
  .sc__b { flex: none; height: 22px; padding: 0 8px; border: 1px solid var(--ff-border-hi); background: var(--ff-elev); color: var(--ff-text); border-radius: var(--ff-radius-md); font-size: 11px; font-family: var(--ff-font); }
  .sc__b--g { background: none; border-color: transparent; color: var(--ff-text-mid); }
  .sc__undo { flex: none; border: none; background: none; color: var(--ff-muted); font-size: 12px; padding: 0 2px; }
</style>
