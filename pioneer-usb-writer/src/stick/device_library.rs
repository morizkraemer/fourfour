use std::collections::BTreeMap;

use anyhow::{Context, Result};
use rbl_pdb::Pdb;

use super::model::{
    StickAlbum, StickDevice, StickHistory, StickMenu, StickModel, StickName, StickPlaylist, StickTrack,
};

pub mod table {
    pub const TRACKS: u32 = 0;
    pub const GENRES: u32 = 1;
    pub const ARTISTS: u32 = 2;
    pub const ALBUMS: u32 = 3;
    pub const LABELS: u32 = 4;
    pub const KEYS: u32 = 5;
    pub const COLORS: u32 = 6;
    pub const PLAYLIST_TREE: u32 = 7;
    pub const PLAYLIST_ENTRIES: u32 = 8;
    pub const HISTORY_PLAYLISTS: u32 = 11;
    pub const HISTORY_ENTRIES: u32 = 12;
    pub const ARTWORK: u32 = 13;
    pub const COLUMNS: u32 = 16;
    pub const CATEGORIES: u32 = 17;
    pub const SORTS: u32 = 18;
    pub const PROPERTY: u32 = 19;
    pub const COUNT: u32 = 20;

    pub fn name(table: u32) -> &'static str {
        match table {
            TRACKS => "tracks",
            GENRES => "genres",
            ARTISTS => "artists",
            ALBUMS => "albums",
            LABELS => "labels",
            KEYS => "keys",
            COLORS => "colors",
            PLAYLIST_TREE => "playlist_tree",
            PLAYLIST_ENTRIES => "playlist_entries",
            HISTORY_PLAYLISTS => "history_playlists",
            HISTORY_ENTRIES => "history_entries",
            ARTWORK => "artwork",
            COLUMNS => "columns",
            CATEGORIES => "categories",
            SORTS => "sorts",
            PROPERTY => "property",
            _ => "unused",
        }
    }
}

pub const TRACK_FIXED_LEN: usize = 0x5e;
pub const TRACK_STRINGS: usize = 21;

pub mod slot {
    pub const ISRC: usize = 0;
    pub const HOT_CUE_AUTO_LOAD: usize = 7;
    pub const DATE_CREATED: usize = 10;
    pub const RELEASE_DATE: usize = 11;
    pub const MIX_NAME: usize = 12;
    pub const ANALYZE_PATH: usize = 14;
    pub const DATE_ADDED: usize = 15;
    pub const COMMENT: usize = 16;
    pub const TITLE: usize = 17;
    pub const FILENAME: usize = 19;
    pub const FILE_PATH: usize = 20;

    pub const MODELLED: [usize; 11] = [
        ISRC,
        HOT_CUE_AUTO_LOAD,
        DATE_CREATED,
        RELEASE_DATE,
        MIX_NAME,
        ANALYZE_PATH,
        DATE_ADDED,
        COMMENT,
        TITLE,
        FILENAME,
        FILE_PATH,
    ];
}

pub(crate) const EMPTY_STRING: [u8; 1] = [0x03];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRows {
    pub table: u32,
    pub rows: Vec<Vec<u8>>,
}

fn u8_at(row: &[u8], at: usize) -> u8 {
    row.get(at).copied().unwrap_or(0)
}

fn u16_at(row: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([u8_at(row, at), u8_at(row, at + 1)])
}

fn u32_at(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([u8_at(row, at), u8_at(row, at + 1), u8_at(row, at + 2), u8_at(row, at + 3)])
}

pub(crate) fn encoded_string_len(bytes: &[u8], at: usize) -> usize {
    match u8_at(bytes, at) {
        0x40 | 0x90 => usize::from(u16_at(bytes, at + 1)).max(4),
        short if short % 2 == 1 => usize::from(short.saturating_sub(1) / 2).max(1),
        _ => 1,
    }
}

