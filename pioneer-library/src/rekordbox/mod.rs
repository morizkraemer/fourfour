//! Import of existing Rekordbox libraries: the desktop `master.db`, a Pioneer
//! USB stick (OneLibrary) and `rekordbox.xml`.
//!
//! Flow: `load_*` reads a source into a [`SourceLibrary`], [`load_analyses`]
//! parses its ANLZ files (no library lock needed), then
//! [`LocalLibrary::rekordbox_preview`] and [`LocalLibrary::import_rekordbox`]
//! compare against and write into the library.

mod anlz;
mod masterdb;
mod model;
mod remap;
mod sidecar;
mod usb;
mod xml;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use pioneer_usb_writer::models::Track;

use crate::{LocalLibrary, queries};

pub use anlz::{LoadedAnalysis, load_all as load_analyses};
pub use model::*;
pub use remap::{CaseInsensitiveResolver, PathRemap, detect_common_root};
pub use sidecar::ImportedMeta;

pub(crate) use anlz::to_cue_points;

pub fn load_masterdb(db_path: &Path, share_dir: Option<&Path>) -> Result<SourceLibrary> {
    masterdb::load(db_path, share_dir)
}

pub fn load_usb(usb_root: &Path) -> Result<SourceLibrary> {
    usb::load(usb_root)
}

pub fn load_xml(xml_path: &Path) -> Result<SourceLibrary> {
    xml::load(xml_path)
}

/// Replacement prefix from the options, or the one the source implies, plus the
/// common root detected across all of the source's track paths.
pub fn resolve_remap(source: &SourceLibrary, options: &ImportOptions) -> (String, Option<PathRemap>) {
    let paths: Vec<&str> = source.tracks.iter().map(|t| t.path.as_str()).collect();
    let detected_root = detect_common_root(&paths);
    let remap = match options.remap_to.as_deref().filter(|to| !to.is_empty()) {
        Some(to) => Some(PathRemap {
            from_prefix: options
                .remap_from
                .clone()
                .filter(|from| !from.is_empty())
                .unwrap_or_else(|| detected_root.clone()),
            to_prefix: to.to_string(),
        }),
        None => source.implied_remap.clone(),
    };
    (detected_root, remap)
}

struct Located {
    path: PathBuf,
    found: bool,
}

fn locate(source: &SourceLibrary, remap: Option<&PathRemap>) -> Vec<Located> {
    let mut resolver = CaseInsensitiveResolver::default();
    source
        .tracks
        .iter()
        .map(|t| {
            let candidate = remap.map_or_else(|| PathBuf::from(&t.path), |r| r.apply(&t.path));
            match resolver.resolve(&candidate) {
                Some(path) => Located { path, found: true },
                None => Located { path: candidate, found: false },
            }
        })
        .collect()
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

fn fingerprint(path: &Path, file_size: u64) -> Option<(String, u64)> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    (file_size > 0).then_some((name, file_size))
}

/// Existing library tracks, matched by case-insensitive path or by
/// (lowercase file name, file size).
struct LibraryIndex {
    by_path: HashMap<String, i64>,
    by_fingerprint: HashMap<(String, u64), i64>,
}

impl LibraryIndex {
    fn build(library: &LocalLibrary) -> Result<Self> {
        let mut by_path = HashMap::new();
        let mut by_fingerprint = HashMap::new();
        for track in library.get_all_tracks()? {
            let id = track.id as i64;
            by_path.insert(path_key(&track.source_path), id);
            if let Some(fp) = fingerprint(&track.source_path, track.file_size) {
                by_fingerprint.entry(fp).or_insert(id);
            }
        }
        Ok(Self { by_path, by_fingerprint })
    }

    fn find(&self, path: &Path, file_size: u64) -> Option<i64> {
        self.by_path.get(&path_key(path)).copied().or_else(|| {
            fingerprint(path, file_size).and_then(|fp| self.by_fingerprint.get(&fp).copied())
        })
    }

    fn insert(&mut self, path: &Path, file_size: u64, id: i64) {
        self.by_path.insert(path_key(path), id);
        if let Some(fp) = fingerprint(path, file_size) {
            self.by_fingerprint.entry(fp).or_insert(id);
        }
    }
}

