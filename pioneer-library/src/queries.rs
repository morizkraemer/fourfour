use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pioneer_usb_writer::models::{Playlist, Track};

use crate::schema::LEGACY_ARTWORK_TABLE;
use crate::{AnalysisFailure, DuplicateCandidate, file_name_key, normalise_tag, path_key};

// ---------------------------------------------------------------------------
// Track queries
// ---------------------------------------------------------------------------

pub fn insert_track(conn: &Connection, track: &Track) -> Result<i64> {
    conn.prepare_cached(
        "INSERT INTO tracks (
            source_path, usb_path, title, artist, album, genre, label, remixer,
            comment, year, disc_number, track_number, tempo, key, duration_secs,
            sample_rate, bitrate, file_size, date_added, artist_key, title_key, path_key, file_name_key
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
            ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23
        )",
    )?
    .execute(params![
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
            normalise_tag(&track.artist),
            normalise_tag(&track.title),
            path_key(&track.source_path),
            file_name_key(&track.source_path),
        ])
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
    let mut stmt = conn.prepare_cached("SELECT * FROM tracks WHERE id = ?1")?;
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
            duration_secs = ?15, sample_rate = ?16, bitrate = ?17, file_size = ?18,
            artist_key = ?20, title_key = ?21, path_key = ?22, file_name_key = ?23
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
            normalise_tag(&track.artist),
            normalise_tag(&track.title),
            path_key(&track.source_path),
            file_name_key(&track.source_path),
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

fn row_to_duplicate_candidate(row: &rusqlite::Row) -> rusqlite::Result<DuplicateCandidate> {
    let id: i64 = row.get(0)?;
    let source_path: String = row.get(1)?;
    let file_size: i64 = row.get(2)?;
    Ok(DuplicateCandidate {
        id: id as u32,
        source_path: std::path::PathBuf::from(source_path),
        file_size: file_size as u64,
        artist: row.get(3)?,
        title: row.get(4)?,
        duration_secs: row.get(5)?,
        artist_key: row.get(6)?,
        title_key: row.get(7)?,
        path_key: row.get(8)?,
        file_name_key: row.get(9)?,
    })
}

const DUPLICATE_CANDIDATE_COLUMNS: &str = "id, source_path, file_size, artist, title, duration_secs, artist_key, title_key, path_key, file_name_key";

pub fn select_duplicate_candidates(conn: &Connection) -> Result<Vec<DuplicateCandidate>> {
    let mut stmt = conn.prepare(&format!("SELECT {DUPLICATE_CANDIDATE_COLUMNS} FROM tracks ORDER BY id"))?;
    let rows = stmt.query_map([], row_to_duplicate_candidate)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn select_duplicate_candidates_of(conn: &Connection, ids: &[i64]) -> Result<Vec<DuplicateCandidate>> {
    let mut stmt = conn.prepare_cached(&format!("SELECT {DUPLICATE_CANDIDATE_COLUMNS} FROM tracks WHERE id = ?1"))?;
    let mut found = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(candidate) = stmt.query_row([id], row_to_duplicate_candidate).optional()? {
            found.push(candidate);
        }
    }
    Ok(found)
}

