export const FOCUS_DELAY_MS = 300;
export const FOCUS_BAND_ROWS = 40;
export const MAX_FOCUS = 200;

export interface FocusTrack {
  id: number;
  analyzed?: boolean;
  analysisPending?: boolean;
}

function needsAnalysis(track: FocusTrack | undefined): track is FocusTrack {
  return !!track && (!track.analyzed || track.analysisPending === true);
}

/**
 * Ids the analysis should take next for a list scrolled to rows [first, end): the rows on screen top to bottom, then
 * the band around them nearest first (below before above), only tracks that still wait for analysis.
 */
export function focusIds(
  tracks: readonly (FocusTrack | undefined)[],
  first: number,
  end: number,
  band = FOCUS_BAND_ROWS,
  max = MAX_FOCUS,
): number[] {
  const ids: number[] = [];
  const take = (index: number) => {
    const track = tracks[index];
    if (ids.length < max && needsAnalysis(track)) ids.push(track.id);
  };
  const start = Math.max(0, first);
  const stop = Math.min(tracks.length, end);
  for (let i = start; i < stop; i++) take(i);
  for (let step = 0; step < band; step++) {
    if (stop + step < tracks.length) take(stop + step);
    if (start - 1 - step >= 0) take(start - 1 - step);
  }
  return ids;
}

function sameIds(a: readonly number[], b: readonly number[]) {
  return a.length === b.length && a.every((id, i) => id === b[i]);
}

type View = [tracks: readonly (FocusTrack | undefined)[], first: number, end: number];

/**
 * Tells the analysis what the list shows (scroll, sort, search, another playlist, a track finishing): `delayMs` after
 * the first change, with the view as it is then, and only when the ids differ from the last ones sent. Changes that
 * keep coming (analysis results arrive several times a second) delay nothing.
 */
export function createAnalysisFocus(send: (ids: number[]) => unknown, delayMs = FOCUS_DELAY_MS) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let latest: View | null = null;
  let sent: number[] | null = null;

  function flush() {
    timer = undefined;
    if (!latest) return;
    const ids = focusIds(...latest);
    latest = null;
    if (sent && sameIds(sent, ids)) return;
    sent = ids;
    void send(ids);
  }

  return {
    request(tracks: readonly (FocusTrack | undefined)[], first: number, end: number) {
      latest = [tracks, first, end];
      timer ??= setTimeout(flush, delayMs);
    },
    dispose() {
      clearTimeout(timer);
      timer = undefined;
      latest = null;
    },
  };
}
