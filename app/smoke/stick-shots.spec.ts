import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test as base, type Locator, type Page } from '@playwright/test';
import type { LoadedState, TrackInfo } from '../src/services/tauri.svelte.ts';
import { mockAnalysisData } from '../src/mocks/analysis.ts';
import { installTauriMock, type SmokeStick, type SmokeWindow } from './tauri-mock.ts';

const OUT = process.env.FF_STICK_SHOTS;

const TRACKS: [string, string, string, string, number, string][] = [
  ['Glue', 'Bicep', 'Bicep', 'Breakbeat', 130, '8A'],
  ['Opal (Four Tet Remix)', 'Bicep', 'Opal', 'Breakbeat', 128, '5A'],
  ['Kid Cudi (Baby)', 'Solomun', 'Nobody Is Not Loved', 'Melodic House', 122, '4A'],
  ['Home', 'Solomun', 'Nobody Is Not Loved', 'Melodic House', 120, '9A'],
  ['Breathe', 'Ben Böhmer', 'Breathing', 'Melodic House', 122, '6A'],
  ['Beyond Beliefs', 'Ben Böhmer', 'Begin Again', 'Melodic House', 124, '11B'],
  ['Baby', 'Four Tet', 'Sixteen Oceans', 'House', 127, '10A'],
  ['Teenage Birdsong', 'Four Tet', 'Sixteen Oceans', 'House', 125, '2B'],
  ['Gosh', 'Jamie xx', 'In Colour', 'UK Bass', 124, '1A'],
  ['Loud Places', 'Jamie xx', 'In Colour', 'UK Bass', 119, '7B'],
  ['Marea (We Lost Dancing)', 'Fred again..', 'Actual Life 2', 'House', 126, '3A'],
  ['Turn On The Lights again..', 'Fred again..', 'Turn On The Lights again..', 'House', 132, '12A'],
  ['Pick Up', 'DJ Koze', 'Knock Knock', 'House', 123, '4B'],
  ['Seeing Aliens', 'DJ Koze', 'Knock Knock', 'House', 118, '9B'],
  ['Inspector Norse', 'Todd Terje', "It's Album Time", 'Disco', 120, '8B'],
  ['Strandbar', 'Todd Terje', "It's Album Time", 'Disco', 122, '6B'],
  ['Cola', 'CamelPhat, Elderbrook', 'Dark Matter', 'Tech House', 124, '5B'],
  ['Panic Room', 'CamelPhat, Au/Ra', 'Dark Matter', 'Tech House', 124, '2A'],
  ['Renaissance', 'Tale Of Us', 'Endless', 'Melodic Techno', 123, '10B'],
  ['Monument', 'Tale Of Us', 'Afterlife Voyage', 'Melodic Techno', 124, '1B'],
  ['Ghost Voices', 'Virtual Self', 'Virtual Self', 'Trance', 134, '11A'],
  ['Xtal', 'Aphex Twin', 'Selected Ambient Works 85-92', 'Ambient Techno', 115, '3B'],
  ['Silhouettes', 'Floating Points', 'Elaenia', 'Electronica', 121, '7A'],
  ['LesAlpx', 'Floating Points', 'Crush', 'Breakbeat', 131, '12B'],
];

function track(i: number): TrackInfo {
  const [title, artist, album, genre, bpm, key] = TRACKS[i];
  return {
    id: i + 1,
    source_path: `/Music/${artist}/${album}/${title}.flac`,
    title,
    artist,
    album,
    genre,
    tempo: bpm * 100,
    key,
    duration_secs: 290 + ((i * 37) % 170),
    bitrate: 320,
    file_size: 11_400_000 + ((i * 2_713_000) % 9_000_000),
    has_artwork: false,
    has_cues: true,
    has_analysis: true,
    analysis_pending: false,
  };
}

const STATE: LoadedState = {
  tracks: TRACKS.map((_, i) => track(i)),
  playlists: [
    { id: 1, name: 'Warmup', track_ids: [3, 4, 5, 13, 14, 23], parent_id: 0, is_folder: false },
    { id: 2, name: 'Peak Time', track_ids: [1, 2, 11, 12, 17, 18, 19, 24], parent_id: 0, is_folder: false },
    { id: 3, name: 'Closing', track_ids: [10, 15, 16, 22], parent_id: 0, is_folder: false },
    { id: 4, name: 'Pudel, Friday', track_ids: [7, 8, 9, 20, 21], parent_id: 0, is_folder: false },
  ],
};

