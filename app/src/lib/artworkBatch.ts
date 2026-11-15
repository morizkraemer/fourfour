/** Splits the `get_track_artworks` answer: a u32 count, one u32 byte length per track (0 for none), then the images. */
export function decodeArtworkBatch(buffer: ArrayBuffer): (ArrayBuffer | null)[] {
  const view = new DataView(buffer);
  const count = view.getUint32(0, true);
  let at = 4 + 4 * count;
  const images: (ArrayBuffer | null)[] = [];
  for (let i = 0; i < count; i++) {
    const length = view.getUint32(4 + 4 * i, true);
    images.push(length > 0 ? buffer.slice(at, at + length) : null);
    at += length;
  }
  return images;
}

export function encodeArtworkBatch(images: readonly (ArrayBuffer | null)[]): ArrayBuffer {
  const total = images.reduce((sum, image) => sum + (image?.byteLength ?? 0), 0);
  const out = new Uint8Array(4 + 4 * images.length + total);
  const view = new DataView(out.buffer);
  view.setUint32(0, images.length, true);
  let at = 4 + 4 * images.length;
  images.forEach((image, i) => {
    view.setUint32(4 + 4 * i, image?.byteLength ?? 0, true);
    if (image) out.set(new Uint8Array(image), at);
    at += image?.byteLength ?? 0;
  });
  return out.buffer;
}
