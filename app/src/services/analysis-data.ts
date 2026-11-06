/**
 * Validation and normalization for get_analysis_data payloads.
 */

const PREVIEW_LEN = 400;

export function isValidWaveformPreview(preview: unknown): preview is number[] {
  if (!Array.isArray(preview) || preview.length !== PREVIEW_LEN) return false;
  const inRange = preview.every(
    (b) => typeof b === 'number' && Number.isFinite(b) && b >= 0 && b <= 255,
  );
  if (!inRange) return false;
  // Reject the all-zero placeholder `get_analysis` returns when the DB index
  // exists but the ANLZ files are missing — its 5-bit height is flat across the
  // board, so caching it would render (and lock in) a fake flat waveform.
  return preview.some((b) => (b & 0x1f) !== 0);
}

export function isValidColorWaveform(color: unknown): color is Array<{
  amp: number;
  r: number;
  g: number;
  b: number;
}> {
  if (!Array.isArray(color) || color.length < 8) return false;
  return color.every((s) => {
    if (!s || typeof s !== 'object') return false;
    const { amp, r, g, b } = s as Record<string, unknown>;
    return [amp, r, g, b].every(
      (v) => typeof v === 'number' && Number.isFinite(v) && v >= 0 && v <= 1.25,
    );
  });
}

export interface ColorWaveSample {
  amp: number;
  r: number;
  g: number;
  b: number;
}

/** Colour waveform as sent by `get_analysis_data`: band bytes (low, mid, high per column; base64 text in browser mocks) plus the PWAV calibration. */
export interface EncodedColorWaveform {
  len: number;
  calibration: number;
  bands: string | Uint8Array;
  /** The bytes are the stick's PWV7 / PWV6 values: drawn as `byte / 127`, no calibration or balancing. */
  pwv7?: boolean;
}

const MIN_COLOR_COLUMNS = 8;

function isEncodedColorWaveform(color: unknown): color is EncodedColorWaveform {
  if (!color || typeof color !== 'object') return false;
  const { len, calibration, bands } = color as Record<string, unknown>;
  return (
    typeof len === 'number' &&
    len >= MIN_COLOR_COLUMNS &&
    typeof calibration === 'number' &&
    Number.isFinite(calibration) &&
    (typeof bands === 'string' || bands instanceof Uint8Array)
  );
}

function isUsableColorWaveform(color: unknown): boolean {
  return isEncodedColorWaveform(color) || isValidColorWaveform(color);
}