const GB = 1_000_000_000;
const INTENSO = '/Volumes/INTENSO';
const MK_BLACK = '/Volumes/MK_BLACK';
const SANDISK = '/Volumes/SANDISK 8GB';
const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();

function intenso(): SmokeStick {
  return {
    total: 31.9 * GB,
    free: 29.2 * GB,
    contents: { stick_id: 'stick-intenso', track_ids: [6, 9], playlist_ids: [1, 2], kept_as_is: [] },
    written: [6, 9, 3, 4, 5, 13, 14, 23, 1, 2, 11, 12, 17, 18, 19, 24],
    foreign: 0,
    playlists: [
      { id: 1, name: 'Warmup', track_count: 6 },
      { id: 2, name: 'Peak Time', track_count: 8 },
      { id: 9, name: 'Hamburg Oct 2023', track_count: 26 },
    ],
    last_sync: minutesAgo(2),
  };
}

function mkBlack(): SmokeStick {
  return {
    total: 62.5 * GB,
    free: 41.3 * GB,
    contents: { stick_id: 'stick-mk', track_ids: [22], playlist_ids: [2, 4], kept_as_is: [] },
    written: [22, 1, 2, 11, 12, 17, 18, 19],
    foreign: 0,
    playlists: [{ id: 2, name: 'Peak Time', track_count: 7 }],
    last_sync: minutesAgo(60 * 26),
  };
}

function sandisk(): SmokeStick {
  return {
    total: 7.8 * GB,
    free: 41_000_000,
    contents: { stick_id: 'stick-sd', track_ids: [], playlist_ids: [2, 3], kept_as_is: [] },
    written: [10, 15, 16, 22],
    foreign: 0,
    playlists: [{ id: 3, name: 'Closing', track_count: 4 }],
    last_sync: minutesAgo(60 * 24 * 12),
  };
}

const test = base.extend<{ sticks: Record<string, SmokeStick> }>({
  sticks: [{}, { option: true }],
  page: async ({ page, sticks }, use) => {
    await page.exposeFunction('__smokeAnalysis', (trackId: number) => {
      const t = STATE.tracks.find((x) => x.id === trackId)!;
      return mockAnalysisData(t.id, t.duration_secs, t.tempo, t.key);
    });
    await page.addInitScript(installTauriMock, STATE);
    await page.addInitScript((seed) => {
      (window as unknown as SmokeWindow).__smokeSticks = seed;
      if (!sessionStorage.getItem('shots.seeded')) {
        localStorage.setItem('fourfour.organize.expanded', 'false');
        sessionStorage.setItem('shots.seeded', '1');
      }
    }, sticks);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor();
    await use(page);
  },
});

test.use({ deviceScaleFactor: 2 });
test.skip(!OUT, 'set FF_STICK_SHOTS=<dir>');

const stickRow = (page: Page, path: string) => page.locator(`[data-drop-stick-path="${path}"][data-drop-surface="row"]`);
const sidebarSticks = (page: Page) => page.locator('.ff-sidebar__bottom');
const inspector = (page: Page) => page.locator('[data-stick-inspector]');
const topRight = (page: Page) => page.locator('.ff-gheader__right');
const view = (page: Page) => page.locator('.ff-browse__slot--table [data-stick-view]');

async function shot(target: Page | Locator, name: string) {
  mkdirSync(OUT!, { recursive: true });
  await target.screenshot({ path: join(OUT!, `${name}.png`) });
}

async function openStick(page: Page, path: string) {
  await stickRow(page, path).locator('.ff-sbrow__label').click();
  await expect(view(page)).toHaveAttribute('data-stick-view', path);
  await expect(inspector(page)).toBeVisible();
  await page.mouse.move(700, 600);
}

async function settled(page: Page, path: string, kind: string) {
  await expect(stickRow(page, path).locator('[data-stick-status]')).toHaveAttribute('data-stick-status', kind);
}

