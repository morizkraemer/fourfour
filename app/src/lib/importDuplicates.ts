import type { DuplicateRow, ImportCommit, ImportPlan } from '../services/tauri.svelte.ts';

export interface ImportSteps {
  plan(paths: string[]): Promise<ImportPlan>;
  ask(rows: DuplicateRow[]): Promise<boolean>;
  commit(token: number, skipDuplicates: boolean): Promise<ImportCommit>;
}

/** Scans, asks once when some files look like library tracks, then adds. Without duplicates nothing is asked. */
export async function importWithDuplicateCheck(paths: string[], steps: ImportSteps): Promise<ImportCommit> {
  const plan = await steps.plan(paths);
  const skip = plan.duplicates.length === 0 ? true : await steps.ask(plan.duplicates);
  return steps.commit(plan.token, skip);
}

export function duplicatesTitle(count: number): string {
  return count === 1 ? '1 track looks like one you already have' : `${count} tracks look like ones you already have`;
}

export function trackTime(secs: number): string {
  if (!(secs > 0)) return '—';
  const whole = Math.round(secs);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

export function importSummary(added: number, skipped: number): string {
  const parts: string[] = [];
  if (added > 0) parts.push(`Imported ${added} track${added === 1 ? '' : 's'}`);
  if (skipped > 0) parts.push(`${added > 0 ? 'skipped' : 'Skipped'} ${skipped} duplicate${skipped === 1 ? '' : 's'}`);
  return parts.join(', ');
}
