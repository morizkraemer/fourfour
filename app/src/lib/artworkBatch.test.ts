import { describe, expect, it } from 'vitest';
import { decodeArtworkBatch, encodeArtworkBatch } from './artworkBatch.ts';

const bytes = (...values: number[]) => new Uint8Array(values).buffer;

describe('artwork batch', () => {
  it('splits the images in request order and maps empty entries to null', () => {
    const out = decodeArtworkBatch(encodeArtworkBatch([bytes(1, 2, 3), null, bytes(9), null]));
    expect(out.map((image) => image && [...new Uint8Array(image)])).toEqual([[1, 2, 3], null, [9], null]);
  });

  it('reads the byte layout the backend writes', () => {
    const wire = new Uint8Array([2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 7, 8]).buffer;
    const out = decodeArtworkBatch(wire);
    expect(out.map((image) => image && [...new Uint8Array(image)])).toEqual([[7, 8], null]);
  });

  it('reads an empty batch', () => {
    expect(decodeArtworkBatch(new Uint8Array([0, 0, 0, 0]).buffer)).toEqual([]);
  });
});
