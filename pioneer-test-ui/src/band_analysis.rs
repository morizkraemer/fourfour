use std::fs::File;
use std::path::Path;

use pioneer_usb_writer::models::AnalysisResult;
use pioneer_usb_writer::waveform::{analyze, DetailMeter, DetailSeries, Meter, Waveforms};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

fn decode_blocks(path: &Path, mut on_block: impl FnMut(u32, usize, &[f32])) -> Result<(), String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("probe {}: {e}", path.display()))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("no audio track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("decoder: {e}"))?;

    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(format!("read packet: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode: {e}")),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count();
        let needed = decoded.capacity() * channels;
        if buffer.as_ref().is_none_or(|b| b.capacity() < needed) {
            buffer = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
        }
        let samples = buffer.as_mut().expect("buffer was just created");
        samples.copy_interleaved_ref(decoded);
        on_block(spec.rate, channels, samples.samples());
    }
    Ok(())
}

/// Decodes `path` and measures the 3-band waveforms with the shared `waveform` module,
/// the same pass whose bytes the USB export writes.
pub fn measure_waveforms(path: &Path) -> Result<Waveforms, String> {
    let mut meter: Option<Meter> = None;
    decode_blocks(path, |rate, channels, samples| {
        meter.get_or_insert_with(|| Meter::new(rate, channels)).push(samples);
    })?;
    let meter = meter.ok_or("no audio decoded")?;
    Ok(analyze(&meter.finish()))
}

/// Decodes `path` and measures the app-only high-resolution series for the zoomed waveform.
pub fn measure_detail(path: &Path) -> Result<DetailSeries, String> {
    let mut meter: Option<DetailMeter> = None;
    decode_blocks(path, |rate, channels, samples| {
        meter.get_or_insert_with(|| DetailMeter::new(rate, channels)).push(samples);
    })?;
    meter.map(DetailMeter::finish).ok_or_else(|| "no audio decoded".to_string())
}

/// Replaces the colour data of `analysis` with bands measured from the audio at `path`,
/// so it no longer depends on the Python filter bank. The mono preview keeps its source.
pub fn apply_measured_bands(analysis: &mut AnalysisResult, path: &Path) -> Result<(), String> {
    let waves = measure_waveforms(path)?;
    analysis.color_waveform = Some(waves.color_waveform());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &Path, rate: u32, seconds: f64, hz: f64) {
        let frames = (rate as f64 * seconds) as usize;
        let mut data = Vec::with_capacity(frames * 4);
        for i in 0..frames {
            let v = (0.5 * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin() * 32767.0) as i16;
            data.extend_from_slice(&v.to_le_bytes());
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        std::fs::write(path, wav).unwrap();
    }

    #[test]
    fn measures_a_wav_into_150_columns_per_second() {
        let path = std::env::temp_dir().join(format!("ff-band-analysis-{}.wav", std::process::id()));
        write_wav(&path, 44_100, 2.0, 80.0);
        let waves = measure_waveforms(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(waves.pwv7.len(), 300);
        assert_eq!(waves.pwv6.len(), 1200);
        let steady = waves.pwv7[100];
        assert!(steady[0] > 40 && steady[0] > steady[2], "an 80 Hz tone is low: {steady:?}");
    }

    #[test]
    fn measures_a_wav_into_2400_detail_columns_per_second() {
        let path = std::env::temp_dir().join(format!("ff-detail-analysis-{}.wav", std::process::id()));
        write_wav(&path, 44_100, 2.0, 80.0);
        let detail = measure_detail(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(detail.len(), 4800);
        let steady = [detail.planes[0][2400], detail.planes[1][2400], detail.planes[2][2400]];
        assert!(steady[0] > 40 && steady[0] > steady[2], "an 80 Hz tone is low: {steady:?}");
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        assert!(measure_waveforms(Path::new("/nonexistent/track.mp3")).is_err());
    }
}
