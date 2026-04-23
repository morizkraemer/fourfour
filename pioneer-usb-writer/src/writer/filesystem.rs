use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::ImageFormat;
use std::io::Cursor;
use std::path::Path;

use anyhow::ensure;
use crate::models::{AnalysisResult, ExistingTrack, Playlist, SyncAction, SyncReport, Track};
use crate::reader;
use crate::writer::{anlz, onelibrary, pdb, sync};

/// Write the complete Pioneer USB structure to `output_dir`.
///
/// `tracks` and `analyses` must have the same length — the function returns an
/// error immediately if they do not.
///
/// # Directory structure created
/// ```text
/// <output_dir>/
///   Contents/<artist>/<filename>      — copies of the source audio files
///   PIONEER/
///     rekordbox/
///       export.pdb                    — legacy DeviceSQL binary database
///       exportLibrary.db              — SQLCipher-encrypted OneLibrary database
///     USBANLZ/P{xxx}/{hash}/
///       ANLZ0000.DAT                  — beat grid, waveform preview, cue points
///       ANLZ0000.EXT                  — color waveforms, extended beat grid
///     Artwork/00001/
///       a{id}.jpg / a{id}_m.jpg       — 80×80 and 240×240 cover art (JPEG)
///       b{id}.jpg / b{id}_m.jpg       — duplicate set used by CDJ for redundancy
/// ```
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

    // Copy audio files
    for track in tracks {
        let dest = output_dir.join(track.usb_path.trim_start_matches('/'));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&track.source_path, &dest)
            .with_context(|| format!("Failed to copy {}", track.source_path.display()))?;
    }

    let artwork_dir = pioneer_dir.join("Artwork/00001");
    std::fs::create_dir_all(&artwork_dir)?;
    for track in tracks {
        if let Some(ref art_data) = track.artwork {
            if let Err(e) = write_artwork(&artwork_dir, track.id, art_data) {
                eprintln!("Warning: artwork failed for track {}: {}", track.id, e);
            }
        }
    }

    // Write ANLZ files (.DAT and .EXT)
    for (track, analysis) in tracks.iter().zip(analyses.iter()) {
        let dat_path = output_dir.join(anlz::anlz_path_for_track(track));
        anlz::write_anlz_dat(&dat_path, track, analysis)
            .with_context(|| format!("Failed to write ANLZ .DAT for track {}", track.id))?;

        let ext_path = output_dir.join(anlz::anlz_ext_path_for_track(track));
        anlz::write_anlz_ext(&ext_path, track, analysis)
            .with_context(|| format!("Failed to write ANLZ .EXT for track {}", track.id))?;

        // Write .2EX for full 3-band color on modern players (CDJ-3000X, XDJ-AZ,
        // OPUS-QUAD) and CDJ-3000 (non-X) when available. The CDJ prefers .2EX
        // over .EXT PWV4 when both are present, which avoids the proprietary
        // PWV4 format that we have not yet reverse-engineered.
        let ex_path = output_dir.join(anlz::anlz_2ex_path_for_track(track));
        anlz::write_anlz_2ex(&ex_path, track, analysis)
            .with_context(|| format!("Failed to write ANLZ .2EX for track {}", track.id))?;
    }

    // Write PDB database
    let pdb_path = rekordbox_dir.join("export.pdb");
    pdb::write_pdb(&pdb_path, tracks, playlists)?;

    onelibrary::write_onelibrary(output_dir, tracks, analyses, playlists)?;

    // macOS creates ._* AppleDouble resource-fork files on FAT/exFAT volumes.
    // CDJ firmware may try to parse these as ANLZ/PDB files and crash.
    clean_dot_underscore(output_dir);

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

