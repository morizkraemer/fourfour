<script>
  import Win from './Win.svelte';
  import Frame from './Frame.svelte';
  import W2Top from './W2Top.svelte';
  import W2InspNow from './W2InspNow.svelte';
  import W3InspA from './W3InspA.svelte';
  import W3InspB from './W3InspB.svelte';
  import W3InspC from './W3InspC.svelte';

  let { variant = 'now' } = $props();

  const META = {
    now: {
      tag: 'NOW',
      title: 'Inspector today (278 px, right of the list)',
      model: 'Real DetailPane. Read-only except BPM and key (ScrubValue) and the cue chips.',
      good: [
        'BPM and key scrub in place; a BPM change never moves the grid',
        'Cue chips jump, rename and recolour',
      ],
      cost: [
        'Most tags cannot be edited and several are not shown; nothing says BPM or key were hand-set',
        'The 290 px cover pushes the data below the fold',
      ],
    },
    a: {
      tag: 'A',
      title: 'Document: one quiet column',
      model: 'Cover, title and artist first. Then BPM, key and length as three figures with air around them. One line says what is hand-set. Tags are plain label and value rows, cues a short list, the file path last.',
      good: [
        'Reads top to bottom in the order of importance; nothing hides, nothing is boxed',
        'Every field is visible and one click from editable, so no mode or fold to learn',
        'Smallest build: ghost fields and the real ScrubValue only',
      ],
      cost: [
        'Everything is on screen at once, so it is the longest of the three; comment and file sit at the fold on a short window',
        'No picture of the track; cue times are typed',
      ],
    },
    b: {
      tag: 'B',
      title: 'Hero and folds',
      model: 'Cover as a hero with title and artist on it. BPM, key and length in one strip with a single padlock. Below, three folds: Cues open, Details and File closed with a one-line summary each. Click a fold to edit its fields.',
      good: [
        'Calmest resting state: three blocks and a handful of words; fits any window height',
        'Summaries (genre, label, year) answer most glances without opening anything',
        'Cover gets real size, which helps recognising a track',
      ],
      cost: [
        'Rating, colour, album and comment are one click further away than in A',
        'The hero costs 168 px of height that could carry data; text over artwork needs the dark gradient',
      ],
    },
    c: {
      tag: 'C',
      title: 'Waveform spine',
      model: 'Small cover and title on top, BPM, key and length next. The track itself, a waveform with cue flags, is the middle of the panel with the cue list directly under it. Tags collapse into one quiet line (genre, label, year), then album, stars, colour and comment.',
      good: [
        'Cues are seen where they are, and the list under the picture explains the flags',
        'Tags take three lines instead of a form; the panel stays short',
        'One padlock covers grid and cues, next to the waveform that shows them',
      ],
      cost: [
        'Tags in a line are harder to scan and to hit than a labelled list',
        'Needs a waveform strip in the side panel (the same strip the list preview and DetailPane already draw)',
      ],
    },
  };
  const m = $derived(META[variant]);
</script>

<Frame tag={m.tag} title={m.title} model={m.model} good={m.good} cost={m.cost}>
  <Win cols="mid">
    {#snippet top()}<W2Top />{/snippet}
    {#snippet right()}
      {#if variant === 'now'}<W2InspNow width={278} />
      {:else if variant === 'a'}<W3InspA width={340} />
      {:else if variant === 'b'}<W3InspB width={340} />
      {:else}<W3InspC width={340} />{/if}
    {/snippet}
  </Win>
</Frame>
