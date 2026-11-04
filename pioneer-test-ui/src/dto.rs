use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use pioneer_usb_writer::models::Track;

pub fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn serialize_preview<S: serde::Serializer>(
    preview: &Option<Vec<u8>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match preview {
        Some(bytes) => serializer.serialize_str(&base64_encode(bytes)),
        None => serializer.serialize_none(),
    }
}

/// Lightweight track info sent over the IPC boundary.
/// Omits artwork bytes and other heavy fields.
#[derive(Serialize, Clone)]
pub struct TrackInfo {
    pub id: u32,
    pub source_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    /// BPM * 100, 0 = not yet analyzed
    pub tempo: u32,
    /// Empty string = not yet analyzed
    pub key: String,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub file_size: u64,
    pub has_artwork: bool,
    pub has_cues: bool,
    /// True when an analysis row exists in the DB index (authoritative, not `tempo > 0`).
    pub has_analysis: bool,
    /// True when the analysis row lacks stages (key or grid still to come); the track stays queued for them.
    pub analysis_pending: bool,
    /// 400-byte mono waveform preview as base64 (a JSON number array is ~2.6x
    /// larger), present only when a real (non-placeholder) preview exists. Lets
    /// the track list paint mini waveforms without an N+1 fetch.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_preview")]
    pub waveform_preview: Option<Vec<u8>>,
    /// 3-band row preview (`ROW_BAND_COLUMNS` columns of low, mid, high bytes, 0..127) as base64.
    /// When present it replaces `waveform_preview`, which then stays unset.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_preview")]
    pub waveform_bands: Option<Vec<u8>>,
    /// Star rating 0-5 imported from Rekordbox.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,
    /// Colour label imported from Rekordbox: a colour name or `#rrggbb`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl From<&Track> for TrackInfo {
    fn from(t: &Track) -> Self {
        Self {
            id: t.id,
            source_path: t.source_path.to_string_lossy().to_string(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            genre: t.genre.clone(),
            tempo: t.tempo,
            key: t.key.clone(),
            duration_secs: t.duration_secs,
            bitrate: t.bitrate,
            file_size: t.file_size,
            has_artwork: t.artwork.is_some(),
            has_cues: false,
            has_analysis: false,
            analysis_pending: false,
            waveform_preview: None,
            waveform_bands: None,
            rating: None,
            color: None,
        }
    }
}

/// Row preview of one track, fetched for the rows on screen (same encoding as in [`TrackInfo`]).
#[derive(Serialize, Clone)]
pub struct RowPreviewInfo {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_preview")]
    pub waveform_bands: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_preview")]
    pub waveform_preview: Option<Vec<u8>>,
}

/// The rating and colour of a track as its sidecar files give them now; both `None` when they were cleared.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TrackLabelInfo {
    pub id: u32,
    pub rating: Option<u8>,
    pub color: Option<String>,
}

/// Input from the frontend describing a playlist to create on the USB.
#[derive(Serialize, Deserialize, Clone)]
pub struct PlaylistInput {
    pub id: u32,
    pub name: String,
    pub track_ids: Vec<u32>,
    #[serde(default)]
    pub parent_id: u32,
    #[serde(default)]
    pub is_folder: bool,
}

/// Payload emitted during analysis to report progress.
#[derive(Serialize, Clone)]
pub struct ProgressPayload {
    pub current: u32,
    pub total: u32,
    pub message: String,
    pub track_id: Option<u32>,
}

/// State returned by `load_state` so the frontend can restore everything.
///
/// The tracks go over the wire as a [`TrackTable`] (`app/src/services/track-table.ts` decodes it back into
/// `TrackInfo` objects): at 50k tracks the object list is 21.6 MB of JSON, mostly repeated keys and folders.
#[derive(Serialize, Clone)]
pub struct LoadedState {
    pub tracks: TrackTable,
    pub playlists: Vec<PlaylistInput>,
}

const TRACK_FLAG_ARTWORK: u8 = 1;
const TRACK_FLAG_CUES: u8 = 2;
const TRACK_FLAG_ANALYSIS: u8 = 4;
const TRACK_FLAG_PENDING: u8 = 8;

