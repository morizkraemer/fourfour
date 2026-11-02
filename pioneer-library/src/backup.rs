//! Library backup: one `.tar.gz` holding the database, the per-track analysis folder
//! (ANLZ files and `edits.json`), settings handed in by the app and, optionally, the audio.
//! Audio keeps its folder structure below the tracks' common folder; the manifest maps each
//! track's old absolute path to its place in the archive so a restore can relink it.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};

use crate::analysis_write::STAGING_PREFIX;
use crate::failure::{Failure, LibraryFailure};
use crate::{FileStatus, LocalLibrary, file_status, queries};

pub const BACKUP_FORMAT: &str = "library-backup";
pub const BACKUP_VERSION: u32 = 1;
pub const BACKUP_EXTENSION: &str = "tar.gz";

const MANIFEST: &str = "manifest.json";
const LIBRARY_DIR: &str = "library";
const DB_FILE: &str = "library.db";
const ANLZ_DIR: &str = "anlz";
const SETTINGS_DIR: &str = "settings";
const AUDIO_DIR: &str = "audio";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioEntry {
    pub track_id: i64,
    pub original_path: String,
    pub archive_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub format: String,
    pub version: u32,
    pub created_at: String,
    pub track_count: usize,
    pub playlist_count: usize,
    pub total_bytes: u64,
    pub includes_audio: bool,
    pub audio_root: Option<String>,
    pub audio: Vec<AudioEntry>,
    pub audio_missing: Vec<i64>,
    pub settings: Vec<String>,
}

/// A setting blob stored under `settings/{name}`; the app decides what the bytes mean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BackupSizes {
    pub library_bytes: u64,
    pub audio_bytes: u64,
    pub audio_files: usize,
}

/// A backup unpacked into a staging folder and checked, not yet installed.
#[derive(Debug)]
pub struct UnpackedBackup {
    pub manifest: BackupManifest,
    pub staging: PathBuf,
    pub settings: Vec<SettingsFile>,
    pub audio_paths: HashMap<i64, PathBuf>,
}

impl UnpackedBackup {
    fn db_path(&self) -> PathBuf {
        self.staging.join(LIBRARY_DIR).join(DB_FILE)
    }

    fn anlz_path(&self) -> PathBuf {
        self.staging.join(LIBRARY_DIR).join(ANLZ_DIR)
    }

    pub fn discard(self) {
        let _ = std::fs::remove_dir_all(&self.staging);
    }
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                if !entry.file_name().to_string_lossy().starts_with(STAGING_PREFIX) {
                    pending.push(entry.path());
                }
            } else if kind.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn normal_parts(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().replace(':', "")),
            _ => None,
        })
        .collect()
}

/// The deepest folder every path sits in, as path parts (empty when they share none).
pub fn common_folder(paths: &[PathBuf]) -> Vec<String> {
    let mut iter = paths.iter().map(|p| {
        let mut parts = normal_parts(p);
        parts.pop();
        parts
    });
    let Some(mut common) = iter.next() else { return Vec::new() };
    for parts in iter {
        let shared = common.iter().zip(&parts).take_while(|(a, b)| a == b).count();
        common.truncate(shared);
    }
    common
}

/// Archive name of an audio file: `audio/` plus its path below the common folder.
pub fn audio_archive_path(path: &Path, common: &[String]) -> String {
    let parts = normal_parts(path);
    let mut out = vec![AUDIO_DIR.to_string()];
    out.extend(parts.into_iter().skip(common.len()));
    out.join("/")
}

fn safe_relative(name: &Path) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in name.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => {
                return Err(Failure::error(
                    LibraryFailure::InvalidBackup,
                    format!("The backup contains an unsafe path: {}", name.display()),
                ));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(Failure::error(LibraryFailure::InvalidBackup, "The backup contains an empty path"));
    }
    Ok(out)
}

fn not_a_backup() -> anyhow::Error {
    Failure::error(LibraryFailure::InvalidBackup, "This file is not a library backup")
}

fn damaged_backup() -> Failure {
    Failure::new(LibraryFailure::InvalidBackup, "The backup file is damaged")
}

