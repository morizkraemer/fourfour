export const DECISIONS = {
  title: 'Zoom look: from the 3-band waveform to the samples (Q1-Q3)',
  feedback: [
    { id: 'Q1', text: 'The in-between zoom looks bad: a thin squiggly trace, neither waveform nor clean signal.' },
    { id: 'Q2', text: 'Zooming in shows the samples too big (wide flat steps).' },
    { id: 'Data', text: 'Every board draws the same real data (Andy Volt, Dendy Game, around 1:00.29, a kick onset at the playhead) with the reduction the app uses. Each board has a live row: drag the slider or scroll on it to judge the in-between.' },
  ],
  options: [
    { tag: 'Now', name: 'Morph', idea: 'Round 10: the envelope span slides towards the sample span, a thin trace in between.', board: 'zoom-look/current' },
    { tag: 'A', name: 'Body', idea: "Rekordbox-style: always a symmetric filled 3-band body, the outline sharpens to each column's loudest sample. Never a line, never the sign of the signal.", board: 'zoom-look/option-a' },
    { tag: 'B', name: 'Signal', idea: 'Ableton-style: the envelope fades into the sample span in one colour per column, mixed from the bands; a line with sample dots at the deepest zoom.', board: 'zoom-look/option-b' },
    { tag: 'C', name: 'Layered', idea: 'Soft band fill underneath, the signal as a crisp outline on top that fades in as you zoom.', board: 'zoom-look/option-c' },
    { tag: 'D', name: 'Filled', idea: 'The real signal filled to the centre line with the bands nested inside from the centre out: a waveform body at every zoom that still shows the onset and zero crossings.', board: 'zoom-look/option-d', rec: true },
  ],
  matrix: [
    { q: 'Reads as a waveform mid-zoom (1 s, 300 ms)', cells: ['-Thin trace', '+Yes', '-Line-like', 'Medium', '+Yes'] },
    { q: 'Shows the exact onset for grid Set', cells: ['+Yes', '-Blurred, symmetric', '+Yes', '+Yes', '+Yes'] },
    { q: 'Bands readable deep in', cells: ['-Squeezed', '+Yes', '-One mixed colour', 'Medium (faded)', '+Yes'] },
    { q: 'Calm while zooming', cells: ['-', '+', 'Medium', 'Medium', '+'] },
  ],
  recommendation: {
    pick: 'D: filled. It keeps the 3-band body at every zoom and still shows where the audio starts, so grid Set at sample level has something to aim at.',
    why: [
      'No zoom level turns it into a thin trace: the fill always reaches the centre line.',
      'A is the calmest but hides the onset and zero crossings, which is what deep zoom is for.',
      'The app draws D now; switching to another board is one constant (ZOOM_LOOK in waveform-zoom.js).',
    ],
    then: ['Pick a board, or a mix (for example A down to 300 ms, then D).', 'Judge it with the live row here and in the app on the Mac.'],
  },
};
