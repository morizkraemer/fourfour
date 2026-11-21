//! Merging duplicate entries into a keeper, and the archive that holds what was merged away.
//!
//! A merge never touches an audio file. The removed entries leave the library as a snapshot of their rows (and their
//! `anlz` folder, moved to `anlz/_archive`), so [`LocalLibrary::restore_merge`] brings them back with the same ids,
//! playlist places and cues.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::Hasher;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::dedupe::{files_identical, is_lossless};
use crate::rekordbox::{ImportedCue, ImportedMeta};
use crate::{FileStatus, LocalLibrary, file_status, queries};

pub const ARCHIVE_DIR: &str = "_archive";

const MEMORY_CUE_SAME_PLACE_MS: u32 = 50;
const RELATED_TABLES: [&str; 5] = ["analyses", "artwork", "analysis_failures", "playlist_tracks", "stick_tracks"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub keeper_id: i64,
    pub remove_ids: Vec<i64>,
    /// Compare the files byte for byte first and refuse the merge when they differ.
    #[serde(default)]
    pub require_identical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeOutcome {
    pub merge_id: i64,
    pub keeper_id: i64,
    pub removed_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MergeFailure {
    pub keeper_id: i64,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MergeReport {
    pub merged: Vec<MergeOutcome>,
    pub failed: Vec<MergeFailure>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArchivedTrack {
    pub id: i64,
    pub artist: String,
    pub title: String,
    pub path: String,
    pub format: String,
    pub lossless: bool,
    pub bitrate: u32,
    pub file_present: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArchivedMerge {
    pub merge_id: i64,
    pub merged_at: String,
    pub confidence: String,
    pub keeper_id: i64,
    /// "Artist - Title" of the keeper while it is still in the library.
    pub keeper_label: Option<String>,
    pub tracks: Vec<ArchivedTrack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RestoreOutcome {
    pub merge_id: i64,
    pub restored_ids: Vec<i64>,
    /// The keeper got its cues, rating, colour and comment from before the merge back. False when the keeper changed
    /// since (it keeps what it merged).
    pub keeper_reverted: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Snapshot {
    track: Value,
    related: BTreeMap<String, Vec<Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlaylistAdd {
    playlist_id: i64,
    position: i64,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Result<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2).unwrap_or(""), 16).context("Archived blob is not hex"))
        .collect()
}

fn json_of(value: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => serde_json::json!({ "blob": hex(b) }),
    }
}

fn sql_of(value: &Value) -> Result<rusqlite::types::Value> {
    use rusqlite::types::Value as Sql;
    Ok(match value {
        Value::Null => Sql::Null,
        Value::Number(n) => match n.as_i64() {
            Some(i) => Sql::Integer(i),
            None => Sql::Real(n.as_f64().context("Archived number is out of range")?),
        },
        Value::String(s) => Sql::Text(s.clone()),
        Value::Object(o) => Sql::Blob(unhex(o.get("blob").and_then(Value::as_str).context("Archived value has no blob")?)?),
        _ => bail!("Archived value has an unexpected type"),
    })
}

fn dump_rows(conn: &Connection, table: &str, column: &str, id: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(&format!("SELECT * FROM {table} WHERE {column} = ?1"))?;
    let names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
    let mut rows = stmt.query([id])?;
    let mut dumped = Vec::new();
    while let Some(row) = rows.next()? {
        let mut object = Map::new();
        for (index, name) in names.iter().enumerate() {
            object.insert(name.clone(), json_of(row.get_ref(index)?));
        }
        dumped.push(Value::Object(object));
    }
    Ok(dumped)
}

fn insert_dumped(conn: &Connection, table: &str, row: &Value) -> Result<()> {
    let object = row.as_object().context("Archived row is not an object")?;
    let columns: Vec<&str> = object.keys().map(String::as_str).collect();
    let placeholders: Vec<String> = (1..=columns.len()).map(|n| format!("?{n}")).collect();
    let values = object.values().map(sql_of).collect::<Result<Vec<_>>>()?;
    conn.execute(
        &format!("INSERT INTO {table} ({}) VALUES ({})", columns.join(", "), placeholders.join(", ")),
        rusqlite::params_from_iter(values),
    )?;
    Ok(())
}

fn text_of<'a>(object: &'a Value, column: &str) -> &'a str {
    object.get(column).and_then(Value::as_str).unwrap_or("")
}

fn int_of(object: &Value, column: &str) -> i64 {
    object.get(column).and_then(Value::as_i64).unwrap_or(0)
}

fn regular_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())).map(|e| e.path()).collect();
    files.sort();
    files
}

