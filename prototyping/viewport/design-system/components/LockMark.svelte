<script>
  // LockMark: shows that a value is hand-set ("user-edited") and kept across
  // re-analysis. Closed = locked, open = analysis may replace it. A click toggles.
  //   locked: true | false
  //   label: optional text after the glyph (e.g. "Yours"); omit for the glyph alone
  //   state: 'rest' | 'hover' (showcase)
  import './lock-mark.css';
  import Icon from './Icon.svelte';

  let { locked = true, label = '', state = 'rest', size = 11, title = '', onToggle = undefined } = $props();

  let tip = $derived(
    title || (locked ? 'Hand-set. Re-analysis keeps it. Click to hand it back to analysis.' : 'Analysis may replace this. Click to keep it.')
  );
</script>

<button
  type="button"
  class="ff-lock{locked ? '' : ' ff-lock--open'}{label ? '' : ' ff-lock--icon'}{state === 'hover' ? ' ff-lock--hover' : ''}"
  title={tip}
  aria-label={tip}
  aria-pressed={locked}
  onclick={() => onToggle?.(!locked)}>
  <Icon name={locked ? 'lock' : 'lock-open'} {size} />
  {#if label}<span>{label}</span>{/if}
</button>
