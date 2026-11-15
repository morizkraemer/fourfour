import { describe, expect, it } from 'vitest';
import { UI_BUSY_SIGNAL_EVERY_MS, watchUiBusy } from './ui-busy.ts';

function setup(analysing = true) {
  const target = new EventTarget();
  let t = 0;
  let signals = 0;
  const state = { analysing };
  const stop = watchUiBusy(target as unknown as Window, () => state.analysing, () => signals++, () => t);
  return {
    fire: (type: string, init: Record<string, unknown> = {}) => target.dispatchEvent(Object.assign(new Event(type), init)),
    advance: (ms: number) => (t += ms),
    signals: () => signals,
    state,
    stop,
  };
}

describe('watchUiBusy', () => {
  it('signals scrolling and wheeling at most once per interval while analysing', () => {
    const ui = setup();
    ui.fire('scroll');
    ui.fire('scroll');
    ui.fire('wheel');
    expect(ui.signals()).toBe(1);
    ui.advance(UI_BUSY_SIGNAL_EVERY_MS);
    ui.fire('wheel');
    expect(ui.signals()).toBe(2);
  });

  it('counts pointer moves only while a button is held (a drag)', () => {
    const ui = setup();
    ui.fire('pointermove', { buttons: 0 });
    expect(ui.signals()).toBe(0);
    ui.fire('pointermove', { buttons: 1 });
    expect(ui.signals()).toBe(1);
  });

  it('stays quiet when no analysis runs and after stop', () => {
    const ui = setup(false);
    ui.fire('scroll');
    expect(ui.signals()).toBe(0);
    ui.state.analysing = true;
    ui.stop();
    ui.fire('scroll');
    expect(ui.signals()).toBe(0);
  });
});
