import type { Page } from '@playwright/test';
import { expect, test } from './app.ts';

async function audit(page: Page, label: string) {
  const r = await page.evaluate(() => {
    const focusable = (el: Element) => {
      let n: Element | null = el;
      while (n) {
        const t = n.tagName;
        if (['BUTTON', 'INPUT', 'SELECT', 'TEXTAREA', 'A'].includes(t) && !(n as HTMLButtonElement).disabled) return true;
        const ti = n.getAttribute('tabindex');
        if (ti !== null && Number(ti) >= 0) return true;
        n = n.parentElement;
      }
      return false;
    };
    const out: string[] = [];
    document.querySelectorAll('*').forEach((el) => {
      const cs = getComputedStyle(el);
      if (cs.cursor === 'pointer' && cs.display !== 'none' && (el as HTMLElement).offsetParent !== null && !focusable(el)) {
        const parentPointer = el.parentElement && getComputedStyle(el.parentElement).cursor === 'pointer';
        if (!parentPointer) out.push(el.tagName + '.' + String(el.className).slice(0, 60) + '|' + ((el as HTMLElement).innerText || '').slice(0, 20).replace(/\n/g, ' '));
      }
    });
    const name = (el: Element) => (el.getAttribute('aria-label') || el.getAttribute('title') || (el as HTMLElement).innerText || '').trim();
    const unnamed = [...document.querySelectorAll('button, [role=button], [role=menuitem], [role=checkbox], [role=switch], [role=slider], [role=spinbutton]')]
      .filter((b) => !name(b) && !b.getAttribute('aria-labelledby'))
      .map((b) => b.tagName + '.' + String(b.className).slice(0, 60));
    const noLabel = [...document.querySelectorAll('input, textarea, select')]
      .filter((i) => (i as HTMLInputElement).type !== 'hidden' && !i.getAttribute('aria-label') && !i.getAttribute('aria-labelledby') && !(i as HTMLInputElement).labels?.length)
      .map((i) => i.tagName + '.' + String(i.className).slice(0, 60));
    return { unreachableClickables: [...new Set(out)], unnamed, noLabel };
  });
  console.log(`=== ${label}\n${JSON.stringify(r, null, 1)}`);
}

test('probe initial and surfaces', async ({ smoke: { page } }) => {
  await audit(page, 'initial');
  await page.locator('[data-track-row]').first().click();
  await audit(page, 'selection (inspector)');
  await page.locator('[data-track-row]').first().dblclick();
  await page.waitForTimeout(300);
  await page.locator('.ff-player-compact__expand-btn').click();
  await page.waitForTimeout(500);
  await audit(page, 'expanded player');
  await page.keyboard.press('Control+,');
  for (const key of ['library', 'backup', 'analysis', 'audio', 'keyboard', 'advanced', 'about']) {
    await page.locator(`[data-page="${key}"]`).click();
    await page.waitForTimeout(200);
    await audit(page, 'settings ' + key);
  }
  expect(1).toBe(1);
});
