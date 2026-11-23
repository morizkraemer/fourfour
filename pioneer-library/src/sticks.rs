//! Stick identity, the fourfour manifest on the stick, and what the library
//! remembers per stick ("All tracks" and the playlists put on it).
//!
//! The id lives in `PIONEER/fourfour-manifest.json` on the stick, so it follows
//! the stick across remounts, mount-point changes and renames. The manifest also
//! lists every file fourfour wrote; sync deletes only those.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pioneer_usb_writer::models::SyncReport;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::LocalLibrary;

pub const MANIFEST_REL_PATH: &str = "PIONEER/fourfour-manifest.json";
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedFile {
    pub usb_path: String,
    pub track_id: i64,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastSync {
    pub at: String,
    pub report: SyncReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickManifest {
    pub version: u32,
    pub stick_id: String,
    #[serde(default)]
    pub files: Vec<OwnedFile>,
    #[serde(default)]
    pub last_sync: Option<LastSync>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v2: Option<pioneer_usb_writer::stick::plan::Ownership>,
}

impl StickManifest {
    fn fresh() -> Self {
        Self { version: MANIFEST_VERSION, stick_id: new_stick_id(), files: Vec::new(), last_sync: None, v2: None }
    }

    pub fn owns(&self, usb_path: &str) -> bool {
        self.files.iter().any(|f| f.usb_path == usb_path)
    }
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_REL_PATH)
}

/// The manifest on the stick, or `None` when fourfour never wrote to it.
pub fn read_manifest(root: &Path) -> Result<Option<StickManifest>> {
    let path = manifest_path(root);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .with_context(|| format!("Cannot read {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("Cannot read {}", path.display())),
    }
}

pub fn write_manifest(root: &Path, manifest: &StickManifest) -> Result<()> {
    let path = manifest_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(manifest)?)
        .with_context(|| format!("Cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, &path).with_context(|| format!("Cannot write {}", path.display()))
}

/// The stick id, without writing anything.
pub fn read_stick_id(root: &Path) -> Result<Option<String>> {
    Ok(read_manifest(root)?.map(|m| m.stick_id))
}

/// The stick id; a stick without a manifest gets one holding a new id.
pub fn ensure_stick_id(root: &Path) -> Result<String> {
    if let Some(manifest) = read_manifest(root)? {
        return Ok(manifest.stick_id);
    }
    let manifest = StickManifest::fresh();
    write_manifest(root, &manifest)?;
    Ok(manifest.stick_id)
}

pub(crate) fn manifest_or_fresh(root: &Path) -> Result<StickManifest> {
    Ok(read_manifest(root)?.unwrap_or_else(StickManifest::fresh))
}

fn new_stick_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut halves = [0u64; 2];
    for (salt, half) in halves.iter_mut().enumerate() {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(nanos);
        hasher.write_u32(std::process::id());
        hasher.write_usize(salt);
        *half = hasher.finish();
    }
    format!("{:016x}{:016x}", halves[0], halves[1])
}

/// A playlist on a stick that no longer follows the library: the stick's own track list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptPlaylist {
    pub playlist_id: i64,
    pub as_of: String,
    pub track_ids: Vec<i64>,
}

/// What the library remembers for one stick.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StickContents {
    pub stick_id: Option<String>,
    pub track_ids: Vec<i64>,
    pub playlist_ids: Vec<i64>,
    pub kept_as_is: Vec<KeptPlaylist>,
}

