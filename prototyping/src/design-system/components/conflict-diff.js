import './conflict-diff.css';
import { el } from '../utils/dom.js';

/**
 * @param {object} options
 * @param {string} options.title
 * @param {string} [options.sub]
 * @param {Array<{ type: 'add'|'remove'|'changed', label: string, name: string, subInfo?: string, source?: string }>} options.rows
 * @param {function} [options.onAction]      – triggers when header buttons are clicked (e.g. 'discard', 'keep-both', 'apply-local')
 */
export function createConflictDiff({ title, sub = '', rows = [], onAction } = {}) {
  const leftHeader = el('div', { class: 'ff-conflict-diff__header-left' }, [
    el('h2', { class: 'ff-conflict-diff__title' }, [title]),
    sub ? el('span', { class: 'ff-conflict-diff__subtitle' }, [sub]) : null
  ]);

  const discardBtn = el('button', {
    class: 'ff-conflict-diff__action ff-conflict-diff__action--ghost',
    onClick: () => { if (typeof onAction === 'function') onAction('discard'); }
  }, ['Discard']);

  const keepBothBtn = el('button', {
    class: 'ff-conflict-diff__action ff-conflict-diff__action--default',
    onClick: () => { if (typeof onAction === 'function') onAction('keep-both'); }
  }, ['Keep both']);

  const applyLocalBtn = el('button', {
    class: 'ff-conflict-diff__action ff-conflict-diff__action--primary',
    onClick: () => { if (typeof onAction === 'function') onAction('apply-local'); }
  }, ['Apply local']);

  const rightHeader = el('div', { class: 'ff-conflict-diff__header-right' }, [
    discardBtn,
    keepBothBtn,
    applyLocalBtn
  ]);

  const header = el('div', { class: 'ff-conflict-diff__header' }, [
    leftHeader,
    rightHeader
  ]);

  const rowElements = rows.map(row => {
    let markerText = '';
    if (row.type === 'add') markerText = '+';
    else if (row.type === 'remove') markerText = '−';
    else if (row.type === 'changed') markerText = '~';

    const markerEl = el('span', {
      class: `ff-conflict-diff__row-marker ff-conflict-diff__row-marker--${row.type}`
    }, [markerText]);

    const labelEl = el('span', { class: 'ff-conflict-diff__row-label' }, [row.label]);

    const nameEl = el('span', { class: 'ff-conflict-diff__row-name' }, [
      row.name,
      row.subInfo ? el('span', { class: 'ff-conflict-diff__row-sub-info' }, [` ${row.subInfo}`]) : null
    ]);

    const sourceEl = el('span', { class: 'ff-conflict-diff__row-source' }, [row.source || '']);

    return el('div', { class: 'ff-conflict-diff__row' }, [
      markerEl,
      labelEl,
      nameEl,
      sourceEl
    ]);
  });

  const body = el('div', { class: 'ff-conflict-diff__body' }, rowElements);

  const element = el('div', { class: 'ff-conflict-diff' }, [
    header,
    body
  ]);

  return { element };
}
