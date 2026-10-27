<script>
  import Ico from './Ico.svelte';
  let { value = '', state = 'rest', mono = false, placeholder = '', mixed = false, suggest = null, align = 'left', big = false, multiline = false, height = null } = $props();
</script>

<div class="fld fld--{state}" class:fld--mono={mono} class:fld--big={big} class:fld--right={align === 'right'} class:fld--multi={multiline} style={height ? `height:${height}px` : ''}>
  {#if multiline}
    <textarea class="fld__in" {placeholder} spellcheck="false">{value}</textarea>
  {:else}
    <input class="fld__in" value={mixed ? '' : value} placeholder={mixed ? 'Mixed' : placeholder} spellcheck="false" />
  {/if}
  {#if state === 'hover'}<span class="fld__pen"><Ico name="pencil" size={11} /></span>{/if}
  {#if state === 'edit' && !multiline}<span class="fld__caret" style="left:calc(8px + {value.length * 0.54}em)"></span>{/if}
  {#if suggest}
    <div class="fld__sug">
      {#each suggest as s, i}<div class="fld__opt" class:fld__opt--on={i === 0}>{s}</div>{/each}
    </div>
  {/if}
</div>

<style>
  .fld { position: relative; display: flex; align-items: center; height: 26px; border: 1px solid transparent; border-radius: var(--ff-radius-md); background: transparent; min-width: 0; }
  .fld:hover { background: var(--ff-hover); border-color: var(--ff-border); }
  .fld__in { flex: 1 1 0; min-width: 0; width: 100%; height: 100%; margin: 0; padding: 0 8px; background: transparent; border: none; outline: none; color: var(--ff-text); font-family: var(--ff-font); font-size: 12.5px; text-overflow: ellipsis; resize: none; }
  .fld__in::placeholder { color: var(--ff-muted); font-style: italic; }
  .fld--mono .fld__in { font-family: var(--ff-font-mono); font-size: 12px; }
  .fld--big { height: 30px; }
  .fld--big .fld__in { font-size: 15px; font-weight: 500; }
  .fld--right .fld__in { text-align: right; }
  .fld--multi { align-items: stretch; height: auto; }
  .fld--multi .fld__in { padding: 6px 8px; line-height: 1.45; height: 100%; }
  .fld--hover { background: var(--ff-hover); border-color: var(--ff-border); }
  .fld__pen { position: absolute; right: 7px; color: var(--ff-muted); pointer-events: none; display: flex; }
  .fld--edit { background: var(--ff-bg); border-color: rgba(255, 255, 255, 0.34); }
  .fld__caret { position: absolute; top: 6px; bottom: 6px; width: 1px; background: var(--ff-accent); }
  .fld__sug { position: absolute; z-index: 30; left: 0; right: 0; top: calc(100% + 3px); background: var(--ff-elev); border: 1px solid var(--ff-border-hi); border-radius: var(--ff-radius-lg); box-shadow: var(--ff-shadow-menu); padding: 3px; }
  .fld__opt { padding: 5px 8px; font-size: 12px; color: var(--ff-text-mid); border-radius: var(--ff-radius-sm); }
  .fld__opt--on { background: var(--ff-select); color: var(--ff-text); }
</style>
