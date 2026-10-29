<!--
  LibraryActivityMenu.svelte — Popover under the top-bar library activity button.
  Shows the last import (this session), the missing-files actions while files are missing or
  unreadable, and the import entry points. Uses the shared menu surface (.ff-menu).
-->
<script>
  import { closeOnEscape } from '$ds/components/menuEscape.js';
  import { importActivity } from '../stores/import-activity.svelte.ts';
  import { fileStatus } from '../stores/file-status.svelte.ts';
  import { checkLibraryAgain, openHealthDialog, relocateMissing, showProblemTracks } from '../stores/file-health.svelte.ts';
  import { importDropTarget } from '../stores/ui.svelte.ts';
  import { importIntoSource, importFilesIntoSource } from '../menus/context-menus.ts';

  let { open = false, x = 0, y = 0, hasProblems = false, onClose } = $props();

  let menuEl = $state(null);
  let importing = $derived(importActivity.running);

  $effect(() => {
    if (!open) return;
    function onDoc(e) {
      if (menuEl && !menuEl.contains(e.target)) onClose?.();
    }
    function onKey(e) {
      closeOnEscape(e, onClose);
    }
    function onWindowBlur() {
      onClose?.();
    }
    const t = setTimeout(() => {
      document.addEventListener('pointerdown', onDoc, true);
      document.addEventListener('keydown', onKey);
      window.addEventListener('blur', onWindowBlur);
    }, 0);
    return () => {
      clearTimeout(t);
      document.removeEventListener('pointerdown', onDoc, true);
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('blur', onWindowBlur);
    };
  });

  function place(node) {
    const rect = node.getBoundingClientRect();
    node.style.left = `${Math.max(8, Math.min(x, window.innerWidth - rect.width - 8))}px`;
    node.style.top = `${Math.max(8, Math.min(y, window.innerHeight - rect.height - 8))}px`;
  }

  function run(action) {
    onClose?.();
    action();
  }
</script>

{#snippet entry(label, onclick, disabled = false)}
  <button type="button" class="ff-menu__item ff-menu__item--{disabled ? 'disabled' : 'default'} ff-activity__entry" {disabled} onclick={() => run(onclick)}>
    <span class="ff-menu__item-label">{label}</span>
  </button>
{/snippet}

{#if open}
  <div class="ff-menu ff-context-menu ff-activity" data-library-activity-menu bind:this={menuEl} use:place style="position: fixed; left: {x}px; top: {y}px; z-index: 10000;">
    {#if importing}
      <div class="ff-activity__section" data-import-progress>
        <span class="ff-activity__title">Importing</span>
        <span class="ff-activity__headline">
          {importActivity.total > 0 ? `${importActivity.done} of ${importActivity.total} files` : 'Looking for audio files…'}
        </span>
      </div>
      <div class="ff-menu__separator"></div>
    {:else if importActivity.summary}
      <div class="ff-activity__section" data-last-import>
        <span class="ff-activity__title">Last import</span>
        <span class="ff-activity__headline">{importActivity.summary.headline}</span>
        {#each importActivity.summary.lines as line}
          <span class="ff-activity__line">{line}</span>
        {/each}
      </div>
      <div class="ff-menu__separator"></div>
    {/if}
    {#if hasProblems}
      {@render entry('Relocate missing…', relocateMissing, fileStatus.missing === 0 || fileStatus.relocating)}
      {#if fileStatus.onlyProblems}
        {@render entry('Show all tracks', () => showProblemTracks(false))}
      {:else}
        {@render entry('Show missing tracks', () => showProblemTracks(true))}
      {/if}
      {@render entry('Check again', checkLibraryAgain, fileStatus.running)}
      {@render entry('Library check results…', openHealthDialog)}
      <div class="ff-menu__separator"></div>
    {/if}
    {@render entry('Import folder…', () => importIntoSource(importDropTarget()), importing)}
    {@render entry('Import files…', () => importFilesIntoSource(importDropTarget()), importing)}
  </div>
{/if}

<style>
  .ff-activity {
    min-width: 240px;
    max-width: 320px;
  }
  .ff-activity__section {
    display: flex;
    flex-direction: column;
    gap: var(--ff-space-1);
    padding: var(--ff-space-2) var(--ff-space-4);
    font-size: var(--ff-type-compact);
  }
  .ff-activity__title {
    font-size: var(--ff-type-label);
    color: var(--ff-muted);
  }
  .ff-activity__headline {
    color: var(--ff-text);
  }
  .ff-activity__line {
    color: var(--ff-text-mid);
  }
  .ff-activity__entry {
    width: 100%;
    height: 28px;
    padding-top: 0;
    padding-bottom: 0;
    border: none;
    background: none;
    text-align: left;
  }
  .ff-activity__entry:disabled {
    cursor: default;
  }
</style>
