<script>
  import './resize-handle.css';

  let {
    edge = 'right',
    value,
    min,
    max,
    label = 'Resize panel',
    onResize,
    onCommit,
    onReset,
  } = $props();

  let dragging = $state(false);

  const KEY_STEP = 16;
  const GROWTH_PER_POINTER_PX = { right: 1, left: -1, top: -1 };
  const vertical = $derived(edge === 'top');

  function onPointerDown(e) {
    if (e.button !== 0) return;
    e.preventDefault();
    const isVertical = vertical;
    const start = isVertical ? e.clientY : e.clientX;
    const startValue = value;
    const sign = GROWTH_PER_POINTER_PX[edge];
    const activeClass = isVertical ? 'ff-resize--active-row' : 'ff-resize--active';
    e.currentTarget.setPointerCapture?.(e.pointerId);
    dragging = true;
    document.body.classList.add(activeClass);

    function onMove(ev) {
      onResize?.(startValue + sign * ((isVertical ? ev.clientY : ev.clientX) - start));
    }
    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      document.body.classList.remove(activeClass);
      dragging = false;
      onCommit?.();
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  function onKeyDown(e) {
    const growKey = vertical ? 'ArrowUp' : edge === 'right' ? 'ArrowRight' : 'ArrowLeft';
    const shrinkKey = vertical ? 'ArrowDown' : edge === 'right' ? 'ArrowLeft' : 'ArrowRight';
    const dir = e.key === growKey ? 1 : e.key === shrinkKey ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    onResize?.(value + dir * KEY_STEP);
    onCommit?.();
  }
</script>

<div
  class="ff-resize ff-resize--{edge}"
  class:ff-resize--dragging={dragging}
  role="slider"
  aria-orientation={vertical ? 'horizontal' : 'vertical'}
  aria-label={label}
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  title="Drag to resize, double-click to reset"
  onpointerdown={onPointerDown}
  onkeydown={onKeyDown}
  ondblclick={() => onReset?.()}
></div>
