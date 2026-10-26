export type Binding = string;

export interface KeyEventLike {
  key: string;
  code?: string;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
}

const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock']);

const CODE_TO_KEY: Record<string, string> = {
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backslash: '\\',
  Minus: '-',
  Equal: '=',
  Semicolon: ';',
  Quote: "'",
  Backquote: '`',
  BracketLeft: '[',
  BracketRight: ']',
};

const DISPLAY_KEY: Record<string, string> = {
  Escape: 'Esc',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Enter: '⏎',
  Backspace: '⌫',
  Delete: '⌦',
};

function keyName(e: KeyEventLike): string {
  const code = e.code ?? '';
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (code in CODE_TO_KEY) return CODE_TO_KEY[code];
  if (e.key === ' ' || code === 'Space') return 'Space';
  return e.key.length === 1 ? e.key.toUpperCase() : e.key;
}

/** Canonical binding such as `Mod+Shift+K`, or null for a bare modifier press. */
export function bindingFromEvent(e: KeyEventLike): Binding | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push('Mod');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  parts.push(keyName(e));
  return parts.join('+');
}

export function isPlatformMac(): boolean {
  return typeof navigator !== 'undefined' && /mac/i.test(navigator.platform);
}

/** Display parts for one binding, e.g. ['⌘', 'K'] on macOS or ['Ctrl', 'K'] elsewhere. */
export function bindingParts(binding: Binding | null, mac: boolean = isPlatformMac()): string[] {
  if (!binding) return [];
  const parts = binding.split('+');
  const key = parts.pop() as string;
  const mods = parts.map((m) => {
    if (m === 'Mod') return mac ? '⌘' : 'Ctrl';
    if (m === 'Alt') return mac ? '⌥' : 'Alt';
    return mac ? '⇧' : 'Shift';
  });
  return [...mods, DISPLAY_KEY[key] ?? key];
}

export function formatBinding(binding: Binding | null, mac: boolean = isPlatformMac()): string {
  return bindingParts(binding, mac).join(mac ? '' : '+');
}

/** Whether a binding is safe to fire while the user is typing in a text field. */
export function firesWhileTyping(binding: Binding): boolean {
  const parts = binding.split('+');
  const key = parts[parts.length - 1];
  return parts.includes('Mod') || key === 'Escape' || /^F\d{1,2}$/.test(key);
}
