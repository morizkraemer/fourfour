import fs from 'node:fs';
import type { IncomingMessage, ServerResponse } from 'node:http';
import path from 'node:path';
import type { Plugin } from 'vite';
import { parseNotePost } from '../src/model';
import { createStore } from './store';

const MIME: Record<string, string> = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.mp4': 'video/mp4',
  '.webm': 'video/webm',
};

const MAX_BODY = 64_000;

function send(res: ServerResponse, status: number, body: unknown): void {
  res.statusCode = status;
  res.setHeader('content-type', 'application/json');
  res.setHeader('cache-control', 'no-store');
  res.end(JSON.stringify(body));
}

function readBody(req: IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks: Buffer[] = [];
    req.on('data', (c: Buffer) => {
      size += c.length;
      if (size > MAX_BODY) {
        reject(new Error('body too large'));
        req.destroy();
        return;
      }
      chunks.push(c);
    });
    req.on('end', () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')));
      } catch (e) {
        reject(e);
      }
    });
    req.on('error', reject);
  });
}

function serveMedia(mediaDir: string, req: IncomingMessage, res: ServerResponse): void {
  let file = '';
  try {
    file = path.resolve(mediaDir, decodeURIComponent((req.url ?? '').split('?')[0]!.slice('/media/'.length)));
  } catch {
    file = '';
  }
  const type = MIME[path.extname(file).toLowerCase()];
  if (file === '' || !file.startsWith(mediaDir + path.sep) || !type || !fs.existsSync(file)) {
    res.statusCode = 404;
    res.end('not found');
    return;
  }
  const size = fs.statSync(file).size;
  res.setHeader('content-type', type);
  res.setHeader('accept-ranges', 'bytes');
  res.setHeader('cache-control', 'public, max-age=3600');
  const range = /^bytes=(\d*)-(\d*)$/.exec(req.headers.range ?? '');
  if (!range) {
    res.setHeader('content-length', size);
    fs.createReadStream(file).pipe(res);
    return;
  }
  const start = range[1] === '' ? size - Number(range[2]) : Number(range[1]);
  const end = range[1] === '' || range[2] === '' ? size - 1 : Math.min(Number(range[2]), size - 1);
  if (!(start >= 0 && start <= end)) {
    res.statusCode = 416;
    res.setHeader('content-range', `bytes */${size}`);
    res.end();
    return;
  }
  res.statusCode = 206;
  res.setHeader('content-range', `bytes ${start}-${end}/${size}`);
  res.setHeader('content-length', end - start + 1);
  fs.createReadStream(file, { start, end }).pipe(res);
}

export function labApi(root: string): Plugin {
  const store = createStore(root);
  const middleware = (req: IncomingMessage, res: ServerResponse, next: () => void): void => {
    const url = req.url ?? '';
    if (url.startsWith('/media/') && (req.method === 'GET' || req.method === 'HEAD')) return serveMedia(store.mediaDir, req, res);
    if (url === '/__lab/cards' && req.method === 'GET') return send(res, 200, { cards: store.listCards() });
    if (url === '/__lab/note' && req.method === 'POST') {
      readBody(req)
        .then((body) => send(res, 200, { note: store.addNote(parseNotePost(body)) }))
        .catch((e: Error) => send(res, 400, { error: e.message }));
      return;
    }
    next();
  };
  return {
    name: 'lab-api',
    configureServer: (server) => void server.middlewares.use(middleware),
    configurePreviewServer: (server) => void server.middlewares.use(middleware),
  };
}
