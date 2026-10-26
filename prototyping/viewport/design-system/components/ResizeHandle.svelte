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

  function onPointerDown(e) {
    if (e.button !== 0) return;
    e.preventDefault();
    const startX = e.clientX;
    const startValue = value;
    const sign = edge === 'right' ? 1 : -1;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    dragging = true;
    document.body.classList.add('ff-resize--active');

    function onMove(ev) {
      onResize?.(startValue + sign * (ev.clientX - startX));
    }
    function onUp() {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      document.body.classList.remove('ff-resize--active');
      dragging = false;
      onCommit?.();
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  }

  function onKeyDown(e) {
    const dir = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    const sign = edge === 'right' ? 1 : -1;
    onResize?.(value + sign * dir * KEY_STEP);
    onCommit?.();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="ff-resize ff-resize--{edge}"
  class:ff-resize--dragging={dragging}
  role="separator"
  aria-orientation="vertical"
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
