use std::collections::BTreeMap;

use crate::models::{AnalysisResult, Playlist, Track};
use crate::writer::{anlz, onelibrary};

use super::model::{StickAlbum, StickCue, StickDevice, StickModel, StickName, StickPlaylist, StickTrack};

const LEGACY_CONTENT_LINK: u32 = 0x0700;
const LEGACY_MASTER_DB_ID: u32 = 0x6a76_e5b6;
const LEGACY_SAMPLE_DEPTH: u16 = 16;
const LEGACY_ANALYSED_BITS: i64 = 105;
const LEGACY_STRING_2: [u8; 2] = [0x05, b'3'];
const LEGACY_STRING_3: [u8; 2] = [0x05, 0x01];

pub fn artwork_path(artwork_id: u32) -> String {
    format!("/PIONEER/Artwork/00001/a{artwork_id}.jpg")
}

pub const TRACK_COLORS: [(&str, &str); 8] = [
    ("Pink", "#ff007f"),
    ("Red", "#ff0000"),
    ("Orange", "#ffa500"),
    ("Yellow", "#ffff00"),
    ("Green", "#00ff00"),
    ("Aqua", "#25fde9"),
    ("Blue", "#0000ff"),
    ("Purple", "#660099"),
];

pub fn color_id_of(label: &str) -> u8 {
    let label = label.trim();
    TRACK_COLORS
        .iter()
        .position(|(name, hex)| name.eq_ignore_ascii_case(label) || hex.eq_ignore_ascii_case(label))
        .map_or(0, |index| index as u8 + 1)
}

pub fn filename_of(usb_path: &str) -> String {
    usb_path.rsplit('/').next().unwrap_or(usb_path).to_owned()
}

pub fn key_name(track: &Track, analysis: &AnalysisResult) -> &'static str {
    let key = if analysis.key.is_empty() { &track.key } else { &analysis.key };
    match onelibrary::key_name_to_id(key) {
        0 => "",
        id => onelibrary::KEY_NAMES[id as usize - 1],
    }
}

pub fn cues_of(analysis: &AnalysisResult) -> Vec<StickCue> {
    analysis
        .cue_points
        .iter()
        .map(|cue| StickCue {
            hot_cue: cue.hot_cue_number,
            time_ms: cue.time_ms,
            loop_end_ms: cue.loop_time_ms,
            comment: cue.name.clone(),
            color_index: cue.color.as_deref().and_then(crate::cue_color::table_index),
        })
        .collect()
}

pub fn tempo_x100(track: &Track, analysis: &AnalysisResult) -> u32 {
    if analysis.bpm > 0.0 { (analysis.bpm * 100.0).round() as u32 } else { track.tempo }
}

pub fn new_stick_track(track: &Track, analysis: &AnalysisResult, date: &str) -> StickTrack {
    let filename = filename_of(&track.usb_path);
    let mut row = StickTrack {
        id: track.id,
        file_type: rbl_pdb::rows::audio_file_type(&filename),
        sample_depth: LEGACY_SAMPLE_DEPTH,
        date_added: date.to_owned(),
        date_created: date.to_owned(),
        hot_cue_auto_load: true,
        master_db_id: LEGACY_MASTER_DB_ID,
        master_content_id: (track.id + 5) | 0x100,
        content_link: LEGACY_CONTENT_LINK,
        analysed_bits: Some(LEGACY_ANALYSED_BITS),
        device_library_strings: BTreeMap::from([(2, LEGACY_STRING_2.to_vec()), (3, LEGACY_STRING_3.to_vec())]),
        ..StickTrack::default()
    };
    refresh_stick_track(&mut row, track, analysis);
    row
}

pub fn refresh_stick_track(row: &mut StickTrack, track: &Track, analysis: &AnalysisResult) {
    row.title = track.title.clone();
    row.path = track.usb_path.clone();
    row.filename = filename_of(&track.usb_path);
    row.file_type = rbl_pdb::rows::audio_file_type(&row.filename);
    row.tempo_x100 = tempo_x100(track, analysis);
    row.duration_secs = track.duration_secs.round() as u32;
    row.year = track.year;
    row.track_number = track.track_number;
    row.disc_number = track.disc_number;
    row.bitrate = track.bitrate;
    row.sample_rate = track.sample_rate;
    row.file_size = track.file_size;
    row.comment = track.comment.clone();
    row.release_date = if track.year > 0 { format!("{}-01-01", track.year) } else { String::new() };
    row.analyze_path = anlz::anlz_path_for_pdb(track);
    row.cues = cues_of(analysis);
}

pub fn album_id(model: &mut StickModel, name: &str, artist_id: u32) -> u32 {
    if name.is_empty() {
        return 0;
    }
    if let Some(album) = model.albums.iter().find(|album| album.name == name) {
        return album.id;
    }
    let id = model.albums.iter().map(|album| album.id).max().unwrap_or(0) + 1;
    model.albums.push(StickAlbum { id, name: name.to_owned(), artist_id });
    id
}

pub fn link_names(model: &mut StickModel, row: &mut StickTrack, track: &Track, analysis: &AnalysisResult) {
    row.artist_id = model.intern_artist(&track.artist);
    row.remixer_id = model.intern_artist(&track.remixer);
    row.album_id = album_id(model, &track.album, row.artist_id);
    row.genre_id = model.intern_genre(&track.genre);
    row.label_id = model.intern_label(&track.label);
    row.key_id = model.intern_key(key_name(track, analysis));
}

pub fn seed_fixed_tables(model: &mut StickModel) {
    let numbered = |names: &[&str]| -> Vec<StickName> {
        names.iter().enumerate().map(|(index, name)| StickName { id: index as u32 + 1, name: (*name).to_owned() }).collect()
    };
    if model.keys.is_empty() {
        model.keys = numbered(&onelibrary::KEY_NAMES);
    }
    if model.colors.is_empty() {
        model.colors = numbered(&onelibrary::COLOR_NAMES);
    }
}

pub fn model_from_tracks(
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
    device: StickDevice,
) -> StickModel {
    let mut model = StickModel::default();
    seed_fixed_tables(&mut model);

    for (track, analysis) in tracks.iter().zip(analyses) {
        let mut row = new_stick_track(track, analysis, &device.created_date);
        link_names(&mut model, &mut row, track, analysis);
        if track.artwork.is_some() {
            model.artworks.push(StickName { id: track.id, name: artwork_path(track.id) });
            row.artwork_id = track.id;
        }
        model.tracks.push(row);
    }

    let mut sibling_count: BTreeMap<u32, u32> = BTreeMap::new();
    for playlist in playlists {
        let position = sibling_count.entry(playlist.parent_id).or_default();
        model.playlists.push(StickPlaylist {
            id: playlist.id,
            parent_id: playlist.parent_id,
            name: playlist.name.clone(),
            is_folder: playlist.is_folder,
            sort_order: *position,
            track_ids: playlist.track_ids.clone(),
        });
        *position += 1;
    }

    model.device = StickDevice { track_count: tracks.len() as u32, ..device };
    if model.device.db_version.is_empty() {
        model.device.db_version = rbl_pdb::rows::PROPERTY_DB_VERSION.to_owned();
    }
    model
}
