// Pioneer Test UI — app.js
// Vanilla JS, no framework. All Tauri calls via window.__TAURI__.core.invoke()

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { open: dialogOpen } = window.__TAURI__.dialog;

// ── State ──────────────────────────────────────────────────────────────────
let tracks = [];
let playlists = [];
let selectedTrackIds = new Set();
let outputDir = null;
let nextPlaylistId = 1;
let analyzing = false;
let syncing = false;
let usbTracks = [];
let usbPlaylists = [];

// Right panel mode: null | 'usb' | { playlistId: number }
let rightPanelMode = null;

// ── Init ───────────────────────────────────────────────────────────────────
async function init() {
    // Show version
    try {
        const ver = await invoke('app_version');
        document.getElementById('version-badge').textContent = 'v' + ver;
    } catch (_) {}

    // Show library path
    loadLibraryPath();

    // Restore persisted state before anything else
    try {
        const loaded = await invoke('load_state');
        if (loaded.tracks.length > 0) {
            tracks = loaded.tracks;
            renderTracks();
        }
        if (loaded.playlists.length > 0) {
            playlists = loaded.playlists.map(p => ({
                id: p.id,
                name: p.name,
                trackIds: p.track_ids,
            }));
            nextPlaylistId = Math.max(...playlists.map(p => p.id)) + 1;
            renderPlaylists();
        }
    } catch (err) {
        console.error('load_state failed:', err);
    }

    loadVolumes();
    setupDragDrop();
    setupWaveformInteraction();
    updateLibrarySubtitle();

    // Click outside context menu to close it
    document.addEventListener('click', () => {
        document.getElementById('contextMenu').classList.remove('visible');
    });

    // Sidebar: "All Tracks" row
    document.getElementById('sidebar-all-tracks').addEventListener('click', () => {
        setSidebarActive('sidebar-all-tracks');
        rightPanelMode = null;
        showRightPanel(false);
    });

    listen('analysis-progress', (event) => {
        const { current, total, message } = event.payload;
        showProgress(current, total, message);
    });

    listen('write-complete', (event) => {
        hideProgress();
        syncing = false;
        setButtonStates();
        loadUsbContents();
        const r = event.payload;
        const parts = [];
        if (r.tracks_added > 0) parts.push(r.tracks_added + ' added');
        if (r.tracks_replaced > 0) parts.push(r.tracks_replaced + ' replaced');
        if (r.tracks_updated > 0) parts.push(r.tracks_updated + ' updated');
        if (r.tracks_removed > 0) parts.push(r.tracks_removed + ' removed');
        if (r.tracks_unchanged > 0) parts.push(r.tracks_unchanged + ' unchanged');
        alert('Sync complete: ' + (parts.length > 0 ? parts.join(', ') : 'no changes'));
    });

    // Context menu actions
    document.querySelector('#contextMenu [data-action="open-in-right"]').addEventListener('click', () => {
        const menu = document.getElementById('contextMenu');
        if (menu._playlistId != null) {
            showPlaylistInRightPanel(menu._playlistId);
        }
    });
    document.querySelector('#contextMenu [data-action="delete-playlist"]').addEventListener('click', () => {
        const menu = document.getElementById('contextMenu');
        if (menu._playlistId != null) {
            deletePlaylist(menu._playlistId);
        }
    });
}

// ── Sidebar helpers ────────────────────────────────────────────────────────
function setSidebarActive(id) {
    document.querySelectorAll('.sidebar-row').forEach(r => r.classList.remove('active'));
    document.querySelectorAll('.usb-volume-row').forEach(r => r.classList.remove('active'));
    const el = document.getElementById(id);
    if (el) el.classList.add('active');
}

// ── Right panel visibility ─────────────────────────────────────────────────
function showRightPanel(visible) {
    const main = document.getElementById('main');
    main.classList.toggle('single-panel', !visible);
}

function closeRightPanel() {
    showRightPanel(false);
    rightPanelMode = null;
    // Deactivate USB rows
    document.querySelectorAll('.usb-volume-row').forEach(r => r.classList.remove('active'));
    // Deactivate playlist rows
    document.querySelectorAll('#playlist-list .sidebar-row').forEach(r => r.classList.remove('active'));
    // Hide USB action buttons
    setUsbButtonsVisible(false);
}

function setUsbButtonsVisible(visible) {
    document.getElementById('btn-sync').style.display = visible ? '' : 'none';
    document.getElementById('btn-wipe').style.display = visible ? '' : 'none';
    document.getElementById('btn-eject').style.display = visible ? '' : 'none';
}

