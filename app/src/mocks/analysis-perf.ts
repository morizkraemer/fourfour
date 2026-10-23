const COLUMNS_PER_SECOND = 150;
const STRIP_COLUMNS = 2048;
const OVERVIEW_COLUMNS = 1200;
const CALIBRATION = 0.9;
const PERF_DURATION_SECS = 240;

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

function bandBytes(columns: number, seed: number): Uint8Array {
  const bytes = new Uint8Array(columns * 3);
  for (let i = 0; i < columns; i++) {
    bytes[i * 3] = (i + seed) % 120;
    bytes[i * 3 + 1] = (i * 3) % 100;
    bytes[i * 3 + 2] = (i * 5 + seed) % 90;
  }
  return bytes;
}

function legacyColorSamples(bytes: Uint8Array) {
  const out = [];
  for (let i = 0; i < bytes.length; i += 3) {
    const max = Math.max(bytes[i], bytes[i + 1], bytes[i + 2]);
    const scale = max > 0 ? 1 / max : 0;
    out.push({
      amp: Math.min((max / 127) * CALIBRATION, 1),
      r: bytes[i] * scale,
      g: bytes[i + 1] * scale,
      b: bytes[i + 2] * scale,
    });
  }
  return out;
}

function encodedColor(bytes: Uint8Array) {
  return { len: bytes.length / 3, calibration: CALIBRATION, bands: bytesToBase64(bytes) };
}

const templates = new Map<boolean, string>();

function template(legacy: boolean): string {
  const cached = templates.get(legacy);
  if (cached) return cached;
  const strip = bandBytes(STRIP_COLUMNS, 1);
  const detail = bandBytes(PERF_DURATION_SECS * COLUMNS_PER_SECOND, 2);
  const overview = bandBytes(OVERVIEW_COLUMNS, 3);
  const encode = legacy ? legacyColorSamples : encodedColor;
  const beats = Array.from({ length: 512 }, (_, i) => ({ time_ms: i * 469, bar_position: (i % 4) + 1 }));
  const text = JSON.stringify({
    waveform_preview: Array.from({ length: 400 }, (_, n) => (3 << 5) | (((n * 7) % 31) + 1)),
    waveform_color: encode(strip),
    waveform_color_detail: encode(detail),
    waveform_overview: encode(overview),
    beats,
    cue_points: [
      { hot_cue_number: 1, time_ms: 16000, loop_time_ms: null },
      { hot_cue_number: 2, time_ms: 120000, loop_time_ms: null },
    ],
  });
  templates.set(legacy, text);
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
) {
  const data = JSON.parse(template(legacy));
  return { ...data, duration_ms: Math.round(durationSecs * 1000), bpm: tempo / 100, key };
}
