import { describe, expect, it } from 'vitest';
import { type Card, cardAnswer, inboxOrder, isAnswered, isPicked, newCardMeta, newNote, parseCardPost, parseMediaArgs, parseNotePost, parseOptionsArg, saveAck, slugify, withNotes } from './model';

const NOW = new Date('2026-10-30T10:00:00.000Z');

function card(over: Partial<Card> = {}): Card {
  return { id: 'c', title: 'C', question: '', gallery: [], options: [], status: 'open', round: 1, created: NOW.toISOString(), notes: [], ...over };
}

describe('argument parsing', () => {
  it('parses labelled and bare media, rejects other file types', () => {
    expect(parseMediaArgs(['old=/a/x.png', '/b/y.mp4'])).toEqual([{ label: 'old', path: '/a/x.png' }, { path: '/b/y.mp4' }]);
    expect(() => parseMediaArgs(['a=/x.txt'])).toThrow(/not png/);
  });
  it('parses keyed options with optional labels and rejects duplicates', () => {
    expect(parseOptionsArg('a,b=Tight')).toEqual([{ key: 'a' }, { key: 'b', label: 'Tight' }]);
    expect(() => parseOptionsArg('a,a')).toThrow(/duplicate/);
  });
  it('requires a title and validates gallery urls', () => {
    expect(() => parseCardPost({ question: 'q' })).toThrow(/title/);
    expect(() => parseCardPost({ title: 't', gallery: [{ url: '/etc/x.png' }] })).toThrow(/\/media\//);
  });
});

describe('card ids and notes', () => {
  it('slugifies titles and numbers duplicates', () => {
    expect(slugify('Settings: label clipped!')).toBe('settings-label-clipped');
    const post = parseCardPost({ title: 'Same' });
    expect(newCardMeta(post, new Set(['same']), NOW).id).toBe('same-2');
  });
  it('needs a pick or text in a note', () => {
    expect(() => parseNotePost({ card: 'c', text: '' })).toThrow(/pick or text/);
    expect(parseNotePost({ card: 'c', text: '', pick: 'a' })).toEqual({ card: 'c', text: '', pick: 'a' });
  });
});

describe('answers', () => {
  const options = [{ key: 'a' }, { key: 'b' }];
  const note = (pick: string | undefined, text: string, round: number, t: number) => newNote({ card: 'c', text, ...(pick ? { pick } : {}) }, new Date(NOW.getTime() + t), round, () => 0);

  it('is unanswered until a pick exists when the card has options', () => {
    expect(isAnswered(card({ options }))).toBe(false);
    expect(isAnswered(card({ options, notes: [note(undefined, 'hm', 1, 0)] }))).toBe(false);
    expect(isAnswered(card({ options, notes: [note('a', '', 1, 0)] }))).toBe(true);
  });
  it('uses the latest pick and latest text, and ignores earlier rounds', () => {
    const c = withNotes(card({ options, round: 2 }), [note('a', 'old', 1, 0), note('a', '', 2, 1000), note('b', 'brighter', 2, 2000)]);
    expect(cardAnswer(c)).toEqual({ pick: 'b', note: 'brighter' });
    expect(cardAnswer(withNotes(card({ options, round: 3 }), c.notes))).toBeNull();
  });
  it('splits the inbox into todo and answered, newest first, hiding done cards', () => {
    const older = card({ id: 'o', created: '2026-10-29T00:00:00.000Z' });
    const answered = card({ id: 'a', options, notes: [note('a', '', 1, 0)] });
    const done = card({ id: 'd', status: 'done' });
    const { todo, answered: ans } = inboxOrder([older, card({ id: 'n' }), answered, done]);
    expect(todo.map((c) => c.id)).toEqual(['n', 'o']);
    expect(ans.map((c) => c.id)).toEqual(['a']);
  });
});

describe('pick state', () => {
  it('marks only the latest picked option', () => {
    const answer = { pick: 'b', note: '' };
    expect(isPicked(answer, 'b')).toBe(true);
    expect(isPicked(answer, 'a')).toBe(false);
    expect(isPicked(null, 'a')).toBe(false);
    expect(isPicked({ pick: null, note: 'x' }, 'a')).toBe(false);
  });
  it('words the save confirmation', () => {
    expect(saveAck('a', '')).toBe('Saved: option a');
    expect(saveAck('a', 'x')).toBe('Saved: option a and note');
    expect(saveAck(undefined, 'x')).toBe('Note saved');
  });
});