// ── Volume loading ─────────────────────────────────────────────────────────
async function loadVolumes() {
    try {
        const volumes = await invoke('get_mounted_volumes');

        // Update hidden select (kept for selectVolume compatibility)
        const select = document.getElementById('usb-select');
        while (select.options.length > 1) select.remove(1);
        for (const vol of volumes) {
            const opt = document.createElement('option');
            opt.value = vol;
            opt.textContent = vol;
            select.appendChild(opt);
        }
        if (outputDir) {
            select.value = volumes.includes(outputDir) ? outputDir : '';
            if (select.value === '') outputDir = null;
        }

        // Render sidebar USB rows
        const container = document.getElementById('usb-volume-list');
        if (volumes.length === 0) {
            container.innerHTML = '<div class="sidebar-row" style="cursor:default;"><span class="label" style="color:var(--text-tertiary);font-size:11.5px;font-style:italic;">No volumes found</span></div>';
            return;
        }
        container.innerHTML = '';
        for (const vol of volumes) {
            const row = document.createElement('div');
            row.className = 'usb-volume-row';
            const shortName = vol.replace(/\\/g, '/').split('/').pop() || vol;
            row.innerHTML = `<span class="usb-dot mounted"></span><span class="usb-volume-name" title="${esc(vol)}">${esc(shortName)}</span>`;
            row.addEventListener('click', () => {
                // Deactivate playlist sidebar items
                document.querySelectorAll('#playlist-list .sidebar-row').forEach(r => r.classList.remove('active'));
                document.querySelectorAll('.usb-volume-row').forEach(r => r.classList.remove('active'));
                row.classList.add('active');
                selectVolume(vol);
            });
            if (outputDir === vol) row.classList.add('active');
            container.appendChild(row);
        }
    } catch (err) {
        console.error('get_mounted_volumes failed:', err);
    }
}

function selectVolume(path) {
    outputDir = path || null;
    if (outputDir) {
        rightPanelMode = 'usb';
        showRightPanel(true);
        setUsbButtonsVisible(true);
        document.getElementById('right-panel-title').textContent = outputDir.replace(/\\/g, '/').split('/').pop() || outputDir;
        document.getElementById('right-panel-subtitle').textContent = 'USB Drive';
        loadUsbContents();
    } else {
        usbTracks = [];
        usbPlaylists = [];
        renderUsbTracks();
        renderUsbPlaylistsInSidebar();
        setUsbButtonsVisible(false);
    }
}

async function loadUsbContents() {
    if (!outputDir) return;
    try {
        const result = await invoke('read_usb_state', { path: outputDir });
        if (result) {
            usbTracks = result.tracks;
            usbPlaylists = result.playlists;
        } else {
            usbTracks = [];
            usbPlaylists = [];
        }
    } catch (err) {
        console.error('read_usb_state failed:', err);
        usbTracks = [];
        usbPlaylists = [];
    }
    renderUsbTracks();
    renderUsbPlaylistsInSidebar();
}

async function ejectVolume() {
    if (!outputDir) { alert('Select a USB volume first.'); return; }
    try {
        await invoke('eject_volume', { path: outputDir });
        outputDir = null;
        document.getElementById('usb-select').value = '';
        setUsbButtonsVisible(false);
        closeRightPanel();
        loadVolumes();
    } catch (err) {
        alert('Eject failed: ' + err);
    }
}

async function wipeUsb() {
    if (!outputDir) { alert('Select a USB volume first.'); return; }

    const dialog = document.getElementById('confirm-dialog');
    document.getElementById('confirm-label').textContent =
        '⚠ This will permanently delete all Pioneer data (PIONEER/ and Contents/) from ' + outputDir + '. This cannot be undone.';
    dialog.returnValue = '';
    dialog.showModal();
    dialog.onclose = () => {
        if (dialog.returnValue !== 'ok') return;
        invoke('wipe_usb', { path: outputDir })
            .then(() => {
                usbTracks = [];
                usbPlaylists = [];
                renderUsbTracks();
                renderUsbPlaylistsInSidebar();
                alert('USB wiped successfully.');
            })
            .catch(err => { alert('Wipe failed: ' + err); });
    };
}

// ── Import ─────────────────────────────────────────────────────────────────
async function importFolder() {
    try {
        const dir = await dialogOpen({ directory: true, multiple: false });
        if (!dir) return;
        const newTracks = await invoke('scan_directory', { path: dir });
        // Merge, avoiding duplicate IDs
        const existingIds = new Set(tracks.map(t => t.id));
        const fresh = newTracks.filter(t => !existingIds.has(t.id));
        tracks = [...tracks, ...fresh];
        renderTracks();
        saveState();
    } catch (err) {
        console.error('importFolder failed:', err);
        alert('Import failed: ' + err);
    }
}

