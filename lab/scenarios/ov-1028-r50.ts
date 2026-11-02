import type { Scenario, ScenarioContext } from '/home/merlin/dev/projects/fourfour/lab/scenarios/types.ts';

const LEFT_SIDEBAR = 240;
const PEAK_TIME = JSON.stringify({ kind: 'playlist', id: 'Peak Time', label: 'Peak Time', playlistId: 1 });

const storage = (width: number, withList: boolean) => ({
  'fourfour.organize.expanded': 'true',
  'fourfour.organize.width': String(width),
  ...(withList ? { 'fourfour.organize.target': PEAK_TIME } : {}),
});

const at = (viewportWidth: number, tag: string, kind: 'empty-max' | 'list'): Scenario => async ({ boot, shot }: ScenarioContext) => {
  const viewport = { width: viewportWidth, height: 900 };
  const width = kind === 'empty-max' ? viewportWidth - LEFT_SIDEBAR : 420;
  const page = await boot({ viewport, holdLoad: true, storage: storage(width, kind === 'list') });
  await page.evaluate(() => (window as unknown as { __smokeReleaseLoad?: () => void }).__smokeReleaseLoad?.());
  await page.locator('.ff-organize--expanded').waitFor();
  await page.waitForTimeout(400);
  await shot(`${tag}-${viewportWidth}`);
};

export default {
  'before-empty-1280': at(1280, 'before-empty', 'empty-max'),
  'before-empty-1600': at(1600, 'before-empty', 'empty-max'),
  'after-empty-1280': at(1280, 'after-empty', 'empty-max'),
  'after-empty-1600': at(1600, 'after-empty', 'empty-max'),
  'after-list-1280': at(1280, 'after-list', 'list'),
  'after-list-1600': at(1600, 'after-list', 'list'),
} satisfies Record<string, Scenario>;
