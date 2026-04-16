use anyhow::{Context, Result};
use std::path::Path;

use crate::models::{AnalysisResult, Track};
use crate::writer::{anlz, pdb};

/// Write the complete Pioneer USB structure to the output directory.
pub fn write_usb(
    output_dir: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
) -> Result<()> {
    // Create directory structure
    let pioneer_dir = output_dir.join("PIONEER");
    let rekordbox_dir = pioneer_dir.join("rekordbox");
    let anlz_dir = pioneer_dir.join("USBANLZ");
    let contents_dir = output_dir.join("Contents");

    std::fs::create_dir_all(&rekordbox_dir)?;
    std::fs::create_dir_all(&anlz_dir)?;
    std::fs::create_dir_all(&contents_dir)?;

    println!("Writing audio files...");
    // Copy audio files
    for track in tracks {
        let dest = output_dir.join(track.usb_path.trim_start_matches('/'));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&track.source_path, &dest)
            .with_context(|| format!("Failed to copy {}", track.source_path.display()))?;
    }

    println!("Writing ANLZ files...");
    // Write ANLZ files (.DAT and .EXT)
    for (track, analysis) in tracks.iter().zip(analyses.iter()) {
        let dat_path = output_dir.join(anlz::anlz_path_for_track(track));
        anlz::write_anlz_dat(&dat_path, track, analysis)
            .with_context(|| format!("Failed to write ANLZ .DAT for track {}", track.id))?;

        let ext_path = output_dir.join(anlz::anlz_ext_path_for_track(track));
        anlz::write_anlz_ext(&ext_path, track, analysis)
            .with_context(|| format!("Failed to write ANLZ .EXT for track {}", track.id))?;
    }

    println!("Writing PDB database...");
    // Write PDB database
    let pdb_path = rekordbox_dir.join("export.pdb");
    pdb::write_pdb(&pdb_path, tracks)?;

    println!("USB structure written to {}", output_dir.display());
    Ok(())
}