/// Playlist display names with folders flattened into "Folder / Playlist".
fn flattened_playlists(source: &SourceLibrary) -> Vec<(&SourcePlaylist, String)> {
    let by_id: HashMap<&str, &SourcePlaylist> =
        source.playlists.iter().map(|p| (p.source_id.as_str(), p)).collect();
    let mut out: Vec<_> = source
        .playlists
        .iter()
        .filter(|p| !p.is_folder)
        .map(|p| {
            let mut names = vec![p.name.as_str()];
            let mut parent = p.parent_id.as_deref();
            for _ in 0..16 {
                let Some(folder) = parent.and_then(|id| by_id.get(id)) else { break };
                names.push(folder.name.as_str());
                parent = folder.parent_id.as_deref();
            }
            names.reverse();
            (p, names.join(" / "))
        })
        .collect();
    out.sort_by(|(a, an), (b, bn)| (a.sequence, an).cmp(&(b.sequence, bn)));
    out
}

fn sanitize_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

fn usb_path_for(track: &Track, file: &Path) -> String {
    let artist = if track.artist.is_empty() { "Unknown" } else { &track.artist };
    let name = file.file_name().map_or_else(|| "unknown.mp3".into(), |n| n.to_string_lossy());
    format!("/Contents/{}/{}", sanitize_component(artist), sanitize_component(&name))
}

/// The existing track with every empty field taken from `imported`, or `None`
/// when nothing was empty.
fn fill_empty(existing: &Track, imported: &Track) -> Option<Track> {
    let mut merged = existing.clone();
    macro_rules! fill_text {
        ($($field:ident),*) => { $(if merged.$field.is_empty() { merged.$field = imported.$field.clone(); })* };
    }
    macro_rules! fill_num {
        ($($field:ident),*) => { $(if merged.$field == 0 { merged.$field = imported.$field; })* };
    }
    fill_text!(title, artist, album, genre, label, remixer, comment, key, usb_path);
    fill_num!(year, disc_number, track_number, tempo, sample_rate, bitrate, file_size);
    if merged.duration_secs == 0.0 {
        merged.duration_secs = imported.duration_secs;
    }
    let changed = merged.title != existing.title
        || merged.artist != existing.artist
        || merged.album != existing.album
        || merged.genre != existing.genre
        || merged.label != existing.label
        || merged.remixer != existing.remixer
        || merged.comment != existing.comment
        || merged.key != existing.key
        || merged.usb_path != existing.usb_path
        || merged.year != existing.year
        || merged.disc_number != existing.disc_number
        || merged.track_number != existing.track_number
        || merged.tempo != existing.tempo
        || merged.sample_rate != existing.sample_rate
        || merged.bitrate != existing.bitrate
        || merged.file_size != existing.file_size
        || merged.duration_secs != existing.duration_secs;
    changed.then_some(merged)
}

fn cue_counts<'a>(cues: impl Iterator<Item = &'a ImportedCue>) -> (u32, u32) {
    cues.fold((0, 0), |(hot, memory), c| {
        if c.hot_cue_number > 0 { (hot + 1, memory) } else { (hot, memory + 1) }
    })
}

impl LocalLibrary {
    pub fn imported_meta(&self, track_id: i64) -> Option<ImportedMeta> {
        ImportedMeta::read(&self.track_anlz_dir(track_id)?)
    }

    /// Let re-analysis replace the imported beat grid and cues of a track again.
    pub fn release_imported_locks(&self, track_id: i64) -> Result<()> {
        let Some(dir) = self.track_anlz_dir(track_id) else { return Ok(()) };
        if let Some(mut meta) = ImportedMeta::read(&dir) {
            meta.grid_locked = false;
            meta.cues_locked = false;
            meta.write(&dir)?;
        }
        Ok(())
    }

