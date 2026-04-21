use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use pioneer_usb_writer::models::{Playlist, Track};

// ---------------------------------------------------------------------------
// Track queries
// ---------------------------------------------------------------------------

pub fn insert_track(conn: &Connection, track: &Track) -> Result<i64> {
    conn.execute(
        "INSERT INTO tracks (
            source_path, usb_path, title, artist, album, genre, label, remixer,
            comment, year, disc_number, track_number, tempo, key, duration_secs,
            sample_rate, bitrate, file_size, date_added
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19
        )",
        params![
            track.source_path.to_string_lossy().as_ref(),
            track.usb_path,
            track.title,
            track.artist,
            track.album,
            track.genre,
            track.label,
            track.remixer,
            track.comment,
            track.year,
            track.disc_number,
            track.track_number,
            track.tempo,
            track.key,
            track.duration_secs,
            track.sample_rate,
            track.bitrate,
            track.file_size as i64,
            chrono::Utc::now().format("%Y-%m-%d").to_string(),
        ],
    )
    .context("Failed to insert track")?;

    Ok(conn.last_insert_rowid())
}

fn row_to_track(row: &rusqlite::Row) -> rusqlite::Result<Track> {
    let source_path: String = row.get("source_path")?;
    let file_size: i64 = row.get("file_size")?;
    let id: i64 = row.get("id")?;
    Ok(Track {
        id: id as u32,
        source_path: std::path::PathBuf::from(source_path),
        usb_path: row.get("usb_path")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        genre: row.get("genre")?,
        label: row.get("label")?,
        remixer: row.get("remixer")?,
        comment: row.get("comment")?,
        year: row.get("year")?,
        disc_number: row.get("disc_number")?,
        track_number: row.get("track_number")?,
        tempo: row.get("tempo")?,
        key: row.get("key")?,
        duration_secs: row.get("duration_secs")?,
        sample_rate: row.get("sample_rate")?,
        bitrate: row.get("bitrate")?,
        file_size: file_size as u64,
        artwork: None,
    })
}

