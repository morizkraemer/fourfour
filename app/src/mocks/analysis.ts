import { bandBytes, encodeWire, previewFromBands, syntheticBands } from './synthetic-bands.ts';

const STRIP_COLUMNS = 2048;
const OVERVIEW_COLUMNS = 1200;
const DETAIL_COLUMNS_PER_SECOND = 150;

export function mockAnalysisData(trackId: number, durationSecs: number, tempo: number, key: string) {
  const durationMs = Math.max(1, Math.round(durationSecs * 1000));
  const bpm = tempo / 100;
  const beatMs = 60000 / bpm;
  const beats: { time_ms: number; bar_position: number }[] = [];
  for (let t = 0, n = 0; t < durationMs; t += beatMs, n++) {
    beats.push({ time_ms: Math.round(t), bar_position: (n % 4) + 1 });
  }
  const seconds = durationMs / 1000;
  const bandsAt = (columns: number) => syntheticBands(columns, trackId, seconds, bpm);
  const strip = bandsAt(STRIP_COLUMNS);
  return {
    track_id: trackId,
    duration_ms: durationMs,
    bpm,
    key,
    beats,
    cue_points: [{ time_ms: Math.round(durationMs * 0.08) }, { time_ms: Math.round(durationMs * 0.5) }],
    waveform_preview: previewFromBands(strip),
    waveform_color: encodeWire(bandBytes(strip)),
    waveform_color_detail: encodeWire(bandBytes(bandsAt(Math.max(8, Math.round(seconds * DETAIL_COLUMNS_PER_SECOND))))),
    waveform_overview: encodeWire(bandBytes(bandsAt(OVERVIEW_COLUMNS))),
  };
}
