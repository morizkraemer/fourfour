import { describe, expect, it } from 'vitest';
import {
  AHEAD_GAIN,
  HOLD_MS,
  PlayheadClock,
  SNAP_MS,
} from '../../../prototyping/viewport/design-system/playhead-clock.js';

const REPORT_INTERVAL_MS = 50;
const FRAME_MS = 1000 / 60;

function seededRandom(seed: number) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) % 4294967296;
    return state / 4294967296;
  };
}

/** Reports of a transport playing at 1x from `startMs`: emitted every 50 ms, delivered after a jittery delay that is a multiple of the frame time (the page receives events between frames). */
function jitteredReports(seconds: number, maxDelayFrames: number, seed: number) {
  const random = seededRandom(seed);
  const reports: Array<{ arrivesAt: number; positionMs: number }> = [];
  for (let emitted = 0; emitted <= seconds * 1000; emitted += REPORT_INTERVAL_MS) {
    const delay = Math.ceil(random() * maxDelayFrames) * FRAME_MS;
    reports.push({ arrivesAt: emitted + delay, positionMs: emitted });
  }
  return reports.sort((a, b) => a.arrivesAt - b.arrivesAt);
}

function frameSpeeds(positionAtFrame: (now: number) => number, reports: ReturnType<typeof jitteredReports>, onReport: (r: { arrivesAt: number; positionMs: number }) => void, seconds: number) {
  const speeds: number[] = [];
  let next = 0;
  let previous: number | null = null;
  for (let now = 0; now <= seconds * 1000; now += FRAME_MS) {
    while (next < reports.length && reports[next].arrivesAt <= now) onReport(reports[next++]);
    const position = positionAtFrame(now);
    if (previous !== null && now > 2000) speeds.push((position - previous) / FRAME_MS);
    previous = position;
  }
  return speeds;
}

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
const spread = (xs: number[]) => Math.sqrt(mean(xs.map((x) => (x - mean(xs)) ** 2))) / mean(xs);

/** The playhead before the clock: render 80 ms behind and interpolate between the two buffered reports around that time. */
function bracketInterpolation() {
  const samples: Array<{ f: number; t: number }> = [];
  return {
    report(r: { arrivesAt: number; positionMs: number }) {
      samples.push({ f: r.positionMs, t: r.arrivesAt });
      if (samples.length > 8) samples.shift();
    },
    positionAt(now: number) {
      if (samples.length < 2) return samples[0]?.f ?? 0;
      const renderAt = now - 80;
      let i = samples.length - 2;
      for (let k = 0; k < samples.length - 1; k++) {
        if (samples[k + 1].t > renderAt) {
          i = k;
          break;
        }
      }
      const a = samples[i];
      const b = samples[i + 1];
      const span = b.t - a.t;
      const u = span > 0 ? Math.max(0, Math.min(2, (renderAt - a.t) / span)) : 1;
      return a.f + (b.f - a.f) * u;
    },
  };
}

