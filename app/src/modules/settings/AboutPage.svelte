<script>
  import { SettingRow } from '$ds';
  import { library } from '../../stores/library.svelte.ts';
  import appIcon from '../../assets/app-icon.png';

  let version = $derived(library.appVersion ? `Version ${library.appVersion}` : 'Version unknown');
</script>

<div class="ff-about" data-settings-about>
  <img class="ff-about__icon" src={appIcon} alt="" width="96" height="96" />
  <div class="ff-about__name">fourfour</div>
  <div class="ff-about__version" data-about-version>{version}</div>
  <div class="ff-about__tagline">Sync, play and organize your DJ library.</div>
</div>

<div class="ff-settings-section">Credits</div>
<div class="ff-settings-card" data-about-credits>
  <SettingRow title="Beat This!" description="Beat and downbeat model by the Institute of Computational Perception, JKU Linz. MIT licence." />
  <SettingRow title="beat-this-rs" description="Rust port of Beat This! MIT licence." />
</div>

<style>
  .ff-about {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 24px 16px;
    text-align: center;
  }

  .ff-about__icon {
    width: 96px;
    height: 96px;
    margin-bottom: 8px;
  }

  .ff-about__name {
    font-family: var(--ff-font);
    font-size: var(--ff-type-h1);
    font-weight: 600;
    color: var(--ff-text);
  }

  .ff-about__version {
    font-family: var(--ff-font-mono);
    font-size: var(--ff-type-label);
    color: var(--ff-muted);
  }

  .ff-about__tagline {
    margin-top: 4px;
    font-family: var(--ff-font);
    font-size: var(--ff-type-body);
    color: var(--ff-muted);
  }
</style>
