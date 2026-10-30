export const CARD_STATUSES = ['open', 'done'] as const;
export type CardStatus = (typeof CARD_STATUSES)[number];

export interface MediaItem {
  url: string;
  label?: string;
}

export interface CardOption {
  key: string;
  label?: string;
}

export interface CardNote {
  id: string;
  text: string;
  pick?: string;
  time: string;
  round: number;
}

export interface CardMeta {
  id: string;
  title: string;
  question: string;
  url?: string;
  gallery: MediaItem[];
  options: CardOption[];
  status: CardStatus;
  round: number;
  created: string;
}

export interface Card extends CardMeta {
  notes: CardNote[];
}

export interface CardPost {
  id?: string;
  title: string;
  question: string;
  url?: string;
  gallery: MediaItem[];
  options: CardOption[];
}

export interface NotePost {
  card: string;
  text: string;
  pick?: string;
}

export const CARD_ID = /^[a-z0-9][a-z0-9-]{0,47}$/;
export const MEDIA_EXT = /\.(png|jpe?g|gif|webp|mp4|webm)$/i;
const VIDEO_EXT = /\.(webm|mp4)$/i;
const MAX_TEXT = 4000;

function fail(where: string, what: string): never {
  throw new Error(`lab card: ${where}: ${what}`);
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function text(v: unknown, where: string, required: boolean): string {
  if (v === undefined || v === null) return required ? fail(where, 'is required') : '';
  if (typeof v !== 'string') return fail(where, 'must be a string');
  if (v.length > MAX_TEXT) return fail(where, `longer than ${MAX_TEXT} chars`);
  if (required && v.trim() === '') return fail(where, 'must not be empty');
  return v;
}

export function isCardId(s: unknown): s is string {
  return typeof s === 'string' && CARD_ID.test(s);
}

export function slugify(title: string): string {
  const slug = title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48)
    .replace(/-+$/, '');
  return slug || 'card';
}

