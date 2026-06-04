<!--
  AppStatusBar.svelte — Bottom status bar module.
  Wraps the $ds StatusBar primitive with library store data and footer settings control.
-->
<script>
  import { onMount } from 'svelte';
  import { StatusBar, LevelMeter, Icon } from '$ds';
  import { library } from '../stores/library.svelte.ts';
  import { player, initPlayerTransport } from '../stores/player.svelte.ts';
  import { openSettings } from '../stores/ui.svelte.ts';

  onMount(() => {
    void initPlayerTransport();
  });

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
  >
    {#snippet rightExtras()}
      <LevelMeter left={player.levelLeft} right={player.levelRight} />
      <button
        type="button"
        class="ff-statusbar__settings"
        title="Settings"
        aria-label="Settings"
        onclick={openSettings}
      >
        <Icon name="settings" size={13} />
      </button>
    {/snippet}
  </StatusBar>
</div>

<style>
  .ff-statusbar-module {
    width: 100%;
    background: var(--ff-bg);
    border-top: 1px solid var(--ff-border);
  }

  .ff-statusbar__settings {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    margin: 0;
    border: none;
    border-radius: var(--ff-radius-xs);
    background: transparent;
    color: var(--ff-muted);
    cursor: pointer;
    outline: none;
    transition:
      color var(--ff-motion-fast) var(--ff-easing),
      background-color var(--ff-motion-fast) var(--ff-easing);
  }

  .ff-statusbar__settings:hover {
    color: var(--ff-text);
    background: var(--ff-elev);
  }

  .ff-statusbar__settings:focus-visible {
    color: var(--ff-text);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.3);
  }

  .ff-statusbar__settings:active {
    background: var(--ff-select);
    color: var(--ff-text);
  }
</style>
