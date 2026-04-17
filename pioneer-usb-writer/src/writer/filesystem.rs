use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::ImageFormat;
use std::io::Cursor;
use std::path::Path;

use anyhow::ensure;
use crate::models::{AnalysisResult, Playlist, Track};
use crate::writer::{anlz, onelibrary, pdb};

/// Write the complete Pioneer USB structure to the output directory.
pub fn write_usb(
    output_dir: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
) -> Result<()> {
    ensure!(
        tracks.len() == analyses.len(),
        "tracks ({}) and analyses ({}) must have the same length",
        tracks.len(),
        analyses.len()
    );

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

    println!("Writing artwork...");
    let artwork_dir = pioneer_dir.join("Artwork/00001");
    std::fs::create_dir_all(&artwork_dir)?;
    for track in tracks {
        if let Some(ref art_data) = track.artwork {
            if let Err(e) = write_artwork(&artwork_dir, track.id, art_data) {
                eprintln!("Warning: artwork failed for track {}: {}", track.id, e);
            }
        }
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
    pdb::write_pdb(&pdb_path, tracks, playlists)?;

    println!("Writing OneLibrary database...");
    onelibrary::write_onelibrary(output_dir, tracks, analyses, playlists)?;

    println!("USB structure written to {}", output_dir.display());
    Ok(())
}

/// Write artwork files for a track: a{id}.jpg (80x80), a{id}_m.jpg (240x240), and b copies.
fn write_artwork(artwork_dir: &Path, artwork_id: u32, image_data: &[u8]) -> Result<()> {
    let img = image::load_from_memory(image_data)
        .context("Failed to decode cover art")?;

    let small = img.resize_to_fill(80, 80, FilterType::Lanczos3);
    let medium = img.resize_to_fill(240, 240, FilterType::Lanczos3);

    let mut small_buf = Cursor::new(Vec::new());
    small.write_to(&mut small_buf, ImageFormat::Jpeg)?;
    let small_bytes = small_buf.into_inner();

    let mut medium_buf = Cursor::new(Vec::new());
    medium.write_to(&mut medium_buf, ImageFormat::Jpeg)?;
    let medium_bytes = medium_buf.into_inner();

    for prefix in ["a", "b"] {
        std::fs::write(artwork_dir.join(format!("{}{}.jpg", prefix, artwork_id)), &small_bytes)?;
        std::fs::write(artwork_dir.join(format!("{}{}_m.jpg", prefix, artwork_id)), &medium_bytes)?;
    }

    Ok(())
}