fn read_manifest_entry<R: Read>(entries: &mut tar::Entries<'_, R>) -> Result<BackupManifest> {
    let mut first = entries.next().ok_or_else(not_a_backup)?.map_err(|_| not_a_backup())?;
    if first.path().map_err(|_| not_a_backup())?.as_ref() != Path::new(MANIFEST) {
        return Err(not_a_backup());
    }
    let mut bytes = Vec::new();
    first.read_to_end(&mut bytes).map_err(|_| not_a_backup())?;
    let manifest: BackupManifest = serde_json::from_slice(&bytes).map_err(|_| not_a_backup())?;
    if manifest.format != BACKUP_FORMAT {
        return Err(not_a_backup());
    }
    if manifest.version > BACKUP_VERSION {
        return Err(Failure::error(
            LibraryFailure::NewerVersion,
            format!("This backup was made by a newer version of the app (format {})", manifest.version),
        ));
    }
    Ok(manifest)
}

fn open_archive(path: &Path) -> Result<tar::Archive<GzDecoder<BufReader<File>>>> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    Ok(tar::Archive::new(GzDecoder::new(BufReader::new(file))))
}

/// Reads only the manifest: enough to describe the backup before restoring it.
pub fn read_backup_manifest(archive: &Path) -> Result<BackupManifest> {
    let mut archive = open_archive(archive)?;
    let mut entries = archive.entries().map_err(|_| not_a_backup())?;
    read_manifest_entry(&mut entries)
}

/// Picks a file name next to `wanted` that does not exist yet: `name (2).ext`, `name (3).ext`, ...
fn free_path(wanted: &Path) -> PathBuf {
    let stem = wanted.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = wanted.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| wanted.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("an unused file name")
}

fn write_new_file<R: Read>(target: &Path, reader: &mut R) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .with_context(|| format!("Failed to create {}", target.display()))?;
    let mut out = BufWriter::new(file);
    std::io::copy(reader, &mut out).map_err(|e| {
        let message = format!("Failed to write {}", target.display());
        if matches!(e.kind(), std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::InvalidData) {
            anyhow::Error::new(e).context(Failure::new(LibraryFailure::InvalidBackup, message))
        } else {
            anyhow::Error::new(e).context(message)
        }
    })?;
    out.flush()?;
    Ok(())
}

/// Unpacks `archive` into `staging` (which must not exist) and checks it is a complete,
/// readable backup. Audio goes below `audio_dest` when given and is skipped otherwise; an
/// audio file already there with the same size is reused, a different one is never
/// overwritten. On any error the staging folder is removed and nothing else changes.
pub fn unpack_backup(
    archive: &Path,
    staging: &Path,
    audio_dest: Option<&Path>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<UnpackedBackup> {
    if staging.exists() {
        bail!("{} already exists", staging.display());
    }
    std::fs::create_dir_all(staging).with_context(|| format!("Failed to create {}", staging.display()))?;
    let result = unpack_into(archive, staging, audio_dest, progress);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(staging);
    }
    result
}

fn unpack_into(
    archive_path: &Path,
    staging: &Path,
    audio_dest: Option<&Path>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<UnpackedBackup> {
    let mut archive = open_archive(archive_path)?;
    let mut entries = archive.entries().map_err(|_| not_a_backup())?;
    let manifest = read_manifest_entry(&mut entries)?;
    let audio_ids: HashMap<&str, i64> = manifest.audio.iter().map(|a| (a.archive_path.as_str(), a.track_id)).collect();
    let mut settings = Vec::new();
    let mut audio_paths = HashMap::new();
    let mut done = 0u64;
    for entry in entries {
        let mut entry = entry.context(damaged_backup())?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let name = safe_relative(&entry.path().context(damaged_backup())?)?;
        let size = entry.size();
        let first = name.components().next().map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default();
        match first.as_str() {
            LIBRARY_DIR => write_new_file(&staging.join(&name), &mut entry)?,
            SETTINGS_DIR => {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).context(damaged_backup())?;
                let file_name = name.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                settings.push(SettingsFile { name: file_name, bytes });
            }
            AUDIO_DIR => {
                let archive_name = name.to_string_lossy().replace('\\', "/");
                match (audio_dest, audio_ids.get(archive_name.as_str())) {
                    (Some(dest), Some(&track_id)) => {
                        let below: PathBuf = name.components().skip(1).collect();
                        let wanted = dest.join(below);
                        let target = if wanted.exists() && file_len(&wanted) == size {
                            std::io::copy(&mut entry, &mut std::io::sink()).context(damaged_backup())?;
                            wanted
                        } else {
                            let target = if wanted.exists() { free_path(&wanted) } else { wanted };
                            write_new_file(&target, &mut entry)?;
                            target
                        };
                        audio_paths.insert(track_id, target);
                    }
                    _ => {
                        std::io::copy(&mut entry, &mut std::io::sink()).context(damaged_backup())?;
                    }
                }
            }
            _ => {
                std::io::copy(&mut entry, &mut std::io::sink()).context(damaged_backup())?;
            }
        }
        done += size;
        progress(done, manifest.total_bytes);
    }
    let unpacked = UnpackedBackup { manifest, staging: staging.to_path_buf(), settings, audio_paths };
    let db = unpacked.db_path();
    if !db.is_file() {
        return Err(Failure::error(LibraryFailure::InvalidBackup, "The backup has no library database"));
    }
    let restored = LocalLibrary::open(&db).context(Failure::new(LibraryFailure::InvalidBackup, "The library in the backup can't be opened"))?;
    let tracks = queries::count_tracks(&restored.conn)?;
    if tracks != unpacked.manifest.track_count {
        return Err(Failure::error(
            LibraryFailure::InvalidBackup,
            format!("The backup is incomplete: {tracks} of {} tracks", unpacked.manifest.track_count),
        ));
    }
    Ok(unpacked)
}

