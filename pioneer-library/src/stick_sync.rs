use std::path::Path;

use anyhow::{Context, Result};
use pioneer_usb_writer::models::SyncReport;
use pioneer_usb_writer::stick::plan::{self, Ownership, StickPlan, Wanted};
use pioneer_usb_writer::stick::{Stick, StickDevice};

use crate::sticks::{self, LastSync, OwnedFile, StickManifest};
use crate::{prepare_for_usb, LocalLibrary};

pub fn ownership_on(stick: &Stick, manifest: Option<&StickManifest>) -> Ownership {
    let Some(manifest) = manifest else { return Ownership::default() };
    if let Some(recorded) = &manifest.v2 {
        return recorded.clone();
    }
    Ownership {
        paths: manifest.files.iter().map(|file| file.usb_path.clone()).collect(),
        playlist_ids: stick
            .library()
            .map(|library| library.playlists.iter().map(|playlist| playlist.id).collect())
            .unwrap_or_default(),
        fingerprints: Default::default(),
    }
}

fn record(root: &Path, plan: &StickPlan, library_ids: &[i64], report: &SyncReport) -> Result<()> {
    let mut manifest = sticks::manifest_or_fresh(root)?;
    manifest.files = plan
        .tracks
        .iter()
        .filter(|planned| plan.ownership.paths.contains(&planned.track.usb_path))
        .map(|planned| OwnedFile {
            usb_path: planned.track.usb_path.clone(),
            track_id: library_ids[planned.wanted_index],
            size_bytes: planned.track.file_size,
        })
        .collect();
    manifest.v2 = Some(plan.ownership.clone());
    manifest.last_sync = Some(LastSync { at: chrono::Utc::now().to_rfc3339(), report: report.clone() });
    sticks::write_manifest(root, &manifest)
}

impl LocalLibrary {
    pub fn sync_usb_v2(&self, output_dir: &Path, device_name: &str) -> Result<SyncReport> {
        let (tracks, analyses) = self.get_analyzed_tracks()?;
        let library_ids: Vec<i64> = tracks.iter().map(|track| i64::from(track.id)).collect();
        let playlists = self.get_all_playlists()?;
        let (tracks, analyses, playlists) = prepare_for_usb(tracks, analyses, playlists);
        let wanted = Wanted { tracks: &tracks, analyses: &analyses, playlists: &playlists };

        let stick = Stick::read(output_dir).with_context(|| format!("Cannot read the library on {}", output_dir.display()))?;
        let manifest = sticks::read_manifest(output_dir)?;
        let owned = ownership_on(&stick, manifest.as_ref());
        let device = StickDevice {
            name: device_name.to_owned(),
            created_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            ..StickDevice::default()
        };
        let plan = plan::plan(&stick, wanted, &owned, device)?;
        let report = plan::apply(output_dir, &plan, wanted)?;
        let first_v2_sync = manifest.as_ref().is_none_or(|manifest| manifest.v2.is_none());
        if !plan.is_noop() || first_v2_sync {
            record(output_dir, &plan, &library_ids, &report)?;
        }
        Ok(report)
    }
}
