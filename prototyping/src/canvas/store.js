/*
 * Position store. localStorage only — holds just the user's manual drag
 * overrides, keyed by artboard id: { [id]: { x, y } }. Everything else
 * (which artboards exist, default layout) is derived from code each load.
 */
const KEY = 'fourfour:canvas:positions';

function read() {
  try {
    return JSON.parse(localStorage.getItem(KEY)) || {};
  } catch {
    return {};
  }
}

export function load() {
  return read();
}

export function savePos(id, pos) {
  const all = read();
  all[id] = pos;
  localStorage.setItem(KEY, JSON.stringify(all));
}

export function reset() {
  localStorage.removeItem(KEY);
}
