<script>
  import './scrub-value.css';
  import ContextMenu from './ContextMenu.svelte';

  const DRAG_THRESHOLD_PX = 3;
  const WHEEL_NOTCH = 40;
  const COMMIT_DELAY_MS = 350;

  /**
   * @type {{
   *   model: any,
   *   value: number | string,
   *   onCommit: (next: any) => void,
   *   label: string,
   *   color?: string,
   *   disabled?: boolean,
   *   extraItems?: Array<{ label: string, disabled?: boolean, onClick: () => void }>,
   * }}
   */
  let { model, value, onCommit, label, color = undefined, disabled = false, extraItems = [] } = $props();

  let draft = $state(null);
  let dragging = $state(false);
  let editing = $state(false);
  let text = $state('');
  let invalid = $state(false);
  let menu = $state({ open: false, x: 0, y: 0 });
  let el = $state(null);
  let inputEl = $state(null);

  let commitTimer = 0;
  let wheelAcc = 0;

  let shown = $derived(draft ?? value);
  let hasValue = $derived(model.valid(shown));
  let interactive = $derived(!disabled && hasValue);

  function commitNow() {
    clearTimeout(commitTimer);
    const next = draft;
    draft = null;
    if (next !== null && next !== value) onCommit(next);
  }

  function commitSoon() {
    clearTimeout(commitTimer);
    commitTimer = setTimeout(commitNow, COMMIT_DELAY_MS);
  }

  function step(units, event, forcedTier) {
    const tier = forcedTier ?? model.tier(event);
    draft = model.step(shown, units, tier);
  }

  function applyAction(action) {
    const next = action.apply(shown);
    if (next === null) return;
    draft = next;
    commitNow();
  }

  function onPointerDown(event) {
    if (event.button !== 0 || !interactive || editing) return;
    event.preventDefault();
    el?.focus({ preventScroll: true });
    let acc = 0;
    let travelled = 0;
    let lastY = event.clientY;

    function onMove(ev) {
      const movement = ev.clientY - lastY;
      lastY = ev.clientY;
      travelled += Math.abs(movement);
      if (!dragging) {
        if (travelled < DRAG_THRESHOLD_PX) return;
        dragging = true;
        document.body.classList.add('ff-scrub-dragging');
      }
      acc -= movement;
      const units = Math.trunc(acc / model.pxPerUnit);
      if (units !== 0) {
        step(units, ev);
        acc -= units * model.pxPerUnit;
      }
    }

    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      document.body.classList.remove('ff-scrub-dragging');
      if (dragging) {
        dragging = false;
        commitNow();
      }
    }

    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  function onKeyDown(event) {
    if (editing || !interactive) return;
    if (event.key === 'Enter' || event.key === 'F2') {
      event.preventDefault();
      startEditing();
      return;
    }
    if (event.metaKey || event.ctrlKey) {
      const action = model.actions.find((candidate) => candidate.hotkey === event.key);
      if (action) {
        event.preventDefault();
        applyAction(action);
      }
      return;
    }
    const direction = event.key === 'ArrowUp' || event.key === 'PageUp' ? 1 : event.key === 'ArrowDown' || event.key === 'PageDown' ? -1 : 0;
    if (direction === 0) return;
    event.preventDefault();
    step(direction, event, event.key.startsWith('Page') ? 'coarse' : undefined);
    commitSoon();
  }

  function onWheel(event) {
    if (editing || !interactive || document.activeElement !== el) return;
    event.preventDefault();
    wheelAcc -= event.deltaY;
    const units = Math.trunc(wheelAcc / WHEEL_NOTCH);
    if (units === 0) return;
    wheelAcc -= units * WHEEL_NOTCH;
    step(units, event);
    commitSoon();
  }

  $effect(() => {
    const node = el;
    if (!node) return;
    node.addEventListener('wheel', onWheel, { passive: false });
    return () => node.removeEventListener('wheel', onWheel);
  });

  function startEditing() {
    if (!interactive) return;
    commitNow();
    text = model.editText(shown);
    invalid = false;
    editing = true;
    queueMicrotask(() => inputEl?.select());
  }

  function finishEditing(commit) {
    if (!editing) return;
    if (commit) {
      const parsed = model.parse(text);
      if (parsed === null) {
        invalid = true;
        return;
      }
      editing = false;
      draft = parsed;
      commitNow();
    } else {
      editing = false;
    }
    queueMicrotask(() => el?.focus({ preventScroll: true }));
  }

  function onInputKeyDown(event) {
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
    if (model.parse(text) === null) editing = false;
    else finishEditing(true);
  }

  function onContextMenu(event) {
    if (!interactive) return;
    event.preventDefault();
    menu = { open: true, x: event.clientX, y: event.clientY };
  }

  let menuItems = $derived([
    ...model.actions.map((action) => ({
      label: action.label,
      shortcut: action.shortcut,
      disabled: action.apply(shown) === null,
      onClick: () => applyAction(action),
    })),
    ...(extraItems.length ? [{ isSeparator: true }, ...extraItems] : []),
  ]);
</script>

{#if editing}
  <input
    bind:this={inputEl}
    bind:value={text}
    class="ff-scrub__input"
    class:ff-scrub__input--invalid={invalid}
    type="text"
    inputmode="decimal"
    spellcheck="false"
    autocomplete="off"
    aria-label={label}
    aria-invalid={invalid}
    onkeydown={onInputKeyDown}
    oninput={() => (invalid = false)}
    onblur={onInputBlur}
  />
{:else}
  <span
    bind:this={el}
    class="ff-scrub"
    class:ff-scrub--interactive={interactive}
    class:ff-scrub--dragging={dragging}
    class:ff-scrub--pending={draft !== null}
    style:color={hasValue ? color : undefined}
    role="spinbutton"
    tabindex={interactive ? 0 : -1}
    aria-label={label}
    aria-valuemin={model.min}
    aria-valuemax={model.max}
    aria-valuenow={hasValue ? model.ariaNow(shown) : undefined}
    aria-valuetext={hasValue ? model.ariaText(shown) : 'not analyzed'}
    aria-disabled={!interactive}
    title={interactive ? model.hint : undefined}
    onpointerdown={onPointerDown}
    onkeydown={onKeyDown}
    ondblclick={startEditing}
    oncontextmenu={onContextMenu}
    onblur={commitNow}
  >
    <span class="ff-scrub__text">{hasValue ? model.format(shown) : '—'}</span>
    {#if interactive}
      <svg class="ff-scrub__grip" width="6" height="10" viewBox="0 0 6 10" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M1 3.5 3 1.5 5 3.5M1 6.5 3 8.5 5 6.5" />
      </svg>
    {/if}
  </span>
{/if}

<ContextMenu open={menu.open} x={menu.x} y={menu.y} items={menuItems} onClose={() => (menu = { ...menu, open: false })} />
