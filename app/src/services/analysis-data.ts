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

/** True when the payload is safe to cache and render (mono preview required). */
export function isValidAnalysisPayload(data: unknown): boolean {
  if (!data || typeof data !== 'object') return false;
  return isValidWaveformPreview((data as { waveform_preview?: unknown }).waveform_preview);
}

/** Drop corrupt color bands; keep mono preview for the player strip. */
export function sanitizeAnalysisPayload(data: any): any | null {
  if (!isValidAnalysisPayload(data)) return null;
  if (isValidColorWaveform(data.waveform_color)) return data;
  return { ...data, waveform_color: [] };
}

export function previewPeaksNormalized(preview: number[]): number[] {
  return preview.map((byte) => (byte & 0x1f) / 31.0);
}

export function previewBytesFromPayload(data: { waveform_preview: number[] }): number[] {
  return Array.isArray(data.waveform_preview)
    ? data.waveform_preview
    : Array.from(data.waveform_preview);
}
