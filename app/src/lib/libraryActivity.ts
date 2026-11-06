export interface LibraryActivityInput {
  importing: boolean;
  importDone: number;
  importTotal: number;
  syncing: boolean;
  analyzing: boolean;
  analysisCurrent: number;
  analysisTotal: number;
  resultHeadline: string | null;
  resultFailed: boolean;
  problems: string;
  trackCount: number;
}

export type LibraryActivity =
  | { kind: 'importing'; label: string; fraction: number | null }
  | { kind: 'syncing'; label: string }
  | { kind: 'analyzing'; label: string; detail: string; fraction: number }
  | { kind: 'result'; label: string; failed: boolean }
  | { kind: 'problems'; label: string }
  | { kind: 'rest'; label: string };

function progressFraction(done: number, total: number): number {
  return total > 0 ? Math.min(1, Math.max(0, done / total)) : 0;
}

export function libraryActivity(input: LibraryActivityInput): LibraryActivity {
  if (input.importing) {
    const known = input.importTotal > 0;
    return {
      kind: 'importing',
      label: known ? `Importing ${input.importDone}/${input.importTotal}…` : 'Importing…',
      fraction: known ? progressFraction(input.importDone, input.importTotal) : null,
    };
  }
  if (input.syncing) return { kind: 'syncing', label: 'Syncing' };
  if (input.analyzing) {
    return {
      kind: 'analyzing',
      label: 'Analyzing',
      detail: `${input.analysisCurrent} / ${input.analysisTotal}`,
      fraction: progressFraction(input.analysisCurrent, input.analysisTotal),
    };
  }
  if (input.resultHeadline) return { kind: 'result', label: input.resultHeadline, failed: input.resultFailed };
  if (input.problems) return { kind: 'problems', label: input.problems };
  return { kind: 'rest', label: `${input.trackCount.toLocaleString()} ${input.trackCount === 1 ? 'track' : 'tracks'}` };
}

/** The first finished track of an analysis run: where the pace is measured from. */
export interface AnalysisPace {
  at: number;
  done: number;
}

export function nextAnalysisPace(pace: AnalysisPace | null, done: number, now: number): AnalysisPace {
  return pace && done >= pace.done ? pace : { at: now, done };
}

const MIN_PACE_TRACKS = 2;
const MIN_PACE_MS = 4000;

/** Rough time left from the pace since the first finished track, or null while there is too little to go on. */
export function analysisTimeLeft(pace: AnalysisPace | null, done: number, total: number, now: number): string | null {
  if (!pace || done >= total) return null;
  const finished = done - pace.done;
  const elapsed = now - pace.at;
  if (finished < MIN_PACE_TRACKS || elapsed < MIN_PACE_MS) return null;
  const minutes = ((elapsed / finished) * (total - done)) / 60_000;
  if (minutes < 1) return 'Less than a minute left';
  if (minutes < 60) return `About ${Math.round(minutes)} min left`;
  const hours = Math.floor(minutes / 60);
  const rest = Math.round(minutes % 60);
  return rest === 0 ? `About ${hours} h left` : `About ${hours} h ${rest} min left`;
}
