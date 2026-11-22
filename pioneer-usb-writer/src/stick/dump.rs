use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::types::ValueRef;

use super::device_library::{self, table, TableRows};
use super::model::{StickModel, StickTrack};
use super::{one_library, read};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DumpOptions {
    pub raw: bool,
    pub normalise: bool,
    pub pages: bool,
}

impl Default for DumpOptions {
    fn default() -> Self {
        Self { raw: false, normalise: true, pages: false }
    }
}

pub const MASKED: &str = "<masked>";

pub const IGNORED_NAMES: [&str; 6] =
    ["RBFLTR.DAT", "USBMNG.DAT", "exportLibrary.db-wal", "exportLibrary.db-shm", ".DS_Store", "DeviceLibBackup"];

pub fn is_ignored(name: &str) -> bool {
    name.starts_with("._") || IGNORED_NAMES.contains(&name)
}

pub fn fnv64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn quoted(text: &str) -> String {
    format!("{text:?}")
}

fn le32(bytes: &[u8], at: usize) -> u32 {
    bytes.get(at..at + 4).and_then(|word| word.try_into().ok()).map_or(0, u32::from_le_bytes)
}

fn le16(bytes: &[u8], at: usize) -> u16 {
    bytes.get(at..at + 2).and_then(|word| word.try_into().ok()).map_or(0, u16::from_le_bytes)
}

fn track_line(prefix: &str, model: &StickModel, track: &StickTrack) -> String {
    let extra: Vec<String> =
        track.device_library_strings.iter().map(|(slot, bytes)| format!("s{slot}={}", hex(bytes))).collect();
    format!(
        "{prefix} track id={} title={} path={} file={} artist={} album={} genre={} label={} key={} remixer={} composer={} original_artist={} artwork={} color={} rating={} bpm_x100={} secs={} year={} track_no={} disc_no={} bitrate={} rate={} depth={} size={} type={} plays={} comment={} mix={} isrc={} added={} created={} released={} anlz={} autoload={} master_db={} master_content={} link={} analysed={} cues={} extra=[{}]",
        track.id,
        quoted(&track.title),
        quoted(&track.path),
        quoted(&track.filename),
        quoted(model.artist_name(track.artist_id)),
        quoted(model.album_name(track.album_id)),
        quoted(model.genre_name(track.genre_id)),
        quoted(model.label_name(track.label_id)),
        quoted(model.key_name(track.key_id)),
        quoted(model.artist_name(track.remixer_id)),
        quoted(model.artist_name(track.composer_id)),
        quoted(model.artist_name(track.original_artist_id)),
        quoted(model.artwork_path(track.artwork_id)),
        track.color_id,
        track.rating,
        track.tempo_x100,
        track.duration_secs,
        track.year,
        track.track_number,
        track.disc_number,
        track.bitrate,
        track.sample_rate,
        track.sample_depth,
        track.file_size,
        track.file_type,
        track.play_count,
        quoted(&track.comment),
        quoted(&track.mix_name),
        quoted(&track.isrc),
        quoted(&track.date_added),
        quoted(&track.date_created),
        quoted(&track.release_date),
        quoted(&track.analyze_path),
        track.hot_cue_auto_load,
        track.master_db_id,
        track.master_content_id,
        track.content_link,
        track.analysed_bits.map_or_else(|| "-".to_owned(), |bits| bits.to_string()),
        track.cues.len(),
        extra.join(","),
    )
}

