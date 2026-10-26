<script>
  let { title = '', feedback = [], options = [], matrix = [], recommendation = { pick: '', why: [], then: [] }, questions = [], kept = [] } = $props();
</script>

<div class="dc">
  <header class="dc__head">
    <span class="dc__kicker">DECISIONS</span>
    <h1>{title}</h1>
    <div class="dc__fb">
      {#each feedback as f}<span><b>{f.id}</b> {f.text}</span>{/each}
    </div>
  </header>

  <div class="dc__opts">
    {#each options as o}
      <div class="dc__opt" class:dc__opt--rec={o.rec}>
        <div class="dc__opt-top"><span class="dc__tag">{o.tag}</span><span class="dc__name">{o.name}</span>{#if o.rec}<span class="dc__recb">recommended</span>{/if}</div>
        <p>{o.idea}</p>
        <code>{o.board}</code>
      </div>
    {/each}
  </div>

  {#if matrix.length}
    <table class="dc__tbl">
      <thead>
        <tr><th>Question</th>{#each options as o}<th>{o.tag}</th>{/each}</tr>
      </thead>
      <tbody>
        {#each matrix as row}
          <tr>
            <td>{row.q}</td>
            {#each row.cells as c}<td class={c.startsWith('+') ? 'good' : c.startsWith('-') ? 'bad' : ''}>{c.slice(c.startsWith('+') || c.startsWith('-') ? 1 : 0)}</td>{/each}
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  <div class="dc__cols">
    <section class="dc__rec">
      <h2>Recommendation</h2>
      <p class="dc__pick">{recommendation.pick}</p>
      <ul>{#each recommendation.why as w}<li>{w}</li>{/each}</ul>
      {#if recommendation.then.length}
        <h3>Then</h3>
        <ul>{#each recommendation.then as w}<li>{w}</li>{/each}</ul>
      {/if}
    </section>
    <section class="dc__q">
      <h2>Open questions for Merlin</h2>
      <ol>{#each questions as q}<li>{q}</li>{/each}</ol>
      {#if kept.length}
        <h3>Kept from earlier decisions</h3>
        <ul class="dc__kept">{#each kept as k}<li>{k}</li>{/each}</ul>
      {/if}
    </section>
  </div>
</div>

<style>
  .dc { width: 1440px; min-height: 760px; box-sizing: border-box; padding: 32px 40px 40px; background: var(--ff-bg); border: 1px solid var(--ff-border-hi); border-radius: 10px; font-family: var(--ff-font); color: var(--ff-text); display: flex; flex-direction: column; gap: 22px; }
  .dc__kicker { font-family: var(--ff-font-mono); font-size: 10px; letter-spacing: 0.08em; color: var(--ff-faint); }
  h1 { margin: 4px 0 8px; font-size: 22px; font-weight: 500; }
  h2 { margin: 0 0 8px; font-size: 14px; font-weight: 500; }
  h3 { margin: 14px 0 6px; font-family: var(--ff-font-mono); font-size: 10px; letter-spacing: 0.06em; font-weight: 400; color: var(--ff-faint); text-transform: uppercase; }
  .dc__fb { display: flex; flex-direction: column; gap: 3px; font-size: 12.5px; color: var(--ff-text-mid); }
  .dc__fb b { font-family: var(--ff-font-mono); color: var(--ff-text); font-weight: 500; margin-right: 6px; }
  .dc__opts { display: grid; grid-template-columns: repeat(3, 1fr); gap: 14px; }
  .dc__opt { padding: 14px 16px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-xl); background: var(--ff-surface); display: flex; flex-direction: column; gap: 8px; }
  .dc__opt--rec { border-color: rgba(255, 255, 255, 0.34); }
  .dc__opt-top { display: flex; align-items: center; gap: 8px; }
  .dc__tag { font-family: var(--ff-font-mono); font-size: 10px; padding: 2px 7px; border-radius: 3px; background: var(--ff-accent); color: #0a0a0a; font-weight: 600; }
  .dc__name { font-size: 14px; font-weight: 500; flex: 1; }
  .dc__recb { font-family: var(--ff-font-mono); font-size: 9.5px; letter-spacing: 0.06em; color: var(--ff-status-online); text-transform: uppercase; }
  .dc__opt p { margin: 0; font-size: 12.5px; line-height: 1.5; color: var(--ff-text-mid); }
  .dc__opt code { font-family: var(--ff-font-mono); font-size: 11px; color: var(--ff-muted); }
  .dc__tbl { width: 100%; border-collapse: collapse; font-size: 12.5px; }
  .dc__tbl th { text-align: left; font-family: var(--ff-font-mono); font-size: 10px; letter-spacing: 0.06em; color: var(--ff-faint); padding: 6px 12px 6px 0; border-bottom: 1px solid var(--ff-border-hi); font-weight: 400; }
  .dc__tbl td { padding: 8px 14px 8px 0; border-bottom: 1px solid var(--ff-border); vertical-align: top; color: var(--ff-text-mid); width: 26%; line-height: 1.4; }
  .dc__tbl td:first-child { width: 22%; color: var(--ff-text); }
  .dc__tbl td.good::before { content: '+ '; color: var(--ff-status-online); font-family: var(--ff-font-mono); }
  .dc__tbl td.bad::before { content: '− '; color: var(--ff-danger); font-family: var(--ff-font-mono); }
  .dc__cols { display: grid; grid-template-columns: 1.1fr 1fr; gap: 28px; }
  section { padding: 16px 18px; border: 1px solid var(--ff-border); border-radius: var(--ff-radius-xl); background: var(--ff-surface); }
  .dc__rec { border-color: rgba(255, 255, 255, 0.2); }
  .dc__pick { margin: 0 0 8px; font-size: 14px; line-height: 1.5; }
  ul, ol { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 5px; font-size: 12.5px; line-height: 1.5; color: var(--ff-text-mid); }
  .dc__kept { color: var(--ff-muted); }
</style>
