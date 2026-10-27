export type BoardTrack = {
  bpmExact?: number;
  key?: string;
  raw?: { duration_secs?: number };
};

export type BoardSummary = {
  count: number;
  durationSecs: number;
  avgBpm: number | null;
  keyRange: string | null;
};

function camelotNumber(key: string | undefined): number | null {
  const match = /^(\d{1,2})[AB]$/.exec(key ?? '');
  return match ? Number(match[1]) : null;
}

export function summarizeBoard(tracks: BoardTrack[]): BoardSummary {
  const bpms = tracks.map((t) => t.bpmExact ?? 0).filter((bpm) => bpm > 0);
  const keys = tracks
    .map((t) => ({ key: t.key as string, number: camelotNumber(t.key) }))
    .filter((k): k is { key: string; number: number } => k.number !== null)
    .sort((a, b) => a.number - b.number || a.key.localeCompare(b.key));
  const first = keys[0]?.key;
  const last = keys[keys.length - 1]?.key;
  return {
    count: tracks.length,
    durationSecs: tracks.reduce((sum, t) => sum + (t.raw?.duration_secs ?? 0), 0),
    avgBpm: bpms.length > 0 ? bpms.reduce((a, b) => a + b, 0) / bpms.length : null,
    keyRange: first && last ? (first === last ? first : `${first}–${last}`) : null,
  };
}

export function formatBoardDuration(secs: number): string {
  const minutes = Math.round(secs / 60);
  const hours = Math.floor(minutes / 60);
  return hours > 0 ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
}
