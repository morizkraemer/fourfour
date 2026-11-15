export type FileState = 'missing' | 'error' | 'relocated';

export interface HealthCounts {
  total: number;
  missing: number;
  error: number;
}

export const RELOCATED_MARK_MS = 5000;

const GLYPHS: Record<FileState, string> = { missing: '!', error: '×', relocated: '✓' };

export function fileStateGlyph(state: FileState | undefined): string {
  return state ? GLYPHS[state] : '';
}

export function fileStateTooltip(state: FileState | undefined, path: string): string | undefined {
  if (state === 'missing') return `File missing: ${path}\nRelocate it from the library status button in the top bar.`;
  if (state === 'error') return `Can't read this file: ${path}\nCheck that it opens in another app, or relocate it from the library status button in the top bar.`;
  if (state === 'relocated') return `Relinked to ${path}`;
  return undefined;
}

export function isUnplayable(state: FileState | undefined): boolean {
  return state === 'missing' || state === 'error';
}

/** The top-bar button label; empty when every file is fine, so the button stays hidden. */
export function problemLabel(counts: Pick<HealthCounts, 'missing' | 'error'>): string {
  const parts: string[] = [];
  if (counts.missing > 0) parts.push(`${counts.missing} missing`);
  if (counts.error > 0) parts.push(`${counts.error} unreadable`);
  return parts.join(' · ');
}

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

export function checkSummary(counts: HealthCounts, durationMs: number): string {
  const took = durationMs < 1000 ? 'under a second' : `${Math.round(durationMs / 1000)} s`;
  const problems = counts.missing + counts.error;
  const checked = `${plural(counts.total, 'track', 'tracks')} checked in ${took}.`;
  return problems === 0 ? `${checked} Every file is where it should be.` : `${checked} ${problems} ${problems === 1 ? 'needs' : 'need'} a look.`;
}

export function lastCheckText(checkedAtMs: number | null, counts: Pick<HealthCounts, 'missing' | 'error'>, now = new Date()): string {
  if (checkedAtMs === null) return 'Not checked yet.';
  const at = new Date(checkedAtMs);
  const time = at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const day = at.toDateString() === now.toDateString() ? 'today' : at.toLocaleDateString();
  const result = problemLabel(counts) || 'all files found';
  return `Last check ${day} ${time}: ${result}.`;
}

export function relocateSummary(report: { matched: unknown[]; ambiguous: unknown[]; not_found: unknown[] }): string {
  const parts = [`${plural(report.matched.length, 'track', 'tracks')} relinked`];
  if (report.ambiguous.length > 0) parts.push(`${report.ambiguous.length} with more than one match`);
  if (report.not_found.length > 0) parts.push(`${report.not_found.length} not found`);
  return parts.join(', ') + '.';
}
