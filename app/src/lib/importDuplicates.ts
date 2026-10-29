import type { DuplicateRow, ImportCommit, ImportPlan, SkippedFiles } from '../services/tauri.svelte.ts';

export interface ImportSteps {
  plan(paths: string[]): Promise<ImportPlan>;
  ask(rows: DuplicateRow[]): Promise<boolean>;
  commit(token: number, skipDuplicates: boolean): Promise<ImportCommit>;
}

export interface ImportOutcome extends ImportCommit {
  plan: ImportPlan;
}

/** Scans, asks once when some files look like library tracks, then adds. Without duplicates nothing is asked. */
export async function importWithDuplicateCheck(paths: string[], steps: ImportSteps): Promise<ImportOutcome> {
  const plan = await steps.plan(paths);
  const skip = plan.duplicates.length === 0 ? true : await steps.ask(plan.duplicates);
  return { ...(await steps.commit(plan.token, skip)), plan };
}

export function duplicatesTitle(count: number): string {
  return count === 1 ? '1 track looks like one you already have' : `${count} tracks look like ones you already have`;
}

export function trackTime(secs: number): string {
  if (!(secs > 0)) return '—';
  const whole = Math.round(secs);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

function count(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? '' : 's'}`;
}

const MAX_LISTED_EXTS = 4;

function skippedFileParts(skipped: SkippedFiles): string[] {
  const parts: string[] = [];
  if (skipped.unsupported > 0) {
    const exts = skipped.unsupported_exts;
    const listed = exts.slice(0, MAX_LISTED_EXTS).join(', ') + (exts.length > MAX_LISTED_EXTS ? ', …' : '');
    parts.push(`${count(skipped.unsupported, 'file')} skipped: unsupported format${listed ? ` (${listed})` : ''}`);
  }
  if (skipped.unreadable > 0) parts.push(`${count(skipped.unreadable, 'audio file')} couldn’t be read`);
  return parts;
}

function sentence(parts: string[]): string {
  const text = parts.join(' · ');
  return `${text.charAt(0).toUpperCase()}${text.slice(1)}${text.endsWith('.') ? '' : '.'}`;
}

/** Status line after importing into the library, from the real counts of the scan and commit. */
export function libraryImportMessage(outcome: ImportOutcome): string {
  const added = outcome.added.length;
  const duplicates = outcome.skipped_ids.length;
  const known = outcome.plan.already_in_library;
  const extra = skippedFileParts(outcome.plan.skipped);
  if (added === 0 && duplicates === 0 && extra.length === 0 && known > 0) {
    return known === 1 ? 'This file is already in your library.' : `All ${known} files are already in your library.`;
  }
  const parts: string[] = [];
  if (added > 0) parts.push(`Added ${count(added, 'track')}`);
  if (duplicates > 0) parts.push(`${count(duplicates, 'duplicate')} skipped`);
  if (known > 0) parts.push(`${known} already in your library`);
  parts.push(...extra);
  return parts.length === 0 ? 'No audio files found.' : sentence(parts);
}

export type PlaylistDropResult =
  | { kind: 'added'; name: string; tracks: number; addedToPlaylist: number }
  | { kind: 'created'; names: string[]; tracks: number };

/** Status line after a drop that filled a playlist: says which tracks were new to the library. */
export function playlistImportMessage(outcome: ImportOutcome, result: PlaylistDropResult): string {
  const added = outcome.added.length;
  const extra = skippedFileParts(outcome.plan.skipped);
  if (result.kind === 'added' && result.addedToPlaylist === 0) {
    const lead =
      result.tracks === 1 ? `That track is already in ‘${result.name}’` : `All ${result.tracks} tracks are already in ‘${result.name}’`;
    return sentence([lead, ...extra]);
  }
  const placed = result.kind === 'added' ? result.addedToPlaylist : result.tracks;
  const lead =
    result.kind === 'added'
      ? `Added ${count(placed, 'track')} to ‘${result.name}’`
      : result.names.length === 1
        ? `Created ‘${result.names[0]}’ with ${count(placed, 'track')}`
        : `Created ${result.names.length} playlists (${result.names.map((n) => `‘${n}’`).join(', ')}) with ${count(placed, 'track')}`;
  const known = Math.max(0, placed - added);
  const origin = known === 0 ? '' : added === 0 ? ' (already in library)' : ` (${added} new, ${known} already in library)`;
  return sentence([lead + origin, ...extra]);
}
