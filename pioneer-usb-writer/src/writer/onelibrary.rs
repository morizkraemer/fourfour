//! OneLibrary SQLCipher database writer and reader for Pioneer CDJ-3000.
//!
//! Writes `exportLibrary.db` — the newer SQLCipher-encrypted SQLite format
//! that sits alongside the legacy `export.pdb` on Pioneer USB drives.

use anyhow::{Context, Result};
use chrono::Local;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

use crate::models::{
    AnalysisResult, ExistingPlaylist, ExistingTrack, ExistingUsbState, Playlist, Track,
};

/// SQLCipher encryption key for exportLibrary.db (from rekordbox binary).
const DB_KEY: &str = "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls";

/// Relative path from the USB root to the database file.
const DB_REL_PATH: [&str; 3] = ["PIONEER", "rekordbox", "exportLibrary.db"];

// ── Key Names (rekordbox IDs 1-24) ───────────────────────────────

/// Rekordbox key names indexed by key_id (1-based).
/// IDs 1-12 = major keys, 13-24 = minor keys.
const KEY_NAMES: [&str; 24] = [
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
    "Cm", "Dbm", "Dm", "Ebm", "Em", "Fm", "F#m", "Gm", "Abm", "Am", "Bbm", "Bm",
];

// ── Color Names (Pioneer fixed 8 colors) ─────────────────────────

const COLOR_NAMES: [&str; 8] = [
    "Pink", "Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple",
];

// ── Menu Items ───────────────────────────────────────────────────

/// CDJ browse menu items. Names wrapped in \ufffa...\ufffb delimiters.
const MENU_ITEMS: [(i32, i32, &str); 21] = [
    (1, 1, "GENRE"),
    (2, 2, "ARTIST"),
    (3, 3, "ALBUM"),
    (4, 4, "TRACK"),
    (5, 6, "PLAYLIST"),
    (6, 7, "HISTORY"),
    (7, 10, "KEY"),
    (8, 12, "BPM"),
    (9, 13, "RATING"),
    (10, 14, "COLOR"),
    (11, 16, "TIME"),
    (12, 17, "BITRATE"),
    (13, 19, "FILENAME"),
    (14, 23, "LABEL"),
    (15, 28, "REMIXER"),
    (16, 29, "DJ PLAY COUNT"),
    (17, 30, "YEAR"),
    (18, 31, "HOT CUE BANK LIST"),
    (19, 33, "MY TAG"),
    (20, 34, "COMMENT"),
    (21, 35, "DATE ADDED"),
];

// ── Public API ───────────────────────────────────────────────────

