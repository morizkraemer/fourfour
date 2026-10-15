const PREVIEW_LEN = 400;
const COLOR_LEN = 600;

function envelope(i: number, count: number, seed: number): number {
  const t = i / count;
  const phrase = 0.55 + 0.45 * Math.sin(t * Math.PI * (6 + seed));
  const grain = 0.5 + 0.5 * Math.sin(i * 0.9 + seed);
  const breakdown = t > 0.4 && t < 0.55 ? 0.35 : 1;
  return Math.min(1, phrase * (0.6 + 0.4 * grain) * breakdown);
}

export function mockAnalysisData(trackId: number, durationSecs: number, tempo: number, key: string) {
  const durationMs = Math.max(1, Math.round(durationSecs * 1000));
  const bpm = tempo / 100;
  const beatMs = 60000 / bpm;
  const beats: { time_ms: number; bar_position: number }[] = [];
  for (let t = 0, n = 0; t < durationMs; t += beatMs, n++) {
    beats.push({ time_ms: Math.round(t), bar_position: (n % 4) + 1 });
  }
  const waveform_preview = Array.from({ length: PREVIEW_LEN }, (_, i) => {
    const height = 4 + Math.round(envelope(i, PREVIEW_LEN, trackId) * 27);
    return (3 << 5) | height;
  });
  const waveform_color = Array.from({ length: COLOR_LEN }, (_, i) => {
    const amp = envelope(i, COLOR_LEN, trackId);
    const warmth = 0.5 + 0.5 * Math.sin(i / 40 + trackId);
    return { amp, r: 0.3 + 0.7 * warmth, g: 0.4 + 0.3 * (1 - warmth), b: 0.9 - 0.6 * warmth };
  });
  return {
    track_id: trackId,
    duration_ms: durationMs,
    bpm,
    key,
    beats,
    cue_points: [{ time_ms: Math.round(durationMs * 0.08) }, { time_ms: Math.round(durationMs * 0.5) }],
    waveform_preview,
    waveform_color,
    waveform_color_detail: waveform_color,
    waveform_overview: waveform_color,
  };
}
