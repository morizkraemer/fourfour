import {
  showConfirmDialog,
  showContextMenu,
  showProjectPickerDialog,
  showRenameDialog,
} from './sidebar-context-menu.js';

const copyIconHtml = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
    <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
  </svg>
`;

const pinIconHtml = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M12 17v5" />
    <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1z" />
  </svg>
`;

const noteIconHtml = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
  </svg>
`;

function makeListReorderable(listContainer, onReorder) {
  let draggedItem = null;

  listContainer.addEventListener('dragstart', (event) => {
    const item = event.target.closest('.sidebar-item--artboard');
    if (!item) return;
    draggedItem = item;
    draggedItem.classList.add('sidebar-item--dragging');
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', item.dataset.id);
  });

  listContainer.addEventListener('dragover', (event) => {
    event.preventDefault();
    if (!draggedItem) return;
    const target = event.target.closest('.sidebar-item--artboard');
    if (!target || target === draggedItem) return;

    const rect = target.getBoundingClientRect();
    const placeAfter = event.clientY - rect.top > rect.height / 2;
    listContainer.insertBefore(draggedItem, placeAfter ? target.nextSibling : target);
  });

  listContainer.addEventListener('dragend', () => {
    if (!draggedItem) return;
    draggedItem.classList.remove('sidebar-item--dragging');
    draggedItem = null;
    const ids = Array.from(listContainer.querySelectorAll('.sidebar-item--artboard'))
      .map((item) => item.dataset.id);
    onReorder(ids);
  });
}

function getDisplayName(artboard) {
  return artboard.displayName || artboard.sourceTitle;
}

export function createSidebar(options) {
  const sidebar = document.createElement('aside');
  sidebar.className = 'artboard-sidebar';

  const content = document.createElement('div');
  content.className = 'sidebar-content';
  sidebar.appendChild(content);

  const scroll = document.createElement('div');
  scroll.className = 'sidebar-scroll';
  content.appendChild(scroll);

  const stickyBottom = document.createElement('div');
  stickyBottom.className = 'sidebar-sticky-bottom';
  content.appendChild(stickyBottom);

  const footer = document.createElement('div');
  footer.className = 'sidebar-footer';
  const footerText = document.createElement('span');
  footerText.className = 'sidebar-footer-text';
  footerText.textContent = 'canvas-document v1';
  footer.appendChild(footerText);
  sidebar.appendChild(footer);

  let model = null;
  let activeArtboardId = null;
  let archiveCollapsed = true;
  const subfolderCollapsed = {}; // Key format: `${projectId}:${subfolderName}` -> boolean

  // Elegant dashed "+ New Project" button at the top
  const newProjectBtn = document.createElement('button');
  newProjectBtn.type = 'button';
  newProjectBtn.className = 'sidebar-new-project-btn';
  newProjectBtn.innerHTML = '<span>+ New Project</span>';
  newProjectBtn.addEventListener('click', () => {
    const name = window.prompt('Project name');
    if (name && name.trim()) options.onCreateProject?.(name.trim());
  });

  // Resizable handle element
  const resizeHandle = document.createElement('div');
  resizeHandle.className = 'sidebar-resize';
  resizeHandle.setAttribute('aria-hidden', 'true');
  sidebar.appendChild(resizeHandle);

  let sidebarWidth = 220;
  const MIN_WIDTH = 160;
  const MAX_WIDTH = 600;
  let resizing = false;

  function applyWidth() {
    sidebar.style.width = `${sidebarWidth}px`;
    document.documentElement.style.setProperty('--ff-sidebar-w', `${sidebarWidth}px`);
  }

  resizeHandle.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    resizing = true;
    resizeHandle.setPointerCapture(e.pointerId);
    document.body.classList.add('sidebar-resizing');
  });

  resizeHandle.addEventListener('pointermove', (e) => {
    if (!resizing) return;
    const next = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, e.clientX));
    sidebarWidth = next;
    applyWidth();
    options.onWidthChange?.(sidebarWidth);
  });

  function endResize(e) {
    if (!resizing) return;
    resizing = false;
    if (resizeHandle.hasPointerCapture(e.pointerId)) {
      resizeHandle.releasePointerCapture(e.pointerId);
    }
    document.body.classList.remove('sidebar-resizing');
  }

  resizeHandle.addEventListener('pointerup', endResize);
  resizeHandle.addEventListener('pointercancel', endResize);

  function setActiveStyles() {
    sidebar.querySelectorAll('.sidebar-item--artboard').forEach((item) => {
      item.classList.toggle('sidebar-item--active', item.dataset.id === activeArtboardId);
    });
  }

  function createSectionHeaderButton(title, collapsed, onToggle, extraText = '') {
    const header = document.createElement('button');
    header.type = 'button';
    header.className = 'sidebar-section-header sidebar-section-header-btn';
    header.innerHTML = `
      <span class="sidebar-section-chevron">${collapsed ? '▸' : '▾'}</span>
      <span class="sidebar-section-title">${title}</span>
      ${extraText ? `<span class="sidebar-section-count-inline">${extraText}</span>` : ''}
    `;
    header.addEventListener('click', onToggle);
    return header;
  }

  function createSubfolderHeader(title, collapsed, count, onToggle) {
    const header = document.createElement('button');
    header.type = 'button';
    header.className = 'sidebar-subfolder-header';
    header.innerHTML = `
      <span class="sidebar-subfolder-chevron">${collapsed ? '▸' : '▾'}</span>
      <span class="sidebar-subfolder-title">${title}</span>
      <span class="sidebar-subfolder-count">${count}</span>
    `;
    header.addEventListener('click', onToggle);
    return header;
  }

  function createArtboardRow(artboard, index, project) {
    const row = document.createElement('div');
    row.className = 'sidebar-item sidebar-item--artboard';
    row.dataset.id = artboard.id;
    row.dataset.projectId = project.id;
    row.draggable = true;

    const body = document.createElement('button');
    body.type = 'button';
    body.className = 'sidebar-item-body';

    const labelWrap = document.createElement('span');
    labelWrap.className = 'sidebar-item-label-wrap';

    const label = document.createElement('span');
    label.className = 'sidebar-item-label';
    label.textContent = getDisplayName(artboard);
    labelWrap.appendChild(label);

    const meta = document.createElement('span');
    meta.className = 'sidebar-item-meta';
    meta.textContent = artboard.id;
    labelWrap.appendChild(meta);
    body.appendChild(labelWrap);

    body.addEventListener('click', () => options.onNavigateArtboard?.(artboard.id));

    const actions = document.createElement('div');
    actions.className = 'sidebar-item-actions';

    const copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'sidebar-item-action';
    copyBtn.innerHTML = copyIconHtml;
    copyBtn.title = 'Copy reference';
    copyBtn.addEventListener('click', (event) => {
      event.stopPropagation();
      options.onCopyArtboard?.(artboard.id, { extended: event.shiftKey || event.altKey });
    });
    actions.appendChild(copyBtn);

    const noteBtn = document.createElement('button');
    noteBtn.type = 'button';
    noteBtn.className = 'sidebar-item-action';
    noteBtn.innerHTML = noteIconHtml;
    noteBtn.classList.toggle('sidebar-item-action--active', artboard.noteCount > 0);
    noteBtn.title = artboard.noteCount > 0 ? 'Edit note' : 'Add note';
    noteBtn.addEventListener('click', (event) => {
      event.stopPropagation();
      options.onEditArtboardNote?.(artboard.id);
    });
    actions.appendChild(noteBtn);

    const pinBtn = document.createElement('button');
    pinBtn.type = 'button';
    pinBtn.className = 'sidebar-item-action';
    pinBtn.innerHTML = pinIconHtml;
    pinBtn.classList.toggle('sidebar-item-action--active', artboard.pinned);
    pinBtn.title = artboard.pinned ? 'Unpin' : 'Pin';
    pinBtn.addEventListener('click', (event) => {
      event.stopPropagation();
      options.onTogglePin?.(artboard.id);
    });
    actions.appendChild(pinBtn);

    row.appendChild(body);
    row.appendChild(actions);

    row.addEventListener('contextmenu', (event) => {
      showContextMenu(event, [
        {
          label: 'Edit name',
          onSelect: () => showRenameDialog(getDisplayName(artboard), (value) => options.onRenameArtboard?.(artboard.id, value)),
        },
        {
          label: artboard.pinned ? 'Unpin' : 'Pin',
          onSelect: () => options.onTogglePin?.(artboard.id),
        },
        {
          label: artboard.noteCount > 0 ? 'Edit note...' : 'Add note...',
          onSelect: () => options.onEditArtboardNote?.(artboard.id),
        },
        {
          label: 'Move to project...',
          onSelect: () => showProjectPickerDialog({
            projects: model.projects,
            currentProjectId: artboard.projectId,
            onSelect: (nextProjectId) => options.onMoveArtboardProject?.(artboard.id, nextProjectId),
          }),
        },
        { type: 'divider' },
        {
          label: 'Archive',
          muted: true,
          onSelect: () => showConfirmDialog({
            title: 'Archive artboard',
            message: `Archive "${getDisplayName(artboard)}"?`,
            confirmLabel: 'Archive',
            onConfirm: () => options.onArchiveArtboard?.(artboard.id),
          }),
        },
      ]);
    });

    return row;
  }

  function buildProjectsSection() {
    const section = document.createElement('section');
    section.className = 'sidebar-section sidebar-section--projects';

    model.projects.forEach((project) => {
      const projectWrap = document.createElement('div');
      projectWrap.className = 'sidebar-project';
      projectWrap.dataset.projectId = project.id;

      const projectHeader = document.createElement('button');
      projectHeader.type = 'button';
      projectHeader.className = 'sidebar-project-header sidebar-project-header-btn';
      projectHeader.addEventListener('click', () => {
        options.onSelectActiveProject?.(project.id);
        options.onToggleProjectCollapse?.(project.id);
      });

      const chevron = document.createElement('span');
      chevron.className = 'sidebar-project-chevron';
      chevron.textContent = project.collapsed ? '▸' : '▾';
      projectHeader.appendChild(chevron);

      const title = document.createElement('span');
      title.className = 'sidebar-project-title';
      title.textContent = project.name;
      title.classList.toggle('sidebar-project-title--active', model.activeProjectId === project.id);
      projectHeader.appendChild(title);

      projectWrap.appendChild(projectHeader);

      const list = document.createElement('div');
      list.className = 'sidebar-project-list';
      list.classList.toggle('sidebar-project-list--collapsed', project.collapsed);
      projectWrap.appendChild(list);

      const projectArtboards = model.artboardsByProject[project.id] || [];
      const primitives = projectArtboards.filter((a) => a.layer === 'primitive');
      const composites = projectArtboards.filter((a) => a.layer !== 'primitive');

      if (!project.collapsed) {
        // Primitives subfolder
        const primKey = `${project.id}:primitives`;
        const primCollapsed = subfolderCollapsed[primKey] ?? false;

        const primHeader = createSubfolderHeader('Primitives', primCollapsed, primitives.length, () => {
          subfolderCollapsed[primKey] = !primCollapsed;
          render(model);
        });
        list.appendChild(primHeader);

        const primList = document.createElement('div');
        primList.className = 'sidebar-subfolder-list';
        primList.classList.toggle('sidebar-subfolder-list--collapsed', primCollapsed);

        primitives.forEach((artboard, index) => {
          primList.appendChild(createArtboardRow(artboard, index, project));
        });
        list.appendChild(primList);

        // Artboards subfolder
        const artKey = `${project.id}:artboards`;
        const artCollapsed = subfolderCollapsed[artKey] ?? false;

        const artHeader = createSubfolderHeader('Artboards', artCollapsed, composites.length, () => {
          subfolderCollapsed[artKey] = !artCollapsed;
          render(model);
        });
        list.appendChild(artHeader);

        const artList = document.createElement('div');
        artList.className = 'sidebar-subfolder-list';
        artList.classList.toggle('sidebar-subfolder-list--collapsed', artCollapsed);

        composites.forEach((artboard, index) => {
          artList.appendChild(createArtboardRow(artboard, index, project));
        });
        list.appendChild(artList);

        // Set up separate list drag-reordering
        const getMergedIds = () => {
          const primIds = Array.from(primList.querySelectorAll('.sidebar-item--artboard')).map((el) => el.dataset.id);
          const compIds = Array.from(artList.querySelectorAll('.sidebar-item--artboard')).map((el) => el.dataset.id);
          return [...primIds, ...compIds];
        };

        makeListReorderable(primList, () => {
          options.onReorderProjectArtboards?.(project.id, getMergedIds());
        });

        makeListReorderable(artList, () => {
          options.onReorderProjectArtboards?.(project.id, getMergedIds());
        });
      }

      section.appendChild(projectWrap);
    });

    return section;
  }

  function buildArchiveSection() {
    const section = document.createElement('section');
    section.className = 'sidebar-section sidebar-section--archive';

    const header = createSectionHeaderButton('Archive', archiveCollapsed, () => {
      archiveCollapsed = !archiveCollapsed;
      render(model);
    }, String(model.archivedArtboards.length));
    section.appendChild(header);

    const list = document.createElement('div');
    list.className = 'sidebar-archive-list';
    list.classList.toggle('sidebar-archive-list--collapsed', archiveCollapsed);
    section.appendChild(list);

    model.archivedArtboards.forEach((artboard) => {
      const item = document.createElement('button');
      item.type = 'button';
      item.className = 'sidebar-item sidebar-item--archived';
      item.textContent = `${getDisplayName(artboard)} (${artboard.id})`;
      item.addEventListener('click', () => options.onRestoreArtboard?.(artboard.id));
      item.addEventListener('contextmenu', (event) => {
        showContextMenu(event, [
          {
            label: 'Edit name',
            onSelect: () => showRenameDialog(getDisplayName(artboard), (value) => options.onRenameArtboard?.(artboard.id, value)),
          },
          {
            label: 'Restore',
            onSelect: () => options.onRestoreArtboard?.(artboard.id),
          },
        ]);
      });
      list.appendChild(item);
    });

    return section;
  }

  function render(nextModel) {
    model = nextModel;
    if (model.ui?.leftSidebarWidth) {
      sidebarWidth = model.ui.leftSidebarWidth;
      applyWidth();
    }
    scroll.innerHTML = '';
    stickyBottom.innerHTML = '';

    scroll.appendChild(newProjectBtn);
    scroll.appendChild(buildProjectsSection());
    stickyBottom.appendChild(buildArchiveSection());
    setActiveStyles();
  }

  function setActiveArtboard(id) {
    activeArtboardId = id;
    setActiveStyles();
  }

  return {
    element: sidebar,
    render,
    setActiveArtboard,
  };
}
