import { CommandError, commandErrorKind, commandErrorPath, type ErrorKind } from '../services/command-error';

type Reply = string | ((match: RegExpMatchArray) => string);

interface Rule {
  test: RegExp;
  reply: Reply;
}

const SHARED_RULES: Rule[] = [
  { test: /no space left|disk full|os error 28|not enough space/i, reply: 'The disk is full. Free up some space and try again.' },
  {
    test: /permission denied|os error 13\b|read-only file system|os error 30\b|operation not permitted|access is denied/i,
    reply: "fourfour doesn't have permission to use that location. Pick another one.",
  },
  {
    test: /no such file|os error 2\b|file not found|cannot find the (file|path)|not a folder|not a directory/i,
    reply: 'A file or folder is missing. Check that the drive is connected, then try again.',
  },
  {
    test: /os error 5\b|input\/output error|device not configured|os error 6\b|no such device|os error 19\b/i,
    reply: 'The drive stopped responding. Check the connection and try again.',
  },
  { test: /database is locked|database is busy|sqlite_busy/i, reply: 'The library is busy. Wait a moment and try again.' },
  { test: /poison/i, reply: 'The library hit an internal problem. Restart fourfour and try again.' },
  {
    test: /unsupported schema version/i,
    reply: 'This library was made by a newer version of fourfour. Update the app to open it.',
  },
  {
    test: /file is not a database|database disk image is malformed/i,
    reply: "That library file is damaged or isn't a fourfour library.",
  },
];

type KindCauses = Partial<Record<ErrorKind, string>>;

const SHARED_KIND_CAUSES: KindCauses = {
  disk_full: 'The disk is full. Free up some space and try again.',
  no_space_on_stick: 'The disk is full. Free up some space and try again.',
  permission_denied: "fourfour doesn't have permission to use that location. Pick another one.",
  not_found: 'A file or folder is missing. Check that the drive is connected, then try again.',
  drive_gone: 'The drive stopped responding. Check the connection and try again.',
  library_locked: 'The library is busy. Wait a moment and try again.',
  library_newer_version: 'This library was made by a newer version of fourfour. Update the app to open it.',
  library_damaged: "That library file is damaged or isn't a fourfour library.",
};

const BACKUP_KIND_CAUSES: KindCauses = {
  library_newer_version: 'That backup was made by a newer version of fourfour. Update the app, then try again.',
};

