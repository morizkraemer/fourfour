(() => {
  if (window.top !== window || window.__ffProbeStarted) return;
  window.__ffProbeStarted = true;
  const cfg = window.__FF_PROBE__ || {};
  const send = (name, values = {}) => {
    try {
      window.__TAURI_INTERNALS__.invoke('perf_mark', { name, values: { page_ms: performance.now(), ...values } });
    } catch (e) {
      setTimeout(() => send(name, values), 50);
    }
  };
  const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const pct = (xs, p) => (xs.length ? [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(xs.length * p))] : 0);
  const frameStats = (gaps) => ({
    frames: gaps.length,
    frame_p50_ms: pct(gaps, 0.5),
    frame_p95_ms: pct(gaps, 0.95),
    frame_max_ms: gaps.length ? Math.max(...gaps) : 0,
    frames_dropped: gaps.reduce((n, g) => n + Math.max(0, Math.round(g / (1000 / 60)) - 1), 0),
    long_frames: gaps.filter((g) => g > 50).length,
    long_frame_ms_total: gaps.filter((g) => g > 50).reduce((s, g) => s + g, 0),
  });

  async function waitFor(test, timeoutMs) {
    const t0 = performance.now();
    while (!test()) {
      if (performance.now() - t0 > timeoutMs) return false;
      await frame();
    }
    return true;
  }

  async function scrollGaps(el, ms, gaps) {
    const t0 = performance.now();
    let last = await frame();
    let direction = 1;
    while (performance.now() - t0 < ms) {
      const max = el.scrollHeight - el.clientHeight;
      if (el.scrollTop >= max - 1) direction = -1;
      if (el.scrollTop <= 0) direction = 1;
      el.scrollTop += direction * (cfg.scrollStep || 120);
      const now = await frame();
      gaps.push(now - last);
      last = now;
    }
  }

  async function scroll(seconds, label) {
    const el = document.querySelector('.ff-table__scroll');
    if (!el) return send(`${label}_skipped`, { reason: 'no .ff-table__scroll' });
    el.scrollTop = 0;
    const gaps = [];
    await scrollGaps(el, seconds * 1000, gaps);
    send(label, frameStats(gaps));
  }

  async function scrollBursts(count, scrollMs, restMs, label) {
    const el = document.querySelector('.ff-table__scroll');
    if (!el) return send(`${label}_skipped`, { reason: 'no .ff-table__scroll' });
    const gaps = [];
    for (let i = 0; i < count; i++) {
      await scrollGaps(el, scrollMs, gaps);
      await sleep(restMs);
    }
    send(label, frameStats(gaps));
  }

  async function loadTracks() {
    const rows = [...document.querySelectorAll('[data-track-row]')];
    const times = [];
    for (const index of [3, 5, 7, 9, 11]) {
      const row = rows[index];
      if (!row) break;
      performance.clearMarks('ff:player-ready');
      const t0 = performance.now();
      row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
      const ok = await waitFor(() => performance.getEntriesByName('ff:player-ready').length > 0, 15000);
      const mark = performance.getEntriesByName('ff:player-ready').at(-1);
      times.push(ok && mark ? mark.startTime - t0 : null);
      await sleep(500);
    }
    const done = times.filter((t) => t !== null);
    send('track_load', { samples: times.length, timed_out: times.length - done.length, player_ready_ms_median: pct(done, 0.5), player_ready_ms_max: done.length ? Math.max(...done) : 0 });
  }

  async function firstExpand() {
    const row = document.querySelectorAll('[data-track-row]')[3];
    if (!row) return send('first_expand_skipped', { reason: 'no row 3' });
    performance.clearMarks('ff:player-ready');
    row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
    if (!(await waitFor(() => performance.getEntriesByName('ff:player-ready').length > 0, 15000))) return send('first_expand_skipped', { reason: 'track load timed out' });
    await sleep(cfg.expandSettleMs ?? 1500);
    const button = document.querySelector('.ff-player-compact__expand-btn');
    if (!button) return send('first_expand_skipped', { reason: 'no expand button' });
    const gaps = [];
    let last = await frame();
    const t0 = performance.now();
    button.click();
    const zoomDetail = () => document.querySelector('.ff-xp .waveform-display__zoom')?.dataset.bands === 'detail';
    while (!zoomDetail() && performance.now() - t0 < 15000) {
      const now = await frame();
      gaps.push(now - last);
      last = now;
    }
    const drawnFrames = zoomDetail() ? 2 : 0;
    for (let i = 0; i < drawnFrames; i++) {
      const now = await frame();
      gaps.push(now - last);
      last = now;
    }
    const drawn = performance.now() - t0;
    for (let i = 0; i < 10; i++) {
      const now = await frame();
      gaps.push(now - last);
      last = now;
    }
    send('first_expand', { drawn_ms: drawnFrames ? drawn : null, longest_frame_ms: Math.max(...gaps), frames: gaps.length, long_frames: gaps.filter((g) => g > 50).length });
  }

  async function run() {
    send('page_start', { time_origin: performance.timeOrigin });
    const found = await waitFor(() => document.querySelector('[data-track-row]'), 120000);
    if (!found) return send('probe_done', { error: 'no track row within 120 s' });
    send('first_row');
    const gaps = [];
    let last = await frame();
    let streakStart = last;
    let streak = 0;
    let longest = 0;
    const t0 = last;
    while (streak < 10 && last - t0 < 30000) {
      const now = await frame();
      const gap = now - last;
      gaps.push(gap);
      longest = Math.max(longest, gap);
      if (gap > 50) {
        streak = 0;
        streakStart = now;
      } else {
        streak += 1;
      }
      last = now;
    }
    send('interactive', { at_page_ms: streakStart, longest_frame_ms: longest, frames_after_first_row: gaps.length });
    await sleep(cfg.settleMs ?? 2000);
    if (cfg.expand) await firstExpand();
    if (cfg.trackLoad !== false) await loadTracks();
    if (cfg.scroll !== false) {
      send('scroll_idle_start');
      await scroll(cfg.scrollSeconds ?? 5, 'scroll_idle');
    }
    if (cfg.analyse) {
      window.__TAURI_INTERNALS__.invoke('analyze_tracks').catch((e) => send('analysis_error', { error: String(e) }));
      await sleep(cfg.analysisWarmupMs ?? 8000);
      send('scroll_analysis_start');
      await scroll(cfg.analysisScrollSeconds ?? 10, 'scroll_during_analysis');
      if (cfg.analysisBursts) await scrollBursts(cfg.analysisBursts, 1500, 2500, 'scroll_bursts_during_analysis');
      await sleep(cfg.analysisRunMs ?? 0);
      await window.__TAURI_INTERNALS__.invoke('cancel_analysis').catch(() => {});
    }
    send('probe_done');
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run, { once: true });
  else run();
})();
