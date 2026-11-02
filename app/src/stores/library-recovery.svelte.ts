import {
  pickDirectory,
  recoverNewLibrary,
  recoverOpenLibrary,
  recoverRetryLibrary,
  type RecoveryOutcome,
} from '../services/tauri.svelte.ts';
import { parseOpenFailure, recoveryFailureText } from '../lib/libraryOpenFailure.ts';
import { library, initLibrary } from './library.svelte.ts';
import { chooseRestoreFile } from './backup.svelte.ts';

export const recovery = $state({ busy: false, error: '' });

function message(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

async function run(action: () => Promise<RecoveryOutcome>, onFailure?: (err: unknown) => void) {
  if (recovery.busy) return;
  recovery.busy = true;
  recovery.error = '';
  try {
    const outcome = await action();
    await finishRecovery(outcome);
  } catch (err) {
    const raw = message(err);
    console.info('library recovery failed:', raw);
    recovery.error = recoveryFailureText(err);
    onFailure?.(err);
  } finally {
    recovery.busy = false;
  }
}

export async function finishRecovery(outcome: RecoveryOutcome) {
  library.libraryPath = outcome.library_path;
  library.openFailure = null;
  await initLibrary();
  if (outcome.kept_path) {
    library.statusMessage = `Started on another library. Your previous library file is still at ${outcome.kept_path}.`;
  }
}

export function retryOpeningLibrary() {
  return run(recoverRetryLibrary, (err) => {
    library.openFailure = parseOpenFailure(err) ?? library.openFailure;
    recovery.error = "Still can't open the library. " + (library.openFailure?.kind === 'locked' ? 'Close the other program and try again.' : 'Choose another option below.');
  });
}

export async function openAnotherLibrary() {
  const folder = await pickDirectory();
  if (folder) await run(() => recoverOpenLibrary(folder));
}

export function startNewLibrary() {
  return run(recoverNewLibrary);
}

export async function restoreLibraryBackup() {
  recovery.error = '';
  await chooseRestoreFile();
}
