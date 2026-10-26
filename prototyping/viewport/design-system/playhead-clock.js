/*
 * Playback clock for the scrolling waveform. The transport reports its position about 20 times a second, each report
 * delayed by however long the event took to reach the page (a busy page delivers it late, and in bursts). Scrolling
 * straight from those reports steps, and interpolating between two of them follows their jitter: the speed changes
 * with every report.
 *
 * The clock runs at the true playback speed (1 ms per ms) on the page's own timer, so any frame can ask for the
 * position and gets one that advances smoothly. Each report only corrects it: a report ahead of the clock means the
 * clock runs late and is corrected quickly, one behind it was probably delivered late and is corrected slowly. A
 * correction is spread over SLEW_TIME_MS, never applied as a jump; a report far from the clock is a seek and jumps.
 */

export const SNAP_MS = 250;
export const AHEAD_GAIN = 0.35;
export const BEHIND_GAIN = 0.03;
export const SLEW_TIME_MS = 150;
export const HOLD_MS = 700;

export class PlayheadClock {
  constructor() {
    this._running = false;
    this._positionMs = 0;
    this._at = 0;
    this._pendingMs = 0;
    this._lastReportAt = 0;
  }

  get running() {
    return this._running;
  }

  /** Run from `positionMs`, as of `nowMs`. */
  start(positionMs, nowMs) {
    this._running = true;
    this._positionMs = positionMs;
    this._at = nowMs;
    this._pendingMs = 0;
    this._lastReportAt = nowMs;
  }

  /** Stand still at `positionMs`. */
  stop(positionMs) {
    this._running = false;
    this._positionMs = positionMs;
    this._pendingMs = 0;
  }

  /** The transport reported `positionMs` at `nowMs` while playing. */
  report(positionMs, nowMs) {
    if (!this._running) {
      this.start(positionMs, nowMs);
      return;
    }
    this._advance(nowMs);
    const error = positionMs - (this._positionMs + this._pendingMs);
    if (Math.abs(error) > SNAP_MS) {
      this._positionMs = positionMs;
      this._pendingMs = 0;
    } else {
      this._pendingMs += error * (error > 0 ? AHEAD_GAIN : BEHIND_GAIN);
    }
    this._lastReportAt = nowMs;
  }

  /** The position at `nowMs`. Calls must not go back in time. */
  positionAt(nowMs) {
    if (this._running) this._advance(nowMs);
    return this._positionMs;
  }

  _advance(nowMs) {
    const until = Math.min(nowMs, this._lastReportAt + HOLD_MS);
    const elapsed = until - this._at;
    if (elapsed > 0) {
      const slewed = this._pendingMs * (1 - Math.exp(-elapsed / SLEW_TIME_MS));
      this._positionMs += elapsed + slewed;
      this._pendingMs -= slewed;
    }
    this._at = Math.max(this._at, nowMs);
  }
}
