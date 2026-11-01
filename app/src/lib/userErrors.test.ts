import { describe, expect, it } from 'vitest';
import {
  analysisFailureText,
  backupReadFailureText,
  backupRestoreFailureText,
  backupSaveFailureText,
  ejectFailureText,
  failureText,
  importFailureText,
  playbackFailureText,
  rekordboxFailureText,
  syncFailureText,
} from './userErrors.ts';

const INTERNALS = /os error|Error:|poison|\/Users\/|SQLITE|anyhow|Failed to |statvfs|\{|\}/;

function expectPlain(text: string) {
  expect(text).not.toMatch(INTERNALS);
  expect(text).not.toMatch(/[–—!]/);
  expect(text.length).toBeLessThan(220);
}

describe('failureText', () => {
  it('leads with what happened and ends with a next step', () => {
    expect(failureText("Couldn't import those files.", 'x')).toBe("Couldn't import those files. Try again.");
  });

  it('turns raw io errors into a cause and a next step', () => {
    const text = failureText("Couldn't save the backup.", new Error('Failed to create /Users/dj/b.partial: No space left on device (os error 28)'));
    expect(text).toBe("Couldn't save the backup. The disk is full. Free up some space and try again.");
    expectPlain(text);
  });

  it('never repeats paths or lock internals', () => {
    expectPlain(importFailureText('poisoned lock: another task failed inside'));
    expectPlain(importFailureText('Permission denied (os error 13)'));
    expect(importFailureText('database is locked')).toContain('library is busy');
  });
});

describe('playbackFailureText', () => {
  it('names the file, not its folder', () => {
    const text = playbackFailureText('Open /Users/dj/Music/a.mp3: No such file or directory (os error 2)', '/Users/dj/Music/a.mp3');
    expect(text).toBe('Can\'t play "a.mp3". The file is missing or its drive is off. Reconnect the drive or relocate the file.');
    expectPlain(text);
  });

  it('separates a damaged file from a lost audio output', () => {
    expect(playbackFailureText('Decode error in /m/a.flac: bad frame', '/m/a.flac')).toContain('damaged');
    expect(playbackFailureText('Audio output error: device unplugged')).toBe(
      "Can't play. fourfour can't reach the audio output. Check your speakers or interface, or choose another output in Settings.",
    );
  });

  it('hides the shell state error when the audio engine never started', () => {
    const raw = 'state not managed for field `engine` on command `playback_play`. You must call `.manage()` before using this command';
    expect(playbackFailureText(raw, '/m/a.mp3')).toContain("can't reach the audio output");
  });

  it('hides internal decoder wording', () => {
    expectPlain(playbackFailureText('decode buffer missing', '/m/a.mp3'));
    expectPlain(playbackFailureText('Stream reset loop in /m/a.mp3', '/m/a.mp3'));
  });
});

describe('backup messages', () => {
  it('keeps the restore safety copy location', () => {
    const raw = 'Restore failed: The backup file is damaged. Your previous library is saved at /Users/dj/before-restore-1.ffbackup.gz';
    expect(backupRestoreFailureText(raw)).toBe(
      "Couldn't restore the backup. That backup file is damaged. Choose another one. Your previous library is saved at /Users/dj/before-restore-1.ffbackup.gz",
    );
    expect(backupRestoreFailureText('Failed to write /x/library.db: whatever')).toBe(
      "Couldn't restore the backup. Your library was not changed. Try again.",
    );
  });

  it('says what is wrong with a file that is not a backup', () => {
    expect(backupReadFailureText(new Error('This file is not a library backup'))).toBe(
      "Couldn't open that backup. That file isn't a fourfour library backup. Choose another file.",
    );
  });

  it('tells a permission problem from an unknown one', () => {
    expect(backupSaveFailureText('Failed to create /Volumes/X/b.partial: Permission denied (os error 13)')).toContain('permission');
    expect(backupSaveFailureText('whatever')).toBe("Couldn't save the backup. Try again, or save it somewhere else.");
  });
});

describe('sync, eject, analysis and rekordbox', () => {
  it("passes the engine's own space message through", () => {
    const raw = 'Not enough space on STICK: needs 2.0 GB, 1.0 GB free. Nothing was written.';
    expect(syncFailureText(raw)).toBe(raw);
  });

  it('turns an unreachable stick into a next step', () => {
    expect(syncFailureText('Cannot open /Volumes/STICK: No such file or directory (os error 2)')).toContain('plugged in');
  });

  it('explains a busy stick on eject', () => {
    expect(ejectFailureText('Eject failed: Volume STICK failed to eject: dissented by PID 12')).toContain('Something is still using it');
  });

  it('says when analysis models are missing', () => {
    expect(analysisFailureText('No analysis backend available: the Rust model weights (beat_this.onnx) were not found')).toBe(
      "Couldn't start the analysis. Analysis isn't available because its models are missing. Reinstall fourfour.",
    );
    expect(analysisFailureText('Analysis failed for all 4 tracks - see debug log')).toBe(
      'None of the tracks could be analyzed. Check that the files play, then try again.',
    );
  });

  it('points a Rekordbox decryption failure at the XML export', () => {
    expect(rekordboxFailureText('master.db decryption failed: not a Rekordbox master.db or wrong key')).toContain('XML export');
  });
});
