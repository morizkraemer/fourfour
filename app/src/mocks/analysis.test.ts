import { describe, expect, it } from 'vitest';
import { mockAnalysisData } from './analysis.ts';
import { decodeColorWaveform, isValidAnalysisPayload, sanitizeAnalysisPayload } from '../services/analysis-data.ts';

describe('mockAnalysisData', () => {
  const data = mockAnalysisData(7, 180, 12800, 'Am');

  it('produces a payload the validators accept', () => {
    expect(isValidAnalysisPayload(data)).toBe(true);
    expect(decodeColorWaveform(data.waveform_color)).not.toBeNull();
    expect(sanitizeAnalysisPayload(data).waveform_color).toBe(data.waveform_color);
  });
  it('converts centi-bpm tempo and builds a 4/4 beat grid', () => {
    expect(data.bpm).toBe(128);
    expect(data.duration_ms).toBe(180000);
    expect(data.beats[0]).toEqual({ time_ms: 0, bar_position: 1 });
    expect(data.beats.slice(0, 5).map((b) => b.bar_position)).toEqual([1, 2, 3, 4, 1]);
    expect(data.beats[1].time_ms).toBe(Math.round(60000 / 128));
    expect(data.beats.at(-1)!.time_ms).toBeLessThan(data.duration_ms);
  });
  it('is deterministic per track id and differs between ids', () => {
    expect(mockAnalysisData(7, 180, 12800, 'Am')).toEqual(data);
    expect(mockAnalysisData(8, 180, 12800, 'Am').waveform_preview).not.toEqual(data.waveform_preview);
  });
  it('clamps zero duration to 1 ms and keeps cues inside the track', () => {
    expect(mockAnalysisData(1, 0, 12000, 'C').duration_ms).toBe(1);
    for (const cue of data.cue_points) {
      expect(cue.time_ms).toBeGreaterThan(0);
      expect(cue.time_ms).toBeLessThan(data.duration_ms);
    }
  });
  it('keeps preview bytes in range with a varying whiteness', () => {
    expect(data.waveform_preview).toHaveLength(400);
    expect(data.waveform_preview.every((b) => b >= 0 && b <= 255 && (b & 0x1f) >= 1)).toBe(true);
    expect(new Set(data.waveform_preview.map((b) => b >> 5)).size).toBeGreaterThan(2);
  });
  it('sends the three colour waveforms in the wire format at their resolutions', () => {
    expect(data.waveform_color.len).toBe(2048);
    expect(data.waveform_overview.len).toBe(1200);
    expect(data.waveform_color_detail.len).toBe(180 * 150);
  });
});