    pub fn rekordbox_preview(
        &self,
        source: &SourceLibrary,
        options: &ImportOptions,
    ) -> Result<ImportPreview> {
        let (detected_root, remap) = resolve_remap(source, options);
        let located = locate(source, remap.as_ref());
        let index = LibraryIndex::build(self)?;

        let mut files_found = 0;
        let mut missing_examples = Vec::new();
        let mut conflicts = 0;
        let mut tracks_with_grid = 0;
        let (mut hot_cues, mut memory_cues) = (0, 0);
        for (track, loc) in source.tracks.iter().zip(&located) {
            if loc.found {
                files_found += 1;
            } else if missing_examples.len() < 10 {
                missing_examples.push(loc.path.to_string_lossy().to_string());
            }
            if index.find(&loc.path, track.track.file_size).is_some() {
                conflicts += 1;
            }
            let anlz = track.anlz_dat.as_ref().filter(|p| p.exists());
            if anlz.is_some() {
                tracks_with_grid += 1;
            }
            let ext_cues;
            let cues: &[ImportedCue] = match anlz {
                Some(dat) if track.cues.is_empty() => {
                    ext_cues = anlz::read_ext_cues(&dat.with_extension("EXT"));
                    &ext_cues
                }
                _ => &track.cues,
            };
            let (hot, memory) = cue_counts(cues.iter());
            hot_cues += hot;
            memory_cues += memory;
        }

        let existing_names: HashSet<String> =
            self.get_all_playlists()?.into_iter().map(|p| p.name).collect();
        let named = flattened_playlists(source);
        let playlist_conflicts =
            named.iter().filter(|(_, name)| existing_names.contains(name)).count() as u32;
        let tracks = source.tracks.len() as u32;

        Ok(ImportPreview {
            source: source.kind,
            tracks,
            playlists: named.len() as u32,
            folders: source.playlists.iter().filter(|p| p.is_folder).count() as u32,
            skipped_smart_playlists: source.skipped_smart_playlists,
            hot_cues,
            memory_cues,
            tracks_with_grid,
            detected_root,
            remap,
            files_found,
            files_missing: tracks - files_found,
            missing_examples,
            conflicts,
            new_tracks: tracks - conflicts,
            playlist_conflicts,
        })
    }

    /// Convenience wrapper that parses the ANLZ files itself.
    pub fn import_rekordbox_from(
        &self,
        source: &SourceLibrary,
        options: &ImportOptions,
        progress: &(dyn Fn(ImportProgress) + Sync),
    ) -> Result<RekordboxImportReport> {
        let analyses = load_analyses(source, progress);
        self.import_rekordbox(source, &analyses, options, progress)
    }

    /// Merge policy: tracks already in the library (same path ignoring case, or
    /// same file name and size) keep every field the user has; only empty
    /// fields are filled. Existing analyses are never replaced and nothing is
    /// ever deleted. New tracks get the imported analysis, which is locked
    /// against re-analysis.
    pub fn import_rekordbox(
        &self,
        source: &SourceLibrary,
        analyses: &HashMap<String, LoadedAnalysis>,
        options: &ImportOptions,
        progress: &(dyn Fn(ImportProgress) + Sync),
    ) -> Result<RekordboxImportReport> {
        let (_, remap) = resolve_remap(source, options);
        let located = locate(source, remap.as_ref());
        let mut index = LibraryIndex::build(self)?;
        let mut report = RekordboxImportReport::default();
        let mut library_ids: HashMap<&str, i64> = HashMap::new();

        let tx = self.conn.unchecked_transaction()?;
        let total = source.tracks.len() as u32;
        for (i, (src, loc)) in source.tracks.iter().zip(&located).enumerate() {
            let done = i as u32 + 1;
            if done % 25 == 0 || done == total {
                progress(ImportProgress { stage: "tracks", done, total });
            }
            if loc.found {
                report.files_found += 1;
            } else {
                report.files_missing += 1;
                if !options.include_missing {
                    report.tracks_skipped_missing += 1;
                    continue;
                }
            }

            let mut track = src.track.clone();
            track.source_path = loc.path.clone();
            if track.usb_path.is_empty() {
                track.usb_path = usb_path_for(&track, &loc.path);
            }

            let id = match index.find(&loc.path, track.file_size) {
                Some(id) => {
                    if let Some(existing) = queries::select_track(&self.conn, id)?
                        && let Some(filled) = fill_empty(&existing, &track)
                    {
                        queries::update_track(&self.conn, id, &filled)?;
                    }
                    report.tracks_merged += 1;
                    id
                }
                None => {
                    let id = queries::insert_track(&self.conn, &track)?;
                    index.insert(&loc.path, track.file_size, id);
                    report.tracks_imported += 1;
                    id
                }
            };
            library_ids.insert(&src.source_id, id);

            if let Some(art) = src.artwork.as_ref()
                && queries::select_artwork(&self.conn, id)?.is_none()
                && let Ok(bytes) = std::fs::read(art)
            {
                queries::upsert_artwork(&self.conn, id, &bytes)?;
            }

            self.import_analysis_and_meta(id, source.kind, src, analyses.get(&src.source_id), &mut report)?;
        }

        tx.commit()?;
        self.import_playlists(source, &library_ids, progress, &mut report)?;
        Ok(report)
    }

