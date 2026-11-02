export type LogSend = (level: 'error' | 'warn', message: string) => Promise<unknown>;

interface ForwardingTarget {
  console: Pick<Console, 'error'>;
  addEventListener: (type: string, listener: (event: any) => void) => void;
}

const BURST = 20;
const WINDOW_MS = 10_000;

export function describeValue(value: unknown): string {
  if (value instanceof Error) return value.stack ?? `${value.name}: ${value.message}`;
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

export function installErrorForwarding(
  target: ForwardingTarget,
  send: LogSend,
  now: () => number = Date.now,
): void {
  let windowStart = now();
  let sent = 0;

  const forward = (level: 'error' | 'warn', message: string) => {
    const at = now();
    if (at - windowStart >= WINDOW_MS) {
      windowStart = at;
      sent = 0;
    }
    if (sent >= BURST) return;
    sent += 1;
    try {
      send(level, message).catch(ignoreLogFailure);
    } catch (err) {
      ignoreLogFailure(err);
    }
  };

  const original = target.console.error.bind(target.console);
  target.console.error = (...args: unknown[]) => {
    original(...args);
    forward('error', args.map(describeValue).join(' '));
  };
  target.addEventListener('error', (event) => {
    const where = event.filename ? ` at ${event.filename}:${event.lineno}:${event.colno}` : '';
    forward('error', `uncaught ${describeValue(event.error ?? event.message)}${where}`);
  });
  target.addEventListener('unhandledrejection', (event) => {
    forward('error', `unhandled rejection: ${describeValue(event.reason)}`);
  });
}

function ignoreLogFailure(_err: unknown): void {}
