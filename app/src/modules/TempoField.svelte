<!--
  TempoField.svelte — one value of the player's tempo bar: − value + with a fixed width.

  Click the value to type over it (Enter or blur commits, Esc cancels). Drag it up or down to change
  it; the pointer is locked while dragging so the drag runs past the screen edge. Alt-click or
  double-click resets it. Alt makes − / + and drags fine, Shift makes drags coarse.
-->
<script lang="ts">
  import { Icon } from '$ds';
  import { tierOf, type Tier } from '../lib/tempo.ts';

  const DRAG_THRESHOLD_PX = 3;

  let {
    label,
    text,
    width,
    pxPerUnit,
    color = undefined,
    disabled = false,
    stepper = true,
    editText = text,
    onStep,
    onSet,
    onReset,
  }: {
    label: string;
    text: string;
    width: string;
    pxPerUnit: number;
    color?: string;
    disabled?: boolean;
    stepper?: boolean;
    editText?: string;
    onStep: (units: number, tier: Tier, fromButton: boolean) => void;
    onSet: (typed: string) => boolean;
    onReset: () => void;
  } = $props();

  let editing = $state(false);
  let draft = $state('');
  let invalid = $state(false);
  let dragging = $state(false);
  let valueEl: HTMLElement | undefined = $state();
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (editing && inputEl) {
      inputEl.focus();
      inputEl.select();
    }
  });

  function startEditing() {
    draft = editText;
    invalid = false;
    editing = true;
  }

  function finishEditing(commit: boolean) {
    if (!editing) return;
    if (commit && draft.trim() !== '' && !onSet(draft)) {
      invalid = true;
      return;
    }
    editing = false;
  }

  function onInputKeyDown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Enter') {
      event.preventDefault();
      finishEditing(true);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      finishEditing(false);
    }
  }

  function onInputBlur() {
    if (!editing) return;
    if (draft.trim() !== '') onSet(draft);
    editing = false;
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0 || disabled || editing) return;
    event.preventDefault();
    if (event.altKey) {
      onReset();
      return;
    }
    let lastY = event.clientY;
    let travelled = 0;
    let accumulated = 0;
    let locked = false;

    function onMove(move: PointerEvent) {
      const movement = move.movementY || move.clientY - lastY;
      lastY = move.clientY;
      travelled += Math.abs(movement);
      if (!dragging) {
        if (travelled < DRAG_THRESHOLD_PX) return;
        dragging = true;
        document.body.classList.add('ff-tempo-dragging');
        if (!locked) {
          locked = true;
          try {
            void Promise.resolve(valueEl?.requestPointerLock()).catch(() => {});
          } catch {}
        }
      }
      accumulated -= movement;
      const units = Math.trunc(accumulated / pxPerUnit);
      if (units !== 0) {
        onStep(units, tierOf(move), false);
        accumulated -= units * pxPerUnit;
      }
    }

    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      document.body.classList.remove('ff-tempo-dragging');
      if (locked && document.pointerLockElement === valueEl) document.exitPointerLock();
      if (dragging) dragging = false;
      else startEditing();
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  function onKeyDown(event: KeyboardEvent) {
    if (editing || disabled) return;
    if (event.key === 'Enter') {
      event.preventDefault();
      startEditing();
    } else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      onStep(event.key === 'ArrowUp' ? 1 : -1, tierOf(event), true);
    }
  }

  function onDoubleClick() {
    if (disabled) return;
    editing = false;
    onReset();
  }
</script>

<div class="ff-tfield" class:ff-tfield--bare={!stepper} class:ff-tfield--disabled={disabled} role="group" aria-label={label}>
  {#if stepper}
    <button
      type="button"
      class="ff-tfield__btn"
      title={`Lower ${label}`}
      aria-label={`Lower ${label}`}
      {disabled}
      onclick={(event) => onStep(-1, event.altKey ? 'fine' : 'normal', true)}><Icon name="minus" size={11} /></button>
  {/if}
  <div
    class="ff-tfield__value"
    class:ff-tfield__value--dragging={dragging}
    style:width={width}
    bind:this={valueEl}
    role="spinbutton"
    tabindex={disabled ? -1 : 0}
    aria-label={label}
    aria-valuetext={text}
    title={disabled ? undefined : 'Click to type, drag to change, double-click or Alt-click to reset'}
    onpointerdown={onPointerDown}
    onkeydown={onKeyDown}
    ondblclick={onDoubleClick}>
    {#if editing}
      <input
        bind:this={inputEl}
        bind:value={draft}
        class="ff-tfield__input"
        class:ff-tfield__input--invalid={invalid}
        type="text"
        spellcheck="false"
        autocomplete="off"
        aria-label={label}
        style:color={color}
        onkeydown={onInputKeyDown}
        oninput={() => (invalid = false)}
        onblur={onInputBlur} />
    {:else}
      <span class="ff-tfield__text" style:color={color}>{text}</span>
    {/if}
  </div>
  {#if stepper}
    <button
      type="button"
      class="ff-tfield__btn"
      title={`Raise ${label}`}
      aria-label={`Raise ${label}`}
      {disabled}
      onclick={(event) => onStep(1, event.altKey ? 'fine' : 'normal', true)}><Icon name="plus" size={11} /></button>
  {/if}
</div>

<style>
  .ff-tfield {
    display: inline-flex;
    align-items: stretch;
    flex: none;
    height: 22px;
    box-sizing: border-box;
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-sm);
    background: var(--ff-bg);
    overflow: hidden;
    user-select: none;
  }
  .ff-tfield--disabled {
    opacity: 0.45;
  }
  .ff-tfield__btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--ff-muted);
    cursor: pointer;
  }
  .ff-tfield__btn:hover:not(:disabled) {
    background: var(--ff-hover);
    color: var(--ff-text);
  }
  .ff-tfield__btn:active:not(:disabled) {
    background: var(--ff-select);
  }
  .ff-tfield__value {
    display: flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    border-left: 1px solid var(--ff-border);
    border-right: 1px solid var(--ff-border);
    font-family: var(--ff-font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--ff-text);
    cursor: ns-resize;
    outline: none;
  }
  .ff-tfield--bare .ff-tfield__value {
    border: none;
  }
  .ff-tfield__value:hover,
  .ff-tfield__value:focus-visible {
    background: var(--ff-hover);
  }
  .ff-tfield__value--dragging {
    background: var(--ff-select);
  }
  .ff-tfield__text {
    white-space: nowrap;
  }
  .ff-tfield__input {
    width: 100%;
    height: 100%;
    min-width: 0;
    padding: 0;
    box-sizing: border-box;
    border: none;
    background: var(--ff-elev);
    box-shadow: inset 0 0 0 1px var(--ff-border-hi);
    font: inherit;
    text-align: center;
    color: var(--ff-text);
    outline: none;
  }
  .ff-tfield__input--invalid {
    box-shadow: inset 0 0 0 1px var(--ff-danger);
  }
  :global(body.ff-tempo-dragging),
  :global(body.ff-tempo-dragging *) {
    cursor: ns-resize !important;
    user-select: none !important;
  }
</style>
