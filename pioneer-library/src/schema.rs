use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};

use crate::duplicates::{file_name_key, normalise_tag, path_key};
use crate::failure::{Failure, LibraryFailure};

use pioneer_usb_writer::models::{AnalysisResult, BeatGrid, CuePoint, WaveformPreview};

pub const SCHEMA_VERSION: u32 = 8;

/// Version of the folds (`normalise_tag`, `path_key`, `file_name_key`) the stored keys were made with; bump it when the fold
/// changes and the keys are refilled at the next open.
pub const TAG_FOLD_VERSION: &str = "1";

/// The v7 cover blobs, set aside by the v8 migration until `LocalLibrary::move_legacy_covers` has moved them
/// into the cover store.
pub const LEGACY_ARTWORK_TABLE: &str = "artwork_v7_blobs";

/// Schema for a fresh v2 database.
const SCHEMA_DDL: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
    id            INTEGER PRIMARY KEY,
    source_path   TEXT    NOT NULL UNIQUE,
    usb_path      TEXT    NOT NULL,
    title         TEXT    NOT NULL DEFAULT '',
    artist        TEXT    NOT NULL DEFAULT '',
    album         TEXT    NOT NULL DEFAULT '',
    genre         TEXT    NOT NULL DEFAULT '',
    label         TEXT    NOT NULL DEFAULT '',
    remixer       TEXT    NOT NULL DEFAULT '',
    comment       TEXT    NOT NULL DEFAULT '',
    year          INTEGER NOT NULL DEFAULT 0,
    disc_number   INTEGER NOT NULL DEFAULT 0,
    track_number  INTEGER NOT NULL DEFAULT 0,
    tempo         INTEGER NOT NULL DEFAULT 0,
    key           TEXT    NOT NULL DEFAULT '',
    duration_secs REAL    NOT NULL DEFAULT 0.0,
    sample_rate   INTEGER NOT NULL DEFAULT 0,
    bitrate       INTEGER NOT NULL DEFAULT 0,
    file_size     INTEGER NOT NULL DEFAULT 0,
    date_added    TEXT    NOT NULL DEFAULT '',
    artist_key    TEXT    NOT NULL DEFAULT '',
    title_key     TEXT    NOT NULL DEFAULT '',
    path_key      TEXT    NOT NULL DEFAULT '',
    file_name_key TEXT    NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS analyses (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    bpm      REAL NOT NULL DEFAULT 0.0,
    key      TEXT NOT NULL DEFAULT '',
    energy   INTEGER,
    row_preview BLOB,
    stages   INTEGER NOT NULL DEFAULT 7
);

CREATE TABLE IF NOT EXISTS playlists (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    parent_id  INTEGER REFERENCES playlists(id) ON DELETE CASCADE,
    is_folder  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, track_id)
);

CREATE INDEX IF NOT EXISTS idx_tracks_source_path ON tracks(source_path);
CREATE INDEX IF NOT EXISTS idx_tracks_usb_path ON tracks(usb_path);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
";

/// v8: the artwork table holds only the hash of the cover file in the cover store (`cover_store`); `NULL` while
/// the cover is still in [`LEGACY_ARTWORK_TABLE`]. Failures of the analysis, and the library's own settings.
const V8_DDL: &str = "
CREATE TABLE IF NOT EXISTS artwork (
    track_id   INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    cover_hash TEXT
);
CREATE INDEX IF NOT EXISTS idx_artwork_hash ON artwork(cover_hash);

