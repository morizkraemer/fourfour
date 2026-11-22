use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::{AnalysisResult, Playlist, Track};
use crate::writer::{anlz, filesystem};

use super::model::{LibraryKind, StickDevice, StickModel, StickTrack};
use super::read::Stick;
use super::{convert, device_library, ext_library, one_library};

pub const ANALYSIS_EXTENSIONS: [&str; 3] = ["DAT", "EXT", "2EX"];
const ARTWORK_VARIANTS: [(&str, &str); 4] = [("a", ""), ("a", "_m"), ("b", ""), ("b", "_m")];

#[derive(Debug, Clone)]
pub enum AudioSource {
    Copy(PathBuf),
    AlreadyThere,
}

#[derive(Debug, Clone)]
pub enum AnalysisSource<'a> {
    Carry(PathBuf),
    Author { track: &'a Track, analysis: &'a AnalysisResult },
    Missing,
}

#[derive(Debug, Clone)]
pub enum ArtworkSource<'a> {
    Encode(&'a [u8]),
    CopyFromStick(PathBuf),
    Missing,
}

#[derive(Debug, Clone)]
pub struct TrackFiles<'a> {
    pub track_id: u32,
    pub audio: AudioSource,
    pub analysis: AnalysisSource<'a>,
    pub artwork: ArtworkSource<'a>,
}

#[derive(Debug, Clone)]
pub struct FreshExport<'a> {
    pub model: &'a StickModel,
    pub files: Vec<TrackFiles<'a>>,
}

fn on_stick(root: &Path, stick_path: &str) -> PathBuf {
    root.join(stick_path.trim_start_matches('/'))
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("Cannot create {}", parent.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("Cannot write {}", path.display()))
}

pub fn carried_analysis_bytes(source: &Path, stick_path: &str) -> Result<Vec<u8>> {
    let parsed = rbl_anlz::Anlz::read(source).with_context(|| format!("Cannot read {}", source.display()))?;
    Ok(parsed.with_path(stick_path))
}

fn write_analysis(root: &Path, track: &StickTrack, source: &AnalysisSource) -> Result<()> {
    if track.analyze_path.is_empty() {
        return Ok(());
    }
    let dat = on_stick(root, &track.analyze_path);
    match source {
        AnalysisSource::Missing => Ok(()),
        AnalysisSource::Carry(source_dat) => {
            for extension in ANALYSIS_EXTENSIONS {
                let source = source_dat.with_extension(extension);
                if source.is_file() {
                    write_file(&dat.with_extension(extension), &carried_analysis_bytes(&source, &track.path)?)?;
                }
            }
            Ok(())
        }
        AnalysisSource::Author { track: legacy, analysis } => {
            anyhow::ensure!(
                legacy.usb_path == track.path,
                "Analysis for {} was handed to the stick track {}",
                legacy.usb_path,
                track.path
            );
            anlz::write_anlz_dat(&dat, legacy, analysis)
                .with_context(|| format!("Cannot write the analysis for {}", track.path))?;
            anlz::write_anlz_ext(&dat.with_extension("EXT"), legacy, analysis)
                .with_context(|| format!("Cannot write the analysis for {}", track.path))?;
            anlz::write_anlz_2ex(&dat.with_extension("2EX"), legacy, analysis)
                .with_context(|| format!("Cannot write the analysis for {}", track.path))?;
            Ok(())
        }
    }
}

fn artwork_variant(small_path: &str, prefix: &str, suffix: &str) -> Option<String> {
    let (dir, file) = small_path.rsplit_once('/')?;
    let stem = file.strip_suffix(".jpg")?;
    let number = stem.strip_prefix('a')?;
    Some(format!("{dir}/{prefix}{number}{suffix}.jpg"))
}

pub fn artwork_variants(small_path: &str) -> Vec<String> {
    ARTWORK_VARIANTS.iter().filter_map(|(prefix, suffix)| artwork_variant(small_path, prefix, suffix)).collect()
}