pub fn select_track_files(conn: &Connection) -> Result<Vec<(i64, String, i64)>> {
    let mut stmt = conn.prepare("SELECT id, source_path, file_size FROM tracks ORDER BY id")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn update_source_path(conn: &Connection, id: i64, source_path: &Path) -> Result<bool> {
    let changed = conn
        .execute(
            "UPDATE tracks SET source_path = ?2, path_key = ?3, file_name_key = ?4 WHERE id = ?1",
            params![id, source_path.to_string_lossy().as_ref(), path_key(source_path), file_name_key(source_path)],
        )
        .with_context(|| format!("Failed to relink track {id} to {}", source_path.display()))?;
    Ok(changed > 0)
}

pub fn track_count(conn: &Connection) -> Result<u32> {
    let count: u32 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Artwork queries
// ---------------------------------------------------------------------------

pub fn upsert_artwork(conn: &Connection, track_id: i64, cover_hash: &str) -> Result<()> {
    conn.prepare_cached(
        "INSERT INTO artwork (track_id, cover_hash) VALUES (?1, ?2)
         ON CONFLICT(track_id) DO UPDATE SET cover_hash = excluded.cover_hash",
    )?
    .execute(params![track_id, cover_hash])?;
    Ok(())
}

pub fn queue_pending_artwork(conn: &Connection, track_id: i64) -> Result<()> {
    conn.prepare_cached("INSERT OR IGNORE INTO pending_artwork (track_id) VALUES (?1)")?.execute([track_id])?;
    Ok(())
}

pub fn select_pending_artwork(conn: &Connection, after: i64, limit: usize) -> Result<Vec<(i64, PathBuf)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT p.track_id, t.source_path FROM pending_artwork p JOIN tracks t ON t.id = p.track_id
         WHERE p.track_id > ?1 ORDER BY p.track_id LIMIT ?2",
    )?;
    let rows = stmt.query_map([after, limit as i64], |row| Ok((row.get(0)?, PathBuf::from(row.get::<_, String>(1)?))))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn insert_pending_artwork(conn: &Connection, track_id: i64, source_path: &Path, cover_hash: &str) -> Result<bool> {
    let inserted = conn
        .prepare_cached(
            "INSERT INTO artwork (track_id, cover_hash)
             SELECT ?1, ?3 WHERE EXISTS (
                 SELECT 1 FROM pending_artwork p JOIN tracks t ON t.id = p.track_id WHERE p.track_id = ?1 AND t.source_path = ?2
             )
             ON CONFLICT(track_id) DO NOTHING",
        )?
        .execute(params![track_id, source_path.to_string_lossy().as_ref(), cover_hash])?;
    Ok(inserted > 0)
}

pub fn unqueue_pending_artwork(conn: &Connection, track_id: i64, source_path: &Path) -> Result<()> {
    conn.prepare_cached(
        "DELETE FROM pending_artwork WHERE track_id = ?1
         AND EXISTS (SELECT 1 FROM tracks WHERE id = ?1 AND source_path = ?2)",
    )?
    .execute(params![track_id, source_path.to_string_lossy().as_ref()])?;
    Ok(())
}

pub fn select_track_paths(conn: &Connection) -> Result<std::collections::HashSet<PathBuf>> {
    let mut stmt = conn.prepare("SELECT source_path FROM tracks")?;
    let rows = stmt.query_map([], |row| Ok(PathBuf::from(row.get::<_, String>(0)?)))?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

/// The track's cover reference: `None` = no cover, `Some(None)` = the cover is still a v7 blob.
pub fn select_artwork_hash(conn: &Connection, track_id: i64) -> Result<Option<Option<String>>> {
    Ok(conn
        .prepare_cached("SELECT cover_hash FROM artwork WHERE track_id = ?1")?
        .query_row([track_id], |row| row.get(0))
        .optional()?)
}

pub fn select_artwork_hashes_of(conn: &Connection, track_ids: &[i64]) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached("SELECT cover_hash FROM artwork WHERE track_id = ?1 AND cover_hash IS NOT NULL")?;
    let mut hashes = Vec::new();
    for id in track_ids {
        if let Some(hash) = stmt.query_row([id], |row| row.get(0)).optional()? {
            hashes.push(hash);
        }
    }
    Ok(hashes)
}

pub fn cover_is_referenced(conn: &Connection, cover_hash: &str) -> Result<bool> {
    Ok(conn
        .prepare_cached("SELECT EXISTS (SELECT 1 FROM artwork WHERE cover_hash = ?1)")?
        .query_row([cover_hash], |row| row.get(0))?)
}

pub fn select_referenced_covers(conn: &Connection) -> Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT cover_hash FROM artwork WHERE cover_hash IS NOT NULL")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}

pub fn delete_artwork(conn: &Connection, track_id: i64) -> Result<()> {
    conn.execute("DELETE FROM artwork WHERE track_id = ?1", [track_id])?;
    Ok(())
}

pub fn legacy_artwork_exists(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [LEGACY_ARTWORK_TABLE],
        |row| row.get(0),
    )?)
}

pub fn select_legacy_artwork(conn: &Connection, track_id: i64) -> Result<Option<Vec<u8>>> {
    if !legacy_artwork_exists(conn)? {
        return Ok(None);
    }
    Ok(conn
        .prepare_cached(&format!("SELECT image_data FROM {LEGACY_ARTWORK_TABLE} WHERE track_id = ?1"))?
        .query_row([track_id], |row| row.get(0))
        .optional()?)
}

