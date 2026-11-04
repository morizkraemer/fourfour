import { listen } from '@tauri-apps/api/event';
import { currentWebviewWindow } from './webview-window.ts';
import { getIsTauri } from './tauri.svelte.ts';

/** macOS re-applies trafficLightPosition from tauri.conf after resize (incl. title-reset bug). */
async function nudgeTrafficLightPosition(): Promise<void> {
  if (!getIsTauri()) return;
  const [win, { PhysicalSize }] = await Promise.all([currentWebviewWindow(), import('@tauri-apps/api/dpi')]);
  const size = await win.outerSize();
  await win.setSize(new PhysicalSize(size.width, size.height));
}

export function installMacWindowChrome(): () => void {
  if (!getIsTauri()) return () => {};

  void nudgeTrafficLightPosition();
  requestAnimationFrame(() => void nudgeTrafficLightPosition());

  const unlisteners: Promise<() => void>[] = [
    currentWebviewWindow().then((win) => win.onResized(() => void nudgeTrafficLightPosition())),
    listen('tauri://theme-changed', () => void nudgeTrafficLightPosition()),
  ];

  return () => {
    void Promise.all(unlisteners).then((fns) => fns.forEach((fn) => fn()));
  };
}
