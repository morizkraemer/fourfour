import { lazyView } from './lazy.svelte.ts';

export const inspector = lazyView(() => import('../modules/Detail.svelte'));
export const appDialogs = lazyView(() => import('../modules/AppDialogs.svelte'));
export const stickView = lazyView(() => import('../modules/StickView.svelte'));
export const stickBoard = lazyView(() => import('../modules/StickBoard.svelte'));
export const stickInspector = lazyView(() => import('../modules/StickInspector.svelte'));
export const expandedPlayer = lazyView(() => import('../modules/ExpandedPlayer.svelte'));
export const libraryActivityMenu = lazyView(() => import('../modules/LibraryActivityMenu.svelte'));
export const trackKeyPopup = lazyView(() => import('../modules/TrackKeyPopup.svelte'));
export const trackBpmPopup = lazyView(() => import('../modules/TrackBpmPopup.svelte'));
