use anyhow::{Context, Result};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use walkdir::WalkDir;

use crate::models::Track;

const MAX_FLAC_METADATA_BLOCKS: usize = 4096;
const MAX_MP4_ATOMS: usize = 100_000;
const MAX_MP4_DEPTH: usize = 12;
const MP4_CONTAINERS: [&[u8; 4]; 7] = [b"moov", b"trak", b"mdia", b"minf", b"udta", b"meta", b"ilst"];

fn check_flac_blocks(file: &mut BufReader<File>) -> Result<()> {
    let mut head = [0u8; 10];
    if file.read_exact(&mut head).is_err() {
        return Ok(());
    }
    let mut start = 0u64;
    if &head[..3] == b"ID3" {
        start = 10 + ((head[6] as u64) << 21 | (head[7] as u64) << 14 | (head[8] as u64) << 7 | head[9] as u64);
    }
    let mut magic = [0u8; 4];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut magic).is_err() || &magic != b"fLaC" {
        return Ok(());
    }
    for _ in 0..MAX_FLAC_METADATA_BLOCKS {
        let mut header = [0u8; 4];
        if file.read_exact(&mut header).is_err() || header[0] & 0x80 != 0 {
            return Ok(());
        }
        let len = (header[1] as i64) << 16 | (header[2] as i64) << 8 | header[3] as i64;
        file.seek(SeekFrom::Current(len))?;
    }
    anyhow::bail!("FLAC has more than {MAX_FLAC_METADATA_BLOCKS} metadata blocks")
}

fn walk_mp4_atoms(file: &mut BufReader<File>, mut at: u64, end: u64, depth: usize, count: &mut usize) -> Result<()> {
    while at + 8 <= end {
        *count += 1;
        if *count > MAX_MP4_ATOMS {
            anyhow::bail!("MP4 has more than {MAX_MP4_ATOMS} atoms");
        }
        file.seek(SeekFrom::Start(at))?;
        let mut header = [0u8; 8];
        file.read_exact(&mut header)?;
        let size = u32::from_be_bytes(header[..4].try_into().unwrap()) as u64;
        let kind: [u8; 4] = header[4..].try_into().unwrap();
        let (header_len, size) = match size {
            0 => (8, end - at),
            1 => {
                let mut large = [0u8; 8];
                file.read_exact(&mut large)?;
                (16, u64::from_be_bytes(large))
            }
            n => (8, n),
        };
        if size < header_len {
            return Ok(());
        }
        let atom_end = at.saturating_add(size).min(end);
        if depth < MAX_MP4_DEPTH && MP4_CONTAINERS.contains(&&kind) {
            let body = at + header_len + if &kind == b"meta" { 4 } else { 0 };
            walk_mp4_atoms(file, body, atom_end, depth + 1, count)?;
        }
        at = atom_end;
    }
    Ok(())
}

fn check_tag_structure(path: &Path) -> Result<()> {
    let mut file = BufReader::new(File::open(path)?);
    let mut magic = [0u8; 8];
    let got = file.read(&mut magic)?;
    file.seek(SeekFrom::Start(0))?;
    if got < 8 {
        return Ok(());
    }
    if &magic[4..8] == b"ftyp" {
        let end = std::fs::metadata(path)?.len();
        walk_mp4_atoms(&mut file, 0, end, 0, &mut 0)
    } else if &magic[..3] == b"ID3" || &magic[..4] == b"fLaC" {
        check_flac_blocks(&mut file)
    } else {
        Ok(())
    }
}

pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "aiff", "aif", "m4a", "aac"];

/// Recursively scan a directory for audio files and return their metadata as a list of [`Track`]s.
///
/// Walks `dir` (following symlinks), skips any file whose extension is not in the
/// supported set (`mp3`, `flac`, `wav`, `aiff`, `aif`, `m4a`, `aac`), and reads
/// tag metadata from each matching file via [`read_track_metadata`].
///
/// Files that cannot be opened or parsed emit a warning to stderr and are skipped.
///
/// The returned list is **sorted** by title → artist → source path, and track IDs
/// are reassigned in that sorted order starting at 1.
pub fn scan_directory(dir: &Path) -> Result<Vec<Track>> {
    scan_directory_with_progress(dir, &mut || {})
}

