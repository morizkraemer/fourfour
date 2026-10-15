use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pioneer_usb_writer::reader::anlz::read_anlz;
use rusqlite::Connection;
use serde_json::json;

const MASTER_DB_KEY: &str = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let library = PathBuf::from(args.next().context("usage: rbref <rekordbox-library-dir> <out.json>")?);
    let out = PathBuf::from(args.next().context("missing out.json")?);

    let conn = Connection::open(library.join("master.db"))?;
    conn.execute_batch(&format!("PRAGMA key = '{MASTER_DB_KEY}';"))?;
    let mut stmt = conn.prepare(
        "SELECT c.FolderPath, c.AnalysisDataPath, c.BPM, c.Length, c.Title, a.Name
         FROM djmdContent c LEFT JOIN djmdArtist a ON a.ID = c.ArtistID
         WHERE c.rb_local_deleted = 0 OR c.rb_local_deleted IS NULL",
    )?;
    let contents = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut rows = Vec::new();
    let (mut with_grid, mut without) = (0, 0);
    for (path, anlz, bpm_x100, length, title, artist) in contents {
        let grid = if anlz.is_empty() {
            None
        } else {
            read_dat_beats(&library.join("share").join(anlz.trim_start_matches('/')))
        };
        match &grid {
            Some(_) => with_grid += 1,
            None => without += 1,
        }
        rows.push(json!({
            "path": path,
            "artist": artist,
            "title": title,
            "bpm": bpm_x100 as f64 / 100.0,
            "duration_s": length,
            "beats": grid.map(|g| g.into_iter().map(|b| json!([b.0, b.1, b.2])).collect::<Vec<_>>()),
        }));
    }
    std::fs::write(&out, serde_json::to_string(&rows)?)?;
    eprintln!("tracks {} with grid {} without {}", rows.len(), with_grid, without);
    Ok(())
}

fn read_dat_beats(dat: &Path) -> Option<Vec<(u8, u32, u32)>> {
    let result = read_anlz(dat).ok()?;
    let beats: Vec<_> = result
        .beat_grid
        .beats
        .iter()
        .map(|b| (b.bar_position, b.time_ms, b.tempo))
        .collect();
    if beats.is_empty() {
        None
    } else {
        Some(beats)
    }
}
