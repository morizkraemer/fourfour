import type { Page } from '@playwright/test';
import { expect, loadByDoubleClick, test } from './app.ts';
import { loadFactor } from './load-factor.ts';

const SLIDE_PX = 100000;
const FRACS = [0.0213, 0.1, 0.3717, 0.5, 0.7311, 0.9302];

type Swapped = { el: HTMLElement; animation: Animation; time: number }[];

async function asStyleTransform(page: Page): Promise<number> {
  return page.evaluate((slidePx) => {
    const swapped: Swapped = [];
    for (const el of document.querySelectorAll<HTMLElement>('.waveform-display div')) {
      for (const animation of el.getAnimations()) {
        const time = Number(animation.currentTime);
        animation.cancel();
        el.style.transform = `translateX(${time - slidePx}px)`;
        swapped.push({ el, animation, time });
      }
    }
    (window as unknown as { __swapped: Swapped }).__swapped = swapped;
    return swapped.length;
  }, SLIDE_PX);
}

async function backToAnimations(page: Page) {
  await page.evaluate(() => {
    for (const { el, animation, time } of (window as unknown as { __swapped: Swapped }).__swapped) {
      el.style.transform = '';
      animation.currentTime = time;
    }
  });
}

async function pixelDelta(page: Page, a: Buffer, b: Buffer) {
  return page.evaluate(async ([a64, b64]) => {
    const pixels = async (data: string) => {
      const image = new Image();
      image.src = `data:image/png;base64,${data}`;
      await image.decode();
      const canvas = document.createElement('canvas');
      canvas.width = image.width;
      canvas.height = image.height;
      const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
      ctx.drawImage(image, 0, 0);
      return ctx.getImageData(0, 0, image.width, image.height).data;
    };
    const [pa, pb] = [await pixels(a64), await pixels(b64)];
    let max = 0;
    let differing = 0;
    for (let i = 0; i < pa.length; i += 4) {
      const d = Math.max(Math.abs(pa[i] - pb[i]), Math.abs(pa[i + 1] - pb[i + 1]), Math.abs(pa[i + 2] - pb[i + 2]));
      if (d) differing++;
      max = Math.max(max, d);
    }
    return { max, differing };
  }, [a.toString('base64'), b.toString('base64')]);
}

async function compareWithStyleTransform(page: Page, selector: string, label: string) {
  await page.waitForTimeout(250);
  const box = (await page.locator(selector).first().boundingBox())!;
  const animated = await page.screenshot({ clip: box, animations: 'allow' });
  expect(await asStyleTransform(page), `${label}: elements moved by a paused animation`).toBeGreaterThanOrEqual(1);
  const styled = await page.screenshot({ clip: box, animations: 'allow' });
  await backToAnimations(page);
  return animated.equals(styled) ? { max: 0, differing: 0 } : pixelDelta(page, animated, styled);
}

for (const deviceScaleFactor of [1, 2]) {
  test.describe(`at device pixel ratio ${deviceScaleFactor}`, () => {
    test.use({ deviceScaleFactor });

    test('the playheads and played mask draw the same pixels as the style.transform they replaced', async ({ smoke: { page } }) => {
      test.setTimeout(120_000 * loadFactor());
      await loadByDoubleClick(page, 'Smoke Track 03');
      await page.locator('.ff-player-compact__expand-btn').click();
      await expect(page.locator('.ff-xp')).toBeVisible();
      if ((await page.locator('.ff-xp__play').getAttribute('aria-label')) === 'Pause') await page.keyboard.press('Space');
      await expect(page.locator('.ff-xp__play')).toHaveAttribute('aria-label', 'Play');
      const overview = (await page.locator('.ff-xp .waveform-display__overview').boundingBox())!;
      const zoom = (await page.locator('.ff-xp .waveform-display__zoom').boundingBox())!;
      let compared = 0;
      for (const frac of FRACS) {
        await page.mouse.click(overview.x + overview.width * frac, overview.y + overview.height / 2);
        for (const wheel of [0, -300]) {
          if (wheel) {
            await page.mouse.move(zoom.x + zoom.width / 2, zoom.y + zoom.height / 2);
            await page.mouse.wheel(0, wheel);
          }
          const delta = await compareWithStyleTransform(page, '.ff-xp .waveform-display', `expanded frac ${frac} wheel ${wheel}`);
          expect(delta, `expanded frac ${frac} wheel ${wheel}: identical`).toEqual({ max: 0, differing: 0 });
          compared++;
        }
      }

      await page.locator('.ff-xp__collapse').click();
      await expect(page.locator('.ff-xp')).toHaveCount(0);
      const strip = (await page.locator('.ff-player-compact .waveform-display__zoom').boundingBox())!;
      for (const frac of FRACS) {
        await page.mouse.click(strip.x + strip.width * frac, strip.y + strip.height / 2);
        await page.mouse.move(0, 0);
        const delta = await compareWithStyleTransform(page, '.ff-player-compact', `compact frac ${frac}`);
        console.log(`PLAYHEAD dpr ${deviceScaleFactor} compact frac ${frac}: ${delta.differing} pixels differ, by at most ${delta.max}/255`);
        expect(delta.max, `compact frac ${frac}: at most one level of rounding in the composited played mask`).toBeLessThanOrEqual(1);
        compared++;
      }
      expect(compared).toBe(3 * FRACS.length);
    });
  });
}
