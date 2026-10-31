import io, sys, os, json, zipfile, urllib.request

URL = 'https://zenodo.org/records/3967852/files/FSL10K.zip?download=1'


class HttpFile(io.RawIOBase):
    def __init__(self, url):
        self.url = url
        req = urllib.request.Request(url, method='HEAD')
        with urllib.request.urlopen(req, timeout=60) as r:
            self.url = r.url
            self.size = int(r.headers['Content-Length'])
        self.pos = 0
        self.fetched = 0

    def seekable(self):
        return True

    def readable(self):
        return True

    def seek(self, off, whence=0):
        self.pos = off if whence == 0 else self.pos + off if whence == 1 else self.size + off
        return self.pos

    def tell(self):
        return self.pos

    def read(self, n=-1):
        if n is None or n < 0:
            n = self.size - self.pos
        if n == 0 or self.pos >= self.size:
            return b''
        end = min(self.pos + n, self.size) - 1
        req = urllib.request.Request(self.url, headers={'Range': f'bytes={self.pos}-{end}'})
        with urllib.request.urlopen(req, timeout=120) as r:
            data = r.read()
        self.pos += len(data)
        self.fetched += len(data)
        return data

    def readinto(self, b):
        d = self.read(len(b))
        b[:len(d)] = d
        return len(d)


def main():
    out = sys.argv[1]
    cmd = sys.argv[2]
    f = io.BufferedReader(HttpFile(URL), buffer_size=1 << 20)
    raw = f.raw
    z = zipfile.ZipFile(f)
    names = z.namelist()
    if cmd == 'list':
        json.dump(names, open(os.path.join(out, 'names.json'), 'w'))
        print(len(names), names[:20], raw.fetched)
    elif cmd == 'get':
        wanted = set(open(sys.argv[3]).read().split())
        done = 0
        for n in names:
            base = os.path.basename(n)
            if n in wanted or base in wanted:
                dst = os.path.join(out, base)
                if os.path.exists(dst):
                    continue
                with open(dst + '.part', 'wb') as w:
                    w.write(z.read(n))
                os.rename(dst + '.part', dst)
                done += 1
                if done % 100 == 0:
                    print(done, raw.fetched // 1000000, 'MB', flush=True)
        print('done', done, raw.fetched // 1000000, 'MB')


main()
