<script>
  // Spinner (Svelte port of spinner.js).
  //   size: 'default' | 'small'
  import './spinner.css';
  let { size = 'default', class: className = '' } = $props();
</script>

<div
  class={['ff-spinner', size === 'small' && 'ff-spinner--small', className]
    .filter(Boolean)
    .join(' ')}
  role="status"
  aria-label="Loading">
</div>
