import { expect, test as base, type Locator, type Page } from '@playwright/test';
import { fixtureAnalysis, fixtureState } from './fixture.ts';
import { installTauriMock, type SmokeWindow } from './tauri-mock.ts';

export interface Smoke {
  page: Page;
  invokes(cmd: string): Promise<Record<string, any>[]>;
  answerDialogs(answer: boolean): Promise<void>;
  dialogs(): Promise<string[]>;
}

const BENIGN_PAGE_ERRORS = [/^ResizeObserver loop completed with undelivered notifications\.?$/];
const BENIGN_CONSOLE = [/smoke: track not analysed/];

export const test = base.extend<{ smoke: Smoke }>({
  smoke: async ({ page }, use) => {
    const problems: string[] = [];
    page.on('console', (message) => {
      if ((message.type() === 'error' || message.type() === 'warning') && !BENIGN_CONSOLE.some((benign) => benign.test(message.text()))) problems.push(`console.${message.type()}: ${message.text()}`);
    });
    page.on('pageerror', (error) => {
      if (!BENIGN_PAGE_ERRORS.some((benign) => benign.test(error.message))) problems.push(`pageerror: ${error.message}`);
    });
    await page.exposeFunction('__smokeAnalysis', (trackId: number) => fixtureAnalysis(trackId));
    await page.addInitScript(installTauriMock, fixtureState());
    await page.goto('/');
    await page.locator('[data-track-row]').first().waitFor();

    await use({
      page,
      invokes: (cmd) =>
        page.evaluate(
          (name) => (window as unknown as SmokeWindow).__smokeInvokes.filter((i) => i.cmd === name).map((i) => i.args as Record<string, any>),
          cmd,
        ),
      answerDialogs: (answer) => page.evaluate((a) => ((window as unknown as SmokeWindow).__smokeAnswerDialog = a), answer).then(() => undefined),
      dialogs: () => page.evaluate(() => (window as unknown as SmokeWindow).__smokeDialogs),
    });

    const unhandled = await page.evaluate(() => (window as unknown as SmokeWindow).__smokeUnhandled);
    expect(unhandled, 'invoke commands the fixture backend does not know').toEqual([]);
    expect(problems, 'console errors and page errors').toEqual([]);
  },
});

export { expect };

export const rows = (page: Page) => page.locator('[data-track-row]');
export const rowTitles = (page: Page) => rows(page).locator('.ff-trow__cell--title .ff-trow__text').allTextContents();
export const rowByTitle = (page: Page, title: string) => rows(page).filter({ hasText: title }).first();
export const sidebarPlaylist = (page: Page, name: string) =>
  page.locator('[data-drop-track-playlist-id]').filter({ has: page.locator('.ff-sbrow__label', { hasText: new RegExp(`^${name}$`) }) });

export async function loadByDoubleClick(page: Page, title: string) {
  await rowByTitle(page, title).dblclick();
  await expect(page.locator('.ff-player-compact__meta-title')).toHaveText(title);
}

export async function dragTo(page: Page, from: Locator, to: Locator, hovering: Locator) {
  const source = (await from.locator('.ff-trow__cell--title').boundingBox())!;
  const target = (await to.boundingBox())!;
  await page.mouse.move(source.x + 20, source.y + source.height / 2);
  await page.mouse.down();
  await page.mouse.move(source.x + 30, source.y + source.height / 2 + 12, { steps: 4 });
  await page.mouse.move(target.x + target.width / 2, target.y + target.height / 2, { steps: 12 });
  await expect(hovering).toBeVisible();
  await page.mouse.up();
}
