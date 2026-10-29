import type { DuplicateRow } from '../services/tauri.svelte.ts';

export const duplicatesPrompt = $state({ open: false, rows: [] as DuplicateRow[] });

let answer: ((skipDuplicates: boolean) => void) | null = null;

export function answerDuplicates(skipDuplicates: boolean) {
  const resolve = answer;
  answer = null;
  duplicatesPrompt.open = false;
  duplicatesPrompt.rows = [];
  resolve?.(skipDuplicates);
}

/** Opens the duplicates dialog; resolves true for "Skip duplicates" (also Escape), false for "Import anyway". */
export function askAboutDuplicates(rows: DuplicateRow[]): Promise<boolean> {
  answerDuplicates(true);
  duplicatesPrompt.rows = rows;
  duplicatesPrompt.open = true;
  return new Promise((resolve) => {
    answer = resolve;
  });
}
