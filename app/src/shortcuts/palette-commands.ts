import { library, ejectVolume } from '../stores/library.svelte.ts';
import { isStickSyncBusy, syncStick } from '../stores/stick-sync.svelte.ts';
import { dockOrganizeTarget, ui } from '../stores/ui.svelte.ts';
import { ALL_COLUMN_KEYS, getLayout, resolveLayoutKind, resetColumns, toggleColumnVisibility } from '../stores/table-columns.svelte.ts';
import { stickLabel } from '../lib/topbarStatus.ts';
import type { PaletteActionSource } from './palette-model.ts';

const STICK_WORDS = ['usb', 'drive', 'device'];

function stickCommands(volume: string): PaletteActionSource[] {
  const name = stickLabel(volume);
  const canSync = !library.syncing && !isStickSyncBusy();
  const sync: PaletteActionSource[] = canSync
    ? [
        {
          id: `stick.sync:${volume}`,
          label: `Sync stick ${name}`,
          shortcut: '',
          keywords: ['export', 'write', ...STICK_WORDS],
          run: () => void syncStick(volume),
        },
      ]
    : [];
  return [
    ...sync,
    {
      id: `stick.board:${volume}`,
      label: `Show stick board ${name}`,
      shortcut: '',
      keywords: ['open', 'contents', 'panel', ...STICK_WORDS],
      run: () => dockOrganizeTarget({ kind: 'usb', id: volume, label: name }),
    },
    {
      id: `stick.eject:${volume}`,
      label: `Eject ${name}`,
      shortcut: '',
      keywords: ['unmount', 'remove', ...STICK_WORDS],
      run: () => void ejectVolume(volume),
    },
  ];
}

function columnCommands(): PaletteActionSource[] {
  const kind = resolveLayoutKind(ui.activeSidebarRow);
  const hidden = new Map(getLayout(kind).columns.map((c) => [c.key, c.hidden === true]));
  const toggles = ALL_COLUMN_KEYS.map(({ key, label }): PaletteActionSource => {
    const isHidden = hidden.get(key) ?? true;
    return {
      id: `column.toggle:${key}`,
      label: `${isHidden ? 'Show' : 'Hide'} column ${label}`,
      shortcut: '',
      keywords: ['table', 'columns', 'toggle', 'field'],
      run: () => toggleColumnVisibility(kind, key),
    };
  });
  return [
    ...toggles,
    {
      id: 'column.reset',
      label: 'Reset table columns',
      shortcut: '',
      keywords: ['table', 'default', 'layout', 'width'],
      run: () => resetColumns(kind),
    },
  ];
}

/** Commands whose rows depend on what is mounted or which list is open. */
export function buildDynamicCommands(): PaletteActionSource[] {
  return [...library.volumes.flatMap(stickCommands), ...columnCommands()];
}