pub(crate) fn decode_string(encoded: &[u8]) -> String {
    match u8_at(encoded, 0) {
        0x40 => {
            let len = usize::from(u16_at(encoded, 1)).min(encoded.len());
            String::from_utf8_lossy(encoded.get(4..len).unwrap_or(&[])).into_owned()
        }
        0x90 => {
            let len = usize::from(u16_at(encoded, 1)).min(encoded.len());
            let units: Vec<u16> = encoded
                .get(4..len)
                .unwrap_or(&[])
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        }
        short if short % 2 == 1 => {
            let len = usize::from(short.saturating_sub(1) / 2).min(encoded.len());
            String::from_utf8_lossy(encoded.get(1..len).unwrap_or(&[])).into_owned()
        }
        _ => String::new(),
    }
}

fn string_in(row: &[u8], at: usize) -> String {
    if at == 0 || at >= row.len() {
        return String::new();
    }
    decode_string(&row[at..])
}

fn decode_isrc(encoded: &[u8]) -> String {
    if u8_at(encoded, 0) == 0x90 && u8_at(encoded, 4) == 0x03 {
        let len = usize::from(u16_at(encoded, 1)).min(encoded.len());
        let text = encoded.get(5..len).unwrap_or(&[]);
        let text = text.strip_suffix(&[0]).unwrap_or(text);
        return String::from_utf8_lossy(text).into_owned();
    }
    decode_string(encoded)
}

fn row_len(table: u32, bytes: &[u8], at: usize) -> usize {
    let string_end = |ofs: usize| ofs + encoded_string_len(bytes, at + ofs);
    match table {
        table::TRACKS => (0..TRACK_STRINGS)
            .map(|slot| string_end(usize::from(u16_at(bytes, at + TRACK_FIXED_LEN + slot * 2))))
            .max()
            .unwrap_or(TRACK_FIXED_LEN)
            .max(TRACK_FIXED_LEN + TRACK_STRINGS * 2),
        table::GENRES | table::LABELS | table::ARTWORK | table::HISTORY_PLAYLISTS | table::COLUMNS => string_end(4),
        table::KEYS | table::COLORS => string_end(8),
        table::ARTISTS => {
            let ofs = if u16_at(bytes, at) & 0x04 != 0 {
                usize::from(u16_at(bytes, at + 0x0a))
            } else {
                usize::from(u8_at(bytes, at + 0x09))
            };
            string_end(ofs)
        }
        table::ALBUMS => {
            let ofs = if u16_at(bytes, at) & 0x04 != 0 {
                usize::from(u16_at(bytes, at + 0x16))
            } else {
                usize::from(u8_at(bytes, at + 0x15))
            };
            string_end(ofs)
        }
        table::PLAYLIST_TREE => string_end(20),
        table::PLAYLIST_ENTRIES | table::HISTORY_ENTRIES => 12,
        table::CATEGORIES | table::SORTS => 8,
        table::PROPERTY => {
            let date_end = string_end(0x0c);
            let version_end = date_end + 2 + encoded_string_len(bytes, at + date_end + 2);
            let name_end = version_end + encoded_string_len(bytes, at + version_end);
            (name_end + 8).div_ceil(4) * 4
        }
        _ => 0,
    }
}

pub fn table_rows(bytes: &[u8]) -> Result<Vec<TableRows>> {
    let pdb = Pdb::parse(bytes).context("export.pdb is not a DeviceSQL database")?;
    let mut tables = Vec::with_capacity(pdb.tables.len());
    for (index, table_ref) in pdb.tables.iter().enumerate() {
        let table = u32::from_le_bytes(
            bytes.get(28 + index * 16..32 + index * 16).and_then(|word| word.try_into().ok()).unwrap_or([0; 4]),
        );
        let rows = pdb
            .rows(table_ref)
            .into_iter()
            .filter_map(|row| {
                let len = row_len(table, bytes, row.offset);
                bytes.get(row.offset..row.offset + len).map(<[u8]>::to_vec)
            })
            .filter(|row| !row.is_empty())
            .collect();
        tables.push(TableRows { table, rows });
    }
    Ok(tables)
}

