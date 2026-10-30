import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { formatUnread, parseCardPost, parseNotePost, parseOptionsArg, unreadNotes } from '../src/model';
import { createStore } from './store';

const NOW = new Date('2026-10-30T10:00:00.000Z');

function withRoot(run: (root: string) => void): void {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'lab-store-'));
  try {
    run(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

describe('store', () => {
  it('saves a card with media, records picks and notes, and prints them once through hub:notes logic', () => {
    withRoot((root) => {
      const png = path.join(root, 'shot.png');
      fs.writeFileSync(png, 'png');
      const store = createStore(root);
      const card = store.saveCard(parseCardPost({ title: 'Settings clipped', question: 'Fix how?', options: parseOptionsArg('a=Wrap,b=Widen') }), [{ label: 'now', path: png }], NOW);
      expect(card.id).toBe('settings-clipped');
      expect(card.gallery).toEqual([{ url: '/media/settings-clipped/r1-1-shot.png', label: 'now' }]);
      expect(fs.existsSync(path.join(root, 'media/settings-clipped/r1-1-shot.png'))).toBe(true);

      store.addNote(parseNotePost({ card: card.id, text: '', pick: 'a' }), NOW);
      store.addNote(parseNotePost({ card: card.id, text: 'but smaller', pick: 'b' }), new Date(NOW.getTime() + 1000));
      expect(() => store.addNote(parseNotePost({ card: card.id, text: '', pick: 'zzz' }), NOW)).toThrow(/not one of/);

      const out = formatUnread(unreadNotes(store.listCards(), store.readSet()));
      expect(out).toContain('PICK a (Wrap)');
      expect(out).toContain('PICK b (Widen)\nbut smaller');

      store.markRead(unreadNotes(store.listCards(), store.readSet()).map((i) => i.note.id));
      expect(formatUnread(unreadNotes(store.listCards(), store.readSet()))).toBe('no unread notes');
    });
  });

  it('starts a new round when a card id is posted again, and locks cards', () => {
    withRoot((root) => {
      const store = createStore(root);
      store.saveCard(parseCardPost({ id: 'x', title: 'X', options: [{ key: 'a' }] }), [], NOW);
      store.addNote(parseNotePost({ card: 'x', text: '', pick: 'a' }), NOW);
      const again = store.saveCard(parseCardPost({ id: 'x', title: 'X v2', options: [{ key: 'a' }] }), [], NOW);
      expect(again.round).toBe(2);
      expect(again.notes).toHaveLength(1);
      expect(store.lockCard('x').status).toBe('done');
    });
  });

  it('rejects unknown cards', () => {
    withRoot((root) => {
      expect(() => createStore(root).addNote(parseNotePost({ card: 'nope', text: 'hi' }), NOW)).toThrow(/unknown card/);
    });
  });
});