/// Write the OneLibrary SQLCipher database.
/// Creates `{output_dir}/PIONEER/rekordbox/exportLibrary.db`.
pub fn write_onelibrary(
    output_dir: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
) -> Result<()> {
    let db_path = output_dir
        .join(DB_REL_PATH[0])
        .join(DB_REL_PATH[1])
        .join(DB_REL_PATH[2]);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create rekordbox directory for OneLibrary")?;
    }

    // Remove stale DB + WAL/SHM files to start fresh
    for ext in ["", "-wal", "-shm"] {
        let p = db_path.with_extension(format!("db{ext}"));
        let _ = std::fs::remove_file(&p);
    }

    let conn = open_db(&db_path)?;

    create_tables(&conn).context("Failed to create OneLibrary tables")?;

    // Populate static lookup tables
    populate_key_table(&conn).context("Failed to populate key table")?;
    populate_color_table(&conn).context("Failed to populate color table")?;
    populate_menu_item_table(&conn).context("Failed to populate menuItem table")?;
    populate_category_table(&conn).context("Failed to populate category table")?;
    populate_sort_table(&conn).context("Failed to populate sort table")?;

    // Dynamic data — all in one transaction for performance
    conn.execute_batch("BEGIN TRANSACTION")
        .context("Failed to begin transaction")?;

    // Dedup lookup values
    let (artists, artist_map) = dedup_ci(tracks.iter().map(|t| t.artist.clone()));
    let (albums, album_map) = dedup_ci(tracks.iter().map(|t| t.album.clone()));
    let (genres, genre_map) = dedup_ci(
        tracks.iter().filter(|t| !t.genre.is_empty()).map(|t| t.genre.clone()),
    );
    let (labels, label_map) = dedup_ci(
        tracks.iter().filter(|t| !t.label.is_empty()).map(|t| t.label.clone()),
    );

    // Album → artist mapping (first artist seen per album)
    let mut album_artist_map: HashMap<String, u32> = HashMap::new();
    for track in tracks {
        let album_key = track.album.to_lowercase();
        if !album_artist_map.contains_key(&album_key) {
            let artist_id = *artist_map.get(&track.artist.to_lowercase()).unwrap_or(&0);
            album_artist_map.insert(album_key, artist_id);
        }
    }

    // Image table: one row per track with artwork
    let mut image_map: HashMap<u32, u32> = HashMap::new(); // track.id → image_id
    let mut image_id_counter: u32 = 0;
    for track in tracks {
        if track.artwork.is_some() {
            image_id_counter += 1;
            image_map.insert(track.id, image_id_counter);
        }
    }

    // Album → image mapping (first artwork in album)
    let mut album_image_map: HashMap<String, u32> = HashMap::new();
    for track in tracks {
        let album_key = track.album.to_lowercase();
        if !album_image_map.contains_key(&album_key) {
            if let Some(&img_id) = image_map.get(&track.id) {
                album_image_map.insert(album_key, img_id);
            }
        }
    }

    // Remixer dedup — remixers go into the artist table with offset IDs
    let (remixers, remixer_map_raw) = dedup_ci(
        tracks.iter().filter(|t| !t.remixer.is_empty()).map(|t| t.remixer.clone()),
    );
    let remixer_offset = artists.len() as u32;
    let remixer_map: HashMap<String, u32> = remixer_map_raw
        .into_iter()
        .map(|(k, v)| (k, v + remixer_offset))
        .collect();

    write_artists(&conn, &artists, &remixers)
        .context("Failed to write artist rows")?;
    write_albums(&conn, &albums, &album_map, &album_artist_map, &album_image_map)
        .context("Failed to write album rows")?;
    write_genres(&conn, &genres).context("Failed to write genre rows")?;
    write_labels(&conn, &labels).context("Failed to write label rows")?;
    write_images(&conn, &image_map).context("Failed to write image rows")?;

    write_content(
        &conn,
        tracks,
        analyses,
        &artist_map,
        &remixer_map,
        &album_map,
        &genre_map,
        &label_map,
        &image_map,
    )
    .context("Failed to write content rows")?;

    write_cues(&conn, tracks, analyses).context("Failed to write cue rows")?;
    write_playlists(&conn, playlists).context("Failed to write playlist rows")?;

    write_property(&conn, tracks.len() as i32)
        .context("Failed to write property row")?;

    conn.execute_batch("COMMIT")
        .context("Failed to commit transaction")?;

    // Checkpoint WAL so the data is in the main file for subsequent reads
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .context("Failed to checkpoint WAL")?;

    Ok(())
}

/// Read existing USB state from the OneLibrary database.
/// Returns `Ok(None)` if no `exportLibrary.db` exists at the expected path.
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

// ── Database Connection ──────────────────────────────────────────

/// Open (or create) the SQLCipher database and set the encryption key.
fn open_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("Failed to open OneLibrary DB at {}", path.display()))?;

    conn.execute_batch(&format!("PRAGMA key='{DB_KEY}'"))
        .context("Failed to set SQLCipher key")?;

    // Verify decryption by reading the schema
    conn.execute_batch("SELECT count(*) FROM sqlite_master")
        .context("SQLCipher key verification failed")?;

    conn.execute_batch("PRAGMA journal_mode=WAL")
        .context("Failed to set WAL journal mode")?;

    Ok(conn)
}

