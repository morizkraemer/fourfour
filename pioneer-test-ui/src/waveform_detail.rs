use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pioneer_usb_writer::waveform::{DetailSeries, WindowSeries};

use crate::band_analysis::{measure_detail, measure_detail_window};

const CACHE_LIMIT_CEILING_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const CACHE_LIMIT_FLOOR_BYTES: u64 = 512 * 1024 * 1024;
const CACHE_SHARE_OF_FREE_DISK: u64 = 20;
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

fn cache_limit_for_free_space(free_bytes: u64) -> u64 {
    (free_bytes / CACHE_SHARE_OF_FREE_DISK).clamp(CACHE_LIMIT_FLOOR_BYTES, CACHE_LIMIT_CEILING_BYTES)
}

#[cfg(unix)]
fn free_space_of(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statvfs(path.as_ptr(), &mut stats) } == 0).then(|| stats.f_bavail as u64 * stats.f_frsize as u64)
}

#[cfg(windows)]
fn free_space_of(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(directory: *const u16, free_to_caller: *mut u64, total: *mut u64, total_free: *mut u64) -> i32;
    }

    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free_to_caller = 0u64;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free_to_caller, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(free_to_caller)
}

fn cache_limit(dir: &Path) -> u64 {
    static LIMIT: OnceLock<u64> = OnceLock::new();
    *LIMIT.get_or_init(|| free_space_of(dir).map_or(CACHE_LIMIT_FLOOR_BYTES, cache_limit_for_free_space))
}

fn trim_cache(dir: &Path, limit_bytes: u64) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut files: Vec<_> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ffwd"))
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

pub const STALE_PART_AGE: Duration = Duration::from_secs(600);

fn part_file_pid(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".part")?;
    let writer = stem.rsplit('.').next()?;
    writer.split('-').next()?.parse().ok()
}

/// Removes `.part` files a killed process left behind: only files at least `min_age` old that do not carry this
/// process's pid, so a write in progress here is never touched. Returns how many were removed.
pub fn sweep_stale_parts(dir: &Path, min_age: Duration) -> usize {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    let own_pid = std::process::id();
    entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.ends_with(".part") && part_file_pid(&name) != Some(own_pid)
        })
        .filter(|entry| {
            let age = entry.metadata().and_then(|meta| meta.modified()).ok().and_then(|at| at.elapsed().ok());
            age.is_some_and(|age| age >= min_age)
        })
        .filter(|entry| fs::remove_file(entry.path()).is_ok())
        .count()
}

fn path_lock(file: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    locks.entry(file.to_path_buf()).or_default().clone()
}

const DETAIL_MAGIC: &[u8; 4] = b"FFWD";
const DETAIL_FORMAT_VERSION: u8 = 3;
const DETAIL_HEADER_LEN: usize = 28;
const DETAIL_BYTES_PER_COLUMN: usize = 5;

fn is_detail_file(bytes: &[u8]) -> bool {
    if bytes.len() < DETAIL_HEADER_LEN || &bytes[..4] != DETAIL_MAGIC || bytes[4] != DETAIL_FORMAT_VERSION {
        return false;
    }
    let columns = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes")) as usize;
    bytes.len() == DETAIL_HEADER_LEN + DETAIL_BYTES_PER_COLUMN * columns
}

fn cached_bytes(file: &Path) -> Option<Vec<u8>> {
    let bytes = fs::read(file).ok()?;
    if !is_detail_file(&bytes) {
        return None;
    }
    if let Ok(handle) = fs::File::options().write(true).open(file) {
        handle.set_modified(SystemTime::now()).ok();
    }
    Some(bytes)
}

/// The high-resolution series of `audio` as `DetailSeries::to_bytes`: read from the cache in `dir`, or measured now
/// (one decode pass) and written there. One file is measured once at a time: a second caller waits for the first and
/// reads its result. A background `warm_detail` of the same file does not hold it up: both measure, both write the
/// same bytes. The analysis stores the series of every track it analyses; the cache keeps the most recently used files
/// up to 2 GiB (about 450 tracks of 6 minutes).
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
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let partial = file.with_extension(format!("{}-{}.part", std::process::id(), WRITES.fetch_add(1, Ordering::Relaxed)));
    fs::write(&partial, bytes).map_err(|e| format!("write {}: {e}", partial.display()))?;
    fs::rename(&partial, file).map_err(|e| format!("rename {}: {e}", partial.display()))?;
    trim_cache(dir, cache_limit(dir));
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

