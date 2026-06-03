const DOC_VERSION = 1;
const DEFAULT_PROJECT_ID = 'default';
const DOCUMENT_KEY = 'fourfour:canvas:document';
const ARTBOARD_BASE_X = 48;
const ARTBOARD_BASE_Y = 56;
const ARTBOARD_COL_SPACING = 360;
const ARTBOARD_ROW_SPACING = 300;

const DEFAULT_UI = {
  leftSidebarOpen: true,
  pinPanelOpen: true,
  pinPanelWidth: 280,
  activeProjectId: DEFAULT_PROJECT_ID,
  layoutVersion: 1,
};

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function debounce(fn, ms) {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

function nowIso() {
  return new Date().toISOString();
}

function slugify(input) {
  const base = String(input || '')
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return base || 'project';
}

export function createEmptyDocument() {
  return {
    version: DOC_VERSION,
    projects: [
      {
        id: DEFAULT_PROJECT_ID,
        name: 'fourfour',
        collapsed: false,
        frame: { x: 0, y: 0 },
        createdAt: nowIso(),
      },
    ],
    artboards: {},
    notes: [],
    ui: { ...DEFAULT_UI },
  };
}

function ensureDefaultProject(doc) {
  const hasDefault = doc.projects.some((project) => project.id === DEFAULT_PROJECT_ID);
  if (!hasDefault) {
    doc.projects.unshift({
      id: DEFAULT_PROJECT_ID,
      name: 'fourfour',
      collapsed: false,
      frame: { x: 0, y: 0 },
      createdAt: nowIso(),
    });
  }
}

function normalizeProject(project) {
  return {
    id: typeof project?.id === 'string' && project.id ? project.id : slugify(project?.name || 'project'),
    name: typeof project?.name === 'string' && project.name ? project.name : 'Untitled Project',
    collapsed: Boolean(project?.collapsed),
    frame: {
      x: Number.isFinite(project?.frame?.x) ? project.frame.x : 0,
      y: Number.isFinite(project?.frame?.y) ? project.frame.y : 0,
    },
    createdAt: typeof project?.createdAt === 'string' ? project.createdAt : nowIso(),
  };
}

function normalizeUi(ui) {
  return {
    leftSidebarOpen: ui?.leftSidebarOpen !== false,
    pinPanelOpen: ui?.pinPanelOpen !== false,
    pinPanelWidth: Number.isFinite(ui?.pinPanelWidth) ? Math.max(220, Math.min(800, ui.pinPanelWidth)) : DEFAULT_UI.pinPanelWidth,
    activeProjectId: typeof ui?.activeProjectId === 'string' && ui.activeProjectId ? ui.activeProjectId : DEFAULT_PROJECT_ID,
    layoutVersion: Number.isFinite(ui?.layoutVersion) ? ui.layoutVersion : DEFAULT_UI.layoutVersion,
  };
}

function normalizeNotes(notes) {
  if (!Array.isArray(notes)) return [];
  return notes
    .filter((note) => note && note.target?.type === 'artboard' && typeof note.target.artboardId === 'string')
    .map((note) => ({
      id: typeof note.id === 'string' && note.id ? note.id : `note-${Math.random().toString(16).slice(2)}`,
      target: { type: 'artboard', artboardId: note.target.artboardId },
      text: typeof note.text === 'string' ? note.text : '',
      canvasOffset: {
        x: Number.isFinite(note.canvasOffset?.x) ? note.canvasOffset.x : 8,
        y: Number.isFinite(note.canvasOffset?.y) ? note.canvasOffset.y : -32,
      },
      createdAt: typeof note.createdAt === 'string' ? note.createdAt : nowIso(),
      updatedAt: typeof note.updatedAt === 'string' ? note.updatedAt : nowIso(),
    }));
}

function normalizeArtboard(id, artboard, projectIds) {
  const fallbackProjectId = projectIds.has(DEFAULT_PROJECT_ID)
    ? DEFAULT_PROJECT_ID
    : [...projectIds][0];
  const projectId = typeof artboard?.projectId === 'string' && projectIds.has(artboard.projectId)
    ? artboard.projectId
    : fallbackProjectId;
  return {
    sourceTitle: typeof artboard?.sourceTitle === 'string' && artboard.sourceTitle
      ? artboard.sourceTitle
      : id,
    displayName: typeof artboard?.displayName === 'string' && artboard.displayName.trim()
      ? artboard.displayName.trim()
      : null,
    projectId,
    position: {
      x: Number.isFinite(artboard?.position?.x) ? artboard.position.x : 24,
      y: Number.isFinite(artboard?.position?.y) ? artboard.position.y : 48,
    },
    size: artboard?.size && Number.isFinite(artboard.size.width) && Number.isFinite(artboard.size.height)
      ? { width: artboard.size.width, height: artboard.size.height }
      : null,
    archived: Boolean(artboard?.archived),
    pinned: Boolean(artboard?.pinned),
    sidebarOrder: Number.isFinite(artboard?.sidebarOrder) ? artboard.sidebarOrder : 0,
    orphaned: Boolean(artboard?.orphaned),
  };
}

function normalizeSidebarOrder(doc) {
  const projectIds = doc.projects.map((project) => project.id);
  projectIds.forEach((projectId) => {
    const ids = Object.keys(doc.artboards)
      .filter((id) => doc.artboards[id].projectId === projectId)
      .sort((a, b) => doc.artboards[a].sidebarOrder - doc.artboards[b].sidebarOrder);
    ids.forEach((id, index) => {
      doc.artboards[id].sidebarOrder = index;
    });
  });
}

function ensureSinglePin(doc) {
  const pinned = Object.keys(doc.artboards).filter((id) => doc.artboards[id].pinned && !doc.artboards[id].archived);
  if (pinned.length <= 1) return;
  pinned.slice(1).forEach((id) => {
    doc.artboards[id].pinned = false;
  });
}

function ensureVisibleRegistryArtboards(doc, registry) {
  const registryIds = registry.map((entry) => entry.id);
  const visibleCount = registryIds.filter((id) => {
    const artboard = doc.artboards[id];
    return artboard && !artboard.archived && !artboard.orphaned;
  }).length;

  if (visibleCount > 0) return;

  registryIds.forEach((id, index) => {
    const artboard = doc.artboards[id];
    if (!artboard) return;
    artboard.archived = false;
    artboard.orphaned = false;
    if (!artboard.position) {
      artboard.position = {
        x: ARTBOARD_BASE_X + (index % 3) * ARTBOARD_COL_SPACING,
        y: ARTBOARD_BASE_Y + Math.floor(index / 3) * ARTBOARD_ROW_SPACING,
      };
    }
    if (!Number.isFinite(artboard.sidebarOrder)) {
      artboard.sidebarOrder = index;
    }
  });
}

export function normalizeDocument(raw) {
  const base = raw && typeof raw === 'object' ? raw : createEmptyDocument();
  const doc = {
    version: DOC_VERSION,
    projects: Array.isArray(base.projects) ? base.projects.map(normalizeProject) : [],
    artboards: {},
    notes: normalizeNotes(base.notes),
    ui: normalizeUi(base.ui),
  };

  ensureDefaultProject(doc);

  const projectIds = new Set(doc.projects.map((project) => project.id));
  const rawArtboards = base.artboards && typeof base.artboards === 'object' ? base.artboards : {};
  Object.keys(rawArtboards).forEach((id) => {
    doc.artboards[id] = normalizeArtboard(id, rawArtboards[id], projectIds);
  });

  if (!projectIds.has(doc.ui.activeProjectId)) {
    doc.ui.activeProjectId = DEFAULT_PROJECT_ID;
  }

  normalizeSidebarOrder(doc);
  ensureSinglePin(doc);
  return doc;
}

function getProjectArtboardCount(doc, projectId) {
  return Object.values(doc.artboards).filter((entry) => entry.projectId === projectId).length;
}

function getAutoPosition(doc, projectId) {
  const index = getProjectArtboardCount(doc, projectId);
  const col = index % 3;
  const row = Math.floor(index / 3);
  return {
    x: ARTBOARD_BASE_X + col * ARTBOARD_COL_SPACING,
    y: ARTBOARD_BASE_Y + row * ARTBOARD_ROW_SPACING,
  };
}

function syncRegistry(doc, registry) {
  const projectIds = new Set(doc.projects.map((project) => project.id));
  registry.forEach((entry) => {
    if (!doc.artboards[entry.id]) {
      const projectId = projectIds.has(doc.ui.activeProjectId) ? doc.ui.activeProjectId : DEFAULT_PROJECT_ID;
      doc.artboards[entry.id] = {
        sourceTitle: entry.sourceTitle,
        displayName: null,
        projectId,
        position: getAutoPosition(doc, projectId),
        size: null,
        archived: false,
        pinned: false,
        sidebarOrder: getProjectArtboardCount(doc, projectId),
        orphaned: false,
      };
      return;
    }
    doc.artboards[entry.id].sourceTitle = entry.sourceTitle;
    doc.artboards[entry.id].orphaned = false;
  });

  const registryIds = new Set(registry.map((entry) => entry.id));
  Object.keys(doc.artboards).forEach((id) => {
    if (!registryIds.has(id)) {
      doc.artboards[id].orphaned = true;
    }
    if (!projectIds.has(doc.artboards[id].projectId)) {
      doc.artboards[id].projectId = DEFAULT_PROJECT_ID;
    }
    if (!doc.artboards[id].position) {
      doc.artboards[id].position = getAutoPosition(doc, doc.artboards[id].projectId);
    }
  });

  normalizeSidebarOrder(doc);
  ensureSinglePin(doc);
  ensureVisibleRegistryArtboards(doc, registry);
  reflowLegacyDenseGrid(doc, registry);
  resolveAllOverlaps(doc);
  return doc;
}

function isLegacyDenseGridPosition(position) {
  if (!position || !Number.isFinite(position.x) || !Number.isFinite(position.y)) return false;
  const xOffset = position.x - 48;
  const yOffset = position.y - 56;
  return xOffset % 520 === 0 && yOffset % 360 === 0;
}

function reflowLegacyDenseGrid(doc, registry) {
  const registryIds = new Set(registry.map((entry) => entry.id));
  doc.projects.forEach((project) => {
    const ids = Object.keys(doc.artboards)
      .filter((id) => {
        const artboard = doc.artboards[id];
        return (
          artboard.projectId === project.id &&
          !artboard.archived &&
          !artboard.orphaned &&
          registryIds.has(id)
        );
      })
      .sort((a, b) => doc.artboards[a].sidebarOrder - doc.artboards[b].sidebarOrder);

    if (!ids.length) return;
    const allLegacy = ids.every((id) => isLegacyDenseGridPosition(doc.artboards[id].position));
    if (!allLegacy) return;

    ids.forEach((id, index) => {
      const col = index % 3;
      const row = Math.floor(index / 3);
      doc.artboards[id].position = {
        x: ARTBOARD_BASE_X + col * ARTBOARD_COL_SPACING,
        y: ARTBOARD_BASE_Y + row * ARTBOARD_ROW_SPACING,
      };
      doc.artboards[id].sidebarOrder = index;
    });
  });
}

export function resolveAllOverlaps(doc) {
  const GAP = 24;

  doc.projects.forEach((project) => {
    const ids = Object.keys(doc.artboards).filter((id) => {
      const artboard = doc.artboards[id];
      return artboard.projectId === project.id && !artboard.archived && !artboard.orphaned;
    });

    // Sort by coordinate flow (top-to-bottom, left-to-right) so top-left items act as anchors
    ids.sort((a, b) => {
      const posA = doc.artboards[a].position;
      const posB = doc.artboards[b].position;
      if (posA.y !== posB.y) return posA.y - posB.y;
      return posA.x - posB.x;
    });

    const positionedRects = [];

    ids.forEach((id) => {
      const artboard = doc.artboards[id];
      const size = artboard.size || { width: 320, height: 260 };
      
      let x = artboard.position.x;
      let y = artboard.position.y;
      let w = size.width;
      let h = size.height;

      let hasOverlap = true;
      let iterations = 0;
      while (hasOverlap && iterations < 100) {
        hasOverlap = false;
        const rectB = { left: x, top: y, right: x + w, bottom: y + h };

        for (const rectA of positionedRects) {
          const intersects = rectA.left < rectB.right &&
                             rectA.right > rectB.left &&
                             rectA.top < rectB.bottom &&
                             rectA.bottom > rectB.top;
          
          if (intersects) {
            hasOverlap = true;
            const shiftRight = rectA.right + GAP - rectB.left;
            const shiftDown = rectA.bottom + GAP - rectB.top;

            if (shiftRight < shiftDown) {
              x += shiftRight;
            } else {
              y += shiftDown;
            }
            x = Math.round(x / 8) * 8;
            y = Math.round(y / 8) * 8;
            break;
          }
        }
        iterations++;
      }

      positionedRects.push({
        id,
        left: x,
        top: y,
        right: x + w,
        bottom: y + h
      });

      if (x !== artboard.position.x || y !== artboard.position.y) {
        artboard.position = { x, y };
      }
    });
  });
}

export function loadInitialDocument(registry) {
  let raw = null;
  try {
    const data = localStorage.getItem(DOCUMENT_KEY);
    raw = data ? JSON.parse(data) : null;
  } catch (err) {
    console.error('Failed to load canvas document from localStorage', err);
  }

  if (!raw) {
    raw = createEmptyDocument();
  }

  const normalized = normalizeDocument(raw);
  const synced = syncRegistry(normalized, registry);
  const finalDoc = normalizeDocument(synced);

  try {
    localStorage.setItem(DOCUMENT_KEY, JSON.stringify(finalDoc));
  } catch (err) {
    console.error('Failed to write initialized canvas document', err);
  }

  return finalDoc;
}

export function createDocumentStore(initialDoc) {
  let doc = clone(initialDoc);
  const listeners = new Set();
  let hasPendingSave = false;

  const saveDebounced = debounce(() => {
    if (!hasPendingSave) return;
    hasPendingSave = false;
    try {
      localStorage.setItem(DOCUMENT_KEY, JSON.stringify(doc));
    } catch (err) {
      console.error('Failed to persist canvas document mutation', err);
    }
  }, 400);

  function notify() {
    listeners.forEach((listener) => listener(clone(doc)));
  }

  function mutate(mutator, { persist = true } = {}) {
    const draft = clone(doc);
    mutator(draft);
    doc = normalizeDocument(draft);
    if (persist) {
      hasPendingSave = true;
      saveDebounced();
    }
    notify();
  }

  return {
    get() {
      return clone(doc);
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    mutate,
    saveNow() {
      hasPendingSave = false;
      try {
        localStorage.setItem(DOCUMENT_KEY, JSON.stringify(doc));
      } catch (err) {
        console.error('Failed to save document immediately', err);
      }
    },
    createProject(name) {
      const base = slugify(name);
      let id = base;
      let counter = 2;
      const projectIds = new Set(doc.projects.map((project) => project.id));
      while (projectIds.has(id)) {
        id = `${base}-${counter}`;
        counter += 1;
      }
      mutate((draft) => {
        const offset = draft.projects.length;
        draft.projects.push({
          id,
          name: name.trim() || 'Untitled Project',
          collapsed: false,
          frame: { x: 40 + offset * 40, y: 32 + offset * 24 },
          createdAt: nowIso(),
        });
      });
      return id;
    },
    resolveOverlaps() {
      mutate((draft) => {
        resolveAllOverlaps(draft);
      });
    },
  };
}
