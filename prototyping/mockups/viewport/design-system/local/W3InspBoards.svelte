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
      title: 'Inspector as built (340 px, right of the list)',
      model: 'The real DetailPane after Merlin picked A: read-only today for title, artist, album and genre until a metadata write command exists; BPM and key scrub, cues edit in place.',
      good: [
        'Same component as option A, fed by the app mock instead of the board mock',
        'Hand-set line appears only when BPM or key were set by hand',
      ],
      cost: [
        'Label, year, comment, rating and colour have no data in the app yet, so no rows',
      ],
    },
    a: {
      tag: 'A',
      title: 'Document: one quiet column (chosen, with changes)',
      model: 'Cover and title first. BPM, key and length as three dark cards. One line says what is hand-set. Then the waveform with cue markers (letters for hot cues, thin marks for memory cues), a compact cue list, plain label and value rows, the file path last. This board is the real DetailPane; editing is switched on here (title, artist, album, genre). Click the waveform to seek, a marker to jump.',
      good: [
        'The track itself is in the panel, and cues are seen where they are',
        'Cards give the three DJ figures weight; everything else stays quiet text',
        'Every row is plain text until touched, then editable',
      ],
      cost: [
        'Needs 340 px: three cards with BPM at two decimals do not fit in 278',
        'Tag editing needs a metadata write command that does not exist yet, so the app shows these rows read-only',
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
      {#if variant === 'now'}<W2InspNow width={340} />
      {:else if variant === 'a'}<W3InspA width={340} />
      {:else if variant === 'b'}<W3InspB width={340} />
      {:else}<W3InspC width={340} />{/if}
    {/snippet}
  </Win>
</Frame>
