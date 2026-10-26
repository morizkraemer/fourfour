use std::fs::File;
use std::path::Path;

use anyhow::{anyhow, Result};
use beat_this::__probe::StreamResampler;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

pub struct Mono {
    pub samples: Vec<f32>,
    pub rate: u32,
}

pub fn decode_mono(path: &Path) -> Result<Mono> {
    let mss = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe().probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())?;
    let track = format.first_track_known_codec(TrackType::Audio).ok_or_else(|| anyhow!("no supported audio tracks"))?;
    let track_id = track.id;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or_else(|| anyhow!("audio track missing codec parameters"))?;
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(params, &AudioDecoderOptions::default())?;

    let mut samples: Vec<f32> = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    while let Some(packet) = format.next_packet()? {
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = decoded.spec();
                rate = spec.rate();
                let channels = spec.channels().count();
                decoded.copy_to_vec_interleaved(&mut scratch);
                if channels == 1 {
                    samples.extend_from_slice(&scratch);
                } else {
                    samples.extend(scratch.chunks(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32));
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(err) => return Err(anyhow!(err)),
        }
    }
    if rate == 0 {
        return Err(anyhow!("failed to decode any audio packets"));
    }
    Ok(Mono { samples, rate })
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
