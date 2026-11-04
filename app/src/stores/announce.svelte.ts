export const announcer = $state({ message: '', nonce: 0 });

export function announce(message: string) {
  announcer.message = message;
  announcer.nonce += 1;
}