CREATE TABLE IF NOT EXISTS analysis_failures (
    track_id         INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    source_path      TEXT    NOT NULL,
    file_size        INTEGER,
    modified_ms      INTEGER,
    kind             TEXT    NOT NULL,
    attempts         INTEGER NOT NULL,
    analyzer_version TEXT    NOT NULL,
    failed_at_ms     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS library_meta (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

const STICK_DDL: &str = "
CREATE TABLE IF NOT EXISTS sticks (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL DEFAULT '',
    last_path  TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS stick_tracks (
    stick_id TEXT NOT NULL REFERENCES sticks(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY (stick_id, track_id)
);

CREATE TABLE IF NOT EXISTS stick_playlists (
    stick_id    TEXT NOT NULL REFERENCES sticks(id) ON DELETE CASCADE,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL,
    PRIMARY KEY (stick_id, playlist_id)
);
";

/// Additive, no version bump: tracks added by an import whose covers are still to be read
/// from their files (`LocalLibrary::add_tracks_covers_later`).
const PENDING_ARTWORK_DDL: &str = "CREATE TABLE IF NOT EXISTS pending_artwork (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE
);";

/// Additive, no version bump: a sampled content hash per track, taken while its file is there, so a moved
/// file can be recognised later; and the folder relocations that can still be undone.
const RECONNECT_DDL: &str = "
CREATE TABLE IF NOT EXISTS track_fingerprints (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    size     INTEGER NOT NULL,
    hash     TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS folder_relocations (
    id         INTEGER PRIMARY KEY,
    old_root   TEXT NOT NULL,
    new_root   TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS folder_relocation_tracks (
    relocation_id INTEGER NOT NULL REFERENCES folder_relocations(id) ON DELETE CASCADE,
    track_id      INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    old_path      TEXT NOT NULL,
    new_path      TEXT NOT NULL,
    PRIMARY KEY (relocation_id, track_id)
);
";

pub fn initialize(conn: &Connection, db_path: Option<&Path>) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .context("Failed to set WAL journal mode")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .context("Failed to enable foreign keys")?;

    match stored_version(conn)? {
        None => {
            let tx = conn.unchecked_transaction().context("Failed to start creating the library")?;
            tx.execute_batch(SCHEMA_DDL)
                .context("Failed to create schema")?;
            tx.execute_batch(STICK_DDL)
                .context("Failed to create stick tables")?;
            tx.execute_batch(V8_DDL).context("Failed to create the v8 tables")?;
            tx.execute("INSERT INTO library_meta (name, value) VALUES ('tag_fold', ?1)", [TAG_FOLD_VERSION])?;
            tx.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
            tx.commit().context("Failed to create the library")?;
        }
        Some(v) if (1..SCHEMA_VERSION).contains(&v) => {
            if v < 2 {
                migrate_v1_to_v2(conn, db_path)?;
            }
            if v < 3 {
                migrate_v2_to_v3(conn)?;
            }
            if v < 4 {
                migrate_v3_to_v4(conn)?;
            }
            if v < 5 {
                migrate_v4_to_v5(conn)?;
            }
            if v < 6 {
                migrate_v5_to_v6(conn)?;
            }
            if v < 7 {
                migrate_v6_to_v7(conn)?;
            }
            migrate_v7_to_v8(conn)?;
        }
        Some(v) if v == SCHEMA_VERSION => refill_tag_keys_if_stale(conn)?,
        Some(0) => {
            return Err(Failure::error(
                LibraryFailure::Damaged,
                "The library has schema version 0, which no version of this app writes",
            ));
        }
        Some(v) => {
            return Err(Failure::error(
                LibraryFailure::NewerVersion,
                format!(
                    "The library was saved by a newer version of the app (schema version {v}, this app knows up to {SCHEMA_VERSION}); update the app to open it"
                ),
            ));
        }
    }
    conn.execute_batch(PENDING_ARTWORK_DDL).context("Failed to create the pending artwork queue")?;
    conn.execute_batch(RECONNECT_DDL).context("Failed to create the reconnect tables")?;

    Ok(())
}

fn stored_version(conn: &Connection) -> Result<Option<u32>> {
    let table_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'", [], |row| row.get(0))
        .context("Failed to read the library's table list")?;
    if table_count == 0 {
        return Ok(None);
    }
    let has_version_table: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .context("Failed to read the library's table list")?;
    if !has_version_table {
        return Err(Failure::error(
            LibraryFailure::Damaged,
            "The library has tables but no schema version table; it was not opened so nothing in it is changed",
        ));
    }
    let version: Option<u32> = conn
        .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
        .optional()
        .context(Failure::new(
            LibraryFailure::Damaged,
            "The library's schema version is unreadable; it was not opened so nothing in it is changed",
        ))?;
    match version {
        Some(v) => Ok(Some(v)),
        None => Err(Failure::error(
            LibraryFailure::Damaged,
            "The library has no schema version row; it was not opened so nothing in it is changed",
        )),
    }
}

/// Migrate from v1 (analyses stored as JSON blobs in SQLite) to v2 (ANLZ files on disk).
fn migrate_v1_to_v2(conn: &Connection, db_path: Option<&Path>) -> Result<()> {
    // 1. Read all existing analysis rows from the old schema.
    let old_analyses = read_v1_analyses(conn)?;

    // 2. Write ANLZ files for each analysis (if we have a disk path).
    if let Some(path) = db_path {
        let anlz_base = path.parent().unwrap_or(Path::new(".")).join("anlz");
        for (track_id, source_path, analysis) in &old_analyses {
            let dir = anlz_base.join(track_id.to_string());
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create ANLZ dir: {}", dir.display()))?;

            // Build a minimal Track for the ANLZ writer (it needs usb_path for PPTH).
            let track_for_anlz = pioneer_usb_writer::models::Track {
                id: *track_id as u32,
                source_path: std::path::PathBuf::from(source_path),
                usb_path: source_path.clone(),
                title: String::new(),
                artist: String::new(),
                album: String::new(),
                genre: String::new(),
                label: String::new(),
                remixer: String::new(),
                comment: String::new(),
                year: 0,
                disc_number: 0,
                track_number: 0,
                tempo: (analysis.bpm * 100.0) as u32,
                key: analysis.key.clone(),
                duration_secs: 0.0,
                sample_rate: 44100,
                bitrate: 0,
                file_size: 0,
                artwork: None,
            };

            pioneer_usb_writer::writer::anlz::write_anlz_dat(
                &dir.join("ANLZ0000.DAT"),
                &track_for_anlz,
                analysis,
            )?;
            pioneer_usb_writer::writer::anlz::write_anlz_ext(
                &dir.join("ANLZ0000.EXT"),
                &track_for_anlz,
                analysis,
            )?;
            pioneer_usb_writer::writer::anlz::write_anlz_2ex(
                &dir.join("ANLZ0000.2EX"),
                &track_for_anlz,
                analysis,
            )?;
        }
    }

    // 3. Recreate the analyses table with the simplified v2 schema.
    conn.execute_batch("DROP TABLE IF EXISTS analyses;")?;
    conn.execute_batch(
        "CREATE TABLE analyses (
            track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
            bpm      REAL NOT NULL DEFAULT 0.0,
            key      TEXT NOT NULL DEFAULT ''
        );",
    )?;

    // 4. Re-insert bpm/key index rows.
    {
        let mut stmt = conn.prepare(
            "INSERT INTO analyses (track_id, bpm, key) VALUES (?1, ?2, ?3)",
        )?;
        for (track_id, _, analysis) in &old_analyses {
            stmt.execute(rusqlite::params![track_id, analysis.bpm, analysis.key])?;
        }
    }

    // 5. Update version.
    conn.execute("UPDATE schema_version SET version = 2", [])?;

    Ok(())
}

/// Read all v1 analysis rows (beat_grid JSON, waveform BLOB, bpm, key, cue_points JSON)
/// along with the track's source_path.
fn read_v1_analyses(conn: &Connection) -> Result<Vec<(i64, String, AnalysisResult)>> {
    let mut stmt = conn.prepare(
        "SELECT a.track_id, t.source_path, a.beat_grid, a.waveform, a.bpm, a.key, a.cue_points
         FROM analyses a
         INNER JOIN tracks t ON t.id = a.track_id",
    )?;

    let rows = stmt.query_map([], |row| {
        let track_id: i64 = row.get(0)?;
        let source_path: String = row.get(1)?;
        let beat_grid_json: String = row.get(2)?;
        let waveform_blob: Vec<u8> = row.get(3)?;
        let bpm: f64 = row.get(4)?;
        let key: String = row.get(5)?;
        let cue_points_json: String = row.get(6)?;
        Ok((track_id, source_path, beat_grid_json, waveform_blob, bpm, key, cue_points_json))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (track_id, source_path, beat_grid_json, waveform_blob, bpm, key, cue_points_json) = row?;
        let beat_grid: BeatGrid = serde_json::from_str(&beat_grid_json)
            .context("Failed to deserialize beat grid during migration")?;
        let cue_points: Vec<CuePoint> = serde_json::from_str(&cue_points_json)
            .context("Failed to deserialize cue points during migration")?;
        let mut waveform_data = [0u8; 400];
        let len = waveform_blob.len().min(400);
        waveform_data[..len].copy_from_slice(&waveform_blob[..len]);

        results.push((
            track_id,
            source_path,
            AnalysisResult {
                beat_grid,
                waveform: WaveformPreview { data: waveform_data },
                bpm,
                key,
                energy: None,
                cue_points,
                color_waveform: None,
            },
        ));
    }

    Ok(results)
}

/// Migrate from v2 (analyses table without energy) to v3 (add energy column).
fn migrate_v2_to_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE analyses ADD COLUMN energy INTEGER;")
        .context("Failed to add energy column")?;
    conn.execute("UPDATE schema_version SET version = 3", [])?;
    Ok(())
}

/// Migrate from v3 to v4: the 3-band row preview next to bpm/key, so the list
/// does not read ANLZ files per track. Additive and nullable: old rows stay
/// `NULL` until `LocalLibrary::backfill_row_previews` fills them, and an older
/// build simply ignores the column.
fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE analyses ADD COLUMN row_preview BLOB;")
        .context("Failed to add row_preview column")?;
    conn.execute("UPDATE schema_version SET version = 4", [])?;
    Ok(())
}

/// Every existing analysis row is complete: all three stage bits set.
fn migrate_v4_to_v5(conn: &Connection) -> Result<()> {
    let has_stages: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('analyses') WHERE name = 'stages'",
        [],
        |row| row.get(0),
    )?;
    if !has_stages {
        conn.execute_batch("ALTER TABLE analyses ADD COLUMN stages INTEGER NOT NULL DEFAULT 7;")
            .context("Failed to add stages column")?;
    }
    conn.execute("UPDATE schema_version SET version = 5", [])?;
    Ok(())
}

fn migrate_v5_to_v6(conn: &Connection) -> Result<()> {
    conn.execute_batch(STICK_DDL)
        .context("Failed to create stick tables")?;
    conn.execute("UPDATE schema_version SET version = 6", [])?;
    Ok(())
}

/// Playlist folders: folders are playlist rows, like Pioneer's own model. Additive:
/// `parent_id` is `NULL` for the root level and deleting a folder cascades to its contents.
fn migrate_v6_to_v7(conn: &Connection) -> Result<()> {
    let has_column = |name: &str| -> Result<bool> {
        Ok(conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('playlists') WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?)
    };
    if !has_column("parent_id")? {
        conn.execute_batch(
            "ALTER TABLE playlists ADD COLUMN parent_id INTEGER REFERENCES playlists(id) ON DELETE CASCADE;",
        )
        .context("Failed to add playlists.parent_id")?;
    }
    if !has_column("is_folder")? {
        conn.execute_batch("ALTER TABLE playlists ADD COLUMN is_folder INTEGER NOT NULL DEFAULT 0;")
            .context("Failed to add playlists.is_folder")?;
    }
    conn.execute("UPDATE schema_version SET version = 7", [])?;
    Ok(())
}

/// Folded tag keys, covers out of the database, analysis failures: one transaction. The cover blobs stay where they
/// are (renamed to [`LEGACY_ARTWORK_TABLE`]) and are moved by `LocalLibrary::move_legacy_covers` after the open.
fn migrate_v7_to_v8(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction().context("Failed to start the v8 migration")?;
    let has_column = |table: &str, name: &str| -> Result<bool> {
        Ok(tx.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2",
            [table, name],
            |row| row.get(0),
        )?)
    };
    for key in ["artist_key", "title_key", "path_key", "file_name_key"] {
        if !has_column("tracks", key)? {
            tx.execute_batch(&format!("ALTER TABLE tracks ADD COLUMN {key} TEXT NOT NULL DEFAULT '';"))
                .context("Failed to add the folded tag keys")?;
        }
    }
    if has_column("artwork", "image_data")? {
        tx.execute_batch(&format!(
            "ALTER TABLE artwork RENAME TO {LEGACY_ARTWORK_TABLE};
             {V8_DDL}
             INSERT INTO artwork (track_id) SELECT track_id FROM {LEGACY_ARTWORK_TABLE};"
        ))
        .context("Failed to set the cover blobs aside")?;
    }
    tx.execute_batch(V8_DDL).context("Failed to create the v8 tables")?;
    tx.execute_batch("DROP INDEX IF EXISTS idx_artwork_track;")?;
    fill_tag_keys(&tx)?;
    tx.execute("UPDATE schema_version SET version = 8", [])?;
    tx.commit().context("Failed to finish the v8 migration")?;
    Ok(())
}

