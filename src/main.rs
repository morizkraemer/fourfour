mod analyzer;
mod models;
mod scanner;
mod waveform;
mod writer;

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pioneer-usb-writer")]
#[command(about = "Write audio files to a Pioneer CDJ-compatible USB stick")]
struct Cli {
    /// Directory containing audio files to export
    input_dir: PathBuf,

    /// Output directory (USB stick mount point or test directory)
    output_dir: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Step 1: Scan for audio files
    println!("Scanning {}...", cli.input_dir.display());
    let mut tracks = scanner::scan_directory(&cli.input_dir)
        .context("Failed to scan input directory")?;

    if tracks.is_empty() {
        println!("No audio files found.");
        return Ok(());
    }

    println!("Found {} tracks", tracks.len());

    // Step 2: Analyze each track
    let pb = ProgressBar::new(tracks.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut analyses = Vec::with_capacity(tracks.len());
    let mut failed = 0u32;

    for track in &mut tracks {
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
                });
                failed += 1;
            }
        }

        pb.inc(1);
    }

    pb.finish_with_message("Analysis complete");

    // Step 3: Write USB structure
    println!("\nWriting to {}...", cli.output_dir.display());
    writer::filesystem::write_usb(&cli.output_dir, &tracks, &analyses)?;

    // Summary
    println!("\nDone!");
    println!("  Tracks exported: {}", tracks.len() - failed as usize);
    if failed > 0 {
        println!("  Failed: {}", failed);
    }
    println!("\n  Tracks with BPM:");
    for track in &tracks {
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
