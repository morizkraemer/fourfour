import type { LoadedState, SkippedFiles, TrackInfo } from '../src/services/tauri.svelte.ts';
import type { CuePointPayload } from '../src/lib/cues.ts';

export interface SmokeWindow {
  __smokeInvokes: { cmd: string; args: unknown }[];
  __smokeUnhandled: string[];
  __smokeDialogs: string[];
  __smokeAnswerDialog: boolean;
  __smokeAnalysis: (trackId: number) => Promise<Record<string, unknown>>;
  __smokeNoAnalysis: number[];
  __smokeIpcMs: number;
  __smokeMissing: number[];
  __smokeUnreadable: number[];
  __smokeNotFound: number[];
  __smokePickPath: string | null;
  __smokeDuplicates: unknown[];
  __smokeReconnectPlan: unknown;
  __smokeRemapPreview: unknown;
  __smokeDupGroups: { key: number; confidence: string; keeper_id: number; members: { id: number; artist: string; title: string }[] }[];
  __smokeDupArchive: { merge_id: number; tracks: { id: number }[] }[];
  __smokeHoldExport: boolean;
  __smokeReleaseExport: (() => void) | null;
  __smokeImport: SmokeImport | null;
  __smokeSurvey: { path: string; folder: boolean; files: string[] }[] | null;
  __smokeHoldImport: boolean;
  __smokeHoldLoad: boolean;
  __smokeLoadError: { kind: string; message: string; path?: string } | null;
  __smokeReleaseLoad: (() => void) | null;
  __smokeAnalyzeAfterImport: boolean;
  __smokeReleaseImport: (() => void) | null;
  __smokeEmit: (event: string, payload: unknown) => void;
  __smokeDetail: 'none' | 'serve' | 'hold';
  __smokeReleaseDetail: (() => void) | null;
  __smokeLabelChanges: { id: number; rating: number | null; color: string | null }[];
  __smokeSticks: Record<string, SmokeStick>;
  __smokeArtwork: boolean;
}

export interface SmokeStick {
  total: number;
  free: number;
  contents: { stick_id: string; track_ids: number[]; playlist_ids: number[]; kept_as_is?: { playlist_id: number; as_of: string }[] };
  written: number[];
  foreign: number;
  playlists: { id: number; name: string; track_count: number }[];
  last_sync: string | null;
}

export const SMOKE_STICKS_KEY = 'smoke.sticks';

export interface SmokeImport {
  tracks: TrackInfo[];
  already: number;
  skipped?: Partial<SkippedFiles>;
}

