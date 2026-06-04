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
export { createWaveform } from './components/waveform.js';
export { keyColor } from './utils/key-color.js';
export { createCheckbox, createToggle, createRadio } from './components/checkbox.js';
export { createInput } from './components/input.js';
export { createColorPicker } from './components/color-picker.js';
export { createSlider } from './components/slider.js';
export { createSegmentedControl } from './components/segmented-control.js';
export { createStars } from './components/stars.js';
export { createKbd } from './components/kbd.js';

// Newly added components
export { createProgressBar } from './components/progress-bar.js';
export { createSpinner } from './components/spinner.js';
export { createEmptyState } from './components/empty-state.js';
export { createToast } from './components/toast.js';
export { createDialog } from './components/dialog.js';
export { createInlineAlert } from './components/inline-alert.js';
export { createMenu } from './components/menu.js';
export { createCommandPalette } from './components/command-palette.js';
export { createSidebarSection } from './components/sidebar-section.js';
export { createPanelHeader } from './components/panel-header.js';
export { createFilterInput } from './components/filter-input.js';
export { createNudge } from './components/nudge.js';
export { createStatusBar } from './components/status-bar.js';
export { createTagChip, createDigitPill, createTargetChip } from './components/tag-chip.js';
export { createDragGhost, createDropLine, createDropOverlay } from './components/drag-ghost.js';
export { createConflictDiff } from './components/conflict-diff.js';
export { createBulkActionBar } from './components/bulk-action-bar.js';
export { createPlayerCompact } from './components/player-compact.js';
export { createPlayerExpanded } from './components/player-expanded.js';
export { createDetailPane } from './components/detail-pane.js';
export { createOnboarding } from './components/onboarding.js';
export { createSettingsPage } from './components/settings-page.js';
export { createLevelMeter } from './components/level-meter.js';

