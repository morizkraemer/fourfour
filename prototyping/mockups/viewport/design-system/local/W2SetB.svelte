<script>
  import { Dropdown, SegmentedControl, Button, Icon, Kbd, FilterInput } from '../../../../viewport/design-system/svelte.js';
  import { createToggle } from '../../../../viewport/design-system/index.js';
  import Vanilla from './Vanilla.svelte';
  import Ico from './Ico.svelte';

  let { page = 'palette' } = $props();

  const WORKERS = [{ value: 'auto', label: 'Auto (7 of 8 cores)' }];

  const ROWS = [
    ['Esc', '', 'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12'],
    ['`', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', 'Delete'],
    ['Tab', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '[', ']', '\\'],
    ['Caps', 'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', ';', "'", 'Return'],
    ['Shift', 'Z', 'X', 'C', 'V', 'B', 'N', 'M', ',', '.', '/', 'Shift '],
    ['Ctrl', 'Opt', 'Cmd', 'Space', 'Cmd ', 'Opt ', 'Ctrl '],
  ];
  const WIDE = { Delete: 1.6, Tab: 1.5, '\\': 1.5, Caps: 1.8, Return: 2.2, Shift: 2.3, 'Shift ': 2.9, Ctrl: 1.4, 'Ctrl ': 1.4, Opt: 1.2, 'Opt ': 1.2, Cmd: 1.4, 'Cmd ': 1.4, Space: 6.6, Esc: 1.5 };
  const BOUND = {
    Esc: 'Close', F6: 'Focus ›', '1': 'Fav 1', '2': 'Fav 2', '3': 'Fav 3', '4': 'Fav 4', '5': 'Fav 5', '6': 'Fav 6', '7': 'Fav 7', '8': 'Fav 8', '9': 'Fav 9',
    Tab: 'Player', C: 'Cue', M: 'Memory', Space: 'Play',
  };
  const SELECTED = 'C';
  const unit = 54;
</script>

{#if page === 'palette'}
  <div class="scrim">
    <div class="ff-cp pal">
      <div class="ff-cp__input-row">
        <Icon name="search" size={14} class="ff-cp__search-icon" />
        <span class="pal__q">analy<i class="pal__caret"></i></span>
        <kbd class="ff-cp__esc-kbd">esc</kbd>
      </div>
      <div class="ff-cp__content">
        <div class="ff-cp__section-head">SETTINGS · ANALYSIS</div>
        <div class="ff-cp__row ff-cp__row--active">
          <Icon name="settings" size={14} class="ff-cp__row-icon" />
          <span class="ff-cp__row-name"><b class="ff-cp__hit">Analy</b>ze new tracks on import <span class="ff-cp__row-sub">starts when files are added</span></span>
          <span class="pal__ctl"><Vanilla make={() => createToggle({ on: true })} /></span>
        </div>
        <div class="ff-cp__row">
          <Icon name="settings" size={14} class="ff-cp__row-icon" />
          <span class="ff-cp__row-name"><b class="ff-cp__hit">Analy</b>sis engine</span>
          <span class="pal__ctl"><SegmentedControl items={['Built-in', 'Python']} activeIndex={0} /></span>
        </div>
        <div class="ff-cp__row">
          <Icon name="settings" size={14} class="ff-cp__row-icon" />
          <span class="ff-cp__row-name">Parallel workers <span class="ff-cp__row-sub">for <b class="ff-cp__hit">analy</b>sis</span></span>
          <span class="pal__ctl pal__ctl--dd"><Dropdown options={WORKERS} value="auto" /></span>
        </div>
        <div class="ff-cp__section-head">COMMANDS</div>
        <div class="ff-cp__row">
          <Icon name="chevron-right" size={14} class="ff-cp__row-icon" />
          <span class="ff-cp__row-name"><b class="ff-cp__hit">Analy</b>ze selection</span>
          <span class="ff-cp__row-right">unassigned</span>
        </div>
        <div class="ff-cp__row">
          <Icon name="chevron-right" size={14} class="ff-cp__row-icon" />
          <span class="ff-cp__row-name">Re-<b class="ff-cp__hit">analy</b>ze library…</span>
          <span class="ff-cp__row-right"></span>
        </div>
        <div class="ff-cp__section-head">PAGES</div>
        <div class="ff-cp__row">
          <Icon name="chevron-right" size={14} class="ff-cp__row-icon" />
          <span class="ff-cp__row-name">Open <b class="ff-cp__hit">Analy</b>sis settings (log, last run, engine details)</span>
          <span class="ff-cp__row-right">⌘,</span>
        </div>
      </div>
      <div class="ff-cp__footer">
        <span class="ff-cp__footer-left">↑↓ navigate  ⏎ change  Space toggle  esc close</span>
        <span class="ff-cp__footer-right">6 results</span>
      </div>
    </div>
  </div>
{:else}
  <div class="scrim">
    <div class="km">
      <div class="km__top">
        <span class="km__t">Keyboard</span>
        <SegmentedControl items={['Plain', '⇧', '⌘', '⌥⇧']} activeIndex={0} />
        <div class="km__f"><FilterInput placeholder="Find an action…" /></div>
        <Button label="Reset all" size="small" />
        <button class="km__x" aria-label="Close"><Ico name="x" size={14} /></button>
      </div>
      <div class="km__body">
        <div class="km__board" style="width:{unit * 14.5}px">
          {#each ROWS as row}
            <div class="km__row">
              {#each row as k}
                {#if k === ''}
                  <span style="width:{unit * 0.55}px"></span>
                {:else}
                  {@const base = k.trim()}
                  <div class="km__key" class:km__key--on={BOUND[base]} class:km__key--sel={base === SELECTED && k === base} class:km__key--fn={/^F\d+$|^Esc$/.test(base)} style="width:{(WIDE[k] ?? 1) * unit - 4}px">
                    <span class="km__cap">{base}</span>
                    {#if BOUND[base] && k === base}<span class="km__act">{BOUND[base]}</span>{/if}
                  </div>
                {/if}
              {/each}
            </div>
          {/each}
        </div>
        <div class="km__side">
          <div class="km__sel"><span class="km__big">C</span><div><b>Cue</b><span>Return to the cue point, or set it at the playhead</span></div></div>
          <div class="km__btns"><Button label="Change…" size="small" /><Button label="Clear" size="small" variant="ghost" /></div>
          <div class="km__sec">ON THIS KEY, WITH MODIFIERS</div>
          <div class="km__l"><span>⇧C</span><em>unassigned</em></div>
          <div class="km__l"><span>⌘C</span><em>unassigned</em></div>
          <div class="km__sec">KEY NOTES</div>
          <p>Press any key on the board to see or change what it does. Keys that clash with text fields only act outside them.</p>
          <div class="km__sec">LEGEND</div>
          <div class="km__leg"><i class="km__d km__d--on"></i>assigned<i class="km__d"></i>free<i class="km__d km__d--sel"></i>selected</div>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim { position: absolute; inset: 0; background: rgba(0, 0, 0, 0.55); display: flex; align-items: flex-start; justify-content: center; padding-top: 96px; box-sizing: border-box; }
  .pal { max-width: 680px; }
  .pal__q { flex: 1; font-size: 14px; color: var(--ff-text); display: flex; align-items: center; }
  .pal__caret { width: 1px; height: 16px; background: var(--ff-accent); margin-left: 1px; }
  .pal__ctl { flex: none; display: flex; align-items: center; }
  .pal__ctl--dd { width: 190px; }
  .pal__ctl--dd :global(.ff-dropdown) { width: 100%; }

  .km { margin-top: 0; width: 1180px; background: var(--ff-surface); border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-xl); box-shadow: var(--ff-shadow-palette); font-family: var(--ff-font); color: var(--ff-text); overflow: hidden; }
  .km__top { display: flex; align-items: center; gap: 12px; height: 46px; padding: 0 12px 0 18px; border-bottom: 1px solid var(--ff-border); }
  .km__t { font-size: 13px; font-weight: 500; margin-right: 6px; }
  .km__f { flex: 1 1 0; max-width: 300px; margin-left: auto; }
  .km__x { border: none; background: none; color: var(--ff-text-mid); display: flex; padding: 5px; }
  .km__body { display: flex; gap: 24px; padding: 22px; }
  .km__board { flex: none; display: flex; flex-direction: column; gap: 4px; }
  .km__row { display: flex; gap: 4px; }
  .km__key { box-sizing: border-box; height: 50px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-lg); background: var(--ff-bg); padding: 5px 6px; display: flex; flex-direction: column; justify-content: space-between; color: var(--ff-faint); font-family: var(--ff-font-mono); }
  .km__key--fn { height: 32px; }
  .km__cap { font-size: 11px; }
  .km__act { font-family: var(--ff-font); font-size: 10.5px; color: var(--ff-text); white-space: nowrap; }
  .km__key--on { background: var(--ff-elev); border-color: var(--ff-border-hi); color: var(--ff-text-mid); }
  .km__key--sel { border-color: var(--ff-accent); box-shadow: 0 0 0 1px var(--ff-accent); }
  .km__side { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: 8px; }
  .km__sel { display: flex; gap: 12px; align-items: center; padding: 12px; border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-lg); background: var(--ff-bg); }
  .km__big { width: 44px; height: 44px; border: 1px solid var(--ff-accent); border-radius: var(--ff-radius-lg); display: flex; align-items: center; justify-content: center; font-family: var(--ff-font-mono); font-size: 18px; flex: none; }
  .km__sel b { display: block; font-size: 13px; font-weight: 500; }
  .km__sel span { font-size: 11.5px; color: var(--ff-muted); }
  .km__btns { display: flex; gap: 6px; }
  .km__sec { margin-top: 10px; font-family: var(--ff-font-mono); font-size: var(--ff-type-label); letter-spacing: 0.06em; color: var(--ff-muted); }
  .km__l { display: flex; justify-content: space-between; font-size: 12px; font-family: var(--ff-font-mono); color: var(--ff-text-mid); }
  .km__l em { font-style: normal; font-family: var(--ff-font); color: var(--ff-faint); }
  .km__side p { margin: 0; font-size: 11.5px; line-height: 1.5; color: var(--ff-muted); }
  .km__leg { display: flex; align-items: center; gap: 6px; font-size: 11.5px; color: var(--ff-text-mid); }
  .km__d { width: 12px; height: 12px; border: 1px solid var(--ff-border); border-radius: 3px; background: var(--ff-bg); display: inline-block; margin-left: 8px; }
  .km__d--on { background: var(--ff-elev); border-color: var(--ff-border-hi); margin-left: 0; }
  .km__d--sel { border-color: var(--ff-accent); }
</style>
