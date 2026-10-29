export interface LibraryActivityInput {
  importing: boolean;
  importDone: number;
  importTotal: number;
  resultHeadline: string | null;
  resultFailed: boolean;
  problems: string;
}

export type LibraryActivity =
  | { kind: 'importing'; label: string; fraction: number | null }
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
  if (input.resultHeadline) return { kind: 'result', label: input.resultHeadline, failed: input.resultFailed };
  if (input.problems) return { kind: 'problems', label: input.problems };
  return { kind: 'rest', label: 'Import…' };
}
