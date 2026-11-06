import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { bufferOf, env } from './fixtures.ts';
import { describe, expect, it } from 'vitest';
import { decodeStateBytes, decodeTrackTable } from './track-table.ts';

const PAIRS = (env.FF_STATE_PAIRS ?? '')
  .split(',')
  .filter(Boolean)
  .map((entry) => entry.split('=') as [string, string]);

describe.skipIf(PAIRS.length === 0)('binary load_state answer of a real library (FF_STATE_PAIRS=<state.json>=<state.bin>,...)', () => {
  for (const [jsonPath, binPath] of PAIRS) {
    it(`${binPath} decodes to the tracks and playlists of ${jsonPath}, row by row`, () => {
      const json = JSON.parse(readFileSync(jsonPath, 'utf8'));
      const decoded = decodeStateBytes(bufferOf(binPath));
      const expected = decodeTrackTable(json.tracks);
      expect(decoded.tracks.length).toBe(expected.length);
      const hash = (tracks: unknown[]) => {
        const sha = createHash('sha256');
        for (const track of tracks) sha.update(JSON.stringify(track) + '\n');
        return sha.digest('hex');
      };
      const wanted = hash(expected);
      expect(hash(decoded.tracks)).toBe(wanted);
      expect(JSON.stringify(decoded.playlists)).toBe(JSON.stringify(json.playlists));
      console.log(`${decoded.tracks.length} rows, sha256 of the rows ${wanted}`);
    });
  }
});
