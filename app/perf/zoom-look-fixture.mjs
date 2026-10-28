import { readFileSync, writeFileSync } from 'node:fs';

/**
 * The real data of the zoom-look mockups (`prototyping/mockups`, page zoom-look): a stretch of a track's 2400 columns per
 * second series (`waveform_detail_dump` → NAME.ffwd) and a window of one column per sample around the same point, fetched
 * from `waveform_window_dump serve PORT AUDIO`. The window starts half a sample early, so every column is centred on its sample.
 *
 * node app/perf/zoom-look-fixture.mjs NAME.ffwd http://127.0.0.1:PORT CENTRE_S OUT.json
 */
const [ffwdPath, server, centreArg, out] = process.argv.slice(2);
const centreS = Number(centreArg);
const DETAIL_HALF_S = 3;
const WINDOW_HALF_S = 0.18;
const HEADER = 28;

const ffwd = readFileSync(ffwdPath);
const view = new DataView(ffwd.buffer, ffwd.byteOffset, ffwd.byteLength);
const columns = view.getUint32(12, true);
const perSecond = view.getUint32(8, true) || 2400;
const signalGain = view.getFloat32(24, true);
const first = Math.max(0, Math.floor((centreS - DETAIL_HALF_S) * perSecond));
const last = Math.min(columns, Math.ceil((centreS + DETAIL_HALF_S) * perSecond));
const plane = (k) => Buffer.from(ffwd.subarray(HEADER + k * columns + first, HEADER + k * columns + last)).toString('base64');

const response = await fetch(`${server}/window?probe=1&start=0&end=0.01&columns=441`);
const probe = Buffer.from(await response.arrayBuffer());
const rate = probe.readUInt32LE(12);
const firstSample = Math.round((centreS - WINDOW_HALF_S) * rate);
const samples = Math.round(2 * WINDOW_HALF_S * rate);
const win = Buffer.from(await (await fetch(
  `${server}/window?start=${(firstSample - 0.5) / rate}&end=${(firstSample + samples - 0.5) / rate}&columns=${samples}`,
)).arrayBuffer());
if (win.toString('latin1', 0, 4) !== 'FFWW' || win.readUInt32LE(8) !== samples) throw new Error('bad window');
const W = 32;
const slice = (from, bytes) => win.subarray(from, from + bytes).toString('base64');

writeFileSync(out, JSON.stringify({
  centreMs: centreS * 1000,
  detail: {
    startMs: (first / perSecond) * 1000,
    columnsPerSecond: perSecond,
    signalGain,
    low: plane(0), mid: plane(1), high: plane(2), signalLow: plane(3), signalHigh: plane(4),
  },
  window: {
    sampleRate: rate,
    firstSample,
    lowest: slice(W, 2 * samples),
    highest: slice(W + 2 * samples, 2 * samples),
    low: slice(W + 4 * samples, samples),
    mid: slice(W + 5 * samples, samples),
    high: slice(W + 6 * samples, samples),
  },
}));
console.log(`wrote ${out}: detail ${last - first} columns, window ${samples} samples at ${rate} Hz`);
