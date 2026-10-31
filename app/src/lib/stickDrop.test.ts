import { describe, expect, it } from 'vitest';
import { shortDate, stickDropLabel, syncAge } from './stickDrop.ts';

describe('stick drop label', () => {
  it('puts a library playlist on the stick, kept updated', () => {
    expect(stickDropLabel('SanDisk 32GB', { kind: 'playlist', fromStick: false, alreadyThere: false })).toBe(
      'Put on SanDisk 32GB · Keep updated',
    );
  });

  it('copies a playlist from another stick, kept updated by default', () => {
    expect(stickDropLabel('BACKUP', { kind: 'playlist', fromStick: true, alreadyThere: false })).toBe(
      'Copy to BACKUP · Keep updated',
    );
  });

  it('says when the playlist is already there', () => {
    expect(stickDropLabel('BACKUP', { kind: 'playlist', fromStick: true, alreadyThere: true })).toBe('Already on BACKUP');
  });

  it('counts tracks', () => {
    expect(stickDropLabel('A', { kind: 'tracks', count: 1 })).toBe('Add 1 track to A');
    expect(stickDropLabel('A', { kind: 'tracks', count: 3 })).toBe('Add 3 tracks to A');
  });
});

describe('sync age', () => {
  const now = Date.parse('2026-10-31T12:00:00Z');

  it('reads as a short relative time', () => {
    expect(syncAge('2026-10-31T11:59:40Z', now)).toBe('just now');
    expect(syncAge('2026-10-31T11:58:00Z', now)).toBe('2 min ago');
    expect(syncAge('2026-10-31T09:00:00Z', now)).toBe('3 h ago');
    expect(syncAge('2026-10-30T09:00:00Z', now)).toBe('yesterday');
    expect(syncAge('2026-10-03T09:00:00Z', now)).toBe(shortDate('2026-10-03T09:00:00Z'));
  });

  it('is empty for a bad date', () => {
    expect(syncAge('nope', now)).toBe('');
  });
});
