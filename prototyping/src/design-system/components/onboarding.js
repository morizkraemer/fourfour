import './onboarding.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {function} [options.onImport]
 * @param {function} [options.onOpenFolder]
 */
export function createOnboarding({ onImport, onOpenFolder } = {}) {
  const wordmark = el('h1', { class: 'ff-onboarding__wordmark' }, ['fourfour']);

  const message = el('p', { class: 'ff-onboarding__message' }, [
    'Import your music library, curate playlists, and sync to Pioneer CDJ-compatible USB drives — all without Rekordbox.'
  ]);

  const importBtn = el('button', {
    class: 'ff-onboarding__btn ff-onboarding__btn--primary',
    onClick: onImport
  }, ['Import Library']);

  const openFolderBtn = el('button', {
    class: 'ff-onboarding__btn ff-onboarding__btn--default',
    onClick: onOpenFolder
  }, ['Open Folder…']);

  const actions = el('div', { class: 'ff-onboarding__actions' }, [importBtn, openFolderBtn]);

  const hint = el('div', { class: 'ff-onboarding__hint' }, [
    'or press ',
    el('kbd', { class: 'ff-onboarding__kbd' }, ['⌘O']),
    ' to open a folder'
  ]);

  const element = el('div', { class: 'ff-onboarding' }, [
    wordmark,
    message,
    actions,
    hint
  ]);

  return { element };
}