describe('PlayheadClock', () => {
  it('runs at the playback speed between reports', () => {
    const clock = new PlayheadClock();
    clock.start(1000, 0);
    expect(clock.positionAt(0)).toBe(1000);
    expect(clock.positionAt(100)).toBeCloseTo(1100, 6);
    expect(clock.positionAt(400)).toBeCloseTo(1400, 6);
  });

  it('stands still when stopped and when it never ran', () => {
    const clock = new PlayheadClock();
    expect(clock.positionAt(500)).toBe(0);
    clock.start(2000, 0);
    clock.stop(2300);
    expect(clock.positionAt(900)).toBe(2300);
    expect(clock.running).toBe(false);
  });

  it('starts at the first report when it was not running', () => {
    const clock = new PlayheadClock();
    clock.report(5000, 100);
    expect(clock.running).toBe(true);
    expect(clock.positionAt(150)).toBeCloseTo(5050, 6);
  });

  it('jumps to the reported position for a seek', () => {
    const clock = new PlayheadClock();
    clock.start(1000, 0);
    clock.report(60_000, 100);
    expect(clock.positionAt(100)).toBe(60_000);
    clock.report(1000, 200);
    expect(clock.positionAt(200)).toBe(1000);
    expect(SNAP_MS).toBeLessThan(1000);
  });

  it('does not jump for a small difference: it speeds up or slows down and converges', () => {
    const clock = new PlayheadClock();
    clock.start(0, 0);
    clock.report(500, 500);
    clock.report(1000 + 30, 1000);
    expect(clock.positionAt(1000)).toBeCloseTo(1000, 6);
    let previous = clock.positionAt(1000);
    for (let now = 1000 + FRAME_MS; now <= 1600; now += FRAME_MS) {
      const position = clock.positionAt(now);
      expect(position - previous).toBeGreaterThan(0);
      expect(position - previous).toBeLessThan(FRAME_MS * 1.1);
      previous = position;
    }
    expect(clock.positionAt(1600)).toBeGreaterThan(1600 + 30 * AHEAD_GAIN * 0.95);
  });

  it('corrects a report that is behind much more slowly than one that is ahead', () => {
    const behind = new PlayheadClock();
    behind.start(0, 0);
    behind.report(500, 500);
    behind.report(1000 - 30, 1000);
    const ahead = new PlayheadClock();
    ahead.start(0, 0);
    ahead.report(500, 500);
    ahead.report(1000 + 30, 1000);
    const lag = 1000 + 600 - behind.positionAt(1600);
    const lead = ahead.positionAt(1600) - (1000 + 600);
    expect(lag).toBeLessThan(lead);
  });

  it('stops advancing when the reports stop', () => {
    const clock = new PlayheadClock();
    clock.start(0, 0);
    clock.report(0, 0);
    const held = clock.positionAt(HOLD_MS);
    expect(clock.positionAt(HOLD_MS + 5000)).toBe(held);
  });

  it('never moves backwards while playing, whatever the delivery jitter', () => {
    const seconds = 20;
    const reports = jitteredReports(seconds, 4, 7);
    const clock = new PlayheadClock();
    clock.start(0, 0);
    const speeds = frameSpeeds((now) => clock.positionAt(now), reports, (r) => clock.report(r.positionMs, r.arrivesAt), seconds);
    expect(Math.min(...speeds)).toBeGreaterThan(0.8);
    expect(Math.max(...speeds)).toBeLessThan(1.2);
  });

  it('scrolls at a steady speed where interpolating between reports does not', () => {
    const seconds = 20;
    const reports = jitteredReports(seconds, 4, 3);

    const clock = new PlayheadClock();
    clock.start(0, 0);
    const clockSpeeds = frameSpeeds((now) => clock.positionAt(now), reports, (r) => clock.report(r.positionMs, r.arrivesAt), seconds);

    const old = bracketInterpolation();
    const oldSpeeds = frameSpeeds((now) => old.positionAt(now), reports, (r) => old.report(r), seconds);

    expect(mean(clockSpeeds)).toBeCloseTo(1, 1);
    expect(spread(clockSpeeds)).toBeLessThan(0.05);
    expect(spread(oldSpeeds)).toBeGreaterThan(spread(clockSpeeds) * 5);
  });

  it('stays within the delivery delay of the true position', () => {
    const seconds = 20;
    const reports = jitteredReports(seconds, 4, 11);
    const clock = new PlayheadClock();
    clock.start(0, 0);
    const errors: number[] = [];
    let next = 0;
    for (let now = 0; now <= seconds * 1000; now += FRAME_MS) {
      while (next < reports.length && reports[next].arrivesAt <= now) {
        clock.report(reports[next].positionMs, reports[next].arrivesAt);
        next++;
      }
      if (now > 3000) errors.push(clock.positionAt(now) - now);
    }
    expect(Math.max(...errors.map(Math.abs))).toBeLessThan(4 * FRAME_MS);
    expect(Math.abs(mean(errors))).toBeLessThan(2 * FRAME_MS);
  });
});

describe('PlayheadClock at a tempo', () => {
  it('advances by the ratio per millisecond', () => {
    const clock = new PlayheadClock();
    clock.report(1000, 0, 1.08);
    expect(clock.positionAt(500)).toBeCloseTo(1000 + 500 * 1.08, 6);
  });

  it('takes the new rate from the next report without rewriting the past', () => {
    const clock = new PlayheadClock();
    clock.report(0, 0, 1);
    clock.report(50, 50, 1);
    const before = clock.positionAt(100);
    clock.report(before, 100, 1.16);
    expect(clock.positionAt(200)).toBeCloseTo(before + 100 * 1.16, 6);
  });

  it('runs at 1x without a rate', () => {
    const clock = new PlayheadClock();
    clock.report(0, 0);
    expect(clock.positionAt(250)).toBeCloseTo(250, 6);
  });
});
