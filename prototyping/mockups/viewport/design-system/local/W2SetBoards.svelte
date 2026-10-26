<script>
  import Win from './Win.svelte';
  import Frame from './Frame.svelte';
  import W2Top from './W2Top.svelte';
  import W2SetNow from './W2SetNow.svelte';
  import W2SetA from './W2SetA.svelte';
  import W2SetB from './W2SetB.svelte';
  import W2SetC from './W2SetC.svelte';

  let { variant = 'now' } = $props();

  const NOW_COST = [
    'A full-window takeover: the library, the player and the music disappear while you change audio settings',
    'One 680 px column hugging the left; every row is a label plus a two-line description, so 4 settings fill a screen',
    'No analysis section: "analyze new tracks" (config.json, default on) and the engine and worker count have no UI; after an import a dialog asks every time',
    'Library, import, version, audio, shortcuts and a destructive reset all have equal weight; shortcuts is one 42-row list with no search',
  ];

  const META = {
    'now-general': {
      tag: 'NOW',
      title: 'Settings today: General',
      model: 'Real SettingsPage (fullscreen) with the app\'s own fields: library database path + Change, Import from Rekordbox, app version. Opens from the status bar gear or ⌘,.',
      good: ['Familiar two-pane preferences; Back and Esc close it', 'Section nav: General, Audio, Keyboard shortcuts, Debug'],
      cost: NOW_COST,
    },
    'now-audio': {
      tag: 'NOW',
      title: 'Settings today: Audio',
      model: 'Playback engine (CoreAudio is the only entry), output device, sample rate (3 choices), buffer size (5 choices). Changing any of them stops playback.',
      good: ['Real controls: Dropdown and SegmentedControl', 'Defaults are named in the description (48 kHz, 128)'],
      cost: [
        'A dropdown with one entry (engine) sits first; it is not a choice',
        'No level meter or test: you must close settings and press play to hear whether a change worked',
        'Sample rate and buffer are expert settings with the same weight as the output device',
      ],
    },
    'now-shortcuts': {
      tag: 'NOW',
      title: 'Settings today: Keyboard shortcuts',
      model: 'Every registry action by group; click the binding, press the new keys, Esc cancels, Backspace clears, conflicts ask Replace or Cancel. Cropped here; the real page scrolls through 42 rows (9 favorites, 16 hot-cue rows).',
      good: ['Click to record is quick; conflicts are explained, never silent', 'Reset per row and Reset all'],
      cost: [
        'Long single list, no search, no way to answer "what is on key X?"',
        'Hot cue A to H (set and delete) and favorites 1 to 9 take 25 of 42 rows for 3 ideas',
        'Rows are as tall as other settings with a description; the page reads like a manual',
      ],
    },
    'now-debug': {
      tag: 'NOW',
      title: 'Settings today: Debug',
      model: 'Library contents, IPC diagnostic, last error (when there is one) and Reset library database (destructive, behind a confirm dialog).',
      good: ['Reset asks for confirmation and says what it deletes'],
      cost: [
        'Debug mixes diagnostics with the one dangerous action; a user hunting for "where is my library" lands here',
        'Nothing says what is safe: the library path in General is next to it, but the reset text is only visible here',
      ],
    },
    a: {
      tag: 'A',
      title: 'Preferences sheet, organised by object (Analysis shown)',
      model: 'A centred sheet over the library (⌘, opens, Esc closes). The left list is by object, not by setting type, and each entry carries its live state. Pages are short: a live status card, then compact rows.',
      good: [
        'You see what each area is doing before you open it: "Running 120 / 340", "Scarlett 2i2 · 48 kHz"',
        'Analysis finally exists: import toggle, engine, workers, re-analyze (keeps your hand edits); the status card has Pause and Cancel',
        'Compact rows, one line of description; a dot marks values that differ from default',
      ],
      cost: [
        'Still a modal: you cannot hear an audio change and watch the player at once',
        'Five pages for about a dozen settings; shortcuts is still the long one (next board)',
      ],
    },
    'a-keys': {
      tag: 'A',
      title: 'Preferences sheet: Keyboard page',
      model: 'The same list, shorter: a filter, group chips and two columns of compact rows. One row is recording ("Press keys…"), one shows the conflict with Replace / Cancel.',
      good: [
        'Filter finds an action in two keystrokes; two columns halve the scrolling',
        'Same click-to-record model as today, so nothing to relearn',
      ],
      cost: [
        'Hot cue A to H and favorites 1 to 9 are still 25 of 42 rows unless they fold into one row each ("Hot cue A to H: ⇧1 to ⇧8")',
        'Still answers "what does this action use" but not "what is on this key"',
      ],
    },
    b: {
      tag: 'B',
      title: 'Settings live in the command palette',
      model: 'No settings window for the daily ones. ⌘K, type "analy": toggles, segmented controls and dropdowns appear as rows you can change right there, next to the commands that match. A small page remains for log and details.',
      good: [
        'Fastest path to any one setting: three keys, change, Esc; Space toggles, ←→ moves a segment',
        'Settings, commands and their shortcuts are searched together; the palette is already the app\'s map',
        'No new window to build: rows with a control on the right of the DS palette row',
      ],
      cost: [
        'Discoverability: you must know a setting exists; mitigated by an empty-query list of all settings and the status-bar gear opening the palette on "settings"',
        'No room for explanations or live feedback (progress, level meter); weak for a first look at everything',
      ],
    },
    'b-keys': {
      tag: 'B',
      title: 'Palette idea + keyboard map for shortcuts',
      model: 'Shortcuts get a picture: a keyboard layout where assigned keys show their action. Click a key to see and change it; the layers Plain, ⇧, ⌘ and ⌥⇧ switch what each key does. Opens from the palette ("Keyboard") or ⌘/.',
      good: [
        'Answers "what is on this key" and "what is free" at a glance; hot cues on ⇧1 to ⇧8 are one row of keys, not 16 list rows',
        'Conflicts become visible before you try (a key already lit)',
        'Can be combined with any option: it replaces the shortcuts list everywhere',
      ],
      cost: [
        'New, custom-built widget (the keyboard grid); a list stays necessary for actions with no key and for screen readers',
        'Layout depends on the keyboard (ANSI / ISO / German): needs a layout switch or follows the OS layout',
      ],
    },
    c: {
      tag: 'C',
      title: 'Settings as a side panel, next to the player',
      model: 'The right slot (where the inspector lives) turns into a Settings panel, opened by the status-bar gear. Accordion sections with a summary on each header. Non-modal: the list and the player stay live while you change things.',
      good: [
        'Change the buffer or output device and press play: you hear it with the library still on screen; a level meter and Test tone sit in Audio',
        'Collapsed sections show their state ("Scarlett 2i2 · 48 kHz · 128"), so most visits never expand anything',
        'Cheap to build: same panel slot, same width resize, no modal to manage',
      ],
      cost: [
        'Narrow (400 px): no descriptions, only short labels; long settings (shortcuts) need the keyboard map or a full sheet',
        'Replaces the inspector while open, so a track selection and settings are never visible together',
      ],
    },
  };
  const m = $derived(META[variant]);
</script>

<Frame tag={m.tag} title={m.title} model={m.model} good={m.good} cost={m.cost}>
  {#if variant === 'now-general'}
    <W2SetNow tab="general" />
  {:else if variant === 'now-audio'}
    <W2SetNow tab="audio" />
  {:else if variant === 'now-shortcuts'}
    <W2SetNow tab="shortcuts" />
  {:else if variant === 'now-debug'}
    <W2SetNow tab="debug" />
  {:else if variant === 'a'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet overlay()}<W2SetA page="analysis" />{/snippet}
    </Win>
  {:else if variant === 'a-keys'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet overlay()}<W2SetA page="keys" />{/snippet}
    </Win>
  {:else if variant === 'b'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet overlay()}<W2SetB page="palette" />{/snippet}
    </Win>
  {:else if variant === 'b-keys'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet overlay()}<W2SetB page="keys" />{/snippet}
    </Win>
  {:else if variant === 'c'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet right()}<W2SetC width={400} />{/snippet}
    </Win>
  {/if}
</Frame>