pub fn model_lines(prefix: &str, model: &StickModel, options: DumpOptions) -> Vec<String> {
    let mut lines = Vec::new();
    let device = &model.device;
    lines.push(format!(
        "{prefix} device name={} db_version={} created={} background={} tracks={}",
        quoted(&device.name),
        quoted(&device.db_version),
        if options.normalise { MASKED.to_owned() } else { quoted(&device.created_date) },
        device.background_color,
        device.track_count,
    ));
    lines.push(format!(
        "{prefix} counts tracks={} artists={} albums={} genres={} labels={} keys={} colors={} artworks={} playlists={} histories={}",
        model.tracks.len(),
        model.artists.len(),
        model.albums.len(),
        model.genres.len(),
        model.labels.len(),
        model.keys.len(),
        model.colors.len(),
        model.artworks.len(),
        model.playlists.len(),
        model.histories.len(),
    ));

    let mut tracks: Vec<&StickTrack> = model.tracks.iter().collect();
    tracks.sort_by_key(|track| track.id);
    lines.extend(tracks.into_iter().map(|track| track_line(prefix, model, track)));

    let mut named = |kind: &str, rows: &[super::model::StickName]| {
        let mut rows: Vec<_> = rows.iter().collect();
        rows.sort_by_key(|row| row.id);
        lines.extend(rows.into_iter().map(|row| format!("{prefix} {kind} id={} name={}", row.id, quoted(&row.name))));
    };
    named("artist", &model.artists);
    named("genre", &model.genres);
    named("label", &model.labels);
    named("key", &model.keys);
    named("color", &model.colors);
    named("artwork", &model.artworks);

    let mut albums: Vec<_> = model.albums.iter().collect();
    albums.sort_by_key(|album| album.id);
    lines.extend(albums.into_iter().map(|album| {
        format!(
            "{prefix} album id={} name={} artist={}",
            album.id,
            quoted(&album.name),
            quoted(model.artist_name(album.artist_id))
        )
    }));

    let mut playlists: Vec<_> = model.playlists.iter().collect();
    playlists.sort_by_key(|playlist| playlist.id);
    lines.extend(playlists.into_iter().map(|playlist| {
        format!(
            "{prefix} playlist id={} parent={} order={} folder={} name={} tracks={:?}",
            playlist.id,
            playlist.parent_id,
            playlist.sort_order,
            playlist.is_folder,
            quoted(&playlist.name),
            playlist.track_ids
        )
    }));

    let mut tags: Vec<_> = model.my_tags.iter().collect();
    tags.sort_by_key(|tag| tag.id);
    lines.extend(tags.into_iter().map(|tag| {
        format!(
            "{prefix} mytag id={} parent={} order={} category={} name={}",
            tag.id,
            tag.parent_id,
            tag.sort_order,
            tag.is_category,
            quoted(&tag.name)
        )
    }));
    let mut tagged = model.track_tags.clone();
    tagged.sort();
    lines.extend(tagged.into_iter().map(|(track, tag)| format!("{prefix} tagged track={track} tag={tag}")));
    if model.my_tag_master_db_id != 0 {
        lines.push(format!("{prefix} mytag-master {}", model.my_tag_master_db_id));
    }

    let mut histories: Vec<_> = model.histories.iter().collect();
    histories.sort_by_key(|history| history.id);
    lines.extend(histories.into_iter().map(|history| {
        format!("{prefix} history id={} name={} tracks={:?}", history.id, quoted(&history.name), history.track_ids)
    }));
    lines
}

fn row_key(table: u32, row: &[u8], index: usize) -> String {
    match table {
        table::TRACKS => format!("{:010}", le32(row, 0x48)),
        table::ARTISTS => format!("{:010}", le32(row, 4)),
        table::ALBUMS => format!("{:010}", le32(row, 0x0c)),
        table::GENRES | table::LABELS | table::KEYS | table::ARTWORK | table::HISTORY_PLAYLISTS => {
            format!("{:010}", le32(row, 0))
        }
        table::COLORS => format!("{:010}", le16(row, 5)),
        table::PLAYLIST_TREE => format!("{:010}", le32(row, 12)),
        table::PLAYLIST_ENTRIES => format!("{:010}.{:010}", le32(row, 8), le32(row, 0)),
        table::HISTORY_ENTRIES => format!("{:010}.{:010}", le32(row, 4), le32(row, 8)),
        table::COLUMNS | table::CATEGORIES | table::SORTS => format!("{:010}", le16(row, 0)),
        _ => format!("{index:010}"),
    }
}

pub fn normalised_row(table: u32, row: &[u8]) -> Vec<u8> {
    let mut row = row.to_vec();
    if matches!(table, table::TRACKS | table::ARTISTS | table::ALBUMS | table::PROPERTY) && row.len() >= 4 {
        row[2] = 0;
        row[3] = 0;
    }
    let mut gaps: Vec<(usize, usize)> = Vec::new();
    match table {
        table::TRACKS => {
            let mut end = device_library::TRACK_FIXED_LEN + device_library::TRACK_STRINGS * 2;
            for slot in 0..device_library::TRACK_STRINGS {
                let at = usize::from(le16(&row, device_library::TRACK_FIXED_LEN + slot * 2));
                if at > end {
                    gaps.push((end, at));
                }
                end = end.max(at + device_library::encoded_string_len(&row, at));
            }
        }
        table::ARTISTS if le16(&row, 0) & 0x04 == 0 => gaps.push((0x0a, usize::from(row.get(0x09).copied().unwrap_or(0)))),
        table::ALBUMS if le16(&row, 0) & 0x04 == 0 => gaps.push((0x16, usize::from(row.get(0x15).copied().unwrap_or(0)))),
        _ => {}
    }
    for (from, to) in gaps {
        for byte in row.iter_mut().take(to).skip(from) {
            *byte = 0;
        }
    }
    if table == table::PROPERTY {
        let date_len = device_library::encoded_string_len(&row, 0x0c);
        for byte in row.iter_mut().skip(0x0d).take(date_len.saturating_sub(1)) {
            *byte = b'#';
        }
    }
    row
}

