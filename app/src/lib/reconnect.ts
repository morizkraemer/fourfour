import type { ChoiceReason, ReconnectPlan, RemapPreview } from '../services/tauri.svelte.ts';

function segments(path: string): string[] {
  return path.split(/[\\/]/);
}

/** The deepest folder that holds every path, or '' when they share none. */
export function commonFolder(paths: string[]): string {
  if (paths.length === 0) return '';
  const separator = paths[0].includes('\\') && !paths[0].includes('/') ? '\\' : '/';
  const folders = paths.map((p) => segments(p).slice(0, -1));
  const shared = folders.reduce((common, parts) => {
    let n = 0;
    while (n < common.length && n < parts.length && common[n] === parts[n]) n++;
    return common.slice(0, n);
  });
  const joined = shared.join(separator);
  return joined === '' && shared.length === 1 ? separator : joined;
}

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

export function reconnectSummary(plan: ReconnectPlan): string {
  const parts = [`${plural(plan.reconnect.length, 'track', 'tracks')} reconnected`];
  if (plan.needs_choice.length > 0) parts.push(`${plan.needs_choice.length} need${plan.needs_choice.length === 1 ? 's' : ''} your choice`);
  if (plan.still_missing.length > 0) parts.push(`${plan.still_missing.length} still missing`);
  return parts.join(', ') + '.';
}

const REASONS: Record<ChoiceReason, string> = {
  multiple_candidates: 'more than one match',
  unconfirmed: 'a likely match, not verified',
  copy: 'looks like a copy of a track you have',
  shared_file: 'another missing track matches the same file',
};

export function choiceReasonText(reason: ChoiceReason): string {
  return REASONS[reason];
}

export function remapPreviewText(preview: RemapPreview): string {
  if (preview.total === 0) return 'No track lives in that folder.';
  const found = `${preview.found} found`;
  return preview.still_missing > 0 ? `${found} · ${preview.still_missing} still missing` : `${found} · none missing`;
}

export function canApplyRemap(preview: RemapPreview | null): boolean {
  return preview !== null && preview.found > 0;
}

export function relocationText(oldRoot: string, newRoot: string, tracks: number): string {
  return `${plural(tracks, 'track', 'tracks')} moved from ${oldRoot} to ${newRoot}.`;
}

export function candidateLabel(path: string): string {
  const parts = segments(path.replace(/[\\/]+$/, ''));
  return parts.slice(-2).join('/');
}
