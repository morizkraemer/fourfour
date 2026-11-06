declare module 'node:fs' {
  export function readFileSync(path: URL | string): Uint8Array;
  export function readFileSync(path: URL | string, encoding: 'utf8'): string;
  export function readdirSync(path: URL | string): string[];
  export function writeFileSync(path: URL | string, data: string): void;
}

declare module 'node:crypto' {
  export function createHash(algorithm: 'sha256'): { update(data: string | Uint8Array): void; digest(encoding: 'hex'): string };
}

declare module 'node:inspector' {
  export class Session {
    connect(): void;
    disconnect(): void;
    post(method: string, params?: object, callback?: (error: unknown, result: { profile: unknown }) => void): void;
    post(method: string, callback: (error: unknown, result: { profile: unknown }) => void): void;
  }
}