// ── Schema Creation ──────────────────────────────────────────────

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS content(
            content_id INTEGER PRIMARY KEY,
            title VARCHAR,
            titleForSearch VARCHAR,
            subtitle VARCHAR,
            bpmx100 INTEGER,
            length INTEGER,
            trackNo INTEGER,
            discNo INTEGER,
            artist_id_artist INTEGER,
            artist_id_remixer INTEGER,
            artist_id_originalArtist INTEGER,
            artist_id_composer INTEGER,
            artist_id_lyricist INTEGER,
            album_id INTEGER,
            genre_id INTEGER,
            label_id INTEGER,
            key_id INTEGER,
            color_id INTEGER,
            image_id INTEGER,
            djComment VARCHAR,
            rating INTEGER,
            releaseYear INTEGER,
            releaseDate VARCHAR,
            dateCreated VARCHAR,
            dateAdded VARCHAR,
            path VARCHAR,
            fileName VARCHAR,
            fileSize INTEGER,
            fileType INTEGER,
            bitrate INTEGER,
            bitDepth INTEGER,
            samplingRate INTEGER,
            isrc VARCHAR,
            djPlayCount INTEGER,
            isHotCueAutoLoadOn INTEGER,
            isKuvoDeliverStatusOn INTEGER,
            kuvoDeliveryComment VARCHAR,
            masterDbId INTEGER,
            masterContentId INTEGER,
            analysisDataFilePath VARCHAR,
            analysedBits INTEGER,
            contentLink INTEGER,
            hasModified INTEGER,
            cueUpdateCount INTEGER,
            analysisDataUpdateCount INTEGER,
            informationUpdateCount INTEGER
        );

        CREATE TABLE IF NOT EXISTS artist(
            artist_id INTEGER PRIMARY KEY,
            name VARCHAR,
            nameForSearch VARCHAR
        );

        CREATE TABLE IF NOT EXISTS album(
            album_id INTEGER PRIMARY KEY,
            name VARCHAR,
            artist_id INTEGER,
            image_id INTEGER,
            isComplation INTEGER,
            nameForSearch VARCHAR
        );

        CREATE TABLE IF NOT EXISTS genre(
            genre_id INTEGER PRIMARY KEY,
            name VARCHAR
        );

        CREATE TABLE IF NOT EXISTS label(
            label_id INTEGER PRIMARY KEY,
            name VARCHAR
        );

        CREATE TABLE IF NOT EXISTS key(
            key_id INTEGER PRIMARY KEY,
            name VARCHAR
        );

        CREATE TABLE IF NOT EXISTS color(
            color_id INTEGER PRIMARY KEY,
            name VARCHAR
        );

        CREATE TABLE IF NOT EXISTS image(
            image_id INTEGER PRIMARY KEY,
            path VARCHAR
        );

        CREATE TABLE IF NOT EXISTS cue(
            cue_id INTEGER PRIMARY KEY,
            content_id INTEGER,
            kind INTEGER,
            colorTableIndex INTEGER,
            cueComment VARCHAR,
            isActiveLoop INTEGER,
            beatLoopNumerator INTEGER,
            beatLoopDenominator INTEGER,
            inUsec INTEGER,
            outUsec INTEGER,
            in150FramePerSec INTEGER,
            out150FramePerSec INTEGER,
            inMpegFrameNumber INTEGER,
            outMpegFrameNumber INTEGER,
            inMpegAbs INTEGER,
            outMpegAbs INTEGER,
            inDecodingStartFramePosition INTEGER,
            outDecodingStartFramePosition INTEGER,
            inFileOffsetInBlock INTEGER,
            OutFileOffsetInBlock INTEGER,
            inNumberOfSampleInBlock INTEGER,
            outNumberOfSampleInBlock INTEGER
        );

        CREATE TABLE IF NOT EXISTS playlist(
            playlist_id INTEGER PRIMARY KEY,
            sequenceNo INTEGER,
            name VARCHAR,
            image_id INTEGER,
            attribute INTEGER,
            playlist_id_parent INTEGER
        );

        CREATE TABLE IF NOT EXISTS playlist_content(
            playlist_id INTEGER,
            content_id INTEGER,
            sequenceNo INTEGER
        );

        CREATE TABLE IF NOT EXISTS history(
            history_id INTEGER PRIMARY KEY,
            sequenceNo INTEGER,
            name VARCHAR,
            attribute INTEGER,
            history_id_parent INTEGER
        );

        CREATE TABLE IF NOT EXISTS history_content(
            history_id INTEGER,
            content_id INTEGER,
            sequenceNo INTEGER
        );

        CREATE TABLE IF NOT EXISTS hotCueBankList(
            hotCueBankList_id INTEGER PRIMARY KEY,
            sequenceNo INTEGER,
            name VARCHAR,
            image_id INTEGER,
            attribute INTEGER,
            hotCueBankList_id_parent INTEGER
        );

        CREATE TABLE IF NOT EXISTS hotCueBankList_cue(
            hotCueBankList_id INTEGER,
            cue_id INTEGER,
            sequenceNo INTEGER
        );

        CREATE TABLE IF NOT EXISTS myTag(
            myTag_id INTEGER PRIMARY KEY,
            sequenceNo INTEGER,
            name VARCHAR,
            attribute INTEGER,
            myTag_id_parent INTEGER
        );

        CREATE TABLE IF NOT EXISTS myTag_content(
            myTag_id INTEGER,
            content_id INTEGER
        );

        CREATE TABLE IF NOT EXISTS menuItem(
            menuItem_id INTEGER PRIMARY KEY,
            kind INTEGER,
            name VARCHAR
        );

        CREATE TABLE IF NOT EXISTS category(
            category_id INTEGER PRIMARY KEY,
            menuItem_id INTEGER,
            sequenceNo INTEGER,
            isVisible INTEGER
        );

        CREATE TABLE IF NOT EXISTS sort(
            sort_id INTEGER PRIMARY KEY,
            menuItem_id INTEGER,
            sequenceNo INTEGER,
            isVisible INTEGER,
            isSelectedAsSubColumn INTEGER
        );

        CREATE TABLE IF NOT EXISTS property(
            deviceName VARCHAR,
            dbVersion VARCHAR,
            numberOfContents INTEGER,
            createdDate VARCHAR,
            backGroundColorType INTEGER,
            myTagMasterDBID INTEGER
        );

        CREATE TABLE IF NOT EXISTS recommendedLike(
            content_id_1 INTEGER,
            content_id_2 INTEGER,
            rating INTEGER,
            createdDate INTEGER
        );
        ",
    )?;

    Ok(())
}