pub fn raw_row_lines(prefix: &str, tables: &[TableRows], normalise: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for rows in tables {
        let mut keyed: Vec<(String, Vec<u8>)> = rows
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let bytes = if normalise { normalised_row(rows.table, row) } else { row.clone() };
                (row_key(rows.table, row, index), bytes)
            })
            .collect();
        keyed.sort();
        lines.extend(
            keyed.into_iter().map(|(key, row)| format!("{prefix} raw {} {key} {}", table::name(rows.table), hex(&row))),
        );
    }
    lines
}

fn page_lines(prefix: &str, bytes: &[u8], normalise: bool) -> Vec<String> {
    let page_size = le32(bytes, 4) as usize;
    let table_count = le32(bytes, 8) as usize;
    let mut lines = Vec::new();
    if page_size == 0 {
        return lines;
    }
    for index in 0..table_count.min(64) {
        let at = 28 + index * 16;
        let (kind, first, last) = (le32(bytes, at), le32(bytes, at + 8), le32(bytes, at + 12));
        let mut page = first;
        let mut nth = 0;
        while let Some(body) = bytes.get(page as usize * page_size..(page as usize + 1) * page_size) {
            let packed = le32(body, 0x18) & 0x00ff_ffff;
            let mut line = format!(
                "{prefix} page {} n={nth} flags={:#04x} rows={} row_bits={:#x} free={} used={} w20={:#06x} w22={:#06x} w24={:#06x} w26={:#06x}",
                table::name(kind),
                body[0x1b],
                packed & 0x1fff,
                packed >> 13,
                le16(body, 0x1c),
                le16(body, 0x1e),
                le16(body, 0x20),
                le16(body, 0x22),
                le16(body, 0x24),
                le16(body, 0x26),
            );
            if !normalise {
                let _ = write!(line, " index={page} next={} seq={}", le32(body, 0x0c), le32(body, 0x10));
            }
            lines.push(line);
            let next = le32(body, 0x0c);
            if page == last || next == page || next == 0 || nth > 100_000 {
                break;
            }
            page = next;
            nth += 1;
        }
    }
    lines
}

pub fn device_library_lines(prefix: &str, bytes: &[u8], options: DumpOptions) -> Result<Vec<String>> {
    let tables = device_library::table_rows(bytes)?;
    let mut lines = vec![format!("{prefix} header page_size={} tables={}", le32(bytes, 4), le32(bytes, 8))];
    if !options.normalise {
        lines.push(format!(
            "{prefix} header next_unused={} sequence={} pages={}",
            le32(bytes, 0x0c),
            le32(bytes, 0x14),
            bytes.len() / (le32(bytes, 4).max(1) as usize)
        ));
    }
    lines.extend(tables.iter().map(|rows| format!("{prefix} table {} rows={}", table::name(rows.table), rows.rows.len())));
    if options.pages {
        lines.extend(page_lines(prefix, bytes, options.normalise));
    }
    lines.extend(model_lines(prefix, &device_library::model_from_tables(&tables), options));
    if options.raw {
        lines.extend(raw_row_lines(prefix, &tables, options.normalise));
    }
    Ok(lines)
}

fn sql_value(value: ValueRef) -> String {
    match value {
        ValueRef::Null => "NULL".to_owned(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) => quoted(&String::from_utf8_lossy(v)),
        ValueRef::Blob(v) => format!("x{}", hex(v)),
    }
}

