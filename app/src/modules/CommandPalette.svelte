<!--
  CommandPalette.svelte — the dropdown under the top bar search field (⌘F, ⌘K, ⌥↩): tracks,
  playlists/sticks to jump to, and every command. A leading `>` narrows it to commands.
  Tracks use the same matching as the table filter. Actions come from the shortcut registry, so labels and shortcuts never drift.
  Settings rows carry their control inline and keep the palette open while it is changed.
-->
<script>
  import { tick } from 'svelte';
  import { CommandPalette } from '$ds';
  import { ui, closePalette } from '../stores/ui.svelte.ts';
  import { library, filterTracks } from '../stores/library.svelte.ts';
  import { loadAppSettings } from '../stores/app-settings.svelte.ts';
  import { buildSettingSources } from '../stores/settings-rows.ts';
  import { shortcuts } from '../shortcuts/index.ts';
  import { formatBinding } from '../shortcuts/keybinding.ts';
  import { goToSource, jumpToTrack } from '../shortcuts/navigation.ts';
  import { buildPaletteGroups, isCommandQuery } from '../shortcuts/palette-model.ts';

  const MIN_DROPDOWN_WIDTH = 520;
  const WINDOW_MARGIN = 12;

  let anchorRect = $state(null);
  let keySource = $derived(ui.searchAnchor?.querySelector('input') ?? null);

  function measureAnchor() {
    const rect = ui.searchAnchor?.getBoundingClientRect();
    if (!rect) {
      anchorRect = null;
      return;
    }
    const width = Math.min(Math.max(rect.width, MIN_DROPDOWN_WIDTH), window.innerWidth - rect.left - WINDOW_MARGIN);
    anchorRect = { left: rect.left, top: rect.bottom + 4, width };
  }

  $effect(() => {
    if (!ui.paletteOpen) return;
    measureAnchor();
    void loadAppSettings();
    window.addEventListener('resize', measureAnchor);
    return () => window.removeEventListener('resize', measureAnchor);
  });

  $effect(() => {
    if (!ui.paletteOpen) return;
    function onOutsidePointerDown(e) {
      if (ui.searchAnchor?.contains(e.target) || e.target.closest?.('.ff-cp--anchored')) return;
      closePalette();
    }
    window.addEventListener('pointerdown', onOutsidePointerDown, true);
    return () => window.removeEventListener('pointerdown', onOutsidePointerDown, true);
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
      tracks: (isCommandQuery(ui.searchText) ? [] : filterTracks(library.tracks, ui.searchText)).slice(0, 8).map((t) => ({
        id: t.id,
        title: t.title ?? '',
        artist: t.artist ?? '',
        run: () => void jumpToTrack(t.id),
      })),
    };
  });

  let model = $derived(buildPaletteGroups(ui.searchText, sources));

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
  anchored
  open={ui.paletteOpen}
  query={ui.searchText}
  {anchorRect}
  {keySource}
  groups={model}
  {onRun}
  onChoose={(item, value) => model.flatMap((g) => g.items).find((e) => e.id === item.id)?.choose?.(value)}
  onClose={closePalette}
/>