// ── Static Lookup Table Population ───────────────────────────────

fn populate_key_table(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("INSERT INTO key (key_id, name) VALUES (?1, ?2)")?;
    for (i, name) in KEY_NAMES.iter().enumerate() {
        stmt.execute(params![(i + 1) as u32, name])?;
    }
    Ok(())
}

fn populate_color_table(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("INSERT INTO color (color_id, name) VALUES (?1, ?2)")?;
    for (i, name) in COLOR_NAMES.iter().enumerate() {
        stmt.execute(params![(i + 1) as u32, name])?;
    }
    Ok(())
}

fn populate_menu_item_table(conn: &Connection) -> Result<()> {
    let mut stmt =
        conn.prepare("INSERT INTO menuItem (menuItem_id, kind, name) VALUES (?1, ?2, ?3)")?;
    for &(id, kind, name) in &MENU_ITEMS {
        let delimited = format!("\u{fffa}{name}\u{fffb}");
        stmt.execute(params![id, kind, delimited])?;
    }
    Ok(())
}

fn populate_category_table(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO category (category_id, menuItem_id, sequenceNo, isVisible) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (i, &(menu_id, _, _)) in MENU_ITEMS.iter().enumerate() {
        let seq = (i + 1) as i32;
        stmt.execute(params![seq, menu_id, seq, 1])?;
    }
    Ok(())
}

