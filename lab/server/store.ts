import fs from 'node:fs';
import path from 'node:path';
import {
  type Card,
  type CardMeta,
  type CardNote,
  type CardPost,
  type MediaItem,
  type NotePost,
  checkPick,
  compareCards,
  isCardId,
  newCardMeta,
  newNote,
  parseCardMeta,
  parseNote,
  parseReadFile,
  serializeReadFile,
  updateCardMeta,
  withNotes,
} from '../src/model';

export interface MediaSource {
  label?: string;
  path: string;
}

function json(file: string): unknown {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function writeWhole(file: string, data: string): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const partial = `${file}.partial`;
  try {
    fs.writeFileSync(partial, data);
    fs.renameSync(partial, file);
  } catch (err) {
    fs.rmSync(partial, { force: true });
    throw err;
  }
}

function write(file: string, value: unknown): void {
  writeWhole(file, `${JSON.stringify(value, null, 2)}\n`);
}

function safeFileName(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9.]+/g, '-').replace(/^-+/, '');
}

export function createStore(root: string) {
  const cardsDir = path.resolve(root, 'data/cards');
  const readFile = path.resolve(root, 'data/.read.json');
  const mediaDir = path.resolve(root, 'media');
  const metaFile = (id: string) => path.join(cardsDir, `${id}.json`);
  const notesDir = (id: string) => path.join(cardsDir, id, 'notes');

  function cardIds(): string[] {
    if (!fs.existsSync(cardsDir)) return [];
    return fs
      .readdirSync(cardsDir)
      .filter((f) => f.endsWith('.json') && isCardId(f.slice(0, -5)))
      .map((f) => f.slice(0, -5))
      .sort();
  }

  function readMeta(id: string): CardMeta {
    if (!isCardId(id) || !fs.existsSync(metaFile(id))) throw new Error(`unknown card "${id}"`);
    const meta = parseCardMeta(json(metaFile(id)), id);
    if (meta.id !== id) throw new Error(`lab card ${id}: file holds id "${meta.id}"`);
    return meta;
  }

  function readNotes(id: string): CardNote[] {
    const dir = notesDir(id);
    if (!fs.existsSync(dir)) return [];
    return fs
      .readdirSync(dir)
      .filter((f) => f.endsWith('.json'))
      .map((f) => {
        const note = parseNote(json(path.join(dir, f)), `${id}/${f}`);
        if (`${note.id}.json` !== f) throw new Error(`lab card ${id}: note file ${f} holds id "${note.id}"`);
        return note;
      });
  }

  function readCard(id: string): Card {
    return withNotes(readMeta(id), readNotes(id));
  }

  function readSet(): Set<string> {
    return fs.existsSync(readFile) ? parseReadFile(json(readFile)) : new Set();
  }

  function copyMedia(id: string, round: number, sources: readonly MediaSource[]): MediaItem[] {
    return sources.map((src, i) => {
      if (!fs.existsSync(src.path)) throw new Error(`media file not found: ${src.path}`);
      const name = `r${round}-${i + 1}-${safeFileName(path.basename(src.path))}`;
      fs.mkdirSync(path.join(mediaDir, id), { recursive: true });
      fs.copyFileSync(src.path, path.join(mediaDir, id, name));
      const url = `/media/${id}/${name}`;
      return src.label === undefined ? { url } : { url, label: src.label };
    });
  }

  return {
    mediaDir,
    listCards: (): Card[] => cardIds().map(readCard).sort(compareCards),
    readCard,
    saveCard(post: CardPost, sources: readonly MediaSource[] = [], now = new Date()): Card {
      const existing = post.id !== undefined && fs.existsSync(metaFile(post.id)) ? readMeta(post.id) : null;
      const base = existing ? updateCardMeta(existing, post) : newCardMeta(post, new Set(cardIds()), now);
      const meta = sources.length ? { ...base, gallery: [...post.gallery, ...copyMedia(base.id, base.round, sources)] } : base;
      write(metaFile(meta.id), meta);
      return readCard(meta.id);
    },
    addNote(post: NotePost, now = new Date()): CardNote {
      const meta = readMeta(post.card);
      checkPick(meta, post.pick);
      const note = newNote(post, now, meta.round);
      write(path.join(notesDir(post.card), `${note.id}.json`), note);
      return note;
    },
    lockCard(id: string): Card {
      const meta = readMeta(id);
      write(metaFile(id), { ...meta, status: 'done' });
      return readCard(id);
    },
    readSet,
    markRead(ids: Iterable<string>): void {
      const read = readSet();
      for (const id of ids) read.add(id);
      writeWhole(readFile, serializeReadFile(read));
    },
  };
}
