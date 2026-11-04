import { lazyView } from './lazy.svelte.ts';

export const appDialogs = lazyView(() => import('../modules/AppDialogs.svelte'));
export const inspector = lazyView(() => import('../modules/Detail.svelte'));
export const stickView = lazyView(() => import('../modules/StickView.svelte'));
export const expandedPlayer = lazyView(() => import('../modules/ExpandedPlayer.svelte'));
export const libraryActivityMenu = lazyView(() => import('../modules/LibraryActivityMenu.svelte'));
export const trackKeyPopup = lazyView(() => import('../modules/TrackKeyPopup.svelte'));
export const trackBpmPopup = lazyView(() => import('../modules/TrackBpmPopup.svelte'));