export function installTauriMock(initial: LoadedState & { payloadUrl?: string }) {
  const w = window as unknown as Window & SmokeWindow & Record<string, unknown>;
  w.__smokeInvokes = [];
  w.__smokeUnhandled = [];
  w.__smokeDialogs = [];
  w.__smokeAnswerDialog = true;
  w.__smokeMissing = [];
  w.__smokeUnreadable = [];
  w.__smokeNotFound = [];
  w.__smokePickPath = '/Music/moved';
  w.__smokeDuplicates = [];
  w.__smokeReconnectPlan = null;
  w.__smokeRemapPreview = { total: 0, found: 0, still_missing: 0 };
  w.__smokeDupGroups = [];
  w.__smokeDupArchive = [];
  w.__smokeHoldExport = false;
  w.__smokeReleaseExport = null;
  w.__smokeImport = null;
  w.__smokeSurvey = null;
  w.__smokeHoldImport = false;
  w.__smokeHoldLoad = false;
  const storedLoadError = localStorage.getItem('smoke.loadError');
  w.__smokeLoadError = storedLoadError ? JSON.parse(storedLoadError) : null;
  w.__smokeReleaseLoad = null;
  w.__smokeAnalyzeAfterImport = false;
  w.__smokeReleaseImport = null;
  w.__smokeDetail = 'none';
  w.__smokeNoAnalysis = [];
  w.__smokeIpcMs = 0;
  const artworkBySize = new Map<number, ArrayBuffer>();
  w.__smokeArtwork = false;
  const smokeArtwork = async (trackId: number, maxPx: number | undefined): Promise<ArrayBuffer> => {
    if (!w.__smokeArtwork) return new ArrayBuffer(0);
    const size = Math.min(maxPx ?? 512, 512);
    const encoded = w.__smokeIpcMs ? artworkBySize.get(size) : undefined;
    if (encoded) return encoded.slice(0);
    const canvas = new OffscreenCanvas(size, size);
    const ctx = canvas.getContext('2d')!;
    ctx.fillStyle = `hsl(${(trackId * 47) % 360} 60% 45%)`;
    ctx.fillRect(0, 0, size, size);
    const jpeg = await (await canvas.convertToBlob({ type: 'image/jpeg', quality: 0.85 })).arrayBuffer();
    if (w.__smokeIpcMs) artworkBySize.set(size, jpeg);
    if (size < 512) return jpeg;
    const padded = new Uint8Array(jpeg.byteLength + 8192);
    padded.set(new Uint8Array(jpeg));
    return padded.buffer;
  };
  w.__smokeReleaseDetail = null;
  w.__smokeLabelChanges ??= [];
  const GB = 1_000_000_000;
  const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();
  w.__smokeSticks =
    localStorage.getItem('smoke.sticks') === '1'
      ? {
          '/Volumes/SanDisk 32GB': {
            total: 29.8 * GB,
            free: 22.2 * GB,
            contents: { stick_id: 'stick-a', track_ids: [7, 8], playlist_ids: [1], kept_as_is: [] },
            written: [1, 2, 3, 7, 8, 11, 12],
            foreign: 2,
            playlists: [
              { id: 1, name: 'Peak Time', track_count: 3 },
              { id: 2, name: 'RB Warmup Edits', track_count: 18 },
              { id: 3, name: 'Hamburg Oct 2023', track_count: 26 },
            ],
            last_sync: minutesAgo(2),
          },
          '/Volumes/BACKUP 32GB': {
            total: 32 * GB,
            free: 30.1 * GB,
            contents: { stick_id: 'stick-b', track_ids: [], playlist_ids: [], kept_as_is: [] },
            written: [],
            foreign: 0,
            playlists: [],
            last_sync: null,
          },
          '/Volumes/PANORAMA 8GB': {
            total: 8 * GB,
            free: 20_000_000,
            contents: { stick_id: 'stick-c', track_ids: [], playlist_ids: [1], kept_as_is: [] },
            written: [],
            foreign: 0,
            playlists: [],
            last_sync: minutesAgo(60 * 24 * 20),
          },
        }
      : {};
  let lastBackup: unknown = null;
  const relocated = new Set<number>();
  let lastRelocation: unknown = null;

  let nextCallbackId = 1;
  const listeners = new Map<number, { event: string; handler: number }>();
  let nextListenerId = 1;

  const edits = {
    cues: new Map<number, CuePointPayload[]>(),
    grids: new Map<number, { beats: unknown[]; grid_offset_ms: number; grid_anchor_ms: number | null }>(),
    tempo: new Map<number, number>(),
  };
  let playlists = initial.playlists;
  if (localStorage.getItem('smoke.reviewFolder')) {
    playlists = [
      ...playlists,
      { id: 10, name: 'Festival sets', track_ids: [], parent_id: 0, is_folder: true },
      { id: 11, name: 'Sunday main stage', track_ids: [5, 6], parent_id: 10, is_folder: false },
    ];
  }
  const tracks = [...initial.tracks];
  let libraryTrackCount = initial.tracks.length;
  const served = initial.payloadUrl ? servedPayload(initial.payloadUrl) : null;

  const SERVED_LIST_COMMANDS = new Set(['read_usb_state', 'plan_usb_sync', 'sync_usb_many']);

  function servedPayload(url: string) {
    const answer = fetch(`${url}/answer`)
      .then((response) => response.json())
      .then((state: { tracks: TrackInfo[] | { id: number[] }; playlists: LoadedState['playlists'] }) => {
        libraryTrackCount = Array.isArray(state.tracks) ? state.tracks.length : state.tracks.id.length;
        return state;
      });
    let listed: Promise<void> | null = null;
    return {
      answer,
      previews: (ids: number[]) => fetch(`${url}/previews?ids=${ids.join(',')}`).then((response) => response.json()),
      tracks: () =>
        (listed ??= fetch(`${url}/tracks`)
          .then((response) => response.json())
          .then((list: TrackInfo[]) => {
            const addedSince = tracks.splice(0);
            for (const track of list) tracks.push(track);
            for (const track of addedSince) tracks.push(track);
          })),
    };
  }
  let pendingImport: TrackInfo[] = [];
  const playback = { path: '', position: 0, duration: 0, playing: false, timer: 0 };

  function emit(event: string, payload: unknown) {
    for (const { event: name, handler } of listeners.values()) {
      if (name !== event) continue;
      const callback = w[`_${handler}`] as ((message: unknown) => void) | undefined;
      callback?.({ event, id: 0, payload });
    }
  }
  w.__smokeEmit = emit;

  function tick() {
    emit('playback-tick', {
      position_ms: playback.position,
      duration_ms: playback.duration,
      playing: playback.playing,
      level_left: 0,
      level_right: 0,
      tempo_ratio: 1,
    });
  }

  function startTimer() {
    window.clearInterval(playback.timer);
    playback.timer = window.setInterval(() => {
      if (!playback.playing) return;
      playback.position = Math.min(playback.duration, playback.position + 50);
      tick();
    }, 50);
  }

  async function analysisFor(trackId: number) {
    if (w.__smokeNoAnalysis.includes(trackId)) throw new Error('smoke: track not analysed');
    const data = await w.__smokeAnalysis(trackId);
    const grid = edits.grids.get(trackId);
    const cues = edits.cues.get(trackId);
    return { ...data, ...(grid ?? {}), ...(cues ? { cue_points: cues } : {}) };
  }

  async function detailFor(trackId: number) {
    const { duration_ms } = (await w.__smokeAnalysis(trackId)) as { duration_ms: number };
    const columns = Math.ceil((duration_ms / 1000) * 2400);
    const buffer = new ArrayBuffer(28 + 5 * columns);
    const view = new DataView(buffer);
    'FFWD'.split('').forEach((c, i) => view.setUint8(i, c.charCodeAt(0)));
    view.setUint8(4, 3);
    view.setUint32(8, 2400, true);
    view.setUint32(12, columns, true);
    view.setUint16(16, 300, true);
    view.setFloat32(24, 379.5, true);
    const bands = new Uint8Array(buffer, 28, 3 * columns);
    const signal = new Int8Array(buffer, 28 + 3 * columns, 2 * columns);
    for (let i = 0; i < columns; i++) {
      const beat = Math.abs(Math.sin((i / 2400) * Math.PI * 2.1));
      bands[i] = Math.round(30 + 80 * beat);
      bands[columns + i] = Math.round(20 + 40 * Math.abs(Math.sin(i / 700)));
      bands[2 * columns + i] = Math.round(10 + 30 * Math.abs(Math.sin(i / 230)));
      signal[i] = -Math.round(20 + 70 * beat);
      signal[columns + i] = Math.round(20 + 70 * beat);
    }
    return buffer;
  }

  function health() {
    return {
      running: false,
      checked_at_ms: Date.now(),
      duration_ms: 40,
      total: libraryTrackCount,
      missing: [...w.__smokeMissing],
      error: [...w.__smokeUnreadable],
      relocated: [...relocated],
    };
  }

  function relink(ids: number[], folder: string) {
    w.__smokeMissing = w.__smokeMissing.filter((id) => !ids.includes(id));
    w.__smokeUnreadable = w.__smokeUnreadable.filter((id) => !ids.includes(id));
    ids.forEach((id) => relocated.add(id));
    return ids.map((id) => ({ track_id: id, source_path: `${folder}/Smoke Track ${String(id).padStart(2, '0')}.mp3` }));
  }

  function stickAt(path: string): SmokeStick {
    const stick = w.__smokeSticks[path];
    if (!stick) throw new Error(`smoke: no stick at ${path}`);
    return stick;
  }

  function stickContents(stick: SmokeStick) {
    const contents = JSON.parse(JSON.stringify(stick.contents));
    if (localStorage.getItem('smoke.sticks.oldEngine') === '1') delete contents.kept_as_is;
    return contents;
  }

  function editStick(path: string, edit: (stick: SmokeStick) => number) {
    const stick = stickAt(path);
    const changed = edit(stick);
    return [changed, stickContents(stick)];
  }

  function addIds(list: number[], ids: number[]) {
    const fresh = [...new Set(ids)].filter((id) => !list.includes(id));
    list.push(...fresh);
    return fresh.length;
  }

  function removeIds(list: number[], ids: number[]) {
    const before = list.length;
    const kept = list.filter((id) => !ids.includes(id));
    list.splice(0, list.length, ...kept);
    return before - kept.length;
  }

  function wantedIds(selection: { playlist_ids: number[]; track_ids: number[] }) {
    const wanted = new Set<number>(selection.track_ids);
    for (const p of playlists) if (selection.playlist_ids.includes(p.id)) p.track_ids.forEach((id) => wanted.add(id));
    return [...wanted].filter((id) => tracks.some((t) => t.id === id));
  }

  function planTrack(id: number) {
    const t = tracks.find((x) => x.id === id);
    return { track_id: id, title: t?.title ?? `Track ${id}`, artist: t?.artist ?? '', size_bytes: t?.file_size ?? null };
  }

  function stickPlan(target: { volume: string; selection: { playlist_ids: number[]; track_ids: number[] } }) {
    const stick = stickAt(target.volume);
    const wanted = wantedIds(target.selection);
    const add = wanted.filter((id) => !stick.written.includes(id)).map(planTrack);
    const remove = stick.written.filter((id) => !wanted.includes(id)).map(planTrack);
    const bytesToAdd = add.reduce((sum, t) => sum + (t.size_bytes ?? 0), 0);
    const needed = bytesToAdd + 16 * 1024 * 1024 + 128 * 1024 * wanted.length;
    return {
      volume: target.volume.split('/').pop(),
      stick_id: stick.contents.stick_id,
      playlist_count: target.selection.playlist_ids.length,
      add,
      remove,
      update_count: 0,
      unchanged_count: wanted.length - add.length,
      skipped_unanalyzed: [],
      foreign_kept_count: stick.foreign,
      bytes_to_add: bytesToAdd,
      bytes_to_free: remove.reduce((sum, t) => sum + (t.size_bytes ?? 0), 0),
      bytes_needed: needed,
      free_bytes: stick.free,
      fits: needed <= stick.free,
    };
  }

  function recovered(libraryPath: string, keptPath: string | null) {
    w.__smokeLoadError = null;
    localStorage.removeItem('smoke.loadError');
    return { library_path: libraryPath, kept_path: keptPath, ui_settings: null };
  }

  async function handle(cmd: string, args: Record<string, any>): Promise<unknown> {
    switch (cmd) {
      case 'get_stick':
        return stickContents(stickAt(args.path));
      case 'stick_add_tracks':
        return editStick(args.path, (s) => addIds(s.contents.track_ids, args.trackIds));
      case 'stick_remove_tracks':
        return editStick(args.path, (s) => removeIds(s.contents.track_ids, args.trackIds));
      case 'stick_add_playlists':
        return editStick(args.path, (s) => addIds(s.contents.playlist_ids, args.playlistIds));
      case 'stick_remove_playlists':
        return editStick(args.path, (s) => removeIds(s.contents.playlist_ids, args.playlistIds));
      case 'stick_set_keep_updated':
        return editStick(args.path, (s) => {
          const before = (s.contents.kept_as_is ?? []).length;
          const kept = (s.contents.kept_as_is ?? []).filter((k) => k.playlist_id !== args.playlistId);
          if (!args.keepUpdated) kept.push({ playlist_id: args.playlistId, as_of: new Date().toISOString() });
          s.contents.kept_as_is = kept;
          return kept.length === before ? 0 : 1;
        });
      case 'get_volume_capacity': {
        const stick = stickAt(args.path);
        return { total_bytes: stick.total, free_bytes: stick.free };
      }
      case 'stick_track_path': {
        const t = tracks.find((x) => x.id === args.trackId);
        return t && stickAt(args.path).written.includes(t.id) ? `${args.path}/Contents/${t.artist}/${t.title}.mp3` : null;
      }
      case 'read_usb_state': {
        const stick = stickAt(args.path);
        const onStick = tracks.filter((t) => stick.written.includes(t.id));
        return {
          tracks: onStick.map((t) => ({
            id: t.id,
            title: t.title,
            artist: t.artist,
            album: t.album,
            genre: t.genre,
            key: t.key,
            bpm: t.tempo / 100,
            duration: t.duration_secs,
            usb_path: `/Contents/${t.artist}/${t.title}.mp3`,
          })),
          playlists: stick.playlists,
          last_sync: stick.last_sync
            ? { at: stick.last_sync, report: { tracks_added: 0, tracks_updated: 0, tracks_replaced: 0, tracks_removed: 0, tracks_unchanged: onStick.length } }
            : null,
        };
      }
      case 'plan_usb_sync':
        return stickPlan(args.target);
      case 'sync_usb_many':
        return args.targets.map((target: { volume: string; selection: { playlist_ids: number[]; track_ids: number[] } }) => {
          const plan = stickPlan(target);
          const stick = stickAt(target.volume);
          stick.written = wantedIds(target.selection);
          stick.last_sync = new Date().toISOString();
          return {
            volume: plan.volume,
            path: target.volume,
            status: 'done',
            report: { tracks_added: plan.add.length, tracks_updated: 0, tracks_replaced: 0, tracks_removed: plan.remove.length, tracks_unchanged: plan.unchanged_count },
            error: null,
            skipped_unanalyzed: 0,
            elapsed_ms: 10,
          };
        });
      case 'eject_volume':
        delete w.__smokeSticks[args.path];
        return null;
      case 'get_library_health':
      case 'check_library_files':
        return health();
      case 'relocate_missing_in_folder': {
        const notFound = w.__smokeMissing.filter((id) => w.__smokeNotFound.includes(id));
        const matched = relink(w.__smokeMissing.filter((id) => !notFound.includes(id)), args.folder);
        return { report: { matched, ambiguous: [], not_found: notFound }, health: health() };
      }
      case 'locate_track_file':
        return { relinked: { track_id: args.trackId, source_path: args.path }, health: (relink([args.trackId], ''), health()) };
      case 'survey_import': {
        if (w.__smokeSurvey) return w.__smokeSurvey;
        const files = (w.__smokeImport?.tracks ?? []).map((t) => t.source_path);
        return (args.paths as string[]).map((path) =>
          files.includes(path) ? { path, folder: false, files: [path] } : { path, folder: true, files: files.filter((f) => f.startsWith(`${path}/`)) },
        );
      }
      case 'plan_import': {
        const next = w.__smokeImport;
        pendingImport = next?.tracks ?? [];
        if (w.__smokeHoldImport) await new Promise<void>((done) => (w.__smokeReleaseImport = done));
        return {
          token: 1,
          new_count: next ? next.tracks.length : 2,
          already_in_library: next?.already ?? 0,
          duplicates: w.__smokeDuplicates,
          skipped: { unsupported: 0, unsupported_exts: [], unreadable: 0, ...next?.skipped },
        };
      }
      case 'commit_import':
        tracks.push(...pendingImport);
        return {
          added: pendingImport.splice(0),
          skipped_ids: args.skipDuplicates ? (w.__smokeDuplicates as { existing_id: number }[]).map((d) => d.existing_id) : [],
        };
      case 'get_backup_status':
        return { sizes: { library_bytes: 38_000_000, audio_bytes: 61_000_000_000, audio_files: libraryTrackCount }, last: lastBackup };
      case 'export_backup': {
        emit('backup-progress', { phase: 'export', done: 27_450_000_000, total: 61_038_000_000 });
        if (w.__smokeHoldExport) await new Promise<void>((done) => (w.__smokeReleaseExport = done));
        lastBackup = { path: args.path, at_ms: Date.now(), bytes: 38_000_000, includes_audio: args.includeAudio };
        return lastBackup;
      }
      case 'find_reconnect_candidates':
        return w.__smokeReconnectPlan ?? { reconnect: [], needs_choice: [], still_missing: [...w.__smokeMissing] };
      case 'apply_reconnects': {
        const picks = args.picks as { track_id: number; path: string }[];
        relink(picks.map((p) => p.track_id), '');
        return { relinked: picks.map((p) => ({ track_id: p.track_id, source_path: p.path })), health: health() };
      }
      case 'preview_relocate_folder':
        return w.__smokeRemapPreview;
      case 'apply_relocate_folder': {
        const ids = [...w.__smokeMissing];
        const remapped = relink(ids, args.newRoot);
        lastRelocation = { id: 1, old_root: args.oldRoot, new_root: args.newRoot, tracks: ids.length, created_at: new Date().toISOString() };
        return { applied: { relocation_id: 1, remapped, still_missing: w.__smokeMissing.length }, health: health() };
      }
      case 'undo_relocate_folder':
        lastRelocation = null;
        return { undone: { reverted: [], skipped: 0 }, health: health() };
      case 'last_folder_relocation':
        return lastRelocation;
      case 'list_duplicate_groups':
        return w.__smokeDupGroups;
      case 'list_duplicate_archive':
        return w.__smokeDupArchive;
      case 'apply_duplicate_merges': {
        const requests = args.requests as { keeper_id: number; remove_ids: number[] }[];
        const merged = requests.map((request) => {
          const group = w.__smokeDupGroups.find((g) => g.members.some((m) => m.id === request.keeper_id))!;
          const keeper = group.members.find((m) => m.id === request.keeper_id)!;
          const merge_id = w.__smokeDupArchive.length + 1;
          w.__smokeDupArchive = [
            {
              merge_id,
              merged_at: '2026-11-21T09:00:00Z',
              confidence: group.confidence,
              keeper_id: keeper.id,
              keeper_label: `${keeper.artist} - ${keeper.title}`,
              tracks: group.members.filter((m) => request.remove_ids.includes(m.id)),
            },
            ...w.__smokeDupArchive,
          ];
          w.__smokeDupGroups = w.__smokeDupGroups.filter((g) => g !== group);
          return { merge_id, keeper_id: keeper.id, removed_ids: request.remove_ids };
        });
        return { merged, failed: [] };
      }
      case 'restore_duplicate_merge': {
        const restored = w.__smokeDupArchive.find((m) => m.merge_id === args.mergeId)!;
        w.__smokeDupArchive = w.__smokeDupArchive.filter((m) => m !== restored);
        return { merge_id: restored.merge_id, restored_ids: restored.tracks.map((t) => t.id), keeper_reverted: true };
      }
      case 'playlist_export_file_name':
        return `Peak Time 2026-11-21.${({ m3u8: 'm3u8', csv: 'csv', text: 'txt', traktor_nml: 'nml', rekordbox_xml: 'xml' } as Record<string, string>)[args.format]}`;
      case 'export_playlist':
        return { written: 3, missing: 1, missing_examples: ['/Music/gone.mp3'] };
      case 'playlist_tracklist':
        return 'Artist One - Track One\nArtist Two - Track Two\n';
      case 'read_backup':
        if (!String(args.path).endsWith('.tar.gz')) throw 'This file is not a library backup';
        return {
          path: args.path,
          created_at: new Date(Date.now() - 12 * 86_400_000).toISOString(),
          track_count: 176,
          playlist_count: 4,
          includes_audio: false,
          audio_files: 0,
          current_tracks: libraryTrackCount,
        };
      case 'restore_backup':
        sessionStorage.setItem('smoke.restore', JSON.stringify(args));
        return { ui_settings: JSON.stringify({ 'fourfour.quantize.v1': '1' }), safety_path: '/smoke/Backups/before-restore.tar.gz', relinked: 0 };
      case 'recover_retry_library':
        if (w.__smokeLoadError) throw w.__smokeLoadError;
        return recovered('/smoke/library/library.db', null);
      case 'recover_open_library':
        return recovered(`${args.folderPath}/library.db`, '/smoke/library/library.db');
      case 'recover_new_library':
        return recovered('/smoke/Libraries/library-2026-11-02-100000/library.db', '/smoke/library/library.db');
      case 'recover_restore_backup':
        sessionStorage.setItem('smoke.restore', JSON.stringify(args));
        recovered('/smoke/Libraries/library-2026-11-02-100000/library.db', '/smoke/library/library.db');
        return { library_path: '/smoke/Libraries/library-2026-11-02-100000/library.db', kept_path: '/smoke/library/library.db', ui_settings: JSON.stringify({ 'fourfour.quantize.v1': '1' }) };
      case 'plugin:dialog|save':
        return `/Backups/${args.options?.defaultPath ?? 'backup.tar.gz'}`;
      case 'plugin:dialog|open':
        return w.__smokePickPath;
      case 'load_state':
        if (w.__smokeHoldLoad) await new Promise<void>((done) => (w.__smokeReleaseLoad = done));
        if (w.__smokeLoadError) throw w.__smokeLoadError;
        return { tracks: tracks.map(({ waveform_bands: _bands, waveform_preview: _preview, ...listed }) => listed), playlists };
      case 'get_row_previews': {
        if (w.__smokeIpcMs) await new Promise((done) => setTimeout(done, w.__smokeIpcMs));
        const wanted = new Set(args.trackIds as number[]);
        return tracks
          .filter((t) => wanted.has(t.id) && (t.waveform_bands || t.waveform_preview))
          .map((t) => ({ id: t.id, waveform_bands: t.waveform_bands ?? undefined, waveform_preview: t.waveform_preview ?? undefined }));
      }
      case 'refresh_track_labels':
        return w.__smokeLabelChanges;
      case 'save_state':
        playlists = args.playlists;
        return null;
      case 'get_analysis_data':
        return analysisFor(args.trackId);
      case 'set_cues':
        edits.cues.set(args.trackId, args.cues);
        return null;
      case 'set_beat_grid':
        edits.grids.set(args.trackId, { beats: args.beats, grid_offset_ms: args.offsetMs, grid_anchor_ms: args.anchorMs });
        return null;
      case 'rescale_beat_grid':
        edits.tempo.set(args.trackId, args.bpm);
        return null;
      case 'playback_play':
        Object.assign(playback, {
          path: args.path,
          position: args.positionMs,
          duration: args.durationMs ?? 240_000,
          playing: true,
        });
        startTimer();
        tick();
        return null;
      case 'playback_pause':
        playback.playing = false;
        tick();
        return playback.position;
      case 'playback_resume':
        playback.playing = true;
        tick();
        return null;
      case 'playback_seek':
      case 'playback_set_position':
        playback.position = args.positionMs;
        tick();
        return null;
      case 'playback_stop':
        Object.assign(playback, { playing: false, position: 0 });
        tick();
        return null;
      case 'playback_configure':
      case 'playback_get_config':
        return { sample_rate: 44100, buffer_frames: 512 };
      case 'playback_list_output_devices':
        return [{ name: 'Smoke Output', is_default: true }];
      case 'playback_get_output_device':
        return null;
      case 'playback_set_output_device':
      case 'playback_set_tempo':
      case 'playback_set_key_shift':
      case 'playback_set_key_lock':
        return null;
      case 'get_waveform_detail':
        if (w.__smokeDetail === 'none') throw new Error('smoke: no audio');
        if (w.__smokeDetail === 'hold') await new Promise<void>((done) => (w.__smokeReleaseDetail = done));
        return detailFor(args.trackId);
      case 'get_waveform_window':
        throw new Error('smoke: no audio');
      case 'prefetch_waveform_detail':
        return null;
      case 'get_track_artwork': {
        if (w.__smokeIpcMs) await new Promise((done) => setTimeout(done, w.__smokeIpcMs));
        return smokeArtwork(args.trackId, args.maxPx);
      }
      case 'get_track_artworks': {
        if (w.__smokeIpcMs) await new Promise((done) => setTimeout(done, w.__smokeIpcMs));
        const images = await Promise.all((args.trackIds as number[]).map((id) => smokeArtwork(id, args.maxPx)));
        const total = images.reduce((sum, image) => sum + image.byteLength, 0);
        const out = new Uint8Array(4 + 4 * images.length + total);
        const view = new DataView(out.buffer);
        view.setUint32(0, images.length, true);
        let at = 4 + 4 * images.length;
        images.forEach((image, i) => {
          view.setUint32(4 + 4 * i, image.byteLength, true);
          out.set(new Uint8Array(image), at);
          at += image.byteLength;
        });
        return out.buffer;
      }
      case 'get_mounted_volumes':
        return Object.keys(w.__smokeSticks);
      case 'app_version':
        return '0.0.0-smoke';
      case 'get_library_path':
        return '/smoke/library';
      case 'get_auto_analyze_new':
        return false;
      case 'get_analyze_after_import':
        return w.__smokeAnalyzeAfterImport;
      case 'set_analyze_after_import':
        w.__smokeAnalyzeAfterImport = args.enabled;
        return null;
      case 'get_analysis_backend':
        return null;
      case 'set_analysis_backend':
        return null;
      case 'log_frontend':
        return null;
      case 'log_directory':
        return '/smoke/logs';
      case 'resume_analysis':
        return 0;
      case 'enqueue_analysis':
        return 0;
      case 'analysis_ui_busy':
        (w.__ffUiBusy as (() => void) | undefined)?.();
        return null;
      case 'focus_analysis':
      case 'prioritize_analysis':
      case 'cancel_analysis':
      case 'set_track_key':
      case 'set_track_rating':
      case 'release_user_edits':
      case 'release_user_field':
      case 'analyze_track_ids':
        return null;
      case 'plugin:event|listen': {
        const id = nextListenerId++;
        listeners.set(id, { event: args.event, handler: args.handler });
        return id;
      }
      case 'plugin:event|unlisten':
        listeners.delete(args.eventId);
        return null;
      case 'plugin:dialog|message':
        w.__smokeDialogs.push(String(args.message));
        return w.__smokeAnswerDialog ? 'Yes' : 'No';
      case 'plugin:window|outer_size':
        return { width: 1600, height: 1000 };
      case 'plugin:window|set_size':
      case 'plugin:window|start_dragging':
        return null;
      default:
        w.__smokeUnhandled.push(cmd);
        return null;
    }
  }

  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  w.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    transformCallback(callback: (message: unknown) => void, once = false) {
      const id = nextCallbackId++;
      w[`_${id}`] = (message: unknown) => {
        if (once) delete w[`_${id}`];
        callback(message);
      };
      return id;
    },
    unregisterCallback(id: number) {
      delete w[`_${id}`];
    },
    convertFileSrc: (path: string) => path,
    async invoke(cmd: string, args: Record<string, unknown> = {}) {
      w.__smokeInvokes.push({ cmd, args });
      if (served && cmd === 'load_state') {
        const answer = await served.answer;
        playlists = answer.playlists;
        return answer;
      }
      if (served && cmd === 'get_row_previews') return served.previews(args.trackIds as number[]);
      if (served && SERVED_LIST_COMMANDS.has(cmd)) await served.tracks();
      return handle(cmd, args);
    },
  };
}
