import type { BandColumns } from '../services/waveform-bands.ts';
import { WIRE_CALIBRATION, bandBytes, encodeStickWire, overviewLevel, previewFromBands, rowBandsBase64, syntheticBands } from './synthetic-bands.ts';

const COLUMNS_PER_SECOND = 150;
const STRIP_COLUMNS = 2048;
const OVERVIEW_COLUMNS = 1200;
const PERF_DURATION_SECS = 240;
const TEMPLATE_VARIANTS = 4;

interface BaseBands {
  strip: BandColumns;
  detail: BandColumns;
  overview: BandColumns;
}

let base: BaseBands | null = null;

function baseBands(): BaseBands {
  base ??= {
    strip: syntheticBands(STRIP_COLUMNS, 1, PERF_DURATION_SECS, 124),
    detail: syntheticBands(PERF_DURATION_SECS * COLUMNS_PER_SECOND, 1, PERF_DURATION_SECS, 124),
    overview: syntheticBands(OVERVIEW_COLUMNS, 1, PERF_DURATION_SECS, 124),
  };
  return base;
}

/** The same track started `variant / TEMPLATE_VARIANTS` of the way in: variants cost a copy, not a synthesis. */
function rotated(bands: BandColumns, variant: number): BandColumns {
  const columns = bands.low.length;
  const shift = Math.floor((columns * (variant - 1)) / TEMPLATE_VARIANTS);
  const rotate = (values: Float32Array) => {
    const out = new Float32Array(columns);
    out.set(values.subarray(shift), 0);
    out.set(values.subarray(0, shift), columns - shift);
    return out;
  };
  return { low: rotate(bands.low), mid: rotate(bands.mid), high: rotate(bands.high) };
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

function buildTemplate(legacy: boolean, variant: number): string {
  const strip = rotated(baseBands().strip, variant);
  const stripBytes = bandBytes(strip);
  const detail = bandBytes(rotated(baseBands().detail, variant));
  const overview = overviewLevel(bandBytes(rotated(baseBands().overview, variant)));
  const encode = legacy ? legacyColorSamples : (bytes: Uint8Array) => encodeStickWire(bytes);
  const beats = Array.from({ length: 512 }, (_, i) => ({ time_ms: i * 484, bar_position: (i % 4) + 1 }));
  return JSON.stringify({
    waveform_preview: previewFromBands(strip),
    waveform_row_bands: rowBandsBase64(strip),
    waveform_color: encode(stripBytes),
    waveform_color_detail: encode(detail),
    waveform_overview: encode(overview),
    beats,
    cue_points: [
      { hot_cue_number: 1, time_ms: 16000, loop_time_ms: null },
      { hot_cue_number: 2, time_ms: 120000, loop_time_ms: null },
    ],
  });
}

/** All variants at once on first use: the first answer pays for the synthesis, later ones are lookups, as with a backend that has the data ready. */
function template(legacy: boolean, seed: number): string {
  const key = `${legacy}:${seed % TEMPLATE_VARIANTS}`;
  const cached = templates.get(key);
  if (cached) return cached;
  for (let variant = 1; variant <= TEMPLATE_VARIANTS; variant++) {
    templates.set(`${legacy}:${variant - 1}`, buildTemplate(legacy, variant));
  }
  return templates.get(key)!;
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
