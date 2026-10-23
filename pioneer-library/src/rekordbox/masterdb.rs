use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, Row};

use pioneer_usb_writer::models::Track;

use super::model::{ImportedCue, SourceKind, SourceLibrary, SourcePlaylist, SourceTrack};
use super::remap::PathRemap;

/// Rekordbox's published SQLCipher key for `master.db`; the same constant lives
/// privately in `pioneer_usb_writer::reader::masterdb`.
const MASTER_DB_KEY: &str = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";

pub(super) fn text(row: &Row, idx: usize) -> rusqlite::Result<String> {
    Ok(match row.get_ref(idx)? {
        ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        ValueRef::Integer(n) => n.to_string(),
        ValueRef::Real(f) => f.to_string(),
        _ => String::new(),
    })
}

pub(super) fn int(row: &Row, idx: usize) -> rusqlite::Result<i64> {
    Ok(match row.get_ref(idx)? {
        ValueRef::Integer(n) => n,
        ValueRef::Real(f) => f as i64,
        ValueRef::Text(t) => String::from_utf8_lossy(t).trim().parse().unwrap_or(0),
        _ => 0,
    })
}

fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Failed to open master.db at {}", db_path.display()))?;
    conn.execute_batch(&format!("PRAGMA key = '{MASTER_DB_KEY}';"))
        .context("Failed to set SQLCipher key for master.db")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .context("master.db decryption failed: not a Rekordbox master.db or wrong key")?;
    Ok(conn)
}

