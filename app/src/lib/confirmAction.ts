import { ask } from '@tauri-apps/plugin-dialog';
import { getIsTauri } from '../services/tauri.svelte.ts';

export async function confirmAction(message: string): Promise<boolean> {
  if (!getIsTauri()) return window.confirm(message);
  return ask(message, { title: 'fourfour', kind: 'warning' });
}