impl LocalLibrary {
    pub fn remember_stick(&self, stick_id: &str, name: &str, path: &Path) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sticks (id, name, last_path, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = ?2, last_path = ?3, updated_at = ?4",
            params![stick_id, name, path.to_string_lossy(), chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn stick_contents(&self, stick_id: &str) -> Result<StickContents> {
        let ids = |sql: &str| -> Result<Vec<i64>> {
            let mut stmt = self.conn.prepare(sql)?;
            let rows = stmt.query_map([stick_id], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<Vec<i64>>>()?)
        };
        Ok(StickContents {
            stick_id: Some(stick_id.to_string()),
            track_ids: ids("SELECT track_id FROM stick_tracks WHERE stick_id = ?1 ORDER BY position")?,
            playlist_ids: ids(
                "SELECT playlist_id FROM stick_playlists WHERE stick_id = ?1 ORDER BY position",
            )?,
            kept_as_is: self.kept_stick_playlists(stick_id)?,
        })
    }

    fn kept_stick_playlists(&self, stick_id: &str) -> Result<Vec<KeptPlaylist>> {
        let mut stmt = self.conn.prepare(
            "SELECT k.playlist_id, k.as_of, k.track_ids FROM stick_kept_playlists k
             JOIN stick_playlists p ON p.stick_id = k.stick_id AND p.playlist_id = k.playlist_id
             WHERE k.stick_id = ?1 ORDER BY p.position",
        )?;
        let rows = stmt.query_map([stick_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        let mut kept = Vec::new();
        for row in rows {
            let (playlist_id, as_of, track_ids) = row?;
            let track_ids = serde_json::from_str(&track_ids)
                .with_context(|| format!("Cannot read the kept track list of playlist {playlist_id}"))?;
            kept.push(KeptPlaylist { playlist_id, as_of, track_ids });
        }
        Ok(kept)
    }

    /// Gives a playlist on the stick its own track list, so it stops following the library.
    /// The date it was first kept stays when the list is edited later. Returns 1 when the
    /// stored list changed, 0 when the playlist is not on the stick or nothing changed.
    pub fn keep_stick_playlist(&self, stick_id: &str, playlist_id: i64, track_ids: &[i64]) -> Result<usize> {
        let on_stick: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM stick_playlists WHERE stick_id = ?1 AND playlist_id = ?2)",
            params![stick_id, playlist_id],
            |row| row.get(0),
        )?;
        if !on_stick {
            return Ok(0);
        }
        let list = serde_json::to_string(track_ids)?;
        Ok(self.conn.execute(
            "INSERT INTO stick_kept_playlists (stick_id, playlist_id, as_of, track_ids) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(stick_id, playlist_id) DO UPDATE SET track_ids = ?4 WHERE track_ids <> ?4",
            params![stick_id, playlist_id, chrono::Utc::now().to_rfc3339(), list],
        )?)
    }

    /// Makes a playlist on the stick follow the library again.
    pub fn follow_stick_playlist(&self, stick_id: &str, playlist_id: i64) -> Result<usize> {
        Ok(self.conn.execute(
            "DELETE FROM stick_kept_playlists WHERE stick_id = ?1 AND playlist_id = ?2",
            params![stick_id, playlist_id],
        )?)
    }

    /// Appends the tracks the stick does not hold yet; returns how many were added.
    pub fn add_stick_tracks(&self, stick_id: &str, track_ids: &[i64]) -> Result<usize> {
        self.append_ids("stick_tracks", "track_id", stick_id, track_ids)
    }

    pub fn remove_stick_tracks(&self, stick_id: &str, track_ids: &[i64]) -> Result<usize> {
        self.delete_ids("stick_tracks", "track_id", stick_id, track_ids)
    }

    /// Puts playlists on the stick; returns how many were new.
    pub fn add_stick_playlists(&self, stick_id: &str, playlist_ids: &[i64]) -> Result<usize> {
        self.append_ids("stick_playlists", "playlist_id", stick_id, playlist_ids)
    }

    pub fn remove_stick_playlists(&self, stick_id: &str, playlist_ids: &[i64]) -> Result<usize> {
        self.delete_ids("stick_kept_playlists", "playlist_id", stick_id, playlist_ids)?;
        self.delete_ids("stick_playlists", "playlist_id", stick_id, playlist_ids)
    }

    fn append_ids(&self, table: &str, column: &str, stick_id: &str, ids: &[i64]) -> Result<usize> {
        self.conn.execute(
            "INSERT OR IGNORE INTO sticks (id, updated_at) VALUES (?1, ?2)",
            params![stick_id, chrono::Utc::now().to_rfc3339()],
        )?;
        let tx = self.conn.unchecked_transaction()?;
        let mut next: i64 = tx.query_row(
            &format!("SELECT COALESCE(MAX(position) + 1, 0) FROM {table} WHERE stick_id = ?1"),
            [stick_id],
            |row| row.get(0),
        )?;
        let mut added = 0;
        for id in ids {
            let inserted = tx.execute(
                &format!("INSERT OR IGNORE INTO {table} (stick_id, {column}, position) VALUES (?1, ?2, ?3)"),
                params![stick_id, id, next],
            )?;
            if inserted > 0 {
                added += 1;
                next += 1;
            }
        }
        tx.commit()?;
        Ok(added)
    }

    fn delete_ids(&self, table: &str, column: &str, stick_id: &str, ids: &[i64]) -> Result<usize> {
        let mut removed = 0;
        for id in ids {
            removed += self.conn.execute(
                &format!("DELETE FROM {table} WHERE stick_id = ?1 AND {column} = ?2"),
                params![stick_id, id],
            )?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_created_once_and_survives_a_move() {
        let dir = tempfile::tempdir().unwrap();
        let stick = dir.path().join("STICK");
        std::fs::create_dir_all(&stick).unwrap();
        assert_eq!(read_stick_id(&stick).unwrap(), None);
        let id = ensure_stick_id(&stick).unwrap();
        assert_eq!(id.len(), 32);
        assert_eq!(ensure_stick_id(&stick).unwrap(), id);

        let renamed = dir.path().join("RENAMED");
        std::fs::rename(&stick, &renamed).unwrap();
        assert_eq!(read_stick_id(&renamed).unwrap(), Some(id));
    }

    #[test]
    fn two_sticks_get_different_ids() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("A");
        let b = dir.path().join("B");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        assert_ne!(ensure_stick_id(&a).unwrap(), ensure_stick_id(&b).unwrap());
    }

    #[test]
    fn contents_append_without_duplicates_and_keep_order() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let track = |name: &str| lib.add_track(&crate::tests::make_track(name, &format!("/music/{name}.mp3"))).unwrap();
        let (a, b, c) = (track("a"), track("b"), track("c"));
        let warmup = lib.create_playlist("Warmup").unwrap();

        assert_eq!(lib.add_stick_tracks("s1", &[b, a]).unwrap(), 2);
        assert_eq!(lib.add_stick_tracks("s1", &[a, c]).unwrap(), 1);
        assert_eq!(lib.add_stick_playlists("s1", &[warmup, warmup]).unwrap(), 1);
        let contents = lib.stick_contents("s1").unwrap();
        assert_eq!(contents.track_ids, vec![b, a, c]);
        assert_eq!(contents.playlist_ids, vec![warmup]);
        assert!(lib.stick_contents("other").unwrap().track_ids.is_empty());

        assert_eq!(lib.remove_stick_tracks("s1", &[a]).unwrap(), 1);
        assert_eq!(lib.remove_stick_playlists("s1", &[warmup]).unwrap(), 1);
        let contents = lib.stick_contents("s1").unwrap();
        assert_eq!(contents.track_ids, vec![b, c]);
        assert!(contents.playlist_ids.is_empty());

        lib.remove_track(c).unwrap();
        assert_eq!(lib.stick_contents("s1").unwrap().track_ids, vec![b]);
    }

    #[test]
    fn kept_playlist_holds_its_own_list_until_it_follows_again() {
        let lib = LocalLibrary::open_in_memory().unwrap();
        let track = |name: &str| lib.add_track(&crate::tests::make_track(name, &format!("/music/{name}.mp3"))).unwrap();
        let (a, b, c) = (track("a"), track("b"), track("c"));
        let warmup = lib.create_playlist("Warmup").unwrap();
        lib.set_playlist_tracks(warmup, &[a, b, c]).unwrap();

        assert_eq!(lib.keep_stick_playlist("s1", warmup, &[a, b, c]).unwrap(), 0, "not on the stick yet");
        lib.add_stick_playlists("s1", &[warmup]).unwrap();
        assert!(lib.stick_contents("s1").unwrap().kept_as_is.is_empty());

        assert_eq!(lib.keep_stick_playlist("s1", warmup, &[a, b, c]).unwrap(), 1);
        let kept = lib.stick_contents("s1").unwrap().kept_as_is;
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].playlist_id, kept[0].track_ids.clone()), (warmup, vec![a, b, c]));
        let as_of = kept[0].as_of.clone();

