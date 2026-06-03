import '../src/design-system/index.js';
import { createCanvas } from './canvas/canvas.js';
import { createSidebar } from './canvas/sidebar.js';
import { createPinSidebar } from './canvas/pin-sidebar.js';
import { loadInitialDocument, createDocumentStore } from './canvas/document-store.js';
import { showTextDialog } from './canvas/sidebar-context-menu.js';
import { getPrimaryArtboardNote, upsertArtboardPrimaryNote } from './canvas/notes.js';

// Auto-discovery registry: discovers all *.artboard.js files in src/artboards
const modules = import.meta.glob('./artboards/**/*.artboard.js', { eager: true });
const registry = Object.entries(modules).map(([path, mod]) => {
  const id = path.split('/').pop().replace('.artboard.js', '');
  return {
    id,
    sourceTitle: mod.meta?.title ?? id,
    layer: mod.meta?.layer ?? 'primitive',
    render: mod.default,
  };
});

// Load document state
const bootstrapDoc = loadInitialDocument(registry);
const documentStore = createDocumentStore(bootstrapDoc);

// Instantiate right pin sidebar panel
const pinSidebar = createPinSidebar({
  onLayoutChange(width, pinned, meta) {
    canvas.onSidebarLayoutChange(width, pinned, meta);
  },
});
document.body.appendChild(pinSidebar.element);

// Instantiate central canvas
const canvas = createCanvas({
  viewport: document.getElementById('viewport'),
  documentStore,
  pinSidebar,
  registry,
  onSelectActiveArtboard: (id) => sidebar.setActiveArtboard(id),
});

// Helper for mapping document structure to sidebar explorer
function buildSidebarModel(documentState) {
  const artboardsByProject = {};
  documentState.projects.forEach((project) => {
    artboardsByProject[project.id] = [];
  });

  const archivedArtboards = [];
  Object.entries(documentState.artboards).forEach(([id, artboard]) => {
    if (artboard.orphaned) return;
    
    const regEntry = registry.find(r => r.id === id);
    const layer = regEntry ? regEntry.layer : 'primitive';

    const modelEntry = {
      id,
      sourceTitle: artboard.sourceTitle,
      displayName: artboard.displayName,
      projectId: artboard.projectId,
      pinned: artboard.pinned && !artboard.archived,
      noteCount: (documentState.notes || []).filter(n => n.target?.artboardId === id).length,
      sidebarOrder: artboard.sidebarOrder,
      layer,
    };
    if (artboard.archived) archivedArtboards.push(modelEntry);
    else artboardsByProject[artboard.projectId]?.push(modelEntry);
  });

  Object.values(artboardsByProject).forEach((rows) => {
    rows.sort((a, b) => a.sidebarOrder - b.sidebarOrder);
  });
  archivedArtboards.sort((a, b) => a.id.localeCompare(b.id));

  const primitiveCategories = [
    { sourceKey: 'primitive', title: 'Primitives' },
    { sourceKey: 'module', title: 'Modules' },
    { sourceKey: 'composite', title: 'Composites' },
  ];

  return {
    projects: documentState.projects,
    activeProjectId: documentState.ui.activeProjectId,
    artboardsByProject,
    archivedArtboards,
    primitives: primitiveCategories,
  };
}

// Instantiate left tree sidebar explorer
const sidebar = createSidebar({
  onCreateProject(name) {
    const newId = documentStore.createProject(name);
    if (!newId) return;
    documentStore.mutate((draft) => {
      draft.ui.activeProjectId = newId;
    });
  },
  onSelectActiveProject(projectId) {
    documentStore.mutate((draft) => {
      draft.ui.activeProjectId = projectId;
    });
  },
  onToggleProjectCollapse(projectId) {
    documentStore.mutate((draft) => {
      const project = draft.projects.find((item) => item.id === projectId);
      if (project) project.collapsed = !project.collapsed;
    });
  },
  onNavigatePrimitive(sourceKey) {
    const foundCard = Array.from(document.querySelectorAll(`.ff-card[data-layer="${sourceKey}"]`))[0];
    if (foundCard) {
      canvas.navigateToCanvasNode(foundCard);
    }
  },
  onNavigateArtboard(artboardId) {
    canvas.centerOn(artboardId);
  },
  onRenameArtboard(artboardId, newName) {
    documentStore.mutate((draft) => {
      if (!draft.artboards[artboardId]) return;
      draft.artboards[artboardId].displayName = newName;
    });
  },
  onArchiveArtboard(artboardId) {
    documentStore.mutate((draft) => {
      if (!draft.artboards[artboardId]) return;
      draft.artboards[artboardId].archived = true;
      draft.artboards[artboardId].pinned = false;
    });
  },
  onRestoreArtboard(artboardId) {
    documentStore.mutate((draft) => {
      if (!draft.artboards[artboardId]) return;
      draft.artboards[artboardId].archived = false;
    });
  },
  onTogglePin(artboardId) {
    documentStore.mutate((draft) => {
      const pinned = Object.keys(draft.artboards).find((id) => draft.artboards[id].pinned && !draft.artboards[id].archived);
      Object.keys(draft.artboards).forEach((id) => {
        draft.artboards[id].pinned = false;
      });
      if (pinned !== artboardId && !draft.artboards[artboardId]?.archived) {
        draft.artboards[artboardId].pinned = true;
      }
    });
  },
  onCopyArtboard(artboardId, options = {}) {
    const state = documentStore.get();
    const artboard = state.artboards[artboardId];
    if (!artboard) return;
    const project = state.projects.find((p) => p.id === artboard.projectId);
    const displayName = artboard.displayName || artboard.sourceTitle;
    const text = options.extended 
      ? `Artboard: ${artboardId}\nName: ${displayName}\nProject: ${project?.name || artboard.projectId}`
      : `${artboardId} - ${displayName}`;
    navigator.clipboard?.writeText(text).catch(() => {});
    canvas.showCopyToast(`Copied ${artboardId}`);
  },
  onMoveArtboardProject(artboardId, projectId) {
    documentStore.mutate((draft) => {
      const artboard = draft.artboards[artboardId];
      if (artboard) {
        artboard.projectId = projectId;
        artboard.sidebarOrder = Object.values(draft.artboards).filter((a) => a.projectId === projectId).length;
      }
    });
  },
  onEditArtboardNote(artboardId) {
    const state = documentStore.get();
    const note = getPrimaryArtboardNote(state, artboardId);
    showTextDialog({
      title: `Note: ${artboardId}`,
      initialValue: note?.text || '',
      placeholder: 'Add a note for this artboard...',
      confirmLabel: 'Save',
      onSubmit: (value) => {
        documentStore.mutate((draft) => {
          upsertArtboardPrimaryNote(draft, artboardId, value);
        });
      },
    });
  },
  onReorderProjectArtboards(projectId, orderedIds) {
    documentStore.mutate((draft) => {
      orderedIds.forEach((id, index) => {
        if (draft.artboards[id] && draft.artboards[id].projectId === projectId) {
          draft.artboards[id].sidebarOrder = index;
        }
      });
    });
  },
});

document.body.appendChild(sidebar.element);

// Subscribe to store mutations to trigger re-renders
documentStore.subscribe((state) => {
  canvas.render(state);
  sidebar.render(buildSidebarModel(state));
});

// Initial Render
const initialState = documentStore.get();
canvas.render(initialState);
sidebar.render(buildSidebarModel(initialState));

// Resolve card overlaps on reload after layout finishes rendering
setTimeout(() => {
  documentStore.resolveOverlaps();
}, 200);
