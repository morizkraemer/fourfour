import './canvas.css';
import Konva from 'konva';
import { createArtboard } from './artboard.js';
import { createProjectFrame } from './project-frame.js';
import { showRenameDialog } from './sidebar-context-menu.js';

const SCALE_BY = 1.05;
const MIN_SCALE = 0.15;
const MAX_SCALE = 4;
const SNAP = 8;

const ARTBOARD_BASE_X = 48;
const ARTBOARD_BASE_Y = 56;
const ARTBOARD_COL_SPACING = 360;
const ARTBOARD_ROW_SPACING = 300;

export function createCanvas({ viewport, world, documentStore, pinSidebar, registry, onSelectActiveArtboard }) {
  const cards = new Map(); // id -> { id, element, pinBtn, renameBtn, label }
  const projectFrames = new Map(); // projectId -> ProjectFrame components
  let copyToast = null;

  const LEFT_SIDEBAR_WIDTH = 220;
  let pinSidebarWidth = pinSidebar.getWidth();
  let pinSidebarPinned = pinSidebar.isActive();
  let leftSidebarOpen = true;
  let pinPanelOpen = true;

  function getCanvasInsets() {
    return {
      left: leftSidebarOpen ? LEFT_SIDEBAR_WIDTH : 0,
      right: pinSidebarPinned && pinPanelOpen ? pinSidebarWidth : 0,
    };
  }

  function getCanvasWidth() {
    const { left, right } = getCanvasInsets();
    return window.innerWidth - left - right;
  }

  function getViewportCenterX() {
    const { left } = getCanvasInsets();
    return left + getCanvasWidth() / 2;
  }

  function clientXToCanvasX(clientX) {
    return clientX - getCanvasInsets().left;
  }

  let leftSidebarToggle;
  let pinSidebarToggle;

  function updateSidebarToggles() {
    if (!leftSidebarToggle || !pinSidebarToggle) return;

    const { left, right } = getCanvasInsets();
    leftSidebarToggle.style.left = leftSidebarOpen ? `${LEFT_SIDEBAR_WIDTH - 1}px` : '0';
    leftSidebarToggle.dataset.open = leftSidebarOpen ? 'true' : 'false';
    leftSidebarToggle.title = leftSidebarOpen ? 'Hide sidebar' : 'Show sidebar';
    leftSidebarToggle.setAttribute('aria-label', leftSidebarToggle.title);

    const showPinToggle = pinSidebarPinned;
    pinSidebarToggle.hidden = !showPinToggle;
    if (showPinToggle) {
      pinSidebarToggle.style.right = pinPanelOpen ? `${right - 1}px` : '0';
      pinSidebarToggle.dataset.open = pinPanelOpen ? 'true' : 'false';
      pinSidebarToggle.title = pinPanelOpen ? 'Hide pin panel' : 'Show pin panel';
      pinSidebarToggle.setAttribute('aria-label', pinSidebarToggle.title);
    }
  }

  function applyCanvasLayout() {
    const { left, right } = getCanvasInsets();
    viewport.style.left = `${left}px`;
    viewport.style.right = `${right}px`;
    stage.width(getCanvasWidth());
    stage.height(window.innerHeight);
    stage.batchDraw();
    if (zoomUI) zoomUI.style.right = `${16 + right}px`;
    pinSidebar.refreshAll();
    updateSidebarToggles();
  }

  function toggleLeftSidebar() {
    leftSidebarOpen = !leftSidebarOpen;
    const sidebarEl = document.getElementById('sidebar');
    sidebarEl?.classList.toggle('artboard-sidebar--hidden', !leftSidebarOpen);
    documentStore.mutate((draft) => {
      draft.ui.leftSidebarOpen = leftSidebarOpen;
    });
    applyCanvasLayout();
  }

  function togglePinPanel() {
    if (!pinSidebarPinned) return;
    pinPanelOpen = !pinPanelOpen;
    documentStore.mutate((draft) => {
      draft.ui.pinPanelOpen = pinPanelOpen;
    });
    applyCanvasLayout();
  }

  function createSidebarToggle(side, onClick) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = `sidebar-edge-toggle sidebar-edge-toggle--${side}`;
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      onClick();
    });
    document.body.appendChild(btn);
    return btn;
  }

  // ── 1. Create Konva Stage for Panning/Zooming and Grid ──
  const stage = new Konva.Stage({
    container: viewport,
    width: getCanvasWidth(),
    height: window.innerHeight,
    draggable: true,
  });

  const bgLayer = new Konva.Layer();
  stage.add(bgLayer);

  // Large background rect as drag target
  bgLayer.add(new Konva.Rect({
    x: -10000,
    y: -10000,
    width: 20000,
    height: 20000,
    fill: 'transparent',
    listening: true,
  }));

  // Render grid dots
  const gridSpacing = 40;
  const gridRange = 3000;
  for (let x = -gridRange; x <= gridRange; x += gridSpacing) {
    for (let y = -gridRange; y <= gridRange; y += gridSpacing) {
      bgLayer.add(new Konva.Circle({
        x, y,
        radius: 1,
        fill: 'rgba(255, 255, 255, 0.08)',
        listening: false,
      }));
    }
  }
  bgLayer.draw();

  // ── 2. Viewport Transform Syncing ──
  function syncDOM() {
    const s = stage.scaleX();
    const x = stage.x();
    const y = stage.y();
    world.style.transform = `translate(${x}px, ${y}px) scale(${s})`;
    if (zoomLevel) zoomLevel.textContent = `${Math.round(s * 100)}%`;
  }

  stage.on('dragmove', syncDOM);
  stage.on('dragend', syncDOM);

  // Convert client cursor coords to scaled world coords
  function clientToWorld(clientX, clientY) {
    const worldRect = world.getBoundingClientRect();
    const scale = stage.scaleX();
    return {
      x: (clientX - worldRect.left) / scale,
      y: (clientY - worldRect.top) / scale,
    };
  }

  // ── 3. Wheel: scale-aware trackpad pan & zoom-to-cursor ──
  viewport.addEventListener('wheel', (e) => {
    e.preventDefault();

    const r = viewport.getBoundingClientRect();
    const px = e.clientX - r.left;
    const py = e.clientY - r.top;

    if (e.ctrlKey || e.metaKey) {
      // Zoom toward cursor
      const oldScale = stage.scaleX();
      const mousePointTo = {
        x: (px - stage.x()) / oldScale,
        y: (py - stage.y()) / oldScale,
      };

      let direction = e.deltaY > 0 ? 1 : -1;
      if (e.ctrlKey || e.metaKey) direction = -direction;

      const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE,
        direction > 0 ? oldScale * SCALE_BY : oldScale / SCALE_BY
      ));

      stage.scale({ x: newScale, y: newScale });
      stage.position({
        x: px - mousePointTo.x * newScale,
        y: py - mousePointTo.y * newScale,
      });
      stage.batchDraw();
      syncDOM();
    } else {
      // Panning divided by scale so trackpad swipes stay 1:1 at all zooms
      const scale = stage.scaleX();
      const dx = e.deltaX / scale;
      const dy = e.deltaY / scale;
      stage.position({
        x: stage.x() - dx,
        y: stage.y() - dy,
      });
      stage.batchDraw();
      syncDOM();
    }
  }, { passive: false });

  // ── 4. Card Mount & Interaction Bindings ──
  function ensureCardEntry(id) {
    if (cards.has(id)) return cards.get(id);

    const reg = registry.find(item => item.id === id);
    if (!reg) return null;

    const el = createArtboard(reg.title, reg.render(), { layer: reg.layer });
    el.classList.add('module-card--positioned');
    el.dataset.artboardId = id;

    const entry = {
      id,
      element: el,
      pinBtn: el.pinBtn,
      renameBtn: el.renameBtn,
      label: el.titleEl,
      dragBound: false,
    };
    cards.set(id, entry);
    return entry;
  }

  function getDisplayTitle(artboardState) {
    return artboardState.displayName || artboardState.sourceTitle;
  }

  function getProjectVisibleArtboardIds(documentState, projectId) {
    return Object.keys(documentState.artboards)
      .filter((id) => {
        const artboard = documentState.artboards[id];
        return artboard.projectId === projectId && !artboard.archived && !artboard.orphaned;
      })
      .sort((a, b) => documentState.artboards[a].sidebarOrder - documentState.artboards[b].sidebarOrder);
  }

  function getProjectToggleAnchor(documentState, projectId, projectIndex) {
    const ids = getProjectVisibleArtboardIds(documentState, projectId);
    if (!ids.length) {
      return {
        x: ARTBOARD_BASE_X,
        y: ARTBOARD_BASE_Y + projectIndex * 28,
      };
    }

    let minX = Number.POSITIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    ids.forEach((id) => {
      const position = documentState.artboards[id].position;
      if (position.x < minX) minX = position.x;
      if (position.y < minY) minY = position.y;
    });
    return {
      x: Number.isFinite(minX) ? minX : ARTBOARD_BASE_X,
      y: Number.isFinite(minY) ? Math.max(0, minY - 28) : ARTBOARD_BASE_Y + projectIndex * 28,
    };
  }

  function getSinglePinnedId(documentState) {
    const pinned = Object.keys(documentState.artboards).find((id) => {
      const entry = documentState.artboards[id];
      return entry.pinned && !entry.archived;
    });
    return pinned || null;
  }

  function attachCardInteractions(id) {
    const entry = cards.get(id);
    if (!entry || entry.dragBound) return;
    entry.dragBound = true;

    // Pin Toggle
    entry.pinBtn.addEventListener('click', (event) => {
      event.stopPropagation();
      documentStore.mutate((draft) => {
        const currentPinned = getSinglePinnedId(draft);
        Object.keys(draft.artboards).forEach((artId) => {
          draft.artboards[artId].pinned = false;
        });
        if (currentPinned !== id && !draft.artboards[id].archived) {
          draft.artboards[id].pinned = true;
        }
      });
    });

    // Rename Dialog
    entry.renameBtn.addEventListener('click', (event) => {
      event.stopPropagation();
      const currentDoc = documentStore.get();
      const artboard = currentDoc.artboards[id];
      if (!artboard) return;
      showRenameDialog(getDisplayTitle(artboard), (value) => {
        documentStore.mutate((draft) => {
          if (!draft.artboards[id]) return;
          draft.artboards[id].displayName = value;
        });
      });
    });

    // Pointer Drag Handle
    const dragHandle = entry.element.handle;
    dragHandle.addEventListener('pointerdown', (event) => {
      if (event.button !== 0) return;
      if (event.target.closest('.ff-card__pin, .ff-card__rename')) return;
      event.preventDefault();
      event.stopPropagation();

      const currentDoc = documentStore.get();
      const artboard = currentDoc.artboards[id];
      if (!artboard || artboard.archived) return;

      const startPointer = clientToWorld(event.clientX, event.clientY);
      const startPosition = { ...artboard.position };

      // Temporarily disable Konva stage panning while dragging card
      stage.draggable(false);
      entry.element.classList.add('module-card--dragging');
      const pointerId = event.pointerId;

      const handleMove = (moveEvent) => {
        if (moveEvent.pointerId !== pointerId) return;
        const pointer = clientToWorld(moveEvent.clientX, moveEvent.clientY);
        const dx = pointer.x - startPointer.x;
        const dy = pointer.y - startPointer.y;
        const nextX = Math.round(startPosition.x + dx);
        const nextY = Math.round(startPosition.y + dy);
        entry.element.style.left = `${nextX}px`;
        entry.element.style.top = `${nextY}px`;
      };

      const handleUp = (upEvent) => {
        if (upEvent.pointerId !== pointerId) return;
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);
        window.removeEventListener('pointercancel', handleUp);

        // Re-enable Konva stage panning
        stage.draggable(true);
        entry.element.classList.remove('module-card--dragging');

        const pointer = clientToWorld(upEvent.clientX, upEvent.clientY);
        const dx = pointer.x - startPointer.x;
        const dy = pointer.y - startPointer.y;
        const snappedX = Math.round((startPosition.x + dx) / SNAP) * SNAP;
        const snappedY = Math.round((startPosition.y + dy) / SNAP) * SNAP;

        documentStore.mutate((draft) => {
          if (!draft.artboards[id]) return;
          draft.artboards[id].position = { x: snappedX, y: snappedY };
        });
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
      window.addEventListener('pointercancel', handleUp);
    });
  }

  // ── 5. Project Framing and Dynamic Render Updates ──
  function renderProjectFrames(documentState) {
    const renderedIds = new Set();
    const renderedProjectIds = new Set();

    documentState.projects.forEach((project, projectIndex) => {
      const artboardIds = getProjectVisibleArtboardIds(documentState, project.id);
      
      // Inline toggle anchor
      const anchor = getProjectToggleAnchor(documentState, project.id, projectIndex);
      let frameComp = projectFrames.get(project.id);
      
      if (!frameComp) {
        frameComp = createProjectFrame(project, {
          onToggleCollapse: () => {
            documentStore.mutate((draft) => {
              const projectDraft = draft.projects.find((p) => p.id === project.id);
              if (projectDraft) projectDraft.collapsed = !projectDraft.collapsed;
            });
          }
        });
        projectFrames.set(project.id, frameComp);
      }

      frameComp.setTitle(project.name);
      frameComp.setCollapsed(project.collapsed);
      frameComp.element.style.left = `${anchor.x}px`;
      frameComp.element.style.top = `${anchor.y}px`;
      frameComp.element.style.display = 'none'; // Only toggle buttons are displayed inline in the overlay
      
      // Setup toggles directly on background
      let toggleBtn = world.querySelector(`.project-toggle-${project.id}`);
      if (!toggleBtn) {
        toggleBtn = document.createElement('button');
        toggleBtn.type = 'button';
        toggleBtn.className = `project-inline-toggle project-toggle-${project.id}`;
        toggleBtn.addEventListener('click', (event) => {
          event.stopPropagation();
          documentStore.mutate((draft) => {
            const projectDraft = draft.projects.find((p) => p.id === project.id);
            if (projectDraft) projectDraft.collapsed = !projectDraft.collapsed;
          });
        });
        world.appendChild(toggleBtn);
      }
      toggleBtn.textContent = `${project.collapsed ? '[+]' : '[-]'} ${project.name}`;
      toggleBtn.style.left = `${anchor.x}px`;
      toggleBtn.style.top = `${anchor.y}px`;
      renderedProjectIds.add(project.id);

      // Render cards
      artboardIds.forEach((id) => {
        const cardEntry = ensureCardEntry(id);
        if (!cardEntry) return;

        attachCardInteractions(id);

        const state = documentState.artboards[id];
        const title = getDisplayTitle(state);
        cardEntry.label.textContent = title;
        cardEntry.element.titleEl.textContent = title;

        const isPinned = state.pinned && !state.archived;
        cardEntry.pinBtn.classList.toggle('ff-card-pin--active', isPinned);
        cardEntry.pinBtn.setAttribute('aria-pressed', isPinned ? 'true' : 'false');
        cardEntry.pinBtn.setAttribute('aria-label', isPinned ? `Unpin "${title}"` : `Pin "${title}"`);

        cardEntry.renameBtn.hidden = false;

        cardEntry.element.style.left = `${state.position.x}px`;
        cardEntry.element.style.top = `${state.position.y}px`;
        cardEntry.element.style.display = project.collapsed ? 'none' : '';
        
        if (!cardEntry.element.parentNode) {
          world.appendChild(cardEntry.element);
        }

        renderedIds.add(id);
      });
    });

    // Cleanup unrendered cards
    cards.forEach((entry, id) => {
      if (!renderedIds.has(id)) {
        entry.element.remove();
      }
    });

    // Cleanup unrendered projects
    projectFrames.forEach((frameComp, id) => {
      if (!renderedProjectIds.has(id)) {
        frameComp.element.remove();
        projectFrames.delete(id);
      }
    });
    world.querySelectorAll('.project-inline-toggle').forEach((toggle) => {
      const match = toggle.className.match(/project-toggle-([a-zA-Z0-9_-]+)/);
      if (match && !renderedProjectIds.has(match[1])) {
        toggle.remove();
      }
    });
  }

  function applyPinSidebar(documentState) {
    const pinnedId = getSinglePinnedId(documentState);
    if (!pinnedId) {
      pinSidebar.unpinAll();
      return;
    }
    const cardEntry = cards.get(pinnedId);
    const artboardState = documentState.artboards[pinnedId];
    if (!cardEntry || !artboardState) return;

    pinSidebar.pin(
      pinnedId,
      cardEntry.element.bodyEl,
      getDisplayTitle(artboardState),
      () => {
        documentStore.mutate((draft) => {
          if (draft.artboards[pinnedId]) draft.artboards[pinnedId].pinned = false;
        });
      }
    );
  }

  // Centering Transition Easing Logic
  function animateCanvasTo(targetScale, targetX, targetY, duration = 400) {
    const startScale = stage.scaleX();
    const startX = stage.x();
    const startY = stage.y();
    const startTime = performance.now();

    function tick(now) {
      let t = Math.min((now - startTime) / duration, 1);
      t = 1 - Math.pow(1 - t, 3); // cubic ease out

      const s = startScale + (targetScale - startScale) * t;
      const x = startX + (targetX - startX) * t;
      const y = startY + (targetY - startY) * t;

      stage.scale({ x: s, y: s });
      stage.position({ x, y });
      stage.batchDraw();
      syncDOM();

      if (t < 1) requestAnimationFrame(tick);
    }
    requestAnimationFrame(tick);
  }

  function navigateToCanvasNode(targetEl, artboardId = null) {
    const rect = targetEl.getBoundingClientRect();
    const worldRect = world.getBoundingClientRect();
    const scale = stage.scaleX();
    const worldX = (rect.left - worldRect.left) / scale;
    const worldY = (rect.top - worldRect.top) / scale;
    const worldW = rect.width / scale;
    const worldH = rect.height / scale;

    // Viewport dimensions accounting for sidebars
    const leftWidth = documentStore.get().ui.leftSidebarOpen !== false ? 220 : 0;
    const rightWidth = pinSidebar.isActive() && documentStore.get().ui.pinPanelOpen !== false ? pinSidebar.getWidth() : 0;
    const vpW = window.innerWidth - leftWidth - rightWidth;
    const vpH = window.innerHeight;

    const availW = vpW * 0.8;
    const availH = vpH * 0.8;
    const fitScale = Math.min(availW / worldW, availH / worldH);
    const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, fitScale));

    const vpCenterX = leftWidth + vpW / 2;
    const vpCenterY = vpH / 2;
    const centerX = worldX + worldW / 2;
    const centerY = worldY + worldH / 2;

    animateCanvasTo(newScale, vpCenterX - centerX * newScale, vpCenterY - centerY * newScale);
    if (artboardId) onSelectActiveArtboard?.(artboardId);
  }

  function centerOn(id) {
    const entry = cards.get(id);
    if (!entry) return;
    navigateToCanvasNode(entry.element, id);
  }

  // ── 6. UI Helpers, Zoom Panel & Hints ──
  function showCopyToast(message) {
    if (!copyToast) {
      copyToast = document.createElement('div');
      copyToast.className = 'canvas-copy-toast';
      document.body.appendChild(copyToast);
    }
    copyToast.textContent = message;
    copyToast.classList.add('canvas-copy-toast--visible');
    clearTimeout(showCopyToast._timer);
    showCopyToast._timer = setTimeout(() => {
      copyToast?.classList.remove('canvas-copy-toast--visible');
    }, 1300);
  }

  // Zoom control panel
  const zoomUI = document.createElement('div');
  zoomUI.className = 'zoom-controls';

  const zoomOut = document.createElement('button');
  zoomOut.textContent = '−';
  zoomOut.addEventListener('click', (e) => {
    e.stopPropagation();
    const cur = stage.scaleX();
    const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, cur / 1.3));
    animateCanvasTo(newScale, stage.x(), stage.y());
  });

  const zoomLevel = document.createElement('span');
  zoomLevel.className = 'zoom-level';
  zoomLevel.textContent = '100%';

  const zoomIn = document.createElement('button');
  zoomIn.textContent = '+';
  zoomIn.addEventListener('click', (e) => {
    e.stopPropagation();
    const cur = stage.scaleX();
    const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, cur * 1.3));
    animateCanvasTo(newScale, stage.x(), stage.y());
  });

  const zoomFit = document.createElement('button');
  zoomFit.textContent = '⊡';
  zoomFit.title = 'Reset';
  zoomFit.addEventListener('click', (e) => {
    e.stopPropagation();
    animateCanvasTo(1, 0, 0);
  });

  zoomUI.appendChild(zoomOut);
  zoomUI.appendChild(zoomLevel);
  zoomUI.appendChild(zoomIn);
  zoomUI.appendChild(zoomFit);
  document.body.appendChild(zoomUI);

  // Canvas interaction hints
  const hint = document.createElement('div');
  hint.className = 'canvas-hint';
  hint.textContent = 'Drag grid space to pan · Pinch/ctrl+scroll to zoom · ⊡ to reset';
  document.body.appendChild(hint);
  setTimeout(() => hint.classList.add('hidden'), 5000);

  // Resize handler
  window.addEventListener('resize', () => {
    applyCanvasLayout();
  });

  // ── 7. Render triggers ──
  function render(nextDocument) {
    leftSidebarOpen = nextDocument.ui.leftSidebarOpen !== false;
    pinPanelOpen = nextDocument.ui.pinPanelOpen !== false;
    pinSidebarWidth = nextDocument.ui.pinPanelWidth || pinSidebar.getWidth();
    pinSidebarPinned = getSinglePinnedId(nextDocument) !== null;

    const sidebarEl = document.getElementById('sidebar');
    sidebarEl?.classList.toggle('artboard-sidebar--hidden', !leftSidebarOpen);

    renderProjectFrames(nextDocument);
    applyPinSidebar(nextDocument);
    applyCanvasLayout();
  }

  // Initial layout call
  setTimeout(() => {
    applyCanvasLayout();
  }, 0);

  return {
    render,
    centerOn,
    navigateToCanvasNode,
    showCopyToast,
    onSidebarLayoutChange(width, pinned, meta) {
      pinSidebarWidth = width;
      if (typeof pinned === 'boolean') {
        pinSidebarPinned = pinned;
        if (!pinned) pinPanelOpen = true;
      }
      applyCanvasLayout();
    }
  };
}
