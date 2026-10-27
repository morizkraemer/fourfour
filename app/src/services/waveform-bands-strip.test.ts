import { describe, expect, it } from 'vitest';
import { stripBandsFromAnalysis } from './waveform-bands.ts';

function wire(bytes: number[]) {
  return { len: bytes.length / 3, calibration: 1, pwv7: true, bands: btoa(String.fromCharCode(...bytes)) };
}

describe('stripBandsFromAnalysis', () => {
  it('prefers the overview series and marks it as overview', () => {
    const bands = stripBandsFromAnalysis({
      waveform_overview: wire([127, 64, 0, 127, 64, 0]),
      waveform_color_detail: wire([10, 10, 10, 10, 10, 10, 10, 10, 10]),
    });
    expect(bands?.overview).toBe(true);
    expect(bands?.low.length).toBe(2);
    expect(bands?.low[0]).toBeCloseTo(1);
  });

  it('falls back to the detail columns without the overview flag, so the renderer scales them', () => {
    const bands = stripBandsFromAnalysis({ waveform_color_detail: wire([127, 127, 127, 64, 64, 64]) });
    expect(bands?.overview).toBeUndefined();
    expect(bands?.low.length).toBe(2);
  });

  it('is null without any waveform', () => {
    expect(stripBandsFromAnalysis({})).toBeNull();
    expect(stripBandsFromAnalysis(null)).toBeNull();
  });
});
