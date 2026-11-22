use std::collections::BTreeMap;

use crate::models::{AnalysisResult, Playlist, Track};
use crate::writer::{anlz, onelibrary};

use super::model::{StickAlbum, StickCue, StickDevice, StickModel, StickName, StickPlaylist, StickTrack};

const LEGACY_CONTENT_LINK: u32 = 0x0700;
const LEGACY_MASTER_DB_ID: u32 = 0x6a76_e5b6;
const LEGACY_SAMPLE_DEPTH: u16 = 16;
const LEGACY_STRING_2: [u8; 2] = [0x05, b'3'];
const LEGACY_STRING_3: [u8; 2] = [0x05, 0x01];

pub fn artwork_path(artwork_id: u32) -> String {
    format!("/PIONEER/Artwork/00001/a{artwork_id}.jpg")
}

fn filename_of(usb_path: &str) -> String {
    usb_path.rsplit('/').next().unwrap_or(usb_path).to_owned()
}

fn album_id(model: &mut StickModel, name: &str, artist_id: u32) -> u32 {
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

pub fn model_from_tracks(
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
    device: StickDevice,
) -> StickModel {
    let mut model = StickModel {
        keys: onelibrary::KEY_NAMES
            .iter()
            .enumerate()
            .map(|(index, name)| StickName { id: index as u32 + 1, name: (*name).to_owned() })
            .collect(),
        colors: onelibrary::COLOR_NAMES
            .iter()
            .enumerate()
            .map(|(index, name)| StickName { id: index as u32 + 1, name: (*name).to_owned() })
            .collect(),
        ..StickModel::default()
    };

    for (track, analysis) in tracks.iter().zip(analyses) {
        let artist_id = model.intern_artist(&track.artist);
        let remixer_id = model.intern_artist(&track.remixer);
        let album_id = album_id(&mut model, &track.album, artist_id);
        let genre_id = model.intern_genre(&track.genre);
        let label_id = model.intern_label(&track.label);
        let artwork_id = if track.artwork.is_some() {
            model.artworks.push(StickName { id: track.id, name: artwork_path(track.id) });
            track.id
        } else {
            0
        };
        let filename = filename_of(&track.usb_path);
        let key = if analysis.key.is_empty() { &track.key } else { &analysis.key };
        let tempo_x100 = if analysis.bpm > 0.0 { (analysis.bpm * 100.0).round() as u32 } else { track.tempo };
        model.tracks.push(StickTrack {
            id: track.id,
            title: track.title.clone(),
            path: track.usb_path.clone(),
            file_type: rbl_pdb::rows::audio_file_type(&filename),
            filename,
            artist_id,
            album_id,
            genre_id,
            label_id,
            key_id: onelibrary::key_name_to_id(key),
            remixer_id,
            artwork_id,
            tempo_x100,
            duration_secs: track.duration_secs.round() as u32,
            year: track.year,
            track_number: track.track_number,
            disc_number: track.disc_number,
            bitrate: track.bitrate,
            sample_rate: track.sample_rate,
            sample_depth: LEGACY_SAMPLE_DEPTH,
            file_size: track.file_size,
            comment: track.comment.clone(),
            date_added: device.created_date.clone(),
            date_created: device.created_date.clone(),
            release_date: if track.year > 0 { format!("{}-01-01", track.year) } else { String::new() },
            analyze_path: anlz::anlz_path_for_pdb(track),
            hot_cue_auto_load: true,
            master_db_id: LEGACY_MASTER_DB_ID,
            master_content_id: (track.id + 5) | 0x100,
            content_link: LEGACY_CONTENT_LINK,
            analysed_bits: Some(105),
            cues: analysis
                .cue_points
                .iter()
                .map(|cue| StickCue {
                    hot_cue: cue.hot_cue_number,
                    time_ms: cue.time_ms,
                    loop_end_ms: cue.loop_time_ms,
                    comment: cue.name.clone(),
                    color_index: cue.color.as_deref().and_then(crate::cue_color::table_index).map(|index| index as u8),
                })
                .collect(),
            device_library_strings: BTreeMap::from([
                (2, LEGACY_STRING_2.to_vec()),
                (3, LEGACY_STRING_3.to_vec()),
            ]),
            ..StickTrack::default()
        });
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