fn populate_sort_table(conn: &Connection) -> Result<()> {
    // Sort entries — visible sort columns for CDJ browsing
    let sort_items: [(i32, i32); 10] = [
        (4, 1),   // TRACK
        (2, 2),   // ARTIST
        (3, 3),   // ALBUM
        (8, 4),   // BPM
        (9, 5),   // RATING
        (1, 6),   // GENRE
        (7, 7),   // KEY
        (14, 8),  // LABEL
        (18, 9),  // YEAR
        (21, 10), // DATE ADDED
    ];
    let mut stmt = conn.prepare(
        "INSERT INTO sort (sort_id, menuItem_id, sequenceNo, isVisible, isSelectedAsSubColumn) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (i, &(menu_id, seq)) in sort_items.iter().enumerate() {
        let is_selected = if i == 0 { 1 } else { 0 };
        stmt.execute(params![(i + 1) as i32, menu_id, seq, 1, is_selected])?;
    }
    Ok(())
}

// ── Key Mapping ──────────────────────────────────────────────────

/// Map DJ notation key (e.g. "1A", "5B") to rekordbox key ID (1-24).
fn key_name_to_id(key: &str) -> u32 {
    // key_id 1-12 = C,Db,D,Eb,E,F,F#,G,Ab,A,Bb,B (major)
    // key_id 13-24 = Cm,Dbm,...,Bm (minor)
    // DJ notation: 1A=C(1), 2A=G(8), 3A=D(3), 4A=A(10), 5A=E(5), 6A=B(12),
    //              7A=F#(7), 8A=Db(2), 9A=Ab(9), 10A=Eb(4), 11A=Bb(11), 12A=F(6)
    // Minor: 1B=Am(22), 2B=Em(17), etc.
    let major_map: &[u32] = &[1, 8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6]; // 1A..12A
    let minor_map: &[u32] = &[22, 17, 24, 19, 14, 21, 16, 23, 18, 13, 20, 15]; // 1B..12B

    if key.is_empty() {
        return 0;
    }

    if key.ends_with('A') || key.ends_with('B') {
        let is_minor = key.ends_with('B');
        if let Ok(num) = key[..key.len() - 1].parse::<usize>() {
            if (1..=12).contains(&num) {
                return if is_minor {
                    minor_map[num - 1]
                } else {
                    major_map[num - 1]
                };
            }
        }
    }
    0
}

/// Map rekordbox key ID (1-24) back to DJ notation string.
fn key_id_to_name(id: u32) -> &'static str {
    // Reverse the mapping: find which DJ notation maps to this key_id
    let major_map: &[u32] = &[1, 8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6];
    let minor_map: &[u32] = &[22, 17, 24, 19, 14, 21, 16, 23, 18, 13, 20, 15];

    // Major keys (1A-12A)
    for (i, &kid) in major_map.iter().enumerate() {
        if kid == id {
            return match i + 1 {
                1 => "1A",
                2 => "2A",
                3 => "3A",
                4 => "4A",
                5 => "5A",
                6 => "6A",
                7 => "7A",
                8 => "8A",
                9 => "9A",
                10 => "10A",
                11 => "11A",
                12 => "12A",
                _ => "",
            };
        }
    }

    // Minor keys (1B-12B)
    for (i, &kid) in minor_map.iter().enumerate() {
        if kid == id {
            return match i + 1 {
                1 => "1B",
                2 => "2B",
                3 => "3B",
                4 => "4B",
                5 => "5B",
                6 => "6B",
                7 => "7B",
                8 => "8B",
                9 => "9B",
                10 => "10B",
                11 => "11B",
                12 => "12B",
                _ => "",
            };
        }
    }

    ""
}

// ── File Type Detection ──────────────────────────────────────────

/// Map file extension to Pioneer fileType integer.
fn file_type_from_path(path: &str) -> i32 {
    let lower = path.to_lowercase();
    if lower.ends_with(".mp3") {
        1
    } else if lower.ends_with(".m4a") || lower.ends_with(".aac") {
        4
    } else if lower.ends_with(".flac") {
        5
    } else if lower.ends_with(".wav") {
        11
    } else if lower.ends_with(".aiff") || lower.ends_with(".aif") {
        12
    } else {
        0
    }
}

