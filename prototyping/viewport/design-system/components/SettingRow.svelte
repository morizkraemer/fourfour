<script>
  import './settings-sheet.css';

  /** @type {{ title: string, description?: string, control?: import('svelte').Snippet }} */
  let { title, description = '', control } = $props();
</script>

<div class="ff-setting-row">
  <div class="ff-setting-row__texts">
    <span class="ff-setting-row__title">{title}</span>
    {#if description}<span class="ff-setting-row__desc">{description}</span>{/if}
  </div>
  <div class="ff-setting-row__control">{@render control?.()}</div>
</div>