const BACKUP_RULES: Rule[] = [
  { test: /not a library backup/i, reply: "That file isn't a fourfour library backup. Choose another file." },
  { test: /backup file is damaged|unsafe path|empty path/i, reply: 'That backup file is damaged. Choose another one.' },
  {
    test: /made by a newer version/i,
    reply: 'That backup was made by a newer version of fourfour. Update the app, then try again.',
  },
  { test: /backup is incomplete/i, reply: 'That backup is incomplete. Choose another one.' },
  { test: /backup has no library database/i, reply: "That backup doesn't hold a library. Choose another file." },
  { test: /library in the backup can't be opened/i, reply: "The library inside that backup can't be opened. Choose another file." },
  {
    test: /Could not save the current library first/i,
    reply: "Couldn't save your current library first, so nothing was changed. Try again.",
  },
  { test: /in-memory library/i, reply: "This library can't be backed up." },
];

const REKORDBOX_RULES: Rule[] = [
  {
    test: /decryption failed|set sqlcipher key/i,
    reply: "fourfour can't read this Rekordbox library. It may be from a Rekordbox version fourfour doesn't support yet. Try the XML export instead.",
  },
  { test: /Not a Rekordbox XML export/i, reply: "That file isn't a Rekordbox XML export. Choose the rekordbox.xml you exported from Rekordbox." },
  { test: /No PIONEER\/rekordbox\/exportLibrary\.db/i, reply: "That folder isn't a Rekordbox USB drive. Choose the top folder of the drive." },
  { test: /only has the legacy export\.pdb/i, reply: "That USB drive uses Rekordbox's older format, which fourfour can't read yet." },
];

const SYNC_RULES: Rule[] = [
  { test: /is already being synced/i, reply: 'That stick is already syncing. Wait for it to finish.' },
  { test: /was selected twice/i, reply: 'That stick was chosen twice. Sync it once.' },
  { test: /No USB sticks selected/i, reply: 'No stick is selected. Pick a stick to sync.' },
  { test: /Cannot open|Cannot read free space|Capacity is not available/i, reply: "fourfour can't reach the stick. Check that it is still plugged in, then sync again." },
  { test: /Cannot read the library on/i, reply: "fourfour can't read the library on that stick. Sync again to rewrite it." },
];

const EDIT_RULES: Rule[] = [
  { test: /^Title cannot be empty/i, reply: "The title can't be empty." },
  { test: /^(\w+) is longer than (\d+) characters/i, reply: (m) => `The ${m[1].toLowerCase()} can be at most ${m[2]} characters.` },
  { test: /contains a control character/i, reply: "That text has a character fourfour can't store. Remove it and try again." },
  { test: /no analysis to/i, reply: 'The track has to be analyzed first. Analyze it, then try again.' },
  { test: /is not a Camelot key/i, reply: "That isn't a Camelot key. Use a key like 8A." },
  { test: /outside \d+ to \d+|above \d+ stars/i, reply: 'That value is outside the allowed range.' },
  { test: /is not in the library|Track not found/i, reply: 'The track is no longer in the library.' },
];

const PLAYBACK_RULES: Rule[] = [
  { test: /^(Open .+: .*(no such file|os error 2\b|not found))/i, reply: 'The file is missing or its drive is off. Reconnect the drive or relocate the file.' },
  { test: /^Open .+: .*(permission denied|os error 13\b)/i, reply: "fourfour doesn't have permission to read the file." },
  {
    test: /Unsupported or corrupt audio file|Unsupported codec|Decode error|Read error|Stream reset loop|Decoder crashed|No default audio track|Unknown track duration|^Open /i,
    reply: "The file is damaged or in a format fourfour can't read.",
  },
  {
    test: /Audio output error|No default output audio device|output stream|does not support f32|enumerate output devices|Audio output not initialized|state not managed|\.manage\(\)/i,
    reply: "fourfour can't reach the audio output. Check your speakers or interface, or choose another output in Settings.",
  },
  { test: /Audio output changed while loading/i, reply: 'The audio output changed. Press play again.' },
  { test: /.*/, reply: 'Playback stopped unexpectedly. Press play to try again.' },
];

const ANALYSIS_RULES: Rule[] = [
  {
    test: /No analysis backend available/i,
    reply: "Analysis isn't available because its models are missing. Reinstall fourfour.",
  },
];

export function rawText(err: unknown): string {
  const text = err instanceof Error ? err.message : typeof err === 'string' ? err : JSON.stringify(err) ?? String(err);
  return text.replace(/^Error:\s*/, '');
}

export interface FailureOptions {
  next?: string;
  rules?: Rule[];
  kinds?: KindCauses;
}

function kindCause(err: unknown, overrides?: KindCauses): string | null {
  const kind = commandErrorKind(err);
  if (!kind || kind === 'other') return null;
  return overrides?.[kind] ?? SHARED_KIND_CAUSES[kind] ?? null;
}

function match(raw: string, rules: Rule[]): string | null {
  for (const rule of rules) {
    const found = raw.match(rule.test);
    if (found) return typeof rule.reply === 'function' ? rule.reply(found) : rule.reply;
  }
  return null;
}

function fileNameOf(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() || path;
}

/** What went wrong, in words a DJ can act on; `what` leads ("Couldn't import those files."), then the cause and next step. */
export function failureText(what: string, err: unknown, options: FailureOptions = {}): string {
  const raw = rawText(err);
  const cause = kindCause(err, options.kinds) ?? match(raw, [...(options.rules ?? []), ...SHARED_RULES]);
  return `${what} ${cause ?? options.next ?? 'Try again.'}`;
}

export function editFailureText(what: string, err: unknown): string {
  return failureText(`Couldn't ${what}.`, err, { rules: EDIT_RULES });
}

export function importFailureText(err: unknown): string {
  return failureText("Couldn't import those files.", err, { next: "Check that they're still there, then import again." });
}

export function backupSaveFailureText(err: unknown): string {
  return failureText("Couldn't save the backup.", err, { rules: BACKUP_RULES, kinds: BACKUP_KIND_CAUSES, next: 'Try again, or save it somewhere else.' });
}

export function backupReadFailureText(err: unknown): string {
  return failureText("Couldn't open that backup.", err, { rules: BACKUP_RULES, kinds: BACKUP_KIND_CAUSES, next: 'Choose another file.' });
}

const SAFETY_COPY_SENTENCE = 'Your previous library is saved at';

export function backupRestoreFailureText(err: unknown): string {
  const safetyPath = rawText(err).includes(SAFETY_COPY_SENTENCE) ? commandErrorPath(err) : null;
  const options = { rules: BACKUP_RULES, kinds: BACKUP_KIND_CAUSES };
  if (!safetyPath) return failureText("Couldn't restore the backup. Your library was not changed.", err, options);
  const cause = kindCause(err, options.kinds) ?? match(rawText(err), [...BACKUP_RULES, ...SHARED_RULES]);
  return `Couldn't restore the backup. ${cause ? `${cause} ` : ''}${SAFETY_COPY_SENTENCE} ${safetyPath}`;
}

export function rekordboxFailureText(err: unknown, what = "Couldn't read the Rekordbox library."): string {
  return failureText(what, err, { rules: REKORDBOX_RULES, next: 'Check the file or folder, then try again.' });
}

const SYNC_KIND_CAUSES: KindCauses = {
  stick_busy: 'That stick is already syncing. Wait for it to finish.',
  drive_gone: "fourfour can't reach the stick. Check that it is still plugged in, then sync again.",
};

export function syncFailureText(err: unknown, kind: ErrorKind | null = commandErrorKind(err)): string {
  const raw = rawText(err);
  if (kind === 'no_space_on_stick' || /^Not enough space/i.test(raw)) return raw;
  const typed = kind ? new CommandError(kind, raw) : err;
  return failureText('The sync stopped.', typed, { rules: SYNC_RULES, kinds: SYNC_KIND_CAUSES, next: 'Check the stick, then sync again.' });
}

export function relocateFailureText(err: unknown): string {
  return failureText("Couldn't relink the files.", err, {
    rules: [
      { test: /is not a readable file/i, reply: "fourfour can't read that file. Choose a different one." },
      { test: /is not a folder/i, reply: "That isn't a folder. Choose a folder." },
      { test: /is not in the library/i, reply: 'The track is no longer in the library.' },
    ],
    next: 'Choose the folder or file again.',
  });
}

export function libraryOpenFailureText(err: unknown): string {
  return failureText("Couldn't open a library in that folder.", err, { next: 'Choose another folder.' });
}

export function analysisFailureText(err: unknown): string {
  const kind = commandErrorKind(err);
  if (kind === 'analysis_failed' || /Analysis failed for all/i.test(rawText(err))) {
    return "None of the tracks could be analyzed. Check that the files play, then try again.";
  }
  return failureText("Couldn't start the analysis.", err, {
    rules: ANALYSIS_RULES,
    kinds: { models_missing: "Analysis isn't available because its models are missing. Reinstall fourfour." },
    next: 'Try again.',
  });
}

export function revealFailureText(err: unknown): string {
  return failureText("Couldn't show the file.", err, {
    rules: [{ test: /File not found/i, reply: 'It may have been moved or deleted.' }],
    kinds: { not_found: 'It may have been moved or deleted.' },
    next: 'Try again.',
  });
}

export function ejectFailureText(err: unknown): string {
  return failureText("Couldn't eject the stick.", err, {
    rules: [{ test: /busy|in use|resource busy|dissented/i, reply: 'Something is still using it. Close other apps using the stick, then try again.' }],
    kinds: { stick_busy: 'Something is still using it. Close other apps using the stick, then try again.' },
    next: 'Close anything using it, then try again.',
  });
}

const PLAYBACK_KIND_CAUSES: KindCauses = {
  not_found: 'The file is missing or its drive is off. Reconnect the drive or relocate the file.',
  drive_gone: 'The file is missing or its drive is off. Reconnect the drive or relocate the file.',
  permission_denied: "fourfour doesn't have permission to read the file.",
  decode_failed: "The file is damaged or in a format fourfour can't read.",
  audio_output_unavailable:
    "fourfour can't reach the audio output. Check your speakers or interface, or choose another output in Settings.",
};

export function playbackFailureText(rawMessage: string, trackPath?: string | null, kind: ErrorKind | null = null): string {
  const name = trackPath ? fileNameOf(trackPath) : null;
  const cause = (kind && kind !== 'other' ? PLAYBACK_KIND_CAUSES[kind] : null) ?? match(rawMessage, PLAYBACK_RULES);
  const lead = name ? `Can't play "${name}".` : "Can't play.";
  return `${lead} ${cause}`;
}

export function analysisFileFailureText(fileName: string): string {
  return `Couldn't analyze "${fileName}". Check that the file plays, then analyze it again.`;
}

export function analysisRunFailureText(count: number): string {
  return count === 1
    ? "1 track couldn't be analyzed. Check that it plays, then analyze it again."
    : `${count} tracks couldn't be analyzed. Check that they play, then analyze them again.`;
}