// ── Case-Insensitive Dedup ───────────────────────────────────────

/// Deduplicates strings case-insensitively, returning unique display names
/// and a lowercase → 1-based ID mapping.
fn dedup_ci(values: impl Iterator<Item = String>) -> (Vec<String>, HashMap<String, u32>) {
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut unique = Vec::new();
    for v in values {
        let key = v.to_lowercase();
        if !seen.contains_key(&key) {
            let id = unique.len() as u32 + 1;
            seen.insert(key, id);
            unique.push(v);
        }
    }
    (unique, seen)
}

// ── Dynamic Lookup Table Writers ─────────────────────────────────

fn write_artists(conn: &Connection, artists: &[String], remixers: &[String]) -> Result<()> {
    let mut stmt =
        conn.prepare("INSERT INTO artist (artist_id, name, nameForSearch) VALUES (?1, ?2, ?3)")?;
    for (i, name) in artists.iter().enumerate() {
        let id = (i + 1) as u32;
        let search = name.to_lowercase();
        stmt.execute(params![id, name, search])?;
    }
    // Remixers get IDs after all regular artists
    let offset = artists.len();
    for (i, name) in remixers.iter().enumerate() {
        let id = (offset + i + 1) as u32;
        let search = name.to_lowercase();
        stmt.execute(params![id, name, search])?;
    }
    Ok(())
}

fn write_albums(
    conn: &Connection,
    albums: &[String],
    album_map: &HashMap<String, u32>,
    album_artist_map: &HashMap<String, u32>,
    album_image_map: &HashMap<String, u32>,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO album (album_id, name, artist_id, image_id, isComplation, nameForSearch)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for name in albums {
        let key = name.to_lowercase();
        let id = album_map[&key];
        let artist_id = album_artist_map.get(&key).copied().unwrap_or(0);
        let image_id = album_image_map.get(&key).copied().unwrap_or(0);
        let search = name.to_lowercase();
        stmt.execute(params![id, name, artist_id, image_id, 0, search])?;
    }
    Ok(())
}

fn write_genres(conn: &Connection, genres: &[String]) -> Result<()> {
    let mut stmt = conn.prepare("INSERT INTO genre (genre_id, name) VALUES (?1, ?2)")?;
    for (i, name) in genres.iter().enumerate() {
        stmt.execute(params![(i + 1) as u32, name])?;
    }
    Ok(())
}

fn write_labels(conn: &Connection, labels: &[String]) -> Result<()> {
    let mut stmt = conn.prepare("INSERT INTO label (label_id, name) VALUES (?1, ?2)")?;
    for (i, name) in labels.iter().enumerate() {
        stmt.execute(params![(i + 1) as u32, name])?;
    }
    Ok(())
}

fn write_images(conn: &Connection, image_map: &HashMap<u32, u32>) -> Result<()> {
    let mut stmt = conn.prepare("INSERT INTO image (image_id, path) VALUES (?1, ?2)")?;
    // Sort by image_id for deterministic output
    let mut entries: Vec<_> = image_map.iter().collect();
    entries.sort_by_key(|&(_, img_id)| *img_id);
    for (_, &img_id) in entries {
        let path = format!("/PIONEER/Artwork/00001/b{img_id}.jpg");
        stmt.execute(params![img_id, path])?;
    }
    Ok(())
}

