import { resolve } from 'node:path';
import { formatUnread, unreadNotes } from '../src/model';
import { createStore } from '../server/store';

const store = createStore(resolve(import.meta.dirname, '..'));
const all = process.argv.includes('--all');
const items = unreadNotes(store.listCards(), all ? new Set() : store.readSet());
console.log(formatUnread(items));
if (!all) store.markRead(items.map((i) => i.note.id));
