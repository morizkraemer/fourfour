<!--
  RekordboxImport.svelte — Import tracks, playlists, grids and cues from a
  Rekordbox library (desktop master.db, USB stick, or rekordbox.xml).
  Opened from Settings → Library.
-->
<script>
  import { Dialog, Button, Icon, SegmentedControl, ProgressBar } from '$ds';
  import {
    rekordboxImport,
    REKORDBOX_SOURCES,
    rowStats,
    closeRekordboxImport,
    setRekordboxKind,
    setRekordboxIncludeMissing,
    chooseRekordboxSource,
    addRekordboxRemapRow,
    removeRekordboxRemapRow,
    chooseRekordboxRemapTarget,
    rescanRekordboxRemap,
    runRekordboxImport,
  } from '../stores/rekordbox-import.svelte.ts';

  const MISSING_CHOICES = ['Skip missing files', 'Keep missing files'];
  const STAGE_LABELS = {
    reading: 'Reading library',
    analysis: 'Reading beat grids',
    tracks: 'Importing tracks',
    playlists: 'Importing playlists',
  };

  let kindIndex = $derived(REKORDBOX_SOURCES.findIndex((s) => s.kind === rekordboxImport.kind));
  let busy = $derived(rekordboxImport.phase === 'scanning' || rekordboxImport.phase === 'importing');
  let needsLocations = $derived(rekordboxImport.kind !== 'usb');
  let preview = $derived(rekordboxImport.preview);
  let report = $derived(rekordboxImport.report);
  let stats = $derived(rowStats());
  let canImport = $derived(rekordboxImport.phase === 'ready' && !!preview && preview.tracks > 0);
  let importCount = $derived(
    preview ? (rekordboxImport.includeMissing ? preview.tracks : preview.files_found) : 0,
  );
  let progress = $derived(rekordboxImport.progress);
  let percent = $derived(progress && progress.total > 0 ? (progress.done / progress.total) * 100 : 0);
  let sourceHint = $derived(
    rekordboxImport.kind === 'master_db'
      ? 'Choose master.db from your Rekordbox folder'
      : rekordboxImport.kind === 'usb'
        ? 'Choose the root folder of the stick'
        : 'Choose a rekordbox.xml export',
  );

  function onFromKeydown(e) {
    if (e.key === 'Enter') void rescanRekordboxRemap();
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
        <div class="ff-rbimport__section">Import finished</div>
        <div class="ff-rbimport__tiles">
          <div class="ff-rbimport__tile"><b>{report.tracks_imported}</b><span>Tracks added</span></div>
          <div class="ff-rbimport__tile"><b>{report.tracks_merged}</b><span>Tracks merged</span></div>
          <div class="ff-rbimport__tile"><b>{report.playlists_imported + report.playlists_merged}</b><span>Playlists</span></div>
          <div class="ff-rbimport__tile"><b>{report.grids_imported}</b><span>Beat grids</span></div>
          <div class="ff-rbimport__tile"><b>{report.hot_cues_imported}</b><span>Hot cues</span></div>
          <div class="ff-rbimport__tile"><b>{report.memory_cues_imported}</b><span>Memory cues</span></div>
          <div class="ff-rbimport__tile" class:ff-rbimport__tile--warn={report.tracks_skipped_missing > 0}>
            <b>{report.tracks_skipped_missing}</b><span>Skipped, file missing</span>
          </div>
          <div class="ff-rbimport__tile"><b>{report.files_found}</b><span>Files found</span></div>
        </div>
      {:else}
        <div class="ff-rbimport__sources">
          <SegmentedControl
            items={REKORDBOX_SOURCES.map((s) => s.label)}
            activeIndex={kindIndex}
            disabled={busy}
            onChange={(i) => setRekordboxKind(REKORDBOX_SOURCES[i].kind)}
          />
        </div>

        <div class="ff-rbimport__section">Library</div>
        <div class="ff-rbimport__field">
          <span class="ff-rbimport__path" class:ff-rbimport__path--empty={!rekordboxImport.path} title={rekordboxImport.path}>
            {rekordboxImport.path || sourceHint}
          </span>
          <Button label={rekordboxImport.path ? 'Change…' : 'Choose…'} size="small" disabled={busy} onclick={() => void chooseRekordboxSource()} />
        </div>

        {#if needsLocations && preview}
          <div class="ff-rbimport__section">Where your music is now</div>
          <div class="ff-rbimport__rows">
            {#each rekordboxImport.rows as row, i}
              {@const stat = stats[i]}
              <div class="ff-rbimport__row">
                <input
                  class="ff-rbimport__from"
                  type="text"
                  spellcheck="false"
                  autocomplete="off"
                  placeholder="Old location in Rekordbox"
                  title={row.from}
                  bind:value={row.from}
                  disabled={busy}
                  onkeydown={onFromKeydown}
                  onchange={() => void rescanRekordboxRemap()}
                />
                <Icon name="chevron-right" size={12} />
                <div class="ff-rbimport__field ff-rbimport__field--row">
                  <span class="ff-rbimport__path" class:ff-rbimport__path--empty={!row.to} title={row.to}>
                    {row.to || 'New folder'}
                  </span>
                  <Button label={row.to ? 'Change…' : 'Choose…'} size="small" disabled={busy} onclick={() => void chooseRekordboxRemapTarget(i)} />
                </div>
                <Button
                  icon="x"
                  variant="ghost"
                  size="small"
                  ariaLabel="Remove location"
                  disabled={busy || rekordboxImport.rows.length === 1}
                  onclick={() => void removeRekordboxRemapRow(i)}
                />
                {#if stat}
                  <span class="ff-rbimport__rowstat">
                    {stat.files_found} found
                    <span class:ff-rbimport__missing={stat.files_missing > 0}>· {stat.files_missing} missing</span>
                  </span>
                {/if}
              </div>
            {/each}
          </div>
          <div class="ff-rbimport__add">
            <Button icon="plus" label="Add location" variant="ghost" size="small" disabled={busy} onclick={addRekordboxRemapRow} />
          </div>
        {/if}

        {#if preview}
          <div class="ff-rbimport__section">Preview</div>
          <div class="ff-rbimport__tiles">
            <div class="ff-rbimport__tile"><b>{preview.tracks}</b><span>Tracks</span></div>
            <div class="ff-rbimport__tile"><b>{preview.new_tracks}</b><span>New</span></div>
            <div class="ff-rbimport__tile"><b>{preview.conflicts}</b><span>Already in library</span></div>
            <div class="ff-rbimport__tile">
              <b>{preview.playlists}</b><span>Playlists{preview.folders > 0 ? ` in ${preview.folders} ${preview.folders === 1 ? 'folder' : 'folders'}` : ''}</span>
            </div>
            <div class="ff-rbimport__tile"><b>{preview.files_found}</b><span>Files found</span></div>
            <div class="ff-rbimport__tile" class:ff-rbimport__tile--warn={preview.files_missing > 0}>
              <b>{preview.files_missing}</b><span>Files missing</span>
            </div>
            <div class="ff-rbimport__tile"><b>{preview.tracks_with_grid}</b><span>Beat grids</span></div>
            <div class="ff-rbimport__tile"><b>{preview.hot_cues} / {preview.memory_cues}</b><span>Hot / memory cues</span></div>
          </div>
          {#if preview.files_missing > 0}
            <div class="ff-rbimport__missing-choice">
              <SegmentedControl
                items={MISSING_CHOICES}
                activeIndex={rekordboxImport.includeMissing ? 1 : 0}
                disabled={busy}
                onChange={(i) => setRekordboxIncludeMissing(i === 1)}
              />
            </div>
          {/if}
          <p class="ff-rbimport__note">
            Tracks already in your library keep your edits; only empty fields are filled. Nothing is deleted.
          </p>
        {/if}

        {#if rekordboxImport.phase === 'scanning'}
          <ProgressBar variant="indeterminate" />
          <p class="ff-rbimport__note">Reading library…</p>
        {:else if rekordboxImport.phase === 'importing'}
          <ProgressBar value={percent} />
          <p class="ff-rbimport__note">
            {progress
              ? `${STAGE_LABELS[progress.stage] ?? progress.stage}${progress.total > 0 ? `: ${progress.done} of ${progress.total}` : ''}`
              : 'Reading library…'}
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
        label={rekordboxImport.phase === 'importing' ? 'Importing…' : canImport ? `Import ${importCount} tracks` : 'Import'}
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
    gap: var(--ff-space-4);
    padding-top: var(--ff-space-3);
    font-family: var(--ff-font);
    font-size: var(--ff-type-row);
  }

  .ff-rbimport__sources {
    display: flex;
    justify-content: flex-start;
    margin-bottom: var(--ff-space-2);
  }

  .ff-rbimport__section {
    margin-top: var(--ff-space-4);
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ff-muted);
  }

  .ff-rbimport__field {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-width: 0;
    padding: 3px 3px 3px 10px;
    background: var(--ff-bg);
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-md);
  }

  .ff-rbimport__field--row {
    flex: 1 1 0;
  }

  .ff-rbimport__path {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-caption);
    color: var(--ff-text);
  }

  .ff-rbimport__path--empty {
    font-family: var(--ff-font);
    color: var(--ff-faint);
  }

  .ff-rbimport__rows {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-5);
  }

  .ff-rbimport__row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--ff-space-4);
    color: var(--ff-muted);
  }

  .ff-rbimport__from {
    flex: 1 1 0;
    min-width: 0;
    height: 32px;
    box-sizing: border-box;
    padding: 0 10px;
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-caption);
    color: var(--ff-text);
    background: var(--ff-bg);
    border: 1px solid var(--ff-border-hi);
    border-radius: var(--ff-radius-md);
    outline: none;
  }

  .ff-rbimport__from:focus {
    border-color: rgba(255, 255, 255, 0.3);
  }

  .ff-rbimport__rowstat {
    flex: 0 0 100%;
    padding-left: 2px;
    font-size: var(--ff-type-caption);
    color: var(--ff-text-mid);
  }

  .ff-rbimport__missing {
    color: var(--ff-danger);
  }

  .ff-rbimport__add {
    display: flex;
  }

  .ff-rbimport__tiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--ff-space-4);
  }

  .ff-rbimport__tile {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 9px 12px;
    background: var(--ff-elev);
    border: 1px solid var(--ff-border);
    border-radius: var(--ff-radius-lg);
    min-width: 0;
  }

  .ff-rbimport__tile b {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-h2);
    font-weight: 500;
    color: var(--ff-text);
  }

  .ff-rbimport__tile span {
    font-size: var(--ff-type-caption);
    color: var(--ff-muted);
  }

  .ff-rbimport__tile--warn b {
    color: var(--ff-danger);
  }

  .ff-rbimport__missing-choice {
    display: flex;
  }

  .ff-rbimport__note {
    margin: 0;
    font-size: var(--ff-type-caption);
    color: var(--ff-muted);
  }

  .ff-rbimport__error {
    margin: 0;
    color: var(--ff-danger);
    word-break: break-word;
  }
</style>
