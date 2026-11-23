import { describe, expect, it } from 'vitest';
import { buildAppActions } from './app-actions.ts';
import { player } from '../stores/player.svelte.ts';

const playerAction = buildAppActions().find((a) => a.id === 'view.player')!;

describe('Tab player toggle', () => {
  it('is bound to Tab', () => {
    expect(playerAction.defaultBinding).toBe('Tab');
  });

  it('fires with no track loaded, so Tab opens the empty player instead of moving focus', () => {
    expect(player.track).toBeNull();
    expect(playerAction.when?.() ?? true).toBe(true);
  });
});
