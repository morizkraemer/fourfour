<!--
  AppStatusBar.svelte — Bottom status bar module.
  Wraps the $ds StatusBar primitive with library store data.
-->
<script>
  import { StatusBar } from '$ds';
  import { library } from '../stores/library.svelte.ts';

  let showSpinner = $derived(library.analyzing || library.syncing);
  
  let progress = $derived(
    library.analyzing && library.analysisProgress.total > 0
      ? (library.analysisProgress.current / library.analysisProgress.total) * 100
      : undefined
  );

  let versionStr = $derived(library.appVersion ? `v${library.appVersion}` : '');
</script>

<div class="ff-statusbar-module">
  <StatusBar
    message={library.statusMessage}
    context={versionStr}
    count="{library.trackCount} tracks"
    {showSpinner}
    {progress}
  />
</div>

<style>
  .ff-statusbar-module {
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }
</style>
