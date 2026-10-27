<!--
  ShortcutsSettings.svelte — "Keyboard" page of Preferences.
  Lists every registry action by group, filterable, in two columns. Click a binding, press the new keys;
  Esc cancels, Backspace clears. Conflicts are explained and need an explicit Replace.
-->
<script>
  import { Button, FilterInput, SegmentedControl } from '$ds';
  import { shortcuts } from '../shortcuts/index.ts';
  import { bindingFromEvent, formatBinding } from '../shortcuts/keybinding.ts';

  let capturingId = $state(null);
  let conflict = $state(null);

  function stopCapture() {
    capturingId = null;
  }

  function startCapture(id) {
    conflict = null;
    capturingId = id;
  }

  function apply(id, binding, replace = false) {
    const result = shortcuts.rebind(id, binding, { replace });
    if (result.ok) {
      conflict = null;
      return;
    }
    conflict = { id, binding, message: result.message };
  }

  $effect(() => {
    const id = capturingId;
    if (!id) return;
    function onKey(e) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Escape') {
        stopCapture();
        return;
      }
      if (e.key === 'Backspace' || e.key === 'Delete') {
        shortcuts.unbind(id);
        conflict = null;
        stopCapture();
        return;
      }
      const binding = bindingFromEvent(e);
      if (!binding) return;
      stopCapture();
      apply(id, binding);
    }
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  });

  function bindingLabel(id) {
    const binding = shortcuts.bindingOf(id);
    return binding ? formatBinding(binding) : 'Unassigned';
  }

  let filter = $state('');
  let groupIndex = $state(0);

  let allGroups = $derived(shortcuts.groups());
  let groupNames = $derived(['All', ...allGroups.map((g) => g.group)]);
  let visibleGroups = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const only = groupIndex > 0 ? groupNames[groupIndex] : null;
    return allGroups
      .filter((g) => !only || g.group === only)
      .map((g) => ({
        group: g.group,
        actions: q ? g.actions.filter((a) => a.label.toLowerCase().includes(q)) : g.actions,
      }))
      .filter((g) => g.actions.length > 0);
  });
  let total = $derived(allGroups.reduce((n, g) => n + g.actions.length, 0));
</script>

<div class="ff-shortcuts">
  <div class="ff-shortcuts__bar">
    <div class="ff-shortcuts__filter"><FilterInput bind:value={filter} placeholder="Filter {total} shortcuts…" /></div>
    <Button
      label="Reset all"
      size="small"
      disabled={!shortcuts.hasCustom()}
      onclick={() => {
        conflict = null;
        shortcuts.resetAll();
      }}
    />
  </div>
  <SegmentedControl items={groupNames} activeIndex={groupIndex} onChange={(index) => (groupIndex = index)} />
  <p class="ff-shortcuts__note">
    Click a shortcut, then press the new keys. Esc cancels, Backspace clears. F6 and Shift+F6 move focus between regions.
  </p>

  <div class="ff-shortcuts__columns">
    {#each visibleGroups as { group, actions } (group)}
      <section class="ff-shortcuts__group">
        <h3 class="ff-shortcuts__group-title">{group}</h3>
        <div class="ff-settings-card">
          {#each actions as action (action.id)}
            <div class="ff-setting-row ff-shortcuts__row" data-shortcut-id={action.id}>
              <div class="ff-setting-row__texts">
                <span class="ff-shortcuts__label">{action.label}</span>
                {#if conflict?.id === action.id}
                  <span class="ff-shortcuts__conflict" role="alert">{conflict.message}</span>
                {/if}
              </div>
              <div class="ff-shortcuts__controls">
                {#if conflict?.id === action.id}
                  <Button label="Replace" size="small" onclick={() => apply(action.id, conflict.binding, true)} />
                  <Button label="Cancel" size="small" variant="ghost" onclick={() => (conflict = null)} />
                {:else}
                  {#if shortcuts.isCustom(action.id)}
                    <Button label="Reset" size="small" variant="ghost" onclick={() => shortcuts.reset(action.id)} />
                  {/if}
                  <Button
                    label={capturingId === action.id ? 'Press keys…' : bindingLabel(action.id)}
                    size="small"
                    onclick={() => (capturingId === action.id ? stopCapture() : startCapture(action.id))}
                  />
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </section>
    {:else}
      <p class="ff-shortcuts__note">No shortcut matches “{filter}”.</p>
    {/each}
  </div>
</div>

<style>
  .ff-shortcuts {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-3);
  }

  .ff-shortcuts__bar {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
  }

  .ff-shortcuts__filter {
    flex: 1 1 0;
  }

  .ff-shortcuts__note {
    margin: 0;
    font-family: var(--ff-font);
    font-size: var(--ff-type-caption);
    line-height: 1.5;
    color: var(--ff-muted);
  }

  .ff-shortcuts__columns {
    column-count: 2;
    column-gap: 20px;
  }

  .ff-shortcuts__group {
    break-inside: avoid;
    margin-bottom: var(--ff-space-5);
  }

  .ff-shortcuts__group-title {
    margin: 0 0 var(--ff-space-2);
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ff-muted);
  }

  .ff-shortcuts__row {
    gap: var(--ff-space-3);
    padding: 6px 10px;
  }

  .ff-shortcuts__label {
    font-family: var(--ff-font);
    font-size: 12px;
    color: var(--ff-text);
  }

  .ff-shortcuts__controls {
    display: flex;
    align-items: center;
    gap: var(--ff-space-2);
  }

  .ff-shortcuts__conflict {
    font-family: var(--ff-font);
    font-size: var(--ff-type-caption);
    color: var(--ff-danger);
  }
</style>
