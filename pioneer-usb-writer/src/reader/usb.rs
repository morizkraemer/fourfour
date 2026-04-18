//! Reader for existing Pioneer USB OneLibrary databases.
//!
//! Provides [`read_usb_state`] to inspect what tracks and playlists are
//! already stored on a USB drive before performing an incremental sync.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;

use crate::models::{ExistingPlaylist, ExistingTrack, ExistingUsbState};

/// SQLCipher encryption key for exportLibrary.db.
const USB_DB_KEY: &str = "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls";

/// Relative path from the USB root to the OneLibrary database file.
const DB_REL_PATH: [&str; 3] = ["PIONEER", "rekordbox", "exportLibrary.db"];

/// Read the existing track and playlist state from the OneLibrary database at
/// `{output_dir}/PIONEER/rekordbox/exportLibrary.db`.
///
/// Returns `Ok(None)` if the file does not exist (i.e. a fresh USB).
/// Returns an error if the file exists but cannot be decrypted or queried.
pub fn read_usb_state(output_dir: &Path) -> Result<Option<ExistingUsbState>> {
    let db_path = output_dir
        .join(DB_REL_PATH[0])
        .join(DB_REL_PATH[1])
        .join(DB_REL_PATH[2]);

    if !db_path.exists() {
        return Ok(None);
    }

    let conn = open_db(&db_path)?;

    // Verify decryption works
    conn.execute_batch("SELECT count(*) FROM sqlite_master")
        .context("Failed to verify OneLibrary decryption — wrong key?")?;

    // Read lookup tables
    let artists = read_lookup_table(&conn, "artist", "artist_id", "name")?;
    let albums = read_lookup_table(&conn, "album", "album_id", "name")?;
    let genres = read_lookup_table(&conn, "genre", "genre_id", "name")?;
    let labels = read_lookup_table(&conn, "label", "label_id", "name")?;
    let _keys = read_lookup_table(&conn, "key", "key_id", "name")?;

    // Read tracks
    let mut tracks: Vec<ExistingTrack> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT content_id, path, title, artist_id_artist, artist_id_remixer,
                    album_id, genre_id, label_id, key_id, djComment,
                    releaseYear, trackNo, discNo, bpmx100, length,
                    samplingRate, bitrate, fileSize, image_id
             FROM content ORDER BY content_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let content_id: u32 = row.get(0)?;
            let path: String = row.get(1)?;
            let title: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let artist_id: u32 = row.get::<_, Option<u32>>(3)?.unwrap_or(0);
            let remixer_id: u32 = row.get::<_, Option<u32>>(4)?.unwrap_or(0);
            let album_id: u32 = row.get::<_, Option<u32>>(5)?.unwrap_or(0);
            let genre_id: u32 = row.get::<_, Option<u32>>(6)?.unwrap_or(0);
            let label_id: u32 = row.get::<_, Option<u32>>(7)?.unwrap_or(0);
            let key_id: u32 = row.get::<_, Option<u32>>(8)?.unwrap_or(0);
            let comment: String = row.get::<_, Option<String>>(9)?.unwrap_or_default();
            let year: i32 = row.get::<_, Option<i32>>(10)?.unwrap_or(0);
            let track_no: u32 = row.get::<_, Option<u32>>(11)?.unwrap_or(0);
            let disc_no: i32 = row.get::<_, Option<i32>>(12)?.unwrap_or(0);
            let bpmx100: u32 = row.get::<_, Option<u32>>(13)?.unwrap_or(0);
            let length: i32 = row.get::<_, Option<i32>>(14)?.unwrap_or(0);
            let sample_rate: u32 = row.get::<_, Option<u32>>(15)?.unwrap_or(0);
            let bitrate: u32 = row.get::<_, Option<u32>>(16)?.unwrap_or(0);
            let file_size: i64 = row.get::<_, Option<i64>>(17)?.unwrap_or(0);
            let image_id: u32 = row.get::<_, Option<u32>>(18)?.unwrap_or(0);

            Ok((
                content_id, path, title, artist_id, remixer_id, album_id,
                genre_id, label_id, key_id, comment, year, track_no, disc_no,
                bpmx100, length, sample_rate, bitrate, file_size, image_id,
            ))
        })?;

        for row in rows {
            let (
                content_id, path, title, artist_id, remixer_id, album_id,
                genre_id, label_id, key_id, comment, year, track_no, disc_no,
                bpmx100, length, sample_rate, bitrate, file_size, image_id,
            ) = row?;

            let artist = artists.get(&artist_id).cloned().unwrap_or_default();
            let remixer = artists.get(&remixer_id).cloned().unwrap_or_default();
            let album = albums.get(&album_id).cloned().unwrap_or_default();
            let genre = genres.get(&genre_id).cloned().unwrap_or_default();
            let label = labels.get(&label_id).cloned().unwrap_or_default();
            let key = key_id_to_name(key_id);

            tracks.push(ExistingTrack {
                id: content_id,
                usb_path: path,
                title,
                artist,
                remixer,
                album,
                genre,
                label,
                key: key.to_string(),
                comment,
                year: year as u16,
                track_number: track_no,
                disc_number: disc_no as u16,
                tempo: bpmx100,
                duration_secs: length as f64,
                sample_rate,
                bitrate,
                file_size: file_size as u64,
                has_artwork: image_id > 0,
            });
        }
    }

    // Read playlists (skip root)
    let mut playlists: Vec<ExistingPlaylist> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT playlist_id, name FROM playlist
             WHERE playlist_id != 0 AND attribute != 1
             ORDER BY sequenceNo",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: u32 = row.get(0)?;
            let name: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
            Ok((id, name))
        })?;

        for row in rows {
            let (id, name) = row?;

            let mut track_stmt = conn.prepare(
                "SELECT content_id FROM playlist_content
                 WHERE playlist_id = ? ORDER BY sequenceNo",
            )?;
            let track_ids: Vec<u32> = track_stmt
                .query_map(params![id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            playlists.push(ExistingPlaylist {
                id,
                name,
                track_ids,
            });
        }
    }

    // Compute next IDs
    let next_track_id = tracks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    let next_playlist_id = playlists.iter().map(|p| p.id).max().unwrap_or(0) + 1;

    Ok(Some(ExistingUsbState {
        tracks,
        playlists,
        next_track_id,
        next_playlist_id,
    }))
}