/// One array per field. A path is split after its last `/` or `\` into its folder (`dir`, an index into the
/// folder list) and `file`. Each folder is its parent's path plus `folder_name` (`folder_parent` is the
/// parent's index plus one, 0 for none). Titles and file names are sent as they are, the other texts that
/// repeat (artists, albums, genres, keys, colours) as indexes into `strings`, which holds each of them once.
/// `ratings` and `colors` are `[row, value, ...]` pairs for the tracks that have one (a colour value is a
/// string index).
#[derive(Serialize, Clone, Default)]
pub struct TrackTable {
    strings: TextColumn,
    folder_parent: Vec<u32>,
    folder_name: TextColumn,
    id: Vec<u32>,
    dir: Vec<u32>,
    file: TextColumn,
    title: TextColumn,
    artist: Vec<u32>,
    album: Vec<u32>,
    genre: Vec<u32>,
    key: Vec<u32>,
    tempo: Vec<u32>,
    duration_secs: Vec<f64>,
    bitrate: Vec<u32>,
    file_size: Vec<u64>,
    flags: Vec<u8>,
    ratings: Vec<u32>,
    colors: Vec<u32>,
}

impl TrackTable {
    pub fn len(&self) -> usize {
        self.id.len()
    }
}

/// Texts stored back to back, sent as an array of strings.
#[derive(Clone, Default)]
struct TextColumn {
    text: String,
    ends: Vec<usize>,
}

impl TextColumn {
    fn push(&mut self, text: &str) {
        self.text.push_str(text);
        self.ends.push(self.text.len());
    }

    fn len(&self) -> usize {
        self.ends.len()
    }

    fn append(&mut self, other: &TextColumn) {
        let shift = self.text.len();
        self.text.push_str(&other.text);
        self.ends.extend(other.ends.iter().map(|end| end + shift));
    }

    fn get(&self, index: usize) -> &str {
        let start = if index == 0 { 0 } else { self.ends[index - 1] };
        &self.text[start..self.ends[index]]
    }

    fn iter(&self) -> impl Iterator<Item = &str> {
        let starts = std::iter::once(0).chain(self.ends.iter().copied());
        starts.zip(self.ends.iter().copied()).map(|(start, end)| &self.text[start..end])
    }
}

impl Serialize for TextColumn {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[derive(Default, Clone, Copy)]
struct FastHasher(u64);

impl FastHasher {
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl std::hash::Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut words = bytes.chunks_exact(8);
        for word in &mut words {
            self.add(u64::from_le_bytes(word.try_into().expect("a chunk of 8 bytes")));
        }
        let rest = words.remainder();
        if !rest.is_empty() {
            let mut last = [0u8; 8];
            last[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(last));
        }
    }

    fn write_u8(&mut self, byte: u8) {
        self.add(u64::from(byte));
    }

    fn finish(&self) -> u64 {
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }
}

/// Distinct texts, found again by an open-addressing table of indexes into the texts.
#[derive(Default)]
struct TextIndex {
    texts: TextColumn,
    slots: Vec<u32>,
}

impl TextIndex {
    fn hash(text: &str) -> usize {
        use std::hash::Hasher;
        let mut hasher = FastHasher::default();
        hasher.write(text.as_bytes());
        hasher.finish() as usize
    }

    fn find(&self, text: &str) -> Option<u32> {
        if self.slots.is_empty() {
            return None;
        }
        let mask = self.slots.len() - 1;
        let mut slot = Self::hash(text) & mask;
        loop {
            match self.slots[slot] {
                0 => return None,
                found if self.texts.get(found as usize - 1) == text => return Some(found - 1),
                _ => slot = (slot + 1) & mask,
            }
        }
    }

    fn insert(&mut self, text: &str) -> u32 {
        if (self.texts.len() + 1) * 2 > self.slots.len() {
            let size = (self.slots.len() * 2).max(64);
            self.slots = vec![0; size];
            for index in 0..self.texts.len() {
                self.place(Self::hash(self.texts.get(index)), index as u32);
            }
        }
        let index = self.texts.len() as u32;
        self.texts.push(text);
        self.place(Self::hash(text), index);
        index
    }

