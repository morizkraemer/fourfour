use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use pioneer_library::LocalLibrary;

pub const BATCH_THUMBNAIL_THREADS: usize = 4;

const ARTWORK_THUMB_CACHE_LIMIT: usize = 400;
const ARTWORK_THUMB_JPEG_QUALITY: u8 = 85;

#[derive(Default)]
pub struct ArtworkThumbCache {
    entries: HashMap<(i64, u32, u64), Arc<Vec<u8>>>,
    order: VecDeque<(i64, u32, u64)>,
}

pub type SharedArtworkThumbs = Arc<Mutex<ArtworkThumbCache>>;

fn bytes_fingerprint(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

pub fn artwork_thumbnail(bytes: &[u8], max_px: u32) -> Vec<u8> {
    let Ok(decoded) = image::load_from_memory(bytes) else {
        return bytes.to_vec();
    };
    if decoded.width() <= max_px && decoded.height() <= max_px {
        return bytes.to_vec();
    }
    let thumb = decoded.thumbnail(max_px, max_px).to_rgb8();
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, ARTWORK_THUMB_JPEG_QUALITY);
    match thumb.write_with_encoder(encoder) {
        Ok(()) => out,
        Err(_) => bytes.to_vec(),
    }
}

pub fn cached_artwork_thumbnail(
    cache: &SharedArtworkThumbs,
    track_id: i64,
    bytes: &[u8],
    max_px: u32,
) -> Arc<Vec<u8>> {
    let key = (track_id, max_px, bytes_fingerprint(bytes));
    if let Some(hit) = cache.lock().ok().and_then(|c| c.entries.get(&key).cloned()) {
        return hit;
    }
    let thumb = Arc::new(artwork_thumbnail(bytes, max_px));
    if let Ok(mut c) = cache.lock() {
        if c.entries.insert(key, thumb.clone()).is_none() {
            c.order.push_back(key);
            if c.order.len() > ARTWORK_THUMB_CACHE_LIMIT {
                if let Some(oldest) = c.order.pop_front() {
                    c.entries.remove(&oldest);
                }
            }
        }
    }
    thumb
}

pub fn stored_covers(lib: &LocalLibrary, track_ids: &[i64]) -> Result<Vec<Option<Vec<u8>>>, String> {
    track_ids.iter().map(|&id| lib.get_artwork(id).map_err(|e| e.to_string())).collect()
}

