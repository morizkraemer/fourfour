import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const [raw, out, size] = process.argv.slice(2);
const read = (name) => (existsSync(join(raw, name)) ? readFileSync(join(raw, name), 'utf8') : '');
const jsonl = (name) => read(name).split('\n').filter(Boolean).map((line) => JSON.parse(line));
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

const [loadBefore, cores, loadAfter] = read('load.txt').split('\n');
const fixture = read('fixture.json') ? JSON.parse(read('fixture.json')) : null;
const report = { when: new Date().toISOString(), size: size ?? null, fixture, cores: Number(cores), load_before: loadBefore?.replace('load_before=', ''), load_after: loadAfter?.replace('load_after=', ''), paths: {} };
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
const bulk = read('frontend-bulk.json') ? JSON.parse(read('frontend-bulk.json')) : null;
if (bulk) {
  const keep = ['frameP95', 'frameMax', 'framesLost', 'taskMsPerSec', 'longTaskMsTotal', 'longTaskMsMax', 'keyMedian', 'keyWorst', 'paletteMedian', 'paletteWorst'];
  const flat = {};
  for (const [scenario, values] of Object.entries(bulk.results)) {
    for (const k of keep) if (k in values) flat[`${scenario} ${k}`] = values[k];
  }
  put('ui', 'during_bulk_analysis', flat);
}

const at = (path) => path.split('.').reduce((node, key) => node?.[key], report.paths);
const sum = (...paths) => (paths.every((p) => typeof at(p) === 'number') ? paths.reduce((s, p) => s + at(p), 0) : undefined);
const derived = {
  'startup cold first row ms': sum('startup.startup_backend_cold.load_state_ms', 'startup.frontend.startup cold first row ms'),
  'startup cold interactive ms': sum('startup.startup_backend_cold.load_state_ms', 'startup.frontend.startup cold interactive ms'),
  'startup warm first row ms': sum('startup.startup_backend_warm.load_state_ms', 'startup.frontend.startup warm first row ms'),
  'startup warm interactive ms': sum('startup.startup_backend_warm.load_state_ms', 'startup.frontend.startup warm interactive ms'),
  'track load to first audio ms': sum('track_load.track_load_backend.get_analysis_data_ms_median', 'track_load.playback_engine.load(with duration) -> first audio median ms'),
  'import end to end ms': sum('import.import_backend_empty_cold.plan_import_ms', 'import.import_backend_empty_cold.commit_import_ms'),
  'import rescan into library ms': sum('import.import_backend_into_library.plan_import_ms', 'import.import_backend_into_library.commit_import_ms'),
};
const bytes = at('startup.startup_backend_warm.json_bytes');
if (typeof bytes === 'number') derived['load_state payload MB'] = bytes / 1e6;
const tpm = at('analysis.analysis_throughput.tracks_per_min');
if (typeof tpm === 'number' && tpm > 0) derived['analysis hours for 10k tracks (extrapolated)'] = 10_000 / tpm / 60;
report.paths.derived = { all: Object.fromEntries(Object.entries(derived).filter(([, v]) => v !== undefined)) };

const here = dirname(fileURLToPath(import.meta.url));
const budgets = size && existsSync(join(here, 'perf-budgets.json')) ? JSON.parse(readFileSync(join(here, 'perf-budgets.json'), 'utf8')) : null;
if (budgets) {
  report.budgets = budgets.metrics.map((m) => {
    const value = at(m.path);
    const now = m.now?.[size] ?? null;
    const target = m.target?.[size] ?? null;
    const lower = m.better !== 'higher';
    const within = (limit, slack) => (lower ? value <= limit * (1 + slack) : value >= limit * (1 - slack));
    const budget = typeof value !== 'number' ? 'missing' : now === null ? 'unset' : within(now, budgets.tolerance) ? 'pass' : 'FAIL';
    const goal = typeof value !== 'number' || target === null ? '-' : within(target, 0) ? 'met' : 'open';
    const offBy = typeof value === 'number' && target ? (lower ? value / target : target / value) : null;
    return { id: m.id, label: m.label, unit: m.unit, value, now, target, budget, goal, off_by: offBy };
  });
}

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

if (report.budgets) {
  const cell = (v) => (typeof v === 'number' ? fmt(v) : v === null || v === undefined ? '-' : String(v));
  console.log(`\nBudgets for ${size} (budget = "now" + ${Math.round(budgets.tolerance * 100)}% noise; target = where we want to be)\n`);
  console.log(`  ${'metric'.padEnd(52)} ${'unit'.padEnd(8)} ${'measured'.padStart(10)} ${'now'.padStart(10)} ${'target'.padStart(10)}  budget  target  x off`);
  for (const b of report.budgets) {
    console.log(`  ${b.label.padEnd(52)} ${b.unit.padEnd(8)} ${cell(b.value).padStart(10)} ${cell(b.now).padStart(10)} ${cell(b.target).padStart(10)}  ${b.budget.padEnd(6)}  ${b.goal.padEnd(6)}  ${b.off_by ? b.off_by.toFixed(1) : '-'}`);
  }
  const worst = report.budgets.filter((b) => b.off_by && b.goal === 'open').sort((a, b) => b.off_by - a.off_by).slice(0, 5);
  console.log(`\n  worst against target: ${worst.map((b) => `${b.label} (${b.off_by.toFixed(1)}x)`).join('; ') || 'none'}`);
  const failed = report.budgets.filter((b) => b.budget === 'FAIL').length;
  console.log(`  budget: ${failed ? `${failed} FAIL` : 'all pass'} (${report.budgets.filter((b) => b.budget === 'pass').length} pass, ${report.budgets.filter((b) => b.budget === 'unset').length} unset, ${report.budgets.filter((b) => b.budget === 'missing').length} missing)`);
}
