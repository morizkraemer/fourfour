import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import type { Page } from '@playwright/test';
import { expect, rows, test } from './app.ts';
import type { SmokeImport, SmokeWindow } from './tauri-mock.ts';

const SHOTS = resolve(import.meta.dirname, '../../.agent/activity');
const CHIP_SHOTS = resolve(import.meta.dirname, '../../.agent/statuschip');
mkdirSync(SHOTS, { recursive: true });
mkdirSync(CHIP_SHOTS, { recursive: true });

const FOLDER = '/Downloads/Fresh Finds';
const TOP_BAR_CLIP = { x: 0, y: 0, width: 1600, height: 120 };

const button = (page: Page) => page.locator('[data-library-status]');
const menu = (page: Page) => page.locator('[data-library-activity-menu]');
const entries = (page: Page) => menu(page).locator('.ff-menu__item');

function newTrack(id: number, file: string) {
  return {
    id,
    source_path: `${FOLDER}/${file}`,
    title: file.replace(/\.\w+$/, ''),
    artist: 'Drop Artist',
    album: 'Drop Album',
    genre: 'House',
    tempo: 12400,
    key: '8A',
    duration_secs: 300,
    bitrate: 320,
    file_size: 10_000_000,
    has_artwork: false,
    has_cues: false,
    has_analysis: false,
    analysis_pending: false,
  };
}

const TWO_NEW: SmokeImport = { tracks: [newTrack(101, 'Low Sun.mp3'), newTrack(102, 'Side Street.flac')], already: 0 };

async function setWindow(page: Page, patch: Record<string, unknown>) {
  await page.evaluate((p) => Object.assign(window, p), patch);
}

async function emit(page: Page, event: string, payload: unknown) {
  await page.evaluate(([e, p]) => (window as unknown as SmokeWindow).__smokeEmit(e as string, p), [event, payload] as const);
}

async function dismissAnalysisPrompt(page: Page) {
  const notNow = page.getByRole('button', { name: 'Not now' });
  await notNow.click({ timeout: 2000 }).catch(() => undefined);
}

async function importFolder(page: Page) {
  await page.waitForTimeout(450);
  await button(page).click();
  await entries(page).filter({ hasText: 'Import folder…' }).click();
}

async function withProblems(page: Page, missing: number[]) {
  await page.addInitScript((m) => ((window as unknown as Record<string, unknown>).__smokeMissing = m), missing);
  await page.reload();
  await rows(page).first().waitFor();
}

