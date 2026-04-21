use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

use pioneer_usb_writer::models::{AnalysisResult, BeatGrid, CuePoint, WaveformPreview};

pub const SCHEMA_VERSION: u32 = 2;

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
    date_added    TEXT    NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS artwork (
    track_id   INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    image_data BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS analyses (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    bpm      REAL NOT NULL DEFAULT 0.0,
    key      TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS playlists (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
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

pub fn initialize(conn: &Connection, db_path: Option<&Path>) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .context("Failed to set WAL journal mode")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .context("Failed to enable foreign keys")?;

    let version: Option<u32> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_version'",
            [],
            |_| Ok(()),
        )
        .ok()
        .and_then(|()| {
            conn.query_row("SELECT version FROM schema_version", [], |row| row.get(0))
                .ok()
        });

    match version {
        None => {
            // Fresh database — create v2 schema.
            conn.execute_batch(SCHEMA_DDL)
                .context("Failed to create schema")?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
        }
        Some(1) => {
            migrate_v1_to_v2(conn, db_path)?;
        }
        Some(v) if v == SCHEMA_VERSION => {}
        Some(v) => {
            anyhow::bail!(
                "Unsupported schema version {} (expected {})",
                v,
                SCHEMA_VERSION
            );
        }
    }

    Ok(())
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
    conn.execute("UPDATE schema_version SET version = ?1", [SCHEMA_VERSION])?;

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
                cue_points,
                color_waveform: None,
            },
        ));
    }

    Ok(results)
}
