declare module 'node:fs' {
  export function readFileSync(path: URL | string): Uint8Array;
  export function readFileSync(path: URL | string, encoding: 'utf8'): string;
  export function writeFileSync(path: URL | string, data: string): void;
}

declare module 'node:crypto' {
  export function createHash(algorithm: 'sha256'): { update(data: string): void; digest(encoding: 'hex'): string };
}