/// Like [`scan_directory`], calling `on_file` after each audio file has been read or skipped.
pub fn scan_directory_with_progress(dir: &Path, on_file: &mut dyn FnMut()) -> Result<Vec<Track>> {
    let mut tracks = Vec::new();
    let mut id = 1u32;

    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        match read_track_metadata(path, id) {
            Ok(track) => {
                tracks.push(track);
                id += 1;
            }
            Err(e) => {
                eprintln!("Warning: skipping {}: {}", path.display(), e);
            }
        }
        on_file();
    }

    tracks.sort_by(|a, b| {
        a.title.cmp(&b.title)
            .then_with(|| a.artist.cmp(&b.artist))
            .then_with(|| a.source_path.cmp(&b.source_path))
    });

    // Reassign IDs after sorting
    for (i, track) in tracks.iter_mut().enumerate() {
        track.id = (i + 1) as u32;
    }

    Ok(tracks)
}

/// Read metadata from a specific list of file paths and return them as [`Track`]s.
///
/// Unlike [`scan_directory`], this function does not recurse — it processes only
/// the exact paths provided. Non-audio files (unrecognised extension) are silently
/// skipped; files that fail to parse emit a warning and are skipped.
///
/// Tracks are returned **in the order the paths were supplied** (no sorting).
/// IDs are assigned sequentially starting at 1.
pub fn scan_files(paths: &[std::path::PathBuf]) -> Result<Vec<Track>> {
    scan_files_with_progress(paths, &mut || {})
}

/// Like [`scan_files`], calling `on_file` after each audio file has been read or skipped.
pub fn scan_files_with_progress(paths: &[std::path::PathBuf], on_file: &mut dyn FnMut()) -> Result<Vec<Track>> {
    let audio_extensions = ["mp3", "flac", "wav", "aiff", "aif", "m4a", "aac"];
    let mut tracks = Vec::new();
    let mut id = 1u32;

    for path in paths {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if audio_extensions.contains(&ext.to_lowercase().as_str()) {
                match read_track_metadata(path, id) {
                    Ok(track) => {
                        tracks.push(track);
                        id += 1;
                    }
                    Err(e) => eprintln!("Warning: failed to read {}: {}", path.display(), e),
                }
                on_file();
            }
        }
    }

    Ok(tracks)
}

