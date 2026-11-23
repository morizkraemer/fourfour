<script>
  let { tone = 'none', label = '' } = $props();
</script>

{#if tone === 'problem'}
  <svg class="ff-stickwarn" data-stick-dot="problem" viewBox="0 0 16 16" role="img" aria-label={label}>
    <path d="M8 2.2 14.3 13.2H1.7z" />
    <path d="M8 6.6v3.1M8 11.4v.1" />
  </svg>
{:else if tone !== 'none'}
  <span class="ff-stickdot ff-stickdot--{tone}" data-stick-dot={tone} role="img" aria-label={label} title={label}></span>
{/if}

<style>
  .ff-stickwarn {
    flex: none;
    width: 13px;
    height: 13px;
    fill: none;
    stroke: var(--ff-status-warn);
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .ff-stickdot {
    position: relative;
    display: inline-block;
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ff-status-warn);
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
