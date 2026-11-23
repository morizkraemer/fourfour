use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, Row};

use super::model::{
    StickAlbum, StickCue, StickDevice, StickHistory, StickModel, StickMyTag, StickName, StickPlaylist, StickTrack,
};

pub const KEY: &str = "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls";

pub struct OpenedCopy {
    pub conn: Connection,
    _scratch: tempfile::TempDir,
}

pub fn open_copy(db_path: &Path) -> Result<OpenedCopy> {
    let scratch = tempfile::tempdir().context("Cannot create a scratch folder for exportLibrary.db")?;
    let copy = scratch.path().join("exportLibrary.db");
    std::fs::copy(db_path, &copy).with_context(|| format!("Cannot read {}", db_path.display()))?;
    for suffix in ["-wal", "-shm"] {
        let mut side = db_path.as_os_str().to_owned();
        side.push(suffix);
        let side = Path::new(&side);
        if side.exists() {
            let mut target = copy.as_os_str().to_owned();
            target.push(suffix);
            std::fs::copy(side, Path::new(&target)).with_context(|| format!("Cannot read {}", side.display()))?;
        }
    }
    let conn = Connection::open_with_flags(&copy, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .with_context(|| format!("Cannot open {}", db_path.display()))?;
    conn.execute_batch(&format!("PRAGMA key = '{KEY}';")).context("Cannot set the exportLibrary.db key")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .with_context(|| format!("{} is not a readable OneLibrary database", db_path.display()))?;
    Ok(OpenedCopy { conn, _scratch: scratch })
}

fn int(row: &Row, name: &str) -> i64 {
    row.get::<_, Option<i64>>(name).ok().flatten().unwrap_or(0)
}

fn text(row: &Row, name: &str) -> String {
    row.get::<_, Option<String>>(name).ok().flatten().unwrap_or_default()
}

fn id(row: &Row, name: &str) -> u32 {
    u32::try_from(int(row, name)).unwrap_or(0)
}

fn has_table(conn: &Connection, table: &str) -> bool {
    conn.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [table], |row| {
        row.get::<_, i64>(0)
    })
    .map(|count| count > 0)
    .unwrap_or(false)
}

