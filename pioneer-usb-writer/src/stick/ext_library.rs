// The row layouts and the table order follow rbxport v1.10.0, crates/rbl-export/src/ext_pdb.rs
// (Copyright (c) 2026 Chris Le, GPL-2.0-or-later, https://github.com/chrisle/rbxport).

use anyhow::{Context, Result};
use rbl_pdb::build::{long_utf16le, short_ascii, FileBuilder, PageStyle};
use rbl_pdb::Pdb;

use super::device_library::{decode_string, encoded_string_len, TableRows, PAGE_SIZE};
use super::model::{StickModel, StickMyTag};

pub const FILE_NAME: &str = "exportExt.pdb";
pub const TABLE_COUNT: u32 = 9;
pub const MY_TAGS: u32 = 3;
pub const TRACK_TAGS: u32 = 4;
pub const MASTER: u32 = 7;
const TAG_SUBTYPE: u16 = 0x0680;
const TAG_NAME_AREA: usize = 0x20;
const MASTER_ROW_LEN: usize = 60;
const TRACK_TAG_ROW_LEN: usize = 16;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtLibrary {
    pub my_tags: Vec<StickMyTag>,
    pub track_tags: Vec<(u32, u32)>,
    pub master_db_id: u32,
}

impl ExtLibrary {
    pub fn of(model: &StickModel) -> Self {
        Self {
            my_tags: model.my_tags.clone(),
            track_tags: model.track_tags.clone(),
            master_db_id: model.my_tag_master_db_id,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.my_tags.is_empty() && self.track_tags.is_empty() && self.master_db_id == 0
    }

    pub fn put_into(self, model: &mut StickModel) {
        model.my_tags = self.my_tags;
        model.track_tags = self.track_tags;
        model.my_tag_master_db_id = self.master_db_id;
    }
}

fn u8_at(row: &[u8], at: usize) -> u8 {
    row.get(at).copied().unwrap_or(0)
}

fn u32_at(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([u8_at(row, at), u8_at(row, at + 1), u8_at(row, at + 2), u8_at(row, at + 3)])
}

fn row_len(table: u32, bytes: &[u8], at: usize) -> usize {
    match table {
        MY_TAGS => {
            let name_at = usize::from(u8_at(bytes, at + 0x1d));
            let name_len = encoded_string_len(bytes, at + name_at);
            (TAG_NAME_AREA + name_len + 8).div_ceil(4) * 4
        }
        TRACK_TAGS => TRACK_TAG_ROW_LEN,
        MASTER => MASTER_ROW_LEN,
        _ => 0,
    }
}

pub fn normalised_row(table: u32, row: &[u8]) -> Vec<u8> {
    let mut row = row.to_vec();
    if table == MY_TAGS && row.len() > TAG_NAME_AREA {
        row[2] = 0;
        row[3] = 0;
        if usize::from(row[0x1d]) == TAG_NAME_AREA {
            row[TAG_NAME_AREA - 1] = 0;
        }
        let after_name = usize::from(row[0x1e]) + 1;
        for byte in row.iter_mut().skip(after_name) {
            *byte = 0;
        }
    }
    row
}

pub fn table_rows(bytes: &[u8]) -> Result<Vec<TableRows>> {
    let pdb = Pdb::parse(bytes).context("exportExt.pdb is not a DeviceSQL database")?;
    Ok(pdb
        .tables
        .iter()
        .enumerate()
        .map(|(index, table_ref)| {
            let table = u32_at(bytes, 28 + index * 16);
            let rows = pdb
                .rows(table_ref)
                .into_iter()
                .filter_map(|row| {
                    let len = row_len(table, bytes, row.offset);
                    bytes.get(row.offset..row.offset + len).filter(|row| !row.is_empty()).map(<[u8]>::to_vec)
                })
                .collect();
            TableRows { table, rows }
        })
        .collect())
}

fn decode_tag(row: &[u8]) -> StickMyTag {
    let name_at = usize::from(u8_at(row, 0x1d));
    StickMyTag {
        id: u32_at(row, 0x14),
        name: row.get(name_at..).map(decode_string).unwrap_or_default(),
        parent_id: u32_at(row, 0x0c),
        sort_order: u32_at(row, 0x10),
        is_category: u8_at(row, 0x1b) != 0,
    }
}

pub fn from_tables(tables: &[TableRows]) -> ExtLibrary {
    let rows = |table: u32| -> &[Vec<u8>] {
        tables.iter().find(|t| t.table == table).map_or(&[][..], |t| t.rows.as_slice())
    };
    ExtLibrary {
        my_tags: rows(MY_TAGS).iter().map(|row| decode_tag(row)).collect(),
        track_tags: rows(TRACK_TAGS).iter().map(|row| (u32_at(row, 4), u32_at(row, 8))).collect(),
        master_db_id: rows(MASTER).last().map_or(0, |row| u32_at(row, 0x18)),
    }
}

pub fn read(bytes: &[u8]) -> Result<ExtLibrary> {
    Ok(from_tables(&table_rows(bytes)?))
}

fn encode_tag(tag: &StickMyTag) -> Vec<u8> {
    let name = if tag.name.is_ascii() && tag.name.len() < 0x7e { short_ascii(&tag.name) } else { long_utf16le(&tag.name) };
    let name_at = if matches!(name[0], 0x40 | 0x90) { 0x20 } else { 0x1f };
    let end = name_at + name.len();
    let len = (TAG_NAME_AREA + name.len() + 8).div_ceil(4) * 4;
    let mut row = vec![0_u8; len.max(end + 1)];
    row[0..2].copy_from_slice(&TAG_SUBTYPE.to_le_bytes());
    row[0x0c..0x10].copy_from_slice(&(if tag.is_category { 0 } else { tag.parent_id }).to_le_bytes());
    row[0x10..0x14].copy_from_slice(&tag.sort_order.to_le_bytes());
    row[0x14..0x18].copy_from_slice(&tag.id.to_le_bytes());
    row[0x1b] = u8::from(tag.is_category);
    row[0x1c] = 0x03;
    row[0x1d] = name_at as u8;
    row[0x1e] = u8::try_from(end).unwrap_or(0);
    row[name_at..end].copy_from_slice(&name);
    row[end] = 0x03;
    row
}

fn encode_track_tag(track_id: u32, tag_id: u32) -> Vec<u8> {
    let mut row = vec![0_u8; TRACK_TAG_ROW_LEN];
    row[4..8].copy_from_slice(&track_id.to_le_bytes());
    row[8..12].copy_from_slice(&tag_id.to_le_bytes());
    row[12] = 0x03;
    row
}

fn encode_master(master_db_id: u32) -> Vec<u8> {
    let mut row = vec![0_u8; MASTER_ROW_LEN];
    row[0..4].copy_from_slice(&[0x00, 0x07, 0x00, 0x00]);
    row[0x18..0x1c].copy_from_slice(&master_db_id.to_le_bytes());
    row[0x1c] = 0x03;
    row[0x1d..0x22].copy_from_slice(&[0x22, 0x23, 0x24, 0x25, 0x26]);
    row[0x22..0x27].fill(0x03);
    row
}

pub fn encode_tables(ext: &ExtLibrary) -> Vec<TableRows> {
    (0..TABLE_COUNT)
        .map(|table| TableRows {
            table,
            rows: match table {
                MY_TAGS => ext.my_tags.iter().map(encode_tag).collect(),
                TRACK_TAGS => ext.track_tags.iter().map(|&(track, tag)| encode_track_tag(track, tag)).collect(),
                MASTER => vec![encode_master(ext.master_db_id)],
                _ => Vec::new(),
            },
        })
        .collect()
}

pub fn build(ext: &ExtLibrary) -> Vec<u8> {
    let mut file = FileBuilder::new(PAGE_SIZE);
    file.write_order(&[MASTER, MY_TAGS]);
    for rows in encode_tables(ext) {
        match rows.table {
            MY_TAGS => file.add_table_numbered(MY_TAGS, &rows.rows, |row, slot| rbl_pdb::rows::set_index_shift(row, slot)),
            TRACK_TAGS => file.add_table_styled(TRACK_TAGS, &rows.rows, |_, _| {}, PageStyle::LastRowIndex),
            table => file.add_table(table, &rows.rows),
        }
    }
    file.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        text.split_whitespace().map(|byte| u8::from_str_radix(byte, 16).unwrap()).collect()
    }

