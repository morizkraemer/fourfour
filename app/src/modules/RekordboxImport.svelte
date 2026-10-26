<!--
  RekordboxImport.svelte — Import tracks, playlists, grids and cues from a
  Rekordbox library (desktop master.db, USB stick, or rekordbox.xml).
  Opened from Settings → Library.
-->
<script>
  import { Dialog, Button, SegmentedControl, ProgressBar } from '$ds';
  import {
    rekordboxImport,
    REKORDBOX_SOURCES,
    closeRekordboxImport,
    setRekordboxKind,
    setRekordboxIncludeMissing,
    chooseRekordboxSource,
    chooseRekordboxMusicRoot,
    scanRekordboxSource,
    runRekordboxImport,
  } from '../stores/rekordbox-import.svelte.ts';

  const MISSING_CHOICES = ['Skip missing files', 'Keep missing files'];

  let kindIndex = $derived(REKORDBOX_SOURCES.findIndex((s) => s.kind === rekordboxImport.kind));
  let busy = $derived(rekordboxImport.phase === 'scanning' || rekordboxImport.phase === 'importing');
  let needsMusicRoot = $derived(rekordboxImport.kind !== 'usb');
  let preview = $derived(rekordboxImport.preview);
  let report = $derived(rekordboxImport.report);
  let canImport = $derived(rekordboxImport.phase === 'ready' && !!preview && preview.tracks > 0);
  let percent = $derived(
    rekordboxImport.progress && rekordboxImport.progress.total > 0
      ? (rekordboxImport.progress.done / rekordboxImport.progress.total) * 100
      : 0,
  );
  let sourceHint = $derived(
    rekordboxImport.kind === 'master_db'
      ? 'Choose master.db from your Rekordbox folder.'
      : rekordboxImport.kind === 'usb'
        ? 'Choose the root folder of the stick.'
        : 'Choose a rekordbox.xml export.',
  );

  function onPrefixKeydown(e) {
    if (e.key === 'Enter') void scanRekordboxSource();
  }
</script>

<Dialog
  open={rekordboxImport.open}
  title="Import from Rekordbox"
  variant="wide"
  onClose={closeRekordboxImport}
>
  {#snippet body()}
    <div class="ff-rbimport">
      {#if rekordboxImport.phase === 'done' && report}
        <p class="ff-rbimport__lead">Import finished.</p>
        <dl class="ff-rbimport__stats">
          <dt>Tracks added</dt><dd>{report.tracks_imported}</dd>
          <dt>Tracks merged</dt><dd>{report.tracks_merged}</dd>
          <dt>Skipped (file missing)</dt><dd>{report.tracks_skipped_missing}</dd>
          <dt>Playlists added / merged</dt><dd>{report.playlists_imported} / {report.playlists_merged}</dd>
          <dt>Beat grids</dt><dd>{report.grids_imported}</dd>
          <dt>Hot / memory cues</dt><dd>{report.hot_cues_imported} / {report.memory_cues_imported}</dd>
        </dl>
      {:else}
        <SegmentedControl
          items={REKORDBOX_SOURCES.map((s) => s.label)}
          activeIndex={kindIndex}
          disabled={busy}
          onChange={(i) => setRekordboxKind(REKORDBOX_SOURCES[i].kind)}
        />

        <div class="ff-rbimport__row">
          <span class="ff-rbimport__path" title={rekordboxImport.path}>
            {rekordboxImport.path || sourceHint}
          </span>
          <Button label="Choose…" size="small" disabled={busy} onclick={() => void chooseRekordboxSource()} />
        </div>

        {#if needsMusicRoot}
          <div class="ff-rbimport__row">
            <span class="ff-rbimport__path" title={rekordboxImport.musicRoot}>
              {rekordboxImport.musicRoot || 'Folder where your music is now'}
            </span>
            <Button label="Choose…" size="small" disabled={busy} onclick={() => void chooseRekordboxMusicRoot()} />
          </div>
          {#if preview}
            <label class="ff-rbimport__field">
              <span>Rekordbox path to replace</span>
              <input
                type="text"
                spellcheck="false"
                autocomplete="off"
                bind:value={rekordboxImport.prefix}
                disabled={busy}
                onkeydown={onPrefixKeydown}
                onchange={() => void scanRekordboxSource()}
              />
            </label>
          {/if}
        {/if}

        {#if preview}
          <dl class="ff-rbimport__stats">
            <dt>Tracks</dt><dd>{preview.tracks} ({preview.new_tracks} new, {preview.conflicts} already in library)</dd>
            <dt>Files found / missing</dt><dd>{preview.files_found} / {preview.files_missing}</dd>
            <dt>Playlists</dt><dd>{preview.playlists}{preview.folders > 0 ? `, ${preview.folders} folders flattened` : ''}</dd>
            <dt>Beat grids</dt><dd>{preview.tracks_with_grid}</dd>
            <dt>Hot / memory cues</dt><dd>{preview.hot_cues} / {preview.memory_cues}</dd>
          </dl>
          {#if preview.files_missing > 0}
            <SegmentedControl
              items={MISSING_CHOICES}
              activeIndex={rekordboxImport.includeMissing ? 1 : 0}
              disabled={busy}
              onChange={(i) => setRekordboxIncludeMissing(i === 1)}
            />
          {/if}
          <p class="ff-rbimport__note">
            Tracks already in your library keep your edits; only empty fields are filled. Nothing is deleted.
          </p>
        {/if}

        {#if rekordboxImport.phase === 'scanning'}
          <ProgressBar variant="indeterminate" />
        {:else if rekordboxImport.phase === 'importing'}
          <ProgressBar value={percent} />
          <p class="ff-rbimport__note">
            {rekordboxImport.progress
              ? `${rekordboxImport.progress.stage}: ${rekordboxImport.progress.done}/${rekordboxImport.progress.total}`
              : 'Reading…'}
          </p>
        {/if}

        {#if rekordboxImport.phase === 'error'}
          <p class="ff-rbimport__error">{rekordboxImport.error}</p>
        {/if}
      {/if}
    </div>
  {/snippet}
  {#snippet actions()}
    {#if rekordboxImport.phase === 'done'}
      <Button label="Done" variant="primary" onclick={closeRekordboxImport} />
    {:else}
      <Button label="Cancel" variant="ghost" disabled={busy} onclick={closeRekordboxImport} />
      <Button
        label={rekordboxImport.phase === 'importing' ? 'Importing…' : 'Import'}
        variant="primary"
        disabled={!canImport || busy}
        onclick={() => void runRekordboxImport()}
      />
    {/if}
  {/snippet}
</Dialog>

<style>
  .ff-rbimport {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-3);
  }

  .ff-rbimport__row {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
  }

  .ff-rbimport__path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ff-text-mid);
  }

  .ff-rbimport__field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .ff-rbimport__field input {
    font: inherit;
    color: var(--ff-text);
    background: var(--ff-bg);
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-md);
    padding: 6px 8px;
  }

  .ff-rbimport__lead {
    margin: 0;
    color: var(--ff-text);
  }

  .ff-rbimport__stats {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px var(--ff-space-4);
    margin: 0;
  }

  .ff-rbimport__stats dt {
    color: var(--ff-text-mid);
  }

  .ff-rbimport__stats dd {
    margin: 0;
    color: var(--ff-text);
  }

  .ff-rbimport__note {
    margin: 0;
    color: var(--ff-text-mid);
  }

  .ff-rbimport__error {
    margin: 0;
    color: var(--ff-danger);
    word-break: break-word;
  }
</style>
