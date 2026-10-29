use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use pioneer_usb_writer::waveform::{DetailSeries, WindowSeries};

use crate::band_analysis::{measure_detail, measure_detail_window};

const CACHE_LIMIT_BYTES: u64 = 512 * 1024 * 1024;
const WINDOW_MAX_COLUMNS: usize = 16_384;
const WINDOW_MAX_SECONDS: f64 = 30.0;

fn cache_file(dir: &Path, audio: &Path) -> Result<PathBuf, String> {
    let meta = fs::metadata(audio).map_err(|e| format!("stat {}: {e}", audio.display()))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut hasher = DefaultHasher::new();
    (audio, meta.len(), modified).hash(&mut hasher);
    Ok(dir.join(format!("{:016x}.ffwd", hasher.finish())))
}

fn trim_cache(dir: &Path, limit_bytes: u64) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    files.sort();
    for (_, len, path) in &files {
        if total <= limit_bytes {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total -= len;
        }
    }
}

fn path_lock(file: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    locks.entry(file.to_path_buf()).or_default().clone()
}

fn cached_bytes(file: &Path) -> Option<Vec<u8>> {
    let bytes = fs::read(file).ok()?;
    DetailSeries::from_bytes(&bytes).ok()?;
    if let Ok(handle) = fs::File::options().write(true).open(file) {
        handle.set_modified(SystemTime::now()).ok();
    }
    Some(bytes)
}

/// The high-resolution series of `audio` as `DetailSeries::to_bytes`: read from the cache in `dir`, or measured now
/// (one decode pass) and written there. One file is measured once at a time: a second caller waits for the first and
/// reads its result. Nothing about the track is stored in the library, so every existing track gets it the first time
/// its zoomed waveform opens or it is prefetched; the cache keeps the most recently used files up to 512 MiB.
pub fn detail_bytes(dir: &Path, audio: &Path) -> Result<Vec<u8>, String> {
    let file = cache_file(dir, audio)?;
    let lock = path_lock(&file);
    let _measuring = lock.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(bytes) = cached_bytes(&file) {
        return Ok(bytes);
    }
    let bytes = measure_detail(audio)?.to_bytes();
    write_cache_file(dir, &file, &bytes)?;
    Ok(bytes)
}

fn write_cache_file(dir: &Path, file: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let partial = file.with_extension("part");
    fs::write(&partial, bytes).map_err(|e| format!("write {}: {e}", partial.display()))?;
    fs::rename(&partial, file).map_err(|e| format!("rename {}: {e}", partial.display()))?;
    trim_cache(dir, CACHE_LIMIT_BYTES);
    Ok(())
}

/// Puts a series measured elsewhere into the cache as the one of `audio`, for a caller that decodes the track anyway
/// (`band_analysis::measure_waveforms_and_detail`), so its zoomed waveform opens without a decode of its own.
pub fn store_detail(dir: &Path, audio: &Path, series: &DetailSeries) -> Result<(), String> {
    let file = cache_file(dir, audio)?;
    let lock = path_lock(&file);
    let _writing = lock.lock().unwrap_or_else(|e| e.into_inner());
    write_cache_file(dir, &file, &series.to_bytes())
}

pub fn detail_series(dir: &Path, audio: &Path) -> Result<DetailSeries, String> {
    DetailSeries::from_bytes(&detail_bytes(dir, audio)?)
}

/// Measure the series of `audio` into the cache unless it is there already.
pub fn warm_detail(dir: &Path, audio: &Path) -> Result<(), String> {
    if cache_file(dir, audio)?.exists() {
        return Ok(());
    }
    detail_bytes(dir, audio).map(drop)
}

/// Fine windows of the zoomed waveform. Only the newest request matters: a request supersedes the one before it, whose
/// decode stops between blocks. The gains of the last track are kept so a window does not re-read its detail series.
#[derive(Default)]
pub struct DetailWindows {
    generation: AtomicU64,
    gains: Mutex<Option<(PathBuf, [u16; 3])>>,
}