/** The bytes of a colour waveform's `bands`: as they came in the binary answer, or decoded from base64 text. */
export function bandBytes(bands: string | Uint8Array): Uint8Array {
  if (typeof bands !== 'string') return bands;
  const binary = atob(bands);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

const decodedColorWaveforms = new WeakMap<object, ColorWaveSample[]>();

/** Rebuild `{amp, r, g, b}` columns from the wire format (arrays from browser mocks pass through). */
export function decodeColorWaveform(color: unknown): ColorWaveSample[] | null {
  if (isValidColorWaveform(color)) return color;
  if (!isEncodedColorWaveform(color)) return null;
  const memo = decodedColorWaveforms.get(color);
  if (memo) return memo;
  const bytes = bandBytes(color.bands);
  const columns = Math.floor(bytes.length / 3);
  const out = new Array<ColorWaveSample>(columns);
  for (let i = 0; i < columns; i++) {
    const low = bytes[i * 3];
    const mid = bytes[i * 3 + 1];
    const high = bytes[i * 3 + 2];
    const max = Math.max(low, mid, high);
    const scale = max > 0 ? 1 / max : 0;
    out[i] = {
      amp: Math.min((max / 127) * color.calibration, 1),
      r: low * scale,
      g: mid * scale,
      b: high * scale,
    };
  }
  decodedColorWaveforms.set(color, out);
  return out;
}

const ANALYSIS_MAGIC = 0x4e414646;

/**
 * The answer of `get_analysis_data` (`FFAN`, the length of the JSON head as u32, the JSON head, then the band bytes) as
 * the payload object the base64 form had: the same fields, with the `bands` of `waveform_color`,
 * `waveform_color_detail` and `waveform_overview` and `waveform_row_bands` as `Uint8Array` views of the answer
 * (`waveform_row_bands` is copied, rows keep it) instead of base64 text.
 */
export function decodeAnalysisBytes(buffer: ArrayBuffer): any {
  const view = new DataView(buffer);
  if (buffer.byteLength < 8 || view.getUint32(0, true) !== ANALYSIS_MAGIC) throw new Error('Invalid analysis answer from backend');
  const headLength = view.getUint32(4, true);
  if (8 + headLength > buffer.byteLength) throw new Error('Truncated analysis answer from backend');
  const head = JSON.parse(new TextDecoder().decode(new Uint8Array(buffer, 8, headLength)));
  const arena = 8 + headLength;
  const slice = (ref: { offset: number; length: number }) => {
    if (arena + ref.offset + ref.length > buffer.byteLength) throw new Error('Truncated analysis answer from backend');
    return new Uint8Array(buffer, arena + ref.offset, ref.length);
  };
  for (const field of ['waveform_color', 'waveform_color_detail', 'waveform_overview']) {
    const color = head[field];
    if (color && typeof color.bands === 'object') color.bands = slice(color.bands);
  }
  const row = head.waveform_row_bands;
  if (row && typeof row === 'object') head.waveform_row_bands = slice(row).slice();
  return head;
}

/** True when the payload is safe to cache and render (mono preview required). */
export function isValidAnalysisPayload(data: unknown): boolean {
  if (!data || typeof data !== 'object') return false;
  return isValidWaveformPreview((data as { waveform_preview?: unknown }).waveform_preview);
}

/** Drop corrupt color bands; keep mono preview for the player strip. */
export function sanitizeAnalysisPayload(data: any): any | null {
  if (!isValidAnalysisPayload(data)) return null;
  if (isUsableColorWaveform(data.waveform_color)) return data;
  return { ...data, waveform_color: null };
}

export function previewPeaksNormalized(preview: number[]): number[] {
  return preview.map((byte) => (byte & 0x1f) / 31.0);
}

export function previewBytesFromPayload(data: { waveform_preview: number[] }): number[] {
  return Array.isArray(data.waveform_preview)
    ? data.waveform_preview
    : Array.from(data.waveform_preview);
}

/** `load_state` embeds each preview as base64; browser mocks pass plain byte arrays. */
export function decodeTrackPreview(preview: string | number[] | null | undefined): number[] | null {
  if (typeof preview !== 'string') return preview ?? null;
  const binary = atob(preview);
  const bytes = new Array<number>(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** `load_state` and `get_analysis_data` carry the 3-band row preview as base64 (or a byte array in mocks). */
export function decodeBandPreview(preview: string | number[] | Uint8Array | null | undefined): Uint8Array | null {
  if (!preview || preview.length === 0) return null;
  if (preview instanceof Uint8Array) return preview.length % 3 === 0 ? preview.slice() : null;
  if (typeof preview !== 'string') return preview.length % 3 === 0 ? Uint8Array.from(preview) : null;
  const binary = atob(preview);
  if (binary.length === 0 || binary.length % 3 !== 0) return null;
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

const STAGE_WAVEFORM = 1;
const STAGE_KEY = 2;
const STAGE_GRID = 4;
const ALL_STAGES = STAGE_WAVEFORM | STAGE_KEY | STAGE_GRID;

export interface ReadyAnalysisFields {
  waveform: boolean;
  key: boolean;
  bpm: boolean;
}

/**
 * Which parts of a `get_analysis_data` payload are finished results. During a (re-)analysis the stored
 * row still holds the stages in flight, so only stored stages may be shown; a hand-set BPM or key
 * is always final. Payloads without stage bits are complete.
 */
export function readyAnalysisFields(data: {
  analysis_stages?: number;
  bpm_locked?: boolean;
  key_locked?: boolean;
}): ReadyAnalysisFields {
  const stages = typeof data.analysis_stages === 'number' ? data.analysis_stages : ALL_STAGES;
  return {
    waveform: (stages & STAGE_WAVEFORM) !== 0,
    key: (stages & STAGE_KEY) !== 0 || data.key_locked === true,
    bpm: (stages & STAGE_GRID) !== 0 || data.bpm_locked === true,
  };
}
