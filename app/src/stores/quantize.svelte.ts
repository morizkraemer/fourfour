export const QUANTIZE_STORAGE_KEY = 'fourfour.quantize.v1';

function loadQuantize(): boolean {
  try {
    return localStorage.getItem(QUANTIZE_STORAGE_KEY) === '1';
  } catch {
    return false;
  }
}

export const quantize = $state({ on: loadQuantize() });

export function setQuantize(on: boolean) {
  quantize.on = on;
  try {
    localStorage.setItem(QUANTIZE_STORAGE_KEY, on ? '1' : '0');
  } catch {
    return;
  }
}
