<script>
  import './onboarding.css';
  import Button from './Button.svelte';

  let {
    onImport,
    onOpenFolder,
    disabled = false,
  } = $props();
</script>

<div class="ff-onboarding">
  <h1 class="ff-onboarding__wordmark">fourfour</h1>
  <p class="ff-onboarding__message">
    Import your music library, curate playlists, and sync to Pioneer CDJ-compatible USB drives — all
    without Rekordbox.
  </p>
  <div class="ff-onboarding__actions">
    <Button label="Import Library" variant="primary" {disabled} onclick={onImport} />
    <Button label="Open Folder…" variant="default" {disabled} onclick={onOpenFolder} />
  </div>
  <div class="ff-onboarding__hint">
    or press <kbd class="ff-onboarding__kbd">⌘O</kbd> to open a folder
  </div>
</div>