// ── Analyze ────────────────────────────────────────────────────────────────
async function analyzeAll() {
    if (analyzing) return;
    if (tracks.length === 0) { alert('No tracks loaded.'); return; }
    analyzing = true;
    setButtonStates();
    showProgress(0, tracks.length, 'Starting analysis…');
    try {
        const updated = await invoke('analyze_tracks');
        tracks = updated;
        renderTracks();
        saveState();
    } catch (err) {
        console.error('analyze_tracks failed:', err);
        alert('Analysis failed: ' + err);
    } finally {
        analyzing = false;
        setButtonStates();
        hideProgress();
    }
}

// ── Sync ───────────────────────────────────────────────────────────────────
async function syncToUsb() {
    if (syncing) return;
    if (!outputDir) { alert('Select a USB volume first.'); return; }
    if (playlists.length === 0) { alert('No playlists to sync.'); return; }

    const playlistInput = playlists.map(p => ({
        id: p.id,
        name: p.name,
        track_ids: p.trackIds,
    }));

    syncing = true;
    setButtonStates();
    showProgress(0, 1, 'Starting USB write…');
    try {
        await invoke('write_usb', { outputDir, playlists: playlistInput });
        // write-complete event will fire and clean up
    } catch (err) {
        console.error('write_usb failed:', err);
        alert('Sync failed: ' + err);
        syncing = false;
        setButtonStates();
        hideProgress();
    }
}

// ── Persistence ───────────────────────────────────────────────────────────
async function saveState() {
    const playlistInput = playlists.map(p => ({
        id: p.id,
        name: p.name,
        track_ids: p.trackIds,
    }));
    try {
        await invoke('save_state', { playlists: playlistInput });
    } catch (err) {
        console.error('save_state failed:', err);
    }
}

// ── Selection ──────────────────────────────────────────────────────────────
function toggleTrackSelection(trackId) {
    if (selectedTrackIds.has(trackId)) {
        selectedTrackIds.delete(trackId);
    } else {
        selectedTrackIds.add(trackId);
    }
    updateSelectionCount();
    // Toggle the CSS class on the row without a full re-render
    const row = document.querySelector(`div[data-track-id="${trackId}"]`);
    if (row) row.classList.toggle('selected', selectedTrackIds.has(trackId));
}

function updateSelectionCount() {
    const n = selectedTrackIds.size;
    document.getElementById('selection-count').textContent =
        n === 0 ? '' : n === 1 ? '1 selected' : `${n} selected`;
    document.getElementById('btn-delete').disabled = n === 0 || analyzing || syncing;
    document.getElementById('btn-test-cues').disabled = n === 0 || analyzing || syncing;
}

// ── Delete tracks ─────────────────────────────────────────────────────────
async function deleteSelected() {
    if (selectedTrackIds.size === 0) return;
    const ids = [...selectedTrackIds];
    try {
        await invoke('remove_tracks', { ids });
        tracks = tracks.filter(t => !selectedTrackIds.has(t.id));
        // Remove deleted tracks from all playlists
        for (const pl of playlists) {
            pl.trackIds = pl.trackIds.filter(id => !selectedTrackIds.has(id));
        }
        selectedTrackIds.clear();
        updateSelectionCount();
        renderTracks();
        renderPlaylists();
        saveState();
    } catch (err) {
        console.error('remove_tracks failed:', err);
        alert('Delete failed: ' + err);
    }
}

async function setTestCues() {
    if (selectedTrackIds.size === 0) return;
    const ids = [...selectedTrackIds];
    try {
        const updated = await invoke('set_test_cues', { ids });
        tracks = updated;
        renderTracks();
        saveState();
    } catch (err) {
        console.error('set_test_cues failed:', err);
        alert('Set cues failed: ' + err);
    }
}

// ── Playlists ──────────────────────────────────────────────────────────────
function createPlaylist() {
    const dialog = document.getElementById('text-input-dialog');
    const field = document.getElementById('text-input-field');
    document.getElementById('text-input-label').textContent = 'Playlist name:';
    field.value = '';
    dialog.returnValue = '';
    dialog.showModal();
    field.focus();
    dialog.onclose = () => {
        const name = field.value.trim();
        if (!name) return;
        playlists.push({ id: nextPlaylistId++, name, trackIds: [] });
        renderPlaylists();
        saveState();
    };
}

