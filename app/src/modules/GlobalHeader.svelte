<!--
  GlobalHeader.svelte — Top header bar with search field.
  Matches the v3 artboard: centered search box with ⌘F shortcut.
-->
<script>
  import { Icon, Kbd } from '$ds';
</script>

<div class="ff-gheader">
  <div class="ff-gheader__search">
    <Icon name="search" size={12} />
    <span class="ff-gheader__placeholder">Search tracks, artists, labels…</span>
    <Kbd key="⌘F" />
  </div>
</div>

<style>
  .ff-gheader {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    width: 100%;
    padding: var(--ff-space-4) var(--ff-space-7);
    background: var(--ff-bg);
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-gheader__search {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    width: 420px;
    height: 28px;
    padding: 0 10px;
    background: var(--ff-surface);
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-md);
    color: var(--ff-muted);
  }
  .ff-gheader__placeholder {
    flex: 1 1 0;
    font-family: var(--ff-font);
    font-size: var(--ff-type-compact);
    color: var(--ff-faint);
  }
</style>