fn dir_digest(dir: &Path) -> String {
    let mut hasher = std::hash::DefaultHasher::new();
    for file in regular_files(dir) {
        hasher.write(file.file_name().unwrap_or_default().as_encoded_bytes());
        hasher.write(&std::fs::read(&file).unwrap_or_default());
    }
    format!("{:016x}", hasher.finish())
}

fn copy_files(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("Failed to create {}", to.display()))?;
    for file in regular_files(from) {
        let target = to.join(file.file_name().unwrap_or_default());
        std::fs::copy(&file, &target).with_context(|| format!("Failed to copy {}", file.display()))?;
    }
    Ok(())
}

fn replace_files(dir: &Path, with: &Path) -> Result<()> {
    for file in regular_files(dir) {
        std::fs::remove_file(&file).with_context(|| format!("Failed to remove {}", file.display()))?;
    }
    copy_files(with, dir)
}

fn move_dir(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::rename(from, to).with_context(|| format!("Failed to move {} to {}", from.display(), to.display()))
}

fn exists_in(conn: &Connection, sql: &str, args: impl rusqlite::Params) -> Result<bool> {
    Ok(conn.query_row(sql, args, |_| Ok(())).optional()?.is_some())
}

struct KeeperChanges {
    comment: Option<String>,
    rating: Option<u8>,
    color: Option<(String, ImportedMeta)>,
    cues: Option<Vec<ImportedCue>>,
}

impl KeeperChanges {
    fn touches_files(&self) -> bool {
        self.rating.is_some() || self.color.is_some() || self.cues.is_some()
    }
}

#[derive(Default)]
struct Undo {
    moved_dirs: Vec<(PathBuf, PathBuf)>,
    keeper_dir: Option<(PathBuf, PathBuf)>,
}

impl LocalLibrary {
    fn archive_root(&self) -> Option<PathBuf> {
        self.anlz_dir().map(|dir| dir.join(ARCHIVE_DIR))
    }

    fn removed_track_archive_dir(&self, track_id: i64) -> Option<PathBuf> {
        self.archive_root().map(|root| root.join(track_id.to_string()))
    }

    fn keeper_backup_dir(&self, merge_id: i64) -> Option<PathBuf> {
        self.archive_root().map(|root| root.join(format!("keeper-{merge_id}")))
    }

    fn track_path(&self, track_id: i64) -> Result<PathBuf> {
        let track = queries::select_track(&self.conn, track_id)?.with_context(|| format!("Track {track_id} is not in the library"))?;
        Ok(track.source_path)
    }

    /// Moves the playlist places, cues, rating, colour and comment of `remove_ids` onto `keeper_id` and takes the others
    /// out of the library into the archive. One failed merge does not stop the others.
    pub fn apply_duplicate_merges(&self, requests: &[MergeRequest]) -> MergeReport {
        let mut report = MergeReport::default();
        for request in requests {
            match self.merge_into_keeper(request) {
                Ok(outcome) => report.merged.push(outcome),
                Err(error) => report.failed.push(MergeFailure { keeper_id: request.keeper_id, message: format!("{error:#}") }),
            }
        }
        report
    }

    fn plan_keeper_changes(&self, keeper_id: i64, remove_ids: &[i64]) -> Result<KeeperChanges> {
        let keeper = queries::select_track(&self.conn, keeper_id)?.context("The keeper is not in the library")?;
        let keeper_meta = self.imported_meta(keeper_id);
        let comment = (keeper.comment.trim().is_empty())
            .then(|| {
                remove_ids
                    .iter()
                    .filter_map(|id| queries::select_track(&self.conn, *id).ok().flatten())
                    .map(|t| t.comment)
                    .find(|c| !c.trim().is_empty())
            })
            .flatten();

        let keeper_rating = self.effective_rating(keeper_id, keeper_meta.as_ref().and_then(|m| m.rating));
        let rating = if keeper_rating.is_some() {
            None
        } else {
            remove_ids.iter().find_map(|id| self.effective_rating(*id, self.imported_meta(*id).and_then(|m| m.rating)))
        };

        let color = if keeper_meta.as_ref().is_some_and(|m| m.color.is_some()) {
            None
        } else {
            remove_ids.iter().find_map(|id| {
                let donor = self.imported_meta(*id)?;
                let color = donor.color.clone()?;
                let mut meta = keeper_meta.clone().unwrap_or(ImportedMeta {
                    source: donor.source,
                    source_id: donor.source_id.clone(),
                    grid_locked: false,
                    cues_locked: false,
                    rating: None,
                    color: None,
                    cues: Vec::new(),
                    grid: Vec::new(),
                });
                meta.color = Some(color.clone());
                Some((color, meta))
            })
        };

        let keeper_cues = self.chosen_cues(keeper_id);
        let mut merged = keeper_cues.clone().unwrap_or_default();
        let kept = merged.len();
        for id in remove_ids {
            for cue in self.chosen_cues(*id).unwrap_or_default() {
                let taken = if cue.hot_cue_number > 0 {
                    merged.iter().any(|c| c.hot_cue_number == cue.hot_cue_number)
                } else {
                    merged.iter().any(|c| c.hot_cue_number == 0 && c.time_ms.abs_diff(cue.time_ms) <= MEMORY_CUE_SAME_PLACE_MS)
                };
                if !taken {
                    merged.push(cue);
                }
            }
        }
        Ok(KeeperChanges { comment, rating, color, cues: (merged.len() > kept).then_some(merged) })
    }

