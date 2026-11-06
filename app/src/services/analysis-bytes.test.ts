import { beforeEach, describe, expect, it, vi } from 'vitest';

import { bandBytes, decodeAnalysisBytes, decodeBandPreview, sanitizeAnalysisPayload } from './analysis-data.ts';
import { fixtureBuffer, fixtureText } from './fixtures.ts';
import { colorBandsFromPayload, stripBandsFromAnalysis } from './waveform-bands.ts';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const NAMES = ['analysis-pwv7', 'analysis-legacy'];
const COLOUR_FIELDS = ['waveform_color', 'waveform_color_detail', 'waveform_overview'];

function bytesLike(payload: any) {
  const copy = JSON.parse(JSON.stringify(payload));
  for (const field of COLOUR_FIELDS) if (copy[field]) copy[field].bands = Array.from(bandBytes(payload[field].bands));
  if (copy.waveform_row_bands) copy.waveform_row_bands = Array.from(decodeBandPreview(payload.waveform_row_bands)!);
  return copy;
}

describe.each(NAMES)('get_analysis_data answer %s', (name) => {
  const wire = decodeAnalysisBytes(fixtureBuffer(`${name}.bin`));
  const logical = JSON.parse(fixtureText(`${name}.json`));

  it('has the fields of the base64 payload, in the same order', () => {
    expect(Object.keys(wire)).toEqual(Object.keys(logical));
    expect(bytesLike(wire)).toStrictEqual(bytesLike(logical));
  });

  it('carries the band bytes as byte views, the row preview as its own copy', () => {
    for (const field of COLOUR_FIELDS) expect(wire[field].bands).toBeInstanceOf(Uint8Array);
    expect(wire.waveform_row_bands.buffer.byteLength).toBe(wire.waveform_row_bands.length);
  });

  it('draws the same columns as the base64 payload', () => {
    for (const field of COLOUR_FIELDS) {
      const a = colorBandsFromPayload(wire[field]);
      const b = colorBandsFromPayload(logical[field]);
      expect(a).not.toBeNull();
      expect(Array.from(a!.low)).toEqual(Array.from(b!.low));
      expect(Array.from(a!.mid)).toEqual(Array.from(b!.mid));
      expect(Array.from(a!.high)).toEqual(Array.from(b!.high));
      expect(a!.scale).toEqual(b!.scale);
    }
    const strip = stripBandsFromAnalysis(wire)!;
    const reference = stripBandsFromAnalysis(logical)!;
    expect(Array.from(strip.low)).toEqual(Array.from(reference.low));
    expect(Array.from(strip.high)).toEqual(Array.from(reference.high));
  });

  it('passes the payload check as the base64 payload does', () => {
    expect(sanitizeAnalysisPayload(wire)).toBe(wire);
    expect(sanitizeAnalysisPayload(logical)).toBe(logical);
  });
});

describe('damaged answers', () => {
  it('fail loudly instead of drawing garbage', () => {
    const good = new Uint8Array(fixtureBuffer('analysis-legacy.bin'));
    const foreign = good.slice();
    foreign[0] = 0x58;
    expect(() => decodeAnalysisBytes(foreign.buffer)).toThrow(/Invalid analysis answer/);
    expect(() => decodeAnalysisBytes(good.slice(0, 100).buffer)).toThrow(/Truncated analysis answer/);
    expect(() => decodeAnalysisBytes(good.slice(0, good.length - 10).buffer)).toThrow(/Truncated analysis answer/);
    expect(() => decodeAnalysisBytes(new ArrayBuffer(3))).toThrow(/Invalid analysis answer/);
  });
});

describe('getAnalysisData', () => {
  beforeEach(() => invokeMock.mockReset());

  it('decodes the binary answer and still takes the object of browser mocks', async () => {
    vi.resetModules();
    const { getAnalysisData } = await import('./tauri.svelte.ts');
    invokeMock.mockResolvedValueOnce(fixtureBuffer('analysis-pwv7.bin'));
    const fromBytes = await getAnalysisData(1);
    invokeMock.mockResolvedValueOnce(JSON.parse(fixtureText('analysis-pwv7.json')));
    const fromObject = await getAnalysisData(2);

    expect(fromBytes.bpm).toBe(fromObject.bpm);
    expect(fromBytes.beats).toEqual(fromObject.beats);
    expect(fromBytes.cue_points).toEqual(fromObject.cue_points);
    expect(Array.from(bandBytes(fromBytes.waveform_color_detail.bands))).toEqual(Array.from(bandBytes(fromObject.waveform_color_detail.bands)));
  });
});