/// Incrementally synchronize the USB drive to match the given track list.
///
/// Reads existing state from the USB, computes a diff, and performs only the
/// necessary file operations (skipping audio copy for unchanged tracks).
/// Both databases are always rebuilt from scratch for consistency.
///
/// Returns a [`SyncReport`] summarizing what was done.
///
/// The existing [`write_usb`] remains available as the full-overwrite path.
pub fn sync_usb(
    output_dir: &Path,
    tracks: &[Track],
    analyses: &[AnalysisResult],
    playlists: &[Playlist],
) -> Result<SyncReport> {
    ensure!(
        tracks.len() == analyses.len(),
        "tracks ({}) and analyses ({}) must have the same length",
        tracks.len(),
        analyses.len()
    );

    // 1. Read existing USB state
    let existing = reader::read_usb_state(output_dir)?;

    // 2. Compute sync plan
    let plan = sync::compute_sync_plan(tracks, analyses, playlists, existing.as_ref());
    let report = sync::build_sync_report(&plan);

    // 3. Create directory structure (idempotent)
    let pioneer_dir = output_dir.join("PIONEER");
    let rekordbox_dir = pioneer_dir.join("rekordbox");
    let anlz_base = pioneer_dir.join("USBANLZ");
    let contents_dir = output_dir.join("Contents");
    let artwork_dir = pioneer_dir.join("Artwork/00001");

    std::fs::create_dir_all(&rekordbox_dir)?;
    std::fs::create_dir_all(&anlz_base)?;
    std::fs::create_dir_all(&contents_dir)?;
    std::fs::create_dir_all(&artwork_dir)?;

    // 4. Execute file operations per entry
    for entry in &plan.entries {
        match entry.action {
            SyncAction::Add | SyncAction::Replace => {
                // Copy audio file
                let dest = output_dir.join(entry.track.usb_path.trim_start_matches('/'));
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&entry.track.source_path, &dest).with_context(|| {
                    format!("Failed to copy {}", entry.track.source_path.display())
                })?;

                // Write ANLZ
                let dat_path = output_dir.join(anlz::anlz_path_for_track(entry.track));
                anlz::write_anlz_dat(&dat_path, entry.track, entry.analysis)
                    .with_context(|| format!("Failed to write ANLZ .DAT for track {}", entry.usb_id))?;
                let ext_path = output_dir.join(anlz::anlz_ext_path_for_track(entry.track));
                anlz::write_anlz_ext(&ext_path, entry.track, entry.analysis)
                    .with_context(|| format!("Failed to write ANLZ .EXT for track {}", entry.usb_id))?;
                // TODO: .2EX disabled pending CDJ-3000 crash investigation.
                // let ex_path = output_dir.join(anlz::anlz_2ex_path_for_track(entry.track));
                // anlz::write_anlz_2ex(&ex_path, entry.track, entry.analysis)
                //     .with_context(|| format!("Failed to write ANLZ .2EX for track {}", entry.usb_id))?;

                // Write artwork
                if let Some(ref art_data) = entry.track.artwork {
                    if let Err(e) = write_artwork(&artwork_dir, entry.usb_id, art_data) {
                        eprintln!("Warning: artwork failed for track {}: {}", entry.usb_id, e);
                    }
                }
            }
            SyncAction::Update => {
                // Skip audio copy — rewrite ANLZ + artwork
                let dat_path = output_dir.join(anlz::anlz_path_for_track(entry.track));
                anlz::write_anlz_dat(&dat_path, entry.track, entry.analysis)
                    .with_context(|| format!("Failed to write ANLZ .DAT for track {}", entry.usb_id))?;
                let ext_path = output_dir.join(anlz::anlz_ext_path_for_track(entry.track));
                anlz::write_anlz_ext(&ext_path, entry.track, entry.analysis)
                    .with_context(|| format!("Failed to write ANLZ .EXT for track {}", entry.usb_id))?;
                // TODO: .2EX disabled pending CDJ-3000 crash investigation.
                // let ex_path = output_dir.join(anlz::anlz_2ex_path_for_track(entry.track));
                // anlz::write_anlz_2ex(&ex_path, entry.track, entry.analysis)
                //     .with_context(|| format!("Failed to write ANLZ .2EX for track {}", entry.usb_id))?;

                if let Some(ref art_data) = entry.track.artwork {
                    if let Err(e) = write_artwork(&artwork_dir, entry.usb_id, art_data) {
                        eprintln!("Warning: artwork failed for track {}: {}", entry.usb_id, e);
                    }
                }
            }
            SyncAction::Skip => {
                // Nothing to do — files are already correct on USB
            }
        }
    }

    // 5. Remove tracks that are no longer in the library
    for removal in &plan.removals {
        if let Err(e) = remove_track_files(output_dir, removal) {
            eprintln!("Warning: cleanup failed for removed track {}: {}", removal.id, e);
        }
    }

    // 6. Build merged track list with stable USB IDs for database rebuild
    let mut merged_tracks: Vec<Track> = Vec::with_capacity(plan.entries.len());
    let mut merged_analyses: Vec<AnalysisResult> = Vec::with_capacity(plan.entries.len());

    for entry in &plan.entries {
        let mut t = entry.track.clone();
        t.id = entry.usb_id;
        merged_tracks.push(t);
        merged_analyses.push(entry.analysis.clone());
    }

    // 7. Rebuild both databases from scratch
    let pdb_path = rekordbox_dir.join("export.pdb");
    pdb::write_pdb(&pdb_path, &merged_tracks, &plan.playlists)?;
    onelibrary::write_onelibrary(output_dir, &merged_tracks, &merged_analyses, &plan.playlists)?;

    // macOS creates ._* AppleDouble resource-fork files on FAT/exFAT volumes.
    // CDJ firmware may try to parse these as ANLZ/PDB files and crash.
    clean_dot_underscore(output_dir);

    Ok(report)
}

/// Delete all files associated with a track being removed from the USB.
///
/// Failures are non-fatal — a missing file is silently ignored, and other
/// errors are returned so the caller can log a warning.
fn remove_track_files(output_dir: &Path, track: &ExistingTrack) -> Result<()> {
    // Delete audio file
    let audio_path = output_dir.join(track.usb_path.trim_start_matches('/'));
    match std::fs::remove_file(&audio_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => eprintln!("Warning: could not remove audio {}: {}", audio_path.display(), e),
    }

    // Try to remove the empty artist directory
    if let Some(parent) = audio_path.parent() {
        let _ = std::fs::remove_dir(parent);
    }

    // Delete ANLZ directory (contains .DAT, .EXT, and any regenerated files)
    let anlz_dir = output_dir.join(anlz::anlz_dir_for_path(&track.usb_path));
    match std::fs::remove_dir_all(&anlz_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => eprintln!("Warning: could not remove ANLZ dir {}: {}", anlz_dir.display(), e),
    }

    // Delete artwork files
    if track.has_artwork {
        let artwork_dir = output_dir.join("PIONEER/Artwork/00001");
        for prefix in ["a", "b"] {
            for suffix in ["", "_m"] {
                let name = format!("{}{}{}.jpg", prefix, track.id, suffix);
                let _ = std::fs::remove_file(artwork_dir.join(&name));
            }
        }
    }

    Ok(())
}

/// Remove macOS `._*` AppleDouble resource-fork files from the PIONEER directory tree.
///
/// macOS automatically creates these when writing to FAT/exFAT volumes.
/// CDJ firmware may attempt to parse them alongside legitimate files and crash.
fn clean_dot_underscore(output_dir: &Path) {
    fn walk(dir: &Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("._") {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
            }
            if path.is_dir() {
                walk(&path);
            }
        }
    }
    walk(&output_dir.join("PIONEER"));
    walk(&output_dir.join("Contents"));
}
