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
  | { kind: 'analyzing'; label: string; detail: string }
  | { kind: 'result'; label: string; failed: boolean }
  | { kind: 'problems'; label: string }
  | { kind: 'rest'; label: string };

export function libraryActivity(input: LibraryActivityInput): LibraryActivity {
  if (input.importing) {
    const known = input.importTotal > 0;
    return {
      kind: 'importing',
      label: known ? `Importing ${input.importDone}/${input.importTotal}…` : 'Importing…',
      fraction: known ? Math.min(1, input.importDone / input.importTotal) : null,
    };
  }
  if (input.syncing) return { kind: 'syncing', label: 'Syncing' };
  if (input.analyzing) {
    return { kind: 'analyzing', label: 'Analyzing', detail: `${input.analysisCurrent} / ${input.analysisTotal}` };
  }
  if (input.resultHeadline) return { kind: 'result', label: input.resultHeadline, failed: input.resultFailed };
  if (input.problems) return { kind: 'problems', label: input.problems };
  return { kind: 'rest', label: `${input.trackCount.toLocaleString()} ${input.trackCount === 1 ? 'track' : 'tracks'}` };
}
