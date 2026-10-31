import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const [out, size = '10k'] = process.argv.slice(2);
const here = dirname(fileURLToPath(import.meta.url));
const budgets = JSON.parse(readFileSync(join(here, '..', 'perf-budgets.json'), 'utf8'));
const median = (xs) => (xs.length ? [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] : undefined);
const fmt = (v) => (typeof v === 'number' ? (Math.abs(v) >= 100 ? Math.round(v).toLocaleString('en-US') : String(Math.round(v * 10) / 10)) : '-');

function readRun(file) {
  const marks = readFileSync(join(out, file), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
  const by = (name) => marks.find((m) => m.mark === name);
  const firstRow = by('first_row');
  const interactive = by('interactive');
  const run = {
    file,
    setup_ms: by('setup_done')?.ms,
    load_state_ms: by('load_state_built') && by('load_state_start') ? by('load_state_built').ms - by('load_state_start').ms : undefined,
    first_row_ms: firstRow?.ms,
    interactive_ms: firstRow && interactive ? firstRow.ms + (interactive.values.at_page_ms - firstRow.values.page_ms) : undefined,
    longest_frame_after_first_row_ms: interactive?.values.longest_frame_ms,
    track_load_ms: by('track_load')?.values.player_ready_ms_median,
    first_expand: by('first_expand')?.values,
    first_expand_skipped: by('first_expand_skipped')?.values.reason,
    scroll_idle: by('scroll_idle')?.values,
    scroll_analysis: by('scroll_during_analysis')?.values,
    scroll_bursts: by('scroll_bursts_during_analysis')?.values,
    done: !!by('probe_done'),
    error: by('probe_done')?.values.error ?? by('analysis_error')?.values.error,
  };
  return run;
}

const files = readdirSync(out).filter((f) => /^app-run-.*\.jsonl$/.test(f)).sort();
const runs = files.map(readRun);
const cold = runs.find((r) => r.file === 'app-run-1.jsonl');
const warm = runs.filter((r) => /^app-run-\d+\.jsonl$/.test(r.file) && r.file !== 'app-run-1.jsonl');
const analysis = runs.find((r) => r.file === 'app-run-analysis.jsonl');

const mac = {
  startup_cold_first_row: cold?.first_row_ms,
  startup_cold_interactive: cold?.interactive_ms,
  startup_warm_first_row: median(warm.map((r) => r.first_row_ms).filter(Number.isFinite)),
  startup_warm_interactive: median(warm.map((r) => r.interactive_ms).filter(Number.isFinite)),
  scroll_analysis_frame_p95: analysis?.scroll_analysis?.frame_p95_ms,
  scroll_analysis_frames_dropped: analysis?.scroll_analysis?.frames_dropped,
  scroll_analysis_long_tasks: analysis?.scroll_analysis?.long_frame_ms_total,
};

const lines = [`# Mac perf night ${size} (${new Date().toISOString()})`, ''];
lines.push('## WKWebView (real app, HOME sandboxed, process start to mark)', '');
lines.push('| metric | measured | target | vs target |', '|---|---:|---:|---|');
const verdicts = [];
for (const m of budgets.metrics) {
  if (!(m.id in mac)) continue;
  const value = mac[m.id];
  const target = m.target?.[size];
  const lower = m.better !== 'higher';
  const verdict = typeof value !== 'number' ? 'missing' : target == null ? '-' : (lower ? value <= target : value >= target) ? 'met' : 'open';
  verdicts.push(verdict);
  lines.push(`| ${m.label} | ${fmt(value)} ${m.unit} | ${fmt(target)} | ${verdict} |`);
}
lines.push('', '| per run | setup | load_state | first row | interactive | longest frame | track load | idle scroll p95 / dropped |', '|---|---:|---:|---:|---:|---:|---:|---|');
for (const r of runs) {
  lines.push(`| ${r.file}${r.done ? '' : ' (no probe_done)'}${r.error ? ` (${r.error})` : ''} | ${fmt(r.setup_ms)} | ${fmt(r.load_state_ms)} | ${fmt(r.first_row_ms)} | ${fmt(r.interactive_ms)} | ${fmt(r.longest_frame_after_first_row_ms)} | ${fmt(r.track_load_ms)} | ${fmt(r.scroll_idle?.frame_p95_ms)} / ${fmt(r.scroll_idle?.frames_dropped)} |`);
}
if (analysis?.scroll_analysis) {
  const s = analysis.scroll_analysis;
  lines.push('', `Scroll during analysis: ${s.frames} frames, p50 ${fmt(s.frame_p50_ms)} ms, p95 ${fmt(s.frame_p95_ms)} ms, max ${fmt(s.frame_max_ms)} ms, ${s.frames_dropped} dropped, ${s.long_frames} frames over 50 ms (${fmt(s.long_frame_ms_total)} ms). WKWebView has no long-task API; frames over 50 ms stand in for long tasks.`);
}

const expandRuns = runs.filter((r) => /^app-run-expand-\d+\.jsonl$/.test(r.file));
if (expandRuns.length) {
  const measured = expandRuns.map((r) => r.first_expand).filter(Boolean);
  const longest = median(measured.map((e) => e.longest_frame_ms).filter(Number.isFinite));
  const drawn = median(measured.map((e) => e.drawn_ms).filter(Number.isFinite));
  lines.push('', '## Cold first expand (R10)', '', `Median of ${measured.length} launches: longest frame ${fmt(longest)} ms, click to detail drawn ${fmt(drawn)} ms. Trigger for the staged reveal (perf-run-2026-11.md, R10 option 1): median longest frame over 50 ms (${typeof longest === 'number' ? (longest > 50 ? 'over: build it' : 'under: not needed') : 'missing'}).`, '');
  lines.push('| run | longest frame ms | click to drawn ms | frames > 50 ms |', '|---|---:|---:|---:|');
  for (const r of expandRuns) {
    const e = r.first_expand ?? {};
    lines.push(`| ${r.file}${r.first_expand_skipped ? ` (skipped: ${r.first_expand_skipped})` : ''} | ${fmt(e.longest_frame_ms)} | ${fmt(e.drawn_ms)} | ${fmt(e.long_frames)} |`);
  }
}

const headroomRuns = runs.filter((r) => r.file.startsWith('app-run-analysis'));
if (headroomRuns.length) {
  lines.push('', '## UI headroom during analysis (R5)', '', 'analysis-before: no UI pause, cores - 1 threads; analysis: R5 default (cores - 2, analysis held while scrolling); analysis-bgqos: R5 plus background QoS (efficiency cores only).', '');
  lines.push('| run | scroll p50 / p95 / max ms | dropped | frames > 50 ms | bursts p50 / p95 / max ms | bursts dropped |', '|---|---|---:|---:|---|---:|');
  for (const r of headroomRuns) {
    const s = r.scroll_analysis ?? {};
    const b = r.scroll_bursts ?? {};
    lines.push(`| ${r.file} | ${fmt(s.frame_p50_ms)} / ${fmt(s.frame_p95_ms)} / ${fmt(s.frame_max_ms)} | ${fmt(s.frames_dropped)} | ${fmt(s.long_frames)} | ${fmt(b.frame_p50_ms)} / ${fmt(b.frame_p95_ms)} / ${fmt(b.frame_max_ms)} | ${fmt(b.frames_dropped)} |`);
  }
}
const benchFiles = readdirSync(out).filter((f) => /^headroom-.*\.txt$/.test(f) && f !== 'headroom-list.txt').sort();
if (benchFiles.length) {
  lines.push('', '| bench_analysis (12 long files) | tracks/min | first result | CPU s | held for the UI |', '|---|---:|---:|---:|---|');
  for (const f of benchFiles) {
    const text = readFileSync(join(out, f), 'utf8');
    const field = (name) => text.match(new RegExp(`${name}=([0-9.]+)`))?.[1];
    const held = text.match(/held=(\d+x [0-9.]+s \(\d+% of the run\))/)?.[1] ?? '-';
    lines.push(`| ${f.replace(/^headroom-|\.txt$/g, '')} | ${field('tracks_per_min') ?? '-'} | ${field('first_result') ?? '-'} s | ${field('cpu') ?? '-'} | ${held} |`);
  }
}

const backendFile = join(out, 'backend.json');
if (existsSync(backendFile)) {
  const backend = JSON.parse(readFileSync(backendFile, 'utf8'));
  lines.push('', '## Backend suite on this Mac (scripts/perf.sh, frontend skipped)', '', '| metric | measured | box now | target | vs target |', '|---|---:|---:|---:|---|');
  for (const b of backend.budgets ?? []) {
    if (b.id in mac) continue;
    if (b.value === undefined || b.value === null) continue;
    verdicts.push(b.goal === '-' ? '-' : b.goal);
    lines.push(`| ${b.label} | ${fmt(b.value)} ${b.unit} | ${fmt(b.now)} | ${fmt(b.target)} | ${b.goal} |`);
  }
}

const met = verdicts.filter((v) => v === 'met').length;
const open = verdicts.filter((v) => v === 'open').length;
const missing = verdicts.filter((v) => v === 'missing').length;
lines.splice(2, 0, `**${missing ? 'INCOMPLETE' : open ? 'FAIL' : 'PASS'}** against targets: ${met} met, ${open} open, ${missing} missing. Box "now" budgets are dev-box numbers, so the Mac is judged on targets only.`, '');
console.log(lines.join('\n'));
