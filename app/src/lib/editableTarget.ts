interface ElementLike {
  tagName?: string;
  closest?(selector: string): unknown;
}

const TYPING_TAGS = ['INPUT', 'TEXTAREA', 'SELECT'];

export function isEditableTarget(target: unknown): boolean {
  const element = target as ElementLike | null;
  if (!element || typeof element.closest !== 'function') return false;
  if (TYPING_TAGS.includes(element.tagName ?? '')) return true;
  if (element.closest('[contenteditable=""], [contenteditable="true"], [contenteditable="plaintext-only"]')) return true;
  if (element.closest('.ff-settings-screen')) return true;
  return !!element.closest('.ff-dialog-wrapper');
}

export function isKeyboardFocusedControl(target: unknown): boolean {
  const element = target as { matches?(selector: string): boolean } | null;
  return typeof element?.matches === 'function' && element.matches(':focus-visible');
}