fn remove_db_files(db: &Path) -> Result<()> {
    let base = db.to_string_lossy().into_owned();
    for suffix in ["", "-wal", "-shm"] {
        let path = PathBuf::from(format!("{base}{suffix}"));
        if path.exists() {
            std::fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

/// Puts an unpacked backup in place of the library at `library_db` (nothing else in its
/// folder changes) and points restored audio tracks at their unpacked files. The library
/// must be closed. Returns the reopened library.
pub fn install_backup(unpacked: UnpackedBackup, library_db: &Path) -> Result<LocalLibrary> {
    let folder = library_db.parent().unwrap_or(Path::new(".")).to_path_buf();
    let anlz = folder.join(ANLZ_DIR);
    let aside = unpacked.staging.join("previous");
    std::fs::create_dir_all(&aside)?;
    if anlz.exists() {
        std::fs::rename(&anlz, aside.join(ANLZ_DIR)).with_context(|| format!("Failed to move {}", anlz.display()))?;
    }
    remove_db_files(library_db)?;
    std::fs::rename(unpacked.db_path(), library_db).with_context(|| format!("Failed to write {}", library_db.display()))?;
    if unpacked.anlz_path().exists() {
        std::fs::rename(unpacked.anlz_path(), &anlz).with_context(|| format!("Failed to write {}", anlz.display()))?;
    }
    let library = LocalLibrary::open(library_db)?;
    library.relink_paths(&unpacked.audio_paths)?;
    unpacked.discard();
    Ok(library)
}

impl LocalLibrary {
    /// What a backup would hold, for the Settings page: library data and the audio that is there.
    pub fn backup_sizes(&self) -> Result<BackupSizes> {
        let Some(db) = &self.db_path else { return Ok(BackupSizes::default()) };
        let anlz: u64 = self.anlz_dir().map(|d| files_under(&d).iter().map(|f| file_len(f)).sum()).unwrap_or(0);
        let mut sizes = BackupSizes { library_bytes: file_len(db) + anlz, ..BackupSizes::default() };
        for file in self.track_files()? {
            if file_status(&file.path) == FileStatus::Ok {
                sizes.audio_bytes += file_len(&file.path);
                sizes.audio_files += 1;
            }
        }
        Ok(sizes)
    }

    fn relink_paths(&self, paths: &HashMap<i64, PathBuf>) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (track_id, path) in paths {
            queries::update_source_path(&tx, *track_id, path)?;
        }
        tx.commit()?;
        self.forget_duplicates();
        Ok(())
    }

    /// Writes the backup to `dest` (via a `.partial` file, so a failed export leaves nothing
    /// half-written). `progress` gets bytes written so far and the total.
    pub fn export_backup(
        &self,
        dest: &Path,
        include_audio: bool,
        settings: &[SettingsFile],
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<BackupManifest> {
        self.begin_backup(dest, include_audio, settings, progress)?.finish(progress)
    }

    /// Writes everything but the audio: the part that needs the library. [`PendingBackup::finish`]
    /// adds the audio and does not touch the library, so it can run without holding it.
    pub fn begin_backup(
        &self,
        dest: &Path,
        include_audio: bool,
        settings: &[SettingsFile],
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<PendingBackup> {
        let db = self.db_path.as_ref().ok_or_else(|| anyhow!("An in-memory library can't be backed up"))?;
        let partial = PathBuf::from(format!("{}.partial", dest.display()));
        let snapshot = PathBuf::from(format!("{}.db", partial.display()));
        let result = self.write_library_part(db, &partial, &snapshot, include_audio, settings, progress);
        let _ = std::fs::remove_file(&snapshot);
        match result {
            Ok((tar, manifest, done)) => Ok(PendingBackup { tar, manifest, done, partial, dest: dest.to_path_buf() }),
            Err(e) => {
                let _ = std::fs::remove_file(&partial);
                Err(e)
            }
        }
    }

    fn write_library_part(
        &self,
        db: &Path,
        partial: &Path,
        snapshot: &Path,
        include_audio: bool,
        settings: &[SettingsFile],
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<(BackupTar, BackupManifest, u64)> {
        let _ = std::fs::remove_file(snapshot);
        self.conn
            .execute("VACUUM INTO ?1", [snapshot.to_string_lossy()])
            .with_context(|| format!("Failed to copy the library database {}", db.display()))?;
        let anlz_root = self.anlz_dir().unwrap_or_default();
        let anlz_files = files_under(&anlz_root);

        let mut audio = Vec::new();
        let mut audio_missing = Vec::new();
        let mut audio_root = None;
        if include_audio {
            let files = self.track_files()?;
            let (present, missing): (Vec<_>, Vec<_>) = files.into_iter().partition(|f| file_status(&f.path) == FileStatus::Ok);
            audio_missing = missing.iter().map(|f| f.track_id).collect();
            let paths: Vec<PathBuf> = present.iter().map(|f| f.path.clone()).collect();
            let common = common_folder(&paths);
            audio_root = Some(format!("/{}", common.join("/")));
            audio = present
                .iter()
                .map(|f| AudioEntry {
                    track_id: f.track_id,
                    original_path: f.path.to_string_lossy().into_owned(),
                    archive_path: audio_archive_path(&f.path, &common),
                })
                .collect();
        }

        let total_bytes = file_len(snapshot)
            + anlz_files.iter().map(|f| file_len(f)).sum::<u64>()
            + audio.iter().map(|a| file_len(Path::new(&a.original_path))).sum::<u64>()
            + settings.iter().map(|s| s.bytes.len() as u64).sum::<u64>();
        let manifest = BackupManifest {
            format: BACKUP_FORMAT.into(),
            version: BACKUP_VERSION,
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            track_count: queries::count_tracks(&self.conn)?,
            playlist_count: self.get_all_playlists()?.iter().filter(|p| !p.is_folder).count(),
            total_bytes,
            includes_audio: include_audio,
            audio_root,
            audio,
            audio_missing,
            settings: settings.iter().map(|s| s.name.clone()).collect(),
        };

        let file = File::create(partial).with_context(|| format!("Failed to create {}", partial.display()))?;
        let mut tar = tar::Builder::new(GzEncoder::new(BufWriter::new(file), Compression::fast()));
        tar.mode(tar::HeaderMode::Deterministic);
        append_bytes(&mut tar, MANIFEST, &serde_json::to_vec_pretty(&manifest)?)?;
        let mut done = 0u64;
        append_file(&mut tar, &format!("{LIBRARY_DIR}/{DB_FILE}"), snapshot, &mut done)?;
        progress(done, total_bytes);
        for file in &anlz_files {
            let rel = file.strip_prefix(&anlz_root).unwrap_or(file);
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            append_file(&mut tar, &format!("{LIBRARY_DIR}/{ANLZ_DIR}/{rel}"), file, &mut done)?;
            progress(done, total_bytes);
        }
        for setting in settings {
            append_bytes(&mut tar, &format!("{SETTINGS_DIR}/{}", setting.name), &setting.bytes)?;
            done += setting.bytes.len() as u64;
        }
        progress(done, total_bytes);
        Ok((tar, manifest, done))
    }
}

type BackupTar = tar::Builder<GzEncoder<BufWriter<File>>>;

/// A backup whose library part is written; the audio follows in [`PendingBackup::finish`].
pub struct PendingBackup {
    tar: BackupTar,
    manifest: BackupManifest,
    done: u64,
    partial: PathBuf,
    dest: PathBuf,
}

impl PendingBackup {
    pub fn finish(self, progress: &mut dyn FnMut(u64, u64)) -> Result<BackupManifest> {
        let PendingBackup { tar, manifest, done, partial, dest } = self;
        if let Err(e) = write_audio(tar, &manifest, done, progress) {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
        std::fs::rename(&partial, &dest).with_context(|| format!("Failed to write {}", dest.display()))?;
        Ok(manifest)
    }
}

fn write_audio(mut tar: BackupTar, manifest: &BackupManifest, mut done: u64, progress: &mut dyn FnMut(u64, u64)) -> Result<()> {
    for entry in &manifest.audio {
        append_file(&mut tar, &entry.archive_path, Path::new(&entry.original_path), &mut done)?;
        progress(done, manifest.total_bytes);
    }
    let mut writer = tar.into_inner()?.finish()?;
    writer.flush()?;
    writer.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    Ok(())
}

fn append_file<W: Write>(tar: &mut tar::Builder<W>, name: &str, path: &Path, done: &mut u64) -> Result<()> {
    let mut f = File::open(path).with_context(|| format!("Failed to read {}", path.display()))?;
    tar.append_file(name, &mut f).with_context(|| format!("Failed to add {}", path.display()))?;
    *done += file_len(path);
    Ok(())
}

fn append_bytes<W: Write>(tar: &mut tar::Builder<W>, name: &str, bytes: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    tar.append_data(&mut header, name, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BeatPosition;
    use crate::tests::{make_analysis, make_track};

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        lib: LocalLibrary,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let music = root.join("Music");
        std::fs::create_dir_all(music.join("House")).unwrap();
        std::fs::create_dir_all(music.join("Techno/Peak")).unwrap();
        std::fs::write(music.join("House/a.mp3"), b"audio-a").unwrap();
        std::fs::write(music.join("Techno/Peak/b.mp3"), b"audio-bb").unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let lib = LocalLibrary::open(&root.join("lib/library.db")).unwrap();
        let a = lib.add_track(&make_track("A", music.join("House/a.mp3").to_str().unwrap())).unwrap();
        let b = lib.add_track(&make_track("B", music.join("Techno/Peak/b.mp3").to_str().unwrap())).unwrap();
        lib.add_track(&make_track("Gone", music.join("gone.mp3").to_str().unwrap())).unwrap();
        let set = lib.create_playlist("Set").unwrap();
        lib.set_playlist_tracks(set, &[b, a]).unwrap();
        lib.set_analysis(a, &make_analysis()).unwrap();
        lib.set_user_rating(a, 4).unwrap();
        let grid = [BeatPosition { time_ms: 120, bar_position: 1 }, BeatPosition { time_ms: 589, bar_position: 2 }];
        lib.set_beat_grid(a, &grid, Some(0.25), Some(120.25)).unwrap();
        Fixture { _dir: dir, root, lib }
    }

    fn settings() -> Vec<SettingsFile> {
        vec![SettingsFile { name: "ui.json".into(), bytes: br#"{"fourfour.quantize.v1":"1"}"#.to_vec() }]
    }

    fn snapshot(lib: &LocalLibrary) -> serde_json::Value {
        let tracks = lib.get_all_tracks().unwrap();
        let ids: Vec<i64> = tracks.iter().map(|t| t.id as i64).collect();
        serde_json::json!({
            "tracks": tracks,
            "playlists": lib.get_all_playlists().unwrap(),
            "edits": ids.iter().map(|&id| lib.user_edits(id)).collect::<Vec<_>>(),
            "analyses": ids.iter().map(|&id| lib.get_analysis(id).unwrap()).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn export_then_restore_gives_back_the_library_edits_and_settings() {
        let f = fixture();
        let before = snapshot(&f.lib);
        let archive = f.root.join("backup.tar.gz");
        let mut calls = 0;
        let manifest = f.lib.export_backup(&archive, false, &settings(), &mut |_, _| calls += 1).unwrap();
        assert!(calls > 0);
        assert_eq!((manifest.track_count, manifest.playlist_count, manifest.includes_audio), (3, 1, false));
        assert_eq!(read_backup_manifest(&archive).unwrap(), manifest);
        assert!(!f.root.join("backup.tar.gz.partial").exists());

        f.lib.remove_track(1).unwrap();
        f.lib.set_user_rating(2, 1).unwrap();
        let db = f.root.join("lib/library.db");
        drop(f.lib);

        let unpacked = unpack_backup(&archive, &f.root.join("lib/.restore"), None, &mut |_, _| {}).unwrap();
        assert_eq!(unpacked.settings, settings());
        let restored = install_backup(unpacked, &db).unwrap();
        assert_eq!(snapshot(&restored), before);
        assert!(!f.root.join("lib/.restore").exists());
        assert!(f.root.join("Music/House/a.mp3").exists());
        assert_eq!(restored.user_edits(1).rating, Some(4));
        assert_eq!(restored.user_edits(1).grid_offset_ms, Some(0.25));
    }

    #[test]
    fn analysis_staging_folders_stay_out_of_the_backup() {
        let f = fixture();
        let sizes_without = f.lib.backup_sizes().unwrap().library_bytes;
        let staging = f.lib.track_anlz_dir(1).unwrap().join(format!("{STAGING_PREFIX}1-0"));
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("ANLZ0000.DAT"), b"half written").unwrap();
        assert_eq!(f.lib.backup_sizes().unwrap().library_bytes, sizes_without);
        let archive = f.root.join("backup.tar.gz");
        f.lib.export_backup(&archive, false, &settings(), &mut |_, _| {}).unwrap();

        let unpacked = unpack_backup(&archive, &f.root.join("lib/.restore"), None, &mut |_, _| {}).unwrap();
        assert!(!unpacked.anlz_path().join("1").join(format!("{STAGING_PREFIX}1-0")).exists());
        unpacked.discard();
    }

    #[test]
    fn backup_with_audio_keeps_folders_and_relinks_to_the_unpacked_files() {
        let f = fixture();
        let archive = f.root.join("full.tar.gz");
        let manifest = f.lib.export_backup(&archive, true, &[], &mut |_, _| {}).unwrap();
        let music = f.root.join("Music");
        assert_eq!(manifest.audio_root.as_deref(), Some(format!("/{}", normal_parts(&music).join("/")).as_str()));
        let mut names: Vec<_> = manifest.audio.iter().map(|a| (a.archive_path.clone(), a.original_path.clone())).collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                ("audio/House/a.mp3".to_string(), music.join("House/a.mp3").to_string_lossy().into_owned()),
                ("audio/Techno/Peak/b.mp3".to_string(), music.join("Techno/Peak/b.mp3").to_string_lossy().into_owned()),
            ]
        );
        assert_eq!(manifest.audio_missing, vec![3]);

        let db = f.root.join("lib/library.db");
        drop(f.lib);
        let dest = f.root.join("Restored");
        std::fs::create_dir_all(dest.join("Techno/Peak")).unwrap();
        std::fs::write(dest.join("Techno/Peak/b.mp3"), b"someone else's file").unwrap();
        let unpacked = unpack_backup(&archive, &f.root.join("lib/.restore"), Some(&dest), &mut |_, _| {}).unwrap();
        let restored = install_backup(unpacked, &db).unwrap();
        let paths: Vec<PathBuf> = restored.get_all_tracks().unwrap().into_iter().map(|t| t.source_path).collect();
        assert_eq!(paths, vec![dest.join("House/a.mp3"), dest.join("Techno/Peak/b (2).mp3"), music.join("gone.mp3")]);
        assert_eq!(std::fs::read(dest.join("House/a.mp3")).unwrap(), b"audio-a");
        assert_eq!(std::fs::read(dest.join("Techno/Peak/b (2).mp3")).unwrap(), b"audio-bb");
        assert_eq!(std::fs::read(dest.join("Techno/Peak/b.mp3")).unwrap(), b"someone else's file");
        assert_eq!(std::fs::read(music.join("House/a.mp3")).unwrap(), b"audio-a");
    }

    #[test]
    fn foreign_and_damaged_files_are_refused_without_changes() {
        let f = fixture();
        let foreign = f.root.join("foreign.tar.gz");
        std::fs::write(&foreign, b"just some bytes").unwrap();
        assert_eq!(read_backup_manifest(&foreign).unwrap_err().to_string(), "This file is not a library backup");

        let other_tar = f.root.join("other.tar.gz");
        {
            let mut tar = tar::Builder::new(GzEncoder::new(File::create(&other_tar).unwrap(), Compression::fast()));
            append_bytes(&mut tar, "manifest.json", br#"{"format":"something-else"}"#).unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        assert_eq!(read_backup_manifest(&other_tar).unwrap_err().to_string(), "This file is not a library backup");

        let archive = f.root.join("good.tar.gz");
        f.lib.export_backup(&archive, false, &[], &mut |_, _| {}).unwrap();
        let bytes = std::fs::read(&archive).unwrap();
        let damaged = f.root.join("damaged.tar.gz");
        std::fs::write(&damaged, &bytes[..bytes.len() * 2 / 3]).unwrap();
        let staging = f.root.join("lib/.restore");
        assert!(unpack_backup(&damaged, &staging, None, &mut |_, _| {}).is_err());
        assert!(!staging.exists());
        assert_eq!(f.lib.get_all_tracks().unwrap().len(), 3);
    }

    #[test]
    fn refused_backups_carry_a_failure_kind_and_keep_their_text() {
        use crate::failure::{LibraryFailure, classify};
        let f = fixture();

        let foreign = f.root.join("foreign.tar.gz");
        std::fs::write(&foreign, b"just some bytes").unwrap();
        let err = read_backup_manifest(&foreign).unwrap_err();
        assert_eq!(classify(&err), Some(LibraryFailure::InvalidBackup));
        assert_eq!(err.to_string(), "This file is not a library backup");

        let newer = f.root.join("newer.tar.gz");
        {
            let manifest = BackupManifest {
                format: BACKUP_FORMAT.to_string(),
                version: BACKUP_VERSION + 1,
                created_at: String::new(),
                track_count: 0,
                playlist_count: 0,
                total_bytes: 0,
                includes_audio: false,
                audio_root: None,
                audio: Vec::new(),
                audio_missing: Vec::new(),
                settings: Vec::new(),
            };
            let mut tar = tar::Builder::new(GzEncoder::new(File::create(&newer).unwrap(), Compression::fast()));
            append_bytes(&mut tar, MANIFEST, &serde_json::to_vec(&manifest).unwrap()).unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        let err = read_backup_manifest(&newer).unwrap_err();
        assert_eq!(classify(&err), Some(LibraryFailure::NewerVersion));
        assert!(err.to_string().starts_with("This backup was made by a newer version of the app"), "{err}");

        let archive = f.root.join("good.tar.gz");
        f.lib.export_backup(&archive, false, &[], &mut |_, _| {}).unwrap();
        let bytes = std::fs::read(&archive).unwrap();
        let damaged = f.root.join("damaged.tar.gz");
        std::fs::write(&damaged, &bytes[..bytes.len() * 2 / 3]).unwrap();
        let err = unpack_backup(&damaged, &f.root.join("lib/.restore"), None, &mut |_, _| {}).err().unwrap();
        assert_eq!(classify(&err), Some(LibraryFailure::InvalidBackup), "{err:#}");

        let err = safe_relative(Path::new("../evil")).unwrap_err();
        assert_eq!(classify(&err), Some(LibraryFailure::InvalidBackup));
        assert!(err.to_string().starts_with("The backup contains an unsafe path"), "{err}");
        assert_eq!(classify(&safe_relative(Path::new("")).unwrap_err()), Some(LibraryFailure::InvalidBackup));
    }

    #[test]
    fn common_folder_and_archive_paths() {
        let paths = [PathBuf::from("/Users/m/Music/A/x.mp3"), PathBuf::from("/Users/m/Music/B/C/y.mp3")];
        let common = common_folder(&paths);
        assert_eq!(common, ["Users", "m", "Music"]);
        assert_eq!(audio_archive_path(&paths[1], &common), "audio/B/C/y.mp3");
        assert_eq!(common_folder(&[PathBuf::from("/Volumes/X/a.mp3"), PathBuf::from("/Users/a.mp3")]), Vec::<String>::new());
        assert!(safe_relative(Path::new("../evil")).is_err());
        assert!(safe_relative(Path::new("/abs")).is_err());
    }
}