fn refill_tag_keys_if_stale(conn: &Connection) -> Result<()> {
    let stored: Option<String> = conn
        .query_row("SELECT value FROM library_meta WHERE name = 'tag_fold'", [], |row| row.get(0))
        .optional()?;
    if stored.as_deref() == Some(TAG_FOLD_VERSION) {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    fill_tag_keys(&tx)?;
    tx.commit().context("Failed to refill the folded tag keys")?;
    Ok(())
}

fn fill_tag_keys(conn: &Connection) -> Result<()> {
    let tracks: Vec<(i64, String, String, String)> = conn
        .prepare("SELECT id, artist, title, source_path FROM tracks")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut update = conn.prepare("UPDATE tracks SET artist_key = ?2, title_key = ?3, path_key = ?4, file_name_key = ?5 WHERE id = ?1")?;
    for (id, artist, title, source_path) in &tracks {
        let source_path = Path::new(source_path);
        update.execute(rusqlite::params![id, normalise_tag(artist), normalise_tag(title), path_key(source_path), file_name_key(source_path)])?;
    }
    conn.execute(
        "INSERT INTO library_meta (name, value) VALUES ('tag_fold', ?1) ON CONFLICT(name) DO UPDATE SET value = excluded.value",
        [TAG_FOLD_VERSION],
    )?;
    Ok(())
}