    fn merge_into_keeper(&self, request: &MergeRequest) -> Result<MergeOutcome> {
        let keeper_id = request.keeper_id;
        let remove_ids: Vec<i64> = request.remove_ids.iter().copied().collect::<BTreeSet<_>>().into_iter().collect();
        if remove_ids.is_empty() {
            bail!("Nothing to merge into the keeper");
        }
        if remove_ids.contains(&keeper_id) {
            bail!("The keeper cannot be merged into itself");
        }
        let keeper_path = self.track_path(keeper_id)?;
        let mut snapshots = Vec::with_capacity(remove_ids.len());
        for id in &remove_ids {
            let path = self.track_path(*id)?;
            if request.require_identical && !files_identical(&keeper_path, &path) {
                bail!("{} is not an identical copy of {} (or one of them cannot be read); nothing was merged", path.display(), keeper_path.display());
            }
            let track = dump_rows(&self.conn, "tracks", "id", *id)?.into_iter().next().context("Track row is gone")?;
            let related = RELATED_TABLES
                .iter()
                .map(|table| Ok((table.to_string(), dump_rows(&self.conn, table, "track_id", *id)?)))
                .collect::<Result<BTreeMap<_, _>>>()?;
            snapshots.push((*id, Snapshot { track, related }));
        }
        let changes = self.plan_keeper_changes(keeper_id, &remove_ids)?;
        let merge_id: i64 = self.conn.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM duplicate_merges", [], |row| row.get(0))?;

        let mut undo = Undo::default();
        match self.merge_prepared(request, merge_id, &remove_ids, &snapshots, &changes, &mut undo) {
            Ok(()) => {
                self.forget_duplicates();
                let reader = self.track_labels_reader();
                for id in remove_ids.iter().chain([&keeper_id]) {
                    reader.update(*id);
                }
                Ok(MergeOutcome { merge_id, keeper_id, removed_ids: remove_ids })
            }
            Err(error) => {
                for (from, to) in undo.moved_dirs.iter().rev() {
                    let _ = std::fs::rename(to, from);
                }
                if let Some((dir, backup)) = &undo.keeper_dir {
                    let _ = replace_files(dir, backup);
                    let _ = std::fs::remove_dir_all(backup);
                }
                Err(error)
            }
        }
    }