fn write_artwork(root: &Path, model: &StickModel, track: &StickTrack, source: &ArtworkSource) -> Result<()> {
    let small_path = model.artwork_path(track.artwork_id);
    if track.artwork_id == 0 || small_path.is_empty() {
        return Ok(());
    }
    match source {
        ArtworkSource::Missing => Ok(()),
        ArtworkSource::Encode(image) => {
            let dir = on_stick(root, small_path);
            let dir = dir.parent().unwrap_or(root);
            std::fs::create_dir_all(dir).with_context(|| format!("Cannot create {}", dir.display()))?;
            filesystem::write_artwork(dir, track.artwork_id, image)
                .with_context(|| format!("Cannot write the cover of {}", track.path))
        }
        ArtworkSource::CopyFromStick(source_root) => {
            for (prefix, suffix) in ARTWORK_VARIANTS {
                let Some(variant) = artwork_variant(small_path, prefix, suffix) else { continue };
                let source = on_stick(source_root, &variant);
                if source.is_file() {
                    let bytes = std::fs::read(&source).with_context(|| format!("Cannot read {}", source.display()))?;
                    write_file(&on_stick(root, &variant), &bytes)?;
                }
            }
            Ok(())
        }
    }
}

pub fn write_databases(root: &Path, model: &StickModel) -> Result<()> {
    let rekordbox = root.join("PIONEER").join("rekordbox");
    std::fs::create_dir_all(&rekordbox).with_context(|| format!("Cannot create {}", rekordbox.display()))?;
    let pdb = device_library::build(model)?;
    write_file(&rekordbox.join(LibraryKind::DeviceLibrary.file_name()), &pdb)?;
    let ext = ext_library::ExtLibrary::of(model);
    if !ext.is_empty() {
        write_file(&rekordbox.join(ext_library::FILE_NAME), &ext_library::build(&ext))?;
    }
    one_library::write(&rekordbox.join(LibraryKind::OneLibrary.file_name()), model)
}

pub fn write_fresh(root: &Path, export: &FreshExport) -> Result<()> {
    for dir in ["PIONEER/rekordbox", "PIONEER/USBANLZ", "Contents"] {
        let dir = root.join(dir);
        std::fs::create_dir_all(&dir).with_context(|| format!("Cannot create {}", dir.display()))?;
    }
    for files in &export.files {
        let track = export
            .model
            .track(files.track_id)
            .with_context(|| format!("Files were given for track {} that is not in the stick model", files.track_id))?;
        if let AudioSource::Copy(source) = &files.audio {
            let target = on_stick(root, &track.path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).with_context(|| format!("Cannot create {}", parent.display()))?;
            }
            std::fs::copy(source, &target).with_context(|| format!("Cannot copy {}", source.display()))?;
        }
        write_analysis(root, track, &files.analysis)?;
        write_artwork(root, export.model, track, &files.artwork)?;
    }
    write_databases(root, export.model)
}

pub fn write_usb_v2(
    root: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
    device: StickDevice,
) -> Result<StickModel> {
    anyhow::ensure!(
        tracks.len() == analyses.len(),
        "tracks ({}) and analyses ({}) must have the same length",
        tracks.len(),
        analyses.len()
    );
    let model = convert::model_from_tracks(tracks, analyses, playlists, device);
    let files = tracks
        .iter()
        .zip(analyses)
        .map(|(track, analysis)| TrackFiles {
            track_id: track.id,
            audio: AudioSource::Copy(track.source_path.clone()),
            analysis: AnalysisSource::Author { track, analysis },
            artwork: track.artwork.as_deref().map_or(ArtworkSource::Missing, ArtworkSource::Encode),
        })
        .collect();
    write_fresh(root, &FreshExport { model: &model, files })?;
    Ok(model)
}

pub fn copy_stick(source: &Stick, kind: LibraryKind, root: &Path, copy_audio: bool) -> Result<()> {
    let model = match kind {
        LibraryKind::DeviceLibrary => source.device_library.as_ref(),
        LibraryKind::OneLibrary => source.one_library.as_ref(),
    }
    .with_context(|| format!("{} has no {}", source.root.display(), kind.file_name()))?;
    let files = model
        .tracks
        .iter()
        .map(|track| TrackFiles {
            track_id: track.id,
            audio: if copy_audio { AudioSource::Copy(source.audio_path(track)) } else { AudioSource::AlreadyThere },
            analysis: source.analysis_path(track).map_or(AnalysisSource::Missing, AnalysisSource::Carry),
            artwork: ArtworkSource::CopyFromStick(source.root.clone()),
        })
        .collect();
    write_fresh(root, &FreshExport { model, files })
}