fn name_table(conn: &Connection, sql: &str) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare(sql).with_context(|| format!("Failed: {sql}"))?;
    let rows = stmt.query_map([], |r| Ok((text(r, 0)?, text(r, 1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn non_deleted(alias: &str) -> String {
    format!("({alias}.rb_local_deleted = 0 OR {alias}.rb_local_deleted IS NULL)")
}

fn star_rating(raw: i64) -> Option<u8> {
    let stars = if raw > 5 { raw / 51 } else { raw };
    (stars > 0).then_some(stars.min(5) as u8)
}

fn cue_color(color_table_index: i64) -> Option<String> {
    (color_table_index > 0).then(|| format!("table:{color_table_index}"))
}

/// Load a desktop Rekordbox library. `share_dir` is the folder that contains
/// `PIONEER/USBANLZ` and `PIONEER/Artwork`; it defaults to `share/` next to
/// `master.db`.
pub fn load(db_path: &Path, share_dir: Option<&Path>) -> Result<SourceLibrary> {
    let conn = open(db_path)?;
    let share = share_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| db_path.parent().unwrap_or(Path::new(".")).join("share"));

    let artists = name_table(&conn, "SELECT ID, Name FROM djmdArtist")?;
    let albums = name_table(&conn, "SELECT ID, Name FROM djmdAlbum")?;
    let genres = name_table(&conn, "SELECT ID, Name FROM djmdGenre")?;
    let labels = name_table(&conn, "SELECT ID, Name FROM djmdLabel")?;
    let keys = name_table(&conn, "SELECT ID, ScaleName FROM djmdKey")?;
    let colors = name_table(&conn, "SELECT ID, Commnt FROM djmdColor")?;

    let mut cues_by_content = load_cues(&conn)?;

    let mut tracks = Vec::new();
    {
        let sql = format!(
            "SELECT c.ID, c.FolderPath, c.Title, c.ArtistID, c.AlbumID, c.GenreID, c.LabelID,
                    c.RemixerID, c.KeyID, c.Commnt, c.ReleaseYear, c.DiscNo, c.TrackNo, c.BPM,
                    c.Length, c.SampleRate, c.BitRate, c.FileSize, c.ImagePath,
                    c.AnalysisDataPath, c.Rating, c.ColorID
             FROM djmdContent c WHERE {} ORDER BY c.ID",
            non_deleted("c")
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            let source_id = text(r, 0)?;
            let path = text(r, 1)?;
            let lookup = |map: &HashMap<String, String>, idx: usize| -> rusqlite::Result<String> {
                Ok(map.get(&text(r, idx)?).cloned().unwrap_or_default())
            };
            let image = text(r, 18)?;
            let analysis = text(r, 19)?;
            let color_id = text(r, 21)?;
            let track = Track {
                id: 0,
                source_path: PathBuf::new(),
                usb_path: String::new(),
                title: text(r, 2)?,
                artist: lookup(&artists, 3)?,
                album: lookup(&albums, 4)?,
                genre: lookup(&genres, 5)?,
                label: lookup(&labels, 6)?,
                remixer: lookup(&artists, 7)?,
                key: lookup(&keys, 8)?,
                comment: text(r, 9)?,
                year: int(r, 10)?.clamp(0, u16::MAX as i64) as u16,
                disc_number: int(r, 11)?.clamp(0, u16::MAX as i64) as u16,
                track_number: int(r, 12)?.max(0) as u32,
                tempo: int(r, 13)?.max(0) as u32,
                duration_secs: int(r, 14)? as f64,
                sample_rate: int(r, 15)?.max(0) as u32,
                bitrate: int(r, 16)?.max(0) as u32,
                file_size: int(r, 17)?.max(0) as u64,
                artwork: None,
            };
            let color = (color_id != "0" && !color_id.is_empty())
                .then(|| colors.get(&color_id).cloned())
                .flatten()
                .filter(|c| !c.is_empty());
            tracks.push(SourceTrack {
                cues: cues_by_content.remove(&source_id).unwrap_or_default(),
                source_id,
                path,
                track,
                rating: star_rating(int(r, 20)?),
                color,
                anlz_dat: (!analysis.is_empty()).then(|| share.join(analysis.trim_start_matches('/'))),
                grid: None,
                artwork: (!image.is_empty()).then(|| share.join(image.trim_start_matches('/'))),
            });
        }
    }

    let (playlists, skipped_smart) = load_playlists(&conn)?;
    let implied_remap = drive_swap(&conn);

    Ok(SourceLibrary {
        kind: SourceKind::MasterDb,
        tracks,
        playlists,
        implied_remap,
        skipped_smart_playlists: skipped_smart,
    })
}

fn load_cues(conn: &Connection) -> Result<HashMap<String, Vec<ImportedCue>>> {
    let sql = format!(
        "SELECT c.ContentID, c.InMsec, c.OutMsec, c.Kind, c.ColorTableIndex, c.ActiveLoop, c.Comment
         FROM djmdCue c WHERE {} ORDER BY c.ContentID, c.InMsec",
        non_deleted("c")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut out: HashMap<String, Vec<ImportedCue>> = HashMap::new();
    while let Some(r) = rows.next()? {
        let in_ms = int(r, 1)?;
        if in_ms < 0 {
            continue;
        }
        let out_ms = int(r, 2)?;
        let is_loop = int(r, 5)? == 1 && out_ms > in_ms;
        out.entry(text(r, 0)?).or_default().push(ImportedCue {
            hot_cue_number: int(r, 3)?.max(0) as u32,
            time_ms: in_ms as u32,
            loop_time_ms: is_loop.then_some(out_ms as u32),
            color: cue_color(int(r, 4)?),
            name: text(r, 6)?,
        });
    }
    Ok(out)
}

fn load_playlists(conn: &Connection) -> Result<(Vec<SourcePlaylist>, u32)> {
    let sql = format!(
        "SELECT p.ID, p.ParentID, p.Name, p.Attribute, p.Seq FROM djmdPlaylist p WHERE {} ORDER BY p.Seq",
        non_deleted("p")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut playlists = Vec::new();
    let mut skipped_smart = 0;
    while let Some(r) = rows.next()? {
        let attribute = int(r, 3)?;
        if attribute == 4 {
            skipped_smart += 1;
            continue;
        }
        let parent = text(r, 1)?;
        playlists.push(SourcePlaylist {
            source_id: text(r, 0)?,
            parent_id: (!parent.is_empty() && parent != "root").then_some(parent),
            name: text(r, 2)?,
            is_folder: attribute == 1,
            sequence: int(r, 4)?,
            track_ids: Vec::new(),
        });
    }
    drop(rows);
    drop(stmt);

    let sql = format!(
        "SELECT s.PlaylistID, s.ContentID FROM djmdSongPlaylist s WHERE {} ORDER BY s.PlaylistID, s.TrackNo",
        non_deleted("s")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut members: HashMap<String, Vec<String>> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        members.entry(text(r, 0)?).or_default().push(text(r, 1)?);
    }
    for playlist in &mut playlists {
        playlist.track_ids = members.remove(&playlist.source_id).unwrap_or_default();
    }
    Ok((playlists, skipped_smart))
}

fn drive_swap(conn: &Connection) -> Option<PathRemap> {
    conn.query_row(
        "SELECT BaseDBDrive, CurrentDBDrive FROM djmdProperty LIMIT 1",
        [],
        |r| Ok((text(r, 0)?, text(r, 1)?)),
    )
    .ok()
    .filter(|(base, current)| !base.is_empty() && !current.is_empty() && base != current)
    .map(|(from_prefix, to_prefix)| PathRemap { from_prefix, to_prefix })
}