    fn place(&mut self, hash: usize, index: u32) {
        let mask = self.slots.len() - 1;
        let mut slot = hash & mask;
        while self.slots[slot] != 0 {
            slot = (slot + 1) & mask;
        }
        self.slots[slot] = index + 1;
    }

    fn intern(&mut self, text: &str) -> u32 {
        self.find(text).unwrap_or_else(|| self.insert(text))
    }
}

#[derive(Default)]
struct FolderTable {
    paths: TextIndex,
    parent: Vec<u32>,
    name: TextColumn,
}

impl FolderTable {
    fn folder(&mut self, dir: &str) -> u32 {
        if let Some(index) = self.paths.find(dir) {
            return index;
        }
        let cut = dir[..dir.len().saturating_sub(1)].rfind(['/', '\\']).map_or(0, |i| i + 1);
        let parent = if cut == 0 { 0 } else { self.folder(&dir[..cut]) + 1 };
        self.parent.push(parent);
        self.name.push(&dir[cut..]);
        self.paths.insert(dir)
    }
}

/// What the track table holds of one track.
pub struct TrackRow<'a> {
    pub id: u32,
    pub source_path: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub genre: &'a str,
    pub key: &'a str,
    pub tempo: u32,
    pub duration_secs: f64,
    pub bitrate: u32,
    pub file_size: u64,
    pub has_artwork: bool,
    pub has_cues: bool,
    pub has_analysis: bool,
    pub analysis_pending: bool,
    pub rating: Option<u8>,
    pub color: Option<&'a str>,
}

impl<'a> TrackRow<'a> {
    fn of(t: &'a TrackInfo) -> Self {
        TrackRow {
            id: t.id,
            source_path: &t.source_path,
            title: &t.title,
            artist: &t.artist,
            album: &t.album,
            genre: &t.genre,
            key: &t.key,
            tempo: t.tempo,
            duration_secs: t.duration_secs,
            bitrate: t.bitrate,
            file_size: t.file_size,
            has_artwork: t.has_artwork,
            has_cues: t.has_cues,
            has_analysis: t.has_analysis,
            analysis_pending: t.analysis_pending,
            rating: t.rating,
            color: t.color.as_deref(),
        }
    }
}

/// Builds a [`TrackTable`] from rows handed over one by one, without a copy of each text per row.
pub struct TrackTableBuilder {
    table: TrackTable,
    strings: TextIndex,
    folders: FolderTable,
    previous_dir: (String, u32),
    runs: [(String, u32); 5],
}

impl Default for TrackTableBuilder {
    fn default() -> Self {
        TrackTableBuilder {
            table: TrackTable::default(),
            strings: TextIndex::default(),
            folders: FolderTable::default(),
            previous_dir: (String::new(), u32::MAX),
            runs: std::array::from_fn(|_| (String::new(), u32::MAX)),
        }
    }
}

impl TrackTableBuilder {
    fn intern(&mut self, column: usize, text: &str) -> u32 {
        let (previous, index) = &mut self.runs[column];
        if *index != u32::MAX && previous.as_str() == text {
            return *index;
        }
        let interned = self.strings.intern(text);
        previous.clear();
        previous.push_str(text);
        self.runs[column].1 = interned;
        interned
    }

