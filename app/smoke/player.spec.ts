import type { Page } from '@playwright/test';
import { expect, loadByDoubleClick, test } from './app.ts';

const TRACK = 'Smoke Track 03';
const BEAT_MS = 60000 / 126;
const HOT_CUE_A_MS = 97000;
const SHOTS = process.env.SHOTS_DIR;

const distanceToBeat = (ms: number) => Math.abs(ms - Math.round(ms / BEAT_MS) * BEAT_MS);
const nearestBeat = (ms: number) => Math.round(Math.round(ms / BEAT_MS) * BEAT_MS);

const quantizeButton = (page: Page) => page.locator('.ff-xp, .ff-player-compact').getByRole('button', { name: 'Q', exact: true }).first();
const masterTempoButton = (page: Page) => page.locator('.ff-xp, .ff-player-compact').getByRole('button', { name: 'MT', exact: true }).first();

async function expandWithTrack(page: Page) {
  await loadByDoubleClick(page, TRACK);
  await page.locator('.ff-player-compact__expand-btn').click();
  await expect(page.locator('.ff-xp')).toBeVisible();
}

async function quantizeOff(page: Page) {
  const q = quantizeButton(page);
  await expect(q).toHaveAttribute('aria-pressed', 'true');
  await q.click();
  await expect(q).toHaveAttribute('aria-pressed', 'false');
}

async function parkOnHotCueA(page: Page) {
  await page.locator('.ff-xp__pad[data-pad="A"]').click();
  await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Pause');
  await page.keyboard.press('Space');
  await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Play');
}

async function cueATime(smoke: { invokes(cmd: string): Promise<Record<string, any>[]> }, previousSaves: number) {
  await expect.poll(async () => (await smoke.invokes('set_cues')).length).toBeGreaterThan(previousSaves);
  const saves = await smoke.invokes('set_cues');
  return saves[saves.length - 1].cues.find((c: { hot_cue_number: number }) => c.hot_cue_number === 1).time_ms as number;
}

async function cueAFlag(page: Page) {
  let previous = await readCueAFlag(page);
  for (let attempt = 0; attempt < 20; attempt++) {
    await page.waitForTimeout(100);
    const current = await readCueAFlag(page);
    if (current && previous && current.x === previous.x) {
      previous = current;
      break;
    }
    previous = current;
  }
  expect(previous, 'hot cue A flag on the zoom view').not.toBeNull();
  return { x: previous!.x + 6, y: previous!.y };
}

async function readCueAFlag(page: Page) {
  return page.evaluate(() => {
    const canvas = document.querySelector('.ff-xp .waveform-display__zoom') as HTMLCanvasElement;
    const rect = canvas.getBoundingClientRect();
    const scale = canvas.width / rect.width;
    const copy = document.createElement('canvas');
    copy.width = canvas.width;
    copy.height = 12 * Math.ceil(scale);
    const reader = copy.getContext('2d', { willReadFrequently: true })!;
    reader.drawImage(canvas, 0, 0);
    const row = reader.getImageData(0, Math.round(6 * scale), canvas.width, 1).data;
    for (let x = 0; x < canvas.width; x++) {
      if (row[x * 4] === 31 && row[x * 4 + 1] === 211 && row[x * 4 + 2] === 90) return { x: rect.left + x / scale, y: rect.top + 6 };
    }
    return null;
  });
}

async function dragCueA(page: Page, dx: number, alt: boolean) {
  const flag = await cueAFlag(page);
  await page.mouse.move(flag.x, flag.y);
  await page.mouse.down();
  if (alt) await page.keyboard.down('Alt');
  await page.mouse.move(flag.x + dx / 2, flag.y, { steps: 4 });
  await page.mouse.move(flag.x + dx, flag.y, { steps: 4 });
  await page.mouse.up();
  if (alt) await page.keyboard.up('Alt');
}

