import { ui } from '../stores/ui.svelte.ts';
import { selectVolume } from '../stores/library.svelte.ts';
import { select } from '../stores/selection.svelte.ts';

export async function goToSource(source: string, volumePath: string | null = null) {
  ui.activeSidebarRow = source;
  await selectVolume(volumePath);
}

export async function jumpToTrack(trackId: number) {
  await goToSource('All Tracks');
  ui.listFilter = '';
  select(trackId);
  ui.revealTrack = { id: trackId, nonce: (ui.revealTrack?.nonce ?? 0) + 1 };
}
