import { readFileSync } from 'node:fs';

const [, , firstPath, lastPath, topArg] = process.argv;
if (!firstPath || !lastPath) {
  console.error('usage: node perf/heap-diff.mjs first.heapsnapshot last.heapsnapshot [top]');
  process.exit(2);
}
const TOP = Number(topArg ?? 25);

function byConstructor(path) {
  const snapshot = JSON.parse(readFileSync(path, 'utf8'));
  const { node_fields: fields, node_types: types } = snapshot.snapshot.meta;
  const stride = fields.length;
  const typeAt = fields.indexOf('type');
  const nameAt = fields.indexOf('name');
  const sizeAt = fields.indexOf('self_size');
  const typeNames = types[typeAt];
  const counts = new Map();
  for (let i = 0; i < snapshot.nodes.length; i += stride) {
    const type = typeNames[snapshot.nodes[i + typeAt]];
    if (type === 'hidden' || type === 'synthetic') continue;
    const name = type === 'string' || type === 'concatenated string' ? `(${type})` : snapshot.strings[snapshot.nodes[i + nameAt]];
    const key = `${type} ${name.slice(0, 80)}`;
    const entry = counts.get(key) ?? { count: 0, size: 0 };
    entry.count++;
    entry.size += snapshot.nodes[i + sizeAt];
    counts.set(key, entry);
  }
  return counts;
}

const before = byConstructor(firstPath);
const after = byConstructor(lastPath);
const rows = [];
for (const [key, entry] of after) {
  const old = before.get(key) ?? { count: 0, size: 0 };
  rows.push({ key, dCount: entry.count - old.count, dKb: Math.round((entry.size - old.size) / 1024), count: entry.count });
}
const pad = (value, width) => String(value).padStart(width);
console.log('by self size growth');
for (const row of [...rows].sort((a, b) => b.dKb - a.dKb).slice(0, TOP)) console.log(`${pad(row.dKb, 9)} KB ${pad(row.dCount, 8)} objs (now ${pad(row.count, 8)})  ${row.key}`);
console.log('by object count growth');
for (const row of [...rows].sort((a, b) => b.dCount - a.dCount).slice(0, TOP)) console.log(`${pad(row.dKb, 9)} KB ${pad(row.dCount, 8)} objs (now ${pad(row.count, 8)})  ${row.key}`);
