use std::fs::File;
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use beat_this::__probe::StreamResampler;
use symphonia::core::audio::{Channels, Position};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::loudness::{channel_weight, default_channel_weights, Loudness, Meter};

pub struct Mono {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub truncated: bool,
    pub gapless: Range<usize>,
    pub loudness: Option<Loudness>,
    pub loudness_ms: f64,
}

impl Mono {
    pub fn gapless_samples(&self) -> &[f32] {
        &self.samples[self.gapless.clone()]
    }
}

const MIN_DECODED_SECONDS: usize = 1;

fn channel_weights(channels: &Channels) -> Vec<f64> {
    match channels {
        Channels::Positioned(positions) => {
            let weights: Vec<f64> = (0..32)
                .map(|bit| Position::from_bits_truncate(1 << bit))
                .filter(|p| positions.contains(*p) && !p.is_empty())
                .map(|p| {
                    let lfe = p.intersects(Position::LFE1 | Position::LFE2);
                    let surround = p.intersects(Position::REAR_LEFT | Position::REAR_RIGHT | Position::REAR_CENTER | Position::SIDE_LEFT | Position::SIDE_RIGHT);
                    channel_weight(lfe, surround)
                })
                .collect();
            if weights.len() == channels.count() {
                weights
            } else {
                default_channel_weights(channels.count())
            }
        }
        other => default_channel_weights(other.count()),
    }
}

pub fn decode_mono(path: &Path) -> Result<Mono> {
    decode_mono_measured(path, false)
}

pub fn decode_mono_measured(path: &Path, measure_loudness: bool) -> Result<Mono> {
    let mss = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())?;
    let track = format.first_track_known_codec(TrackType::Audio).ok_or_else(|| anyhow!("no supported audio tracks"))?;
    let track_id = track.id;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or_else(|| anyhow!("audio track missing codec parameters"))?;
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(params, &AudioDecoderOptions::default().gapless(false))?;

    let lead = crate::mp3::leading_silence_frames(path) as usize;
    let (delay, padding) = (track.delay.unwrap_or(0) as usize, track.padding.unwrap_or(0) as usize);
    let mut samples: Vec<f32> = vec![0.0; lead];
    let mut scratch: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    let mut truncated = false;
    let mut meter: Option<Meter> = None;
    let mut loudness_ms = 0.0;
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                truncated = true;
                break;
            }
            Err(Error::DecodeError(_)) => {
                truncated = true;
                break;
            }
            Err(err) => return Err(anyhow!(err)),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = decoded.spec();
                rate = spec.rate();
                let channels = spec.channels().count();
                decoded.copy_to_vec_interleaved(&mut scratch);
                if measure_loudness {
                    let t = Instant::now();
                    meter.get_or_insert_with(|| Meter::new(rate, channel_weights(spec.channels()))).push_interleaved(&scratch);
                    loudness_ms += t.elapsed().as_secs_f64() * 1000.0;
                }
                if channels == 1 {
                    samples.extend_from_slice(&scratch);
                } else {
                    samples.extend(scratch.chunks(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32));
                }
            }
            Err(Error::DecodeError(_)) => truncated = true,
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                truncated = true;
                break;
            }
            Err(err) => return Err(anyhow!(err)),
        }
    }
    if rate == 0 {
        return Err(anyhow!("failed to decode any audio packets"));
    }
    if truncated {
        if samples.len() < rate as usize * MIN_DECODED_SECONDS {
            return Err(anyhow!("damaged audio: only {} samples decoded", samples.len()));
        }
        eprintln!("warning: {} is truncated or damaged; analysing the {:.1} s that decoded", path.display(), samples.len() as f64 / rate as f64);
    }
    let start = (lead + delay).min(samples.len());
    let end = if truncated { samples.len() } else { samples.len().saturating_sub(padding).max(start) };
    let t = Instant::now();
    let loudness = meter.and_then(Meter::finish);
    loudness_ms += t.elapsed().as_secs_f64() * 1000.0;
    Ok(Mono { samples, rate, truncated, gapless: start..end, loudness, loudness_ms })
}

pub fn resample_to(samples: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    if from == to {
        return Ok(samples.to_vec());
    }
    let mut resampler = StreamResampler::new(from, to)?;
    let mut out = Vec::with_capacity((samples.len() as f64 * to as f64 / from as f64) as usize + 1024);
    resampler.push(samples, &mut out)?;
    resampler.finish(&mut out)?;
    Ok(out)
}
