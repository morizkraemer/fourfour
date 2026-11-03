const FOCUSABLE =
  'input:not([disabled]), button:not([disabled]), [href], select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function isRendered(el: HTMLElement) {
  return el.getClientRects().length > 0 && getComputedStyle(el).visibility !== 'hidden';
}

function entryPoint(region: HTMLElement) {
  const first = region.querySelector<HTMLElement>('[data-focus-first]');
  if (first && isRendered(first)) return first;
  return Array.from(region.querySelectorAll<HTMLElement>(FOCUSABLE)).find(isRendered);
}

export function focusRegion(step: 1 | -1, root: ParentNode = document) {
  const regions = Array.from(root.querySelectorAll<HTMLElement>('[data-focus-region]'));
  if (regions.length === 0) return;
  const current = regions.findIndex((r) => r.contains(document.activeElement));
  for (let offset = 1; offset <= regions.length; offset++) {
    const index = (((current + step * offset) % regions.length) + regions.length) % regions.length;
    const target = entryPoint(regions[index]);
    if (target) {
      target.focus();
      return;
    }
  }
}
