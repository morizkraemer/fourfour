const FOCUSABLE =
  'input:not([disabled]), button:not([disabled]), [href], select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function focusRegion(step: 1 | -1, root: ParentNode = document) {
  const regions = Array.from(root.querySelectorAll<HTMLElement>('[data-focus-region]'));
  if (regions.length === 0) return;
  const current = regions.findIndex((r) => r.contains(document.activeElement));
  const next = regions[(current + step + regions.length) % regions.length];
  (next.querySelector<HTMLElement>(FOCUSABLE) ?? next).focus();
}
