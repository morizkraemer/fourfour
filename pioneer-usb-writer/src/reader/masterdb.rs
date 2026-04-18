//! Reader for Rekordbox's local `master.db` collection database.
//!
//! Reads the SQLCipher-encrypted SQLite database that Rekordbox maintains
//! locally on macOS/Windows. Returns tracks, playlists, and cue points as
//! standard library types, ready to be passed to the writer or stored in
//! a [`LocalLibrary`](pioneer_library::LocalLibrary).
//!
//! # Path
//! - macOS: `~/Library/Pioneer/rekordbox/master.db`
//! - Windows: `%APPDATA%\Pioneer\rekordbox\master.db`

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::{CuePoint, Playlist, Track};

/// SQLCipher key for Rekordbox's local `master.db`.
const MASTER_DB_KEY: &str =
    "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";

/// Everything read from a single `master.db`.
pub struct MasterDbImport {
    /// All non-deleted tracks, with metadata joined from lookup tables.
    pub tracks: Vec<Track>,
    /// Cue points per track, in the same order as `tracks`.
    pub cue_points: Vec<Vec<CuePoint>>,
    /// All leaf playlists (not folder nodes), with track IDs remapped to
    /// the sequential 1-based IDs assigned to `tracks`.
    pub playlists: Vec<Playlist>,
    /// Absolute paths to artwork files, one per track (same order as `tracks`).
    /// `None` if the track has no artwork or the path is empty.
    pub artwork_paths: Vec<Option<PathBuf>>,
}

