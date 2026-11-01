import { describe, expect, it } from 'vitest';
import { pathBaseName } from './pathName.ts';

describe('pathBaseName', () => {
  it('takes the last segment of a posix path', () => {
    expect(pathBaseName('/Users/dj/Music/track.mp3')).toBe('track.mp3');
  });

  it('takes the last segment of a Windows path', () => {
    expect(pathBaseName('C:\\Users\\dj\\Music\\track.mp3')).toBe('track.mp3');
  });

  it('handles mixed separators', () => {
    expect(pathBaseName('D:\\Music/House\\track.flac')).toBe('track.flac');
  });

  it('ignores trailing separators on volume paths', () => {
    expect(pathBaseName('/Volumes/STICK/')).toBe('STICK');
    expect(pathBaseName('E:\\')).toBe('E:');
  });

  it('returns a bare name unchanged and empty for empty', () => {
    expect(pathBaseName('track.mp3')).toBe('track.mp3');
    expect(pathBaseName('')).toBe('');
  });
});
