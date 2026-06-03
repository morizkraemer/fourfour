/** @schema 2.10 */
const bars = [];
const barWidth = 2;
const gap = 1;
const totalBars = Math.floor((pencil.width + gap) / (barWidth + gap));
const heights = [];
for (let i = 0; i < totalBars; i++) {
  heights.push(8 + Math.random() * 32);
}
const totalWidth = totalBars * barWidth + (totalBars - 1) * gap;
const startX = (pencil.width - totalWidth) / 2;
for (let i = 0; i < totalBars; i++) {
  const h = heights[i];
  bars.push({
    type: "rectangle",
    x: startX + i * (barWidth + gap),
    y: (pencil.height - h) / 2,
    width: barWidth,
    height: h,
    fill: "#3a3a3a",
  });
}
bars.push({
  type: "rectangle",
  x: pencil.width * 0.3,
  y: 0,
  width: 1,
  height: pencil.height,
  fill: "#fafafa",
});

const durationSec = 8 * 60 + 14;
const hotcues = [
  { sec: 0, color: "#EC4899" },      // A
  { sec: 2 * 60 + 54, color: "#38BDF8" }, // B
  { sec: 6 * 60 + 42, color: "#4ADE80" }, // C
  { sec: 8 * 60 + 8, color: "#F59E0B" },  // D
];

for (const cue of hotcues) {
  const x = (cue.sec / durationSec) * pencil.width;
  bars.push({
    type: "ellipse",
    x: Math.max(0, Math.min(pencil.width - 6, x - 3)),
    y: pencil.height - 8,
    width: 6,
    height: 6,
    fill: cue.color,
  });
}

return bars;
