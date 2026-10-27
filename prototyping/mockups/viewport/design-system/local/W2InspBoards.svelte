<script>
  import Win from './Win.svelte';
  import Frame from './Frame.svelte';
  import W2Top from './W2Top.svelte';
  import W2InspNow from './W2InspNow.svelte';
  import W2InspA from './W2InspA.svelte';
  import W2InspB from './W2InspB.svelte';
  import W2InspC from './W2InspC.svelte';

  let { variant = 'now' } = $props();

  const META = {
    now: {
      tag: 'NOW',
      title: 'Inspector today (278 px, right of the list)',
      model: 'Real DetailPane. Read-only except BPM and key (ScrubValue: drag, wheel, type, right-click menu) and the hot / memory cue chips (rename, recolour, delete in a small editor).',
      good: [
        'BPM and key scrub in place; the grid refit is an explicit menu item, so a BPM change never moves the grid',
        'Cue chips jump, rename and recolour; "+" sets a cue at the playhead of the loaded track',
        'Waveform strip shows where the cues sit',
      ],
      cost: [
        'Title, artist, album, label, genre, year, rating, colour and comment cannot be edited; genre, year, rating, colour and comment do not appear at all',
        'Nothing shows that BPM or key were hand-set; the only trace is a right-click item "Let re-analysis replace BPM and key"',
        'No visible lock for grid or cues, no way to edit a cue time, grid actions live only in a context menu',
        'The 290 px cover pushes the data below the fold on a short window; the file path comes last',
      ],
    },
    a: {
      tag: 'A',
      title: 'Form column: every value is a ghost field',
      model: 'One scrolling column in sections (identity, tags, tempo and key, grid, cues, file). Text looks like text; hover shows a pencil, click makes it an input, Enter commits, Esc reverts. Shown: genre being edited with suggestions, the "Break" cue hovered.',
      good: [
        'Everything is where you expect it and editable in place; nothing hides behind a mode, tab or popover',
        'Lock sits on the value it protects: BPM "Yours" with the analysed value below and "Use"; key and grid say "Analysed"; cues carry one lock',
        'Cues are rows with name and time as fields: rename or retime by typing',
      ],
      cost: [
        'Long: grid, cues and file fall below the fold on a 900 px window (fade at the bottom); tags you rarely touch sit above the cues you use every set',
        'Cue times are typed, not dragged; there is no picture of the track, so cue and grid work happens blind',
      ],
    },
    b: {
      tag: 'B',
      title: 'Tiles and focused editors',
      model: 'Compact read-first card. Facts are tiles: click one to open the editor that fits it (key wheel, grid menu, cue editor). Short text fields stay inline. Shown: the key tile open with the Camelot wheel.',
      good: [
        'Fits a 900 px window without scrolling: identity, waveform, six tiles, short fields, cue chips',
        'Each value gets the right editor: wheel for key (compatible keys lit), menu for grid actions, scrub for BPM',
        'Lock is a small padlock on each tile: closed = yours, open (dashed) = analysed; one click flips it',
      ],
      cost: [
        'Two clicks to edit key, grid or a cue (open, then act); slower than typing for a DJ who fixes many tracks',
        'Popovers hide the fields below them; many small editors to build and keep consistent',
      ],
    },
    'b-cue': {
      tag: 'B',
      title: 'Tiles: cue editor open',
      model: 'A cue chip opens the DS CueEditor (name, 16 colours, delete) plus a time row: type a time, nudge by one beat, or set to the playhead.',
      good: [
        'Cue time is editable at last (today it is not); beat nudges keep it on the grid',
        'Colour and name use the editor the app already has',
      ],
      cost: [
        'The cue list grows downward; with 8 hot cues and memory cues the editor pushes the file path off screen',
        'Still no waveform to see what a nudge does',
      ],
    },
    c: {
      tag: 'C',
      title: 'Two jobs: Prep and Tags (Prep shown)',
      model: 'A two-way switch at the top of the panel. Prep = tempo, key, grid and cues around a waveform: overview strip plus a zoomed lane (16 beats) where the selected cue is dragged. Tags = every text field (next board). Panel is 380 px.',
      good: [
        'Cue and grid work happens on a waveform: drag the flag, it snaps to beats, beat nudges, "To playhead"; the tooltip shows time and bar',
        'Lock gets room: BPM shows "Analysed 125.00" next to yours with "Use" to hand it back; key, grid and cues each carry Yours / Analysed',
        'Tags tab is a calm form that is never in the way during prep',
      ],
      cost: [
        'Two modes to remember (remembers the last one per session); tags and prep are never visible together',
        'Needs a zoomable waveform in a side panel: new work, and it overlaps what the expanded player will do',
      ],
    },
    'c-tags': {
      tag: 'C',
      title: 'Prep and Tags: Tags shown',
      model: 'The Tags side: cover, title, artist, album, genre, label, year, rating, colour, comment, then the read-only file block with its actions. No scrolling at 900 px.',
      good: [
        'Label column plus ghost fields reads like a form but edits like text',
        'File facts and actions (reveal, locate, re-analyse) live here, out of the way',
        'The note says where edits go: library only, never the audio file',
      ],
      cost: [
        'Rating and colour need the hand: they are not on the keyboard here (rows can take 1-5 and 0 when focused)',
        'Multi-select: fields show "Mixed", Prep is disabled; needs its own empty state',
      ],
    },
  };
  const m = $derived(META[variant]);
</script>

<Frame tag={m.tag} title={m.title} model={m.model} good={m.good} cost={m.cost}>
  {#if variant === 'now'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet right()}<W2InspNow width={340} />{/snippet}
    </Win>
  {:else if variant === 'a'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet right()}<W2InspA width={340} />{/snippet}
    </Win>
  {:else if variant === 'b'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet right()}<W2InspB width={340} popover="key" />{/snippet}
    </Win>
  {:else if variant === 'b-cue'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet right()}<W2InspB width={340} popover="cue" />{/snippet}
    </Win>
  {:else if variant === 'c'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet right()}<W2InspC width={380} tab="prep" />{/snippet}
    </Win>
  {:else if variant === 'c-tags'}
    <Win cols="mid">
      {#snippet top()}<W2Top />{/snippet}
      {#snippet right()}<W2InspC width={380} tab="tags" />{/snippet}
    </Win>
  {/if}
</Frame>
