import { createShortcutRegistry } from './registry.svelte.ts';
import { buildAppActions } from './app-actions.ts';

function browserStorage() {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

export const shortcuts = createShortcutRegistry(buildAppActions(), browserStorage());
