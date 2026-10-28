use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use pioneer_usb_writer::waveform::DetailSeries;

use crate::band_analysis::measure_detail;

const CACHE_LIMIT_FILES: usize = 256;

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

fn trim_cache(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= CACHE_LIMIT_FILES {
        return;
    }
    files.sort();
    for (_, path) in &files[..files.len() - CACHE_LIMIT_FILES] {
        fs::remove_file(path).ok();
    }
}

/// The high-resolution series of `audio`: read from the cache in `dir`, or measured now (one decode pass)
/// and written there. Nothing about the track is stored in the library, so every existing track gets it
/// the first time its zoomed waveform opens.
pub fn detail_series(dir: &Path, audio: &Path) -> Result<DetailSeries, String> {
    let file = cache_file(dir, audio)?;
    if let Ok(bytes) = fs::read(&file) {
        if let Ok(series) = DetailSeries::from_bytes(&bytes) {
            return Ok(series);
        }
    }
    let series = measure_detail(audio)?;
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let partial = file.with_extension("part");
    fs::write(&partial, series.to_bytes()).map_err(|e| format!("write {}: {e}", partial.display()))?;
    fs::rename(&partial, &file).map_err(|e| format!("rename {}: {e}", partial.display()))?;
    trim_cache(dir);
    Ok(series)
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
}
