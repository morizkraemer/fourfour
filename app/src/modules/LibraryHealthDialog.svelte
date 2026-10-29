<script>
  import { Dialog, Button, StatusDot } from '$ds';
  import { library } from '../stores/library.svelte.ts';
  import { fileStatus } from '../stores/file-status.svelte.ts';
  import {
    relocateResult,
    closeHealthDialog,
    relocateMissing,
    locateTrack,
    checkLibraryAgain,
    showProblemTracks,
  } from '../stores/file-health.svelte.ts';
  import { checkSummary, relocateSummary } from '../lib/fileHealth.ts';

  const MAX_NAMES = 5;

  let fine = $derived(Math.max(0, fileStatus.total - fileStatus.missing - fileStatus.error));
  let report = $derived(relocateResult.report);
  let unresolved = $derived(report ? [...report.ambiguous, ...report.not_found].filter((id) => fileStatus.byId[id] === 'missing') : []);
  let shownUnresolved = $derived(unresolved.slice(0, MAX_NAMES));

  function trackName(id) {
    const t = library.tracks.find((track) => track.id === id);
    if (!t) return `Track ${id}`;
    return t.artist && t.artist !== '—' ? `${t.artist} – ${t.title}` : t.title;
  }

  function why(id) {
    return report?.ambiguous.includes(id) ? 'more than one match' : 'not found';
  }

  function showProblems() {
    showProblemTracks(true);
    closeHealthDialog();
  }
</script>

<Dialog open={fileStatus.dialogOpen} title="Library check" onClose={closeHealthDialog}>
  {#snippet body()}
    <div class="ff-health" data-health-dialog>
      <p class="ff-health__sum">
        {fileStatus.checkedAt === null ? 'The library has not been checked yet.' : checkSummary(fileStatus, fileStatus.durationMs)}
      </p>
      <div class="ff-health__lines">
        <div class="ff-health__line" data-health-line="ok">
          <StatusDot status="online" />
          <span class="ff-health__text">{fine} {fine === 1 ? 'track is' : 'tracks are'} fine</span>
        </div>
        {#if fileStatus.missing > 0}
          <div class="ff-health__line" data-health-line="missing">
            <StatusDot status="warn" />
            <span class="ff-health__text">
              {fileStatus.missing} {fileStatus.missing === 1 ? 'file is' : 'files are'} missing (moved, or on a drive that is off)
            </span>
            <Button label="Relocate…" size="small" disabled={fileStatus.relocating} onclick={() => void relocateMissing()} />
          </div>
        {/if}
        {#if fileStatus.error > 0}
          <div class="ff-health__line" data-health-line="error">
            <StatusDot status="offline" />
            <span class="ff-health__text">{fileStatus.error} {fileStatus.error === 1 ? "file can't" : "files can't"} be read</span>
            <Button label="Show" size="small" variant="ghost" onclick={showProblems} />
          </div>
        {/if}
        {#if fileStatus.relocated > 0}
          <div class="ff-health__line" data-health-line="relocated">
            <StatusDot status="online" />
            <span class="ff-health__text">{fileStatus.relocated} relinked since launch</span>
          </div>
        {/if}
      </div>
      {#if report}
        <section class="ff-health__report" data-relocate-report>
          <h3>Relocate</h3>
          <p>{relocateSummary(report)}</p>
          {#each shownUnresolved as id (id)}
            <div class="ff-health__line" data-relocate-unresolved={id}>
              <StatusDot status="warn" />
              <span class="ff-health__text">{trackName(id)} <em>{why(id)}</em></span>
              <Button label="Locate…" size="small" variant="ghost" onclick={() => void locateTrack(id)} />
            </div>
          {/each}
          {#if unresolved.length > shownUnresolved.length}
            <p class="ff-health__note">and {unresolved.length - shownUnresolved.length} more</p>
          {/if}
        </section>
      {/if}
      {#if relocateResult.error}
        <p class="ff-health__error">{relocateResult.error}</p>
      {/if}
      <p class="ff-health__note">Relinking changes only where a file is. Cues, analysis and playlists stay.</p>
    </div>
  {/snippet}
  {#snippet actions()}
    <Button label="Check again" variant="ghost" disabled={fileStatus.running} onclick={() => void checkLibraryAgain()} />
    <Button label="Done" variant="primary" onclick={closeHealthDialog} />
  {/snippet}
</Dialog>

<style>
  .ff-health {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
  }
  .ff-health p,
  .ff-health h3 {
    margin: 0;
  }
  .ff-health h3 {
    font-size: inherit;
    font-weight: 600;
  }
  .ff-health__sum {
    color: var(--ff-text);
  }
  .ff-health__lines,
  .ff-health__report {
    display: flex;
    flex-direction: column;
  }
  .ff-health__report {
    gap: var(--ff-space-2);
  }
  .ff-health__line {
    display: flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-height: 28px;
  }
  .ff-health__text {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ff-health__text em {
    font-style: normal;
    color: var(--ff-muted);
  }
  .ff-health__note {
    color: var(--ff-muted);
  }
  .ff-health__error {
    color: var(--ff-danger);
  }
</style>
