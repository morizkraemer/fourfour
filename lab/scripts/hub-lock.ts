import { resolve } from 'node:path';
import { createStore } from '../server/store';

const id = process.argv.slice(2).find((a) => !a.startsWith('--'));
if (id === undefined) throw new Error('usage: npm run hub:lock -- <card-id>');
const card = createStore(resolve(import.meta.dirname, '..')).lockCard(id);
console.log(`locked ${card.id}: moved out of the inbox`);
