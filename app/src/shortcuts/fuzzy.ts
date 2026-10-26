export interface FuzzyMatch {
  score: number;
  indices: number[];
}

const WORD_START = /[\s\-_./:()[\]]/;

function matchToken(token: string, text: string): FuzzyMatch | null {
  const t = token.toLowerCase();
  const s = text.toLowerCase();
  const indices: number[] = [];
  let score = 0;
  let from = 0;
  let previous = -2;
  for (let i = 0; i < t.length; i++) {
    const at = s.indexOf(t[i], from);
    if (at === -1) return null;
    indices.push(at);
    score += 1;
    if (at === previous + 1) score += 4;
    if (at === 0) score += 6;
    else if (WORD_START.test(s[at - 1])) score += 5;
    score -= Math.min(at - from, 6) * 0.1;
    previous = at;
    from = at + 1;
  }
  const contiguous = s.indexOf(t);
  if (contiguous !== -1) {
    score += 8;
    if (contiguous === 0) score += 6;
  }
  score -= s.length * 0.01;
  return { score, indices };
}

/** Every whitespace-separated token of `query` must match `text` as an ordered subsequence. */
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  const tokens = query.trim().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return { score: 0, indices: [] };
  let score = 0;
  const indices = new Set<number>();
  for (const token of tokens) {
    const m = matchToken(token, text);
    if (!m) return null;
    score += m.score;
    m.indices.forEach((i) => indices.add(i));
  }
  return { score, indices: [...indices].sort((a, b) => a - b) };
}

export interface Ranked<T> {
  item: T;
  match: FuzzyMatch;
}

export function fuzzyRank<T>(query: string, items: readonly T[], textOf: (item: T) => string): Ranked<T>[] {
  const ranked: Ranked<T>[] = [];
  for (const item of items) {
    const match = fuzzyMatch(query, textOf(item));
    if (match) ranked.push({ item, match });
  }
  return ranked.sort((a, b) => b.match.score - a.match.score);
}
