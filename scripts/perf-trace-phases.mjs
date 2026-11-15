#!/usr/bin/env node
// Splits "expand to detail drawn" into phases from a Chromium trace written by
//   cd app && FF_PERF_Q50=1 FF_PERF_TRACKS=10000 FF_PERF_STATE=<state.json> FF_PERF_PROFILE=expand-phases \
//     FF_PERF_TRACE_OUT=trace.json FF_SMOKE_PORT=5361 npx playwright test perf-q50 --grep 'expand phases'
// Usage: node scripts/perf-trace-phases.mjs trace.json            one row per expand, ms
//        node scripts/perf-trace-phases.mjs trace.json <run>      adds the main-thread frames of that expand
// Marks in the trace: ex-click (the click), ex-detail (data-bands set on the zoom canvas), ex-drawn (what the
// "expand to detail drawn" metric reports). Busy time is main-thread exclusive time per event name, bucketed;
// wait = wall time with the main thread idle (vsync, rAF scheduling).
import { readFileSync } from 'node:fs';

const [file, runArg] = process.argv.slice(2);
if (!file) {
  console.error('usage: perf-trace-phases.mjs trace.json [run]');
  process.exit(2);
}
const trace = JSON.parse(readFileSync(file, 'utf8'));
const events = Array.isArray(trace) ? trace : trace.traceEvents;
const marks = events.filter((e) => e.cat?.includes('blink.user_timing') && /^ex-/.test(e.name));
const { pid, tid } = marks[0];
const at = (name) => marks.filter((m) => m.name === name).map((m) => m.ts);
const clicks = at('ex-click');
const details = at('ex-detail');
const draws = at('ex-drawn');
const main = events.filter((e) => e.pid === pid && e.tid === tid && e.ph === 'X').sort((a, b) => a.ts - b.ts || b.dur - a.dur);

const bucketOf = (name) => {
  if (/GC|MinorGC|MajorGC/.test(name)) return 'gc';
  if (/^(FunctionCall|EvaluateScript|EventDispatch|RunMicrotasks|FireAnimationFrame|TimerFire|V8\.|v8\.|ParseHTML|ResourceFetch)/.test(name)) return 'js';
  if (/^(UpdateLayoutTree|Layout|PrePaint|IntersectionObserver|HitTest)/.test(name)) return 'style+layout';
  if (name === 'Paint') return 'paint';
  if (name === 'Layerize') return 'layerize';
  if (/^(Commit|LayerTreeHost|UpdateLayer)/.test(name)) return 'commit';
  return 'other';
};
const ms = (us) => (us / 1000).toFixed(1);
const order = ['js', 'gc', 'style+layout', 'paint', 'layerize', 'commit', 'other'];

const rows = clicks.map((c, i) => {
  const end = draws[i];
  const detail = details.find((d) => d >= c);
  const inside = main.filter((e) => e.ts + e.dur > c && e.ts < end);
  const buckets = Object.fromEntries(order.map((b) => [b, 0]));
  const stack = [];
  let busy = 0;
  for (const e of inside) {
    while (stack.length && stack.at(-1).ts + stack.at(-1).dur <= e.ts) stack.pop();
    const span = Math.max(0, Math.min(e.ts + e.dur, end) - Math.max(e.ts, c));
    if (!stack.length && e.name === 'RunTask') busy += span;
    if (stack.length) buckets[bucketOf(stack.at(-1).name)] -= span;
    buckets[bucketOf(e.name)] += span;
    stack.push(e);
  }
  const frames = inside.filter((e) => e.name === 'ProxyMain::BeginMainFrame');
  return { i, c, wall: end - c, detail: detail - c, busy, buckets, frames, inside };
});

console.log(['run', 'wall', 'painted', 'busy', 'wait', ...order, 'frames'].join('\t'));
for (const r of rows) {
  const bucketsStr = order.map((b) => ms(Math.max(0, r.buckets[b])));
  console.log([r.i, ms(r.wall), ms(r.detail), ms(r.busy), ms(r.wall - r.busy), ...bucketsStr, r.frames.length].join('\t'));
}
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
console.log(['median', ms(median(rows.map((r) => r.wall))), ms(median(rows.map((r) => r.detail))), ms(median(rows.map((r) => r.busy))), ms(median(rows.map((r) => r.wall - r.busy))), ...order.map((b) => ms(median(rows.map((r) => Math.max(0, r.buckets[b]))))), median(rows.map((r) => r.frames.length))].join('\t'));

if (runArg !== undefined) {
  const r = rows[Number(runArg)];
  for (const f of r.frames) {
    const parts = r.inside.filter((e) => e.ts >= f.ts && e.ts + e.dur <= f.ts + f.dur && ['UpdateLayoutTree', 'Layout', 'FireAnimationFrame', 'Paint', 'Layerize', 'Commit'].includes(e.name));
    console.log(`frame at +${ms(f.ts - r.c)} lasts ${ms(f.dur)}: ${parts.map((p) => `${p.name} ${ms(p.dur)}`).join(', ')}`);
  }
}
