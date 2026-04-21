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
    if (outputDir) {
        loadUsbContents();
    } else {
        usbTracks = [];
        usbPlaylists = [];
        renderUsbTracks();
        renderUsbPlaylists();
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
    renderUsbPlaylists();
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
            .then(() => { usbTracks = []; usbPlaylists = []; renderUsbTracks(); renderUsbPlaylists(); alert('USB wiped successfully.'); })
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
        return `<tr class="track-row${sel}" data-track-id="${t.id}" onclick="toggleTrackSelection(${t.id})" ondblclick="showWaveform(${t.id})">
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

// ── Render: USB tracks ────────────────────────────────────────────────────
function renderUsbTracks() {
    const tbody = document.getElementById('usb-tbody');
    const countEl = document.getElementById('usb-track-count');

    if (usbTracks.length === 0) {
        tbody.innerHTML = '<tr class="empty-row"><td colspan="5">' +
            (outputDir ? 'No OneLibrary database found on this volume' : 'Select a USB volume to view contents') +
            '</td></tr>';
        countEl.textContent = '';
        return;
    }

    countEl.textContent = usbTracks.length === 1 ? '1 track' : usbTracks.length + ' tracks';

    const rows = usbTracks.map(t => {
        const title = esc(t.title || '—');
        const artist = esc(t.artist || '—');
        const bpm = t.bpm > 0 ? t.bpm.toFixed(1) : '—';
        const key = t.key ? esc(t.key) : '—';
        const dur = formatDuration(t.duration);
        return `<tr>
            <td class="col-title">${title}</td>
            <td class="col-artist">${artist}</td>
            <td class="col-bpm">${bpm}</td>
            <td class="col-key">${key}</td>
            <td class="col-dur">${dur}</td>
        </tr>`;
    });
    tbody.innerHTML = rows.join('');
}

// ── Render: USB Playlists ──────────────────────────────────────────────────
function renderUsbPlaylists() {
    const container = document.getElementById('usb-playlist-list');

    if (usbPlaylists.length === 0) {
        container.innerHTML = '<p style="color:var(--text-dim);font-size:11px;padding:12px 10px;font-style:italic;">' +
            (outputDir ? 'No playlists on USB' : 'Select a USB volume') + '</p>';
        return;
    }

    const html = usbPlaylists.map(pl => {
        const countLabel = pl.track_count === 1 ? '1 track' : `${pl.track_count} tracks`;
        return `<div class="usb-playlist-item">
            <span class="usb-playlist-name">${esc(pl.name)}</span>
            <span class="usb-playlist-count">${countLabel}</span>
        </div>`;
    }).join('');
    container.innerHTML = html;
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

async function showWaveform(trackId) {
    const track = tracks.find(t => t.id === trackId);
    if (!track) return;

    document.getElementById('waveform-track-name').textContent = track.title || 'Unknown';
    document.getElementById('waveform-panel').classList.remove('hidden');

    try {
        const data = await invoke('get_analysis_data', { trackId: trackId });
        currentWaveformData = data;
        renderWaveform();
    } catch (err) {
        console.warn('No analysis data:', err);
        document.getElementById('waveform-track-name').textContent += ' (not analyzed)';
    }
}

function closeWaveform() {
    document.getElementById('waveform-panel').classList.add('hidden');
    currentWaveformData = null;
}

function renderWaveform() {
    if (!currentWaveformData) return;

    const canvas = document.getElementById('waveform-canvas');
    const ctx = canvas.getContext('2d');
    const mode = document.getElementById('waveform-mode').value;

    // Set canvas resolution to match display size
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * window.devicePixelRatio;
    canvas.height = rect.height * window.devicePixelRatio;
    ctx.scale(window.devicePixelRatio, window.devicePixelRatio);

    const w = rect.width;
    const h = rect.height;

    // Clear
    ctx.fillStyle = '#0d0d0d';
    ctx.fillRect(0, 0, w, h);

    if (mode === 'color') {
        renderColorWaveform(ctx, w, h, currentWaveformData.waveform_color);
    } else if (mode === 'mono') {
        renderMonoWaveform(ctx, w, h, currentWaveformData.waveform_preview);
    } else if (mode === 'peaks') {
        renderPeaksWaveform(ctx, w, h, currentWaveformData.waveform_peaks);
    }
}

function renderColorWaveform(ctx, w, h, data) {
    if (!data || data.length === 0) return;
    const barWidth = w / data.length;
    const centerY = h / 2;

    for (let i = 0; i < data.length; i++) {
        const { amp, r, g, b } = data[i];
        const barH = amp * centerY;

        // Mix RGB channels into a color
        const red = Math.round(r * 255);
        const green = Math.round(g * 255);
        const blue = Math.round(b * 255);

        ctx.fillStyle = `rgb(${red}, ${green}, ${blue})`;
        ctx.fillRect(i * barWidth, centerY - barH, Math.max(barWidth, 1), barH * 2);
    }
}

function renderMonoWaveform(ctx, w, h, data) {
    if (!data || data.length === 0) return;
    const barWidth = w / data.length;
    const centerY = h / 2;

    for (let i = 0; i < data.length; i++) {
        const byte = data[i];
        const height = (byte & 0x1F) / 31.0;  // 5 low bits
        const whiteness = ((byte >> 5) & 0x07) / 7.0;  // 3 high bits

        const barH = height * centerY;
        const brightness = Math.round(100 + whiteness * 155);

        ctx.fillStyle = `rgb(${brightness}, ${brightness}, ${brightness})`;
        ctx.fillRect(i * barWidth, centerY - barH, Math.max(barWidth, 1), barH * 2);
    }
}

function renderPeaksWaveform(ctx, w, h, data) {
    if (!data || data.length === 0) return;
    const barWidth = w / data.length;
    const centerY = h / 2;

    ctx.fillStyle = '#4a9eff';
    for (let i = 0; i < data.length; i++) {
        const [min, max] = data[i];
        const y1 = centerY - max * centerY;
        const y2 = centerY - min * centerY;
        ctx.fillRect(i * barWidth, y1, Math.max(barWidth, 1), y2 - y1);
    }
}

// ── Bootstrap ─────────────────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', init);
