import { currentPositionMs, player, seekToMs } from './player.svelte.ts';
import { formatClock, isSeekKey, seekTargetMs } from '../lib/waveSeek.ts';

export function waveSliderProps() {
  const track = player.track;
  if (!track) return {};
  const durationMs = track.durationMs;
  return {
    role: 'slider',
    tabindex: 0,
    'aria-label': 'Playback position',
    'aria-orientation': 'horizontal',
    'aria-valuemin': 0,
    'aria-valuemax': Math.floor(durationMs / 1000),
    'aria-valuenow': Math.floor((player.progress * durationMs) / 1000),
    'aria-valuetext': `${formatClock(player.progress * durationMs)} of ${formatClock(durationMs)}`,
    'data-wave-slider': '',
    onkeydown: onWaveSliderKeydown,
  };
}

export function onWaveSliderKeydown(e: KeyboardEvent) {
  const track = player.track;
  if (!track || e.target !== e.currentTarget || e.ctrlKey || e.metaKey || e.altKey || !isSeekKey(e.key)) return;
  e.preventDefault();
  e.stopPropagation();
  void seekToMs(seekTargetMs(e.key, e.shiftKey, { positionMs: currentPositionMs(), durationMs: track.durationMs, beats: track.beats }));
}