/// Read tag metadata from a single audio file and return a [`Track`].
///
/// Extracts title, artist, album, genre, label, remixer, comment, year, disc number,
/// track number, sample rate, bitrate, file size, and cover art bytes via `lofty`.
///
/// **Defaults for missing tags:**
/// - `title` — filename stem (without extension)
/// - `artist` — `"Unknown Artist"`
/// - `album` — `"Unknown Album"`
/// - `genre` — `"Unknown"`
/// - `label`, `remixer`, `comment` — empty string
/// - `year`, `disc_number`, `track_number` — `0`
///
/// The `tempo` and `key` fields are left empty/zero; they must be filled in by
/// the analyzer before writing to USB.
///
/// The USB-relative audio path is derived as `/Contents/<artist>/<filename>`,
/// with path-unsafe characters in the artist and filename replaced by `_`.
pub fn read_track_metadata(path: &Path, id: u32) -> Result<Track> {
    check_tag_structure(path).with_context(|| format!("Refusing to read tags from {}", path.display()))?;
    let tagged_file = Probe::open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?
        .read()
        .with_context(|| format!("Failed to read tags from {}", path.display()))?;

    let properties = tagged_file.properties();
    let duration = properties.duration();
    let sample_rate = properties.sample_rate().unwrap_or(44100);
    let bitrate = properties.overall_bitrate().unwrap_or(320);

    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let title = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    let artist = tag
        .and_then(|t| t.artist().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = tag
        .and_then(|t| t.album().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown Album".to_string());

    let genre = tag
        .and_then(|t| t.genre().map(|s| s.to_string()))
        .unwrap_or_else(|| "Unknown".to_string());

    let label = tag
        .and_then(|t| t.get_string(ItemKey::Publisher).map(|s| s.to_string()))
        .unwrap_or_default();

    let remixer = tag
        .and_then(|t| t.get_string(ItemKey::Remixer).map(|s| s.to_string()))
        .unwrap_or_default();

    let comment = tag
        .and_then(|t| t.get_string(ItemKey::Comment).map(|s| s.to_string()))
        .unwrap_or_default();

    let year = tag
        .and_then(|t| t.get_string(ItemKey::Year).and_then(|s| s.parse::<u16>().ok()))
        .unwrap_or(0);

    let disc_number = tag
        .and_then(|t| t.disk())
        .unwrap_or(0) as u16;

    let track_number = tag
        .and_then(|t| t.track())
        .unwrap_or(0);

    let file_size = std::fs::metadata(path)?.len();

    // Extract cover art
    let artwork = tag
        .and_then(|t| {
            t.pictures()
                .iter()
                .find(|p| p.pic_type() == lofty::picture::PictureType::CoverFront)
                .or_else(|| t.pictures().first())
                .map(|p| p.data().to_vec())
        });

    // Build USB path: Contents/<Artist>/<filename>
    let raw_filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown.mp3");
    let filename = sanitize_path_component(raw_filename);
    let safe_artist = sanitize_path_component(&artist);
    let usb_path = format!("/Contents/{}/{}", safe_artist, filename);

    Ok(Track {
        source_path: path.to_path_buf(),
        usb_path,
        title,
        artist,
        album,
        genre,
        label,
        remixer,
        comment,
        year,
        disc_number,
        track_number,
        tempo: 0, // filled in by analyzer
        key: String::new(),
        duration_secs: duration.as_secs_f64(),
        sample_rate,
        bitrate,
        file_size,
        id,
        artwork,
    })
}

/// Replace filesystem-unsafe characters.
fn sanitize_path_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mp4_with_free_atoms(count: usize) -> Vec<u8> {
        let free: Vec<u8> = (0..count).flat_map(|_| [0, 0, 0, 8, b'f', b'r', b'e', b'e']).collect();
        let udta = [&((free.len() + 8) as u32).to_be_bytes()[..], b"udta", &free].concat();
        let moov = [&((udta.len() + 8) as u32).to_be_bytes()[..], b"moov", &udta].concat();
        [&[0, 0, 0, 16][..], b"ftypM4A     ", &moov].concat()
    }

    fn flac_with_blocks(count: usize) -> Vec<u8> {
        let mut out = b"fLaC".to_vec();
        for i in 0..count {
            out.extend_from_slice(&[if i + 1 == count { 0x82 } else { 0x02 }, 0, 0, 4, b'f', b'u', b'z', b'z']);
        }
        out
    }

    fn check(name: &str, bytes: &[u8]) -> Result<()> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        check_tag_structure(&path)
    }

    #[test]
    fn flac_with_thousands_of_metadata_blocks_is_refused() {
        assert!(check("a.flac", &flac_with_blocks(MAX_FLAC_METADATA_BLOCKS + 50)).is_err());
        assert!(check("b.flac", &flac_with_blocks(20)).is_ok());
    }

    #[test]
    fn id3_tag_claiming_more_than_the_file_is_not_an_error_here() {
        let bytes = [&b"ID3\x04\x00\x00\x7f\x7f\x7f\x7f"[..], &[0u8; 20]].concat();
        assert!(check("d.mp3", &bytes).is_ok());
    }

    #[test]
    fn mp4_with_a_flood_of_atoms_is_refused() {
        assert!(check("a.m4a", &mp4_with_free_atoms(MAX_MP4_ATOMS + 50)).is_err());
        assert!(check("b.m4a", &mp4_with_free_atoms(200)).is_ok());
    }

    #[test]
    fn bogus_atom_sizes_do_not_loop_or_panic() {
        for size in [0u32, 1, 4, 7, 8, 0xffff_ffff] {
            let bytes = [&[0, 0, 0, 16][..], b"ftypM4A     ", &size.to_be_bytes(), b"moov", &[0u8; 64]].concat();
            assert!(check("c.m4a", &bytes).is_ok(), "size {size}");
        }
    }
}
