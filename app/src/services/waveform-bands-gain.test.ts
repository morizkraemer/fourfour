import { describe, expect, it } from 'vitest';

import { bandsFromInterleavedBytes } from './waveform-bands.ts';

function referenceBands(bytes: ArrayLike<number>, fullScale: number, calibration: number) {
  const columns = Math.floor(bytes.length / 3);
  const low = new Float32Array(columns);
  const mid = new Float32Array(columns);
  const high = new Float32Array(columns);
  for (let i = 0; i < columns; i++) {
    const l = bytes[i * 3];
    const m = bytes[i * 3 + 1];
    const h = bytes[i * 3 + 2];
    const max = Math.max(l, m, h);
    if (max === 0) continue;
    const gain = Math.min((max / fullScale) * calibration, 1) / max;
    low[i] = l * gain;
    mid[i] = m * gain;
    high[i] = h * gain;
  }
  return { low, mid, high };
}

function sameBits(a: Float32Array, b: Float32Array) {
  expect(a.length).toBe(b.length);
  expect(Array.from(new Uint32Array(a.buffer))).toEqual(Array.from(new Uint32Array(b.buffer)));
}

describe('bandsFromInterleavedBytes', () => {
  const noise = new Uint8Array(3 * 5000);
  let seed = 12345;
  for (let i = 0; i < noise.length; i++) {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    noise[i] = seed % 7 === 0 ? 0 : seed >> 8;
  }

  it.each([
    [127, 1],
    [127, 0.75],
    [127, 1.8],
    [255, 0.3],
    [100, 2.5],
  ])('gives the same float bits as the per-column formula (full scale %i, calibration %f)', (fullScale, calibration) => {
    const out = bandsFromInterleavedBytes(noise, fullScale, calibration);
    const expected = referenceBands(noise, fullScale, calibration);
    sameBits(out.low, expected.low);
    sameBits(out.mid, expected.mid);
    sameBits(out.high, expected.high);
  });

  it('takes plain arrays, including values above a byte', () => {
    const values = [0, 0, 0, 255, 3, 1, 300, 20, 7, 1, 2, 3];
    const out = bandsFromInterleavedBytes(values, 127, 1.1);
    const expected = referenceBands(values, 127, 1.1);
    sameBits(out.low, expected.low);
    sameBits(out.mid, expected.mid);
    sameBits(out.high, expected.high);
  });
});
