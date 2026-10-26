<script>
  import { Icon } from '../../../../viewport/design-system/svelte.js';
  import Win from './Win.svelte';
  import Frame from './Frame.svelte';
  import NowRight from './NowRight.svelte';
  import InspA from './InspA.svelte';
  import InspB from './InspB.svelte';
  import InspC from './InspC.svelte';
  import { TRACKS } from './data.js';

  let { variant = 'now' } = $props();
  const rail = [TRACKS[2], TRACKS[6], TRACKS[10], TRACKS[3]];
  const META = {
    now: {
      tag: 'NOW',
      title: 'Inspector today',
      model: 'Read-only summary: big cover, strip waveform, label/value list, hot cue chips, file path.',
      good: ['Quick glance at a track without opening anything'],
      cost: [
        'Nothing is editable: title, artist, album, label, BPM, key, rating, colour, comment all need other routes',
        'The cover takes 290px of a ~700px panel; the waveform repeats the player below',
        'Hot cue chips truncate to "Ho…", cues cannot be renamed, recoloured or moved',
        'No rating, colour, comment or genre at all; the file path is the last thing you see',
      ],
    },
    a: {
      tag: 'A',
      title: 'Live form panel',
      model: 'Every value is a ghost input: it looks like text, shows a pencil on hover, becomes an input on click. Same right-hand panel, rebuilt as a form.',
      good: [
        'Hover → pencil, click → caret, Enter/Tab commits, Esc reverts; autocomplete for genre and label (shown open)',
        'BPM and key are scrub fields: drag sideways (ruler + delta bubble, shown mid-drag), ↑↓, or type; ½ ×2 re-detect under BPM',
        'Smallest change from today; everything fits one scrolling column, cues editable as a list',
      ],
      cost: [
        'Narrow: cues are a list without a waveform, so moving a cue means typing a time or nudging by a beat',
        'Long column: the file section and cues fall off the bottom on a 768px-high window',
      ],
    },
    b: {
      tag: 'B',
      title: 'Tabbed panel, cues on the waveform',
      model: 'Identity, BPM and key always on top; Info, Cues, Grid and File as tabs. The Cues tab is a mini waveform you edit directly.',
      good: [
        'Cues tab: drag flags on the waveform (tooltip shows time and beat, snaps to the grid, ⌥ = free), rename in place, colour popover, nudge ± one beat',
        'Always-visible BPM, key and rating, the numbers a DJ checks most; the rest is one tab away',
        'Room to grow: Grid tab for beat grid fixes, File tab for relocate and re-analyze',
      ],
      cost: [
        'Fields you edit rarely (album, year, label, comment) hide behind a tab',
        'Two waveforms near each other (this one and the player): cue editing duplicates the expanded player',
      ],
    },
    c: {
      tag: 'C',
      title: 'Wide editor strip',
      model: 'The inspector moves under the table and spreads out: all fields, the key wheel and a full-width cue lane on one screen with no tabs and no scrolling.',
      good: [
        'Key is chosen on a Camelot wheel (hover shows compatible keys), BPM is a big scrub field',
        'Cue lane: flags on a full-width waveform, selected cue gets name, time, beat nudges, "set to playhead"',
        'Everything visible at once; right side stays free for the board',
      ],
      cost: [
        'Takes ~310px of table height; needs a collapse chevron and ⌘I',
        'Multi-select (batch edit) needs its own design: fields would show "Mixed"',
      ],
    },
  };
  const m = $derived(META[variant]);
</script>

<Frame tag={m.tag} title={m.title} model={m.model} good={m.good} cost={m.cost}>
  {#if variant === 'now'}
    <Win cols="mid">
      {#snippet right()}<NowRight mode="inspector" />{/snippet}
    </Win>
  {:else if variant === 'a'}
    <Win cols="mid">
      {#snippet right()}<InspA width={360} />{/snippet}
    </Win>
  {:else if variant === 'b'}
    <Win cols="mid">
      {#snippet right()}<InspB width={360} />{/snippet}
    </Win>
  {:else}
    <Win cols="wide">
      {#snippet below()}<InspC laneWidth={1126} />{/snippet}
      {#snippet right()}
        <div class="rail">
          <Icon name="chevron-left" size={16} />
          {#each rail as r}<span class="rail__t" style="background-image:url('{r.rowCover}')"></span>{/each}
        </div>
      {/snippet}
    </Win>
  {/if}
</Frame>

<style>
  .rail { flex: none; width: 46px; height: 100%; display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 10px 0; color: var(--ff-text-mid); background: var(--ff-bg); border-left: 1px solid var(--ff-border); }
  .rail__t { width: 30px; height: 30px; border-radius: 6px; background-size: cover; border: 1px solid var(--ff-border-hi); }
</style>
