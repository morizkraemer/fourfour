<!--
  ShortcutsSettings.svelte — "Keyboard shortcuts" page of Preferences.
  Lists every registry action by group. Click a binding, press the new keys;
  Esc cancels, Backspace clears. Conflicts are explained and need an explicit Replace.
-->
<script>
  import { Button } from '$ds';
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

  let groups = $derived(shortcuts.groups());
</script>

<div class="ff-shortcuts">
  <div class="ff-shortcuts__bar">
    <p class="ff-shortcuts__note">
      Click a shortcut, then press the new keys. Esc cancels, Backspace clears.
      Tab and Shift+Tab keep their normal meaning in text fields, dialogs and Preferences.
      Everywhere else use F6 and Shift+F6 to move focus between the sidebar, search, tracks, drawer and player.
    </p>
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

  {#each groups as { group, actions } (group)}
    <h3 class="ff-shortcuts__group">{group}</h3>
    <div class="ff-settings-page__fields-container">
      {#each actions as action (action.id)}
        <div class="ff-settings-page__field" data-shortcut-id={action.id}>
          <div class="ff-settings-page__field-texts">
            <span class="ff-settings-page__field-label">{action.label}</span>
            {#if conflict?.id === action.id}
              <span class="ff-shortcuts__conflict" role="alert">{conflict.message}</span>
            {/if}
          </div>
          <div class="ff-settings-page__field-control ff-shortcuts__controls">
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
  {/each}
</div>

<style>
  .ff-shortcuts {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }

  .ff-shortcuts__bar {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--ff-space-7);
  }

  .ff-shortcuts__note {
    margin: 0;
    max-width: 520px;
    font-family: var(--ff-font);
    font-size: var(--ff-type-caption);
    line-height: 1.5;
    color: var(--ff-muted);
  }

  .ff-shortcuts__group {
    margin: var(--ff-space-6) 0 0;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ff-muted);
  }

  .ff-shortcuts__controls {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
  }

  .ff-shortcuts__conflict {
    font-family: var(--ff-font);
    font-size: var(--ff-type-caption);
    color: var(--ff-danger);
  }
</style>
