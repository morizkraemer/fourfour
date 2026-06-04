<!--
  Sidebar.svelte — Left navigation sidebar module.
  Traffic lights, nav sections (Favorites/Library/Playlists), USB pinned at bottom.
  Matches module-sidebar.artboard.js structure.
-->
<script>
  import { SidebarSection, SidebarRow } from '$ds';
  import { sidebarData } from '../stores/library.svelte.ts';
  import { ui } from '../stores/ui.svelte.ts';

  function handleRowClick(id) {
    ui.activeSidebarRow = id;
  }
</script>

<div class="ff-sidebar">
  <!-- macOS traffic lights -->
  <div class="ff-sidebar__chrome">
    <span class="ff-sidebar__dot ff-sidebar__dot--close"></span>
    <span class="ff-sidebar__dot ff-sidebar__dot--min"></span>
    <span class="ff-sidebar__dot ff-sidebar__dot--max"></span>
  </div>

  <!-- scrollable sections -->
  <div class="ff-sidebar__top">
    <SidebarSection label={sidebarData.favorites.label}>
      {#each sidebarData.favorites.rows as row}
        <SidebarRow
          kind={row.kind}
          digit={row.digit}
          label={row.label}
          count={row.count}
          state={ui.activeSidebarRow === row.label ? 'active' : 'rest'}
          onclick={() => handleRowClick(row.label)}
        />
      {/each}
    </SidebarSection>

    <SidebarSection label={sidebarData.library.label}>
      {#each sidebarData.library.rows as row}
        <SidebarRow
          kind={row.kind}
          icon={row.icon}
          label={row.label}
          count={row.count}
          state={ui.activeSidebarRow === 'all-tracks' && row.label === 'All Tracks' ? 'active' : 'rest'}
          onclick={() => handleRowClick(row.label)}
        />
      {/each}
    </SidebarSection>

    <SidebarSection label={sidebarData.playlists.label} showAdd>
      {#each sidebarData.playlists.rows as row}
        <SidebarRow
          kind={row.kind}
          label={row.label}
          count={row.count}
          state={ui.activeSidebarRow === row.label ? 'active' : 'rest'}
          onclick={() => handleRowClick(row.label)}
        />
      {/each}
    </SidebarSection>
  </div>

  <!-- USB pinned at bottom -->
  <div class="ff-sidebar__bottom">
    <SidebarSection label={sidebarData.usb.label}>
      {#each sidebarData.usb.rows as row}
        <SidebarRow
          kind={row.kind}
          label={row.label}
          status={row.status}
        />
      {/each}
    </SidebarSection>
  </div>
</div>

<style>
  .ff-sidebar {
    display: flex;
    flex-direction: column;
    width: 240px;
    height: 100%;
    background: var(--ff-bg);
    border-right: 1px solid var(--ff-border);
  }
  .ff-sidebar__chrome {
    display: flex;
    align-items: center;
    gap: var(--ff-space-3);
    padding: var(--ff-space-4) var(--ff-space-6);
    -webkit-app-region: drag;
  }
  .ff-sidebar__dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .ff-sidebar__dot--close { background: #ff5f57; }
  .ff-sidebar__dot--min   { background: #febc2e; }
  .ff-sidebar__dot--max   { background: #28c840; }

  .ff-sidebar__top {
    flex: 1 1 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .ff-sidebar__bottom {
    margin-top: auto;
    border-top: 1px solid var(--ff-border);
  }
</style>
