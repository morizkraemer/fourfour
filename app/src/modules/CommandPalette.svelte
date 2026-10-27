<!--
  CommandPalette.svelte — ⌘K palette: actions, playlists/devices to jump to, and tracks.
  Actions come from the shortcut registry, so labels and shortcuts never drift.
  Settings rows carry their control inline and keep the palette open while it is changed.
-->
<script>
  import { tick } from 'svelte';
  import { CommandPalette } from '$ds';
  import { ui, closePalette } from '../stores/ui.svelte.ts';
  import { library } from '../stores/library.svelte.ts';
  import { loadAppSettings } from '../stores/app-settings.svelte.ts';
  import { buildSettingSources } from '../stores/settings-rows.ts';
  import { shortcuts } from '../shortcuts/index.ts';
  import { formatBinding } from '../shortcuts/keybinding.ts';
  import { goToSource, jumpToTrack } from '../shortcuts/navigation.ts';
  import { buildPaletteGroups } from '../shortcuts/palette-model.ts';

  let query = $state('');

  $effect(() => {
    if (!ui.paletteOpen) return;
    query = '';
    void loadAppSettings();
  });

  let sources = $derived.by(() => {
    if (!ui.paletteOpen) return { actions: [], settings: [], places: [], tracks: [] };
    return {
      actions: shortcuts.actions
        .filter((a) => a.palette !== false && (!a.when || a.when()))
        .map((a) => ({
          id: a.id,
          label: a.label,
          shortcut: formatBinding(shortcuts.bindingOf(a.id)),
          run: a.run,
        })),
      settings: buildSettingSources(),
      places: [
        { id: 'all', label: 'All Tracks', sub: 'library', icon: 'list', run: () => void goToSource('All Tracks') },
        { id: 'recent', label: 'Recently Added', sub: 'library', icon: 'clock-3', run: () => void goToSource('Recently Added') },
        ...library.playlists.map((p) => ({
          id: `playlist:${p.id}`,
          label: p.name,
          sub: 'playlist',
          icon: 'list',
          run: () => void goToSource(p.name),
        })),
        ...library.volumes.map((v) => {
          const name = v.replace(/\\/g, '/').split('/').pop() || v;
          return { id: `usb:${v}`, label: name, sub: 'USB', icon: 'disc', run: () => void goToSource(v, v) };
        }),
      ],
      tracks: library.tracks.map((t) => ({
        id: t.id,
        title: t.title ?? '',
        artist: t.artist ?? '',
        run: () => void jumpToTrack(t.id),
      })),
    };
  });

  let model = $derived(buildPaletteGroups(query, sources));

  async function onRun(item) {
    const entry = model.flatMap((g) => g.items).find((e) => e.id === item.id);
    if (entry?.keepOpen) {
      entry.run();
      return;
    }
    closePalette();
    await tick();
    entry?.run();
  }
</script>

<CommandPalette
  open={ui.paletteOpen}
  bind:query
  groups={model}
  {onRun}
  onChoose={(item, value) => model.flatMap((g) => g.items).find((e) => e.id === item.id)?.choose?.(value)}
  onClose={closePalette}
/>
