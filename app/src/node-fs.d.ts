declare module 'node:fs' {
  export function readFileSync(path: URL | string, encoding: 'utf8'): string;
  export function writeFileSync(path: URL | string, data: string): void;
}
