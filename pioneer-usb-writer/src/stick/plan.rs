use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::models::{AnalysisResult, Playlist, SyncReport, Track};

use super::dump::fnv64;
use super::emit::{self, AnalysisSource, ArtworkSource, AudioSource, FreshExport, TrackFiles};
use super::model::{StickCue, StickDevice, StickModel, StickName, StickPlaylist, StickTrack};
use super::read::Stick;
use super::{convert, paths};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ownership {
    #[serde(default)]
    pub paths: BTreeSet<String>,
    #[serde(default)]
    pub playlist_ids: BTreeSet<u32>,
    #[serde(default)]
    pub fingerprints: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Wanted<'a> {
    pub tracks: &'a [Track],
    pub analyses: &'a [AnalysisResult],
    pub playlists: &'a [Playlist],
    pub labels: &'a [TrackLabel],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrackLabel {
    pub rating: u8,
    pub color_id: u8,
}

pub const MAX_RATING: u8 = 5;
pub const COLOR_COUNT: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackAction {
    Add,
    Replace,
    Update,
    Unchanged,
    AdoptedForeign,
}

#[derive(Debug, Clone)]
pub struct PlannedTrack {
    pub wanted_index: usize,
    pub stick_id: u32,
    pub action: TrackAction,
    pub track: Track,
}

#[derive(Debug, Clone)]
pub struct StickPlan {
    pub model: StickModel,
    pub tracks: Vec<PlannedTrack>,
    pub removed: Vec<StickTrack>,
    pub obsolete_artwork: Vec<String>,
    pub foreign_track_count: usize,
    pub ownership: Ownership,
    pub writes_databases: bool,
}

impl StickPlan {
    pub fn report(&self) -> SyncReport {
        let count = |action: TrackAction| self.tracks.iter().filter(|track| track.action == action).count() as u32;
        SyncReport {
            tracks_added: count(TrackAction::Add),
            tracks_updated: count(TrackAction::Update),
            tracks_replaced: count(TrackAction::Replace),
            tracks_removed: self.removed.len() as u32,
            tracks_unchanged: count(TrackAction::Unchanged) + count(TrackAction::AdoptedForeign),
        }
    }

    pub fn is_noop(&self) -> bool {
        !self.writes_databases
            && self.removed.is_empty()
            && self.tracks.iter().all(|track| matches!(track.action, TrackAction::Unchanged | TrackAction::AdoptedForeign))
    }
}

pub fn fingerprint(track: &Track, analysis: &AnalysisResult) -> u64 {
    let analysis_bytes = serde_json::to_vec(analysis).unwrap_or_default();
    let artwork = track.artwork.as_deref().map_or(0, fnv64);
    fnv64(&analysis_bytes) ^ artwork.rotate_left(17) ^ fnv64(track.usb_path.as_bytes()).rotate_left(31)
}

fn without_database_extras(track: &StickTrack) -> StickTrack {
    StickTrack { cues: Vec::new(), analysed_bits: None, ..track.clone() }
}

fn comparable(model: &StickModel) -> StickModel {
    StickModel { tracks: model.tracks.iter().map(without_database_extras).collect(), ..model.clone() }
}

fn refuse_if_the_databases_disagree(existing: &Stick) -> Result<()> {
    let (Some(device), Some(one)) = (&existing.device_library, &existing.one_library) else { return Ok(()) };
    let (primary, secondary, secondary_name) = if one.tracks.len() > device.tracks.len() {
        (one, device, "export.pdb")
    } else {
        (device, one, "exportLibrary.db")
    };
    let known: BTreeSet<&str> = primary.tracks.iter().map(|track| track.path.as_str()).collect();
    let only_in_secondary = secondary.tracks.iter().filter(|track| !known.contains(track.path.as_str())).count();
    anyhow::ensure!(
        only_in_secondary == 0,
        "The two libraries on this stick disagree: {secondary_name} has {only_in_secondary} tracks the other one does not. Nothing was written.",
    );
    Ok(())
}

