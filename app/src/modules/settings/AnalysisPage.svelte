<script>
  import { SettingRow, Switch, SegmentedControl, Button, ProgressBar, Spinner } from '$ds';
  import { library } from '../../stores/library.svelte.ts';
  import {
    appSettings,
    ANALYSIS_ENGINE_CHOICES,
    setAutoAnalyzeNew,
    setAnalysisEngine,
  } from '../../stores/app-settings.svelte.ts';
  import { cancelAnalysis } from '../../services/tauri.svelte.ts';

  let progress = $derived(library.analysisProgress);
  let percent = $derived(progress.total > 0 ? (progress.current / progress.total) * 100 : 0);
  let unanalyzed = $derived(library.tracks.filter((t) => !t.analyzed || t.analysisPending).length);
  let engineIndex = $derived(
    Math.max(0, ANALYSIS_ENGINE_CHOICES.findIndex((c) => c.value === appSettings.analysisEngine)),
  );
  let cores = $derived(typeof navigator === 'undefined' ? 1 : navigator.hardwareConcurrency || 1);
</script>

<div class="ff-settings-card ff-settings-card--status" data-analysis-status>
  {#if library.analyzing}
    <div class="live">
      <Spinner size="small" />
      <b>Analyzing {progress.current} of {progress.total}</b>
      <div class="live__buttons">
        <Button label="Cancel" size="small" variant="ghost" onclick={() => void cancelAnalysis()} />
      </div>
    </div>
    <ProgressBar value={percent} />
  {:else}
    <div class="live">
      <b>Nothing running</b>
      <span>{unanalyzed} of {library.trackCount} tracks still to analyze</span>
    </div>
  {/if}
</div>

<div class="ff-settings-section">When tracks arrive</div>
<div class="ff-settings-card">
  <SettingRow
    title="Analyze new tracks"
    description="Beat grid, key and waveform start by themselves for tracks never analyzed. Off: nothing runs until you choose Analyze."
  >
    {#snippet control()}
      <Switch
        label="Analyze new tracks"
        on={appSettings.autoAnalyzeNew}
        onChange={(on) => void setAutoAnalyzeNew(on)}
      />
    {/snippet}
  </SettingRow>
</div>

<div class="ff-settings-section">Engine</div>
<div class="ff-settings-card">
  <SettingRow
    title="Analysis engine"
    description="Automatic uses the built-in engine when its model files are installed, otherwise Python. Applies to the next run."
  >
    {#snippet control()}
      <SegmentedControl
        items={ANALYSIS_ENGINE_CHOICES.map((c) => c.label)}
        activeIndex={engineIndex}
        onChange={(index) => void setAnalysisEngine(ANALYSIS_ENGINE_CHOICES[index].value)}
      />
    {/snippet}
  </SettingRow>
  <SettingRow
    title="Parallel workers"
    description="Fixed at launch: all cores but one. Override with the FOURFOUR_ANALYSIS_THREADS environment variable."
  >
    {#snippet control()}
      <span class="value">Auto · {Math.max(1, cores - 1)} of {cores} cores</span>
    {/snippet}
  </SettingRow>
</div>

<style>
  .live {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--ff-font);
    font-size: 13px;
  }

  .live span {
    font-size: 12px;
    color: var(--ff-muted);
  }

  .live__buttons {
    margin-left: auto;
    display: flex;
    gap: 6px;
  }

  .value {
    font-family: var(--ff-font);
    font-size: 12px;
    color: var(--ff-text-mid);
    white-space: nowrap;
  }
</style>
