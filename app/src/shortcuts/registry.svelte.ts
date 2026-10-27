import { bindingFromEvent, firesWhileTyping, formatBinding, type Binding, type KeyEventLike } from './keybinding.ts';

export interface ShortcutAction {
  id: string;
  label: string;
  group: string;
  defaultBinding: Binding | null;
  run: () => void;
  when?: () => boolean;
  inInput?: boolean;
  palette?: boolean;
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export type RebindResult =
  | { ok: true }
  | { ok: false; conflict: ShortcutAction; message: string };

export const SHORTCUT_STORAGE_KEY = 'fourfour.shortcuts.v1';

type Overrides = Record<string, Binding | null>;

function loadOverrides(storage: StorageLike | null, knownIds: Set<string>): Overrides {
  if (!storage) return {};
  try {
    const raw = storage.getItem(SHORTCUT_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object') return {};
    const result: Overrides = {};
    for (const [id, value] of Object.entries(parsed)) {
      if (knownIds.has(id) && (value === null || typeof value === 'string')) result[id] = value as Binding | null;
    }
    return result;
  } catch {
    return {};
  }
}

export function createShortcutRegistry(actions: ShortcutAction[], storage: StorageLike | null) {
  const ids = new Set(actions.map((a) => a.id));
  const state = $state({ overrides: loadOverrides(storage, ids) as Overrides });

  function persist() {
    try {
      storage?.setItem(SHORTCUT_STORAGE_KEY, JSON.stringify(state.overrides));
    } catch {
      /* storage unavailable */
    }
  }

  function bindingOf(id: string): Binding | null {
    if (id in state.overrides) return state.overrides[id];
    return actions.find((a) => a.id === id)?.defaultBinding ?? null;
  }

  function conflictFor(id: string, binding: Binding): ShortcutAction | null {
    return actions.find((a) => a.id !== id && bindingOf(a.id) === binding) ?? null;
  }

  function setOverride(id: string, binding: Binding | null) {
    const action = actions.find((a) => a.id === id);
    if (!action) return;
    if (binding === action.defaultBinding) delete state.overrides[id];
    else state.overrides[id] = binding;
    persist();
  }

  function rebind(id: string, binding: Binding, options: { replace?: boolean } = {}): RebindResult {
    const conflict = conflictFor(id, binding);
    if (conflict && !options.replace) {
      return {
        ok: false,
        conflict,
        message: `${formatBinding(binding)} is already used by “${conflict.label}”.`,
      };
    }
    if (conflict) setOverride(conflict.id, null);
    setOverride(id, binding);
    return { ok: true };
  }

  function unbind(id: string) {
    setOverride(id, null);
  }

  function reset(id: string) {
    delete state.overrides[id];
    persist();
  }

  function resetAll() {
    for (const id of Object.keys(state.overrides)) delete state.overrides[id];
    persist();
  }

  function isCustom(id: string): boolean {
    return id in state.overrides;
  }

  function hasCustom(): boolean {
    return Object.keys(state.overrides).length > 0;
  }

  function groups(): { group: string; actions: ShortcutAction[] }[] {
    const order: string[] = [];
    const by = new Map<string, ShortcutAction[]>();
    for (const a of actions) {
      if (!by.has(a.group)) {
        by.set(a.group, []);
        order.push(a.group);
      }
      by.get(a.group)!.push(a);
    }
    return order.map((group) => ({ group, actions: by.get(group)! }));
  }

  /** Runs the first action bound to this key event. Returns whether one ran. */
  function dispatch(e: KeyEventLike & { preventDefault(): void }, env: { typing: boolean }): boolean {
    const binding = bindingFromEvent(e);
    if (!binding) return false;
    for (const action of actions) {
      if (bindingOf(action.id) !== binding) continue;
      if (env.typing && !(action.inInput ?? firesWhileTyping(binding))) continue;
      if (action.when && !action.when()) continue;
      e.preventDefault();
      action.run();
      return true;
    }
    return false;
  }

  return {
    actions,
    byId: (id: string) => actions.find((a) => a.id === id),
    bindingOf,
    conflictFor,
    rebind,
    unbind,
    reset,
    resetAll,
    isCustom,
    hasCustom,
    groups,
    dispatch,
  };
}

export type ShortcutRegistry = ReturnType<typeof createShortcutRegistry>;
