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
    #[serde(serialize_with = "serialize_track_table")]
    pub tracks: Vec<TrackInfo>,
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
#[derive(Serialize)]
pub struct TrackTable<'a> {
    strings: Vec<&'a str>,
    folder_parent: Vec<u32>,
    folder_name: Vec<&'a str>,
    id: Vec<u32>,
    dir: Vec<u32>,
    file: Vec<&'a str>,
    title: Vec<&'a str>,
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

#[derive(Default)]
struct StringTable<'a> {
    list: Vec<&'a str>,
    index: HashMap<&'a str, u32>,
}

impl<'a> StringTable<'a> {
    fn intern(&mut self, text: &'a str) -> u32 {
        let list = &mut self.list;
        *self.index.entry(text).or_insert_with(|| {
            list.push(text);
            (list.len() - 1) as u32
        })
    }
}

#[derive(Default)]
struct FolderTable<'a> {
    parent: Vec<u32>,
    name: Vec<&'a str>,
    index: HashMap<&'a str, u32>,
}

impl<'a> FolderTable<'a> {
    fn folder(&mut self, dir: &'a str) -> u32 {
        if let Some(&index) = self.index.get(dir) {
            return index;
        }
        let cut = dir[..dir.len().saturating_sub(1)].rfind(['/', '\\']).map_or(0, |i| i + 1);
        let parent = if cut == 0 { 0 } else { self.folder(&dir[..cut]) + 1 };
        let index = self.name.len() as u32;
        self.parent.push(parent);
        self.name.push(&dir[cut..]);
        self.index.insert(dir, index);
        index
    }
}

impl<'a> TrackTable<'a> {
    pub fn new(tracks: &'a [TrackInfo]) -> Result<Self, String> {
        let n = tracks.len();
        let mut strings = StringTable::default();
        let mut folders = FolderTable::default();
        let mut table = TrackTable {
            strings: Vec::new(),
            folder_parent: Vec::new(),
            folder_name: Vec::new(),
            id: Vec::with_capacity(n),
            dir: Vec::with_capacity(n),
            file: Vec::with_capacity(n),
            title: Vec::with_capacity(n),
            artist: Vec::with_capacity(n),
            album: Vec::with_capacity(n),
            genre: Vec::with_capacity(n),
            key: Vec::with_capacity(n),
            tempo: Vec::with_capacity(n),
            duration_secs: Vec::with_capacity(n),
            bitrate: Vec::with_capacity(n),
            file_size: Vec::with_capacity(n),
            flags: Vec::with_capacity(n),
            ratings: Vec::new(),
            colors: Vec::new(),
        };
        let mut previous_dir: Option<(&str, u32)> = None;
        let mut runs: [Option<(&str, u32)>; 5] = [None; 5];
        let mut intern = |column: usize, text: &'a str| match runs[column] {
            Some((previous, index)) if previous == text => index,
            _ => {
                let index = strings.intern(text);
                runs[column] = Some((text, index));
                index
            }
        };
        for (row, t) in tracks.iter().enumerate() {
            if t.waveform_preview.is_some() || t.waveform_bands.is_some() {
                return Err(format!("track {} carries a row preview, which the track table does not hold", t.id));
            }
            let cut = t.source_path.rfind(['/', '\\']).map_or(0, |i| i + 1);
            table.id.push(t.id);
            let dir = &t.source_path[..cut];
            let folder = match previous_dir {
                Some((previous, folder)) if previous == dir => folder,
                _ => folders.folder(dir),
            };
            previous_dir = Some((dir, folder));
            table.dir.push(folder);
            table.file.push(&t.source_path[cut..]);
            table.title.push(&t.title);
            table.artist.push(intern(0, &t.artist));
            table.album.push(intern(1, &t.album));
            table.genre.push(intern(2, &t.genre));
            table.key.push(intern(3, &t.key));
            table.tempo.push(t.tempo);
            table.duration_secs.push(t.duration_secs);
            table.bitrate.push(t.bitrate);
            table.file_size.push(t.file_size);
            table.flags.push(
                u8::from(t.has_artwork) * TRACK_FLAG_ARTWORK
                    | u8::from(t.has_cues) * TRACK_FLAG_CUES
                    | u8::from(t.has_analysis) * TRACK_FLAG_ANALYSIS
                    | u8::from(t.analysis_pending) * TRACK_FLAG_PENDING,
            );
            if let Some(rating) = t.rating {
                table.ratings.extend([row as u32, u32::from(rating)]);
            }
            if let Some(color) = &t.color {
                table.colors.extend([row as u32, intern(4, color)]);
            }
        }
        table.strings = strings.list;
        table.folder_parent = folders.parent;
        table.folder_name = folders.name;
        Ok(table)
    }
}

fn serialize_track_table<S: serde::Serializer>(tracks: &[TrackInfo], serializer: S) -> Result<S::Ok, S::Error> {
    TrackTable::new(tracks).map_err(serde::ser::Error::custom)?.serialize(serializer)
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
        let json = serde_json::to_string(&LoadedState { tracks: vec![one, two], playlists: Vec::new() }).unwrap();
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
    fn track_table_splits_windows_paths_and_refuses_row_previews() {
        let tracks = [info(1, r"C:\Music\a.flac"), info(2, "C:/Music/b.flac")];
        let table = TrackTable::new(&tracks).unwrap();
        assert_eq!(table.folder_name, [r"C:\", r"Music\", "C:/", "Music/"]);
        assert_eq!(table.folder_parent, [0, 1, 0, 3]);
        assert_eq!((table.dir.as_slice(), table.file[0]), ([1, 3].as_slice(), "a.flac"));
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
