import { el, mediaStrip } from './media';
import { type Card, type CardNote, cardAnswer, inboxOrder, isAnswered, isPicked, optionLabelOf, optionText, saveAck } from './model';

const app = document.getElementById('app')!;
let acks = new Map<string, string>();

async function loadCards(): Promise<Card[]> {
  const res = await fetch('/__lab/cards', { cache: 'no-store' });
  if (!res.ok) throw new Error(`cards ${res.status}`);
  return ((await res.json()) as { cards: Card[] }).cards;
}

async function postNote(card: string, text: string, pick: string | undefined): Promise<CardNote> {
  const res = await fetch('/__lab/note', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ card, text, ...(pick === undefined ? {} : { pick }) }),
  });
  const body = (await res.json()) as { note?: CardNote; error?: string };
  if (!res.ok || !body.note) throw new Error(body.error ?? `save failed (${res.status})`);
  return body.note;
}

function draftKey(card: Card): string {
  return `lab-draft-${card.id}-${card.round}`;
}

function readDraft(card: Card): string {
  try {
    return localStorage.getItem(draftKey(card)) ?? '';
  } catch {
    return '';
  }
}

function writeDraft(card: Card, value: string): void {
  try {
    if (value === '') localStorage.removeItem(draftKey(card));
    else localStorage.setItem(draftKey(card), value);
  } catch {
    return;
  }
}

function badge(card: Card): HTMLElement {
  const answer = cardAnswer(card);
  if (!isAnswered(card)) return el('span', 'badge todo', 'Waiting for you');
  return el('span', 'badge done', answer?.pick ? `Picked ${answer.pick}` : 'Noted');
}

function cardView(card: Card, rerender: () => void): HTMLElement {
  const root = el('article', isAnswered(card) ? 'card answered' : 'card');
  root.dataset.id = card.id;
  const ack = acks.get(card.id);
  if (ack) {
    root.classList.add('justSaved');
    root.append(el('div', 'ack', `\u2713 ${ack}`));
  }
  const head = el('div', 'head');
  head.append(el('h3', undefined, card.title), badge(card));
  root.append(head);
  if (card.question) root.append(el('p', 'question', card.question));
  if (card.gallery.length) root.append(mediaStrip(card.gallery, card.title, true));
  if (card.url) {
    const link = el('a', 'link', card.url);
    link.href = card.url;
    root.append(link);
  }
  const err = el('div', 'err');
  const answer = cardAnswer(card);
  const note = el('textarea');
  note.placeholder = 'Optional note';
  note.value = readDraft(card);
  note.addEventListener('input', () => writeDraft(card, note.value));

  const controls: HTMLButtonElement[] = [];

  async function save(text: string, pick: string | undefined, pressed: HTMLElement): Promise<void> {
    err.textContent = '';
    for (const c of controls) c.disabled = true;
    pressed.classList.add('saving');
    try {
      card.notes.push(await postNote(card.id, text, pick));
      writeDraft(card, '');
      acks = new Map([[card.id, saveAck(pick, text)]]);
      rerender();
    } catch (e) {
      for (const c of controls) c.disabled = false;
      pressed.classList.remove('saving');
      err.textContent = `Not saved: ${(e as Error).message}`;
    }
  }

  if (card.options.length) {
    const options = el('div', 'options');
    for (const o of card.options) {
      const b = el('button', isPicked(answer, o.key) ? 'option on' : 'option');
      b.type = 'button';
      b.append(el('b', undefined, o.key), el('span', 'optText', optionText(o)), el('span', 'tick', isPicked(answer, o.key) ? '\u2713 Picked' : ''));
      b.setAttribute('aria-pressed', String(isPicked(answer, o.key)));
      controls.push(b);
      b.addEventListener('click', () => void save(note.value.trim(), o.key, b));
      options.append(b);
    }
    root.append(options);
  }
  const row = el('div', 'noteRow');
  const send = el('button', 'send', 'Send note');
  send.type = 'button';
  controls.push(send);
  send.addEventListener('click', () => {
    if (note.value.trim() === '') {
      err.textContent = 'Write a note first';
      return;
    }
    void save(note.value.trim(), undefined, send);
  });
  row.append(note, send);
  root.append(row);
  if (answer) {
    const pick = answer.pick === null ? '' : `Picked ${answer.pick}${optionLabelOf(card, answer.pick)}`;
    root.append(el('div', 'saved', [pick, answer.note].filter((l) => l !== '').join('\n')));
  }
  root.append(err);
  return root;
}

async function render(): Promise<void> {
  let cards: Card[];
  try {
    cards = await loadCards();
  } catch (e) {
    app.replaceChildren(el('div', 'err', `Could not load cards: ${(e as Error).message}`));
    return;
  }
  const { todo, answered } = inboxOrder(cards);
  const rerender = (): void => void render();
  const nodes: HTMLElement[] = [el('h2', undefined, `To review (${todo.length})`)];
  if (!todo.length) nodes.push(el('div', 'empty', 'Nothing waiting.'));
  nodes.push(...todo.map((c) => cardView(c, rerender)));
  if (answered.length) {
    nodes.push(el('h2', undefined, `Answered (${answered.length})`));
    nodes.push(...answered.map((c) => cardView(c, rerender)));
  }
  app.replaceChildren(...nodes);
  app.querySelector('.justSaved')?.scrollIntoView({ block: 'center', behavior: 'smooth' });
}

document.getElementById('refresh')!.addEventListener('click', () => {
  acks = new Map();
  void render();
});
void render();
