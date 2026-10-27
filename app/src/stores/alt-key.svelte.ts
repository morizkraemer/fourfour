export const altKey = $state({ held: false });

if (typeof window !== 'undefined') {
  const sync = (e: KeyboardEvent) => {
    altKey.held = e.altKey;
  };
  window.addEventListener('keydown', sync, true);
  window.addEventListener('keyup', sync, true);
  window.addEventListener('blur', () => {
    altKey.held = false;
  });
}