    fn import_analysis_and_meta(
        &self,
        id: i64,
        kind: SourceKind,
        src: &SourceTrack,
        loaded: Option<&LoadedAnalysis>,
        report: &mut RekordboxImportReport,
    ) -> Result<()> {
        let has_analysis = queries::select_analysis_index(&self.conn, id)?.is_some();
        let dir = self.track_anlz_dir(id);
        let existing_meta = dir.as_deref().and_then(ImportedMeta::read);

        let mut meta = existing_meta.clone().unwrap_or(ImportedMeta {
            source: kind,
            source_id: src.source_id.clone(),
            grid_locked: false,
            cues_locked: false,
            rating: None,
            color: None,
            cues: Vec::new(),
            grid: Vec::new(),
        });
        meta.rating = meta.rating.or(src.rating);
        meta.color = meta.color.clone().or_else(|| src.color.clone());

        if !has_analysis {
            let cues = loaded.map_or(src.cues.as_slice(), |l| l.cues.as_slice());
            if let Some(l) = loaded {
                self.set_analysis(id, &l.analysis)?;
                if !l.analysis.beat_grid.beats.is_empty() {
                    report.grids_imported += 1;
                    meta.grid_locked = true;
                }
            } else if let Some(grid) = src.grid.as_ref() {
                report.grids_imported += 1;
                meta.grid = grid.clone();
                meta.grid_locked = true;
            }
            if !cues.is_empty() {
                let (hot, memory) = cue_counts(cues.iter());
                report.hot_cues_imported += hot;
                report.memory_cues_imported += memory;
                if loaded.is_none() {
                    report.cues_without_analysis += 1;
                }
                meta.cues = cues.to_vec();
                meta.cues_locked = true;
            }
        }

        let has_content = meta.rating.is_some()
            || meta.color.is_some()
            || meta.grid_locked
            || meta.cues_locked;
        if has_content && let Some(dir) = dir {
            meta.write(&dir)?;
        }
        Ok(())
    }

    fn import_playlists(
        &self,
        source: &SourceLibrary,
        library_ids: &HashMap<&str, i64>,
        progress: &(dyn Fn(ImportProgress) + Sync),
        report: &mut RekordboxImportReport,
    ) -> Result<()> {
        let mut existing: HashMap<String, (i64, Vec<i64>)> = self
            .get_all_playlists()?
            .into_iter()
            .map(|p| (p.name.clone(), (p.id as i64, p.track_ids.iter().map(|&t| t as i64).collect())))
            .collect();

        let named = flattened_playlists(source);
        let total = named.len() as u32;
        for (i, (playlist, name)) in named.iter().enumerate() {
            progress(ImportProgress { stage: "playlists", done: i as u32 + 1, total });
            let mut seen = HashSet::new();
            let ids: Vec<i64> = playlist
                .track_ids
                .iter()
                .filter_map(|source_id| library_ids.get(source_id.as_str()).copied())
                .filter(|id| seen.insert(*id))
                .collect();
            if ids.is_empty() && !playlist.track_ids.is_empty() {
                continue;
            }
            match existing.get_mut(name) {
                Some((playlist_id, current)) => {
                    let missing: Vec<i64> = ids.into_iter().filter(|id| !current.contains(id)).collect();
                    if !missing.is_empty() {
                        current.extend(missing);
                        self.set_playlist_tracks(*playlist_id, current)?;
                    }
                    report.playlists_merged += 1;
                }
                None => {
                    let playlist_id = self.create_playlist(name)?;
                    self.set_playlist_tracks(playlist_id, &ids)?;
                    existing.insert(name.clone(), (playlist_id, ids));
                    report.playlists_imported += 1;
                }
            }
        }
        Ok(())
    }
}