impl DetailWindows {
    /// `Ok(None)` when a newer request superseded this one before it finished.
    pub fn window(
        &self,
        cache_dir: &Path,
        audio: &Path,
        start_s: f64,
        end_s: f64,
        columns: usize,
    ) -> Result<Option<WindowSeries>, String> {
        if !(start_s.is_finite() && end_s.is_finite() && end_s > start_s && start_s >= 0.0) {
            return Err(format!("bad window {start_s}..{end_s} s"));
        }
        if end_s - start_s > WINDOW_MAX_SECONDS || columns == 0 || columns > WINDOW_MAX_COLUMNS {
            return Err(format!("window of {:.3} s in {columns} columns is out of range", end_s - start_s));
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let gains = self.gains_of(cache_dir, audio)?;
        let is_current = || self.generation.load(Ordering::SeqCst) == generation;
        measure_detail_window(audio, gains, start_s, end_s, columns, &is_current)
    }

    fn gains_of(&self, cache_dir: &Path, audio: &Path) -> Result<[u16; 3], String> {
        if let Some((path, gains)) = self.gains.lock().map_err(|e| e.to_string())?.as_ref() {
            if path == audio {
                return Ok(*gains);
            }
        }
        let gains = detail_series(cache_dir, audio)?.gains;
        *self.gains.lock().map_err(|e| e.to_string())? = Some((audio.to_path_buf(), gains));
        Ok(gains)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &Path) {
        let rate = 44_100u32;
        let mut data = Vec::new();
        for i in 0..rate as usize {
            let v = ((i as f64 * 0.05).sin() * 12000.0) as i16;
            data.extend_from_slice(&v.to_le_bytes());
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        fs::write(path, wav).unwrap();
    }

    #[test]
    fn a_second_call_reads_the_cache_file() {
        let dir = std::env::temp_dir().join(format!("ff-detail-cache-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("tone.wav");
        write_wav(&audio);
        let cache = dir.join("cache");

        let first = detail_series(&cache, &audio).unwrap();
        let cached = fs::read_dir(&cache).unwrap().count();
        let second = detail_series(&cache, &audio).unwrap();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(cached, 1);
        assert_eq!(first.len(), 2400);
        assert_eq!(first, second);
    }

    #[test]
    fn callers_of_one_file_share_a_single_measurement_and_a_warmed_file_is_read_back() {
        let dir = std::env::temp_dir().join(format!("ff-detail-shared-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("tone.wav");
        write_wav(&audio);
        let cache = dir.join("cache");

        let results: Vec<_> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..3).map(|_| s.spawn(|| warm_detail(&cache, &audio))).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let bytes = detail_bytes(&cache, &audio).unwrap();
        let files = fs::read_dir(&cache).unwrap().count();
        fs::remove_dir_all(&dir).ok();

        assert!(results.iter().all(Result::is_ok));
        assert_eq!(files, 1);
        assert_eq!(DetailSeries::from_bytes(&bytes).unwrap().len(), 2400);
    }

    #[test]
    fn a_stored_series_is_what_the_cache_hands_out_without_measuring() {
        let dir = std::env::temp_dir().join(format!("ff-detail-store-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("tone.wav");
        write_wav(&audio);
        let cache = dir.join("cache");
        let (_, measured) = crate::band_analysis::measure_waveforms_and_detail(&audio).unwrap();
        let marked = DetailSeries { gains: [1, 2, 3], ..measured.clone() };

        store_detail(&cache, &audio, &marked).unwrap();
        let read = detail_series(&cache, &audio).unwrap();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(measured.len(), 2400);
        assert_eq!(read, marked);
    }

    #[test]
    fn the_cache_drops_the_least_recently_used_files_over_its_size() {
        let dir = std::env::temp_dir().join(format!("ff-detail-trim-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let base = SystemTime::now();
        for (i, name) in ["old", "mid", "new"].iter().enumerate() {
            let path = dir.join(format!("{name}.ffwd"));
            fs::write(&path, vec![0u8; 100]).unwrap();
            let file = fs::File::options().write(true).open(&path).unwrap();
            file.set_modified(base - std::time::Duration::from_secs(100 - i as u64 * 10)).unwrap();
        }

        trim_cache(&dir, 200);
        let mut left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        left.sort();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(left, ["mid.ffwd", "new.ffwd"]);
    }

    #[test]
    fn a_window_uses_the_gains_of_the_cached_series_and_rejects_bad_ranges() {
        let dir = std::env::temp_dir().join(format!("ff-detail-window-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("tone.wav");
        write_wav(&audio);
        let cache = dir.join("cache");
        let windows = DetailWindows::default();

        let window = windows.window(&cache, &audio, 0.5, 0.6, 400).unwrap().unwrap();
        let series = detail_series(&cache, &audio).unwrap();
        let bad = [
            windows.window(&cache, &audio, 0.6, 0.5, 400),
            windows.window(&cache, &audio, 0.0, 60.0, 400),
            windows.window(&cache, &audio, 0.0, 0.1, 0),
        ];
        fs::remove_dir_all(&dir).ok();

        assert_eq!(window.len(), 400);
        let low = *window.planes[0].iter().max().unwrap() as i32;
        let detail_low = *series.planes[0][1200..1440].iter().max().unwrap() as i32;
        assert!((low - detail_low).abs() <= 4, "window {low} vs detail {detail_low}");
        assert!(bad.iter().all(|r| r.is_err()));
    }
}