fn names(conn: &Connection, table: &str, id_column: &str) -> Result<Vec<StickName>> {
    if !has_table(conn, table) {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\" ORDER BY {id_column}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(StickName { id: id(row, id_column), name: text(row, if table == "image" { "path" } else { "name" }) })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn list_tracks(conn: &Connection, table: &str, list_column: &str) -> Result<BTreeMap<u32, Vec<u32>>> {
    let mut by_list: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    if !has_table(conn, table) {
        return Ok(by_list);
    }
    let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY {list_column}, sequenceNo"))?;
    let rows = stmt.query_map([], |row| Ok((id(row, list_column), id(row, "content_id"))))?;
    for row in rows {
        let (list, track) = row?;
        by_list.entry(list).or_default().push(track);
    }
    Ok(by_list)
}

fn cues(conn: &Connection) -> Result<BTreeMap<u32, Vec<StickCue>>> {
    let mut by_track: BTreeMap<u32, Vec<StickCue>> = BTreeMap::new();
    if !has_table(conn, "cue") {
        return Ok(by_track);
    }
    let mut stmt = conn.prepare("SELECT * FROM cue ORDER BY content_id, cue_id")?;
    let rows = stmt.query_map([], |row| {
        let out_usec = int(row, "outUsec");
        let color = int(row, "colorTableIndex");
        Ok((
            id(row, "content_id"),
            StickCue {
                hot_cue: id(row, "kind"),
                time_ms: u32::try_from(int(row, "inUsec") / 1000).unwrap_or(0),
                loop_end_ms: (out_usec > 0).then(|| u32::try_from(out_usec / 1000).unwrap_or(0)),
                comment: text(row, "cueComment"),
                color_index: (color > 0).then(|| u8::try_from(color).unwrap_or(0)),
            },
        ))
    })?;
    for row in rows {
        let (track, cue) = row?;
        by_track.entry(track).or_default().push(cue);
    }
    Ok(by_track)
}

fn tracks(conn: &Connection) -> Result<Vec<StickTrack>> {
    if !has_table(conn, "content") {
        return Ok(Vec::new());
    }
    let mut cues = cues(conn)?;
    let mut stmt = conn.prepare("SELECT * FROM content ORDER BY content_id")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(StickTrack {
                id: id(row, "content_id"),
                title: text(row, "title"),
                path: text(row, "path"),
                filename: text(row, "fileName"),
                artist_id: id(row, "artist_id_artist"),
                album_id: id(row, "album_id"),
                genre_id: id(row, "genre_id"),
                label_id: id(row, "label_id"),
                key_id: id(row, "key_id"),
                remixer_id: id(row, "artist_id_remixer"),
                composer_id: id(row, "artist_id_composer"),
                original_artist_id: id(row, "artist_id_originalArtist"),
                artwork_id: id(row, "image_id"),
                color_id: u8::try_from(int(row, "color_id")).unwrap_or(0),
                rating: u8::try_from(int(row, "rating")).unwrap_or(0),
                tempo_x100: id(row, "bpmx100"),
                duration_secs: id(row, "length"),
                year: u16::try_from(int(row, "releaseYear")).unwrap_or(0),
                track_number: id(row, "trackNo"),
                disc_number: u16::try_from(int(row, "discNo")).unwrap_or(0),
                bitrate: id(row, "bitrate"),
                sample_rate: id(row, "samplingRate"),
                sample_depth: u16::try_from(int(row, "bitDepth")).unwrap_or(0),
                file_size: u64::try_from(int(row, "fileSize")).unwrap_or(0),
                file_type: u16::try_from(int(row, "fileType")).unwrap_or(0),
                play_count: id(row, "djPlayCount"),
                comment: text(row, "djComment"),
                mix_name: text(row, "subtitle"),
                isrc: text(row, "isrc"),
                date_added: text(row, "dateAdded"),
                date_created: text(row, "dateCreated"),
                release_date: text(row, "releaseDate"),
                analyze_path: text(row, "analysisDataFilePath"),
                hot_cue_auto_load: int(row, "isHotCueAutoLoadOn") != 0,
                master_db_id: int(row, "masterDbId") as u32,
                master_content_id: int(row, "masterContentId") as u32,
                content_link: int(row, "contentLink") as u32,
                analysed_bits: row.get::<_, Option<i64>>("analysedBits").ok().flatten(),
                cues: Vec::new(),
                device_library_strings: BTreeMap::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .map(|mut track| {
            track.cues = cues.remove(&track.id).unwrap_or_default();
            track
        })
        .collect())
}

pub fn read_connection(conn: &Connection) -> Result<StickModel> {
    let tracks = tracks(conn)?;
    let mut playlist_tracks = list_tracks(conn, "playlist_content", "playlist_id")?;
    let mut history_tracks = list_tracks(conn, "history_content", "history_id")?;

    let albums = if has_table(conn, "album") {
        let mut stmt = conn.prepare("SELECT * FROM album ORDER BY album_id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(StickAlbum { id: id(row, "album_id"), name: text(row, "name"), artist_id: id(row, "artist_id") })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    } else {
        Vec::new()
    };

    let playlists = if has_table(conn, "playlist") {
        let mut stmt = conn.prepare("SELECT * FROM playlist ORDER BY playlist_id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(StickPlaylist {
                    id: id(row, "playlist_id"),
                    parent_id: id(row, "playlist_id_parent"),
                    name: text(row, "name"),
                    is_folder: int(row, "attribute") == 1,
                    sort_order: id(row, "sequenceNo").saturating_sub(1),
                    track_ids: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|mut playlist| {
                playlist.track_ids = playlist_tracks.remove(&playlist.id).unwrap_or_default();
                playlist
            })
            .collect()
    } else {
        Vec::new()
    };

    let histories = if has_table(conn, "history") {
        let mut stmt = conn.prepare("SELECT * FROM history ORDER BY history_id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(StickHistory { id: id(row, "history_id"), name: text(row, "name"), track_ids: Vec::new() })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|mut history| {
                history.track_ids = history_tracks.remove(&history.id).unwrap_or_default();
                history
            })
            .collect()
    } else {
        Vec::new()
    };

    let device = if has_table(conn, "property") {
        conn.query_row("SELECT * FROM property LIMIT 1", [], |row| {
            Ok(StickDevice {
                name: text(row, "deviceName"),
                db_version: text(row, "dbVersion"),
                created_date: text(row, "createdDate"),
                background_color: u8::try_from(int(row, "backGroundColorType")).unwrap_or(0),
                track_count: id(row, "numberOfContents"),
            })
        })
        .unwrap_or_default()
    } else {
        StickDevice::default()
    };

    let my_tags = if has_table(conn, "myTag") {
        let mut stmt = conn.prepare("SELECT * FROM myTag ORDER BY myTag_id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(StickMyTag {
                    id: int(row, "myTag_id") as u32,
                    name: text(row, "name"),
                    parent_id: int(row, "myTag_id_parent") as u32,
                    sort_order: id(row, "sequenceNo").saturating_sub(1),
                    is_category: int(row, "attribute") == 1,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    } else {
        Vec::new()
    };
    let track_tags = if has_table(conn, "myTag_content") {
        let mut stmt = conn.prepare("SELECT * FROM myTag_content ORDER BY content_id, myTag_id")?;
        let rows = stmt
            .query_map([], |row| Ok((id(row, "content_id"), int(row, "myTag_id") as u32)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    } else {
        Vec::new()
    };
    let my_tag_master_db_id = if has_table(conn, "property") {
        conn.query_row("SELECT * FROM property LIMIT 1", [], |row| Ok(int(row, "myTagMasterDBID") as u32)).unwrap_or(0)
    } else {
        0
    };

    Ok(StickModel {
        my_tags,
        track_tags,
        my_tag_master_db_id,
        device,
        tracks,
        artists: names(conn, "artist", "artist_id")?,
        albums,
        genres: names(conn, "genre", "genre_id")?,
        labels: names(conn, "label", "label_id")?,
        keys: names(conn, "key", "key_id")?,
        colors: names(conn, "color", "color_id")?,
        artworks: names(conn, "image", "image_id")?,
        playlists,
        histories,
        menu: Default::default(),
    })
}

pub fn read(db_path: &Path) -> Result<StickModel> {
    let opened = open_copy(db_path)?;
    read_connection(&opened.conn)
}

fn search_name(name: &str) -> String {
    name.to_lowercase()
}

fn filled(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

fn insert_rows(conn: &Connection, model: &StickModel) -> Result<()> {
    use rusqlite::params;

    let mut stmt = conn.prepare("INSERT INTO key (key_id, name) VALUES (?1, ?2)")?;
    for key in &model.keys {
        stmt.execute(params![key.id, key.name])?;
    }
    let mut stmt = conn.prepare("INSERT INTO color (color_id, name) VALUES (?1, ?2)")?;
    for color in &model.colors {
        stmt.execute(params![color.id, color.name])?;
    }
    let mut stmt = conn.prepare("INSERT INTO artist (artist_id, name, nameForSearch) VALUES (?1, ?2, ?3)")?;
    for artist in &model.artists {
        stmt.execute(params![artist.id, artist.name, search_name(&artist.name)])?;
    }
    let mut stmt = conn.prepare(
        "INSERT INTO album (album_id, name, artist_id, image_id, isComplation, nameForSearch)
         VALUES (?1, ?2, ?3, ?4, 0, ?5)",
    )?;
    for album in &model.albums {
        let image_id = model
            .tracks
            .iter()
            .find(|track| track.album_id == album.id && track.artwork_id != 0)
            .map_or(0, |track| track.artwork_id);
        stmt.execute(params![album.id, album.name, album.artist_id, image_id, search_name(&album.name)])?;
    }
    let mut stmt = conn.prepare("INSERT INTO genre (genre_id, name) VALUES (?1, ?2)")?;
    for genre in &model.genres {
        stmt.execute(params![genre.id, genre.name])?;
    }
    let mut stmt = conn.prepare("INSERT INTO label (label_id, name) VALUES (?1, ?2)")?;
    for label in &model.labels {
        stmt.execute(params![label.id, label.name])?;
    }
    let mut stmt = conn.prepare("INSERT INTO image (image_id, path) VALUES (?1, ?2)")?;
    for artwork in &model.artworks {
        stmt.execute(params![artwork.id, artwork.name])?;
    }

    let mut stmt = conn.prepare(
        "INSERT INTO content (
            content_id, title, titleForSearch, subtitle, bpmx100, length, trackNo, discNo,
            artist_id_artist, artist_id_remixer, artist_id_originalArtist, artist_id_composer, artist_id_lyricist,
            album_id, genre_id, label_id, key_id, color_id, image_id, djComment, rating,
            releaseYear, releaseDate, dateCreated, dateAdded, path, fileName, fileSize, fileType,
            bitrate, bitDepth, samplingRate, isrc, djPlayCount, isHotCueAutoLoadOn,
            isKuvoDeliverStatusOn, kuvoDeliveryComment, masterDbId, masterContentId,
            analysisDataFilePath, analysedBits, contentLink,
            hasModified, cueUpdateCount, analysisDataUpdateCount, informationUpdateCount
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, 0, '', ?35, ?36,
            ?37, ?38, ?39, 0, 0, 0, 0
        )",
    )?;
    for track in &model.tracks {
        stmt.execute(params![
            track.id,
            track.title,
            search_name(&track.title),
            track.mix_name,
            track.tempo_x100,
            track.duration_secs,
            track.track_number,
            track.disc_number,
            track.artist_id,
            track.remixer_id,
            track.original_artist_id,
            track.composer_id,
            track.album_id,
            track.genre_id,
            track.label_id,
            track.key_id,
            track.color_id,
            track.artwork_id,
            track.comment,
            track.rating,
            (track.year > 0).then_some(track.year),
            track.release_date,
            track.date_created,
            track.date_added,
            track.path,
            track.filename,
            track.file_size as i64,
            track.file_type,
            track.bitrate,
            track.sample_depth,
            track.sample_rate,
            track.isrc,
            track.play_count,
            track.hot_cue_auto_load,
            track.master_db_id,
            track.master_content_id,
            track.analyze_path,
            track.analysed_bits,
            track.content_link,
        ])?;
    }

    let mut stmt = conn.prepare(
        "INSERT INTO cue (
            cue_id, content_id, kind, colorTableIndex, cueComment, isActiveLoop,
            beatLoopNumerator, beatLoopDenominator, inUsec, outUsec,
            in150FramePerSec, out150FramePerSec, inMpegFrameNumber, outMpegFrameNumber,
            inMpegAbs, outMpegAbs, inDecodingStartFramePosition, outDecodingStartFramePosition,
            inFileOffsetInBlock, OutFileOffsetInBlock, inNumberOfSampleInBlock, outNumberOfSampleInBlock
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, ?8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)",
    )?;
    let mut cue_id = 0_u32;
    for track in &model.tracks {
        for cue in &track.cues {
            cue_id += 1;
            stmt.execute(params![
                cue_id,
                track.id,
                cue.hot_cue,
                cue.color_index.unwrap_or(0),
                cue.comment,
                cue.loop_end_ms.is_some(),
                i64::from(cue.time_ms) * 1000,
                cue.loop_end_ms.map_or(0, |ms| i64::from(ms) * 1000),
            ])?;
        }
    }

    let mut playlist_stmt = conn.prepare(
        "INSERT INTO playlist (playlist_id, sequenceNo, name, image_id, attribute, playlist_id_parent)
         VALUES (?1, ?2, ?3, 0, ?4, ?5)",
    )?;
    let mut entry_stmt =
        conn.prepare("INSERT INTO playlist_content (playlist_id, content_id, sequenceNo) VALUES (?1, ?2, ?3)")?;
    for playlist in &model.playlists {
        playlist_stmt.execute(params![
            playlist.id,
            playlist.sort_order + 1,
            playlist.name,
            playlist.is_folder,
            playlist.parent_id
        ])?;
        for (index, track_id) in playlist.track_ids.iter().enumerate() {
            entry_stmt.execute(params![playlist.id, track_id, index as u32 + 1])?;
        }
    }

    let mut history_stmt = conn.prepare(
        "INSERT INTO history (history_id, sequenceNo, name, attribute, history_id_parent) VALUES (?1, ?2, ?3, 0, 0)",
    )?;
    let mut entry_stmt =
        conn.prepare("INSERT INTO history_content (history_id, content_id, sequenceNo) VALUES (?1, ?2, ?3)")?;
    for (index, history) in model.histories.iter().enumerate() {
        history_stmt.execute(params![history.id, index as u32 + 1, history.name])?;
        for (index, track_id) in history.track_ids.iter().enumerate() {
            entry_stmt.execute(params![history.id, track_id, index as u32 + 1])?;
        }
    }

    let mut tag_stmt = conn.prepare(
        "INSERT INTO myTag (myTag_id, sequenceNo, name, attribute, myTag_id_parent) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for tag in &model.my_tags {
        tag_stmt.execute(params![tag.id, tag.sort_order + 1, tag.name, tag.is_category, tag.parent_id])?;
    }
    let mut tagged_stmt = conn.prepare("INSERT INTO myTag_content (myTag_id, content_id) VALUES (?1, ?2)")?;
    for (track_id, tag_id) in &model.track_tags {
        tagged_stmt.execute(params![tag_id, track_id])?;
    }

    conn.execute(
        "INSERT INTO property (deviceName, dbVersion, numberOfContents, createdDate, backGroundColorType, myTagMasterDBID)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            model.device.name,
            filled(&model.device.db_version).unwrap_or("1000"),
            model.tracks.len() as u32,
            model.device.created_date,
            model.device.background_color,
            model.my_tag_master_db_id
        ],
    )?;
    Ok(())
}

pub fn write(db_path: &Path, model: &StickModel) -> Result<()> {
    use crate::writer::onelibrary as legacy;

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("Cannot create {}", parent.display()))?;
    }
    for suffix in ["", "-wal", "-shm"] {
        let mut stale = db_path.as_os_str().to_owned();
        stale.push(suffix);
        let _ = std::fs::remove_file(Path::new(&stale));
    }
    let conn = legacy::open_db(db_path)?;
    legacy::create_tables(&conn).context("Cannot create the OneLibrary tables")?;
    legacy::populate_menu_item_table(&conn)?;
    legacy::populate_category_table(&conn)?;
    legacy::populate_sort_table(&conn)?;
    conn.execute_batch("BEGIN TRANSACTION")?;
    insert_rows(&conn, model).context("Cannot write the OneLibrary rows")?;
    conn.execute_batch("COMMIT")?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").context("Cannot checkpoint exportLibrary.db")?;
    Ok(())
}
