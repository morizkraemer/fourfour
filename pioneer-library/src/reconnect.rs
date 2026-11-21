//! Moved files: recognise a missing track's audio file somewhere else and point the track at it, and
//! re-point a whole folder root. The track id stays, so analysis, cues, edits and playlists stay.
//!
//! A track is recognised by size and duration (within [`DURATION_TOLERANCE_SECS`]), then confirmed by a
//! sampled content hash taken while its file was still there (`track_fingerprints`). The hash covers the
//! size and three 32 KiB samples, so a file whose tags were rewritten and changed in size is not found.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pioneer_usb_writer::scanner::read_track_metadata;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{DURATION_TOLERANCE_SECS, FileStatus, LocalLibrary, Relinked, file_status, normalise_tag, path_key, queries};

const SAMPLE_LEN: u64 = 32 * 1024;
const SAMPLE_COUNT: u64 = 3;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const PLACEHOLDER_ARTIST: &str = "unknown artist";

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Hash of a file's size and, for files over three samples long, three samples at a quarter, half and
/// three quarters of its length; smaller files are hashed whole. Stable across app versions: it is stored.
pub fn content_hash(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hash = fnv(FNV_OFFSET, &size.to_le_bytes());
    if size <= SAMPLE_LEN * SAMPLE_COUNT {
        let mut whole = Vec::with_capacity(size as usize);
        file.read_to_end(&mut whole)?;
        hash = fnv(hash, &whole);
    } else {
        let mut sample = vec![0u8; SAMPLE_LEN as usize];
        for n in 1..=SAMPLE_COUNT {
            let offset = (size * n / (SAMPLE_COUNT + 1)).min(size - SAMPLE_LEN);
            file.seek(SeekFrom::Start(offset))?;
            file.read_exact(&mut sample)?;
            hash = fnv(hash, &sample);
        }
    }
    Ok(format!("{hash:016x}"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissingTrack {
    pub track_id: i64,
    pub path: PathBuf,
    pub size: u64,
    pub duration_secs: f64,
    pub artist_key: String,
    pub title_key: String,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReconnectInputs {
    pub missing: Vec<MissingTrack>,
    pub known_path_keys: HashSet<String>,
    pub present_hashes: HashSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceReason {
    MultipleCandidates,
    Unconfirmed,
    Copy,
    SharedFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconnectChoice {
    pub track_id: i64,
    pub reason: ChoiceReason,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconnectPlan {
    pub reconnect: Vec<Relinked>,
    pub needs_choice: Vec<ReconnectChoice>,
    pub still_missing: Vec<i64>,
}

struct FileFacts {
    path: PathBuf,
    size: u64,
    duration_secs: f64,
    artist_key: String,
    title_key: String,
}

fn walk_files_of_sizes(folders: &[PathBuf], sizes: &HashSet<u64>, known: &HashSet<String>) -> Vec<(PathBuf, u64)> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    let mut pending: Vec<PathBuf> = folders.to_vec();
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if sizes.contains(&size) && !known.contains(&path_key(&path)) && seen.insert(path.clone()) {
                    found.push((path, size));
                }
            }
        }
    }
    found.sort();
    found
}

fn read_facts(path: PathBuf, size: u64) -> Option<FileFacts> {
    let track = read_track_metadata(&path, 0).ok()?;
    Some(FileFacts {
        path,
        size,
        duration_secs: track.duration_secs,
        artist_key: normalise_tag(&track.artist),
        title_key: normalise_tag(&track.title),
    })
}

fn same_length(missing: &MissingTrack, file: &FileFacts) -> bool {
    file.size == missing.size && (missing.duration_secs <= 0.0 || (file.duration_secs - missing.duration_secs).abs() <= DURATION_TOLERANCE_SECS)
}

fn tags_agree(missing: &MissingTrack, file: &FileFacts) -> bool {
    missing.artist_key == PLACEHOLDER_ARTIST
        || file.artist_key == PLACEHOLDER_ARTIST
        || (missing.artist_key == file.artist_key && missing.title_key == file.title_key)
}

/// Looks for the files of the missing tracks under `folders`. A file already used by a track is not
/// a candidate. Exactly one confirmed candidate is reconnected, unless that file is also wanted by another
/// missing track or is a copy of a track whose file is still there; then, like several candidates or a
/// single one that could not be confirmed against a stored hash, it needs the user's choice.
pub fn plan_reconnect(inputs: &ReconnectInputs, folders: &[PathBuf]) -> ReconnectPlan {
    let sizes: HashSet<u64> = inputs.missing.iter().map(|m| m.size).filter(|size| *size > 0).collect();
    let files: Vec<FileFacts> = walk_files_of_sizes(folders, &sizes, &inputs.known_path_keys)
        .into_iter()
        .filter_map(|(path, size)| read_facts(path, size))
        .collect();
    let mut hashes: HashMap<PathBuf, Option<String>> = HashMap::new();
    let mut hash_of = |path: &Path| -> Option<String> { hashes.entry(path.to_path_buf()).or_insert_with(|| content_hash(path).ok()).clone() };

    struct Found<'a> {
        track_id: i64,
        confirmed: Vec<(&'a FileFacts, String)>,
        unconfirmed: Vec<&'a FileFacts>,
    }
    let mut found: Vec<Found> = Vec::new();
    for missing in &inputs.missing {
        let mut entry = Found { track_id: missing.track_id, confirmed: Vec::new(), unconfirmed: Vec::new() };
        for file in files.iter().filter(|f| same_length(missing, f)) {
            let Some(hash) = hash_of(&file.path) else { continue };
            match &missing.hash {
                Some(stored) if *stored == hash => entry.confirmed.push((file, hash)),
                Some(_) => {}
                None if tags_agree(missing, file) => entry.unconfirmed.push(file),
                None => {}
            }
        }
        found.push(entry);
    }

    let mut wanted_by: HashMap<&Path, usize> = HashMap::new();
    for entry in &found {
        for (file, _) in &entry.confirmed {
            *wanted_by.entry(file.path.as_path()).or_default() += 1;
        }
    }

    let paths = |files: Vec<&FileFacts>| -> Vec<String> { files.iter().map(|f| f.path.to_string_lossy().into_owned()).collect() };
    let mut plan = ReconnectPlan::default();
    for entry in found {
        let track_id = entry.track_id;
        match (entry.confirmed.as_slice(), entry.unconfirmed.as_slice()) {
            ([], []) => plan.still_missing.push(track_id),
            ([], _) => plan.needs_choice.push(ReconnectChoice { track_id, reason: ChoiceReason::Unconfirmed, candidates: paths(entry.unconfirmed) }),
            ([(file, hash)], _) => {
                let reason = if wanted_by[file.path.as_path()] > 1 {
                    Some(ChoiceReason::SharedFile)
                } else if inputs.present_hashes.contains(hash) {
                    Some(ChoiceReason::Copy)
                } else {
                    None
                };
                match reason {
                    None => plan.reconnect.push(Relinked { track_id, source_path: file.path.to_string_lossy().into_owned() }),
                    Some(reason) => plan.needs_choice.push(ReconnectChoice { track_id, reason, candidates: paths(vec![file]) }),
                }
            }
            (several, _) => plan.needs_choice.push(ReconnectChoice {
                track_id,
                reason: ChoiceReason::MultipleCandidates,
                candidates: paths(several.iter().map(|(file, _)| *file).collect()),
            }),
        }
    }
    plan
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FingerprintTarget {
    pub track_id: i64,
    pub path: PathBuf,
}

/// Hashes the readable files among `targets`; unreadable or missing ones are left out.
pub fn fingerprint_files(targets: &[FingerprintTarget]) -> Vec<(i64, u64, String)> {
    targets
        .iter()
        .filter(|t| file_status(&t.path) == FileStatus::Ok)
        .filter_map(|t| {
            let hash = content_hash(&t.path).ok()?;
            let size = std::fs::metadata(&t.path).ok()?.len();
            Some((t.track_id, size, hash))
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemapPreview {
    pub total: usize,
    pub found: usize,
    pub still_missing: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemapApplied {
    pub relocation_id: i64,
    pub remapped: Vec<Relinked>,
    pub still_missing: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemapUndone {
    pub reverted: Vec<Relinked>,
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderRelocation {
    pub id: i64,
    pub old_root: String,
    pub new_root: String,
    pub tracks: usize,
    pub created_at: String,
}

struct RemapRow {
    track_id: i64,
    old_path: PathBuf,
    new_path: PathBuf,
}

impl LocalLibrary {
    /// What [`plan_reconnect`] needs, read in one go so the library need not stay locked while folders are searched.
    pub fn reconnect_inputs(&self) -> Result<ReconnectInputs> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.source_path, t.file_size, t.duration_secs, t.artist_key, t.title_key, t.path_key, f.hash
             FROM tracks t LEFT JOIN track_fingerprints f ON f.track_id = t.id ORDER BY t.id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                MissingTrack {
                    track_id: row.get(0)?,
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    size: row.get::<_, i64>(2)?.max(0) as u64,
                    duration_secs: row.get(3)?,
                    artist_key: row.get(4)?,
                    title_key: row.get(5)?,
                    hash: row.get(7)?,
                },
                row.get::<_, String>(6)?,
            ))
        })?;
        let mut inputs = ReconnectInputs::default();
        for row in rows {
            let (track, key) = row?;
            inputs.known_path_keys.insert(key);
            match file_status(&track.path) {
                FileStatus::Missing => inputs.missing.push(track),
                FileStatus::Ok => inputs.present_hashes.extend(track.hash),
                FileStatus::Error => {}
            }
        }
        Ok(inputs)
    }

    /// Tracks that have no content hash yet.
    pub fn fingerprint_targets(&self) -> Result<Vec<FingerprintTarget>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.source_path FROM tracks t LEFT JOIN track_fingerprints f ON f.track_id = t.id
             WHERE f.track_id IS NULL ORDER BY t.id",
        )?;
        let rows = stmt.query_map([], |row| Ok(FingerprintTarget { track_id: row.get(0)?, path: PathBuf::from(row.get::<_, String>(1)?) }))?;
        rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
    }

    pub fn store_fingerprints(&self, fingerprints: &[(i64, u64, String)]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (track_id, size, hash) in fingerprints {
            store_fingerprint(&tx, *track_id, *size, hash)?;
        }
        tx.commit().context("Failed to store the content hashes")
    }

    /// Points each track at its new file in one transaction. A track whose old file is still there is not
    /// moved but copied, a path another track uses is not free, and neither is relinked: the whole call fails.
    pub fn apply_reconnects(&self, picks: &[(i64, PathBuf)]) -> Result<Vec<Relinked>> {
        let known: HashMap<String, i64> = self.path_owners()?;
        let mut chosen = HashSet::new();
        for (track_id, new_path) in picks {
            let track = self.get_track(*track_id)?.with_context(|| format!("Track {track_id} is not in the library"))?;
            if file_status(&track.source_path) == FileStatus::Ok {
                bail!("The file of track {track_id} is still at {}; a file elsewhere is a copy, not a move", track.source_path.display());
            }
            if file_status(new_path) != FileStatus::Ok {
                bail!("{} is not a readable file", new_path.display());
            }
            let key = path_key(new_path);
            if known.get(&key).is_some_and(|owner| owner != track_id) || !chosen.insert(key) {
                bail!("{} already belongs to another track", new_path.display());
            }
        }
        let hashes: Vec<(i64, u64, String)> = picks
            .iter()
            .map(|(track_id, path)| {
                let hash = content_hash(path).with_context(|| format!("Failed to read {}", path.display()))?;
                Ok((*track_id, std::fs::metadata(path)?.len(), hash))
            })
            .collect::<Result<_>>()?;
        let ids: Vec<i64> = picks.iter().map(|(id, _)| *id).collect();
        let old = self.duplicates_before(&ids)?;
        let tx = self.conn.unchecked_transaction()?;
        for (track_id, new_path) in picks {
            queries::update_source_path(&tx, *track_id, new_path)?;
        }
        for (track_id, size, hash) in &hashes {
            store_fingerprint(&tx, *track_id, *size, hash)?;
        }
        tx.commit().context("Failed to reconnect the tracks")?;
        self.duplicates_after_path_change(old, &picks.iter().cloned().collect());
        Ok(picks.iter().map(|(track_id, path)| Relinked { track_id: *track_id, source_path: path.to_string_lossy().into_owned() }).collect())
    }

    fn path_owners(&self) -> Result<HashMap<String, i64>> {
        let mut stmt = self.conn.prepare("SELECT path_key, id FROM tracks")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
        rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
    }

    fn remap_rows(&self, old_root: &Path, new_root: &Path) -> Result<(Vec<RemapRow>, usize)> {
        if old_root == new_root {
            bail!("The old and the new folder are the same");
        }
        if !new_root.is_dir() {
            bail!("{} is not a folder", new_root.display());
        }
        let owners = self.path_owners()?;
        let mut under = 0;
        let mut rows = Vec::new();
        for file in self.track_files()? {
            let Ok(relative) = file.path.strip_prefix(old_root) else { continue };
            under += 1;
            let new_path = new_root.join(relative);
            let free = owners.get(&path_key(&new_path)).is_none_or(|owner| *owner == file.track_id);
            if free && file_status(&new_path) == FileStatus::Ok {
                rows.push(RemapRow { track_id: file.track_id, old_path: file.path, new_path });
            }
        }
        Ok((rows, under))
    }

    /// How many tracks under `old_root` would be found under the same relative path in `new_root`.
    pub fn preview_remap(&self, old_root: &Path, new_root: &Path) -> Result<RemapPreview> {
        let (rows, total) = self.remap_rows(old_root, new_root)?;
        Ok(RemapPreview { total, found: rows.len(), still_missing: total - rows.len() })
    }

    /// Re-points every track under `old_root` whose file exists under `new_root`, keeping the old
    /// paths in `folder_relocations` so [`Self::undo_remap`] can put them back.
    pub fn apply_remap(&self, old_root: &Path, new_root: &Path) -> Result<RemapApplied> {
        let (rows, total) = self.remap_rows(old_root, new_root)?;
        if rows.is_empty() {
            bail!("No file of {} was found under {}", old_root.display(), new_root.display());
        }
        let ids: Vec<i64> = rows.iter().map(|r| r.track_id).collect();
        let old = self.duplicates_before(&ids)?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO folder_relocations (old_root, new_root, created_at) VALUES (?1, ?2, ?3)",
            params![old_root.to_string_lossy(), new_root.to_string_lossy(), chrono::Utc::now().to_rfc3339()],
        )?;
        let relocation_id = tx.last_insert_rowid();
        for row in &rows {
            tx.execute(
                "INSERT INTO folder_relocation_tracks (relocation_id, track_id, old_path, new_path) VALUES (?1, ?2, ?3, ?4)",
                params![relocation_id, row.track_id, row.old_path.to_string_lossy(), row.new_path.to_string_lossy()],
            )?;
            queries::update_source_path(&tx, row.track_id, &row.new_path)?;
        }
        tx.commit().context("Failed to relocate the folder")?;
        self.duplicates_after_path_change(old, &rows.iter().map(|r| (r.track_id, r.new_path.clone())).collect());
        let remapped = rows.iter().map(|r| Relinked { track_id: r.track_id, source_path: r.new_path.to_string_lossy().into_owned() }).collect();
        Ok(RemapApplied { relocation_id, remapped, still_missing: total - rows.len() })
    }

    /// Puts the old paths back. A track that was moved again since, or whose old path another track now
    /// uses, is left alone and counted as skipped.
    pub fn undo_remap(&self, relocation_id: i64) -> Result<RemapUndone> {
        let mut stmt = self.conn.prepare("SELECT track_id, old_path, new_path FROM folder_relocation_tracks WHERE relocation_id = ?1 ORDER BY track_id")?;
        let rows: Vec<RemapRow> = stmt
            .query_map([relocation_id], |row| {
                Ok(RemapRow { track_id: row.get(0)?, old_path: PathBuf::from(row.get::<_, String>(1)?), new_path: PathBuf::from(row.get::<_, String>(2)?) })
            })?
            .collect::<rusqlite::Result<_>>()?;
        if rows.is_empty() {
            bail!("Relocation {relocation_id} is not there to undo");
        }
        let owners = self.path_owners()?;
        let revert: Vec<&RemapRow> = rows
            .iter()
            .filter(|row| {
                owners.get(&path_key(&row.new_path)) == Some(&row.track_id) && !owners.contains_key(&path_key(&row.old_path))
            })
            .collect();
        let ids: Vec<i64> = revert.iter().map(|r| r.track_id).collect();
        let old = self.duplicates_before(&ids)?;
        let tx = self.conn.unchecked_transaction()?;
        for row in &revert {
            queries::update_source_path(&tx, row.track_id, &row.old_path)?;
        }
        tx.execute("DELETE FROM folder_relocations WHERE id = ?1", [relocation_id])?;
        tx.commit().context("Failed to undo the relocation")?;
        self.duplicates_after_path_change(old, &revert.iter().map(|r| (r.track_id, r.old_path.clone())).collect());
        Ok(RemapUndone {
            reverted: revert.iter().map(|r| Relinked { track_id: r.track_id, source_path: r.old_path.to_string_lossy().into_owned() }).collect(),
            skipped: rows.len() - revert.len(),
        })
    }

    /// The newest relocation that can still be undone.
    pub fn last_folder_relocation(&self) -> Result<Option<FolderRelocation>> {
        self.conn
            .query_row(
                "SELECT r.id, r.old_root, r.new_root, r.created_at, (SELECT COUNT(*) FROM folder_relocation_tracks WHERE relocation_id = r.id)
                 FROM folder_relocations r ORDER BY r.id DESC LIMIT 1",
                [],
                |row| {
                    Ok(FolderRelocation {
                        id: row.get(0)?,
                        old_root: row.get(1)?,
                        new_root: row.get(2)?,
                        created_at: row.get(3)?,
                        tracks: row.get::<_, i64>(4)? as usize,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }
}

fn store_fingerprint(conn: &rusqlite::Connection, track_id: i64, size: u64, hash: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO track_fingerprints (track_id, size, hash) VALUES (?1, ?2, ?3)
         ON CONFLICT(track_id) DO UPDATE SET size = excluded.size, hash = excluded.hash",
        params![track_id, size as i64, hash],
    )?;
    Ok(())
}
