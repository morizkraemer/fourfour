<script>
  import { Button } from '$ds';
  import Popover from './Popover.svelte';
  import StickDot from './StickDot.svelte';
  import { library, libraryTrackById, analyzeTrackIds } from '../stores/library.svelte.ts';
  import { stickName, stickPlaylistIds, stickPlaylistTrackIds, removePlaylistFromStick } from '../stores/sticks.svelte.ts';
  import {
    stickSync,
    stickPending,
    stickProblemView,
    closeStickProblem,
    statusOfStick,
    syncStick,
    dismissStickSync,
  } from '../stores/stick-sync.svelte.ts';
  import { isStickProblem, roundBytes, stickProblem } from '../lib/stickProblem.ts';
  import { showStickInfo } from '../shortcuts/navigation.ts';
  import { ui } from '../stores/ui.svelte.ts';

  const volumePath = $derived(stickProblemView.volumePath);
  const status = $derived(volumePath ? statusOfStick(volumePath) : null);
  const pending = $derived(volumePath ? stickPending.byPath[volumePath] : undefined);

  const playlists = $derived.by(() => {
    if (!volumePath) return [];
    return stickPlaylistIds(volumePath).flatMap((id) => {
      const playlist = library.playlists.find((p) => p.id === id);
      if (!playlist) return [];
      let bytes = 0;
      for (const trackId of stickPlaylistTrackIds(volumePath, id, playlist.track_ids)) bytes += libraryTrackById(trackId)?.raw?.file_size ?? 0;
      return [{ id, name: playlist.name, bytes }];
    });
  });

  const problem = $derived(
    volumePath && status && isStickProblem(status.kind)
      ? stickProblem({
          kind: status.kind,
          stickName: stickName(volumePath),
          bytesNeeded: pending?.bytesNeeded ?? 0,
          freeBytes: pending?.freeBytes ?? null,
          playlists,
          unanalysed: pending?.skipped ?? [],
          error: stickSync.error,
        })
      : null
  );

  const stickShown = $derived(ui.activeSidebarRow === volumePath && ui.detailPaneOpen && !ui.organize.expanded);
  const hasFoot = $derived(!stickShown || problem?.action != null);

  $effect(() => {
    if (volumePath && !problem) closeStickProblem();
  });

  function retry() {
    const path = volumePath;
    closeStickProblem();
    if (path) void syncStick(path);
  }

  function dismiss() {
    closeStickProblem();
    dismissStickSync();
  }

  function analyze() {
    const ids = (pending?.skipped ?? []).map((t) => t.track_id).filter((id) => id != null);
    closeStickProblem();
    if (ids.length > 0) void analyzeTrackIds(ids);
  }

  function showStick() {
    const path = volumePath;
    closeStickProblem();
    if (path) void showStickInfo(path);
  }
</script>

{#if problem && volumePath}
    {#snippet problemFoot()}
      {#if stickShown}<span></span>{:else}<button type="button" class="ff-popover__link" onclick={showStick}>Show stick</button>{/if}
      <span class="ff-stickproblem__actions">
        {#if problem.action === 'retry'}
          <Button label="Dismiss" size="small" variant="ghost" onclick={dismiss} />
          <Button label="Retry" size="small" variant="primary" onclick={retry} />
        {:else if problem.action === 'analyze'}
          <Button label="Analyze" size="small" variant="primary" onclick={analyze} />
        {/if}
      </span>
    {/snippet}
  <Popover
    anchor={stickProblemView.anchor ?? undefined}
    width={300}
    title={problem.title}
    label={problem.title}
    onClose={closeStickProblem}
    foot={hasFoot ? problemFoot : undefined}
  >
    {#snippet heading()}
      <span class="ff-stickproblem__title" data-stick-problem-title><StickDot tone="problem" label="" />{problem.title}</span>
    {/snippet}
    <div class="ff-stickproblem" data-stick-problem={problem.kind}>
      <p class="ff-stickproblem__what">{problem.what}</p>
      {#if problem.names.length > 0}
        <ul class="ff-stickproblem__names">
          {#each problem.names as name}<li>{name}</li>{/each}
          {#if problem.moreNames > 0}<li class="ff-stickproblem__more">and {problem.moreNames} more</li>{/if}
        </ul>
      {/if}
      {#if problem.todo}<p class="ff-stickproblem__todo">{problem.todo}</p>{/if}
      {#if problem.playlists.length > 0}
        <ul class="ff-stickproblem__playlists">
          {#each problem.playlists as playlist (playlist.id)}
            <li data-stick-problem-playlist={playlist.id}>
              <span class="ff-stickproblem__plname">{playlist.name}</span>
              <span class="ff-stickproblem__size">{roundBytes(playlist.bytes)}</span>
              <button type="button" class="ff-popover__link" onclick={() => void removePlaylistFromStick(volumePath, playlist.id)}>Take off</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </Popover>
{/if}

<style>
  .ff-stickproblem__title {
    display: inline-flex;
    align-items: center;
    gap: var(--ff-space-4);
    min-width: 0;
    font-size: var(--ff-type-h2);
    line-height: 20px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ff-stickproblem {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-4);
    font-size: var(--ff-type-compact);
    line-height: 1.45;
    color: var(--ff-text-mid);
  }
  .ff-stickproblem p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .ff-stickproblem__todo {
    color: var(--ff-text);
  }
  .ff-stickproblem ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .ff-stickproblem__names li {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ff-text);
  }
  .ff-stickproblem__names .ff-stickproblem__more {
    color: var(--ff-muted);
  }
  .ff-stickproblem__playlists :global(.ff-popover__link) {
    line-height: 1;
  }
  .ff-stickproblem__playlists {
    border-top: 1px solid var(--ff-border);
  }
  .ff-stickproblem__playlists li {
    display: flex;
    align-items: center;
    gap: var(--ff-space-5);
    height: 28px;
    border-bottom: 1px solid var(--ff-border);
  }
  .ff-stickproblem__plname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ff-text);
  }
  .ff-stickproblem__size {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-num);
    color: var(--ff-muted);
  }
  .ff-stickproblem__actions {
    display: inline-flex;
    gap: var(--ff-space-3);
  }
</style>
