<script>
  /**
   * Tabbed preferences layout — Svelte port of settings-page.js.
   * Each section supplies field rows with optional `control` snippets.
   */
  import './settings-page.css';
  import Icon from './Icon.svelte';

  /** @typedef {{ label: string, key: string, items: Array<{ title: string, description: string, control?: import('svelte').Snippet }> }} SettingsSection */

  /** @type {{ sections?: SettingsSection[], fullscreen?: boolean, onClose?: () => void }} */
  let { sections = [], fullscreen = false, onClose } = $props();

  let activeKey = $state('');

  $effect(() => {
    if (sections.length === 0) return;
    const keys = sections.map((s, i) => s.key ?? String(i));
    if (!activeKey || !keys.includes(activeKey)) {
      activeKey = keys[0];
    }
  });

  /** @param {string} key */
  function selectTab(key) {
    activeKey = key;
  }
</script>

<div
  class="ff-settings-page"
  class:ff-settings-page--fullscreen={fullscreen}
>
  <nav class="ff-settings-page__nav">
    <div class="ff-settings-page__nav-head">
      <span class="ff-settings-page__nav-title">Preferences</span>
      {#if onClose}
        <button
          type="button"
          class="ff-settings-page__close"
          aria-label="Close preferences"
          onclick={onClose}
        >
          <Icon name="x" size={14} />
        </button>
      {/if}
    </div>
    {#each sections as sec, idx (sec.key ?? idx)}
      {@const tabKey = sec.key ?? String(idx)}
      <button
        type="button"
        class="ff-settings-page__nav-item"
        class:ff-settings-page__nav-item--active={activeKey === tabKey}
        onclick={() => selectTab(tabKey)}
      >
        {sec.label}
      </button>
    {/each}
  </nav>

  <div class="ff-settings-page__pane">
    {#each sections as sec, idx (sec.key ?? idx)}
      {@const tabKey = sec.key ?? String(idx)}
      <div
        class="ff-settings-page__pane-tab-content"
        class:ff-settings-page__pane-tab-content--active={activeKey === tabKey}
      >
        <h3 class="ff-settings-page__pane-heading">{sec.label}</h3>
        <p class="ff-settings-page__pane-sub">Configure settings for {sec.label}</p>
        <div class="ff-settings-page__fields-container">
          {#each sec.items as field (field.title)}
            <div class="ff-settings-page__field">
              <div class="ff-settings-page__field-texts">
                <span class="ff-settings-page__field-label">{field.title}</span>
                <span class="ff-settings-page__field-desc">{field.description}</span>
              </div>
              <div class="ff-settings-page__field-control">
                {#if field.control}
                  {@render field.control()}
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</div>