export function isVideoUrl(url: string): boolean {
  return VIDEO_EXT.test(url.split(/[?#]/)[0] ?? '');
}

export function parseMediaArgs(args: readonly string[]): { label?: string; path: string }[] {
  return args.map((arg) => {
    const eq = arg.indexOf('=');
    const label = eq > 0 && !arg.slice(0, eq).includes('/') ? arg.slice(0, eq) : undefined;
    const path = label === undefined ? arg : arg.slice(eq + 1);
    if (!MEDIA_EXT.test(path)) fail('--media', `"${path}" is not png, jpg, gif, webp, mp4 or webm`);
    return label === undefined ? { path } : { label, path };
  });
}

export function parseOptionsArg(arg: string): CardOption[] {
  const seen = new Set<string>();
  return arg
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part !== '')
    .map((part) => {
      const eq = part.indexOf('=');
      const key = eq > 0 ? part.slice(0, eq).trim() : part;
      const label = eq > 0 ? part.slice(eq + 1).trim() : undefined;
      if (!/^[A-Za-z0-9_-]{1,24}$/.test(key)) fail('--options', `bad option key "${key}"`);
      if (seen.has(key)) fail('--options', `duplicate option key "${key}"`);
      seen.add(key);
      return label ? { key, label } : { key };
    });
}

function parseGallery(v: unknown): MediaItem[] {
  if (v === undefined) return [];
  if (!Array.isArray(v)) return fail('gallery', 'must be a list');
  return v.map((item, i) => {
    if (!isRecord(item)) return fail(`gallery[${i}]`, 'must be an object');
    const url = text(item.url, `gallery[${i}].url`, true);
    if (!url.startsWith('/media/') || !MEDIA_EXT.test(url)) fail(`gallery[${i}].url`, 'must be a /media/ png, jpg, gif, webp, mp4 or webm');
    const label = text(item.label, `gallery[${i}].label`, false);
    return label ? { url, label } : { url };
  });
}

function parseOptions(v: unknown): CardOption[] {
  if (v === undefined) return [];
  if (!Array.isArray(v)) return fail('options', 'must be a list');
  return v.map((o, i) => {
    if (!isRecord(o)) return fail(`options[${i}]`, 'must be an object');
    const key = text(o.key, `options[${i}].key`, true);
    const label = text(o.label, `options[${i}].label`, false);
    return label ? { key, label } : { key };
  });
}

export function parseCardPost(v: unknown): CardPost {
  if (!isRecord(v)) return fail('post', 'must be an object');
  const id = v.id === undefined ? undefined : text(v.id, 'id', true);
  if (id !== undefined && !isCardId(id)) fail('id', 'must be lowercase letters, digits and hyphens');
  const url = text(v.url, 'url', false);
  const post: CardPost = {
    title: text(v.title, 'title', true),
    question: text(v.question, 'question', false),
    gallery: parseGallery(v.gallery),
    options: parseOptions(v.options),
  };
  if (id !== undefined) post.id = id;
  if (url) post.url = url;
  return post;
}

export function parseCardMeta(v: unknown, where: string): CardMeta {
  if (!isRecord(v)) return fail(where, 'must be an object');
  const post = parseCardPost(v);
  const id = text(v.id, `${where}.id`, true);
  if (!isCardId(id)) fail(`${where}.id`, 'bad id');
  const status = v.status;
  if (status !== 'open' && status !== 'done') return fail(`${where}.status`, 'must be open or done');
  const round = v.round;
  if (typeof round !== 'number' || !Number.isInteger(round) || round < 1) return fail(`${where}.round`, 'must be a positive integer');
  return { ...post, id, status, round, created: text(v.created, `${where}.created`, true) };
}

export function parseNote(v: unknown, where: string): CardNote {
  if (!isRecord(v)) return fail(where, 'must be an object');
  const pick = v.pick === undefined ? undefined : text(v.pick, `${where}.pick`, true);
  const round = v.round;
  if (typeof round !== 'number' || !Number.isInteger(round) || round < 1) return fail(`${where}.round`, 'must be a positive integer');
  const note: CardNote = { id: text(v.id, `${where}.id`, true), text: text(v.text, `${where}.text`, false), time: text(v.time, `${where}.time`, true), round };
  if (pick !== undefined) note.pick = pick;
  return note;
}

export function parseNotePost(v: unknown): NotePost {
  if (!isRecord(v)) return fail('note', 'must be an object');
  const post: NotePost = { card: text(v.card, 'card', true), text: text(v.text, 'text', false) };
  if (v.pick !== undefined) post.pick = text(v.pick, 'pick', true);
  if (post.text.trim() === '' && post.pick === undefined) fail('note', 'needs a pick or text');
  return post;
}

export function newCardMeta(post: CardPost, existingIds: ReadonlySet<string>, now: Date): CardMeta {
  const base = post.id ?? slugify(post.title);
  let id = base;
  for (let n = 2; existingIds.has(id) && post.id === undefined; n++) id = `${base.slice(0, 44)}-${n}`;
  return { ...post, id, status: 'open', round: 1, created: now.toISOString() };
}

export function updateCardMeta(card: CardMeta, post: CardPost): CardMeta {
  return { ...card, ...post, id: card.id, status: 'open', round: card.round + 1 };
}

export function newNote(post: NotePost, now: Date, round: number, random: () => number = Math.random): CardNote {
  const stamp = now.toISOString().replace(/[-:.]/g, '');
  const rand = Math.floor(random() * 36 ** 4).toString(36).padStart(4, '0');
  const note: CardNote = { id: `${stamp}-${rand}`, text: post.text, time: now.toISOString(), round };
  if (post.pick !== undefined) note.pick = post.pick;
  return note;
}

export function checkPick(card: Pick<CardMeta, 'id' | 'options'>, pick: string | undefined): void {
  if (pick === undefined) return;
  if (!card.options.some((o) => o.key === pick)) fail(card.id, `pick "${pick}" is not one of ${card.options.map((o) => o.key).join(', ') || '(no options)'}`);
}

export function currentRoundNotes(card: Card): CardNote[] {
  return card.notes.filter((n) => n.round === card.round);
}

export function withNotes(meta: CardMeta, notes: readonly CardNote[]): Card {
  return { ...meta, notes: [...notes].sort((a, b) => a.id.localeCompare(b.id)) };
}

export function compareCards(a: CardMeta, b: CardMeta): number {
  return b.created.localeCompare(a.created) || a.id.localeCompare(b.id);
}

export function optionText(option: CardOption): string {
  return option.label ?? option.key;
}

export function optionLabelOf(card: Pick<CardMeta, 'options'>, key: string): string {
  const o = card.options.find((x) => x.key === key);
  return o?.label ? ` (${o.label})` : '';
}

export function mediaCaption(item: MediaItem, index: number, count: number): string {
  return item.label ?? (count > 1 ? String.fromCharCode(97 + index) : '');
}

export interface CardAnswer {
  pick: string | null;
  note: string;
}

export function cardAnswer(card: Card): CardAnswer | null {
  const notes = currentRoundNotes(card);
  if (!notes.length) return null;
  const pick = [...notes].reverse().find((n) => n.pick !== undefined)?.pick ?? null;
  const last = [...notes].reverse().find((n) => n.text !== '');
  return { pick, note: last?.text ?? '' };
}

export function isPicked(answer: CardAnswer | null, key: string): boolean {
  return answer?.pick === key;
}

export function saveAck(pick: string | undefined, text: string): string {
  if (pick !== undefined) return text === '' ? `Saved: option ${pick}` : `Saved: option ${pick} and note`;
  return 'Note saved';
}

export function isAnswered(card: Card): boolean {
  const answer = cardAnswer(card);
  if (!answer) return false;
  return card.options.length > 0 ? answer.pick !== null : true;
}

export function inboxOrder(cards: readonly Card[]): { todo: Card[]; answered: Card[] } {
  const open = cards.filter((c) => c.status === 'open').sort(compareCards);
  return { todo: open.filter((c) => !isAnswered(c)), answered: open.filter(isAnswered) };
}

export interface UnreadNote {
  card: CardMeta;
  note: CardNote;
}

export function unreadNotes(cards: readonly Card[], read: ReadonlySet<string>): UnreadNote[] {
  return cards
    .flatMap((card) => card.notes.map((note) => ({ card, note })))
    .filter(({ note }) => !read.has(note.id))
    .sort((a, b) => a.note.id.localeCompare(b.note.id));
}

export function formatUnread(items: readonly UnreadNote[]): string {
  if (!items.length) return 'no unread notes';
  return items
    .map(({ card, note }) => {
      const head = `[${card.id}] ${card.title} (round ${note.round}, ${note.time})`;
      const pick = note.pick === undefined ? '' : `PICK ${note.pick}${optionLabelOf(card, note.pick)}`;
      return [head, pick, note.text].filter((l) => l !== '').join('\n');
    })
    .join('\n\n');
}

export function parseReadFile(v: unknown): Set<string> {
  if (!Array.isArray(v) || v.some((x) => typeof x !== 'string')) return fail('read file', 'must be a list of note ids');
  return new Set(v as string[]);
}

export function serializeReadFile(read: ReadonlySet<string>): string {
  return `${JSON.stringify([...read].sort(), null, 2)}\n`;
}
