use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryKind {
    DeviceLibrary,
    OneLibrary,
}

impl LibraryKind {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::DeviceLibrary => "export.pdb",
            Self::OneLibrary => "exportLibrary.db",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickDevice {
    pub name: String,
    pub db_version: String,
    pub created_date: String,
    pub background_color: u8,
    pub track_count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickName {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickAlbum {
    pub id: u32,
    pub name: String,
    pub artist_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickCue {
    pub hot_cue: u32,
    pub time_ms: u32,
    pub loop_end_ms: Option<u32>,
    pub comment: String,
    pub color_index: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickTrack {
    pub id: u32,
    pub title: String,
    pub path: String,
    pub filename: String,
    pub artist_id: u32,
    pub album_id: u32,
    pub genre_id: u32,
    pub label_id: u32,
    pub key_id: u32,
    pub remixer_id: u32,
    pub composer_id: u32,
    pub original_artist_id: u32,
    pub artwork_id: u32,
    pub color_id: u8,
    pub rating: u8,
    pub tempo_x100: u32,
    pub duration_secs: u32,
    pub year: u16,
    pub track_number: u32,
    pub disc_number: u16,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub sample_depth: u16,
    pub file_size: u64,
    pub file_type: u16,
    pub play_count: u32,
    pub comment: String,
    pub mix_name: String,
    pub isrc: String,
    pub date_added: String,
    pub date_created: String,
    pub release_date: String,
    pub analyze_path: String,
    pub hot_cue_auto_load: bool,
    pub master_db_id: u32,
    pub master_content_id: u32,
    pub content_link: u32,
    pub analysed_bits: Option<i64>,
    pub cues: Vec<StickCue>,
    pub device_library_strings: BTreeMap<u8, Vec<u8>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickPlaylist {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub is_folder: bool,
    pub sort_order: u32,
    pub track_ids: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickHistory {
    pub id: u32,
    pub name: String,
    pub track_ids: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickMyTag {
    pub id: u32,
    pub name: String,
    pub parent_id: u32,
    pub sort_order: u32,
    pub is_category: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickMenu {
    pub columns: Vec<Vec<u8>>,
    pub categories: Vec<Vec<u8>>,
    pub sorts: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StickModel {
    pub device: StickDevice,
    pub tracks: Vec<StickTrack>,
    pub artists: Vec<StickName>,
    pub albums: Vec<StickAlbum>,
    pub genres: Vec<StickName>,
    pub labels: Vec<StickName>,
    pub keys: Vec<StickName>,
    pub colors: Vec<StickName>,
    pub artworks: Vec<StickName>,
    pub playlists: Vec<StickPlaylist>,
    pub histories: Vec<StickHistory>,
    pub menu: StickMenu,
    pub my_tags: Vec<StickMyTag>,
    pub track_tags: Vec<(u32, u32)>,
    pub my_tag_master_db_id: u32,
}

fn name_of(rows: &[StickName], id: u32) -> &str {
    if id == 0 {
        return "";
    }
    rows.iter().find(|row| row.id == id).map_or("", |row| row.name.as_str())
}

fn intern(rows: &mut Vec<StickName>, name: &str) -> u32 {
    if name.is_empty() {
        return 0;
    }
    if let Some(row) = rows.iter().find(|row| row.name == name) {
        return row.id;
    }
    let id = rows.iter().map(|row| row.id).max().unwrap_or(0) + 1;
    rows.push(StickName { id, name: name.to_owned() });
    id
}

impl StickModel {
    pub fn track(&self, id: u32) -> Option<&StickTrack> {
        self.tracks.iter().find(|track| track.id == id)
    }

    pub fn track_by_path(&self, path: &str) -> Option<&StickTrack> {
        self.tracks.iter().find(|track| track.path == path)
    }

    pub fn artist_name(&self, id: u32) -> &str {
        name_of(&self.artists, id)
    }

    pub fn genre_name(&self, id: u32) -> &str {
        name_of(&self.genres, id)
    }

    pub fn label_name(&self, id: u32) -> &str {
        name_of(&self.labels, id)
    }

    pub fn key_name(&self, id: u32) -> &str {
        name_of(&self.keys, id)
    }

    pub fn color_name(&self, id: u8) -> &str {
        name_of(&self.colors, u32::from(id))
    }

    pub fn artwork_path(&self, id: u32) -> &str {
        name_of(&self.artworks, id)
    }

    pub fn album(&self, id: u32) -> Option<&StickAlbum> {
        self.albums.iter().find(|album| id != 0 && album.id == id)
    }

    pub fn album_name(&self, id: u32) -> &str {
        self.album(id).map_or("", |album| album.name.as_str())
    }

    pub fn playlist(&self, id: u32) -> Option<&StickPlaylist> {
        self.playlists.iter().find(|playlist| playlist.id == id)
    }

    pub fn children(&self, parent_id: u32) -> Vec<&StickPlaylist> {
        let mut nodes: Vec<&StickPlaylist> =
            self.playlists.iter().filter(|playlist| playlist.parent_id == parent_id).collect();
        nodes.sort_by_key(|playlist| playlist.sort_order);
        nodes
    }

    pub fn playlist_tracks(&self, id: u32) -> Vec<&StickTrack> {
        self.playlist(id)
            .map(|playlist| playlist.track_ids.iter().filter_map(|&track_id| self.track(track_id)).collect())
            .unwrap_or_default()
    }

    pub fn tags_of(&self, track_id: u32) -> Vec<&StickMyTag> {
        self.track_tags
            .iter()
            .filter(|(track, _)| *track == track_id)
            .filter_map(|(_, tag_id)| self.my_tags.iter().find(|tag| tag.id == *tag_id))
            .collect()
    }

    pub fn intern_artist(&mut self, name: &str) -> u32 {
        intern(&mut self.artists, name)
    }

    pub fn intern_genre(&mut self, name: &str) -> u32 {
        intern(&mut self.genres, name)
    }

    pub fn intern_label(&mut self, name: &str) -> u32 {
        intern(&mut self.labels, name)
    }

    pub fn intern_key(&mut self, name: &str) -> u32 {
        intern(&mut self.keys, name)
    }

    pub fn intern_album(&mut self, name: &str, artist_id: u32) -> u32 {
        if name.is_empty() {
            return 0;
        }
        if let Some(album) = self.albums.iter().find(|album| album.name == name && album.artist_id == artist_id) {
            return album.id;
        }
        let id = self.albums.iter().map(|album| album.id).max().unwrap_or(0) + 1;
        self.albums.push(StickAlbum { id, name: name.to_owned(), artist_id });
        id
    }

    pub fn intern_artwork(&mut self, path: &str) -> u32 {
        intern(&mut self.artworks, path)
    }
}