    pub fn push(&mut self, row: &TrackRow<'_>) {
        let index = self.table.id.len() as u32;
        let cut = row.source_path.rfind(['/', '\\']).map_or(0, |i| i + 1);
        let dir = &row.source_path[..cut];
        let folder = if self.previous_dir.1 != u32::MAX && self.previous_dir.0 == dir {
            self.previous_dir.1
        } else {
            let folder = self.folders.folder(dir);
            self.previous_dir.0.clear();
            self.previous_dir.0.push_str(dir);
            self.previous_dir.1 = folder;
            folder
        };
        let artist = self.intern(0, row.artist);
        let album = self.intern(1, row.album);
        let genre = self.intern(2, row.genre);
        let key = self.intern(3, row.key);
        let table = &mut self.table;
        table.id.push(row.id);
        table.dir.push(folder);
        table.file.push(&row.source_path[cut..]);
        table.title.push(row.title);
        table.artist.push(artist);
        table.album.push(album);
        table.genre.push(genre);
        table.key.push(key);
        table.tempo.push(row.tempo);
        table.duration_secs.push(row.duration_secs);
        table.bitrate.push(row.bitrate);
        table.file_size.push(row.file_size);
        table.flags.push(
            u8::from(row.has_artwork) * TRACK_FLAG_ARTWORK
                | u8::from(row.has_cues) * TRACK_FLAG_CUES
                | u8::from(row.has_analysis) * TRACK_FLAG_ANALYSIS
                | u8::from(row.analysis_pending) * TRACK_FLAG_PENDING,
        );
        if let Some(rating) = row.rating {
            self.table.ratings.extend([index, u32::from(rating)]);
        }
        if let Some(color) = row.color {
            let color = self.intern(4, color);
            self.table.colors.extend([index, color]);
        }
    }

    /// Adds the rows of `other`, built from the tracks that follow this builder's, as if they had been pushed here.
    pub fn append(&mut self, other: TrackTable) {
        let first_row = self.table.id.len() as u32;
        let strings: Vec<u32> = other.strings.iter().map(|text| self.strings.intern(text)).collect();
        let mut paths = TextColumn::default();
        let mut path = String::new();
        let mut folders = Vec::with_capacity(other.folder_name.len());
        for (index, &parent) in other.folder_parent.iter().enumerate() {
            path.clear();
            if parent > 0 {
                path.push_str(paths.get(parent as usize - 1));
            }
            path.push_str(other.folder_name.get(index));
            paths.push(&path);
            folders.push(self.folders.folder(&path));
        }
        let table = &mut self.table;
        table.id.extend(&other.id);
        table.dir.extend(other.dir.iter().map(|&folder| folders[folder as usize]));
        table.file.append(&other.file);
        table.title.append(&other.title);
        table.artist.extend(other.artist.iter().map(|&text| strings[text as usize]));
        table.album.extend(other.album.iter().map(|&text| strings[text as usize]));
        table.genre.extend(other.genre.iter().map(|&text| strings[text as usize]));
        table.key.extend(other.key.iter().map(|&text| strings[text as usize]));
        table.tempo.extend(&other.tempo);
        table.duration_secs.extend(&other.duration_secs);
        table.bitrate.extend(&other.bitrate);
        table.file_size.extend(&other.file_size);
        table.flags.extend(&other.flags);
        for rating in other.ratings.chunks_exact(2) {
            table.ratings.extend([first_row + rating[0], rating[1]]);
        }
        for color in other.colors.chunks_exact(2) {
            table.colors.extend([first_row + color[0], strings[color[1] as usize]]);
        }
        self.previous_dir.1 = u32::MAX;
        self.runs.iter_mut().for_each(|run| run.1 = u32::MAX);
    }

    pub fn finish(self) -> TrackTable {
        TrackTable {
            strings: self.strings.texts,
            folder_parent: self.folders.parent,
            folder_name: self.folders.name,
            ..self.table
        }
    }
}

