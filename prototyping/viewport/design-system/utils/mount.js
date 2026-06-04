import { mount } from 'svelte';

/**
 * Mount a Svelte component into a layout-neutral host node and return it.
 *
 * The viewport artboard contract wants ONE DOM node back from render(); Svelte's
 * `mount` wants a target to render into. `display:contents` makes the host vanish
 * from layout so the component's own root is what the parent flexbox/grid sees.
 *
 * Use inside artboards: `host(Button, { label: 'Curate', variant: 'primary' })`.
 */
export function host(Component, props = {}) {
  const target = document.createElement('div');
  target.style.display = 'contents';
  mount(Component, { target, props });
  return target;
}
