import { resolve } from 'node:path';
import { parseCardPost, parseMediaArgs, parseOptionsArg } from '../src/model';
import { createStore } from '../server/store';
import { flagValue, flagValues } from './flags';

const usage = 'usage: npm run hub:card -- --title "..." --question "..." [--url /path] [--media label=path.png|gif|mp4 (repeat)] [--options a,b,c | a=Wide,b=Tight] [--id slug]';
const argv = process.argv;
const title = flagValue(argv, 'title');
if (title === undefined) throw new Error(usage);
const options = flagValue(argv, 'options');
const post = parseCardPost({
  id: flagValue(argv, 'id'),
  title,
  question: flagValue(argv, 'question'),
  url: flagValue(argv, 'url'),
  options: options === undefined ? undefined : parseOptionsArg(options),
});
const card = createStore(resolve(import.meta.dirname, '..')).saveCard(post, parseMediaArgs(flagValues(argv, 'media')));
console.log(`saved ${card.id} (round ${card.round}, ${card.gallery.length} media, ${card.options.length} options): data/cards/${card.id}.json`);
