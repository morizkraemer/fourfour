const TABBABLE =
  'input:not([disabled]), button:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])';

export function trapTab(e, container) {
  if (e.key !== 'Tab' || e.defaultPrevented) return;
  const stops = [...container.querySelectorAll(TABBABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement);
  if (stops.length === 0) {
    e.preventDefault();
    container.focus();
    return;
  }
  const first = stops[0];
  const last = stops[stops.length - 1];
  const active = document.activeElement;
  if (e.shiftKey && (active === first || active === container)) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && active === last) {
    e.preventDefault();
    first.focus();
  } else if (!container.contains(active)) {
    e.preventDefault();
    first.focus();
  }
}

const POINTER_FOCUS_WINDOW_MS = 300;

export function trackFocusModality() {
  const root = document.documentElement;
  let lastPointerAt = -Infinity;
  const onPointerDown = () => (lastPointerAt = performance.now());
  const onFocusIn = () => {
    root.dataset.focusModality = performance.now() - lastPointerAt > POINTER_FOCUS_WINDOW_MS ? 'keyboard' : 'pointer';
  };
  window.addEventListener('pointerdown', onPointerDown, true);
  window.addEventListener('focusin', onFocusIn, true);
  return () => {
    window.removeEventListener('pointerdown', onPointerDown, true);
    window.removeEventListener('focusin', onFocusIn, true);
    delete root.dataset.focusModality;
  };
}

export function isKeyboardFocused(element) {
  if (!(element instanceof HTMLElement) || element === document.body || element !== document.activeElement) return false;
  const modality = document.documentElement.dataset.focusModality;
  return modality ? modality === 'keyboard' : element.matches(':focus-visible');
}