fn foreign_cues(existing: &Stick, track: &StickTrack) -> Vec<StickCue> {
    if !track.cues.is_empty() {
        return track.cues.clone();
    }
    if let Some(known) = existing.one_library.as_ref().and_then(|one| one.track(track.id)).filter(|known| known.path == track.path) {
        return known.cues.clone();
    }
    existing
        .read_analysis(track)
        .map(|analysis| {
            analysis
                .cues()
                .into_iter()
                .map(|cue| StickCue {
                    hot_cue: cue.hot_cue,
                    time_ms: cue.time_ms,
                    loop_end_ms: (cue.kind == 2 && cue.loop_time_ms > cue.time_ms).then_some(cue.loop_time_ms),
                    comment: cue.comment.unwrap_or_default(),
                    color_index: cue.color_code.filter(|&code| code > 0),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn next_id(ids: impl Iterator<Item = u32>) -> u32 {
    ids.max().unwrap_or(0) + 1
}

fn referenced_ids(tracks: &[&StickTrack]) -> [BTreeSet<u32>; 5] {
    let mut artists = BTreeSet::new();
    let mut albums = BTreeSet::new();
    let mut genres = BTreeSet::new();
    let mut labels = BTreeSet::new();
    let mut artworks = BTreeSet::new();
    for track in tracks {
        artists.extend([track.artist_id, track.remixer_id, track.composer_id, track.original_artist_id]);
        albums.insert(track.album_id);
        genres.insert(track.genre_id);
        labels.insert(track.label_id);
        artworks.insert(track.artwork_id);
    }
    [artists, albums, genres, labels, artworks]
}

fn drop_rows_only_our_old_tracks_used(model: &mut StickModel, our_old_rows: &[&StickTrack]) -> Vec<String> {
    let [old_artists, old_albums, old_genres, old_labels, old_artworks] = referenced_ids(our_old_rows);
    let now: Vec<&StickTrack> = model.tracks.iter().collect();
    let [mut artists, albums, genres, labels, artworks] = referenced_ids(&now);
    model.albums.retain(|album| !old_albums.contains(&album.id) || albums.contains(&album.id));
    artists.extend(model.albums.iter().map(|album| album.artist_id));
    let old_album_artists: BTreeSet<u32> = our_old_rows
        .iter()
        .filter_map(|track| old_albums.contains(&track.album_id).then_some(track.artist_id))
        .collect();
    model.artists.retain(|artist| {
        !(old_artists.contains(&artist.id) || old_album_artists.contains(&artist.id)) || artists.contains(&artist.id)
    });
    model.genres.retain(|genre| !old_genres.contains(&genre.id) || genres.contains(&genre.id));
    model.labels.retain(|label| !old_labels.contains(&label.id) || labels.contains(&label.id));
    let obsolete: Vec<String> = model
        .artworks
        .iter()
        .filter(|artwork| old_artworks.contains(&artwork.id) && !artworks.contains(&artwork.id))
        .map(|artwork| artwork.name.clone())
        .collect();
    model.artworks.retain(|artwork| !old_artworks.contains(&artwork.id) || artworks.contains(&artwork.id));
    obsolete
}

fn place_playlists(
    model: &mut StickModel,
    old_owned: Vec<StickPlaylist>,
    wanted: &[Playlist],
    stick_track_ids: &BTreeMap<u32, u32>,
) -> BTreeSet<u32> {
    let mut reusable = old_owned;
    let mut highest = next_id(model.playlists.iter().chain(&reusable).map(|playlist| playlist.id)) - 1;
    let mut stick_ids: BTreeMap<u32, u32> = BTreeMap::new();
    let mut next_order: BTreeMap<u32, u32> = BTreeMap::new();
    for playlist in &model.playlists {
        let order = next_order.entry(playlist.parent_id).or_default();
        *order = (*order).max(playlist.sort_order + 1);
    }

    let mut pending: Vec<&Playlist> = wanted.iter().collect();
    while !pending.is_empty() {
        let before = pending.len();
        let mut waiting = Vec::new();
        for playlist in pending {
            let parent = match playlist.parent_id {
                0 => Some(0),
                parent => stick_ids.get(&parent).copied(),
            };
            let Some(parent) = parent else {
                waiting.push(playlist);
                continue;
            };
            let reused = reusable.iter().position(|old| {
                old.parent_id == parent && old.name == playlist.name && old.is_folder == playlist.is_folder
            });
            let id = match reused {
                Some(at) => reusable.remove(at).id,
                None => {
                    highest += 1;
                    highest
                }
            };
            stick_ids.insert(playlist.id, id);
            let order = next_order.entry(parent).or_default();
            model.playlists.push(StickPlaylist {
                id,
                parent_id: parent,
                name: playlist.name.clone(),
                is_folder: playlist.is_folder,
                sort_order: *order,
                track_ids: playlist.track_ids.iter().filter_map(|id| stick_track_ids.get(id).copied()).collect(),
            });
            *order += 1;
        }
        if waiting.len() == before {
            break;
        }
        pending = waiting;
    }
    stick_ids.into_values().collect()
}

pub fn plan(existing: &Stick, wanted: Wanted, owned: &Ownership, device: StickDevice) -> Result<StickPlan> {
    anyhow::ensure!(
        wanted.tracks.len() == wanted.analyses.len(),
        "tracks ({}) and analyses ({}) must have the same length",
        wanted.tracks.len(),
        wanted.analyses.len()
    );
    refuse_if_the_databases_disagree(existing)?;

    let base = existing.library().cloned().unwrap_or_default();
    let mut model = base.clone();
    convert::seed_fixed_tables(&mut model);
    model.device = StickDevice {
        name: if base.device.name.is_empty() { device.name } else { base.device.name.clone() },
        created_date: if base.device.created_date.len() == 10 { base.device.created_date.clone() } else { device.created_date },
        db_version: if base.device.db_version.is_empty() {
            rbl_pdb::rows::PROPERTY_DB_VERSION.to_owned()
        } else {
            base.device.db_version.clone()
        },
        background_color: base.device.background_color,
        track_count: 0,
    };
    let date = model.device.created_date.clone();

    let is_ours = |track: &StickTrack| owned.paths.contains(&track.path);
    let our_old_rows: Vec<&StickTrack> = base.tracks.iter().filter(|track| is_ours(track)).collect();
    let foreign_paths: BTreeSet<&str> =
        base.tracks.iter().filter(|track| !is_ours(track)).map(|track| track.path.as_str()).collect();
    let mut taken: BTreeSet<String> = foreign_paths.iter().map(|path| paths::fold(path)).collect();
    let mut next_track_id = next_id(base.tracks.iter().map(|track| track.id));

    let mut planned = Vec::with_capacity(wanted.tracks.len());
    let mut stick_track_ids: BTreeMap<u32, u32> = BTreeMap::new();
    let mut kept_paths: BTreeSet<String> = BTreeSet::new();
    let mut new_rows: Vec<StickTrack> = Vec::new();
    let mut ownership = Ownership::default();

    for (wanted_index, (track, analysis)) in wanted.tracks.iter().zip(wanted.analyses).enumerate() {
        let safe = paths::safe_stick_path(&track.usb_path);
        if let Some(foreign) = base.tracks.iter().find(|row| row.path == safe && foreign_paths.contains(safe.as_str())) {
            stick_track_ids.insert(track.id, foreign.id);
            planned.push(PlannedTrack {
                wanted_index,
                stick_id: foreign.id,
                action: TrackAction::AdoptedForeign,
                track: Track { id: foreign.id, usb_path: safe, ..track.clone() },
            });
            continue;
        }
        let path = paths::unique_stick_path(&track.usb_path, &mut taken);
        let old = our_old_rows.iter().find(|row| row.path == path).copied();
        let stick_id = old.map_or_else(
            || {
                next_track_id += 1;
                next_track_id - 1
            },
            |row| row.id,
        );
        let final_track = Track { id: stick_id, usb_path: path.clone(), ..track.clone() };
        let mut row = match old {
            Some(old) => {
                let mut row = old.clone();
                convert::refresh_stick_track(&mut row, &final_track, analysis);
                row.analysed_bits = convert::new_stick_track(&final_track, analysis, &date).analysed_bits;
                row
            }
            None => convert::new_stick_track(&final_track, analysis, &date),
        };
        convert::link_names(&mut model, &mut row, &final_track, analysis);
        if let Some(label) = wanted.labels.get(wanted_index) {
            row.rating = label.rating.min(MAX_RATING);
            row.color_id = if label.color_id <= COLOR_COUNT { label.color_id } else { 0 };
        }
        row.artwork_id = match (&final_track.artwork, old.map_or(0, |old| old.artwork_id)) {
            (None, _) => 0,
            (Some(_), kept) if kept != 0 && model.artworks.iter().any(|artwork| artwork.id == kept) => kept,
            (Some(_), _) => {
                let free = !model.artworks.iter().any(|artwork| artwork.id == stick_id);
                let id = if free { stick_id } else { next_id(model.artworks.iter().map(|artwork| artwork.id)) };
                model.artworks.push(StickName { id, name: convert::artwork_path(id) });
                id
            }
        };

        let print = fingerprint(&final_track, analysis);
        let action = match old {
            None => TrackAction::Add,
            Some(old) if old.file_size != row.file_size => TrackAction::Replace,
            Some(_) if owned.fingerprints.get(&path) != Some(&print) => TrackAction::Update,
            Some(old) if without_database_extras(old) != without_database_extras(&row) => TrackAction::Update,
            Some(_) => TrackAction::Unchanged,
        };
        ownership.paths.insert(path.clone());
        ownership.fingerprints.insert(path.clone(), print);
        kept_paths.insert(path);
        stick_track_ids.insert(track.id, stick_id);
        new_rows.push(row);
        planned.push(PlannedTrack { wanted_index, stick_id, action, track: final_track });
    }

    let removed: Vec<StickTrack> =
        our_old_rows.iter().filter(|row| !kept_paths.contains(&row.path)).map(|row| (*row).clone()).collect();
    let mut by_id: BTreeMap<u32, StickTrack> = new_rows.into_iter().map(|row| (row.id, row)).collect();
    let mut tracks: Vec<StickTrack> = Vec::with_capacity(base.tracks.len() + by_id.len());
    for row in &base.tracks {
        if is_ours(row) {
            if let Some(updated) = by_id.remove(&row.id).filter(|_| kept_paths.contains(&row.path)) {
                tracks.push(updated);
            }
        } else {
            tracks.push(StickTrack { cues: foreign_cues(existing, row), ..row.clone() });
        }
    }
    tracks.extend(by_id.into_values());
    model.tracks = tracks;
    let foreign_track_count = model.tracks.len() - kept_paths.len();

    let present: BTreeSet<u32> = model.tracks.iter().map(|track| track.id).collect();
    let (old_owned_playlists, foreign_playlists): (Vec<StickPlaylist>, Vec<StickPlaylist>) =
        std::mem::take(&mut model.playlists).into_iter().partition(|playlist| owned.playlist_ids.contains(&playlist.id));
    model.playlists = foreign_playlists;
    for playlist in &mut model.playlists {
        playlist.track_ids.retain(|id| present.contains(id));
    }
    for history in &mut model.histories {
        history.track_ids.retain(|id| present.contains(id));
    }
    model.track_tags.retain(|(track_id, _)| present.contains(track_id));
    ownership.playlist_ids = place_playlists(&mut model, old_owned_playlists, wanted.playlists, &stick_track_ids);

    let obsolete_artwork = drop_rows_only_our_old_tracks_used(&mut model, &our_old_rows);
    model.device.track_count = model.tracks.len() as u32;

    let one_library_matches = existing.one_library.as_ref().is_some_and(|one| {
        one.tracks.len() == model.tracks.len() && one.playlists.len() == model.playlists.len()
    });
    let asks_for_anything = !planned.is_empty()
        || !removed.is_empty()
        || !wanted.playlists.is_empty()
        || !owned.playlist_ids.is_empty();
    let writes_databases = asks_for_anything
        && (existing.device_library.is_none()
            || !one_library_matches
            || comparable(&model) != comparable(&base)
            || planned.iter().any(|track| !matches!(track.action, TrackAction::Unchanged | TrackAction::AdoptedForeign))
            || !removed.is_empty());

    Ok(StickPlan { model, tracks: planned, removed, obsolete_artwork, foreign_track_count, ownership, writes_databases })
}

fn remove_file_and_empty_parent(path: &Path) {
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

fn remove_our_files(root: &Path, plan: &StickPlan) {
    let on_stick = |path: &str| root.join(path.trim_start_matches('/'));
    let audio_in_use: BTreeSet<String> = plan.model.tracks.iter().map(|track| paths::fold(&track.path)).collect();
    let analysis_in_use: BTreeSet<&str> = plan.model.tracks.iter().map(|track| track.analyze_path.as_str()).collect();
    for row in &plan.removed {
        if !audio_in_use.contains(&paths::fold(&row.path)) {
            remove_file_and_empty_parent(&on_stick(&row.path));
        }
        if !row.analyze_path.is_empty() && !analysis_in_use.contains(row.analyze_path.as_str()) {
            let dat = on_stick(&row.analyze_path);
            for extension in emit::ANALYSIS_EXTENSIONS {
                let _ = std::fs::remove_file(dat.with_extension(extension));
            }
            if let Some(dir) = dat.parent() {
                let _ = std::fs::remove_dir(dir);
                if let Some(shard) = dir.parent() {
                    let _ = std::fs::remove_dir(shard);
                }
            }
        }
    }
    for small in &plan.obsolete_artwork {
        for variant in emit::artwork_variants(small) {
            let _ = std::fs::remove_file(on_stick(&variant));
        }
    }
}

pub fn apply(root: &Path, plan: &StickPlan, wanted: Wanted) -> Result<SyncReport> {
    let report = plan.report();
    if plan.is_noop() {
        return Ok(report);
    }
    let files: Vec<TrackFiles> = plan
        .tracks
        .iter()
        .filter(|planned| !matches!(planned.action, TrackAction::Unchanged | TrackAction::AdoptedForeign))
        .map(|planned| {
            let source = &wanted.tracks[planned.wanted_index];
            let rewrite_audio = matches!(planned.action, TrackAction::Add | TrackAction::Replace);
            TrackFiles {
                track_id: planned.stick_id,
                audio: if rewrite_audio { AudioSource::Copy(source.source_path.clone()) } else { AudioSource::AlreadyThere },
                analysis: AnalysisSource::Author { track: &planned.track, analysis: &wanted.analyses[planned.wanted_index] },
                artwork: source.artwork.as_deref().map_or(ArtworkSource::Missing, ArtworkSource::Encode),
            }
        })
        .collect();
    emit::write_fresh(root, &FreshExport { model: &plan.model, files })
        .with_context(|| format!("Cannot write the library on {}", root.display()))?;
    remove_our_files(root, plan);
    Ok(report)
}