/// Rows and bytes still in the v7 blob table.
pub fn legacy_artwork_size(conn: &Connection) -> Result<(u64, u64)> {
    Ok(conn.query_row(
        &format!("SELECT COUNT(*), COALESCE(SUM(length(image_data)), 0) FROM {LEGACY_ARTWORK_TABLE}"),
        [],
        |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
    )?)
}

pub fn select_legacy_artwork_batch(conn: &Connection, limit: usize) -> Result<Vec<(i64, Vec<u8>)>> {
    let mut stmt =
        conn.prepare_cached(&format!("SELECT track_id, image_data FROM {LEGACY_ARTWORK_TABLE} ORDER BY track_id LIMIT ?1"))?;
    let rows = stmt.query_map([limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Points the track's artwork row at the moved cover (unless a cover was set or removed since) and drops the blob.
pub fn finish_legacy_artwork(conn: &Connection, track_id: i64, cover_hash: &str) -> Result<()> {
    conn.prepare_cached("UPDATE artwork SET cover_hash = ?2 WHERE track_id = ?1 AND cover_hash IS NULL")?
        .execute(params![track_id, cover_hash])?;
    conn.prepare_cached(&format!("DELETE FROM {LEGACY_ARTWORK_TABLE} WHERE track_id = ?1"))?.execute([track_id])?;
    Ok(())
}

pub fn drop_legacy_artwork(conn: &Connection) -> Result<()> {
    conn.execute_batch(&format!("DROP TABLE IF EXISTS {LEGACY_ARTWORK_TABLE}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Analysis failures
// ---------------------------------------------------------------------------

fn row_to_analysis_failure(row: &rusqlite::Row) -> rusqlite::Result<AnalysisFailure> {
    Ok(AnalysisFailure {
        track_id: row.get(0)?,
        source_path: PathBuf::from(row.get::<_, String>(1)?),
        file_size: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
        modified_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
        kind: row.get(4)?,
        attempts: row.get(5)?,
        analyzer_version: row.get(6)?,
        failed_at_ms: row.get::<_, i64>(7)? as u64,
    })
}

const ANALYSIS_FAILURE_COLUMNS: &str =
    "track_id, source_path, file_size, modified_ms, kind, attempts, analyzer_version, failed_at_ms";

pub fn select_analysis_failure(conn: &Connection, track_id: i64) -> Result<Option<AnalysisFailure>> {
    Ok(conn
        .prepare_cached(&format!("SELECT {ANALYSIS_FAILURE_COLUMNS} FROM analysis_failures WHERE track_id = ?1"))?
        .query_row([track_id], row_to_analysis_failure)
        .optional()?)
}

pub fn select_analysis_failures(conn: &Connection) -> Result<Vec<AnalysisFailure>> {
    let mut stmt = conn.prepare(&format!("SELECT {ANALYSIS_FAILURE_COLUMNS} FROM analysis_failures ORDER BY track_id"))?;
    let rows = stmt.query_map([], row_to_analysis_failure)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Stores the failure; a track that is gone is left out.
pub fn upsert_analysis_failure(conn: &Connection, failure: &AnalysisFailure) -> Result<()> {
    conn.prepare_cached(&format!(
        "INSERT INTO analysis_failures ({ANALYSIS_FAILURE_COLUMNS})
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8 WHERE EXISTS (SELECT 1 FROM tracks WHERE id = ?1)
         ON CONFLICT(track_id) DO UPDATE SET source_path = excluded.source_path, file_size = excluded.file_size,
             modified_ms = excluded.modified_ms, kind = excluded.kind, attempts = excluded.attempts,
             analyzer_version = excluded.analyzer_version, failed_at_ms = excluded.failed_at_ms"
    ))?
    .execute(params![
        failure.track_id,
        failure.source_path.to_string_lossy().as_ref(),
        failure.file_size.map(|v| v as i64),
        failure.modified_ms.map(|v| v as i64),
        failure.kind,
        failure.attempts,
        failure.analyzer_version,
        failure.failed_at_ms as i64,
    ])?;
    Ok(())
}

pub fn delete_analysis_failures(conn: &Connection, track_ids: &[i64]) -> Result<usize> {
    let mut stmt = conn.prepare_cached("DELETE FROM analysis_failures WHERE track_id = ?1")?;
    let mut removed = 0;
    for id in track_ids {
        removed += stmt.execute([id])?;
    }
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Analysis index queries (lightweight bpm/key in DB; full data in ANLZ files)
// ---------------------------------------------------------------------------

/// Insert or update the lightweight analysis index (bpm + key + energy + row preview).
pub fn upsert_analysis_index(
    conn: &Connection,
    track_id: i64,
    bpm: f64,
    key: &str,
    energy: Option<u8>,
    row_preview: &[u8],
    stages: u8,
) -> Result<()> {
    conn.execute(
        "INSERT INTO analyses (track_id, bpm, key, energy, row_preview, stages) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(track_id) DO UPDATE SET bpm = excluded.bpm, key = excluded.key,
             energy = excluded.energy, row_preview = excluded.row_preview, stages = excluded.stages",
        params![track_id, bpm, key, energy, row_preview, stages],
    )?;
    Ok(())
}

pub fn update_analysis_bpm(conn: &Connection, track_id: i64, bpm: f64) -> Result<bool> {
    Ok(conn.execute("UPDATE analyses SET bpm = ?2 WHERE track_id = ?1", params![track_id, bpm])? > 0)
}

pub fn update_analysis_key(conn: &Connection, track_id: i64, key: &str) -> Result<bool> {
    Ok(conn.execute("UPDATE analyses SET key = ?2 WHERE track_id = ?1", params![track_id, key])? > 0)
}

pub fn select_analysis_stages(conn: &Connection, track_id: i64) -> Result<Option<u8>> {
    let mut stmt = conn.prepare("SELECT stages FROM analyses WHERE track_id = ?1")?;
    let mut rows = stmt.query_map([track_id], |row| row.get::<_, u8>(0))?;
    rows.next().transpose().map_err(Into::into)
}

pub fn reset_analysis_stages(conn: &Connection, track_id: i64) -> Result<bool> {
    Ok(conn.execute("UPDATE analyses SET stages = 0 WHERE track_id = ?1", [track_id])? > 0)
}

pub fn select_incomplete_analysis_ids(conn: &Connection, complete: u8) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT track_id FROM analyses WHERE stages != ?1 ORDER BY track_id")?;
    let rows = stmt.query_map([complete], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Return IDs of tracks whose analysis has every stage.
pub fn select_complete_analysis_track_ids(conn: &Connection, complete: u8) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT track_id FROM analyses WHERE stages = ?1 ORDER BY track_id")?;
    let rows = stmt.query_map([complete], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// `(track id, row preview blob)` for every analysis row that has one stored.
pub fn select_row_previews(conn: &Connection) -> Result<Vec<(i64, Vec<u8>)>> {
    let mut stmt = conn.prepare("SELECT track_id, row_preview FROM analyses WHERE row_preview IS NOT NULL")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn select_missing_row_preview_ids(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT track_id FROM analyses WHERE row_preview IS NULL ORDER BY track_id")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn select_row_previews_of(conn: &Connection, track_ids: &[i64]) -> Result<Vec<(i64, Vec<u8>)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT track_id, row_preview FROM analyses WHERE track_id = ?1 AND row_preview IS NOT NULL",
    )?;
    let mut out = Vec::with_capacity(track_ids.len());
    for id in track_ids {
        if let Some(row) = stmt.query_row([id], |row| Ok((row.get(0)?, row.get(1)?))).optional()? {
            out.push(row);
        }
    }
    Ok(out)
}

pub fn update_row_preview(conn: &Connection, track_id: i64, row_preview: &[u8]) -> Result<()> {
    conn.execute("UPDATE analyses SET row_preview = ?2 WHERE track_id = ?1", params![track_id, row_preview])?;
    Ok(())
}

/// Check whether an analysis index row exists for the given track.
pub fn select_analysis_index(
    conn: &Connection,
    track_id: i64,
) -> Result<Option<(f64, String, Option<u8>)>> {
    let mut stmt = conn.prepare("SELECT bpm, key, energy FROM analyses WHERE track_id = ?1")?;
    let mut rows = stmt.query_map([track_id], |row| {
        Ok((row.get::<_, f64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<u8>>(2)?))
    })?;
    match rows.next() {
        Some(Ok(triple)) => Ok(Some(triple)),
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

pub fn select_unanalyzed_track_ids(conn: &Connection, complete: u8) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT t.id FROM tracks t LEFT JOIN analyses a ON a.track_id = t.id
         WHERE a.track_id IS NULL OR a.stages != ?1 ORDER BY t.id",
    )?;
    let rows = stmt.query_map([complete], |row| row.get(0))?;
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
        "SELECT t.id, t.source_path, t.usb_path, t.title, t.artist, t.album, t.genre,
                t.label, t.remixer, t.comment, t.year, t.disc_number, t.track_number,
                CASE WHEN a.bpm > 0 THEN CAST(ROUND(a.bpm * 100) AS INTEGER) ELSE t.tempo END,
                COALESCE(NULLIF(a.key, ''), t.key),
                t.duration_secs, t.sample_rate, t.bitrate, t.file_size,
                (art.track_id IS NOT NULL),
                (a.track_id IS NOT NULL)
         FROM tracks t
         LEFT JOIN artwork art ON art.track_id = t.id
         LEFT JOIN analyses a ON a.track_id = t.id
         ORDER BY t.id",
    )?;

    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let source_path: String = row.get(1)?;
        let file_size: i64 = row.get(18)?;

        Ok((
            Track {
                id: id as u32,
                source_path: std::path::PathBuf::from(source_path),
                usb_path: row.get(2)?,
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                genre: row.get(6)?,
                label: row.get(7)?,
                remixer: row.get(8)?,
                comment: row.get(9)?,
                year: row.get(10)?,
                disc_number: row.get(11)?,
                track_number: row.get(12)?,
                tempo: row.get(13)?,
                key: row.get(14)?,
                duration_secs: row.get(15)?,
                sample_rate: row.get(16)?,
                bitrate: row.get(17)?,
                file_size: file_size as u64,
                artwork: None,
            },
            row.get::<_, bool>(19)?,
            row.get::<_, bool>(20)?,
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

/// Every track as the track list shows it, with its artwork and analysis flags, stored analysis stages and row
/// preview: one query.
pub fn select_track_list_rows(conn: &Connection, with_previews: bool) -> Result<Vec<crate::TrackListRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT t.id, t.source_path, t.title, t.artist, t.album, t.genre,
                CASE WHEN a.bpm > 0 THEN CAST(ROUND(a.bpm * 100) AS INTEGER) ELSE t.tempo END,
                COALESCE(NULLIF(a.key, ''), t.key),
                t.duration_secs, t.bitrate, t.file_size,
                (art.track_id IS NOT NULL),
                a.stages,
                CASE WHEN ?1 THEN a.row_preview END
         FROM tracks t
         LEFT JOIN artwork art ON art.track_id = t.id
         LEFT JOIN analyses a ON a.track_id = t.id
         ORDER BY t.id",
    )?;
    let rows = stmt.query_map([with_previews], |row| {
        Ok(crate::TrackListRow {
            id: row.get::<_, i64>(0)? as u32,
            source_path: row.get(1)?,
            title: row.get(2)?,
            artist: row.get(3)?,
            album: row.get(4)?,
            genre: row.get(5)?,
            tempo: row.get(6)?,
            key: row.get(7)?,
            duration_secs: row.get(8)?,
            bitrate: row.get(9)?,
            file_size: row.get::<_, i64>(10)? as u64,
            has_artwork: row.get(11)?,
            analysis_stages: row.get(12)?,
            row_preview: row.get::<_, Option<Vec<u8>>>(13)?.map(crate::RowPreview::from_blob),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Playlist queries
// ---------------------------------------------------------------------------

pub fn insert_playlist(conn: &Connection, name: &str) -> Result<i64> {
    insert_playlist_row(conn, name, 0, false)
}

pub fn insert_playlist_row(conn: &Connection, name: &str, parent_id: u32, is_folder: bool) -> Result<i64> {
    let sort_order: u32 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM playlists",
        [],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO playlists (name, sort_order, parent_id, is_folder) VALUES (?1, ?2, ?3, ?4)",
        params![name, sort_order, (parent_id != 0).then_some(parent_id as i64), is_folder],
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
    write_playlist_tracks(&tx, playlist_id, track_ids.iter().copied())?;
    tx.commit()?;
    Ok(())
}

fn write_playlist_tracks(conn: &Connection, playlist_id: i64, track_ids: impl Iterator<Item = i64>) -> Result<()> {
    conn.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
        [playlist_id],
    )?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
    )?;
    for (pos, track_id) in track_ids.enumerate() {
        insert.execute(params![playlist_id, track_id, pos as i64])?;
    }
    Ok(())
}

fn validate_playlist_tree(playlists: &[Playlist]) -> Result<()> {
    let by_id: HashMap<u32, &Playlist> = playlists.iter().map(|p| (p.id, p)).collect();
    for playlist in playlists {
        if playlist.is_folder && !playlist.track_ids.is_empty() {
            anyhow::bail!("Folder {} cannot hold tracks", playlist.id);
        }
        let mut ancestor = playlist.parent_id;
        for _ in 0..=playlists.len() {
            if ancestor == 0 {
                break;
            }
            let Some(parent) = by_id.get(&ancestor) else {
                anyhow::bail!("Playlist {} has unknown parent {}", playlist.id, ancestor);
            };
            if !parent.is_folder {
                anyhow::bail!("Playlist {} has non-folder parent {}", playlist.id, ancestor);
            }
            ancestor = parent.parent_id;
        }
        if ancestor != 0 {
            anyhow::bail!("Playlist {} is inside its own subtree", playlist.id);
        }
    }
    Ok(())
}

pub fn replace_playlists(conn: &Connection, playlists: &[Playlist]) -> Result<()> {
    validate_playlist_tree(playlists)?;
    let tx = conn.unchecked_transaction()?;
    let existing: Vec<i64> = tx
        .prepare("SELECT id FROM playlists")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for id in existing {
        if !playlists.iter().any(|p| p.id as i64 == id) {
            tx.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        }
    }
    for (sort_order, playlist) in playlists.iter().enumerate() {
        tx.execute(
            "INSERT INTO playlists (id, name, sort_order, is_folder) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = ?2, sort_order = ?3, is_folder = ?4",
            params![playlist.id as i64, playlist.name, sort_order as i64, playlist.is_folder],
        )?;
        let stored = select_playlist_track_ids(&tx, playlist.id as i64)?;
        let unchanged = stored.len() == playlist.track_ids.len()
            && stored.iter().zip(&playlist.track_ids).all(|(&a, &b)| a == b as i64);
        if !unchanged {
            write_playlist_tracks(&tx, playlist.id as i64, playlist.track_ids.iter().map(|&t| t as i64))?;
        }
    }
    for playlist in playlists {
        tx.execute(
            "UPDATE playlists SET parent_id = ?1 WHERE id = ?2",
            params![(playlist.parent_id != 0).then_some(playlist.parent_id as i64), playlist.id as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn select_all_playlists(conn: &Connection) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, COALESCE(parent_id, 0), is_folder FROM playlists ORDER BY sort_order",
    )?;
    let playlist_rows: Vec<(i64, String, i64, bool)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut playlists = Vec::with_capacity(playlist_rows.len());
    for (id, name, parent_id, is_folder) in playlist_rows {
        let track_ids = select_playlist_track_ids(conn, id)?;
        playlists.push(Playlist {
            id: id as u32,
            name,
            track_ids: track_ids.into_iter().map(|id| id as u32).collect(),
            parent_id: parent_id as u32,
            is_folder,
        });
    }
    Ok(playlists)
}

pub fn select_playlist(conn: &Connection, id: i64) -> Result<Option<Playlist>> {
    let mut stmt = conn.prepare("SELECT id, name, COALESCE(parent_id, 0), is_folder FROM playlists WHERE id = ?1")?;
    let mut rows = stmt.query_map([id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, bool>(3)?))
    })?;

    match rows.next() {
        Some(Ok((pl_id, name, parent_id, is_folder))) => {
            let track_ids = select_playlist_track_ids(conn, pl_id)?;
            Ok(Some(Playlist {
                id: pl_id as u32,
                name,
                track_ids: track_ids.into_iter().map(|id| id as u32).collect(),
                parent_id: parent_id as u32,
                is_folder,
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

pub fn count_tracks(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))?;
    Ok(count as usize)
}
