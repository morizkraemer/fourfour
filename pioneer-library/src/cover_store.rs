//! Cover images as files next to the library, one per distinct image, named by the SHA-256 of
//! its bytes: `<library folder>/covers/<first two hex digits>/<hash>`. The database keeps only
//! the hash (`artwork.cover_hash`).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};

pub const COVERS_DIR: &str = "covers";
const TEMP_PREFIX: &str = ".tmp-";

pub(crate) enum CoverStore {
    Dir(PathBuf),
    Memory(Mutex<HashMap<String, Vec<u8>>>),
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

impl CoverStore {
    pub(crate) fn for_library(db_path: Option<&Path>) -> Self {
        match db_path {
            Some(db) => Self::Dir(covers_dir_of(db)),
            None => Self::Memory(Mutex::default()),
        }
    }

    pub(crate) fn dir(&self) -> Option<&Path> {
        match self {
            Self::Dir(dir) => Some(dir),
            Self::Memory(_) => None,
        }
    }

    fn file(dir: &Path, hash: &str) -> PathBuf {
        dir.join(&hash[..2]).join(hash)
    }

    /// Stores `bytes` (unless an identical image is stored already) and returns its hash.
    pub(crate) fn put(&self, bytes: &[u8]) -> Result<String> {
        let hash = sha256_hex(bytes);
        match self {
            Self::Memory(map) => {
                map.lock().expect("cover store lock").entry(hash.clone()).or_insert_with(|| bytes.to_vec());
            }
            Self::Dir(dir) => {
                let target = Self::file(dir, &hash);
                if !target.is_file() {
                    write_synced(&target, bytes)?;
                }
            }
        }
        Ok(hash)
    }

    pub(crate) fn get(&self, hash: &str) -> Result<Option<Vec<u8>>> {
        if !is_hash(hash) {
            return Ok(None);
        }
        match self {
            Self::Memory(map) => Ok(map.lock().expect("cover store lock").get(hash).cloned()),
            Self::Dir(dir) => {
                let path = Self::file(dir, hash);
                match std::fs::read(&path) {
                    Ok(bytes) => Ok(Some(bytes)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(e).with_context(|| format!("Failed to read cover {}", path.display())),
                }
            }
        }
    }

    /// Deletes the stored image (best effort: a file left behind is removed by the next sweep).
    pub(crate) fn remove(&self, hash: &str) {
        if !is_hash(hash) {
            return;
        }
        match self {
            Self::Memory(map) => {
                map.lock().expect("cover store lock").remove(hash);
            }
            Self::Dir(dir) => {
                let _ = std::fs::remove_file(Self::file(dir, hash));
            }
        }
    }

    /// Every stored hash, and temp files a killed write left behind.
    pub(crate) fn list(&self) -> (Vec<String>, Vec<PathBuf>) {
        match self {
            Self::Memory(map) => (map.lock().expect("cover store lock").keys().cloned().collect(), Vec::new()),
            Self::Dir(dir) => {
                let mut hashes = Vec::new();
                let mut temps = Vec::new();
                let Ok(shards) = std::fs::read_dir(dir) else { return (hashes, temps) };
                for shard in shards.flatten() {
                    let Ok(files) = std::fs::read_dir(shard.path()) else { continue };
                    for file in files.flatten() {
                        let name = file.file_name().to_string_lossy().into_owned();
                        if name.starts_with(TEMP_PREFIX) {
                            temps.push(file.path());
                        } else if is_hash(&name) {
                            hashes.push(name);
                        }
                    }
                }
                (hashes, temps)
            }
        }
    }
}

pub fn covers_dir_of(db_path: &Path) -> PathBuf {
    db_path.parent().unwrap_or(Path::new(".")).join(COVERS_DIR)
}

fn is_hash(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn write_synced(target: &Path, bytes: &[u8]) -> Result<()> {
    let dir = target.parent().expect("cover files sit in a shard folder");
    std::fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let temp = dir.join(format!(
        "{TEMP_PREFIX}{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_data()?;
        std::fs::rename(&temp, target)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written.with_context(|| format!("Failed to write cover {}", target.display()))
}

const ROUND: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const INITIAL: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];

fn compress(state: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];
    for (i, word) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(ROUND[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut state = INITIAL;
    let mut blocks = bytes.chunks_exact(64);
    for block in &mut blocks {
        compress(&mut state, block);
    }
    let rest = blocks.remainder();
    let mut tail = [0u8; 128];
    tail[..rest.len()].copy_from_slice(rest);
    tail[rest.len()] = 0x80;
    let tail_len = if rest.len() < 56 { 64 } else { 128 };
    tail[tail_len - 8..tail_len].copy_from_slice(&((bytes.len() as u64) * 8).to_be_bytes());
    for block in tail[..tail_len].chunks_exact(64) {
        compress(&mut state, block);
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            sha256_hex(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn a_stored_image_is_one_file_named_by_its_hash_and_shared_by_equal_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let store = CoverStore::Dir(dir.path().join(COVERS_DIR));
        let hash = store.put(b"cover").unwrap();
        assert_eq!(store.put(b"cover").unwrap(), hash);
        assert!(dir.path().join(COVERS_DIR).join(&hash[..2]).join(&hash).is_file());
        assert_eq!(store.get(&hash).unwrap(), Some(b"cover".to_vec()));
        assert_eq!(store.list(), (vec![hash.clone()], Vec::new()));
        store.remove(&hash);
        assert_eq!(store.get(&hash).unwrap(), None);
        assert_eq!(store.get("../../etc/passwd").unwrap(), None);
    }
}