function deletePlaylist(playlistId) {
    playlists = playlists.filter(p => p.id !== playlistId);
    if (rightPanelMode && rightPanelMode.playlistId === playlistId) {
        closeRightPanel();
    }
    renderPlaylists();
    saveState();
}

function addSelectedToPlaylist(playlistId) {
    if (selectedTrackIds.size === 0) { alert('Select tracks in the track list first.'); return; }
    const pl = playlists.find(p => p.id === playlistId);
    if (!pl) return;
    for (const id of selectedTrackIds) {
        if (!pl.trackIds.includes(id)) pl.trackIds.push(id);
    }
    // Refresh right panel if this playlist is open there
    if (rightPanelMode && rightPanelMode.playlistId === playlistId) {
        renderPlaylistTracks(playlistId);
    }
    renderPlaylists();
    saveState();
}

function removeFromPlaylist(playlistId, trackId) {
    const pl = playlists.find(p => p.id === playlistId);
    if (!pl) return;
    pl.trackIds = pl.trackIds.filter(id => id !== trackId);
    // Refresh right panel if this playlist is open there
    if (rightPanelMode && rightPanelMode.playlistId === playlistId) {
        renderPlaylistTracks(playlistId);
    }
    renderPlaylists();
    saveState();
}

function showPlaylistInRightPanel(playlistId) {
    const pl = playlists.find(p => p.id === playlistId);
    if (!pl) return;
    rightPanelMode = { playlistId };
    setUsbButtonsVisible(false);
    document.getElementById('right-panel-title').textContent = esc(pl.name);
    renderPlaylistTracks(playlistId);
    showRightPanel(true);

    // Mark active in sidebar
    document.querySelectorAll('#playlist-list .sidebar-row').forEach(r => r.classList.remove('active'));
    document.querySelectorAll('.usb-volume-row').forEach(r => r.classList.remove('active'));
    const row = document.querySelector(`#playlist-list .sidebar-row[data-playlist-id="${playlistId}"]`);
    if (row) row.classList.add('active');
}

function renderPlaylistTracks(playlistId) {
    const pl = playlists.find(p => p.id === playlistId);
    const container = document.getElementById('right-track-list');
    if (!pl) {
        container.innerHTML = '<div class="empty-row">Playlist not found.</div>';
        return;
    }

    const count = pl.trackIds.length;
    document.getElementById('right-panel-subtitle').textContent =
        count === 0 ? 'empty' : count === 1 ? '1 track' : `${count} tracks`;

    if (count === 0) {
        container.innerHTML = '<div class="empty-row">No tracks in this playlist. Select tracks and click "+ Add".</div>';
        return;
    }

    const rows = pl.trackIds.map((tid, i) => {
        const t = tracks.find(x => x.id === tid);
        if (!t) return '';
        const title = esc(t.title || basename(t.source_path));
        const artist = esc(t.artist || '—');
        const label = esc(t.label || '—');
        const bpm = formatBpm(t.tempo);
        const key = t.key ? esc(t.key) : '—';
        const dur = formatDuration(t.duration_secs);
        return `<div class="track-row" data-track-id="${tid}" ondblclick="showWaveform(${tid})">
            <span class="track-cell num">${i + 1}</span>
            <span class="track-cell title" title="${esc(t.source_path)}">${title}</span>
            <span class="track-cell artist">${artist}</span>
            <span class="track-cell label-col">${label}</span>
            <span class="track-cell bpm">${bpm}</span>
            <span class="track-cell key">${key}</span>
            <span class="track-cell time">${dur}</span>
        </div>`;
    }).filter(Boolean);

    container.innerHTML = rows.length > 0
        ? rows.join('')
        : '<div class="empty-row">Some tracks in this playlist could not be found.</div>';
}

