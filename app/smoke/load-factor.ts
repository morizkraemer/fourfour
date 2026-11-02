import { cpus, loadavg } from 'node:os';

export function loadFactor(): number {
  const forced = Number(process.env.FF_SMOKE_SLOWMO_FACTOR);
  if (Number.isFinite(forced) && forced > 0) return forced;
  const perCore = loadavg()[0] / Math.max(1, cpus().length);
  return Math.min(4, Math.max(1, Math.ceil(perCore)));
}
