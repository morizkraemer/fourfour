import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [raw, out] = process.argv.slice(2);
const read = (name) => (existsSync(join(raw, name)) ? readFileSync(join(raw, name), 'utf8') : '');
const jsonl = (name) => read(name).split('\n').filter(Boolean).map((line) => JSON.parse(line));
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

const [loadBefore, cores, loadAfter] = read('load.txt').split('\n');
const report = { when: new Date().toISOString(), cores: Number(cores), load_before: loadBefore?.replace('load_before=', ''), load_after: loadAfter?.replace('load_after=', ''), paths: {} };
const put = (path, key, value) => ((report.paths[path] ??= {})[key] = value);

const benches = [...jsonl('startup_cold.jsonl'), ...jsonl('load.jsonl'), ...jsonl('import.jsonl'), ...jsonl('analysis.jsonl')];
const warm = jsonl('startup_warm.jsonl');
if (warm.length) {
  const keys = Object.keys(warm[0].values);
  benches.push({ bench: 'startup_backend_warm', values: Object.fromEntries(keys.map((k) => [k, median(warm.map((w) => w.values[k]))])) });
}
const pathOf = (bench) => (bench.startsWith('startup') ? 'startup' : bench.startsWith('track') ? 'track_load' : bench.startsWith('import') ? 'import' : 'analysis');
for (const { bench, values } of benches) put(pathOf(bench), bench, values);

const latency = read('playback.txt').split('\n').filter(Boolean);
if (latency.length) {
  put('track_load', 'playback_engine', Object.fromEntries(latency.map((l) => {
    const [, name, med] = l.match(/^LATENCY (.+?): .*median ([\d.]+) ms/) ?? [];
    return [`${name} median ms`, Number(med)];
  })));
}

const fe = read('frontend-q50.json') ? JSON.parse(read('frontend-q50.json')) : null;
if (fe) {
  const pick = (prefix) => Object.fromEntries(Object.entries(fe).filter(([k]) => k.startsWith(prefix)));
  put('startup', 'frontend', pick('startup'));
  put('track_load', 'frontend', { ...pick('track load'), ...pick('expand'), ...pick('mock') });
  put('import', 'frontend', pick('import'));
}
const steady = read('frontend-perf10k.json') ? JSON.parse(read('frontend-perf10k.json')) : null;
if (steady) put('ui', 'frontend_perf10k', steady);

writeFileSync(out, JSON.stringify(report, null, 2));

const fmt = (v) => (typeof v === 'number' ? (Math.abs(v) >= 100 ? Math.round(v).toLocaleString('en-US') : String(Math.round(v * 10) / 10)) : String(v));
console.log(`\nQ50 perf  ${report.when}  cores=${report.cores}  load ${report.load_before} -> ${report.load_after}\n`);
for (const [path, groups] of Object.entries(report.paths)) {
  console.log(`== ${path}`);
  for (const [group, values] of Object.entries(groups)) {
    console.log(`  ${group}`);
    for (const [k, v] of Object.entries(values)) console.log(`    ${k.padEnd(46)} ${fmt(v)}`);
  }
}
