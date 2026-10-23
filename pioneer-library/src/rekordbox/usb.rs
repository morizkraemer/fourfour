use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;

use pioneer_usb_writer::models::Track;
use pioneer_usb_writer::reader;

use super::masterdb::{int, text};
use super::model::{ImportedCue, SourceKind, SourceLibrary, SourcePlaylist, SourceTrack};

/// Pioneer's published SQLCipher key for `exportLibrary.db`; the same constant
/// lives privately in `pioneer_usb_writer::reader::usb`.
const ONELIBRARY_KEY: &str = "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls";

fn open(usb_root: &Path) -> Result<Connection> {
    let db_path = usb_root.join("PIONEER/rekordbox/exportLibrary.db");
    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Failed to open {}", db_path.display()))?;
    conn.execute_batch(&format!("PRAGMA key = '{ONELIBRARY_KEY}';"))?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .context("exportLibrary.db decryption failed")?;
    Ok(conn)
}

struct ContentExtras {
    analysis: String,
    rating: Option<u8>,
    color: Option<String>,
}

fn content_extras(conn: &Connection) -> Result<HashMap<u32, ContentExtras>> {
    let colors: HashMap<String, String> = {
        let mut stmt = conn.prepare("SELECT color_id, name FROM color")?;
        let rows = stmt.query_map([], |r| Ok((text(r, 0)?, text(r, 1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut stmt = conn.prepare("SELECT content_id, analysisDataFilePath, rating, color_id FROM content")?;
    let mut rows = stmt.query([])?;
    let mut out = HashMap::new();
    while let Some(r) = rows.next()? {
        let rating = int(r, 2)?;
        let color_id = text(r, 3)?;
        out.insert(
            int(r, 0)? as u32,
            ContentExtras {
                analysis: text(r, 1)?,
                rating: (rating > 0).then_some(rating.min(5) as u8),
                color: colors.get(&color_id).filter(|c| !c.is_empty()).cloned(),
            },
        );
    }
    Ok(out)
}

fn cues(conn: &Connection) -> Result<HashMap<u32, Vec<ImportedCue>>> {
    let mut stmt = conn.prepare(
        "SELECT content_id, kind, colorTableIndex, cueComment, isActiveLoop, inUsec, outUsec
         FROM cue ORDER BY content_id, inUsec",
    )?;
    let mut rows = stmt.query([])?;
    let mut out: HashMap<u32, Vec<ImportedCue>> = HashMap::new();
    while let Some(r) = rows.next()? {
        let in_ms = int(r, 5)? / 1000;
        let out_ms = int(r, 6)? / 1000;
        let color_index = int(r, 2)?;
        out.entry(int(r, 0)? as u32).or_default().push(ImportedCue {
            hot_cue_number: int(r, 1)?.max(0) as u32,
            time_ms: in_ms.max(0) as u32,
            loop_time_ms: (int(r, 4)? == 1 && out_ms > in_ms).then_some(out_ms as u32),
            color: (color_index > 0).then(|| format!("table:{color_index}")),
            name: text(r, 3)?,
        });
    }
    Ok(out)
}

fn playlist_tree(conn: &Connection) -> Result<Vec<SourcePlaylist>> {
    let mut stmt = conn.prepare(
        "SELECT playlist_id, name, attribute, playlist_id_parent, sequenceNo
         FROM playlist WHERE playlist_id != 0 ORDER BY sequenceNo",
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let parent = int(r, 3)?;
        out.push(SourcePlaylist {
            source_id: int(r, 0)?.to_string(),
            parent_id: (parent != 0).then(|| parent.to_string()),
            name: text(r, 1)?,
            is_folder: int(r, 2)? == 1,
            sequence: int(r, 4)?,
            track_ids: Vec::new(),
        });
    }
    Ok(out)
}

/// Load a Pioneer USB stick through its OneLibrary `exportLibrary.db`. Sticks
/// that only carry the legacy `export.pdb` have no reader and are rejected.
pub fn load(usb_root: &Path) -> Result<SourceLibrary> {
    let Some(state) = reader::read_usb_state(usb_root)? else {
        if usb_root.join("PIONEER/rekordbox/export.pdb").exists() {
            bail!("This stick only has the legacy export.pdb; there is no reader for it yet");
        }
        bail!("No PIONEER/rekordbox/exportLibrary.db found under {}", usb_root.display());
    };
    let conn = open(usb_root)?;
    let mut extras = content_extras(&conn)?;
    let mut cues = cues(&conn)?;
    let mut playlists = playlist_tree(&conn)?;

    let members: HashMap<String, Vec<String>> = state
        .playlists
        .iter()
        .map(|p| (p.id.to_string(), p.track_ids.iter().map(u32::to_string).collect()))
        .collect();
    for playlist in &mut playlists {
        playlist.track_ids = members.get(&playlist.source_id).cloned().unwrap_or_default();
    }

    let artwork_dir = usb_root.join("PIONEER/Artwork/00001");
    let tracks = state
        .tracks
        .into_iter()
        .map(|t| {
            let extra = extras.remove(&t.id);
            let absolute = usb_root.join(t.usb_path.trim_start_matches('/'));
            let artwork = t
                .has_artwork
                .then(|| artwork_dir.join(format!("a{}_m.jpg", t.id)))
                .filter(|p| p.exists());
            SourceTrack {
                source_id: t.id.to_string(),
                path: absolute.to_string_lossy().to_string(),
                track: Track {
                    id: 0,
                    source_path: Default::default(),
                    usb_path: t.usb_path,
                    title: t.title,
                    artist: t.artist,
                    album: t.album,
                    genre: t.genre,
                    label: t.label,
                    remixer: t.remixer,
                    comment: t.comment,
                    year: t.year,
                    disc_number: t.disc_number,
                    track_number: t.track_number,
                    tempo: t.tempo,
                    key: t.key,
                    duration_secs: t.duration_secs,
                    sample_rate: t.sample_rate,
                    bitrate: t.bitrate,
                    file_size: t.file_size,
                    artwork: None,
                },
                rating: extra.as_ref().and_then(|e| e.rating),
                color: extra.as_ref().and_then(|e| e.color.clone()),
                anlz_dat: extra
                    .as_ref()
                    .filter(|e| !e.analysis.is_empty())
                    .map(|e| usb_root.join(e.analysis.trim_start_matches('/'))),
                cues: cues.remove(&t.id).unwrap_or_default(),
                grid: None,
                artwork,
            }
        })
        .collect();

    Ok(SourceLibrary {
        kind: SourceKind::Usb,
        tracks,
        playlists,
        implied_remap: None,
        skipped_smart_playlists: 0,
    })
}
