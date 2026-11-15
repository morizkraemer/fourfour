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
    const busy = mainThreadBusy();
    let last = await frame();
    const t0 = performance.now();
    button.click();
    const zoomBands = () => document.querySelector('.ff-xp .waveform-display__zoom')?.dataset.bands;
    const zoomDetail = () => zoomBands() === 'detail';
    let revealedAt = null;
    let pendingFrames = 0;
    let longestAt = 0;
    while (!zoomDetail() && performance.now() - t0 < 15000) {
      const now = await frame();
      if (now - last > Math.max(0, ...gaps)) longestAt = now - t0;
      gaps.push(now - last);
      last = now;
      if (revealedAt === null && document.querySelector('.ff-xp')) revealedAt = now - t0;
      if (zoomBands() === 'pending') pendingFrames += 1;
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
    send('first_expand', {
      drawn_ms: drawnFrames ? drawn : null,
      longest_frame_ms: Math.max(...gaps),
      longest_frame_at_ms: longestAt,
      longest_busy_ms: busy.stop(),
      revealed_ms: revealedAt,
      pending_frames: pendingFrames,
      frames: gaps.length,
      long_frames: gaps.filter((g) => g > 50).length,
    });
  }

  function mainThreadBusy() {
    const channel = new MessageChannel();
    let last = performance.now();
    let longest = 0;
    let running = true;
    channel.port1.onmessage = () => {
      const now = performance.now();
      longest = Math.max(longest, now - last);
      last = now;
      if (running) setTimeout(() => channel.port2.postMessage(0), 1);
    };
    channel.port2.postMessage(0);
    return {
      stop() {
        running = false;
        channel.port1.close();
        return longest;
      },
    };
  }

  const setInputValue = (input, value) => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  };

  const pressKey = (key, code, extra = {}) => {
    const target = document.activeElement || document.body;
    target.dispatchEvent(new KeyboardEvent('keydown', { key, code, bubbles: true, cancelable: true, ...extra }));
  };

  const soakActions = {
    async fling() {
      const el = document.querySelector('.ff-table__scroll');
      if (!el) throw new Error('no .ff-table__scroll');
      for (let n = 0; n < 24; n++) {
        el.scrollTop = n % 2 === 0 ? el.scrollTop + 9000 : el.scrollTop - 4000;
        await frame();
      }
    },

    async search(round) {
      const input = document.querySelector('.ff-gheader__search input');
      if (!input) throw new Error('no search input');
      input.focus();
      const query = ['smoke track 1', 'solomun', 'melodic', 'bicep 12'][round % 4];
      for (let i = 1; i <= query.length; i++) {
        setInputValue(input, query.slice(0, i));
        await sleep(30);
      }
      await sleep(400);
      setInputValue(input, '');
      pressKey('Escape', 'Escape');
      await sleep(200);
    },

    async palette() {
      pressKey('k', 'KeyK', { ctrlKey: true });
      if (!(await waitFor(() => document.querySelector('.ff-cp'), 3000))) throw new Error('palette did not open');
      const input = document.activeElement;
      if (input instanceof HTMLInputElement) setInputValue(input, 'set');
      await sleep(250);
      pressKey('Escape', 'Escape');
      const search = document.querySelector('.ff-gheader__search input');
      if (search) setInputValue(search, '');
      pressKey('Escape', 'Escape');
      await waitFor(() => !document.querySelector('.ff-cp'), 3000);
    },

    async playlists() {
      const rows = [...document.querySelectorAll('[data-drop-track-playlist-id]')].slice(0, 4);
      for (const row of rows) {
        row.click();
        await sleep(600);
      }
      const all = [...document.querySelectorAll('.ff-sbrow__label')].find((el) => el.textContent.trim() === 'All Tracks');
      if (all) all.click();
      await sleep(400);
    },

    async loadAndSeek(round) {
      const scroll = document.querySelector('.ff-table__scroll');
      if (scroll) scroll.scrollTop = 0;
      await sleep(300);
      const rows = [...document.querySelectorAll('[data-track-row]')];
      const row = rows[(round % 10) + 2];
      if (!row) throw new Error('no track row');
      performance.clearMarks('ff:player-ready');
      row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
      if (!(await waitFor(() => performance.getEntriesByName('ff:player-ready').length > 0, 30000))) throw new Error('track did not become ready');
      const expand = document.querySelector('.ff-player-compact__expand-btn');
      if (!expand) throw new Error('no expand button');
      expand.click();
      if (!(await waitFor(() => document.querySelector('.ff-xp .waveform-display__zoom')?.dataset.bands === 'detail', 30000))) {
        pressKey('Escape', 'Escape');
        throw new Error('zoom detail did not arrive');
      }
      const overview = document.querySelector('.ff-xp .waveform-display__overview');
      if (overview) {
        const box = overview.getBoundingClientRect();
        for (const fraction of [0.2, 0.55, 0.8, 0.35]) {
          overview.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, clientX: box.left + box.width * fraction, clientY: box.top + box.height / 2 }));
          await sleep(150);
        }
      }
      pressKey('Escape', 'Escape');
      await waitFor(() => !document.querySelector('.ff-xp'), 5000);
    },

    async settings() {
      pressKey(',', 'Comma', { ctrlKey: true });
      if (!(await waitFor(() => document.querySelector('.ff-settings-sheet__nav-item'), 3000))) throw new Error('settings did not open');
      for (const item of document.querySelectorAll('.ff-settings-sheet__nav-item')) {
        item.click();
        await sleep(120);
      }
      pressKey('Escape', 'Escape');
      await waitFor(() => !document.querySelector('.ff-settings-sheet__nav-item'), 3000);
    },

    async analysis(round) {
      const pool = cfg.soakAnalysisIds || [];
      if (!pool.length) return;
      const ids = [0, 1].map((i) => pool[(round * 2 + i) % pool.length]);
      const run = window.__TAURI_INTERNALS__.invoke('analyze_track_ids', { ids });
      const outcome = await Promise.race([run.then(() => 'done'), sleep(180000).then(() => 'timeout')]);
      if (outcome === 'timeout') {
        await window.__TAURI_INTERNALS__.invoke('cancel_analysis').catch(() => {});
        throw new Error('analysis timed out');
      }
    },
  };

  async function soak() {
    const names = cfg.soakActions || Object.keys(soakActions);
    const started = performance.now();
    const failures = {};
    let cycle = 0;
    const takeSample = () =>
      send('soak_sample', {
        minute: (performance.now() - started) / 60000,
        cycle,
        js_heap_mb: performance.memory ? performance.memory.usedJSHeapSize / 1e6 : null,
        dom_nodes: document.getElementsByTagName('*').length,
        failures: { ...failures },
      });
    send('soak_start', { actions: names, minutes: cfg.soakMinutes, js_heap_api: Boolean(performance.memory) });
    const sampler = setInterval(takeSample, (cfg.soakSampleSecs ?? 60) * 1000);
    while (performance.now() - started < cfg.soakMinutes * 60000) {
      const cycleStart = performance.now();
      for (const name of names) {
        try {
          await soakActions[name](cycle);
        } catch (error) {
          failures[name] = (failures[name] || 0) + 1;
          send('soak_failure', { action: name, cycle, error: String(error) });
          pressKey('Escape', 'Escape');
        }
      }
      cycle += 1;
      send('soak_cycle', { cycle, cycle_ms: performance.now() - cycleStart });
      await sleep(cfg.soakPauseMs ?? 0);
    }
    clearInterval(sampler);
    takeSample();
    send('soak_done', { cycles: cycle, failures });
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
    const longFrames = [];
    const t0 = last;
    while (streak < 10 && last - t0 < 30000) {
      const now = await frame();
      const gap = now - last;
      gaps.push(gap);
      longest = Math.max(longest, gap);
      if (gap > 50) {
        longFrames.push([Math.round(last), Math.round(gap)]);
        streak = 0;
        streakStart = now;
      } else {
        streak += 1;
      }
      last = now;
    }
    send('interactive', { at_page_ms: streakStart, longest_frame_ms: longest, frames_after_first_row: gaps.length });
    send('boot_timeline', {
      long_frames: longFrames,
      marks: performance.getEntriesByType('mark').map((m) => [m.name, Math.round(m.startTime)]),
      resources: performance
        .getEntriesByType('resource')
        .map((r) => [r.name.split('/').pop(), Math.round(r.startTime), Math.round(r.responseEnd)]),
    });
    await sleep(cfg.settleMs ?? 2000);
    if (cfg.soakMinutes) {
      await soak();
      return send('probe_done');
    }
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
