use pioneer_usb_writer::{analyzer, models, scanner, writer};

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pioneer-usb-writer")]
#[command(about = "Write audio files to a Pioneer CDJ-compatible USB stick")]
struct Cli {
    /// Directories containing audio files to export (each becomes a playlist)
    #[arg(required = true)]
    input_dirs: Vec<PathBuf>,

    /// Output directory (USB stick mount point or test directory)
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Step 1: Scan all input directories
    let mut all_tracks = Vec::new();
    let mut playlists = Vec::new();
    let mut global_id = 1u32;

    for input_dir in &cli.input_dirs {
        println!("Scanning {}...", input_dir.display());
        let mut tracks = scanner::scan_directory(input_dir)
            .with_context(|| format!("Failed to scan {}", input_dir.display()))?;

        if tracks.is_empty() {
            println!("  No audio files found, skipping.");
            continue;
        }

        // Assign globally unique IDs
        let mut track_ids = Vec::new();
        for track in &mut tracks {
            track.id = global_id;
            track_ids.push(global_id);
            global_id += 1;
        }

        // Playlist name = directory name
        let playlist_name = input_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();

        println!("  Found {} tracks → playlist \"{}\"", tracks.len(), playlist_name);

        playlists.push(models::Playlist {
            id: playlists.len() as u32 + 1,
            name: playlist_name,
            track_ids,
        });

        all_tracks.extend(tracks);
    }

    if all_tracks.is_empty() {
        println!("No audio files found.");
        return Ok(());
    }

    println!("\nTotal: {} tracks in {} playlists", all_tracks.len(), playlists.len());

    // Step 2: Analyze each track
    let pb = ProgressBar::new(all_tracks.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut analyses = Vec::with_capacity(all_tracks.len());
    let mut failed = 0u32;

    for track in &mut all_tracks {
        pb.set_message(track.title.clone());

        match analyzer::analyze_track(&track.source_path) {
            Ok(analysis) => {
                track.tempo = (analysis.bpm * 100.0) as u32;
                track.key = analysis.key.clone();
                analyses.push(analysis);
            }
            Err(e) => {
                eprintln!("\nWarning: analysis failed for {}: {}", track.title, e);
                analyses.push(models::AnalysisResult {
                    beat_grid: models::BeatGrid { beats: Vec::new() },
                    waveform: models::WaveformPreview { data: [0u8; 400] },
                    bpm: 0.0,
                    key: String::new(),
                    cue_points: Vec::new(),
                });
                failed += 1;
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Analysis complete");

    // Step 3: Write USB structure
    println!("\nWriting to {}...", cli.output.display());
    writer::filesystem::write_usb(&cli.output, &all_tracks, &analyses, &playlists)?;

    // Summary
    println!("\nDone!");
    println!("  Tracks exported: {}", all_tracks.len() - failed as usize);
    if failed > 0 {
        println!("  Failed: {}", failed);
    }
    for playlist in &playlists {
        println!("  Playlist \"{}\": {} tracks", playlist.name, playlist.track_ids.len());
    }
    println!("\n  Tracks with BPM:");
    for track in &all_tracks {
        if track.tempo > 0 {
            println!(
                "    {} - {} | {:.1} BPM | {}",
                track.artist,
                track.title,
                track.tempo as f64 / 100.0,
                track.key
            );
        }
    }

    Ok(())
}
