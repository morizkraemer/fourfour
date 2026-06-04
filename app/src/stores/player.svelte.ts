/**
 * player.svelte.ts — Playback state for the compact player bar.
 * State in a single object so consumers can read reactive fields.
 * Static demo data for now; Tauri wiring in Phase 6.
 */

export const player = $state({
  playing: false,
  progress: 0.35,
  track: {
    title: 'Voidwalker',
    artist: 'Tale Of Us',
    bpm: '122.00',
    key: '7A',
    totalSeconds: 494, // 8:14
    cues: [
      { position: 0.0,  color: '#ec4899' }, // Intro
      { position: 0.19, color: '#38bdf8' }, // Drop
      { position: 0.58, color: '#4ade80' }, // Break
    ],
  },
});

export function currentTime() {
  const sec = Math.floor(player.progress * player.track.totalSeconds);
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export function currentTimeMs() {
  return `.${Math.floor(player.progress * 10) % 10}`;
}

export function totalTime() {
  const sec = player.track.totalSeconds;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

/* ── Actions ────────────────────────────────────────────────────────── */

export function togglePlay() {
  player.playing = !player.playing;
}

export function play() {
  player.playing = true;
}

export function pause() {
  player.playing = false;
}
