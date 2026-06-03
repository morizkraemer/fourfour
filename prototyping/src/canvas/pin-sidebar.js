import './pin-sidebar.css';

export const pinIconHtml = `
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M12 17v5" />
    <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1z" />
  </svg>
`;

const DEFAULT_WIDTH = 280;
const MIN_WIDTH = 220;
const MAX_WIDTH = 800;
const WIDTH_KEY = 'fourfour-canvas-pin-sidebar-width';

function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

function loadWidth(initialWidth) {
  if (Number.isFinite(initialWidth)) {
    return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, initialWidth));
  }
  try {
    const raw = localStorage.getItem(WIDTH_KEY);
    const n = raw ? Number(raw) : DEFAULT_WIDTH;
    return Number.isFinite(n) ? Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, n)) : DEFAULT_WIDTH;
  } catch {
    return DEFAULT_WIDTH;
  }
}

export function createPinSidebar({ onLayoutChange, initialWidth } = {}) {
  const sidebar = document.createElement('aside');
  sidebar.className = 'pin-sidebar pin-sidebar--hidden';

  let sidebarWidth = loadWidth(initialWidth);
  let pinnedKey = null;
  /** @type {null | { sourceKey: string, sourceEl: HTMLElement, panel: HTMLElement, scaleWrap: HTMLElement, observer: ResizeObserver | null, refresh: Function, titleEl: HTMLElement, unpinBtn: HTMLButtonElement }} */
  let entry = null;

  const resizeHandle = document.createElement('div');
  resizeHandle.className = 'pin-sidebar-resize';
  resizeHandle.setAttribute('aria-hidden', 'true');
  sidebar.appendChild(resizeHandle);

  const header = document.createElement('div');
  header.className = 'pin-sidebar-header';
  header.textContent = 'Pinned';
  sidebar.appendChild(header);

  const list = document.createElement('div');
  list.className = 'pin-sidebar-list';
  sidebar.appendChild(list);

  function applyWidth() {
    sidebar.style.width = `${sidebarWidth}px`;
  }

  function emitLayoutChange(meta = {}) {
    if (typeof onLayoutChange === 'function') {
      onLayoutChange(pinnedKey ? sidebarWidth : 0, pinnedKey !== null, meta);
    }
  }

  function notifyLayout() {
    const visible = pinnedKey !== null;
    sidebar.classList.toggle('pin-sidebar--hidden', !visible);
    emitLayoutChange();
  }

  function syncPreview() {
    if (!entry) return;
    const { sourceEl, scaleWrap } = entry;
    const preview = entry.panel.querySelector('.pin-panel-preview');
    if (!sourceEl.isConnected || !preview) return;

    scaleWrap.innerHTML = '';
    scaleWrap.appendChild(sourceEl.cloneNode(true));

    const sourceW = Number(sourceEl.dataset.width) || sourceEl.offsetWidth || 200;
    const sourceH = Number(sourceEl.dataset.height) || sourceEl.offsetHeight || 200;
    const padX = 20;
    const availW = Math.max(preview.clientWidth - padX, 80);
    const scale = Math.min(1, availW / sourceW);
    const scaledW = Math.ceil(sourceW * scale);
    const scaledH = Math.ceil(sourceH * scale);

    const slot = preview.querySelector('.pin-panel-preview-slot');
    if (slot) {
      slot.style.width = `${scaledW}px`;
      slot.style.height = `${scaledH}px`;
    }

    scaleWrap.style.width = `${sourceW}px`;
    scaleWrap.style.height = `${sourceH}px`;
    scaleWrap.style.transform = `scale(${scale})`;
  }

  function clearPanel() {
    if (!entry) return;
    entry.sourceEl.removeEventListener('artboard-resize', entry.refresh);
    entry.observer?.disconnect();
    entry.panel.remove();
    entry = null;
    pinnedKey = null;
  }

  function pin(sourceKey, sourceEl, title, onUnpin) {
    if (pinnedKey === sourceKey) return;

    clearPanel();
    list.innerHTML = '';

    const panel = document.createElement('div');
    panel.className = 'pin-panel';
    panel.dataset.sourceKey = sourceKey;

    const panelHeader = document.createElement('div');
    panelHeader.className = 'pin-panel-header';

    const titleEl = document.createElement('span');
    titleEl.className = 'pin-panel-title';
    titleEl.textContent = title;
    panelHeader.appendChild(titleEl);

    const unpinBtn = document.createElement('button');
    unpinBtn.type = 'button';
    unpinBtn.className = 'pin-panel-unpin';
    unpinBtn.setAttribute('aria-label', `Unpin "${title}"`);
    unpinBtn.innerHTML = pinIconHtml;
    unpinBtn.addEventListener('click', () => onUnpin?.());
    panelHeader.appendChild(unpinBtn);

    const preview = document.createElement('div');
    preview.className = 'pin-panel-preview';

    const slot = document.createElement('div');
    slot.className = 'pin-panel-preview-slot';

    const scaleWrap = document.createElement('div');
    scaleWrap.className = 'pin-panel-scale';
    slot.appendChild(scaleWrap);
    preview.appendChild(slot);

    panel.appendChild(panelHeader);
    panel.appendChild(preview);
    list.appendChild(panel);

    const refresh = debounce(() => syncPreview(), 80);

    const observer = typeof ResizeObserver !== 'undefined'
      ? new ResizeObserver(refresh)
      : null;

    if (observer) observer.observe(sourceEl);
    sourceEl.addEventListener('artboard-resize', refresh);

    entry = { sourceKey, sourceEl, panel, scaleWrap, observer, refresh, titleEl, unpinBtn };
    pinnedKey = sourceKey;
    applyWidth();
    syncPreview();
    notifyLayout();
  }

  function unpin(sourceKey) {
    if (pinnedKey !== sourceKey) return;
    clearPanel();
    notifyLayout();
  }

  function unpinAll() {
    if (!pinnedKey) return;
    clearPanel();
    notifyLayout();
  }

  let resizing = false;

  resizeHandle.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    resizing = true;
    resizeHandle.setPointerCapture(e.pointerId);
    document.body.classList.add('pin-sidebar-resizing');
  });

  resizeHandle.addEventListener('pointermove', (e) => {
    if (!resizing) return;
    const next = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, window.innerWidth - e.clientX));
    sidebarWidth = next;
    applyWidth();
    if (pinnedKey) emitLayoutChange({ widthChanged: true });
    syncPreview();
  });

  function endResize(e) {
    if (!resizing) return;
    resizing = false;
    if (resizeHandle.hasPointerCapture(e.pointerId)) {
      resizeHandle.releasePointerCapture(e.pointerId);
    }
    document.body.classList.remove('pin-sidebar-resizing');
    try {
      localStorage.setItem(WIDTH_KEY, String(sidebarWidth));
    } catch {
      /* ignore */
    }
  }

  resizeHandle.addEventListener('pointerup', endResize);
  resizeHandle.addEventListener('pointercancel', endResize);

  applyWidth();

  return {
    element: sidebar,
    setWidth(width) {
      if (!Number.isFinite(width)) return;
      const next = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width));
      if (Math.abs(next - sidebarWidth) < 1) return;
      sidebarWidth = next;
      applyWidth();
      emitLayoutChange({ widthChanged: true });
      syncPreview();
    },
    getWidth() {
      return sidebarWidth;
    },
    isActive() {
      return pinnedKey !== null;
    },
    isPinned(sourceKey) {
      return pinnedKey === sourceKey;
    },
    getPinnedKey() {
      return pinnedKey;
    },
    getPinnedKeys() {
      return pinnedKey ? [pinnedKey] : [];
    },
    pin,
    unpin,
    unpinAll,
    updateTitle(sourceKey, title) {
      if (pinnedKey !== sourceKey || !entry) return;
      entry.titleEl.textContent = title;
      entry.unpinBtn.setAttribute('aria-label', `Unpin "${title}"`);
    },
    refresh() {
      syncPreview();
    },
    refreshAll() {
      syncPreview();
    },
  };
}
