import { el } from '../design-system/utils/dom.js';
import {
  createDetailPane,
  createOnboarding,
  createSettingsPage,
  createToggle,
  createInput,
  createSegmentedControl,
  createSlider
} from '../design-system/index.js';

export const meta = { title: 'Panels & Pages (detail-pane · onboarding · settings)', layer: 'composite' };

const cap = (t) =>
  el('div', {
    style: {
      fontFamily: 'var(--ff-font-mono)',
      fontSize: 'var(--ff-type-label)',
      letterSpacing: '0.06em',
      fontWeight: '500',
      color: 'var(--ff-faint)',
      marginTop: 'var(--ff-space-4)',
      marginBottom: 'var(--ff-space-2)',
    },
    text: t,
  });

export default function render() {
  // 1. Detail Pane Modes
  const paneEmpty = createDetailPane({ mode: 'empty' }).element;

  const paneSingle = createDetailPane({
    mode: 'single',
    title: 'One More Time',
    artist: 'Daft Punk',
    meta: {
      Album: 'Discovery',
      Label: 'Virgin',
      BPM: '128.00',
      Key: '8A',
      Duration: '05:20',
      Added: '2026-06-03'
    },
    cues: [
      { name: 'Intro', position: '00:00.000' },
      { name: 'Beat In', position: '00:15.000' },
      { name: 'Breakdown', position: '01:45.000' },
      { name: 'Outro', position: '04:40.000' }
    ]
  }).element;

  const paneMulti = createDetailPane({
    mode: 'multi',
    multiCount: 14,
    meta: {
      'Total Size': '182 MB',
      'Total Duration': '01:14:32',
      'BPM Range': '124 - 130',
      'Keys': '8A, 5A, 9B'
    }
  }).element;

  const paneMissing = createDetailPane({
    mode: 'missing',
    title: 'Harder Better Faster Stronger',
    artist: 'Daft Punk'
  }).element;

  const detailPanes = el('div', {
    style: {
      display: 'grid',
      gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
      gap: '16px',
      width: '100%'
    }
  }, [
    el('div', { style: { display: 'flex', flexDirection: 'column', border: '1px solid var(--ff-border)' } }, [
      el('div', { style: { background: 'var(--ff-bg)', padding: '6px 10px', fontSize: '10px', color: 'var(--ff-muted)', borderBottom: '1px solid var(--ff-border)' }, text: 'EMPTY STATE' }),
      paneEmpty
    ]),
    el('div', { style: { display: 'flex', flexDirection: 'column', border: '1px solid var(--ff-border)' } }, [
      el('div', { style: { background: 'var(--ff-bg)', padding: '6px 10px', fontSize: '10px', color: 'var(--ff-muted)', borderBottom: '1px solid var(--ff-border)' }, text: 'SINGLE TRACK SELECTED' }),
      paneSingle
    ]),
    el('div', { style: { display: 'flex', flexDirection: 'column', border: '1px solid var(--ff-border)' } }, [
      el('div', { style: { background: 'var(--ff-bg)', padding: '6px 10px', fontSize: '10px', color: 'var(--ff-muted)', borderBottom: '1px solid var(--ff-border)' }, text: 'MULTI-TRACK SELECTION' }),
      paneMulti
    ]),
    el('div', { style: { display: 'flex', flexDirection: 'column', border: '1px solid var(--ff-border)' } }, [
      el('div', { style: { background: 'var(--ff-bg)', padding: '6px 10px', fontSize: '10px', color: 'var(--ff-muted)', borderBottom: '1px solid var(--ff-border)' }, text: 'TRACK FILE MISSING' }),
      paneMissing
    ])
  ]);

  // 2. Onboarding Card
  const onboarding = createOnboarding({
    onImport: () => alert('Import music library'),
    onOpenFolder: () => alert('Open folder')
  }).element;

  // 3. Settings Page Preference Toggles/Inputs
  const toggleAnalyze = createToggle({ on: true }).element;
  const toggleWaveform = createToggle({ on: true }).element;
  const directoryInput = createInput({ value: '/Volumes/PIONEER' }).element;
  directoryInput.style.width = '160px';

  const metadataFormat = createSegmentedControl({
    items: ['Color', 'Mono'],
    activeIndex: 0
  }).element;

  const toggleAutoSync = createToggle({ on: false }).element;
  const historySlider = createSlider({ min: 10, max: 100, value: 50 }).element;
  historySlider.style.width = '120px';

  const settingsPage = createSettingsPage({
    sections: [
      {
        label: 'General Preferences',
        key: 'general',
        items: [
          { title: 'Auto-analyze tracks', description: 'Automatically run BPM and Key detection on new imports.', control: toggleAnalyze },
          { title: 'High-quality waveform', description: 'Generate high-resolution spectral details for cue editing.', control: toggleWaveform },
          { title: 'Default export path', description: 'The folder path where Pioneer databases are synced.', control: directoryInput }
        ]
      },
      {
        label: 'Library Sync',
        key: 'sync',
        items: [
          { title: 'CDJ metadata format', description: 'Configure tag appearance for Rekordbox-compatible displays.', control: metadataFormat },
          { title: 'Auto-sync on USB mount', description: 'Detect USB drives and write database sync immediately.', control: toggleAutoSync },
          { title: 'Sync history limit', description: 'Number of backup snapshots to preserve on disk.', control: historySlider }
        ]
      }
    ]
  }).element;

  return el('div', {
    style: {
      display: 'flex',
      flexDirection: 'column',
      gap: 'var(--ff-space-4)',
      width: '740px',
      padding: 'var(--ff-space-7)',
      background: 'var(--ff-surface)',
      border: '1px solid var(--ff-border)',
      borderRadius: 'var(--ff-radius-md)',
    }
  }, [
    cap('DETAIL PANE MODES'),
    detailPanes,
    cap('ONBOARDING PAGE'),
    el('div', { style: { display: 'flex', justifyContent: 'center', width: '100%' } }, [onboarding]),
    cap('TABBED PREFERENCES SETTINGS PAGE (TAB SWITCHING ALIVE!)'),
    el('div', { style: { width: '100%' } }, [settingsPage])
  ]);
}