test('with nothing loaded the player keeps its controls, greyed out, and nothing moves when a track loads', async ({ smoke: { page } }) => {
  const compact = page.locator('.ff-player-compact');
  await expect(compact.getByRole('button', { name: 'Play', exact: true })).toBeDisabled();
  await expect(compact.locator('.ff-player-compact__cue-btn')).toBeDisabled();
  await expect(compact.getByRole('button', { name: 'MT', exact: true })).toBeDisabled();
  await expect(compact.getByRole('button', { name: 'Q', exact: true })).toBeDisabled();
  await expect(compact.locator('[role="group"][aria-label="BPM"]')).toBeVisible();
  await expect(compact.locator('[role="group"][aria-label="Key"]')).toBeVisible();
  const compactBefore = await compact.boundingBox();
  const tempoBefore = await compact.locator('.ff-ptempo').boundingBox();
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/empty-compact.png` });

  await page.locator('.ff-player-compact__expand-btn').click();
  const big = page.locator('.ff-xp');
  await expect(big).toBeVisible();
  for (const name of ['Cue', 'Play', 'Zoom in']) await expect(big.getByRole('button', { name, exact: true })).toBeDisabled();
  await expect(big.getByRole('button', { name: 'MT', exact: true })).toBeDisabled();
  await expect(big.getByRole('button', { name: 'Q', exact: true })).toBeDisabled();
  await expect(big.locator('[role="group"][aria-label="BPM"]')).toBeVisible();
  await expect(big.locator('[role="group"][aria-label="Key"]')).toBeVisible();
  const boxes = async () => ({
    player: await page.locator('.ff-player-module').boundingBox(),
    cue: await big.locator('.ff-xp__cue-btn').boundingBox(),
    pads: await big.locator('.ff-xp__pads').boundingBox(),
    mt: await big.getByRole('button', { name: 'MT', exact: true }).boundingBox(),
    q: await big.getByRole('button', { name: 'Q', exact: true }).boundingBox(),
    bpm: await big.locator('[role="group"][aria-label="BPM"]').boundingBox(),
    key: await big.locator('[role="group"][aria-label="Key"]').boundingBox(),
    times: await big.locator('.ff-xp__times').boundingBox(),
  });
  const emptyBig = await boxes();
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/empty-expanded.png` });

  await page.locator('[data-track-row]').filter({ hasText: TRACK }).first().dblclick();
  await expect(big.locator('.ff-xp__title')).toHaveText(TRACK);
  await expect(big.locator('.ff-xp__play')).toBeEnabled();
  expect(await boxes()).toEqual(emptyBig);
  if (SHOTS) await page.screenshot({ path: `${SHOTS}/loaded-expanded.png` });

  await page.keyboard.press('Escape');
  await expect(compact).toBeVisible();
  expect(compactBefore).not.toBeNull();
  expect(tempoBefore).not.toBeNull();
});

test('Q is a text toggle next to MT, stays readable on hover and press, and is remembered after a reload', async ({ smoke: { page } }) => {
  await loadByDoubleClick(page, TRACK);
  const q = quantizeButton(page);
  const mt = masterTempoButton(page);
  const colour = (button: typeof q) => button.evaluate((el) => getComputedStyle(el).color);
  const background = (button: typeof q) => button.evaluate((el) => getComputedStyle(el).backgroundColor);
  const blue = 'rgb(59, 130, 246)';

  await quantizeOff(page);
  await page.mouse.move(0, 0);
  await page.waitForTimeout(400);
  const offColour = await colour(q);
  expect(offColour).not.toBe(blue);
  await q.hover();
  expect(await colour(q)).toBe(offColour);
  await q.click();
  await expect(q).toHaveAttribute('aria-pressed', 'true');
  await page.mouse.move(0, 0);
  await expect.poll(() => colour(q)).toBe(blue);

  await q.hover();
  await expect.poll(() => background(q)).not.toBe('rgba(0, 0, 0, 0)');
  expect(await colour(q)).toBe(blue);
  await page.mouse.down();
  expect(await colour(q)).toBe(blue);
  await page.mouse.move(0, 0);
  await page.mouse.up();
  await expect(q).toHaveAttribute('aria-pressed', 'true');
  if (SHOTS) {
    const header = page.locator('.ff-player-compact__right-toggles');
    await page.mouse.move(0, 0);
    await expect.poll(() => colour(q)).toBe(blue);
    await header.screenshot({ path: `${SHOTS}/toggles-q-on-mt-on.png` });
    await q.hover();
    await header.screenshot({ path: `${SHOTS}/toggles-q-on-hover.png` });
    await mt.click();
    await page.mouse.move(0, 0);
    await header.screenshot({ path: `${SHOTS}/toggles-mt-off-q-on.png` });
    await mt.hover();
    await header.screenshot({ path: `${SHOTS}/toggles-mt-off-hover.png` });
    await mt.click();
  }

  await mt.hover();
  expect(await colour(mt)).toBe(blue);

  await page.reload();
  await page.locator('[data-track-row]').first().waitFor();
  await loadByDoubleClick(page, TRACK);
  await expect(quantizeButton(page)).toHaveAttribute('aria-pressed', 'true');
  await quantizeButton(page).click();
  await page.reload();
  await page.locator('[data-track-row]').first().waitFor();
  await loadByDoubleClick(page, TRACK);
  await expect(quantizeButton(page)).toHaveAttribute('aria-pressed', 'false');
});

test('a hot cue set at the playhead snaps to the grid only with Q on', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await quantizeOff(page);
  await parkOnHotCueA(page);
  await page.keyboard.press('Shift+C');
  await expect.poll(async () => (await smoke.invokes('set_cues')).length).toBeGreaterThan(0);
  const free = (await smoke.invokes('set_cues')).at(-1)!.cues.find((c: { hot_cue_number: number }) => c.hot_cue_number === 2).time_ms as number;
  expect(Math.abs(free - HOT_CUE_A_MS)).toBeLessThan(2000);
  expect(distanceToBeat(free)).toBeGreaterThan(1);

  await page.locator('.ff-xp__pad[data-pad="B"]').click({ modifiers: ['Alt'] });
  await quantizeButton(page).click();
  const saves = (await smoke.invokes('set_cues')).length;
  await page.keyboard.press('Shift+C');
  await expect.poll(async () => (await smoke.invokes('set_cues')).length).toBeGreaterThan(saves);
  const snapped = (await smoke.invokes('set_cues')).at(-1)!.cues.find((c: { hot_cue_number: number }) => c.hot_cue_number === 2).time_ms as number;
  expect(distanceToBeat(snapped)).toBeLessThan(1);
  expect(snapped).toBe(nearestBeat(free));
});