pub fn decode_track(row: &[u8]) -> StickTrack {
    let slot_bytes = |slot: usize| -> &[u8] {
        let at = usize::from(u16_at(row, TRACK_FIXED_LEN + slot * 2));
        let len = encoded_string_len(row, at);
        row.get(at..at + len).unwrap_or(&EMPTY_STRING)
    };
    let text = |slot: usize| decode_string(slot_bytes(slot));
    let device_library_strings: BTreeMap<u8, Vec<u8>> = (0..TRACK_STRINGS)
        .filter(|slot| !slot::MODELLED.contains(slot))
        .filter(|&slot| slot_bytes(slot) != EMPTY_STRING)
        .map(|slot| (slot as u8, slot_bytes(slot).to_vec()))
        .collect();
    StickTrack {
        id: u32_at(row, 0x48),
        title: text(slot::TITLE),
        path: text(slot::FILE_PATH),
        filename: text(slot::FILENAME),
        artist_id: u32_at(row, 0x44),
        album_id: u32_at(row, 0x40),
        genre_id: u32_at(row, 0x3c),
        label_id: u32_at(row, 0x28),
        key_id: u32_at(row, 0x20),
        remixer_id: u32_at(row, 0x2c),
        composer_id: u32_at(row, 0x0c),
        original_artist_id: u32_at(row, 0x24),
        artwork_id: u32_at(row, 0x1c),
        color_id: u8_at(row, 0x58),
        rating: u8_at(row, 0x59),
        tempo_x100: u32_at(row, 0x38),
        duration_secs: u32::from(u16_at(row, 0x54)),
        year: u16_at(row, 0x50),
        track_number: u32_at(row, 0x34),
        disc_number: u16_at(row, 0x4c),
        bitrate: u32_at(row, 0x30),
        sample_rate: u32_at(row, 0x08),
        sample_depth: u16_at(row, 0x52),
        file_size: u64::from(u32_at(row, 0x10)),
        file_type: u16_at(row, 0x5a),
        play_count: u32::from(u16_at(row, 0x4e)),
        comment: text(slot::COMMENT),
        mix_name: text(slot::MIX_NAME),
        isrc: decode_isrc(slot_bytes(slot::ISRC)),
        date_added: text(slot::DATE_ADDED),
        date_created: text(slot::DATE_CREATED),
        release_date: text(slot::RELEASE_DATE),
        analyze_path: text(slot::ANALYZE_PATH),
        hot_cue_auto_load: text(slot::HOT_CUE_AUTO_LOAD) == "ON",
        master_db_id: u32_at(row, 0x18),
        master_content_id: u32_at(row, 0x14),
        content_link: u32_at(row, 0x04),
        analysed_bits: None,
        cues: Vec::new(),
        device_library_strings,
    }
}

fn decode_named(row: &[u8], name_at: usize) -> StickName {
    StickName { id: u32_at(row, 0), name: string_in(row, name_at) }
}

fn decode_artist(row: &[u8]) -> StickName {
    let at = if u16_at(row, 0) & 0x04 != 0 { usize::from(u16_at(row, 0x0a)) } else { usize::from(u8_at(row, 0x09)) };
    StickName { id: u32_at(row, 4), name: string_in(row, at) }
}

fn decode_album(row: &[u8]) -> StickAlbum {
    let at = if u16_at(row, 0) & 0x04 != 0 { usize::from(u16_at(row, 0x16)) } else { usize::from(u8_at(row, 0x15)) };
    StickAlbum { id: u32_at(row, 0x0c), name: string_in(row, at), artist_id: u32_at(row, 0x08) }
}

fn decode_device(row: &[u8]) -> StickDevice {
    let date_end = 0x0c + encoded_string_len(row, 0x0c);
    let version_at = date_end + 2;
    let name_at = version_at + encoded_string_len(row, version_at);
    StickDevice {
        name: string_in(row, name_at),
        db_version: string_in(row, version_at),
        created_date: string_in(row, 0x0c),
        background_color: u8_at(row, 0x09),
        track_count: u32_at(row, 4),
    }
}

