<script>
  // Progress bar (Svelte port of progress-bar.js).
  //   value: 0-100 (ignored for indeterminate)
  //   variant: 'determinate' | 'indeterminate' | 'thin'
  import './progress-bar.css';

  let { value = 0, variant = 'determinate' } = $props();

  let isThin = $derived(variant === 'thin');
  let isIndeterminate = $derived(variant === 'indeterminate');
  let clamped = $derived(Math.min(100, Math.max(0, value)));
  let cls = $derived(
    [
      'ff-progress-bar',
      isThin && 'ff-progress-bar--thin',
      isIndeterminate ? 'ff-progress-bar--indeterminate' : 'ff-progress-bar--determinate',
    ]
      .filter(Boolean)
      .join(' ')
  );
</script>

<div
  class={cls}
  role="progressbar"
  aria-valuenow={isIndeterminate ? undefined : value}
  aria-valuemin="0"
  aria-valuemax="100">
  <div class="ff-progress-bar__fill" style={isIndeterminate ? undefined : `width:${clamped}%`}></div>
</div>