pub fn one_library_lines(prefix: &str, db_path: &Path, options: DumpOptions) -> Result<Vec<String>> {
    let opened = one_library::open_copy(db_path)?;
    let conn = &opened.conn;
    let mut lines = model_lines(prefix, &one_library::read_connection(conn)?, options);
    if !options.raw {
        return Ok(lines);
    }
    let mut stmt = conn.prepare("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name")?;
    let objects = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?.unwrap_or_default()))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (kind, name, sql) in &objects {
        let sql: Vec<&str> = sql.split_whitespace().collect();
        lines.push(format!("{prefix} schema {kind} {name} {}", sql.join(" ")));
    }
    for (_, name, _) in objects.iter().filter(|(kind, _, _)| kind == "table") {
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{name}\""))?;
        let columns: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
        let mut rows = stmt.query([])?;
        let mut table_lines = Vec::new();
        while let Some(row) = rows.next()? {
            let mut line = format!("{prefix} row {name}");
            for (index, column) in columns.iter().enumerate() {
                let masked = options.normalise && name == "property" && column == "createdDate";
                let value = if masked { MASKED.to_owned() } else { sql_value(row.get_ref(index)?) };
                let _ = write!(line, " {column}={value}");
            }
            table_lines.push(line);
        }
        table_lines.sort();
        lines.extend(table_lines);
    }
    Ok(lines)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot list {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn anlz_lines(rel: &str, bytes: &[u8]) -> Vec<String> {
    match rbl_anlz::parse(bytes) {
        Ok(anlz) => {
            let mut lines = vec![format!("anlz {rel} sections={} size={} fnv={:016x}", anlz.sections.len(), bytes.len(), fnv64(bytes))];
            lines.extend(anlz.sections.iter().enumerate().map(|(index, section)| {
                let mut whole = section.header.clone();
                whole.extend_from_slice(&section.payload);
                format!(
                    "anlz {rel} {index:02} {} hdr={} len={} fnv={:016x}",
                    section.tag,
                    section.len_header(),
                    section.len_tag(),
                    fnv64(&whole)
                )
            }));
            lines
        }
        Err(error) => vec![format!("anlz {rel} unreadable ({error}) size={} fnv={:016x}", bytes.len(), fnv64(bytes))],
    }
}

pub fn dump_stick(root: &Path, options: DumpOptions) -> Result<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut ignored: Vec<String> = Vec::new();
    let Some(pioneer) = read::pioneer_dir(root) else {
        return Ok("stick no-library\n".to_owned());
    };
    let pioneer_name = pioneer.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    lines.push(format!("stick pioneer_dir={pioneer_name}"));

    let mut files = Vec::new();
    walk(&pioneer, &mut files)?;
    let mut rest: BTreeMap<String, PathBuf> = BTreeMap::new();
    for path in files {
        let rel = relative(&pioneer, &path);
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        if is_ignored(&name) {
            ignored.push(rel);
            continue;
        }
        rest.insert(rel, path);
    }

    if let Some(path) = rest.remove("rekordbox/export.pdb") {
        let bytes = std::fs::read(&path)?;
        lines.extend(device_library_lines("pdb", &bytes, options)?);
    }
    if let Some(path) = rest.remove("rekordbox/exportExt.pdb") {
        let bytes = std::fs::read(&path)?;
        let tables = super::ext_library::table_rows(&bytes)?;
        for rows in &tables {
            lines.push(format!("ext table {} rows={}", rows.table, rows.rows.len()));
        }
        if options.raw {
            for rows in &tables {
                let mut encoded: Vec<String> = rows
                    .rows
                    .iter()
                    .map(|row| {
                        let row = if options.normalise {
                            super::ext_library::normalised_row(rows.table, row)
                        } else {
                            row.clone()
                        };
                        format!("ext raw {} {}", rows.table, hex(&row))
                    })
                    .collect();
                encoded.sort();
                lines.extend(encoded);
            }
        }
        if options.pages {
            lines.extend(page_lines("ext", &bytes, options.normalise));
        }
    }
    if let Some(path) = rest.remove("rekordbox/exportLibrary.db") {
        lines.extend(one_library_lines("onelib", &path, options)?);
    }

    for (rel, path) in &rest {
        let bytes = std::fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?;
        let is_anlz = rel.starts_with("USBANLZ/")
            && matches!(path.extension().and_then(|ext| ext.to_str()), Some("DAT" | "EXT" | "2EX"));
        if is_anlz {
            lines.extend(anlz_lines(rel, &bytes));
        } else {
            lines.push(format!("file {rel} size={} fnv={:016x}", bytes.len(), fnv64(&bytes)));
        }
    }

    let contents = root.join("Contents");
    if contents.is_dir() {
        let mut audio = Vec::new();
        walk(&contents, &mut audio)?;
        for path in audio {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            if is_ignored(&name) {
                ignored.push(format!("Contents/{}", relative(&contents, &path)));
                continue;
            }
            let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            lines.push(format!("audio Contents/{} size={size}", relative(&contents, &path)));
        }
    }

    lines.extend(ignored.into_iter().map(|rel| format!("not-compared {rel}")));
    let mut out = lines.join("\n");
    out.push('\n');
    Ok(out)
}

pub fn diff_dumps(left: &str, right: &str) -> Vec<String> {
    let mut left_count: BTreeMap<&str, i64> = BTreeMap::new();
    for line in left.lines() {
        *left_count.entry(line).or_default() += 1;
    }
    let mut right_count: BTreeMap<&str, i64> = BTreeMap::new();
    for line in right.lines() {
        *right_count.entry(line).or_default() += 1;
    }
    let mut out = Vec::new();
    for line in left.lines() {
        let other = right_count.entry(line).or_default();
        if *other > 0 {
            *other -= 1;
        } else {
            out.push(format!("-{line}"));
        }
    }
    for line in right.lines() {
        let other = left_count.entry(line).or_default();
        if *other > 0 {
            *other -= 1;
        } else {
            out.push(format!("+{line}"));
        }
    }
    out
}

pub fn diff_sticks(left: &Path, right: &Path, options: DumpOptions) -> Result<Vec<String>> {
    Ok(diff_dumps(&dump_stick(left, options)?, &dump_stick(right, options)?))
}
