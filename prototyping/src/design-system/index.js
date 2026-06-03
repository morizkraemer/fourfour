/*
 * Design-system barrel. Importing this loads tokens + base, then re-exports
 * every primitive and util. This folder is the reuse target: it lifts into
 * the Tauri frontend unchanged.
 */
import './tokens.css';
import './base.css';

// Utils
export * from './utils/dom.js';

// Components
export { createIcon, ICON_NAMES } from './components/icon.js';
export { createButton } from './components/button.js';
export { createTagBadge } from './components/tag-badge.js';
export { createStatusDot } from './components/status-dot.js';
export { createColorSwatch } from './components/color-swatch.js';
export { createColumnHeader } from './components/column-header.js';
export { createSidebarRow } from './components/sidebar-row.js';
export { createTrackRow, TRACK_COLUMNS } from './components/track-row.js';
export { createCheckbox, createToggle, createRadio } from './components/checkbox.js';
export { createInput } from './components/input.js';
export { createColorPicker } from './components/color-picker.js';
export { createSlider } from './components/slider.js';
export { createSegmentedControl } from './components/segmented-control.js';
export { createStars } from './components/stars.js';
export { createKbd } from './components/kbd.js';