test('the status chip rests as the track count and opens the panel while idle', async ({ smoke: { page } }) => {
  await expect(button(page)).toHaveAttribute('data-activity', 'rest');
  await expect(button(page)).toHaveText(/^[\d,]+ tracks$/);
  await page.screenshot({ path: `${SHOTS}/rest.png`, clip: TOP_BAR_CLIP });
  await button(page).click();
  await expect(menu(page)).toBeVisible();
  await expect(menu(page).locator('[data-library-summary]')).toContainText('tracks');
  await expect(entries(page).filter({ hasText: 'Import folder…' })).toBeEnabled();
  await expect(entries(page).filter({ hasText: 'Import files…' })).toBeEnabled();
  await expect(menu(page).locator('[data-import-progress], [data-analysis-progress]')).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/rest-popover.png`, clip: { x: 900, y: 0, width: 700, height: 360 } });
});

test('an import in progress shows counted progress on the button and in the popover', async ({ smoke }) => {
  const { page } = smoke;
  await setWindow(page, { __smokeHoldImport: true, __smokeImport: TWO_NEW });
  await importFolder(page);

  await expect(button(page)).toHaveAttribute('data-activity', 'importing');
  await expect(button(page)).toHaveText('Importing…');
  await emit(page, 'import-progress', { done: 12, total: 40 });
  await expect(button(page)).toHaveText('Importing 12/40…');
  await expect(button(page).locator('.ff-gheader__progress')).toHaveCSS('width', /^\d/);
  const restBox = (await button(page).boundingBox())!;
  expect(restBox.height).toBe(24);
  await page.screenshot({ path: `${SHOTS}/importing.png`, clip: TOP_BAR_CLIP });

  await button(page).click();
  await expect(menu(page).locator('[data-import-progress]')).toContainText('12 of 40 files');
  await expect(entries(page).filter({ hasText: 'Import folder…' })).toBeDisabled();
  await page.screenshot({ path: `${SHOTS}/importing-popover.png`, clip: { x: 900, y: 0, width: 700, height: 300 } });
  await page.keyboard.press('Escape');

  await page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseImport?.());
  await expect(button(page)).toHaveText('Added 2');
  await dismissAnalysisPrompt(page);
});

test('the result shows for a few seconds, then the button returns to rest', async ({ smoke: { page } }) => {
  await setWindow(page, { __smokeImport: TWO_NEW });
  await importFolder(page);

  await expect(button(page)).toHaveAttribute('data-activity', 'result');
  await expect(button(page)).toHaveText('Added 2');
  await page.screenshot({ path: `${SHOTS}/result.png`, clip: TOP_BAR_CLIP });
  await expect(button(page)).toHaveAttribute('data-activity', 'rest', { timeout: 9000 });
  await expect(button(page)).toHaveText(/ tracks$/);
});

test('the popover keeps the full last-import summary after the button rests', async ({ smoke: { page } }) => {
  await setWindow(page, {
    __smokeDuplicates: [
      { new_path: `${FOLDER}/copy-a.mp3`, file_name: 'copy-a.mp3', existing_id: 7, title: 'Smoke Track 07', artist: 'Four Tet', duration_secs: 222, reason: 'same_tags' },
    ],
    __smokeImport: { ...TWO_NEW, already: 5, skipped: { unsupported: 3, unsupported_exts: ['.m4p', '.txt'], unreadable: 1 } },
  });
  await importFolder(page);
  await page.locator('.ff-dialog').getByRole('button', { name: 'Skip duplicates' }).click();
  await expect(button(page)).toHaveText('Added 2 · 5 already in library · 1 duplicate skipped');
  await dismissAnalysisPrompt(page);
  await expect(button(page)).toHaveAttribute('data-activity', 'rest', { timeout: 9000 });

  await button(page).click();
  const last = menu(page).locator('[data-last-import]');
  await expect(last).toContainText('Last import');
  await expect(last.locator('.ff-activity__headline')).toHaveText('Added 2 · 5 already in library · 1 duplicate skipped');
  await expect(last).toContainText('3 files skipped: unsupported format (.m4p, .txt)');
  await expect(last).toContainText('1 audio file couldn’t be read');
  await page.screenshot({ path: `${SHOTS}/popover-last-import.png`, clip: { x: 900, y: 0, width: 700, height: 360 } });
});

test('Import folder and Import files open the existing import paths', async ({ smoke }) => {
  const { page } = smoke;
  await setWindow(page, { __smokePickPath: FOLDER, __smokeImport: TWO_NEW });
  await importFolder(page);
  await expect(button(page)).toHaveText('Added 2');
  await dismissAnalysisPrompt(page);
  expect(await smoke.invokes('plan_import')).toEqual([{ paths: [FOLDER] }]);
  expect(await smoke.invokes('commit_import')).toHaveLength(1);

  await setWindow(page, { __smokePickPath: `${FOLDER}/Low Sun.mp3`, __smokeImport: { tracks: [], already: 1 } });
  await button(page).click();
  await entries(page).filter({ hasText: 'Import files…' }).click();
  await expect(button(page)).toHaveText('Nothing new');
  expect(await smoke.invokes('plan_import')).toEqual([{ paths: [FOLDER] }, { paths: [`${FOLDER}/Low Sun.mp3`] }]);
});

test('the missing state still works and comes back after an import result', async ({ smoke }) => {
  const { page } = smoke;
  await withProblems(page, [3, 5]);
  await expect(button(page)).toHaveAttribute('data-activity', 'problems');
  await expect(button(page)).toHaveText('2 missing');
  await page.screenshot({ path: `${SHOTS}/missing.png`, clip: TOP_BAR_CLIP });

  await button(page).click();
  await expect(entries(page)).toHaveText([
    'Relocate missing…',
    'Show missing tracks',
    'Check again',
    'Library check results…',
    'Import folder…',
    'Import files…',
  ]);
  await expect(menu(page).locator('[data-last-import]')).toHaveCount(0);
  await page.screenshot({ path: `${SHOTS}/missing-popover.png`, clip: { x: 900, y: 0, width: 700, height: 360 } });
  await page.keyboard.press('Escape');

  await setWindow(page, { __smokePickPath: FOLDER, __smokeImport: TWO_NEW });
  await importFolder(page);
  await expect(button(page)).toHaveText('Added 2');
  await dismissAnalysisPrompt(page);
  await expect(button(page)).toHaveText('2 missing', { timeout: 9000 });
});

const prompt = (page: Page) => page.locator('.ff-dialog').filter({ hasText: 'Analyze imported tracks?' });

test('the analyze prompt offers an always-analyze checkbox that is saved and skips the prompt next time', async ({ smoke }) => {
  const { page } = smoke;
  await setWindow(page, { __smokePickPath: FOLDER, __smokeImport: TWO_NEW });
  await importFolder(page);
  await expect(prompt(page)).toContainText('2 tracks were added');
  const always = prompt(page).locator('[data-analyze-always]');
  await expect(always).toContainText('Always analyze new tracks automatically');
  await expect(always).toHaveAttribute('aria-checked', 'false');
  await always.click();
  await expect(always).toHaveAttribute('aria-checked', 'true');
  await page.screenshot({ path: `${SHOTS}/analyze-prompt-always.png` });
  await prompt(page).getByRole('button', { name: 'Analyze' }).click();
  await expect(prompt(page)).toHaveCount(0);
  expect(await smoke.invokes('set_analyze_after_import')).toEqual([{ enabled: true }]);
  expect((await smoke.invokes('analyze_track_ids')).length).toBe(1);

  await setWindow(page, { __smokeImport: { tracks: [newTrack(103, 'Late Bus.mp3')], already: 0 } });
  await button(page).click();
  await entries(page).filter({ hasText: 'Import folder…' }).click();
  await expect(button(page)).toHaveText('Added 1');
  await expect(prompt(page)).toHaveCount(0);
  await expect.poll(async () => (await smoke.invokes('analyze_track_ids')).length).toBe(2);
});

test('Settings → Library can turn automatic analysis after import off again', async ({ smoke }) => {
  const { page } = smoke;
  await setWindow(page, { __smokeAnalyzeAfterImport: true });
  await page.keyboard.press('Control+,');
  await page.locator('.ff-settings-sheet__nav-item').filter({ hasText: 'Library' }).click();
  const toggle = page.getByRole('switch', { name: 'Always analyze new tracks automatically' });
  await expect(toggle).toBeVisible();
  await expect(toggle).toHaveAttribute('aria-checked', 'true');
  await page.screenshot({ path: `${SHOTS}/settings-library-analyze.png` });
  await toggle.click();
  await expect.poll(async () => (await smoke.invokes('set_analyze_after_import')).at(-1)).toEqual({ enabled: false });
});

test('the Analyzing chip opens the same activity panel with the analysis progress', async ({ smoke }) => {
  const { page } = smoke;
  await emit(page, 'analysis-progress', { current: 1, total: 3, message: 'Analyzing' });
  const chip = button(page);
  await expect(chip).toHaveAttribute('data-activity', 'analyzing');
  await expect(chip).toContainText('Analyzing');
  await expect(chip).toContainText('1 / 3');
  await chip.click();
  await expect(menu(page)).toBeVisible();
  await expect(menu(page).locator('[data-analysis-progress]')).toContainText('1 of 3');
  await expect(menu(page).locator('[data-analysis-left]')).toHaveText('2 tracks to go');
  await expect(entries(page).filter({ hasText: 'Import folder…' })).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/analysis-panel.png`, clip: TOP_BAR_CLIP });
  await page.keyboard.press('Escape');
  await expect(menu(page)).toHaveCount(0);
});