impl TrackTable {
    pub fn new(tracks: &[TrackInfo]) -> Result<Self, String> {
        let mut builder = TrackTableBuilder::default();
        for t in tracks {
            if t.waveform_preview.is_some() || t.waveform_bands.is_some() {
                return Err(format!("track {} carries a row preview, which the track table does not hold", t.id));
            }
            builder.push(&TrackRow::of(t));
        }
        Ok(builder.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: u32, source_path: &str) -> TrackInfo {
        TrackInfo {
            id,
            source_path: source_path.into(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            tempo: 0,
            key: String::new(),
            duration_secs: 0.0,
            bitrate: 0,
            file_size: 0,
            has_artwork: false,
            has_cues: false,
            has_analysis: false,
            analysis_pending: false,
            waveform_preview: None,
            waveform_bands: None,
            rating: None,
            color: None,
        }
    }

    #[test]
    fn loaded_state_sends_tracks_as_the_table_the_app_decodes() {
        let one = TrackInfo {
            title: "One".into(),
            artist: "Artist".into(),
            genre: "House".into(),
            tempo: 12800,
            key: "8A".into(),
            duration_secs: 200.5,
            bitrate: 320,
            file_size: 1234,
            has_artwork: true,
            has_analysis: true,
            rating: Some(4),
            ..info(7, "/m/Artist/one.mp3")
        };
        let two = TrackInfo {
            artist: "Artist".into(),
            file_size: 5,
            has_analysis: true,
            analysis_pending: true,
            color: Some("#ff0000".into()),
            ..info(9, "two.wav")
        };
        let tracks = TrackTable::new(&[one, two]).unwrap();
        let json = serde_json::to_string(&LoadedState { tracks, playlists: Vec::new() }).unwrap();
        assert_eq!(
            json,
            concat!(
                r##"{"tracks":{"strings":["Artist","","House","8A","#ff0000"],"folder_parent":[0,1,2,0],"folder_name":["/","m/","Artist/",""],"##,
                r##""id":[7,9],"dir":[2,3],"file":["one.mp3","two.wav"],"title":["One",""],"artist":[0,0],"album":[1,1],"genre":[2,1],"key":[3,1],"##,
                r##""tempo":[12800,0],"duration_secs":[200.5,0.0],"bitrate":[320,0],"file_size":[1234,5],"flags":[5,12],"ratings":[0,4],"colors":[1,4]},"##,
                r##""playlists":[]}"##
            )
        );
    }

    #[test]
    fn appended_tables_equal_the_table_of_all_rows() {
        let tracks: Vec<TrackInfo> = (0..30)
            .map(|n| TrackInfo {
                title: format!("Title {n}"),
                artist: format!("Artist {}", n % 4),
                album: if n < 12 { String::new() } else { format!("Album {}", n % 5) },
                genre: ["House", "Techno"][n % 2].into(),
                key: format!("{}A", n % 3),
                tempo: 12000 + n as u32,
                rating: (n % 4 == 1).then_some(n as u8 % 6),
                color: (n % 5 == 0).then(|| ["#f00", "#0f0", "Artist 2"][n % 3].to_string()),
                has_analysis: n % 2 == 0,
                ..info(n as u32 + 1, &format!("/m/dir {}/{}sub {}/{n}.mp3", n % 3, if n % 2 == 0 { "" } else { "x/" }, n % 6))
            })
            .collect();
        let json = |table: &TrackTable| serde_json::to_string(table).unwrap();
        let whole = json(&TrackTable::new(&tracks).unwrap());
        for cut in [0, 1, 11, 12, 29, 30] {
            let mut builder = TrackTableBuilder::default();
            let (head, tail) = tracks.split_at(cut);
            for t in head {
                builder.push(&TrackRow::of(t));
            }
            builder.append(TrackTable::new(tail).unwrap());
            assert_eq!(json(&builder.finish()), whole, "cut at {cut}");
        }
        let mut builder = TrackTableBuilder::default();
        for chunk in tracks.chunks(7) {
            builder.append(TrackTable::new(chunk).unwrap());
        }
        assert_eq!(json(&builder.finish()), whole);
    }

    #[test]
    fn track_table_splits_windows_paths_and_refuses_row_previews() {
        let tracks = [info(1, r"C:\Music\a.flac"), info(2, "C:/Music/b.flac")];
        let table = TrackTable::new(&tracks).unwrap();
        assert_eq!(table.folder_name.iter().collect::<Vec<_>>(), [r"C:\", r"Music\", "C:/", "Music/"]);
        assert_eq!(table.folder_parent, [0, 1, 0, 3]);
        assert_eq!((table.dir.as_slice(), table.file.iter().next()), ([1, 3].as_slice(), Some("a.flac")));
        let with_preview = [TrackInfo { waveform_bands: Some(vec![1]), ..info(3, "/m/c.mp3") }];
        assert!(TrackTable::new(&with_preview).is_err());
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(&[0xFB, 0xFF, 0xFE]), "+//+");
    }
}