fn open_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    conn.execute_batch(&format!("PRAGMA key = '{USB_DB_KEY}';"))
        .context("Failed to set SQLCipher key")?;
    Ok(conn)
}

/// Map exportLibrary.db key ID (1-24) to DJ notation string.
fn key_id_to_name(id: u32) -> &'static str {
    let major_map: &[u32] = &[1, 8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6];
    let minor_map: &[u32] = &[22, 17, 24, 19, 14, 21, 16, 23, 18, 13, 20, 15];

    for (i, &kid) in major_map.iter().enumerate() {
        if kid == id {
            return match i + 1 {
                1 => "1A", 2 => "2A", 3 => "3A", 4 => "4A",
                5 => "5A", 6 => "6A", 7 => "7A", 8 => "8A",
                9 => "9A", 10 => "10A", 11 => "11A", 12 => "12A",
                _ => "",
            };
        }
    }

    for (i, &kid) in minor_map.iter().enumerate() {
        if kid == id {
            return match i + 1 {
                1 => "1B", 2 => "2B", 3 => "3B", 4 => "4B",
                5 => "5B", 6 => "6B", 7 => "7B", 8 => "8B",
                9 => "9B", 10 => "10B", 11 => "11B", 12 => "12B",
                _ => "",
            };
        }
    }

    ""
}

fn read_lookup_table(
    conn: &Connection,
    table: &str,
    id_col: &str,
    name_col: &str,
) -> Result<HashMap<u32, String>> {
    let sql = format!("SELECT {id_col}, {name_col} FROM {table}");
    let mut stmt = conn
        .prepare(&sql)
        .with_context(|| format!("Failed to read {table} table"))?;
    let rows = stmt.query_map([], |row| {
        let id: u32 = row.get(0)?;
        let name: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
        Ok((id, name))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        map.insert(id, name);
    }
    Ok(map)
}
