export function closeOnEscape(event, onClose) {
  if (event.key !== 'Escape') return false;
  event.preventDefault();
  event.stopPropagation();
  onClose?.();
  return true;
}