pub fn select_track(conn: &Connection, id: i64) -> Result<Option<Track>> {
    let mut stmt = conn.prepare("SELECT * FROM tracks WHERE id = ?1")?;
    let mut rows = stmt.query_map([id], row_to_track)?;
    match rows.next() {
        Some(Ok(track)) => Ok(Some(track)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

pub fn select_all_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut stmt = conn.prepare("SELECT * FROM tracks ORDER BY id")?;
    let rows = stmt.query_map([], row_to_track)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn update_track(conn: &Connection, id: i64, track: &Track) -> Result<()> {
    conn.execute(
        "UPDATE tracks SET
            source_path = ?1, usb_path = ?2, title = ?3, artist = ?4, album = ?5,
            genre = ?6, label = ?7, remixer = ?8, comment = ?9, year = ?10,
            disc_number = ?11, track_number = ?12, tempo = ?13, key = ?14,
            duration_secs = ?15, sample_rate = ?16, bitrate = ?17, file_size = ?18
        WHERE id = ?19",
        params![
            track.source_path.to_string_lossy().as_ref(),
            track.usb_path,
            track.title,
            track.artist,
            track.album,
            track.genre,
            track.label,
            track.remixer,
            track.comment,
            track.year,
            track.disc_number,
            track.track_number,
            track.tempo,
            track.key,
            track.duration_secs,
            track.sample_rate,
            track.bitrate,
            track.file_size as i64,
            id,
        ],
    )?;
    Ok(())
}

pub fn delete_tracks(conn: &Connection, ids: &[i64]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for id in ids {
        tx.execute("DELETE FROM tracks WHERE id = ?1", [id])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn track_exists_by_path(conn: &Connection, source_path: &Path) -> Result<bool> {
    let count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM tracks WHERE source_path = ?1",
        [source_path.to_string_lossy().as_ref()],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn track_count(conn: &Connection) -> Result<u32> {
    let count: u32 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Artwork queries
// ---------------------------------------------------------------------------

pub fn upsert_artwork(conn: &Connection, track_id: i64, image_data: &[u8]) -> Result<()> {
    conn.execute(
        "INSERT INTO artwork (track_id, image_data) VALUES (?1, ?2)
         ON CONFLICT(track_id) DO UPDATE SET image_data = excluded.image_data",
        params![track_id, image_data],
    )?;
    Ok(())
}

pub fn select_artwork(conn: &Connection, track_id: i64) -> Result<Option<Vec<u8>>> {
    let mut stmt = conn.prepare("SELECT image_data FROM artwork WHERE track_id = ?1")?;
    let mut rows = stmt.query_map([track_id], |row| row.get(0))?;
    match rows.next() {
        Some(Ok(data)) => Ok(Some(data)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

pub fn delete_artwork(conn: &Connection, track_id: i64) -> Result<()> {
    conn.execute("DELETE FROM artwork WHERE track_id = ?1", [track_id])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Analysis index queries (lightweight bpm/key in DB; full data in ANLZ files)
// ---------------------------------------------------------------------------

/// Insert or update the lightweight analysis index (bpm + key).
pub fn upsert_analysis_index(conn: &Connection, track_id: i64, bpm: f64, key: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO analyses (track_id, bpm, key) VALUES (?1, ?2, ?3)
         ON CONFLICT(track_id) DO UPDATE SET bpm = excluded.bpm, key = excluded.key",
        params![track_id, bpm, key],
    )?;
    Ok(())
}

/// Check whether an analysis index row exists for the given track.
pub fn select_analysis_index(
    conn: &Connection,
    track_id: i64,
) -> Result<Option<(f64, String)>> {
    let mut stmt = conn.prepare("SELECT bpm, key FROM analyses WHERE track_id = ?1")?;
    let mut rows = stmt.query_map([track_id], |row| {
        Ok((row.get::<_, f64>(0)?, row.get::<_, String>(1)?))
    })?;
    match rows.next() {
        Some(Ok(pair)) => Ok(Some(pair)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// Return IDs of all tracks that have an analysis row.
pub fn select_analyzed_track_ids(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT track_id FROM analyses ORDER BY track_id")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn select_unanalyzed_track_ids(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT t.id FROM tracks t LEFT JOIN analyses a ON a.track_id = t.id WHERE a.track_id IS NULL ORDER BY t.id",
    )?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Select all tracks with flags for artwork and analysis existence.
/// Returns (Track, has_artwork, has_analysis, has_cues).
///
/// `has_cues` is always `false` since cue data now lives in ANLZ files,
/// not in the database. Callers that need cue info should read the full
/// ANLZ file via `get_analysis`.
pub fn select_all_tracks_with_flags(
    conn: &Connection,
) -> Result<Vec<(Track, bool, bool, bool)>> {
    let mut stmt = conn.prepare(
        "SELECT t.*,
                (art.track_id IS NOT NULL) AS has_artwork,
                (a.track_id IS NOT NULL)   AS has_analysis
         FROM tracks t
         LEFT JOIN artwork art ON art.track_id = t.id
         LEFT JOIN analyses a ON a.track_id = t.id
         ORDER BY t.id",
    )?;

    let rows = stmt.query_map([], |row| {
        let source_path: String = row.get("source_path")?;
        let file_size: i64 = row.get("file_size")?;
        let id: i64 = row.get("id")?;
        let has_artwork: bool = row.get("has_artwork")?;
        let has_analysis: bool = row.get("has_analysis")?;

        Ok((
            Track {
                id: id as u32,
                source_path: std::path::PathBuf::from(source_path),
                usb_path: row.get("usb_path")?,
                title: row.get("title")?,
                artist: row.get("artist")?,
                album: row.get("album")?,
                genre: row.get("genre")?,
                label: row.get("label")?,
                remixer: row.get("remixer")?,
                comment: row.get("comment")?,
                year: row.get("year")?,
                disc_number: row.get("disc_number")?,
                track_number: row.get("track_number")?,
                tempo: row.get("tempo")?,
                key: row.get("key")?,
                duration_secs: row.get("duration_secs")?,
                sample_rate: row.get("sample_rate")?,
                bitrate: row.get("bitrate")?,
                file_size: file_size as u64,
                artwork: None,
            },
            has_artwork,
            has_analysis,
        ))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (track, has_artwork, has_analysis) = row?;
        // has_cues is always false — cue data lives in ANLZ files, not the DB.
        result.push((track, has_artwork, has_analysis, false));
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Playlist queries
// ---------------------------------------------------------------------------

pub fn insert_playlist(conn: &Connection, name: &str) -> Result<i64> {
    let sort_order: u32 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM playlists",
        [],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO playlists (name, sort_order) VALUES (?1, ?2)",
        params![name, sort_order],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_playlist(conn: &Connection, id: i64, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE playlists SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    Ok(())
}

pub fn delete_playlist(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_playlist_tracks(conn: &Connection, playlist_id: i64, track_ids: &[i64]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
        [playlist_id],
    )?;
    for (pos, track_id) in track_ids.iter().enumerate() {
        tx.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, pos as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn select_all_playlists(conn: &Connection) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare("SELECT id, name FROM playlists ORDER BY sort_order")?;
    let playlist_rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut playlists = Vec::with_capacity(playlist_rows.len());
    for (id, name) in playlist_rows {
        let track_ids = select_playlist_track_ids(conn, id)?;
        playlists.push(Playlist {
            id: id as u32,
            name,
            track_ids: track_ids.into_iter().map(|id| id as u32).collect(),
        });
    }
    Ok(playlists)
}

pub fn select_playlist(conn: &Connection, id: i64) -> Result<Option<Playlist>> {
    let mut stmt = conn.prepare("SELECT id, name FROM playlists WHERE id = ?1")?;
    let mut rows = stmt.query_map([id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;

    match rows.next() {
        Some(Ok((pl_id, name))) => {
            let track_ids = select_playlist_track_ids(conn, pl_id)?;
            Ok(Some(Playlist {
                id: pl_id as u32,
                name,
                track_ids: track_ids.into_iter().map(|id| id as u32).collect(),
            }))
        }
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

fn select_playlist_track_ids(conn: &Connection, playlist_id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
    )?;
    let rows = stmt.query_map([playlist_id], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
