import { spawnSync } from 'node:child_process';

/**
 * Median of several `frame-pacing.mjs` runs per variant, side by side.
 *
 *   PW_DIR=<dir with playwright> node perf/frame-pacing-compare.mjs <base url> <runs> <jitterMs> <name=overrideDir|name=-> ...
 *
 * A variant is a folder with older copies of design-system files (`waveform-display.js`, `waveform-render.js`, ...) that
 * replace the served ones; `-` runs the working tree as it is.
 */
const [base, runsArg, jitter, ...variants] = process.argv.slice(2);
const runs = Number(runsArg);
const KEYS = ['framesPerSecond', 'scrollStepsPerSecond', 'stalledFramePercent', 'deltaStdPx', 'speedCoefficientOfVariation', 'speedP5', 'speedP95', 'frameMsP95'];
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

const rows = [];
for (const variant of variants) {
  const [name, dir] = variant.split('=');
  const results = [];
  for (let i = 0; i < runs; i++) {
    const env = { ...process.env };
    if (dir !== '-') env.OVERRIDE_DIR = dir;
    const run = spawnSync('node', ['perf/frame-pacing.mjs', base, '8', '2', '0', jitter], { env, encoding: 'utf8', timeout: 180000 });
    const line = run.stdout.trim().split('\n').pop();
    try {
      results.push(JSON.parse(line));
    } catch {
      console.error(`${name} run ${i} failed: ${run.stdout.slice(-200)} ${run.stderr.slice(-200)}`);
    }
  }
  const backward = results.map((r) => Object.entries(r.deltaHistogram).filter(([d]) => Number(d) < 0).reduce((n, [, c]) => n + c, 0));
  rows.push({ name, ...Object.fromEntries(KEYS.map((k) => [k, median(results.map((r) => r[k]))])), backwardFrames: median(backward) });
}
console.table(rows);
console.log(JSON.stringify(rows));