fn entries_by_list(rows: &[Vec<u8>], index_at: usize, track_at: usize, list_at: usize) -> BTreeMap<u32, Vec<u32>> {
    let mut by_list: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for row in rows {
        by_list.entry(u32_at(row, list_at)).or_default().push((u32_at(row, index_at), u32_at(row, track_at)));
    }
    by_list
        .into_iter()
        .map(|(list, mut entries)| {
            entries.sort_by_key(|&(index, _)| index);
            (list, entries.into_iter().map(|(_, track)| track).collect())
        })
        .collect()
}

pub fn model_from_tables(tables: &[TableRows]) -> StickModel {
    let rows = |table: u32| -> &[Vec<u8>] {
        tables.iter().find(|t| t.table == table).map_or(&[][..], |t| t.rows.as_slice())
    };
    let named = |table: u32, name_at: usize| -> Vec<StickName> {
        rows(table).iter().map(|row| decode_named(row, name_at)).collect()
    };
    let mut playlist_tracks = entries_by_list(rows(table::PLAYLIST_ENTRIES), 0, 4, 8);
    let mut history_tracks = entries_by_list(rows(table::HISTORY_ENTRIES), 8, 0, 4);
    StickModel {
        device: rows(table::PROPERTY).last().map(|row| decode_device(row)).unwrap_or_default(),
        tracks: rows(table::TRACKS).iter().map(|row| decode_track(row)).collect(),
        artists: rows(table::ARTISTS).iter().map(|row| decode_artist(row)).collect(),
        albums: rows(table::ALBUMS).iter().map(|row| decode_album(row)).collect(),
        genres: named(table::GENRES, 4),
        labels: named(table::LABELS, 4),
        keys: named(table::KEYS, 8),
        colors: rows(table::COLORS)
            .iter()
            .map(|row| StickName { id: u32::from(u16_at(row, 5)), name: string_in(row, 8) })
            .collect(),
        artworks: named(table::ARTWORK, 4),
        playlists: rows(table::PLAYLIST_TREE)
            .iter()
            .map(|row| {
                let id = u32_at(row, 12);
                StickPlaylist {
                    id,
                    parent_id: u32_at(row, 0),
                    name: string_in(row, 20),
                    is_folder: u32_at(row, 16) != 0,
                    sort_order: u32_at(row, 8),
                    track_ids: playlist_tracks.remove(&id).unwrap_or_default(),
                }
            })
            .collect(),
        histories: rows(table::HISTORY_PLAYLISTS)
            .iter()
            .map(|row| {
                let id = u32_at(row, 0);
                StickHistory { id, name: string_in(row, 4), track_ids: history_tracks.remove(&id).unwrap_or_default() }
            })
            .collect(),
        menu: StickMenu {
            columns: rows(table::COLUMNS).to_vec(),
            categories: rows(table::CATEGORIES).to_vec(),
            sorts: rows(table::SORTS).to_vec(),
        },
        my_tags: Vec::new(),
        track_tags: Vec::new(),
        my_tag_master_db_id: 0,
    }
}

pub fn read(bytes: &[u8]) -> Result<StickModel> {
    Ok(model_from_tables(&table_rows(bytes)?))
}

pub const PAGE_SIZE: usize = 4096;
const TRACK_TRAILER_56: u16 = 0x29;
const TRACK_TRAILER_5C: u16 = 3;