// ── Render: Library Tracks ─────────────────────────────────────────────────
function renderTracks() {
    const container = document.getElementById('track-list');
    if (tracks.length === 0) {
        container.innerHTML = '<div class="empty-row">No tracks loaded. Import a folder or drop files here.</div>';
        updateLibrarySubtitle();
        return;
    }
    const rows = tracks.map((t, i) => {
        const sel = selectedTrackIds.has(t.id) ? ' selected' : '';
        const unanalyzed = !t.tempo && !t.key && !t.duration_secs;
        const analyzing_cls = unanalyzed ? ' analyzing' : '';
        const title = esc(t.title || basename(t.source_path));
        const artist = esc(t.artist || '—');
        const label = esc(t.label || '—');
        const bpm = t.tempo ? formatBpm(t.tempo) : '<span class="muted">—</span>';
        const key = t.key ? esc(t.key) : '<span class="muted">—</span>';
        const dur = t.duration_secs ? formatDuration(t.duration_secs) : '<span class="muted">—</span>';
        return `<div class="track-row${sel}${analyzing_cls}" data-track-id="${t.id}" onclick="toggleTrackSelection(${t.id})" ondblclick="showWaveform(${t.id})">
            <span class="track-cell num">${i + 1}</span>
            <span class="track-cell title" title="${esc(t.source_path)}">${title}</span>
            <span class="track-cell artist">${artist}</span>
            <span class="track-cell label-col">${label}</span>
            <span class="track-cell bpm">${bpm}</span>
            <span class="track-cell key">${key}</span>
            <span class="track-cell time">${dur}</span>
        </div>`;
    });
    container.innerHTML = rows.join('');
    updateLibrarySubtitle();
}

function updateLibrarySubtitle() {
    const count = tracks.length;
    const el = document.getElementById('lib-track-subtitle');
    const countEl = document.getElementById('sidebar-track-count');
    if (el) el.textContent = count === 0 ? '' : count === 1 ? '1 track' : `${count} tracks`;
    if (countEl) countEl.textContent = count > 0 ? String(count) : '';
}

// ── Render: USB tracks ────────────────────────────────────────────────────
function renderUsbTracks() {
    const container = document.getElementById('right-track-list');
    if (rightPanelMode !== 'usb') return; // Don't overwrite playlist view

    const subtitle = document.getElementById('right-panel-subtitle');

    if (usbTracks.length === 0) {
        container.innerHTML = '<div class="empty-row">' +
            (outputDir ? 'No OneLibrary database found on this volume' : 'Select a USB volume to view contents') +
            '</div>';
        if (subtitle) subtitle.textContent = '';
        return;
    }

    if (subtitle) subtitle.textContent = usbTracks.length === 1 ? '1 track' : usbTracks.length + ' tracks';

    const rows = usbTracks.map((t, i) => {
        const title = esc(t.title || '—');
        const artist = esc(t.artist || '—');
        const label = esc(t.label || '—');
        const bpm = t.bpm > 0 ? t.bpm.toFixed(1) : '—';
        const key = t.key ? esc(t.key) : '—';
        const dur = formatDuration(t.duration);
        return `<div class="track-row">
            <span class="track-cell num">${i + 1}</span>
            <span class="track-cell title">${title}</span>
            <span class="track-cell artist">${artist}</span>
            <span class="track-cell label-col">${label}</span>
            <span class="track-cell bpm">${bpm}</span>
            <span class="track-cell key">${key}</span>
            <span class="track-cell time">${dur}</span>
        </div>`;
    });
    container.innerHTML = rows.join('');
}

// ── Render: USB Playlists in sidebar ──────────────────────────────────────
function renderUsbPlaylistsInSidebar() {
    // USB playlists shown in the sidebar USB section as sub-rows,
    // or we can just skip if there's nothing interesting to show.
    // For now we don't add extra rows — the USB volume row itself is the entry point.
}

// Kept for backwards compat (called from old code paths)
function renderUsbPlaylists() {
    renderUsbPlaylistsInSidebar();
}

// ── Render: Playlists (sidebar) ──────────────────────────────────────────
function renderPlaylists() {
    const container = document.getElementById('playlist-list');
    if (playlists.length === 0) {
        container.innerHTML = '<div class="sidebar-row" style="cursor:default;pointer-events:none;"><span class="label" style="color:var(--text-tertiary);font-size:11.5px;font-style:italic;">No playlists yet</span></div>';
        return;
    }

    const activeId = rightPanelMode && rightPanelMode.playlistId;

    const html = playlists.map(pl => {
        const count = pl.trackIds.length;
        const active = activeId === pl.id ? ' active' : '';
        return `<div class="sidebar-row${active}" data-playlist-id="${pl.id}">
            <span class="icon"><svg viewBox="0 0 12 12" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M1 2.5h10M1 5.5h10M1 8.5h6"/></svg></span>
            <span class="label">${esc(pl.name)}</span>
            <span class="count">${count}</span>
            <span class="row-actions">
                <button class="sidebar-row-btn" onclick="event.stopPropagation();addSelectedToPlaylist(${pl.id})" title="Add selected tracks">+</button>
                <button class="sidebar-row-btn" onclick="event.stopPropagation();deletePlaylist(${pl.id})" title="Delete playlist">&#xd7;</button>
            </span>
        </div>`;
    }).join('');
    container.innerHTML = html;

    // Attach click handlers
    container.querySelectorAll('.sidebar-row[data-playlist-id]').forEach(row => {
        row.addEventListener('click', () => {
            const id = parseInt(row.dataset.playlistId, 10);
            showPlaylistInRightPanel(id);
        });
        row.addEventListener('contextmenu', (e) => {
            e.preventDefault();
            const id = parseInt(row.dataset.playlistId, 10);
            const menu = document.getElementById('contextMenu');
            menu._playlistId = id;
            menu.style.left = e.clientX + 'px';
            menu.style.top = e.clientY + 'px';
            menu.classList.add('visible');
        });
    });
}

