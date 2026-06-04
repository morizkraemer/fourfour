/*
 * Svelte design-system barrel — the production entry that lifts into the Tauri
 * app unchanged. Importing it loads tokens + base globally, then re-exports every
 * ported Svelte component plus framework-agnostic helpers.
 *
 * Grows one line per primitive as the port proceeds (see docs/todo.md phase 3).
 * The vanilla `index.js` barrel still exists for the legacy factory artboards;
 * this file is the forward-looking one.
 */
import './tokens.css';
import './base.css';
import './components/dialog.css';
import './components/dialog-native.css';

// Helpers
export { host } from './utils/mount.js';
export { keyColor } from './utils/key-color.js';
export { ICON_NAMES } from './components/icon-glyphs.js';

// Primitives (Svelte) — tier 0 (leaves: tokens/dom only)
export { default as Icon } from './components/Icon.svelte';
export { default as Kbd } from './components/Kbd.svelte';
export { default as Button } from './components/Button.svelte';
export { default as TagBadge } from './components/TagBadge.svelte';
export { default as StatusDot } from './components/StatusDot.svelte';
export { default as ColorSwatch } from './components/ColorSwatch.svelte';
export { default as Spinner } from './components/Spinner.svelte';
export { default as ProgressBar } from './components/ProgressBar.svelte';
export { default as Nudge } from './components/Nudge.svelte';
export { default as SidebarSection } from './components/SidebarSection.svelte';
export { default as Waveform } from './components/Waveform.svelte';
export { default as WaveformStrip } from './components/WaveformStrip.svelte';
export { WaveformRenderer } from './waveform-renderer.ts';

// Primitives (Svelte) — tier 1 (compose tier 0)
export { default as ColumnHeader } from './components/ColumnHeader.svelte';
export { default as StatusBar } from './components/StatusBar.svelte';
export { default as SidebarRow } from './components/SidebarRow.svelte';
export { default as TrackRow, TRACK_COLUMNS } from './components/TrackRow.svelte';
export { default as PlayerCompact } from './components/PlayerCompact.svelte';
export { default as DetailPane } from './components/DetailPane.svelte';
export { default as EmptyState } from './components/EmptyState.svelte';
export { default as Dialog } from './components/Dialog.svelte';