test('dragging a hot cue on the big waveform moves it, snapped to the grid with Q on; Alt places it freely', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await quantizeOff(page);
  await parkOnHotCueA(page);
  await quantizeButton(page).click();
  await expect(quantizeButton(page)).toHaveAttribute('aria-pressed', 'true');

  const saves = (await smoke.invokes('set_cues')).length;
  const flag = await cueAFlag(page);
  await page.mouse.click(flag.x, flag.y);
  await page.waitForTimeout(700);
  expect((await smoke.invokes('set_cues')).length).toBe(saves);

  await dragCueA(page, 61, false);
  const snapped = await cueATime(smoke, saves);
  expect(distanceToBeat(snapped)).toBeLessThan(1);
  expect(snapped).toBeGreaterThan(HOT_CUE_A_MS);

  const afterSnap = (await smoke.invokes('set_cues')).length;
  await dragCueA(page, 29, true);
  const free = await cueATime(smoke, afterSnap);
  expect(Math.abs(free - snapped)).toBeGreaterThan(20);
  expect(distanceToBeat(free)).toBeGreaterThan(8);
});

test('without Q a dragged hot cue lands where it is dropped', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await quantizeOff(page);
  await parkOnHotCueA(page);
  const saves = (await smoke.invokes('set_cues')).length;
  await dragCueA(page, 37, false);
  const moved = await cueATime(smoke, saves);
  expect(moved).toBeGreaterThan(HOT_CUE_A_MS + 20);
  expect(distanceToBeat(moved)).toBeGreaterThan(1);
});

test('CUE starts from the nearest grid beat with Q on, from the playhead with Q off', async ({ smoke }) => {
  const { page } = smoke;
  await expandWithTrack(page);
  await quantizeOff(page);
  await page.keyboard.press('Space');
  await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Play');
  const overview = (await page.locator('.ff-xp .waveform-display__overview').boundingBox())!;
  await page.mouse.click(overview.x + overview.width * 0.3, overview.y + overview.height / 2);
  await expect.poll(async () => (await smoke.invokes('playback_seek')).length + (await smoke.invokes('playback_set_position')).length).toBeGreaterThan(0);
  const position = [...(await smoke.invokes('playback_seek')), ...(await smoke.invokes('playback_set_position'))].at(-1)!.positionMs as number;
  expect(distanceToBeat(position)).toBeGreaterThan(40);

  const pressCue = async () => {
    const cue = page.locator('.ff-xp__cue-btn');
    await cue.hover();
    await page.mouse.down();
    await page.waitForTimeout(150);
    await page.mouse.up();
    await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Play');
  };

  await pressCue();
  const plays = await smoke.invokes('playback_play');
  expect(Math.abs(plays.at(-1)!.positionMs - position)).toBeLessThan(2);

  await quantizeButton(page).click();
  await pressCue();
  const quantized = await smoke.invokes('playback_play');
  expect(quantized.length).toBeGreaterThan(plays.length);
  expect(quantized.at(-1)!.positionMs).toBe(nearestBeat(position));
});

test('holding Alt shows a big x centred on a set inspector pad and keeps its letter', async ({ smoke: { page } }) => {
  await page.locator('[data-track-row]').filter({ hasText: TRACK }).first().click();
  const pad = page.locator('.ff-inspector__pad[data-pad="A"]');
  await expect(pad).toHaveClass(/ff-inspector__pad--set/);
  const after = (el: Element) => getComputedStyle(el, '::after').content;
  expect(await pad.evaluate(after)).toBe('none');
  await page.keyboard.down('Alt');
  await expect(pad).toHaveClass(/ff-inspector__pad--remove/);
  expect(await pad.evaluate(after)).toBe('"×"');
  await expect(pad.locator('.ff-inspector__pad-letter')).toHaveText('A');
  const mark = await pad.evaluate((el) => {
    const style = getComputedStyle(el, '::after');
    return { display: style.display, align: style.alignItems, justify: style.justifyContent, position: style.position, size: style.fontSize };
  });
  expect(mark).toEqual({ display: 'flex', align: 'center', justify: 'center', position: 'absolute', size: '32px' });
  if (SHOTS) await pad.locator('xpath=..').screenshot({ path: `${SHOTS}/inspector-pads-alt.png` });
  await page.keyboard.up('Alt');
  await expect(pad).not.toHaveClass(/ff-inspector__pad--remove/);
  if (SHOTS) await pad.locator('xpath=..').screenshot({ path: `${SHOTS}/inspector-pads.png` });
});
