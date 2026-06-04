<script>
  // Status dot (Svelte port of status-dot.js). 6px circle on USB device rows.
  //   status: 'online' | 'warn' | 'offline'
  import './status-dot.css';
  let { status = 'online' } = $props();
</script>

<span class="ff-status-dot ff-status-dot--{status}"></span>