test('status chip screenshots: idle, importing, analyzing, panel open while idle', async ({ smoke }) => {
  const { page } = smoke;
  const clip = { x: 0, y: 0, width: 1600, height: 60 };
  await page.screenshot({ path: `${CHIP_SHOTS}/idle.png`, clip });
  await button(page).click();
  await expect(menu(page)).toBeVisible();
  await page.screenshot({ path: `${CHIP_SHOTS}/panel-idle.png`, clip: { x: 900, y: 0, width: 700, height: 360 } });
  await page.keyboard.press('Escape');

  await emit(page, 'analysis-progress', { current: 1, total: 3, message: 'Analyzing' });
  await expect(button(page)).toHaveAttribute('data-activity', 'analyzing');
  await page.screenshot({ path: `${CHIP_SHOTS}/analyzing.png`, clip });
  await button(page).click();
  await page.screenshot({ path: `${CHIP_SHOTS}/panel-analyzing.png`, clip: { x: 900, y: 0, width: 700, height: 360 } });
  await page.keyboard.press('Escape');

  await setWindow(page, { __smokeHoldImport: true, __smokeImport: TWO_NEW });
  await importFolder(page);
  await emit(page, 'import-progress', { done: 12, total: 40 });
  await expect(button(page)).toHaveAttribute('data-activity', 'importing');
  await page.screenshot({ path: `${CHIP_SHOTS}/importing.png`, clip });
  await page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseImport?.());
  await dismissAnalysisPrompt(page);
});

