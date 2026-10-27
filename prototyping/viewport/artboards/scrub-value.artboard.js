import { el } from '../design-system/utils/dom.js';
import { host, ScrubValue, bpmScrub, keyScrub } from '../design-system/svelte.js';
import { keyColor } from '../design-system/utils/key-color.js';

export const meta = { title: 'Scrub Value', layer: 'primitive' };

function row(name, control) {
  return el(
    'div',
    { style: { display: 'grid', gridTemplateColumns: '96px 1fr', alignItems: 'center', gap: 'var(--ff-space-3)' } },
    [el('span', { style: { font: '500 10px var(--ff-font-mono)', letterSpacing: '0.06em', color: 'var(--ff-faint)' } }, [name]), control]
  );
}

function liveField(model, initial, label, colorOf) {
  const slot = el('div', { style: { display: 'contents' } });
  const mountWith = (value) => {
    slot.replaceChildren(
      host(ScrubValue, {
        model,
        value,
        label,
        color: colorOf?.(value),
        onCommit: (next) => mountWith(next),
      })
    );
  };
  mountWith(initial);
  return slot;
}

export default function render() {
  return el(
    'div',
    {
      style: {
        width: '280px',
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--ff-space-4)',
        padding: 'var(--ff-space-4)',
        background: 'var(--ff-surface)',
        border: '1px solid var(--ff-border)',
        borderRadius: 'var(--ff-radius-md)',
        fontSize: '12px',
        color: 'var(--ff-text)',
      },
    },
    [
      row('BPM', liveField(bpmScrub, 128, 'BPM')),
      row('KEY', liveField(keyScrub, '8A', 'Key', keyColor)),
      row('NOT ANALYZED', liveField(bpmScrub, 0, 'BPM')),
    ]
  );
}
