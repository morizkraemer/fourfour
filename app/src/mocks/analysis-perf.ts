import { WIRE_CALIBRATION, bandBytes, encodeStickWire, previewFromBands, rowBandsBase64, syntheticBands } from './synthetic-bands.ts';

const COLUMNS_PER_SECOND = 150;
const STRIP_COLUMNS = 2048;
const OVERVIEW_COLUMNS = 1200;
const PERF_DURATION_SECS = 240;

function bandBytesFor(columns: number, seed: number, durationSecs: number): Uint8Array {
  return bandBytes(syntheticBands(columns, seed, durationSecs, 124));
}

function legacyColorSamples(bytes: Uint8Array) {
  const out = [];
  for (let i = 0; i < bytes.length; i += 3) {
    const max = Math.max(bytes[i], bytes[i + 1], bytes[i + 2]);
    const scale = max > 0 ? 1 / max : 0;
    out.push({
      amp: Math.min((max / 127) * WIRE_CALIBRATION, 1),
      r: bytes[i] * scale,
      g: bytes[i + 1] * scale,
      b: bytes[i + 2] * scale,
    });
  }
  return out;
}

const templates = new Map<string, string>();
const TEMPLATE_VARIANTS = 4;

function template(legacy: boolean, seed: number): string {
  const key = `${legacy}:${seed % TEMPLATE_VARIANTS}`;
  const cached = templates.get(key);
  if (cached) return cached;
  const variant = (seed % TEMPLATE_VARIANTS) + 1;
  const strip = bandBytesFor(STRIP_COLUMNS, variant, PERF_DURATION_SECS);
  const detail = bandBytesFor(PERF_DURATION_SECS * COLUMNS_PER_SECOND, variant, PERF_DURATION_SECS);
  const overview = bandBytesFor(OVERVIEW_COLUMNS, variant, PERF_DURATION_SECS);
  const encode = legacy ? legacyColorSamples : (bytes: Uint8Array) => encodeStickWire(bytes);
  const beats = Array.from({ length: 512 }, (_, i) => ({ time_ms: i * 484, bar_position: (i % 4) + 1 }));
  const text = JSON.stringify({
    waveform_preview: previewFromBands(syntheticBands(STRIP_COLUMNS, variant, PERF_DURATION_SECS, 124)),
    waveform_row_bands: rowBandsBase64(syntheticBands(STRIP_COLUMNS, variant, PERF_DURATION_SECS, 124)),
    waveform_color: encode(strip),
    waveform_color_detail: encode(detail),
    waveform_overview: encode(overview),
    beats,
    cue_points: [
      { hot_cue_number: 1, time_ms: 16000, loop_time_ms: null },
      { hot_cue_number: 2, time_ms: 120000, loop_time_ms: null },
    ],
  });
  templates.set(key, text);
  return text;
}

/**
 * Browser-only stand-in for the `get_analysis_data` IPC response at real size
 * (240 s track, 150 columns/s). `legacy` reproduces the pre-binary JSON shape.
 * Parsing the JSON text per call mirrors what Tauri does with the IPC reply.
 */
export function perfAnalysisData(
  durationSecs: number,
  tempo: number,
  key: string,
  legacy: boolean,
  seed = 0,
) {
  const data = JSON.parse(template(legacy, seed));
  return { ...data, duration_ms: Math.round(durationSecs * 1000), bpm: tempo / 100, key };
}