// ── Content (Track) Rows ─────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn write_content(
    conn: &Connection,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    artist_map: &HashMap<String, u32>,
    remixer_map: &HashMap<String, u32>,
    album_map: &HashMap<String, u32>,
    genre_map: &HashMap<String, u32>,
    label_map: &HashMap<String, u32>,
    image_map: &HashMap<u32, u32>,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO content (
            content_id, title, titleForSearch, subtitle,
            bpmx100, length, trackNo, discNo,
            artist_id_artist, artist_id_remixer, artist_id_originalArtist,
            artist_id_composer, artist_id_lyricist,
            album_id, genre_id, label_id, key_id, color_id, image_id,
            djComment, rating, releaseYear, releaseDate, dateCreated, dateAdded,
            path, fileName, fileSize, fileType, bitrate, bitDepth, samplingRate,
            isrc, djPlayCount, isHotCueAutoLoadOn, isKuvoDeliverStatusOn,
            kuvoDeliveryComment, masterDbId, masterContentId,
            analysisDataFilePath, analysedBits, contentLink,
            hasModified, cueUpdateCount, analysisDataUpdateCount, informationUpdateCount
        ) VALUES (
            ?1, ?2, ?3, ?4,
            ?5, ?6, ?7, ?8,
            ?9, ?10, ?11,
            ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19,
            ?20, ?21, ?22, ?23, ?24, ?25,
            ?26, ?27, ?28, ?29, ?30, ?31, ?32,
            ?33, ?34, ?35, ?36,
            ?37, ?38, ?39,
            ?40, ?41, ?42,
            ?43, ?44, ?45, ?46
        )",
    )?;

    let today = Local::now().format("%Y-%m-%d").to_string();

    for (track, analysis) in tracks.iter().zip(analyses.iter()) {
        let artist_id = *artist_map
            .get(&track.artist.to_lowercase())
            .unwrap_or(&0);
        let remixer_id = if track.remixer.is_empty() {
            0u32
        } else {
            *remixer_map
                .get(&track.remixer.to_lowercase())
                .unwrap_or(&0)
        };
        let album_id = *album_map
            .get(&track.album.to_lowercase())
            .unwrap_or(&0);
        let genre_id = if track.genre.is_empty() {
            0u32
        } else {
            *genre_map
                .get(&track.genre.to_lowercase())
                .unwrap_or(&0)
        };
        let label_id = if track.label.is_empty() {
            0u32
        } else {
            *label_map
                .get(&track.label.to_lowercase())
                .unwrap_or(&0)
        };

        let key_id = key_name_to_id(&analysis.key);
        let image_id = image_map.get(&track.id).copied().unwrap_or(0);
        let file_name = Path::new(&track.usb_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_string();
        let file_type = file_type_from_path(&track.usb_path);
        let bpmx100 = (analysis.bpm * 100.0).round() as i32;
        let length = track.duration_secs.round() as i32;

        // Use BPM from analysis as the tempo stored in the track
        let anlz_path = super::anlz::anlz_path_for_pdb(track);

        let release_year = if track.year > 0 {
            Some(track.year as i32)
        } else {
            None
        };

        stmt.execute(params![
            track.id,                          // content_id
            track.title,                       // title
            track.title.to_lowercase(),        // titleForSearch
            "",                                // subtitle
            bpmx100,                           // bpmx100
            length,                            // length
            track.track_number as i32,         // trackNo
            track.disc_number as i32,          // discNo
            artist_id,                         // artist_id_artist
            remixer_id,                        // artist_id_remixer
            0i32,                              // artist_id_originalArtist
            0i32,                              // artist_id_composer
            0i32,                              // artist_id_lyricist
            album_id,                          // album_id
            genre_id,                          // genre_id
            label_id,                          // label_id
            key_id,                            // key_id
            0i32,                              // color_id
            image_id,                          // image_id
            track.comment,                     // djComment
            0i32,                              // rating
            release_year,                      // releaseYear
            "",                                // releaseDate
            &today,                            // dateCreated
            &today,                            // dateAdded
            track.usb_path,                    // path
            file_name,                         // fileName
            track.file_size as i64,            // fileSize
            file_type,                         // fileType
            track.bitrate as i32,              // bitrate
            0i32,                              // bitDepth
            track.sample_rate as i32,          // samplingRate
            "",                                // isrc
            0i32,                              // djPlayCount
            0i32,                              // isHotCueAutoLoadOn
            0i32,                              // isKuvoDeliverStatusOn
            "",                                // kuvoDeliveryComment
            0i32,                              // masterDbId
            0i32,                              // masterContentId
            anlz_path,                         // analysisDataFilePath
            105i32,                            // analysedBits
            0i32,                              // contentLink
            0i32,                              // hasModified
            0i32,                              // cueUpdateCount
            0i32,                              // analysisDataUpdateCount
            0i32,                              // informationUpdateCount
        ])?;
    }

    Ok(())
}