fn put_u16(row: &mut [u8], at: usize, value: u16) {
    row[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(row: &mut [u8], at: usize, value: u32) {
    row[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn clamp_u16(value: u32) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

pub(crate) fn encode_isrc(isrc: &str) -> Vec<u8> {
    if isrc.is_empty() {
        return EMPTY_STRING.to_vec();
    }
    let len = 4 + 1 + isrc.len() + 1;
    let mut out = vec![0x90];
    out.extend_from_slice(&clamp_u16(len as u32).to_le_bytes());
    out.push(0);
    out.push(0x03);
    out.extend_from_slice(isrc.as_bytes());
    out.push(0);
    out
}

fn push_encoded(row: &mut Vec<u8>, encoded: &[u8]) -> usize {
    if matches!(encoded.first(), Some(0x40 | 0x90)) {
        while row.len() % 4 != 0 {
            row.push(0);
        }
    }
    let at = row.len();
    row.extend_from_slice(encoded);
    at
}

pub fn encode_track(track: &StickTrack) -> Vec<u8> {
    use rbl_pdb::build::device_sql_string;

    let mut row = vec![0_u8; TRACK_FIXED_LEN + TRACK_STRINGS * 2];
    put_u16(&mut row, 0x00, 0x24);
    put_u32(&mut row, 0x04, track.content_link);
    put_u32(&mut row, 0x08, track.sample_rate);
    put_u32(&mut row, 0x0c, track.composer_id);
    put_u32(&mut row, 0x10, u32::try_from(track.file_size).unwrap_or(u32::MAX));
    put_u32(&mut row, 0x14, track.master_content_id);
    put_u32(&mut row, 0x18, track.master_db_id);
    put_u32(&mut row, 0x1c, track.artwork_id);
    put_u32(&mut row, 0x20, track.key_id);
    put_u32(&mut row, 0x24, track.original_artist_id);
    put_u32(&mut row, 0x28, track.label_id);
    put_u32(&mut row, 0x2c, track.remixer_id);
    put_u32(&mut row, 0x30, track.bitrate);
    put_u32(&mut row, 0x34, track.track_number);
    put_u32(&mut row, 0x38, track.tempo_x100);
    put_u32(&mut row, 0x3c, track.genre_id);
    put_u32(&mut row, 0x40, track.album_id);
    put_u32(&mut row, 0x44, track.artist_id);
    put_u32(&mut row, 0x48, track.id);
    put_u16(&mut row, 0x4c, track.disc_number);
    put_u16(&mut row, 0x4e, clamp_u16(track.play_count));
    put_u16(&mut row, 0x50, track.year);
    put_u16(&mut row, 0x52, track.sample_depth);
    put_u16(&mut row, 0x54, clamp_u16(track.duration_secs));
    put_u16(&mut row, 0x56, TRACK_TRAILER_56);
    row[0x58] = track.color_id;
    row[0x59] = track.rating;
    put_u16(&mut row, 0x5a, track.file_type);
    put_u16(&mut row, 0x5c, TRACK_TRAILER_5C);

    for slot in 0..TRACK_STRINGS {
        let encoded = match slot {
            slot::ISRC => encode_isrc(&track.isrc),
            slot::HOT_CUE_AUTO_LOAD => device_sql_string(if track.hot_cue_auto_load { "ON" } else { "" }),
            slot::DATE_CREATED => device_sql_string(&track.date_created),
            slot::RELEASE_DATE => device_sql_string(&track.release_date),
            slot::MIX_NAME => device_sql_string(&track.mix_name),
            slot::ANALYZE_PATH => device_sql_string(&track.analyze_path),
            slot::DATE_ADDED => device_sql_string(&track.date_added),
            slot::COMMENT => device_sql_string(&track.comment),
            slot::TITLE => device_sql_string(&track.title),
            slot::FILENAME => device_sql_string(&track.filename),
            slot::FILE_PATH => device_sql_string(&track.path),
            other => track
                .device_library_strings
                .get(&(other as u8))
                .cloned()
                .unwrap_or_else(|| EMPTY_STRING.to_vec()),
        };
        let at = push_encoded(&mut row, &encoded);
        put_u16(&mut row, TRACK_FIXED_LEN + slot * 2, u16::try_from(at).unwrap_or(0));
    }
    row
}

pub fn max_row_len() -> usize {
    rbl_pdb::build::max_row_len(PAGE_SIZE)
}

pub fn encode_tables(model: &StickModel) -> Vec<TableRows> {
    use rbl_pdb::{reference, rows};

    let named = |items: &[StickName]| -> Vec<Vec<u8>> {
        items.iter().map(|item| rows::simple_named_row(item.id, &item.name)).collect()
    };
    let or_reference = |own: &[Vec<u8>], fallback: &[&[u8]]| -> Vec<Vec<u8>> {
        if own.is_empty() { fallback.iter().map(|row| row.to_vec()).collect() } else { own.to_vec() }
    };
    let property = rows::PdbProperty {
        device_name: model.device.name.clone(),
        db_version: if model.device.db_version.is_empty() {
            rows::PROPERTY_DB_VERSION.to_owned()
        } else {
            model.device.db_version.clone()
        },
        contents: u32::try_from(model.tracks.len()).unwrap_or(u32::MAX),
        created_date: model.device.created_date.clone(),
        background_color: model.device.background_color,
    };

    (0..table::COUNT)
        .map(|kind| {
            let rows: Vec<Vec<u8>> = match kind {
                table::TRACKS => model.tracks.iter().map(encode_track).collect(),
                table::GENRES => named(&model.genres),
                table::ARTISTS => model.artists.iter().map(|artist| rows::artist_row(artist.id, &artist.name)).collect(),
                table::ALBUMS => model
                    .albums
                    .iter()
                    .map(|album| rows::album_row(album.id, album.artist_id, &album.name))
                    .collect(),
                table::LABELS => named(&model.labels),
                table::KEYS => model.keys.iter().map(|key| rows::key_row(key.id, &key.name)).collect(),
                table::COLORS => model
                    .colors
                    .iter()
                    .map(|color| rows::color_row(clamp_u16(color.id), &color.name))
                    .collect(),
                table::PLAYLIST_TREE => model
                    .playlists
                    .iter()
                    .map(|playlist| {
                        rows::playlist_row(
                            playlist.id,
                            playlist.parent_id,
                            playlist.sort_order,
                            playlist.is_folder,
                            &playlist.name,
                        )
                    })
                    .collect(),
                table::PLAYLIST_ENTRIES => model
                    .playlists
                    .iter()
                    .flat_map(|playlist| {
                        playlist.track_ids.iter().enumerate().map(move |(index, &track_id)| {
                            rows::playlist_entry_row(index as u32 + 1, track_id, playlist.id)
                        })
                    })
                    .collect(),
                table::HISTORY_PLAYLISTS => model
                    .histories
                    .iter()
                    .map(|history| rows::simple_named_row(history.id, &history.name))
                    .collect(),
                table::HISTORY_ENTRIES => model
                    .histories
                    .iter()
                    .flat_map(|history| {
                        history.track_ids.iter().enumerate().map(move |(index, &track_id)| {
                            let mut row = Vec::with_capacity(12);
                            row.extend_from_slice(&track_id.to_le_bytes());
                            row.extend_from_slice(&history.id.to_le_bytes());
                            row.extend_from_slice(&(index as u32 + 1).to_le_bytes());
                            row
                        })
                    })
                    .collect(),
                table::ARTWORK => named(&model.artworks),
                table::COLUMNS => or_reference(&model.menu.columns, reference::COLUMNS),
                table::CATEGORIES => or_reference(&model.menu.categories, reference::CATEGORIES),
                table::SORTS => or_reference(&model.menu.sorts, reference::SORTS),
                table::PROPERTY => rows::property_row(&property).into_iter().collect(),
                _ => Vec::new(),
            };
            TableRows { table: kind, rows }
        })
        .collect()
}

pub fn build(model: &StickModel) -> Result<Vec<u8>> {
    use rbl_pdb::rows::set_index_shift;

    anyhow::ensure!(
        model.device.created_date.len() == 10 && model.device.created_date.is_ascii(),
        "The stick's creation date must look like 2026-11-22, got {:?}",
        model.device.created_date
    );
    let tables = encode_tables(model);
    let limit = max_row_len();
    for rows in &tables {
        if let Some(row) = rows.rows.iter().find(|row| row.len() > limit) {
            anyhow::bail!(
                "A row in the {} table is {} bytes, more than the {limit} that fit on a page",
                table::name(rows.table),
                row.len()
            );
        }
    }
    let mut file = rbl_pdb::build::FileBuilder::new(PAGE_SIZE);
    for rows in &tables {
        if matches!(rows.table, table::TRACKS | table::ARTISTS | table::ALBUMS | table::PROPERTY) {
            file.add_table_numbered(rows.table, &rows.rows, |row, slot| set_index_shift(row, slot));
        } else {
            file.add_table(rows.table, &rows.rows);
        }
    }
    Ok(file.finish())
}