test.describe('calm: one stick, in sync', () => {
  test.use({ sticks: { [INTENSO]: intenso() } });

  test('top bar and sidebar stay quiet, inspector says in sync', async ({ page }) => {
    await settled(page, INTENSO, 'idle');
    await page.waitForTimeout(1200);
    await expect(page.locator('[data-stick-chip]')).toHaveCount(0);
    await shot(topRight(page), 'topbar-no-problem');
    await shot(sidebarSticks(page), 'sidebar-idle');
    await openStick(page, INTENSO);
    await shot(page, 'full-stick-in-sync');
    await shot(inspector(page), 'inspector-in-sync');
  });

  test('side panel is a plain list; stick left plus panel has one header', async ({ page }) => {
    await stickRow(page, INTENSO).hover();
    await stickRow(page, INTENSO).locator('.ff-sbrow__action').click();
    const panel = page.locator('.ff-organize--expanded');
    await expect(panel.locator('[data-stick-board]')).toBeVisible();
    await page.mouse.move(700, 600);
    await shot(page, 'full-library-plus-stick-panel');
    await shot(panel, 'panel-plain-list');
    await panel.locator('[data-stick-row="playlist"]').first().click();
    await expect(panel.locator('[data-stick-tracks]')).toBeVisible();
    await page.mouse.move(700, 600);
    await shot(panel, 'panel-playlist-tracks');
    await stickRow(page, INTENSO).locator('.ff-sbrow__label').click();
    await page.mouse.move(700, 600);
    await shot(page, 'full-stick-left-plus-panel');
  });

  test('stick playlist: row hover, context menu, reorder', async ({ page }) => {
    await openStick(page, INTENSO);
    await view(page).locator('[data-stick-row="playlist"]').filter({ hasText: 'Peak Time' }).click();
    const rows = view(page).locator('[data-stick-track]');
    await expect(rows).toHaveCount(8);
    await rows.nth(2).hover();
    await shot(page, 'full-stick-playlist-hover');
    await rows.nth(2).click({ button: 'right' });
    await expect(page.locator('.ff-menu, [role="menu"]').first()).toBeVisible();
    await shot(page, 'full-stick-track-menu');
    await page.keyboard.press('Escape');
    const from = (await rows.nth(5).boundingBox())!;
    const to = (await rows.nth(1).boundingBox())!;
    await page.mouse.move(from.x + 200, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(from.x + 210, from.y - 10, { steps: 4 });
    await page.mouse.move(to.x + 220, to.y + 3, { steps: 10 });
    await expect(page.locator('[data-stick-dropline]')).toBeVisible();
    await shot(page, 'full-stick-reorder-drag');
    await page.mouse.up();
    await expect(page.locator('[data-stick-notice]')).toBeVisible();
    await expect(rows.nth(1)).toContainText('Panic Room');
    await page.mouse.move(700, 700);
    await shot(page, 'full-stick-after-reorder');
  });
});

test.describe('mixed: three sticks', () => {
  test.use({ sticks: { [INTENSO]: intenso(), [MK_BLACK]: mkBlack(), [SANDISK]: sandisk() } });

  test('changes waiting, will not fit, syncing, error', async ({ page }) => {
    await settled(page, INTENSO, 'idle');
    await settled(page, MK_BLACK, 'pending');
    await settled(page, SANDISK, 'wont-fit');
    await shot(sidebarSticks(page), 'sidebar-waiting-and-wont-fit');
    await shot(topRight(page), 'topbar-problem');
    await shot(page, 'full-library-with-attention');

    await openStick(page, SANDISK);
    await shot(inspector(page), 'inspector-wont-fit');

    await openStick(page, MK_BLACK);
    await shot(inspector(page), 'inspector-changes-waiting');
    await shot(page, 'full-stick-changes-waiting');

    await page.evaluate(() => ((window as unknown as SmokeWindow).__smokeHoldSync = true));
    await inspector(page).getByRole('button', { name: 'Sync now' }).click();
    await settled(page, MK_BLACK, 'syncing');
    await page.evaluate((volume) => {
      (window as unknown as SmokeWindow).__smokeEmit('usb-sync-progress', {
        volume,
        phase: 'copying',
        done: 3,
        total: 6,
        message: 'Copying Four Tet - Teenage Birdsong',
      });
    }, 'MK_BLACK');
    await expect(inspector(page).locator('[data-stick-detail]')).toContainText('Teenage Birdsong');
    await page.mouse.move(700, 600);
    await shot(inspector(page), 'inspector-syncing');
    await shot(sidebarSticks(page), 'sidebar-syncing');
    await shot(topRight(page), 'topbar-syncing');

    await page.evaluate(() => {
      const w = window as unknown as SmokeWindow;
      w.__smokeFailSync = 'Cannot open /Volumes/MK_BLACK: the stick was removed.';
      w.__smokeHoldSync = false;
      w.__smokeReleaseSync?.();
    });
    await settled(page, MK_BLACK, 'error');
    await shot(inspector(page), 'inspector-error');
    await shot(sidebarSticks(page), 'sidebar-error');
    await shot(topRight(page), 'topbar-error');
  });
});
