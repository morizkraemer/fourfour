export type PadAction = 'set' | 'jump' | 'delete';

export function padAction(altHeld: boolean, hasCue: boolean): PadAction {
  if (!hasCue) return 'set';
  return altHeld ? 'delete' : 'jump';
}

export function padFace(letter: string, hasCue: boolean, altHeld: boolean): string {
  return hasCue && altHeld ? '×' : letter;
}
