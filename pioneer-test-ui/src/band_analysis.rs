use std::fs::File;
use std::path::Path;

use pioneer_usb_writer::models::AnalysisResult;
use pioneer_usb_writer::waveform::{analyze, measure_window, DetailMeter, DetailSeries, Meter, Waveforms, WindowSeries};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

pub(crate) fn decode_blocks(path: &Path, mut on_block: impl FnMut(u32, usize, &[f32])) -> Result<(), String> {
    decode_blocks_from(path, 0.0, |rate, channels, _, samples| {
        on_block(rate, channels, samples);
        true
    })
}

/// Decodes `path` from `from_s` on (an accurate seek, as the player does) and hands every block over with the track
/// frame its first sample is; stops when `on_block` returns false.
fn decode_blocks_from(
    path: &Path,
    from_s: f64,
    mut on_block: impl FnMut(u32, usize, u64, &[f32]) -> bool,
) -> Result<(), String> {
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

    let rate = track.codec_params.sample_rate;
    let time_base = track.codec_params.time_base;
    let lead = fourfour_analysis::mp3::leading_silence_frames(path) as u64;
    let lead_s = rate.map_or(0.0, |r| lead as f64 / r as f64);
    let mut frame: u64 = 0;
    let decode_from_s = from_s - lead_s;
    if decode_from_s <= 0.0 {
        if let (Some(rate), Some(channels), true) = (rate, track.codec_params.channels.map(|c| c.count()), lead > 0) {
            if !on_block(rate, channels, 0, &vec![0.0; lead as usize * channels]) {
                return Ok(());
            }
            frame = lead;
        }
    } else {
        let seeked = format
            .seek(SeekMode::Accurate, SeekTo::Time { time: Time::from(decode_from_s), track_id: Some(track_id) })
            .map_err(|e| format!("seek {}: {e}", path.display()))?;
        decoder.reset();
        frame = match (time_base, rate) {
            (Some(tb), Some(rate)) => {
                let t = tb.calc_time(seeked.actual_ts);
                ((t.seconds as f64 + t.frac) * rate as f64).round() as u64
            }
            _ => seeked.actual_ts,
        } + lead;
    }

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
        let block = samples.samples();
        if !on_block(spec.rate, channels, frame, block) {
            break;
        }
        frame += (block.len() / channels.max(1)) as u64;
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

/// Audio before a window that warms the band filters and envelopes up, and covers a decoder that restarts after a seek.
const WINDOW_PREROLL_S: f64 = 0.25;

/// Decodes only `[start_s, end_s)` of `path` (plus a short pre-roll) and measures it into `columns` columns with the
/// track's detail `gains`. `is_current` is asked between blocks; once it says no, the decode stops and `None` is returned.
pub fn measure_detail_window(
    path: &Path,
    gains: [u16; 3],
    start_s: f64,
    end_s: f64,
    columns: usize,
    is_current: &dyn Fn() -> bool,
) -> Result<Option<WindowSeries>, String> {
    let from_s = (start_s - WINDOW_PREROLL_S).max(0.0);
    let mut mono: Vec<f32> = Vec::new();
    let mut first_frame: Option<u64> = None;
    let mut sample_rate = 0u32;
    let mut superseded = false;
    decode_blocks_from(path, from_s, |rate, channels, frame, samples| {
        if !is_current() {
            superseded = true;
            return false;
        }
        sample_rate = rate;
        let channels = channels.max(1);
        let wanted_from = (from_s * rate as f64).floor() as u64;
        let mut at = frame;
        for f in samples.chunks_exact(channels) {
            if at >= wanted_from {
                first_frame.get_or_insert(at);
                mono.push(f.iter().sum::<f32>() / channels as f32);
            }
            at += 1;
        }
        (at as f64) < end_s * rate as f64 + 2.0
    })?;
    if superseded {
        return Ok(None);
    }
    if sample_rate == 0 {
        return Err("no audio decoded".to_string());
    }
    let first_frame = first_frame.unwrap_or(0);
    Ok(Some(measure_window(&mono, first_frame, sample_rate, gains, start_s, end_s, columns)))
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
    fn a_window_decoded_after_a_seek_lines_up_with_the_full_decode() {
        let path = std::env::temp_dir().join(format!("ff-window-analysis-{}.wav", std::process::id()));
        write_wav(&path, 44_100, 3.0, 80.0);
        let detail = measure_detail(&path).unwrap();
        let window = measure_detail_window(&path, detail.gains, 2.0, 2.01, 441, &|| true).unwrap().unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(window.len(), 441);
        assert_eq!(window.sample_rate, 44_100);
        for (c, (&lo, &hi)) in window.lowest.iter().zip(&window.highest).enumerate() {
            let t = 2.0 + c as f64 / 44_100.0;
            let v = 0.5 * (2.0 * std::f64::consts::PI * 80.0 * t).sin() * i16::MAX as f64;
            assert!((lo as f64 - v).abs() < 150.0 && (hi as f64 - v).abs() < 150.0, "column {c}: {lo}..{hi} vs {v}");
        }
        let steady = *detail.planes[0][4800..4824].iter().max().unwrap() as i32;
        let max_low = *window.planes[0].iter().max().unwrap() as i32;
        assert!((max_low - steady).abs() <= 4, "window low {max_low} vs detail {steady}");
    }

    #[test]
    fn a_superseded_window_stops_without_a_result() {
        let path = std::env::temp_dir().join(format!("ff-window-cancel-{}.wav", std::process::id()));
        write_wav(&path, 44_100, 1.0, 80.0);
        let result = measure_detail_window(&path, [1, 1, 1], 0.5, 0.6, 100, &|| false).unwrap();
        std::fs::remove_file(&path).ok();
        assert!(result.is_none());
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        assert!(measure_waveforms(Path::new("/nonexistent/track.mp3")).is_err());
    }
}