    fn tag(id: u32, sort_order: u32, name: &str, is_category: bool, parent_id: u32) -> StickMyTag {
        StickMyTag { id, name: name.into(), parent_id, sort_order, is_category }
    }

    #[test]
    fn a_category_row_has_the_bytes_rekordbox_writes() {
        let expected = hex(
            "80 06 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 05 00 00 00 00 00 00 01 03 1f 2c 1b \
             4c 65 78 69 63 6f 6e 20 54 61 67 73 03 00 00 00 00 00 00 00 00 00 00 00",
        );
        assert_eq!(encode_tag(&tag(5, 0, "Lexicon Tags", true, 0)), expected);
        assert_eq!(decode_tag(&expected), tag(5, 0, "Lexicon Tags", true, 0));
    }

    #[test]
    fn track_tag_and_master_rows_have_the_bytes_rekordbox_writes() {
        assert_eq!(encode_track_tag(2, 0x07ae_781a), hex("00 00 00 00 02 00 00 00 1a 78 ae 07 03 00 00 00"));
        let master = encode_master(1_744_129_535);
        assert_eq!(
            master[..39],
            hex("00 07 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 ff 4d f5 67 03 22 23 24 25 26 03 03 03 03 03")[..]
        );
        assert_eq!(master.len(), 60);
    }

    #[test]
    fn tags_tracks_and_the_master_id_survive_a_written_file() {
        let ext = ExtLibrary {
            my_tags: vec![
                tag(5, 0, "Genre", true, 0),
                tag(101, 0, "Components \u{25b6} Synth", false, 5),
                tag(100, 1, "Deep", false, 5),
            ],
            track_tags: (1..=600).map(|track| (track, 100 + track % 2)).collect(),
            master_db_id: 77,
        };
        let bytes = build(&ext);
        assert_eq!(read(&bytes).unwrap(), ext);
        assert_eq!(table_rows(&bytes).unwrap().len(), 9);
    }
}
