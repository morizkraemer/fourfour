const NON_TEXT_INPUT_TYPES = new Set(['button', 'checkbox', 'radio', 'range', 'submit', 'reset', 'file', 'image', 'color']);

interface FieldLike {
  tagName: string;
  type?: string;
}

interface TargetLike {
  closest?: (selector: string) => FieldLike | null;
}

/** Elements the platform context menu is useful on: it carries cut, copy, paste and spelling. */
export function isTextEntry(target: unknown): boolean {
  const field = (target as TargetLike | null)?.closest?.('input, textarea, [contenteditable="true"]');
  if (!field) return false;
  if (field.tagName === 'INPUT') return !NON_TEXT_INPUT_TYPES.has(field.type ?? 'text');
  return true;
}

/** Keep the webview's own menu (Reload, Inspect Element) from ever opening outside text fields. */
export function suppressNativeContextMenu(root: Document = document): () => void {
  const onContextMenu = (event: MouseEvent) => {
    if (!isTextEntry(event.target)) event.preventDefault();
  };
  root.addEventListener('contextmenu', onContextMenu);
  return () => root.removeEventListener('contextmenu', onContextMenu);
}
