use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::writer::anlz::anlz_dir_for_path;

use super::model::StickModel;
use super::paths;
use super::read::{self, Stick};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub subject: String,
    pub message: String,
}

fn problem(subject: impl Into<String>, message: impl Into<String>) -> Problem {
    Problem { subject: subject.into(), message: message.into() }
}

pub fn describe(problems: &[Problem]) -> String {
    let shown: Vec<String> =
        problems.iter().take(5).map(|problem| format!("{}: {}", problem.subject, problem.message)).collect();
    let more = problems.len().saturating_sub(shown.len());
    if more > 0 { format!("{} (and {more} more)", shown.join("; ")) } else { shown.join("; ") }
}

pub fn check_model(model: &StickModel) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut track_ids = BTreeSet::new();
    let mut folded_paths = BTreeMap::new();
    for track in &model.tracks {
        if track.id == 0 || !track_ids.insert(track.id) {
            problems.push(problem(&track.path, format!("track id {} is used twice or is zero", track.id)));
        }
        if !track.path.starts_with('/') {
            problems.push(problem(&track.path, "the track path does not start at the stick root"));
        }
        if let Some(other) = folded_paths.insert(paths::fold(&track.path), track.id) {
            problems.push(problem(&track.path, format!("same file as track {other} apart from letter case")));
        }
        for (what, id, known) in [
            ("artist", track.artist_id, model.artists.iter().any(|row| row.id == track.artist_id)),
            ("album", track.album_id, model.albums.iter().any(|row| row.id == track.album_id)),
            ("genre", track.genre_id, model.genres.iter().any(|row| row.id == track.genre_id)),
            ("label", track.label_id, model.labels.iter().any(|row| row.id == track.label_id)),
            ("key", track.key_id, model.keys.iter().any(|row| row.id == track.key_id)),
            ("cover", track.artwork_id, model.artworks.iter().any(|row| row.id == track.artwork_id)),
        ] {
            if id != 0 && !known {
                problems.push(problem(&track.path, format!("{what} {id} is not in the {what} table")));
            }
        }
    }

    let mut playlist_ids = BTreeSet::new();
    let parents: BTreeMap<u32, u32> = model.playlists.iter().map(|playlist| (playlist.id, playlist.parent_id)).collect();
    for playlist in &model.playlists {
        if playlist.id == 0 || !playlist_ids.insert(playlist.id) {
            problems.push(problem(&playlist.name, format!("playlist id {} is used twice or is zero", playlist.id)));
        }
        let mut seen = BTreeSet::from([playlist.id]);
        let mut parent = playlist.parent_id;
        while parent != 0 {
            if !seen.insert(parent) {
                problems.push(problem(&playlist.name, "the playlist folders form a loop"));
                break;
            }
            match parents.get(&parent) {
                Some(next) => parent = *next,
                None => {
                    problems.push(problem(&playlist.name, format!("parent folder {parent} does not exist")));
                    break;
                }
            }
        }
        for track_id in &playlist.track_ids {
            if !track_ids.contains(track_id) {
                problems.push(problem(&playlist.name, format!("lists track {track_id}, which is not on the stick")));
            }
        }
    }
    problems
}

fn first_existing(roots: &[&Path], stick_path: &str) -> Option<PathBuf> {
    roots.iter().map(|root| root.join(stick_path.trim_start_matches('/'))).find(|path| path.is_file())
}

pub fn check_files(model: &StickModel, roots: &[&Path], required_paths: &BTreeSet<String>) -> Vec<Problem> {
    let mut problems = Vec::new();
    for track in model.tracks.iter().filter(|track| required_paths.contains(&track.path)) {
        match first_existing(roots, &track.path) {
            None => problems.push(problem(&track.path, "the audio file is missing")),
            Some(audio) => {
                let size = std::fs::metadata(&audio).map(|meta| meta.len()).unwrap_or(0);
                if size != track.file_size && track.file_size <= u64::from(u32::MAX) {
                    problems.push(problem(&track.path, format!("the audio file has {size} bytes, the library says {}", track.file_size)));
                }
            }
        }
        if track.analyze_path.is_empty() {
            problems.push(problem(&track.path, "no analysis file is named"));
            continue;
        }
        let expected_dir = format!("/{}", anlz_dir_for_path(&track.path));
        if !track.analyze_path.starts_with(&expected_dir) {
            problems.push(problem(&track.path, "the analysis folder does not belong to this file path"));
        }
        match first_existing(roots, &track.analyze_path) {
            None => problems.push(problem(&track.path, "the analysis file is missing")),
            Some(dat) => match read::read_analysis(&dat) {
                Err(error) => problems.push(problem(&track.path, format!("the analysis file cannot be read: {error:#}"))),
                Ok(analysis) => {
                    for (name, file) in [("DAT", &analysis.dat), ("EXT", &analysis.ext), ("2EX", &analysis.two_ex)] {
                        if let Some(named) = file.as_ref().and_then(|file| file.path()) {
                            if named != track.path {
                                problems.push(problem(&track.path, format!("the {name} analysis file names another track")));
                            }
                        }
                    }
                }
            },
        }
    }
    problems
}

fn summary(model: &StickModel) -> (BTreeMap<u32, String>, BTreeMap<u32, (String, Vec<u32>)>) {
    (
        model.tracks.iter().map(|track| (track.id, track.path.clone())).collect(),
        model.playlists.iter().map(|playlist| (playlist.id, (playlist.name.clone(), playlist.track_ids.clone()))).collect(),
    )
}

pub fn check_databases_agree(stick: &Stick) -> Vec<Problem> {
    let (Some(device), Some(one)) = (&stick.device_library, &stick.one_library) else {
        return vec![problem("databases", "export.pdb or exportLibrary.db is missing")];
    };
    if summary(device) == summary(one) {
        Vec::new()
    } else {
        vec![problem("databases", "export.pdb and exportLibrary.db do not hold the same tracks and playlists")]
    }
}

pub fn check_written(database_root: &Path, expected: &StickModel) -> Result<Vec<Problem>> {
    let stick = Stick::read(database_root)?;
    let mut problems = check_databases_agree(&stick);
    if let Some(device) = &stick.device_library {
        if summary(device) != summary(expected) {
            problems.push(problem("export.pdb", "does not hold what was planned"));
        }
        problems.extend(check_model(device));
    }
    Ok(problems)
}

pub fn verify_stick(root: &Path, required_paths: &BTreeSet<String>) -> Result<Vec<Problem>> {
    let stick = Stick::read(root)?;
    let Some(model) = stick.library() else { return Ok(Vec::new()) };
    let mut problems = check_model(model);
    if stick.device_library.is_some() && stick.one_library.as_ref().is_some_and(|one| !one.tracks.is_empty()) {
        problems.extend(check_databases_agree(&stick));
    }
    problems.extend(check_files(model, &[root], required_paths));
    Ok(problems)
}