/// Read the Rekordbox local collection from `master.db` at `db_path`.
///
/// Returns tracks, cue points, playlists, and artwork paths.
/// Track IDs are remapped to sequential 1-based integers regardless of the
/// VARCHAR IDs stored in master.db.
pub fn read_masterdb(db_path: &Path) -> Result<MasterDbImport> {
    let conn = open_db(db_path)?;

    // ── Lookup tables ────────────────────────────────────────────────

    let artists = read_name_table(&conn, "djmdArtist")?;
    let albums = read_name_table(&conn, "djmdAlbum")?;
    let genres = read_name_table(&conn, "djmdGenre")?;
    let labels = read_name_table(&conn, "djmdLabel")?;

    // Keys use ScaleName (Camelot notation: "1A", "5B", etc.)
    let keys = read_key_table(&conn)?;

    // ── Tracks ───────────────────────────────────────────────────────

    // Map VARCHAR content IDs → sequential 1-based u32 IDs
    let mut id_map: HashMap<String, u32> = HashMap::new();
    let mut tracks: Vec<Track> = Vec::new();
    let mut artwork_paths: Vec<Option<PathBuf>> = Vec::new();

    {
        let mut stmt = conn.prepare(
            "SELECT ID, FolderPath, Title, ArtistID, AlbumID, GenreID,
                    LabelID, RemixerID, KeyID, Commnt, ReleaseYear,
                    DiscNo, TrackNo, BPM, Length, SampleRate, BitRate,
                    FileSize, ImagePath, DateCreated
             FROM djmdContent
             WHERE (rb_local_deleted = 0 OR rb_local_deleted IS NULL)
             ORDER BY ID",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,                           // ID
                row.get::<_, String>(1)?,                           // FolderPath
                row.get::<_, Option<String>>(2)?.unwrap_or_default(), // Title
                row.get::<_, Option<String>>(3)?.unwrap_or_default(), // ArtistID
                row.get::<_, Option<String>>(4)?.unwrap_or_default(), // AlbumID
                row.get::<_, Option<String>>(5)?.unwrap_or_default(), // GenreID
                row.get::<_, Option<String>>(6)?.unwrap_or_default(), // LabelID
                row.get::<_, Option<String>>(7)?.unwrap_or_default(), // RemixerID
                row.get::<_, Option<String>>(8)?.unwrap_or_default(), // KeyID
                row.get::<_, Option<String>>(9)?.unwrap_or_default(), // Commnt
                row.get::<_, Option<i32>>(10)?.unwrap_or(0),       // ReleaseYear
                row.get::<_, Option<i32>>(11)?.unwrap_or(0),       // DiscNo
                row.get::<_, Option<i32>>(12)?.unwrap_or(0),       // TrackNo
                row.get::<_, Option<i32>>(13)?.unwrap_or(0),       // BPM (×100)
                row.get::<_, Option<i32>>(14)?.unwrap_or(0),       // Length (secs)
                row.get::<_, Option<i32>>(15)?.unwrap_or(0),       // SampleRate
                row.get::<_, Option<i32>>(16)?.unwrap_or(0),       // BitRate
                row.get::<_, Option<i64>>(17)?.unwrap_or(0),       // FileSize
                row.get::<_, Option<String>>(18)?,                  // ImagePath
                row.get::<_, Option<String>>(19)?.unwrap_or_default(), // DateCreated
            ))
        })?;

        for (seq, row) in rows.enumerate() {
            let (
                content_id, folder_path, title, artist_id, album_id, genre_id,
                label_id, remixer_id, key_id, comment, year, disc_no, track_no,
                bpm_x100, length_secs, sample_rate, bitrate, file_size,
                image_path, _date_created,
            ) = row?;

            let usb_id = (seq + 1) as u32;
            id_map.insert(content_id, usb_id);

            let artist = artists.get(&artist_id).cloned().unwrap_or_default();
            let remixer = artists.get(&remixer_id).cloned().unwrap_or_default();
            let album = albums.get(&album_id).cloned().unwrap_or_default();
            let genre = genres.get(&genre_id).cloned().unwrap_or_default();
            let label = labels.get(&label_id).cloned().unwrap_or_default();
            let key = keys.get(&key_id).cloned().unwrap_or_default();

            let artwork_path = image_path
                .filter(|p| !p.is_empty())
                .map(PathBuf::from);

            artwork_paths.push(artwork_path);

            tracks.push(Track {
                id: usb_id,
                source_path: PathBuf::from(&folder_path),
                // usb_path will be assigned by the writer when exporting
                usb_path: String::new(),
                title,
                artist,
                album,
                genre,
                label,
                remixer,
                comment,
                year: year.clamp(0, u16::MAX as i32) as u16,
                disc_number: disc_no.clamp(0, u16::MAX as i32) as u16,
                track_number: track_no.max(0) as u32,
                tempo: bpm_x100.max(0) as u32,
                key,
                duration_secs: length_secs as f64,
                sample_rate: sample_rate.max(0) as u32,
                bitrate: bitrate.max(0) as u32,
                file_size: file_size as u64,
                artwork: None, // caller loads artwork from artwork_paths if needed
            });
        }
    }

    // ── Cue points ───────────────────────────────────────────────────

    let mut cue_map: HashMap<String, Vec<CuePoint>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT ContentID, InMsec, OutMsec, Kind, ActiveLoop
             FROM djmdCue
             ORDER BY ContentID, ID",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,          // ContentID
                row.get::<_, Option<i64>>(1)?,     // InMsec
                row.get::<_, Option<i64>>(2)?,     // OutMsec (-1 = no loop)
                row.get::<_, Option<i32>>(3)?,     // Kind (hot cue number)
                row.get::<_, Option<i32>>(4)?,     // ActiveLoop (can be NULL)
            ))
        })?;

        for row in rows {
            let (content_id, in_msec, out_msec, kind, active_loop) = row?;

            let time_ms = match in_msec {
                Some(ms) if ms >= 0 => ms as u32,
                _ => continue, // skip cues with no valid position
            };

            // OutMsec = -1 is the sentinel for "no loop end"
            let loop_time_ms = match (active_loop, out_msec) {
                (Some(1), Some(ms)) if ms >= 0 => Some(ms as u32),
                _ => None,
            };

            let hot_cue_number = kind.unwrap_or(0).max(0) as u32;

            cue_map
                .entry(content_id)
                .or_default()
                .push(CuePoint { hot_cue_number, time_ms, loop_time_ms });
        }
    }

    // Re-query content IDs in order to arrange cue points parallel to tracks.
    let ordered_content_ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT ID FROM djmdContent
             WHERE (rb_local_deleted = 0 OR rb_local_deleted IS NULL)
             ORDER BY ID",
        )?;
        stmt.query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect()
    };

    let cue_points: Vec<Vec<CuePoint>> = ordered_content_ids
        .iter()
        .map(|cid| cue_map.remove(cid).unwrap_or_default())
        .collect();

    // ── Playlists ────────────────────────────────────────────────────

    let mut playlists: Vec<Playlist> = Vec::new();
    {
        // Only leaf playlists (Attribute = 0), not folders (Attribute = 1)
        let mut stmt = conn.prepare(
            "SELECT ID, Name FROM djmdPlaylist
             WHERE Attribute = 0
             ORDER BY Seq",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for (pl_seq, row) in rows.enumerate() {
            let (playlist_id, name) = row?;
            let pl_usb_id = (pl_seq + 1) as u32;

            let mut track_stmt = conn.prepare(
                "SELECT ContentID FROM djmdSongPlaylist
                 WHERE PlaylistID = ?1
                 ORDER BY TrackNo",
            )?;

            let track_ids: Vec<u32> = track_stmt
                .query_map(params![playlist_id], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .filter_map(|cid| id_map.get(&cid).copied())
                .collect();

            playlists.push(Playlist {
                id: pl_usb_id,
                name,
                track_ids,
            });
        }
    }

    Ok(MasterDbImport {
        tracks,
        cue_points,
        playlists,
        artwork_paths,
    })
}

fn open_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("Failed to open master.db at {}", path.display()))?;
    conn.execute_batch(&format!("PRAGMA key = '{MASTER_DB_KEY}';"))
        .context("Failed to set SQLCipher key for master.db")?;
    conn.execute_batch("SELECT count(*) FROM sqlite_master")
        .context("master.db decryption verification failed — wrong key?")?;
    Ok(conn)
}

/// Read a simple ID → Name lookup table (djmdArtist, djmdAlbum, etc.).
fn read_name_table(conn: &Connection, table: &str) -> Result<HashMap<String, String>> {
    let sql = format!("SELECT ID, Name FROM {table}");
    let mut stmt = conn
        .prepare(&sql)
        .with_context(|| format!("Failed to prepare {table} query"))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        map.insert(id, name);
    }
    Ok(map)
}

/// Read the djmdKey table: ID → ScaleName (Camelot notation).
fn read_key_table(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn
        .prepare("SELECT ID, ScaleName FROM djmdKey")
        .context("Failed to prepare djmdKey query")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        map.insert(id, name);
    }
    Ok(map)
}