/// Thumbnails of `stored` (one entry per requested track, `None` for a track without a cover) as one buffer,
/// all numbers little-endian: a `u32` count, one `u32` byte length per entry (0 for no cover), then the
/// entries' bytes back to back in the same order. Each entry is what `cached_artwork_thumbnail` returns.
pub fn thumbnail_batch(
    cache: &SharedArtworkThumbs,
    track_ids: &[i64],
    stored: &[Option<Vec<u8>>],
    max_px: u32,
    threads: usize,
) -> Vec<u8> {
    let mut thumbs: Vec<Option<Arc<Vec<u8>>>> = vec![None; stored.len()];
    let work: Vec<usize> = (0..stored.len()).filter(|&i| stored[i].is_some()).collect();
    let threads = threads.min(work.len()).max(1);
    let per_thread = work.len().div_ceil(threads).max(1);
    let done: Vec<Vec<(usize, Arc<Vec<u8>>)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = work
            .chunks(per_thread)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&i| {
                            let bytes = stored[i].as_deref().unwrap_or_default();
                            let thumb = if max_px > 0 {
                                cached_artwork_thumbnail(cache, track_ids[i], bytes, max_px)
                            } else {
                                Arc::new(bytes.to_vec())
                            };
                            (i, thumb)
                        })
                        .collect()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("thumbnail thread")).collect()
    });
    for (i, thumb) in done.into_iter().flatten() {
        thumbs[i] = Some(thumb);
    }
    let total: usize = thumbs.iter().flatten().map(|t| t.len()).sum();
    let mut out = Vec::with_capacity(4 + 4 * thumbs.len() + total);
    out.extend_from_slice(&(thumbs.len() as u32).to_le_bytes());
    for thumb in &thumbs {
        out.extend_from_slice(&(thumb.as_ref().map_or(0, |t| t.len()) as u32).to_le_bytes());
    }
    for thumb in thumbs.iter().flatten() {
        out.extend_from_slice(thumb);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pioneer_usb_writer::models::Track;
    use std::path::PathBuf;

    fn image_bytes(width: u32, height: u32, seed: u32, format: image::ImageFormat) -> Vec<u8> {
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x.wrapping_mul(seed) % 251) as u8, (y.wrapping_add(seed) % 241) as u8, (x ^ y) as u8])
        });
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, format).expect("encode");
        out.into_inner()
    }

    fn track_with(n: usize, artwork: Option<Vec<u8>>) -> Track {
        Track {
            id: 0,
            source_path: PathBuf::from(format!("/music/t{n}.mp3")),
            usb_path: format!("/Contents/t{n}.mp3"),
            title: format!("T{n}"),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            label: String::new(),
            remixer: String::new(),
            comment: String::new(),
            year: 2020,
            disc_number: 1,
            track_number: 1,
            tempo: 12000,
            key: "1A".into(),
            duration_secs: 240.0,
            sample_rate: 44100,
            bitrate: 320,
            file_size: 1,
            artwork,
        }
    }

    fn entries(buffer: &[u8]) -> Vec<&[u8]> {
        let count = u32::from_le_bytes(buffer[..4].try_into().unwrap()) as usize;
        let mut at = 4 + 4 * count;
        (0..count)
            .map(|k| {
                let len = u32::from_le_bytes(buffer[4 + 4 * k..8 + 4 * k].try_into().unwrap()) as usize;
                at += len;
                &buffer[at - len..at]
            })
            .collect()
    }

    #[test]
    fn batch_entries_are_the_bytes_of_the_single_track_path() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let covers = [
            Some(image_bytes(700, 700, 3, image::ImageFormat::Jpeg)),
            Some(image_bytes(900, 500, 7, image::ImageFormat::Png)),
            None,
            Some(image_bytes(40, 40, 11, image::ImageFormat::Png)),
            Some(b"not an image".to_vec()),
            Some(image_bytes(1200, 1200, 5, image::ImageFormat::Jpeg)),
        ];
        let tracks: Vec<Track> = covers.iter().cloned().enumerate().map(|(n, c)| track_with(n, c)).collect();
        let mut ids = lib.add_tracks(&tracks).unwrap();
        ids.push(999_999);
        let stored = stored_covers(&lib, &ids).unwrap();
        for threads in [1, 3, 16] {
            let cache = Arc::new(Mutex::new(ArtworkThumbCache::default()));
            let buffer = thumbnail_batch(&cache, &ids, &stored, 64, threads);
            let got = entries(&buffer);
            assert_eq!(got.len(), ids.len());
            for (k, &id) in ids.iter().enumerate() {
                let expected = match lib.get_artwork(id).unwrap() {
                    Some(bytes) => artwork_thumbnail(&bytes, 64),
                    None => Vec::new(),
                };
                assert_eq!(got[k], expected.as_slice(), "entry {k}, {threads} threads");
            }
            assert!(got[2].is_empty() && got[6].is_empty());
            let again = thumbnail_batch(&cache, &ids, &stored, 64, threads);
            assert_eq!(again, buffer, "cached entries are the same bytes");
        }
    }

    #[test]
    fn batch_without_a_box_returns_the_stored_bytes() {
        let cache = Arc::new(Mutex::new(ArtworkThumbCache::default()));
        let stored = vec![Some(vec![1u8, 2, 3]), None, Some(vec![9u8])];
        let buffer = thumbnail_batch(&cache, &[1, 2, 3], &stored, 0, 2);
        assert_eq!(entries(&buffer), vec![&[1u8, 2, 3][..], &[][..], &[9u8][..]]);
    }

    #[test]
    fn empty_batch_is_a_zero_count() {
        let cache = Arc::new(Mutex::new(ArtworkThumbCache::default()));
        assert_eq!(thumbnail_batch(&cache, &[], &[], 64, 4), vec![0, 0, 0, 0]);
    }
}
