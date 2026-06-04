import './module-global-header.css';
import { el } from '../design-system/utils/dom.js';
import { createIcon, createKbd } from '../design-system/index.js';

export const meta = { title: 'Module · Global Header (search)', layer: 'module' };

/*
 * Global header band with a centered search box (Pencil "Global Header").
 * The v3 main layout omits this top band — search lives in the panel header —
 * so the Browse composite does not use it, but it ships as a complete module.
 * Exports buildGlobalHeader().
 */
export function buildGlobalHeader() {
  const search = el('div', { class: 'ff-gheader__search' }, [
    createIcon({ name: 'search', size: 12 }).element,
    el('span', { class: 'ff-gheader__placeholder' }, ['Search tracks, artists, labels…']),
    createKbd({ key: '⌘F' }).element,
  ]);
  return el('div', { class: 'ff-gheader' }, [search]);
}

export default function render() {
  return el('div', { class: 'ff-gheader-demo' }, [buildGlobalHeader()]);
}
