use anyhow::{Context, Result};
use rusqlite::Connection;

pub const SCHEMA_VERSION: u32 = 1;

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
    track_id   INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    beat_grid  TEXT NOT NULL,
    waveform   BLOB NOT NULL,
    bpm        REAL NOT NULL,
    key        TEXT NOT NULL,
    cue_points TEXT NOT NULL
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

pub fn initialize(conn: &Connection) -> Result<()> {
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
            conn.execute_batch(SCHEMA_DDL)
                .context("Failed to create schema")?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
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
