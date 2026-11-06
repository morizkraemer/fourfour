#[allow(dead_code)]
#[path = "../artwork_thumbs.rs"]
mod artwork_thumbs;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use pioneer_library::LocalLibrary;
use pioneer_usb_writer::models::Track;

use artwork_thumbs::{ArtworkThumbCache, SharedArtworkThumbs, cached_artwork_thumbnail, stored_covers, thumbnail_batch};

const ROW_PX: u32 = 64;
const COVER_PX: u32 = 600;

fn cover_jpeg(i: usize) -> Vec<u8> {
    let seed = (i as u32).wrapping_mul(2_654_435_761);
    let img = image::RgbImage::from_fn(COVER_PX, COVER_PX, |x, y| {
        let h = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663) ^ seed).wrapping_mul(2_246_822_519) >> 27;
        let base = ((x + y) / 5 + i as u32) as u8;
        image::Rgb([base.wrapping_add(h as u8), (y / 3) as u8 ^ (seed >> 24) as u8, (x / 3) as u8 ^ h as u8])
    });
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
    img.write_with_encoder(encoder).unwrap();
    out
}

fn track(i: usize) -> Track {
    Track {
        id: 0,
        source_path: PathBuf::from(format!("/music/bench/track{i:06}.mp3")),
        usb_path: format!("/Contents/track{i:06}.mp3"),
        title: format!("Cover Bench Track {i}"),
        artist: format!("Artist {}", i % 400),
        album: format!("Album {}", i % 1500),
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
        file_size: 9_600_000,
        artwork: Some(cover_jpeg(i)),
    }
}

fn generate(dir: &Path, count: usize) {
    std::fs::create_dir_all(dir).unwrap();
    let lib = LocalLibrary::open(&dir.join("library.db")).unwrap();
    let next = AtomicUsize::new(0);
    let tracks: Mutex<Vec<(usize, Track)>> = Mutex::default();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= count {
                        break;
                    }
                    let t = track(i);
                    tracks.lock().unwrap().push((i, t));
                }
            });
        }
    });
    let mut tracks = tracks.into_inner().unwrap();
    tracks.sort_by_key(|(i, _)| *i);
    let tracks: Vec<Track> = tracks.into_iter().map(|(_, t)| t).collect();
    let bytes: usize = tracks.iter().map(|t| t.artwork.as_ref().map_or(0, Vec::len)).sum();
    lib.add_tracks(&tracks).unwrap();
    println!("{count} covers, {} KB each on average", bytes / count / 1024);
}

fn ids_of(lib: &LocalLibrary, count: usize) -> Vec<i64> {
    let mut ids: Vec<i64> = (1..=count as i64).collect();
    ids.retain(|&id| lib.get_artwork(id).unwrap().is_some());
    ids
}

fn fresh_cache() -> SharedArtworkThumbs {
    Arc::new(Mutex::new(ArtworkThumbCache::default()))
}

fn single(lib: &Mutex<LocalLibrary>, cache: &SharedArtworkThumbs, id: i64) -> Vec<u8> {
    let stored = lib.lock().unwrap().get_artwork(id).unwrap();
    match stored {
        Some(bytes) => cached_artwork_thumbnail(cache, id, &bytes, ROW_PX).as_ref().clone(),
        None => Vec::new(),
    }
}

fn single_parallel(lib: &Mutex<LocalLibrary>, cache: &SharedArtworkThumbs, ids: &[i64], parallel: usize) -> f64 {
    let next = AtomicUsize::new(0);
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..parallel {
            scope.spawn(|| {
                loop {
                    let n = next.fetch_add(1, Ordering::Relaxed);
                    if n >= ids.len() {
                        break;
                    }
                    single(lib, cache, ids[n]);
                }
            });
        }
    });
    ids.len() as f64 / started.elapsed().as_secs_f64()
}

fn batched(lib: &Mutex<LocalLibrary>, cache: &SharedArtworkThumbs, ids: &[i64], batch: usize, threads: usize) -> f64 {
    let started = Instant::now();
    for chunk in ids.chunks(batch) {
        let stored = stored_covers(&lib.lock().unwrap(), chunk).unwrap();
        thumbnail_batch(cache, chunk, &stored, ROW_PX, threads);
    }
    ids.len() as f64 / started.elapsed().as_secs_f64()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn run(dir: &Path, count: usize, rounds: usize) {
    let lib = Mutex::new(LocalLibrary::open(&dir.join("library.db")).unwrap());
    let ids = ids_of(&lib.lock().unwrap(), count);
    let ids = &ids[..ids.len().min(count)];
    println!("{} covers, {} rounds, medians, covers/s", ids.len(), rounds);
    let mut cases: Vec<(String, Box<dyn Fn(&SharedArtworkThumbs) -> f64 + '_>)> = Vec::new();
    for parallel in [1usize, 6] {
        let lib = &lib;
        cases.push((format!("single, {parallel} in flight"), Box::new(move |c| single_parallel(lib, c, ids, parallel))));
    }
    for batch in [8usize, 16, 32, 64] {
        for threads in [1usize, 2, 4, 8] {
            let lib = &lib;
            cases.push((format!("batch {batch}, {threads} threads"), Box::new(move |c| batched(lib, c, ids, batch, threads))));
        }
    }
    for (label, case) in cases {
        let mut colds = Vec::new();
        let mut warms = Vec::new();
        for _ in 0..rounds {
            let cache = fresh_cache();
            colds.push(case(&cache));
            warms.push(case(&cache));
        }
        println!("{label:<24} cold {:>8.0}   warm {:>9.0}", median(colds), median(warms));
    }
}

fn identity(dir: &Path, out: &Path, count: usize) {
    let lib = Mutex::new(LocalLibrary::open(&dir.join("library.db")).unwrap());
    let ids = ids_of(&lib.lock().unwrap(), count);
    let ids = &ids[..ids.len().min(count)];
    std::fs::create_dir_all(out.join("single")).unwrap();
    std::fs::create_dir_all(out.join("batch")).unwrap();
    let cache = fresh_cache();
    for &id in ids {
        std::fs::write(out.join("single").join(id.to_string()), single(&lib, &cache, id)).unwrap();
    }
    let cache = fresh_cache();
    for chunk in ids.chunks(32) {
        let stored = stored_covers(&lib.lock().unwrap(), chunk).unwrap();
        let buf = thumbnail_batch(&cache, chunk, &stored, ROW_PX, 4);
        let n = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
        assert_eq!(n, chunk.len());
        let mut at = 4 + 4 * n;
        for (k, &id) in chunk.iter().enumerate() {
            let len = u32::from_le_bytes(buf[4 + 4 * k..8 + 4 * k].try_into().unwrap()) as usize;
            std::fs::write(out.join("batch").join(id.to_string()), &buf[at..at + len]).unwrap();
            at += len;
        }
        assert_eq!(at, buf.len());
    }
    println!("wrote {} covers each way", ids.len());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("gen") => generate(Path::new(&args[2]), args[3].parse().unwrap()),
        Some("run") => run(Path::new(&args[2]), args[3].parse().unwrap(), args.get(4).map_or(3, |n| n.parse().unwrap())),
        Some("identity") => identity(Path::new(&args[2]), Path::new(&args[3]), args[4].parse().unwrap()),
        _ => eprintln!("usage: bench_covers gen <dir> <count> | run <dir> <count> [rounds] | identity <dir> <outdir> <count>"),
    }
}