/// Measure the series of `audio` into the cache unless it is there already, for work below the UI's priority: it waits
/// only for other warms of the file, never makes `detail_bytes` wait, and leaves a file `detail_bytes` wrote meanwhile.
pub fn warm_detail(dir: &Path, audio: &Path) -> Result<(), String> {
    let file = cache_file(dir, audio)?;
    if file.exists() {
        return Ok(());
    }
    let lock = path_lock(&file.with_extension("warming"));
    let _warming = lock.lock().unwrap_or_else(|e| e.into_inner());
    if file.exists() {
        return Ok(());
    }
    let bytes = measure_detail(audio)?.to_bytes();
    if file.exists() {
        return Ok(());
    }
    write_cache_file(dir, &file, &bytes)
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

    #[test]
    fn free_space_of_an_existing_folder_is_known_and_a_missing_one_is_not() {
        let dir = std::env::temp_dir();
        assert!(free_space_of(&dir).is_some_and(|free| free > 0));
        assert_eq!(free_space_of(&dir.join("ff-no-such-folder-xplat")), None);
    }

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
    fn a_reader_does_not_wait_for_a_warm_of_the_same_file() {
        let dir = std::env::temp_dir().join(format!("ff-detail-warm-reader-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("tone.wav");
        write_wav(&audio);
        let cache = dir.join("cache");
        let file = cache_file(&cache, &audio).unwrap();
        let warm_lock = path_lock(&file.with_extension("warming"));
        let held = warm_lock.lock().unwrap();

        let bytes = std::thread::scope(|s| s.spawn(|| detail_bytes(&cache, &audio)).join().unwrap()).unwrap();
        drop(held);
        warm_detail(&cache, &audio).unwrap();
        let files: Vec<_> = fs::read_dir(&cache).unwrap().flatten().map(|e| e.file_name()).collect();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(DetailSeries::from_bytes(&bytes).unwrap().len(), 2400);
        assert_eq!(files.len(), 1, "{files:?}");
    }

    #[test]
    fn the_cache_trim_leaves_files_being_written() {
        let dir = std::env::temp_dir().join(format!("ff-detail-trim-part-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.ffwd"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("b.1-2.part"), vec![0u8; 100]).unwrap();

        trim_cache(&dir, 0);
        let left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(left, ["b.1-2.part"]);
    }

    #[test]
    fn the_startup_sweep_removes_old_part_files_of_other_processes_and_keeps_the_rest() {
        let dir = std::env::temp_dir().join(format!("ff-detail-sweep-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        let mine = std::process::id();
        let other = mine + 1;
        let long_ago = SystemTime::now() - Duration::from_secs(2 * 3600);
        let files = [
            (format!("a.{other}-1.part"), long_ago),
            ("junk.part".to_string(), long_ago),
            (format!("b.{other}-2.part"), SystemTime::now()),
            (format!("c.{mine}-3.part"), long_ago),
            ("d.ffwd".to_string(), long_ago),
        ];
        for (name, modified) in &files {
            let path = dir.join(name);
            fs::write(&path, b"x").unwrap();
            fs::File::options().write(true).open(&path).unwrap().set_modified(*modified).unwrap();
        }

        let removed = sweep_stale_parts(&dir, STALE_PART_AGE);
        let mut left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
        left.sort();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(removed, 2);
        assert_eq!(left, [format!("b.{other}-2.part"), format!("c.{mine}-3.part"), "d.ffwd".to_string()]);
    }

    #[test]
    fn the_startup_sweep_of_a_missing_dir_does_nothing() {
        let dir = std::env::temp_dir().join(format!("ff-detail-sweep-missing-{}", std::process::id()));
        assert_eq!(sweep_stale_parts(&dir, STALE_PART_AGE), 0);
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
    fn the_cache_limit_is_five_percent_of_free_space_between_the_floor_and_two_gib() {
        const GIB: u64 = 1024 * 1024 * 1024;
        assert_eq!(cache_limit_for_free_space(0), 512 * 1024 * 1024);
        assert_eq!(cache_limit_for_free_space(8 * GIB), 512 * 1024 * 1024);
        assert_eq!(cache_limit_for_free_space(32 * GIB), 32 * GIB / 20);
        assert_eq!(cache_limit_for_free_space(1000 * GIB), 2 * GIB);
    }

    #[test]
    fn trimming_a_dir_of_400_files_is_cheap() {
        let dir = std::env::temp_dir().join(format!("ff-detail-cost-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        for i in 0..400 {
            fs::write(dir.join(format!("{i:04}.ffwd")), vec![0u8; 16]).unwrap();
        }
        let started = std::time::Instant::now();
        for _ in 0..20 {
            trim_cache(&dir, u64::MAX);
        }
        let per_call = started.elapsed() / 20;
        fs::remove_dir_all(&dir).ok();
        eprintln!("TRIM_COST_400_FILES {per_call:?}");
        assert!(per_call < Duration::from_millis(50), "{per_call:?}");
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

    #[test]
    fn cache_check_accepts_what_the_series_reader_accepts() {
        let dir = std::env::temp_dir().join(format!("ff-detail-check-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("tone.wav");
        write_wav(&audio);
        let bytes = measure_detail(&audio).unwrap().to_bytes();
        fs::remove_dir_all(&dir).ok();
        assert!(is_detail_file(&bytes) && DetailSeries::from_bytes(&bytes).is_ok());
        for broken in [bytes[..bytes.len() - 1].to_vec(), bytes[..20].to_vec(), Vec::new(), [bytes.clone(), vec![0]].concat()] {
            assert!(!is_detail_file(&broken) && DetailSeries::from_bytes(&broken).is_err());
        }
        let mut other_version = bytes.clone();
        other_version[4] += 1;
        let mut other_magic = bytes.clone();
        other_magic[0] = b'X';
        for broken in [other_version, other_magic] {
            assert!(!is_detail_file(&broken) && DetailSeries::from_bytes(&broken).is_err());
        }
    }
}
