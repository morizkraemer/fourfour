import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const hooks = vi.hoisted(() => ({ ask: vi.fn(), confirm: vi.fn() }));

vi.mock('@tauri-apps/plugin-dialog', () => ({ ask: hooks.ask, open: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

import { confirmAction } from './confirmAction.ts';

function stubWindow(inTauri: boolean) {
  vi.stubGlobal('window', { confirm: hooks.confirm, ...(inTauri ? { __TAURI_INTERNALS__: {} } : {}) });
}

beforeEach(() => {
  hooks.ask.mockReset();
  hooks.confirm.mockReset();
});

afterEach(() => vi.unstubAllGlobals());

describe('confirmAction', () => {
  it('asks through the native Tauri dialog, because WKWebView ignores window.confirm', async () => {
    stubWindow(true);
    hooks.ask.mockResolvedValue(true);
    expect(await confirmAction('Delete playlist “A”?')).toBe(true);
    expect(hooks.ask).toHaveBeenCalledWith('Delete playlist “A”?', expect.objectContaining({ kind: 'warning' }));
    expect(hooks.confirm).not.toHaveBeenCalled();
  });

  it('returns false when the user cancels the native dialog', async () => {
    stubWindow(true);
    hooks.ask.mockResolvedValue(false);
    expect(await confirmAction('Delete?')).toBe(false);
  });

  it('falls back to window.confirm in the browser build', async () => {
    stubWindow(false);
    hooks.confirm.mockReturnValue(true);
    expect(await confirmAction('Delete?')).toBe(true);
    expect(hooks.confirm).toHaveBeenCalledWith('Delete?');
    expect(hooks.ask).not.toHaveBeenCalled();
  });
});
