<script>
  let { tag = '', title = '', model = '', good = [], cost = [], width = 1440, children } = $props();
</script>

<div class="fr" style="width:{width}px">
  {@render children()}
  <div class="fr__cap">
    <div class="fr__head">
      {#if tag}<span class="fr__tag">{tag}</span>{/if}
      <span class="fr__title">{title}</span>
      <span class="fr__model">{model}</span>
    </div>
    <div class="fr__cols">
      <ul class="fr__list fr__list--good">
        {#each good as g}<li>{g}</li>{/each}
      </ul>
      <ul class="fr__list fr__list--cost">
        {#each cost as c}<li>{c}</li>{/each}
      </ul>
    </div>
  </div>
</div>

<style>
  .fr { display: flex; flex-direction: column; gap: 10px; padding-bottom: 14px; font-family: var(--ff-font); color: var(--ff-text); }
  .fr__cap { display: flex; flex-direction: column; gap: 6px; padding: 2px 4px 0; }
  .fr__head { display: flex; align-items: baseline; gap: 10px; }
  .fr__tag { font-family: var(--ff-font-mono); font-size: 10px; letter-spacing: 0.08em; padding: 2px 7px; border-radius: 3px; background: var(--ff-accent); color: #0a0a0a; font-weight: 600; }
  .fr__title { font-size: 15px; font-weight: 500; }
  .fr__model { font-size: 12.5px; color: var(--ff-text-mid); }
  .fr__cols { display: grid; grid-template-columns: 1fr 1fr; gap: 24px; }
  .fr__list { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 2px; font-size: 12px; color: var(--ff-text-mid); }
  .fr__list li { padding-left: 16px; position: relative; }
  .fr__list li::before { position: absolute; left: 0; font-family: var(--ff-font-mono); }
  .fr__list--good li::before { content: '+'; color: var(--ff-status-online); }
  .fr__list--cost li::before { content: '−'; color: var(--ff-danger); }
</style>
