<script>
  let { selected = '8A', size = 150 } = $props();
  let sel = $derived({ n: parseInt(selected), l: selected.slice(-1) });
  function pos(n, ring) {
    const a = ((n % 12) * 30 - 90) * (Math.PI / 180);
    const r = ring === 'B' ? 58 : 36;
    return [75 + r * Math.cos(a), 75 + r * Math.sin(a)];
  }
  function compat(n, l) {
    if (n === sel.n && l === sel.l) return 'self';
    if (n === sel.n) return 'near';
    if (l === sel.l && (n === (sel.n % 12) + 1 || n === ((sel.n + 10) % 12) + 1)) return 'near';
    return '';
  }
  const items = [];
  for (let n = 1; n <= 12; n++) for (const l of ['A', 'B']) items.push({ n, l });
</script>

<svg viewBox="0 0 150 150" width={size} height={size}>
  <circle cx="75" cy="75" r="58" fill="none" stroke="var(--ff-border)" stroke-dasharray="2 3" />
  <circle cx="75" cy="75" r="36" fill="none" stroke="var(--ff-border)" stroke-dasharray="2 3" />
  {#each items as it}
    {@const p = pos(it.n, it.l)}
    {@const c = compat(it.n, it.l)}
    <g>
      <circle cx={p[0]} cy={p[1]} r="9.5" fill={c === 'self' ? `var(--ff-key-${it.n})` : 'var(--ff-elev)'} stroke={c ? `var(--ff-key-${it.n})` : 'var(--ff-border)'} stroke-width={c ? 1.5 : 1} />
      <text x={p[0]} y={p[1] + 3.2} text-anchor="middle" font-size="8.5" font-family="var(--ff-font-mono)" font-weight="500" fill={c === 'self' ? '#0a0a0a' : `var(--ff-key-${it.n})`}>{it.n}{it.l}</text>
    </g>
  {/each}
  <text x="75" y="73" text-anchor="middle" font-size="7" fill="var(--ff-faint)" font-family="var(--ff-font-mono)">KEY</text>
  <text x="75" y="83" text-anchor="middle" font-size="9" fill="var(--ff-text)" font-family="var(--ff-font-mono)">{selected}</text>
</svg>
