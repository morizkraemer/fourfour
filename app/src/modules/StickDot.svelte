<script>
  let { tone = 'none', label = '' } = $props();
</script>

{#if tone !== 'none'}
  <span class="ff-stickdot ff-stickdot--{tone}" data-stick-dot={tone} role="img" aria-label={label} title={label}></span>
{/if}

<style>
  .ff-stickdot {
    position: relative;
    display: inline-block;
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ff-status-warn);
  }
  .ff-stickdot--problem {
    background: var(--ff-danger);
  }
  .ff-stickdot--busy::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: var(--ff-status-warn);
    animation: ff-stickdot-pulse 1.4s ease-out infinite;
  }
  @keyframes ff-stickdot-pulse {
    from {
      opacity: 0.7;
      transform: scale(1);
    }
    to {
      opacity: 0;
      transform: scale(3);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ff-stickdot--busy::after {
      animation: none;
      opacity: 0.35;
      transform: scale(2);
    }
  }
</style>