// ── Cue Points ───────────────────────────────────────────────────

fn write_cues(conn: &Connection, tracks: &[Track], analyses: &[AnalysisResult]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO cue (
            cue_id, content_id, kind, colorTableIndex, cueComment,
            isActiveLoop, beatLoopNumerator, beatLoopDenominator,
            inUsec, outUsec,
            in150FramePerSec, out150FramePerSec,
            inMpegFrameNumber, outMpegFrameNumber,
            inMpegAbs, outMpegAbs,
            inDecodingStartFramePosition, outDecodingStartFramePosition,
            inFileOffsetInBlock, OutFileOffsetInBlock,
            inNumberOfSampleInBlock, outNumberOfSampleInBlock
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            ?6, ?7, ?8,
            ?9, ?10,
            ?11, ?12,
            ?13, ?14,
            ?15, ?16,
            ?17, ?18,
            ?19, ?20,
            ?21, ?22
        )",
    )?;

    let mut cue_id: u32 = 1;

    for (_track, analysis) in tracks.iter().zip(analyses.iter()) {
        let content_id = _track.id;

        for cue in &analysis.cue_points {
            let in_usec = (cue.time_ms as i64) * 1000;
            let out_usec = cue.loop_time_ms.map(|ms| (ms as i64) * 1000).unwrap_or(0);
            let is_loop = if cue.loop_time_ms.is_some() { 1 } else { 0 };
            let kind = cue.hot_cue_number as i32;

            stmt.execute(params![
                cue_id,      // cue_id
                content_id,  // content_id
                kind,        // kind
                0i32,        // colorTableIndex
                "",          // cueComment
                is_loop,     // isActiveLoop
                0i32,        // beatLoopNumerator
                0i32,        // beatLoopDenominator
                in_usec,     // inUsec
                out_usec,    // outUsec
                0i32,        // in150FramePerSec
                0i32,        // out150FramePerSec
                0i32,        // inMpegFrameNumber
                0i32,        // outMpegFrameNumber
                0i32,        // inMpegAbs
                0i32,        // outMpegAbs
                0i32,        // inDecodingStartFramePosition
                0i32,        // outDecodingStartFramePosition
                0i32,        // inFileOffsetInBlock
                0i32,        // OutFileOffsetInBlock
                0i32,        // inNumberOfSampleInBlock
                0i32,        // outNumberOfSampleInBlock
            ])?;

            cue_id += 1;
        }
    }

    Ok(())
}

// ── Playlists ────────────────────────────────────────────────────

fn write_playlists(conn: &Connection, playlists: &[Playlist]) -> Result<()> {
    let mut playlist_stmt = conn.prepare(
        "INSERT INTO playlist (playlist_id, sequenceNo, name, image_id, attribute, playlist_id_parent)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    let mut content_stmt = conn.prepare(
        "INSERT INTO playlist_content (playlist_id, content_id, sequenceNo)
         VALUES (?1, ?2, ?3)",
    )?;

    for (i, playlist) in playlists.iter().enumerate() {
        let seq = (i + 1) as i32;
        playlist_stmt.execute(params![
            playlist.id,
            seq,
            playlist.name,
            0i32,  // image_id
            0i32,  // attribute (0 = playlist, 1 = folder)
            0i32,  // parent = ROOT
        ])?;

        for (j, &track_id) in playlist.track_ids.iter().enumerate() {
            content_stmt.execute(params![playlist.id, track_id, (j + 1) as i32])?;
        }
    }

    Ok(())
}

// ── Property Row ─────────────────────────────────────────────────

fn write_property(conn: &Connection, track_count: i32) -> Result<()> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO property (deviceName, dbVersion, numberOfContents, createdDate, backGroundColorType, myTagMasterDBID)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params!["", "1000", track_count, today, 0, 0],
    )?;
    Ok(())
}

// ── Generic Lookup Table Reader ──────────────────────────────────

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
