// Pioneer Test UI — app.js
// Vanilla JS, no framework. All Tauri calls via window.__TAURI__.core.invoke()

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ── State ──────────────────────────────────────────────────────────────────
let tracks = [];
let playlists = [];
let selectedTrackIds = new Set();
let outputDir = null;
let nextPlaylistId = 1;
let analyzing = false;
let syncing = false;

// ── Init ───────────────────────────────────────────────────────────────────
async function init() {
    // Show version
    try {
        const ver = await invoke('app_version');
        document.getElementById('version-badge').textContent = 'v' + ver;
    } catch (_) {}

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

    listen('analysis-progress', (event) => {
        const { current, total, message } = event.payload;
        showProgress(current, total, message);
    });

    listen('write-complete', () => {
        hideProgress();
        syncing = false;
        setButtonStates();
        alert('USB write complete!');
    });
}

// ── Volume loading ─────────────────────────────────────────────────────────
async function loadVolumes() {
    try {
        const volumes = await invoke('get_mounted_volumes');
        const select = document.getElementById('usb-select');
        // Keep the placeholder option, rebuild the rest
        while (select.options.length > 1) select.remove(1);
        for (const vol of volumes) {
            const opt = document.createElement('option');
            opt.value = vol;
            opt.textContent = vol;
            select.appendChild(opt);
        }
        // If current outputDir is still in the list, keep it selected
        if (outputDir) {
            select.value = volumes.includes(outputDir) ? outputDir : '';
            if (select.value === '') outputDir = null;
        }
    } catch (err) {
        console.error('get_mounted_volumes failed:', err);
    }
}

function selectVolume(path) {
    outputDir = path || null;
}

async function ejectVolume() {
    if (!outputDir) { alert('Select a USB volume first.'); return; }
    try {
        await invoke('eject_volume', { path: outputDir });
        outputDir = null;
        document.getElementById('usb-select').value = '';
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
            .then(() => { alert('USB wiped successfully.'); })
            .catch(err => { alert('Wipe failed: ' + err); });
    };
}

// ── Import ─────────────────────────────────────────────────────────────────
async function importFolder() {
    try {
        const dir = await invoke('pick_directory');
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
    const row = document.querySelector(`tr[data-track-id="${trackId}"]`);
    if (row) row.classList.toggle('selected', selectedTrackIds.has(trackId));
}

function updateSelectionCount() {
    const n = selectedTrackIds.size;
    document.getElementById('selection-count').textContent =
        n === 1 ? '1 selected' : `${n} selected`;
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
    renderPlaylists();
    saveState();
}

function removeFromPlaylist(playlistId, trackId) {
    const pl = playlists.find(p => p.id === playlistId);
    if (!pl) return;
    pl.trackIds = pl.trackIds.filter(id => id !== trackId);
    renderPlaylists();
    saveState();
}

// ── Render: Tracks ─────────────────────────────────────────────────────────
function renderTracks() {
    const tbody = document.getElementById('track-tbody');
    if (tracks.length === 0) {
        tbody.innerHTML = '<tr class="empty-row"><td colspan="7">No tracks loaded. Import a folder or drop files here.</td></tr>';
        return;
    }
    const rows = tracks.map((t, i) => {
        const sel = selectedTrackIds.has(t.id) ? ' selected' : '';
        const title = esc(t.title || basename(t.source_path));
        const artist = esc(t.artist || '—');
        const bpm = formatBpm(t.tempo);
        const key = t.key ? esc(t.key) : '—';
        const dur = formatDuration(t.duration_secs);
        const cues = t.has_cues ? '●' : '';
        return `<tr class="track-row${sel}" data-track-id="${t.id}" onclick="toggleTrackSelection(${t.id})">
            <td class="col-num">${i + 1}</td>
            <td class="col-title" title="${esc(t.source_path)}">${title}</td>
            <td class="col-artist">${artist}</td>
            <td class="col-bpm">${bpm}</td>
            <td class="col-key">${key}</td>
            <td class="col-cues">${cues}</td>
            <td class="col-dur">${dur}</td>
        </tr>`;
    });
    tbody.innerHTML = rows.join('');
}

// ── Render: Playlists ──────────────────────────────────────────────────────
function renderPlaylists() {
    const container = document.getElementById('playlist-list');
    if (playlists.length === 0) {
        container.innerHTML = '<p style="color:var(--text-dim);font-size:11px;padding:12px 10px;font-style:italic;">No playlists yet.</p>';
        return;
    }
    const html = playlists.map(pl => {
        const trackItems = pl.trackIds.map(tid => {
            const t = tracks.find(x => x.id === tid);
            if (!t) return '';
            const name = esc(t.title || basename(t.source_path));
            const artist = esc(t.artist || '');
            const label = artist ? `${name} — ${artist}` : name;
            return `<li class="playlist-track">
                <span class="playlist-track-name" title="${label}">${label}</span>
                <button class="playlist-track-remove" onclick="removeFromPlaylist(${pl.id}, ${tid})" title="Remove">✕</button>
            </li>`;
        }).join('');

        const count = pl.trackIds.length;
        const countLabel = count === 1 ? '1 track' : `${count} tracks`;

        return `<div class="playlist-item">
            <div class="playlist-header">
                <span class="playlist-name">${esc(pl.name)}</span>
                <span class="playlist-count">${countLabel}</span>
                <button class="playlist-add-btn" onclick="addSelectedToPlaylist(${pl.id})" title="Add selected tracks">+ Add</button>
                <button class="playlist-del-btn" onclick="deletePlaylist(${pl.id})" title="Delete playlist">✕</button>
            </div>
            ${count > 0
                ? `<ul class="playlist-tracks">${trackItems}</ul>`
                : `<p class="playlist-empty">Empty playlist</p>`
            }
        </div>`;
    }).join('');
    container.innerHTML = html;
}

// ── Progress bar ───────────────────────────────────────────────────────────
function showProgress(current, total, message) {
    const wrap = document.getElementById('progress-bar-wrap');
    const fill = document.getElementById('progress-bar-fill');
    const label = document.getElementById('progress-label');

    wrap.classList.remove('hidden');
    const pct = total > 0 ? Math.round((current / total) * 100) : 0;
    fill.style.width = pct + '%';
    label.textContent = message || `${current} / ${total}`;
}

function hideProgress() {
    document.getElementById('progress-bar-wrap').classList.add('hidden');
    document.getElementById('progress-bar-fill').style.width = '0%';
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

// ── Bootstrap ─────────────────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', init);
