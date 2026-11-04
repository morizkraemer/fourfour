import { readFileSync } from 'node:fs';

const node = (globalThis as unknown as { process: { cwd(): string; env: Record<string, string | undefined> } }).process;

export const env = node.env;

export function fixtureText(name: string): string {
  return readFileSync(`${node.cwd()}/src/services/fixtures/${name}`, 'utf8');
}

export function bufferOf(path: string): ArrayBuffer {
  const bytes = readFileSync(path);
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

export function fixtureBuffer(name: string): ArrayBuffer {
  return bufferOf(`${node.cwd()}/src/services/fixtures/${name}`);
}