// ── Progress / Status bar ──────────────────────────────────────────────────
function showProgress(current, total, message) {
    const spinner = document.getElementById('status-spinner');
    const msg = document.getElementById('status-message');
    const right = document.getElementById('status-right');

    spinner.classList.remove('hidden');
    const pct = total > 0 ? Math.round((current / total) * 100) : 0;
    msg.textContent = message || `${current} / ${total}`;
    right.textContent = total > 0 ? `${pct}%` : '';

    // Also update hidden progress elements for any legacy listeners
    const fill = document.getElementById('progress-bar-fill');
    if (fill) fill.style.width = pct + '%';
    const label = document.getElementById('progress-label');
    if (label) label.textContent = message || `${current} / ${total}`;
}

function hideProgress() {
    document.getElementById('status-spinner').classList.add('hidden');
    document.getElementById('status-message').textContent = '';
    document.getElementById('status-right').textContent = '';

    const fill = document.getElementById('progress-bar-fill');
    if (fill) fill.style.width = '0%';
}

// ── Drag and drop ──────────────────────────────────────────────────────────
function setupDragDrop() {
    const overlay = document.getElementById('drop-overlay');
    try {
        const appWindow = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
        appWindow.onDragDropEvent((event) => {
            if (event.payload.type === 'drop') {
                overlay.classList.add('hidden');
                const paths = event.payload.paths;
                if (!paths || paths.length === 0) return;
                invoke('scan_files', { paths }).then(newTracks => {
                    const existingIds = new Set(tracks.map(t => t.id));
                    const fresh = newTracks.filter(t => !existingIds.has(t.id));
                    tracks = [...tracks, ...fresh];
                    renderTracks();
                    saveState();
                }).catch(err => {
                    console.error('scan_files failed:', err);
                    alert('Failed to scan dropped files: ' + err);
                });
            } else if (event.payload.type === 'hover') {
                overlay.classList.remove('hidden');
            } else if (event.payload.type === 'cancel') {
                overlay.classList.add('hidden');
            }
        });
    } catch (err) {
        console.warn('Drag-and-drop setup failed (expected outside Tauri):', err);
    }
}

// ── Button state ───────────────────────────────────────────────────────────
function setButtonStates() {
    document.getElementById('btn-analyze').disabled = analyzing;
    document.getElementById('btn-sync').disabled = syncing;
    document.getElementById('btn-import').disabled = analyzing || syncing;
    document.getElementById('btn-delete').disabled = selectedTrackIds.size === 0 || analyzing || syncing;
    document.getElementById('btn-test-cues').disabled = selectedTrackIds.size === 0 || analyzing || syncing;
}

