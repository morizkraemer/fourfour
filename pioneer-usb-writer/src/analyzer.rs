use anyhow::{Context, Result};
use std::path::Path;
use stratum_dsp::AnalysisConfig;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::models;
use crate::waveform;

/// Decode audio file to mono f32 samples, then run BPM/key/waveform analysis.
pub fn analyze_track(path: &Path) -> Result<models::AnalysisResult> {
    let (samples, sample_rate) = decode_to_mono_f32(path)?;

    // BPM, key, and beat grid detection via stratum-dsp
    let dsp_result = stratum_dsp::analyze_audio(&samples, sample_rate, AnalysisConfig::default())
        .map_err(|e| anyhow::anyhow!("Analysis failed: {:?}", e))?;

    let bpm = dsp_result.bpm as f64;
    let key = dsp_result.key.numerical(); // DJ notation like "1A", "5B"

    // Convert stratum-dsp beat grid to our format
    let beat_grid = convert_beat_grid(&dsp_result.beat_grid, bpm);

    // Generate waveform preview
    let waveform_preview = waveform::generate_preview(&samples);

    Ok(models::AnalysisResult {
        beat_grid,
        waveform: waveform_preview,
        bpm,
        key,
        cue_points: vec![],
    })
}

/// Convert stratum-dsp BeatGrid to our Pioneer-format BeatGrid.
fn convert_beat_grid(
    dsp_grid: &stratum_dsp::BeatGrid,
    bpm: f64,
) -> models::BeatGrid {
    let tempo = (bpm * 100.0) as u32;
    let mut beats = Vec::new();

    // Use the individual beat positions from stratum-dsp
    // Assign bar positions 1-4 cyclically, using downbeats to reset to 1
    let downbeat_set: std::collections::HashSet<u32> = dsp_grid
        .downbeats
        .iter()
        .map(|&t| (t * 1000.0) as u32)
        .collect();

    let mut bar_pos = 1u8;
    for &beat_time in &dsp_grid.beats {
        let time_ms = (beat_time * 1000.0) as u32;

        // If this beat is a downbeat, reset to bar position 1
        if downbeat_set.contains(&time_ms) {
            bar_pos = 1;
        }

        beats.push(models::Beat {
            bar_position: bar_pos,
            time_ms,
            tempo,
        });

        bar_pos = if bar_pos == 4 { 1 } else { bar_pos + 1 };
    }

    models::BeatGrid { beats }
}

/// Decode an audio file to mono f32 PCM samples. Returns (samples, sample_rate).
fn decode_to_mono_f32(path: &Path) -> Result<(Vec<f32>, u32)> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .context("Failed to probe audio format")?;

    let mut format = probed.format;

    let track = format
        .default_track()
        .context("No audio tracks found")?;
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        };

        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let interleaved = sample_buf.samples();

        // Mix down to mono
        for frame in interleaved.chunks(channels) {
            let mono: f32 = frame.iter().sum::<f32>() / channels as f32;
            all_samples.push(mono);
        }
    }

    Ok((all_samples, sample_rate))
}