        lib.set_playlist_tracks(warmup, &[a]).unwrap();
        assert_eq!(lib.stick_contents("s1").unwrap().kept_as_is[0].track_ids, vec![a, b, c], "library edits do not reach it");

        assert_eq!(lib.keep_stick_playlist("s1", warmup, &[c, a]).unwrap(), 1);
        assert_eq!(lib.keep_stick_playlist("s1", warmup, &[c, a]).unwrap(), 0);
        let edited = lib.stick_contents("s1").unwrap().kept_as_is;
        assert_eq!(edited[0].track_ids, vec![c, a]);
        assert_eq!(edited[0].as_of, as_of);
        assert!(lib.stick_contents("other").unwrap().kept_as_is.is_empty());

        assert_eq!(lib.follow_stick_playlist("s1", warmup).unwrap(), 1);
        assert!(lib.stick_contents("s1").unwrap().kept_as_is.is_empty());

        lib.keep_stick_playlist("s1", warmup, &[b]).unwrap();
        lib.remove_stick_playlists("s1", &[warmup]).unwrap();
        lib.add_stick_playlists("s1", &[warmup]).unwrap();
        assert!(lib.stick_contents("s1").unwrap().kept_as_is.is_empty(), "taking it off the stick forgets the kept list");
    }
}