const progressWidth = (page: Page) =>
  button(page).locator('[data-activity-progress]').evaluate((el) => el.getBoundingClientRect().width / el.parentElement!.getBoundingClientRect().width);

test('the chip bar follows import progress, then analysis progress, instead of sitting full', async ({ smoke }) => {
  const { page } = smoke;
  await setWindow(page, { __smokeHoldImport: true, __smokeImport: TWO_NEW, __smokeAnalyzeAfterImport: true });
  await importFolder(page);
  await emit(page, 'import-progress', { done: 10, total: 40 });
  await expect(button(page)).toHaveAttribute('data-activity', 'importing');
  await expect.poll(() => progressWidth(page)).toBeCloseTo(0.25, 1);
  await button(page).click();
  await expect(menu(page).locator('[data-import-progress]')).toContainText('10 of 40 files');
  await expect(menu(page).locator('[data-import-progress] [role="progressbar"]')).toHaveAttribute('aria-valuenow', '25');
  await page.screenshot({ path: `${SHOTS}/importing-panel.png`, clip: { x: 900, y: 0, width: 700, height: 420 } });
  await page.keyboard.press('Escape');

  await page.evaluate(() => (window as unknown as SmokeWindow).__smokeReleaseImport?.());
  await expect.poll(async () => (await smoke.invokes('analyze_track_ids')).length).toBe(1);
  await emit(page, 'analysis-progress', { current: 0, total: 2, message: 'Analyzing' });
  await expect(button(page)).toHaveAttribute('data-activity', 'analyzing');
  await expect.poll(() => progressWidth(page)).toBeCloseTo(0, 1);
  await emit(page, 'analysis-progress', { current: 1, total: 2, message: 'Analyzing' });
  await expect.poll(() => progressWidth(page)).toBeCloseTo(0.5, 1);
  await page.waitForTimeout(450);
  await button(page).click();
  const analysis = menu(page).locator('[data-analysis-progress]');
  await expect(analysis).toContainText('1 of 2');
  await expect(analysis.locator('[data-analysis-now]')).toHaveText('Now Low Sun');
  await expect(analysis.locator('[data-analysis-left]')).toHaveText('1 track to go');
  await page.screenshot({ path: `${SHOTS}/analyzing-panel.png`, clip: { x: 900, y: 0, width: 700, height: 420 } });
});

test('after a folder import the panel turns the folder into a new playlist', async ({ smoke }) => {
  const { page } = smoke;
  await setWindow(page, { __smokePickPath: FOLDER, __smokeImport: TWO_NEW });
  await importFolder(page);
  await expect(button(page)).toHaveText('Added 2');
  await dismissAnalysisPrompt(page);
  await button(page).click();
  const actions = menu(page).locator('[data-import-folder-actions]');
  await expect(actions).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/folder-actions.png`, clip: { x: 900, y: 0, width: 700, height: 420 } });
  await actions.getByRole('button', { name: 'New playlist from folder' }).click();
  await expect(menu(page)).toHaveCount(0);
  await expect(page.locator('.ff-gheader__title')).toHaveText('Fresh Finds');
  await expect(rows(page)).toHaveCount(2);
});

test('after a folder import the panel adds the folder to a playlist picked from a list', async ({ smoke }) => {
  const { page } = smoke;
  await setWindow(page, { __smokePickPath: FOLDER, __smokeImport: TWO_NEW });
  await importFolder(page);
  await expect(button(page)).toHaveText('Added 2');
  await dismissAnalysisPrompt(page);
  await button(page).click();
  await menu(page).getByRole('button', { name: 'Add to playlist…' }).click();
  const picker = menu(page).locator('[data-import-playlist-picker]');
  await expect(picker.locator('.ff-menu__item').first()).toBeVisible();
  await page.screenshot({ path: `${SHOTS}/folder-add-to-playlist.png`, clip: { x: 900, y: 0, width: 700, height: 520 } });
  await picker.locator('.ff-menu__item', { hasText: /^Warmup$/ }).click();
  await expect(menu(page).locator('[data-import-placed]')).toHaveText('Added 2 to ‘Warmup’');
  const saves = await smoke.invokes('save_state');
  const warmup = (saves.at(-1)!.playlists as { name: string; track_ids: number[] }[]).find((p) => p.name === 'Warmup')!;
  expect(warmup.track_ids.slice(-2)).toEqual([101, 102]);
});
