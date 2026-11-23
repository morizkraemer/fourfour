use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rbl_anlz::Anlz;

use super::model::{LibraryKind, StickModel, StickTrack};
use super::{device_library, ext_library, one_library};

pub const PIONEER_DIR_NAMES: [&str; 2] = ["PIONEER", ".PIONEER"];
pub const SETTINGS_FILES: [&str; 5] =
    ["DEVSETTING.DAT", "MYSETTING.DAT", "MYSETTING2.DAT", "DJMMYSETTING.DAT", "djprofile.nxs"];

#[derive(Debug, Clone, Default)]
pub struct Stick {
    pub root: PathBuf,
    pub pioneer_dir: Option<PathBuf>,
    pub device_library: Option<StickModel>,
    pub one_library: Option<StickModel>,
}

#[derive(Debug, Clone, Default)]
pub struct StickAnalysis {
    pub dat: Option<Anlz>,
    pub ext: Option<Anlz>,
    pub two_ex: Option<Anlz>,
}

pub fn pioneer_dir(root: &Path) -> Option<PathBuf> {
    PIONEER_DIR_NAMES.iter().map(|name| root.join(name)).find(|dir| dir.join("rekordbox").is_dir())
}

impl Stick {
    pub fn read(root: &Path) -> Result<Self> {
        let Some(pioneer_dir) = pioneer_dir(root) else {
            return Ok(Self { root: root.to_path_buf(), ..Self::default() });
        };
        let rekordbox = pioneer_dir.join("rekordbox");

        let pdb_path = rekordbox.join(LibraryKind::DeviceLibrary.file_name());
        let device_library = if pdb_path.is_file() {
            let bytes = std::fs::read(&pdb_path).with_context(|| format!("Cannot read {}", pdb_path.display()))?;
            let mut model =
                device_library::read(&bytes).with_context(|| format!("Cannot read {}", pdb_path.display()))?;
            let ext_path = rekordbox.join(ext_library::FILE_NAME);
            if ext_path.is_file() {
                let bytes = std::fs::read(&ext_path).with_context(|| format!("Cannot read {}", ext_path.display()))?;
                ext_library::read(&bytes)
                    .with_context(|| format!("Cannot read {}", ext_path.display()))?
                    .put_into(&mut model);
            }
            Some(model)
        } else {
            None
        };

        let db_path = rekordbox.join(LibraryKind::OneLibrary.file_name());
        let one_library = if db_path.is_file() { Some(one_library::read(&db_path)?) } else { None };

        Ok(Self { root: root.to_path_buf(), pioneer_dir: Some(pioneer_dir), device_library, one_library })
    }

    pub fn settings_files(&self) -> Vec<PathBuf> {
        let Some(pioneer_dir) = &self.pioneer_dir else { return Vec::new() };
        SETTINGS_FILES.iter().map(|name| pioneer_dir.join(name)).filter(|path| path.is_file()).collect()
    }

    pub fn has_library(&self) -> bool {
        self.device_library.is_some() || self.one_library.is_some()
    }

    pub fn primary_kind(&self) -> Option<LibraryKind> {
        match (&self.device_library, &self.one_library) {
            (Some(device), Some(one)) if one.tracks.len() > device.tracks.len() => Some(LibraryKind::OneLibrary),
            (Some(_), _) => Some(LibraryKind::DeviceLibrary),
            (None, Some(_)) => Some(LibraryKind::OneLibrary),
            (None, None) => None,
        }
    }

    pub fn library(&self) -> Option<&StickModel> {
        match self.primary_kind()? {
            LibraryKind::DeviceLibrary => self.device_library.as_ref(),
            LibraryKind::OneLibrary => self.one_library.as_ref(),
        }
    }

    pub fn track_count(&self) -> usize {
        self.library().map_or(0, |library| library.tracks.len())
    }

    pub fn audio_path(&self, track: &StickTrack) -> PathBuf {
        self.root.join(track.path.trim_start_matches('/'))
    }

    pub fn analysis_path(&self, track: &StickTrack) -> Option<PathBuf> {
        (!track.analyze_path.is_empty()).then(|| self.root.join(track.analyze_path.trim_start_matches('/')))
    }

    pub fn read_analysis(&self, track: &StickTrack) -> Result<StickAnalysis> {
        match self.analysis_path(track) {
            Some(dat) => read_analysis(&dat),
            None => Ok(StickAnalysis::default()),
        }
    }
}

fn read_companion(path: &Path) -> Result<Option<Anlz>> {
    if !path.is_file() {
        return Ok(None);
    }
    Anlz::read(path).map(Some).with_context(|| format!("Cannot read {}", path.display()))
}

pub fn read_analysis(dat_path: &Path) -> Result<StickAnalysis> {
    Ok(StickAnalysis {
        dat: read_companion(dat_path)?,
        ext: read_companion(&dat_path.with_extension("EXT"))?,
        two_ex: read_companion(&dat_path.with_extension("2EX"))?,
    })
}

impl StickAnalysis {
    pub fn is_empty(&self) -> bool {
        self.dat.is_none() && self.ext.is_none() && self.two_ex.is_none()
    }

    pub fn beats(&self) -> Vec<rbl_anlz::Beat> {
        self.dat.as_ref().and_then(Anlz::beat_grid).unwrap_or_default()
    }

    pub fn cues(&self) -> Vec<rbl_anlz::CueEntry> {
        let extended = self.ext.as_ref().map(Anlz::cue_entries).unwrap_or_default();
        if !extended.is_empty() {
            return extended;
        }
        self.dat.as_ref().map(Anlz::legacy_cue_entries).unwrap_or_default()
    }

    pub fn has_phrases(&self) -> bool {
        self.ext.as_ref().is_some_and(|ext| ext.section(b"PSSI").is_some())
    }
}