// ── Helpers ────────────────────────────────────────────────────────────────
function formatDuration(secs) {
    if (!secs || secs < 0) return '—';
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${String(s).padStart(2, '0')}`;
}

function formatBpm(tempo) {
    if (!tempo || tempo === 0) return '—';
    return (tempo / 100).toFixed(1);
}

function basename(path) {
    if (!path) return '';
    return path.replace(/\\/g, '/').split('/').pop();
}

// Escape HTML to avoid XSS in dynamic content
function esc(str) {
    if (!str) return '';
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

// ── Library path ──────────────────────────────────────────────────────────
async function loadLibraryPath() {
    try {
        const path = await invoke('get_library_path');
        document.getElementById('library-path').textContent = path;
    } catch (err) {
        document.getElementById('library-path').textContent = '(unknown)';
    }
}

async function changeLibraryPath() {
    try {
        const folder = await dialogOpen({ directory: true, multiple: false });
        if (!folder) return; // user cancelled

        const newPath = await invoke('change_library_path', { folderPath: folder });
        document.getElementById('library-path').textContent = newPath;

        // Reload library contents
        const loaded = await invoke('load_state');
        tracks = loaded.tracks;
        playlists = loaded.playlists.map(p => ({
            id: p.id,
            name: p.name,
            trackIds: p.track_ids,
        }));
        nextPlaylistId = playlists.length > 0
            ? Math.max(...playlists.map(p => p.id)) + 1
            : 1;
        renderTracks();
        renderPlaylists();
    } catch (err) {
        alert('Failed to change library: ' + err);
    }
}

// ── Waveform Display ──────────────────────────────────────────────────────
let currentWaveformData = null;
let waveformZoom = 1.0;    // horizontal zoom: 1.0 = full track visible
let waveformOffset = 0.0;  // scroll offset as fraction of total length
let waveformDragging = false;
let waveformDragStartY = 0;
let waveformDragStartX = 0;
let waveformDragStartZoom = 1.0;
let waveformDragStartOffset = 0.0;

function setupWaveformInteraction() {
    const canvas = document.getElementById('waveform-canvas');

    canvas.addEventListener('mousedown', (e) => {
        waveformDragging = true;
        waveformDragStartY = e.clientY;
        waveformDragStartX = e.clientX;
        waveformDragStartZoom = waveformZoom;
        waveformDragStartOffset = waveformOffset;
        canvas.style.cursor = 'grabbing';
        e.preventDefault();
    });

    window.addEventListener('mousemove', (e) => {
        if (!waveformDragging) return;
        const dy = waveformDragStartY - e.clientY; // drag up = zoom in
        const dx = e.clientX - waveformDragStartX; // drag right = scroll earlier

        const newZoom = Math.max(1.0, Math.min(64.0, waveformDragStartZoom * Math.pow(1.015, dy)));
        waveformZoom = newZoom;

        if (waveformZoom > 1.0) {
            const rect = canvas.getBoundingClientRect();
            const panAmount = (dx / rect.width) / waveformZoom;
            waveformOffset = clampOffset(waveformDragStartOffset - panAmount);
        }

        renderWaveform();
    });

    window.addEventListener('mouseup', () => {
        if (waveformDragging) {
            waveformDragging = false;
            document.getElementById('waveform-canvas').style.cursor = 'default';
        }
    });

    canvas.addEventListener('wheel', (e) => {
        if (waveformZoom <= 1.0) return;
        e.preventDefault();
        waveformOffset = clampOffset(waveformOffset + (e.deltaX + e.deltaY) * 0.005 / waveformZoom);
        renderWaveform();
    }, { passive: false });
}

function clampOffset(offset) {
    return Math.max(0, Math.min(1.0 - (1.0 / waveformZoom), offset));
}

async function showWaveform(trackId) {
    const track = tracks.find(t => t.id === trackId);
    if (!track) return;

    document.getElementById('waveform-track-name').textContent = track.title || 'Unknown';
    waveformZoom = 1.0;
    waveformOffset = 0.0;

    try {
        const data = await invoke('get_analysis_data', { trackId: trackId });
        currentWaveformData = data;
        renderWaveform();
    } catch (err) {
        console.warn('No analysis data:', err);
        document.getElementById('waveform-track-name').textContent += ' (not analyzed)';
        currentWaveformData = null;
        renderWaveform();
    }
}

function renderWaveform() {
    const canvas = document.getElementById('waveform-canvas');
    const ctx = canvas.getContext('2d');
    const mode = document.getElementById('waveform-mode').value;

    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * window.devicePixelRatio;
    canvas.height = rect.height * window.devicePixelRatio;
    ctx.scale(window.devicePixelRatio, window.devicePixelRatio);

    const w = rect.width;
    const h = rect.height;

    ctx.fillStyle = '#0d0d0d';
    ctx.fillRect(0, 0, w, h);

    if (!currentWaveformData) return;

    const visibleFraction = 1.0 / waveformZoom;
    const startFrac = waveformOffset;
    const endFrac = startFrac + visibleFraction;

    if (mode === 'color') {
        renderColorWaveform(ctx, w, h, currentWaveformData.waveform_color, startFrac, endFrac);
    } else if (mode === 'mono') {
        renderMonoWaveform(ctx, w, h, currentWaveformData.waveform_preview, startFrac, endFrac);
    } else if (mode === 'peaks') {
        renderPeaksWaveform(ctx, w, h, currentWaveformData.waveform_peaks, startFrac, endFrac);
    }

    renderBeatGrid(ctx, w, h, startFrac, endFrac);
    renderTimeGrid(ctx, w, h, startFrac, endFrac);
}

function renderColorWaveform(ctx, w, h, data, startFrac, endFrac) {
    if (!data || data.length === 0) return;
    const startIdx = Math.floor(startFrac * data.length);
    const endIdx = Math.ceil(endFrac * data.length);
    const visibleCount = endIdx - startIdx;
    const barWidth = w / visibleCount;
    const centerY = h / 2;

    for (let i = 0; i < visibleCount; i++) {
        const di = startIdx + i;
        if (di >= data.length) break;
        const { amp, r, g, b } = data[di];
        const barH = Math.min(amp * centerY, centerY);
        ctx.fillStyle = `rgb(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)})`;
        ctx.fillRect(i * barWidth, centerY - barH, Math.max(barWidth, 1), barH * 2);
    }
}

function renderMonoWaveform(ctx, w, h, data, startFrac, endFrac) {
    if (!data || data.length === 0) return;
    const startIdx = Math.floor(startFrac * data.length);
    const endIdx = Math.ceil(endFrac * data.length);
    const visibleCount = endIdx - startIdx;
    const barWidth = w / visibleCount;
    const centerY = h / 2;

    for (let i = 0; i < visibleCount; i++) {
        const di = startIdx + i;
        if (di >= data.length) break;
        const byte = data[di];
        const height = (byte & 0x1F) / 31.0;
        const whiteness = ((byte >> 5) & 0x07) / 7.0;
        const barH = Math.min(height * centerY, centerY);
        const brightness = Math.round(100 + whiteness * 155);
        ctx.fillStyle = `rgb(${brightness}, ${brightness}, ${brightness})`;
        ctx.fillRect(i * barWidth, centerY - barH, Math.max(barWidth, 1), barH * 2);
    }
}

function renderPeaksWaveform(ctx, w, h, data, startFrac, endFrac) {
    if (!data || data.length === 0) return;
    const startIdx = Math.floor(startFrac * data.length);
    const endIdx = Math.ceil(endFrac * data.length);
    const visibleCount = endIdx - startIdx;
    const barWidth = w / visibleCount;
    const centerY = h / 2;

    ctx.fillStyle = '#4a9eff';
    for (let i = 0; i < visibleCount; i++) {
        const di = startIdx + i;
        if (di >= data.length) break;
        const [min, max] = data[di];
        const y1 = centerY - max * centerY;
        const y2 = centerY - min * centerY;
        ctx.fillRect(i * barWidth, y1, Math.max(barWidth, 1), y2 - y1);
    }
}

function renderBeatGrid(ctx, w, h, startFrac, endFrac) {
    const beats = currentWaveformData?.beats;
    const durationMs = currentWaveformData?.duration_ms;
    if (!beats || beats.length === 0 || !durationMs) return;

    let barNumber = 0;
    for (const beat of beats) {
        const frac = beat.time_ms / durationMs;
        if (beat.bar_position === 1) barNumber++;
        if (frac < startFrac || frac > endFrac) continue;

        const x = ((frac - startFrac) / (endFrac - startFrac)) * w;

        if (beat.bar_position === 1) {
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.5)';
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, h);
            ctx.stroke();
            ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
            ctx.font = '9px system-ui';
            ctx.fillText(String(barNumber), x + 3, 10);
        } else {
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.15)';
            ctx.lineWidth = 0.5;
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, h);
            ctx.stroke();
        }
    }
}

function renderTimeGrid(ctx, w, h, startFrac, endFrac) {
    const durationMs = currentWaveformData?.duration_ms;
    if (!durationMs) return;

    const visibleMs = (endFrac - startFrac) * durationMs;
    let intervalMs;
    if (visibleMs > 120000) intervalMs = 30000;
    else if (visibleMs > 60000) intervalMs = 10000;
    else if (visibleMs > 20000) intervalMs = 5000;
    else if (visibleMs > 8000) intervalMs = 2000;
    else intervalMs = 1000;

    const startMs = startFrac * durationMs;
    const endMs = endFrac * durationMs;
    const firstTick = Math.ceil(startMs / intervalMs) * intervalMs;

    ctx.fillStyle = 'rgba(255, 255, 255, 0.35)';
    ctx.font = '9px system-ui';

    for (let ms = firstTick; ms <= endMs; ms += intervalMs) {
        const x = ((ms / durationMs - startFrac) / (endFrac - startFrac)) * w;
        const secs = Math.floor(ms / 1000);
        const label = `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`;
        ctx.fillText(label, x + 2, h - 3);
    }
}

// ── Bootstrap ─────────────────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', init);
