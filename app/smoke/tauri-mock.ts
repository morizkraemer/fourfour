import type { LoadedState } from '../src/services/tauri.svelte.ts';
import type { CuePointPayload } from '../src/lib/cues.ts';

export interface SmokeWindow {
  __smokeInvokes: { cmd: string; args: unknown }[];
  __smokeUnhandled: string[];
  __smokeDialogs: string[];
  __smokeAnswerDialog: boolean;
  __smokeAnalysis: (trackId: number) => Promise<Record<string, unknown>>;
}

export function installTauriMock(initial: LoadedState) {
  const w = window as unknown as Window & SmokeWindow & Record<string, unknown>;
  w.__smokeInvokes = [];
  w.__smokeUnhandled = [];
  w.__smokeDialogs = [];
  w.__smokeAnswerDialog = true;

  let nextCallbackId = 1;
  const listeners = new Map<number, { event: string; handler: number }>();
  let nextListenerId = 1;

  const edits = {
    cues: new Map<number, CuePointPayload[]>(),
    grids: new Map<number, { beats: unknown[]; grid_offset_ms: number; grid_anchor_ms: number | null }>(),
    tempo: new Map<number, number>(),
  };
  let playlists = initial.playlists;
  const playback = { path: '', position: 0, duration: 0, playing: false, timer: 0 };

  function emit(event: string, payload: unknown) {
    for (const { event: name, handler } of listeners.values()) {
      if (name !== event) continue;
      const callback = w[`_${handler}`] as ((message: unknown) => void) | undefined;
      callback?.({ event, id: 0, payload });
    }
  }

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
    const data = await w.__smokeAnalysis(trackId);
    const grid = edits.grids.get(trackId);
    const cues = edits.cues.get(trackId);
    return { ...data, ...(grid ?? {}), ...(cues ? { cue_points: cues } : {}) };
  }

  async function handle(cmd: string, args: Record<string, any>): Promise<unknown> {
    switch (cmd) {
      case 'load_state':
        return { tracks: initial.tracks, playlists };
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
      case 'get_waveform_window':
        throw new Error('smoke: no audio');
      case 'get_track_artwork':
        return new ArrayBuffer(0);
      case 'get_mounted_volumes':
        return [];
      case 'app_version':
        return '0.0.0-smoke';
      case 'get_library_path':
        return '/smoke/library';
      case 'get_auto_analyze_new':
        return false;
      case 'get_analysis_backend':
        return null;
      case 'resume_analysis':
        return 0;
      case 'enqueue_analysis':
        return 0;
      case 'prioritize_analysis':
      case 'cancel_analysis':
      case 'set_track_key':
      case 'set_track_rating':
      case 'release_user_edits':
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
      return handle(cmd, args);
    },
  };
}