    fn merge_prepared(
        &self,
        request: &MergeRequest,
        merge_id: i64,
        remove_ids: &[i64],
        snapshots: &[(i64, Snapshot)],
        changes: &KeeperChanges,
        undo: &mut Undo,
    ) -> Result<()> {
        let keeper_id = request.keeper_id;
        let mut keeper_digest = None;
        if changes.touches_files() {
            let dir = self.track_anlz_dir(keeper_id).context("This library keeps no files, so cues cannot be merged")?;
            let backup = self.keeper_backup_dir(merge_id).context("This library keeps no files")?;
            if backup.exists() {
                bail!("{} is already there", backup.display());
            }
            copy_files(&dir, &backup)?;
            undo.keeper_dir = Some((dir.clone(), backup));
            if let Some(rating) = changes.rating {
                self.set_user_rating(keeper_id, rating)?;
            }
            if let Some((_, meta)) = &changes.color {
                meta.write(&dir)?;
            }
            if let Some(cues) = &changes.cues {
                self.set_cues(keeper_id, cues)?;
            }
            keeper_digest = Some(dir_digest(&dir));
        }

        for (id, _) in snapshots {
            let (Some(dir), Some(archived)) = (self.track_anlz_dir(*id), self.removed_track_archive_dir(*id)) else { continue };
            if !dir.exists() {
                continue;
            }
            if archived.exists() {
                bail!("{} is already there", archived.display());
            }
            move_dir(&dir, &archived)?;
            undo.moved_dirs.push((dir, archived));
        }

        let tx = self.conn.unchecked_transaction()?;
        let comment_before = changes.comment.as_ref().map(|_| queries::select_track(&tx, keeper_id)).transpose()?.flatten().map(|t| t.comment);
        let confidence = if request.require_identical { "exact_audio" } else { "metadata_only" };
        let adds = self.move_playlist_places(&tx, keeper_id, remove_ids)?;
        tx.execute(
            "INSERT INTO duplicate_merges (id, keeper_id, confidence, merged_at, comment_before, comment_after, keeper_digest, playlist_adds)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                merge_id,
                keeper_id,
                confidence,
                chrono::Utc::now().to_rfc3339(),
                comment_before,
                changes.comment,
                keeper_digest,
                serde_json::to_string(&adds)?,
            ],
        )?;
        for (id, snapshot) in snapshots {
            let cover_hash = snapshot.related.get("artwork").and_then(|rows| rows.first()).and_then(|row| row.get("cover_hash")).and_then(Value::as_str).map(String::from);
            tx.execute(
                "INSERT INTO archived_tracks (track_id, merge_id, snapshot, cover_hash) VALUES (?1, ?2, ?3, ?4)",
                params![id, merge_id, serde_json::to_string(snapshot)?, cover_hash],
            )?;
        }
        if let Some(comment) = &changes.comment {
            tx.execute("UPDATE tracks SET comment = ?1 WHERE id = ?2", params![comment, keeper_id])?;
        }
        for id in remove_ids {
            tx.execute("DELETE FROM tracks WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn move_playlist_places(&self, conn: &Connection, keeper_id: i64, remove_ids: &[i64]) -> Result<Vec<PlaylistAdd>> {
        let mut first_place: BTreeMap<i64, i64> = BTreeMap::new();
        let mut stmt = conn.prepare("SELECT playlist_id, position FROM playlist_tracks WHERE track_id = ?1")?;
        for id in remove_ids {
            for place in stmt.query_map([id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))? {
                let (playlist_id, position) = place?;
                let entry = first_place.entry(playlist_id).or_insert(position);
                *entry = (*entry).min(position);
            }
        }
        let mut adds = Vec::new();
        for (playlist_id, position) in first_place {
            if exists_in(conn, "SELECT 1 FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2", params![playlist_id, keeper_id])? {
                continue;
            }
            conn.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist_id, keeper_id, position],
            )?;
            adds.push(PlaylistAdd { playlist_id, position });
        }
        Ok(adds)
    }

    /// Everything merged away and not restored yet, newest merge first.
    pub fn list_archive(&self) -> Result<Vec<ArchivedMerge>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.merged_at, m.confidence, m.keeper_id, a.snapshot
             FROM duplicate_merges m JOIN archived_tracks a ON a.merge_id = m.id
             ORDER BY m.id DESC, a.track_id",
        )?;
        let mut rows = stmt.query([])?;
        let mut merges: Vec<ArchivedMerge> = Vec::new();
        while let Some(row) = rows.next()? {
            let merge_id: i64 = row.get(0)?;
            let snapshot: Snapshot = serde_json::from_str(&row.get::<_, String>(4)?).context("Archived snapshot is unreadable")?;
            let path = PathBuf::from(text_of(&snapshot.track, "source_path"));
            let track = ArchivedTrack {
                id: int_of(&snapshot.track, "id"),
                artist: text_of(&snapshot.track, "artist").into(),
                title: text_of(&snapshot.track, "title").into(),
                format: path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default(),
                lossless: is_lossless(&path),
                bitrate: int_of(&snapshot.track, "bitrate") as u32,
                file_present: file_status(&path) == FileStatus::Ok,
                path: path.to_string_lossy().into_owned(),
            };
            match merges.last_mut().filter(|m| m.merge_id == merge_id) {
                Some(merge) => merge.tracks.push(track),
                None => {
                    let keeper_id: i64 = row.get(3)?;
                    let keeper_label = queries::select_track(&self.conn, keeper_id)?.map(|t| format!("{} - {}", t.artist, t.title));
                    merges.push(ArchivedMerge { merge_id, merged_at: row.get(1)?, confidence: row.get(2)?, keeper_id, keeper_label, tracks: vec![track] });
                }
            }
        }
        Ok(merges)
    }

    /// Puts every entry of one merge back with its id, rows, playlist places and cues. Fails, changing nothing, when an
    /// id or a file path is taken again.
    pub fn restore_merge(&self, merge_id: i64) -> Result<RestoreOutcome> {
        let (keeper_id, comment_before, comment_after, digest, adds_json): (i64, Option<String>, Option<String>, Option<String>, String) = self
            .conn
            .query_row(
                "SELECT keeper_id, comment_before, comment_after, keeper_digest, playlist_adds FROM duplicate_merges WHERE id = ?1",
                [merge_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?
            .with_context(|| format!("Merge {merge_id} is not in the archive"))?;
        let adds: Vec<PlaylistAdd> = serde_json::from_str(&adds_json).context("Archived playlist places are unreadable")?;
        let mut stmt = self.conn.prepare("SELECT track_id, snapshot FROM archived_tracks WHERE merge_id = ?1 ORDER BY track_id")?;
        let archived: Vec<(i64, Snapshot)> = stmt
            .query_map([merge_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
            .map(|r| {
                let (id, text) = r?;
                Ok((id, serde_json::from_str(&text).context("Archived snapshot is unreadable")?))
            })
            .collect::<Result<_>>()?;
        drop(stmt);

        let taken: HashSet<PathBuf> = self.track_paths()?;
        for (id, snapshot) in &archived {
            if queries::select_track(&self.conn, *id)?.is_some() {
                bail!("Track id {id} is used by another track now; nothing was restored");
            }
            let path = PathBuf::from(text_of(&snapshot.track, "source_path"));
            if taken.contains(&path) {
                bail!("{} is in the library again; nothing was restored", path.display());
            }
            if let (Some(dir), Some(archived_dir)) = (self.track_anlz_dir(*id), self.removed_track_archive_dir(*id)) {
                if archived_dir.exists() && dir.exists() {
                    bail!("{} already exists; nothing was restored", dir.display());
                }
            }
        }

        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        let result = (|| -> Result<()> {
            for (id, _) in &archived {
                let (Some(dir), Some(archived_dir)) = (self.track_anlz_dir(*id), self.removed_track_archive_dir(*id)) else { continue };
                if archived_dir.exists() {
                    move_dir(&archived_dir, &dir)?;
                    moved.push((archived_dir, dir));
                }
            }
            let tx = self.conn.unchecked_transaction()?;
            for (_, snapshot) in &archived {
                insert_dumped(&tx, "tracks", &snapshot.track)?;
                for table in RELATED_TABLES {
                    for row in snapshot.related.get(table).into_iter().flatten() {
                        let parent_gone = match table {
                            "playlist_tracks" => !exists_in(&tx, "SELECT 1 FROM playlists WHERE id = ?1", [int_of(row, "playlist_id")])?,
                            "stick_tracks" => !exists_in(&tx, "SELECT 1 FROM sticks WHERE id = ?1", [text_of(row, "stick_id")])?,
                            _ => false,
                        };
                        if !parent_gone {
                            insert_dumped(&tx, table, row)?;
                        }
                    }
                }
            }
            for add in &adds {
                tx.execute(
                    "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2 AND position = ?3",
                    params![add.playlist_id, keeper_id, add.position],
                )?;
            }
            tx.execute("DELETE FROM duplicate_merges WHERE id = ?1", [merge_id])?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = result {
            for (from, to) in moved.iter().rev() {
                let _ = std::fs::rename(to, from);
            }
            return Err(error);
        }

        let keeper_reverted = self.revert_keeper(merge_id, keeper_id, comment_before, comment_after, digest)?;
        self.forget_duplicates();
        let reader = self.track_labels_reader();
        let restored_ids: Vec<i64> = archived.iter().map(|(id, _)| *id).collect();
        for id in restored_ids.iter().chain([&keeper_id]) {
            reader.update(*id);
        }
        Ok(RestoreOutcome { merge_id, restored_ids, keeper_reverted })
    }

    fn revert_keeper(&self, merge_id: i64, keeper_id: i64, comment_before: Option<String>, comment_after: Option<String>, digest: Option<String>) -> Result<bool> {
        let Some(keeper) = queries::select_track(&self.conn, keeper_id)? else { return Ok(false) };
        let mut reverted = false;
        if let (Some(before), Some(after)) = (comment_before, comment_after) {
            if keeper.comment == after {
                self.conn.execute("UPDATE tracks SET comment = ?1 WHERE id = ?2", params![before, keeper_id])?;
                reverted = true;
            }
        }
        if let (Some(backup), Some(dir)) = (self.keeper_backup_dir(merge_id), self.track_anlz_dir(keeper_id)) {
            if backup.exists() {
                if digest.as_deref() == Some(dir_digest(&dir).as_str()) {
                    replace_files(&dir, &backup)?;
                    reverted = true;
                }
                std::fs::remove_dir_all(&backup).with_context(|| format!("Failed to remove {}", backup.display()))?;
            }
        }
        Ok(reverted)
    }
}
