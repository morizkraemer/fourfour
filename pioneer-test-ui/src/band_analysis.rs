use std::fs::File;
use std::path::Path;

use pioneer_usb_writer::waveform::{analyze, measure_window, DetailMeter, DetailSeries, Meter, Waveforms, WindowSeries};
use symphonia::core::audio::SampleBuffer;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Condvar, Mutex};

use symphonia::core::codecs::{CodecParameters, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, Packet, SeekMode, SeekTo};
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
struct OpenedAudio {
    format: Box<dyn FormatReader>,
    track_id: u32,
    params: CodecParameters,
    lead: u64,
}

fn open_audio(path: &Path) -> Result<OpenedAudio, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probe = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        symphonia::default::get_probe().format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
    }));
    let probed = probe
        .map_err(|_| format!("probe {}: the decoder crashed on the file", path.display()))?
        .map_err(|e| format!("probe {}: {e}", path.display()))?;
    let format = probed.format;
    let track = format.default_track().ok_or("no audio track")?;
    let (track_id, params) = (track.id, track.codec_params.clone());
    let lead = fourfour_analysis::mp3::leading_silence_frames(path) as u64;
    Ok(OpenedAudio { format, track_id, params, lead })
}

fn make_decoder(params: &CodecParameters) -> Result<Box<dyn Decoder>, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| symphonia::default::get_codecs().make(params, &DecoderOptions::default())))
        .map_err(|_| "decoder: the codec crashed on the file".to_string())?
        .map_err(|e| format!("decoder: {e}"))
}

fn decode_blocks_from(
    path: &Path,
    from_s: f64,
    mut on_block: impl FnMut(u32, usize, u64, &[f32]) -> bool,
) -> Result<(), String> {
    let OpenedAudio { mut format, track_id, params, lead } = open_audio(path)?;
    let mut decoder = make_decoder(&params)?;

    let rate = params.sample_rate;
    let time_base = params.time_base;
    let lead_s = rate.map_or(0.0, |r| lead as f64 / r as f64);
    let mut frame: u64 = 0;
    let decode_from_s = from_s - lead_s;
    if decode_from_s <= 0.0 {
        if let (Some(rate), Some(channels), true) = (rate, params.channels.map(|c| c.count()), lead > 0) {
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

/// Packets per segment of `decode_blocks_parallel`: enough audio (about 7 s of MP3) that a decoder per segment costs
/// little, few enough that the first segment is ready early.
const SEGMENT_PACKETS: usize = 256;
/// Packets decoded before a segment and thrown away, so its decoder reaches the state a decode from the start has
/// there: the MP3 bit reservoir reaches at most 511 bytes back, overlap-add and synthesis filters one frame.
const PREROLL_PACKETS: usize = 16;
const MAX_DECODE_THREADS: usize = 6;

struct Block {
    rate: u32,
    channels: usize,
    samples: Vec<f32>,
}

/// The audio of one segment: `preroll` is what its decoder made of the packets before it (for state that carries over
/// from earlier audio, such as filters), `blocks` the segment itself, consecutive packets of one format joined.
pub(crate) struct Segment {
    preroll: Vec<Block>,
    blocks: Vec<Block>,
}

/// Decodes `packets` with a fresh decoder; the first `skip` of them are the preroll.
fn decode_segment(params: &CodecParameters, packets: &[Packet], skip: usize) -> Result<Segment, String> {
    let mut decoder = make_decoder(params)?;
    let mut segment = Segment { preroll: Vec::new(), blocks: Vec::new() };
    let mut buffer: Option<SampleBuffer<f32>> = None;
    for (i, packet) in packets.iter().enumerate() {
        let decoded = match decoder.decode(packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) if i < skip => continue,
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
        let blocks = if i < skip { &mut segment.preroll } else { &mut segment.blocks };
        match blocks.last_mut() {
            Some(block) if block.rate == spec.rate && block.channels == channels => {
                block.samples.extend_from_slice(samples.samples())
            }
            _ => blocks.push(Block { rate: spec.rate, channels, samples: samples.samples().to_vec() }),
        }
    }
    Ok(segment)
}

/// How many segments the reader may be ahead of the one handed to `on_block`, so memory stays bounded when
/// `on_block` is slower than the decode.
struct Lookahead {
    consumed: Mutex<usize>,
    advanced: Condvar,
    limit: usize,
}

impl Lookahead {
    fn wait_for_room(&self, index: usize) {
        let mut consumed = self.consumed.lock().unwrap_or_else(|e| e.into_inner());
        while index >= consumed.saturating_add(self.limit) {
            consumed = self.advanced.wait(consumed).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn advance(&self) {
        *self.consumed.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        self.advanced.notify_all();
    }

    fn release(&self) {
        *self.consumed.lock().unwrap_or_else(|e| e.into_inner()) = usize::MAX;
        self.advanced.notify_all();
    }
}

type Job<T> = (Vec<Packet>, usize, StartChain, SyncSender<Result<T, String>>);

/// The track frame a segment starts at, known once the segment before it is decoded: a segment's `claim` waits for its
/// start and hands the start of the next one on. A segment that never claims leaves the ones after it without a start.
pub(crate) struct StartChain {
    start: Receiver<u64>,
    next: SyncSender<u64>,
}

impl StartChain {
    pub(crate) fn claim(self, frames: u64) -> Result<u64, String> {
        let start = self.start.recv().map_err(|_| "an earlier segment failed".to_string())?;
        let _ = self.next.send(start + frames);
        Ok(start)
    }
}

/// The audio `decode_blocks` hands over, bit for bit and in order (cut into blocks differently), decoded on `threads`
/// threads: a reader thread splits the packets into segments of `SEGMENT_PACKETS`, each segment is decoded by a fresh
/// decoder that first decodes the `PREROLL_PACKETS` before it, and `on_block` gets the segments in order on the
/// calling thread while later ones are still decoding.
pub(crate) fn decode_blocks_parallel(
    path: &Path,
    threads: usize,
    mut on_block: impl FnMut(u32, usize, &[f32]),
) -> Result<(), String> {
    decode_segments_parallel(path, threads, |segment, _| segment.blocks, |arrival| match arrival {
        Arrival::Lead { rate, channels, frames } => on_block(rate, channels, &vec![0.0; frames * channels]),
        Arrival::Segment(blocks) => {
            for block in &blocks {
                on_block(block.rate, block.channels, &block.samples);
            }
        }
    })
}

pub(crate) enum Arrival<T> {
    /// The leading silence of the track, before any segment.
    Lead { rate: u32, channels: usize, frames: usize },
    Segment(T),
}

/// The segments of `decode_blocks_parallel`, each turned into `T` by `process` on its decode thread and handed to
/// `on_arrival` in order on the calling thread.
pub(crate) fn decode_segments_parallel<T: Send>(
    path: &Path,
    threads: usize,
    process: impl Fn(Segment, StartChain) -> T + Sync,
    mut on_arrival: impl FnMut(Arrival<T>),
) -> Result<(), String> {
    let OpenedAudio { mut format, track_id, params, lead } = open_audio(path)?;
    let mut first_start = 0;
    if let (Some(rate), Some(channels), true) = (params.sample_rate, params.channels.map(|c| c.count()), lead > 0) {
        on_arrival(Arrival::Lead { rate, channels, frames: lead as usize });
        first_start = lead;
    }
    let threads = threads.max(1);
    let lookahead = Lookahead { consumed: Mutex::new(0), advanced: Condvar::new(), limit: 2 * threads + 2 };
    let (job_tx, job_rx) = mpsc::sync_channel::<Job<T>>(threads);
    let job_rx = Mutex::new(job_rx);
    let (order_tx, order_rx) = mpsc::sync_channel::<Receiver<Result<T, String>>>(lookahead.limit);
    let mut read_error: Option<String> = None;

    let outcome = std::thread::scope(|scope| {
        let (params, lookahead, job_rx, read_error, process) = (&params, &lookahead, &job_rx, &mut read_error, &process);
        scope.spawn(move || {
            let mut index = 0usize;
            let mut tail: Vec<Packet> = Vec::new();
            let (first_tx, mut start_rx) = mpsc::sync_channel(1);
            let _ = first_tx.send(first_start);
            let mut send = |segment: Vec<Packet>, tail: &mut Vec<Packet>| -> bool {
                lookahead.wait_for_room(index);
                let next_tail = segment[segment.len().saturating_sub(PREROLL_PACKETS)..].to_vec();
                let mut packets = std::mem::replace(tail, next_tail);
                let skip = packets.len();
                packets.extend(segment);
                let (result_tx, result_rx) = mpsc::sync_channel(1);
                let (next_tx, next_rx) = mpsc::sync_channel(1);
                let chain = StartChain { start: std::mem::replace(&mut start_rx, next_rx), next: next_tx };
                index += 1;
                order_tx.send(result_rx).is_ok() && job_tx.send((packets, skip, chain, result_tx)).is_ok()
            };
            let mut segment: Vec<Packet> = Vec::with_capacity(SEGMENT_PACKETS);
            loop {
                match format.next_packet() {
                    Ok(packet) if packet.track_id() == track_id => {
                        segment.push(packet);
                        if segment.len() == SEGMENT_PACKETS
                            && !send(std::mem::replace(&mut segment, Vec::with_capacity(SEGMENT_PACKETS)), &mut tail)
                        {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(SymphoniaError::ResetRequired) => break,
                    Err(e) => {
                        *read_error = Some(format!("read packet: {e}"));
                        break;
                    }
                }
            }
            if !segment.is_empty() {
                send(segment, &mut tail);
            }
        });
        for _ in 0..threads {
            scope.spawn(move || {
                loop {
                    let job = job_rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
                    let Ok((packets, skip, chain, result_tx)) = job else { return };
                    let _ = result_tx.send(decode_segment(params, &packets, skip).map(|segment| process(segment, chain)));
                }
            });
        }

        let mut outcome = Ok(());
        for result_rx in order_rx {
            match result_rx.recv() {
                Ok(Ok(done)) => {
                    on_arrival(Arrival::Segment(done));
                    lookahead.advance();
                }
                Ok(Err(e)) => {
                    outcome = Err(e);
                    break;
                }
                Err(_) => {
                    outcome = Err("a decode thread stopped".to_string());
                    break;
                }
            }
        }
        lookahead.release();
        outcome
    });
    outcome?;
    read_error.map_or(Ok(()), Err)
}

/// Threads for one `decode_blocks_parallel`: the cores but one (the caller measures what they decode), at most six.
pub(crate) fn decode_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(1)).clamp(1, MAX_DECODE_THREADS)
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

/// Decodes `path` and measures the app-only high-resolution series for the zoomed waveform, the decode and the band
/// filters on several threads: each segment's filters are warmed on the audio its decoder made of the packets before
/// it, so the series is the one a single meter over the whole track gives (within the rounding of the filter state).
pub fn measure_detail(path: &Path) -> Result<DetailSeries, String> {
    let mut meter: Option<DetailMeter> = None;
    let mut failed: Option<String> = None;
    decode_segments_parallel(
        path,
        decode_threads(),
        |segment, chain| -> Result<Option<DetailMeter>, String> {
            let frames: u64 = segment.blocks.iter().map(|b| (b.samples.len() / b.channels.max(1)) as u64).sum();
            let start = chain.claim(frames)?;
            let Some(first) = segment.blocks.first() else { return Ok(None) };
            let mut part = DetailMeter::starting_at(first.rate, first.channels, start);
            for block in segment.preroll.iter().filter(|b| b.rate == first.rate && b.channels == first.channels) {
                part.warm(&block.samples);
            }
            for block in &segment.blocks {
                part.push(&block.samples);
            }
            Ok(Some(part))
        },
        |arrival| {
            let joined = match arrival {
                Arrival::Lead { rate, channels, frames } => {
                    meter.get_or_insert_with(|| DetailMeter::new(rate, channels)).push(&vec![0.0; frames * channels]);
                    Ok(())
                }
                Arrival::Segment(Ok(Some(part))) => match meter.as_mut() {
                    Some(meter) => meter.join(part),
                    None => {
                        let mut first = DetailMeter::new(part.sample_rate(), part.channels());
                        let joined = first.join(part);
                        meter = Some(first);
                        joined
                    }
                },
                Arrival::Segment(Ok(None)) => Ok(()),
                Arrival::Segment(Err(e)) => Err(e),
            };
            if let Err(e) = joined {
                failed.get_or_insert(e);
            }
        },
    )?;
    if let Some(e) = failed {
        return Err(e);
    }
    meter.map(DetailMeter::finish).ok_or_else(|| "no audio decoded".to_string())
}

/// The 3-band waveforms and the detail series from one decode, for a caller that decodes the track anyway (the
/// analysis measures the waveforms; with this it can fill the detail cache with `waveform_detail::store_detail`).
#[allow(dead_code, reason = "the analysis switches to it to fill the detail cache (Q50 follow-up, analysis_run.rs)")]
pub fn measure_waveforms_and_detail(path: &Path) -> Result<(Waveforms, DetailSeries), String> {
    let mut meters: Option<(Meter, DetailMeter)> = None;
    decode_blocks_parallel(path, decode_threads(), |rate, channels, samples| {
        let (meter, detail) =
            meters.get_or_insert_with(|| (Meter::new(rate, channels), DetailMeter::new(rate, channels)));
        meter.push(samples);
        detail.push(samples);
    })?;
    let (meter, detail) = meters.ok_or("no audio decoded")?;
    Ok((analyze(&meter.finish()), detail.finish()))
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
    fn the_parallel_decode_hands_over_the_same_audio_and_detail_as_one_thread() {
        let path = std::env::temp_dir().join(format!("ff-parallel-decode-{}.wav", std::process::id()));
        write_wav(&path, 44_100, 40.0, 97.0);
        let mut serial = Vec::new();
        let mut meter: Option<DetailMeter> = None;
        decode_blocks(&path, |rate, channels, samples| {
            serial.extend_from_slice(samples);
            meter.get_or_insert_with(|| DetailMeter::new(rate, channels)).push(samples);
        })
        .unwrap();
        let mut blocks = 0;
        let mut parallel = Vec::new();
        decode_blocks_parallel(&path, 3, |_, _, samples| {
            blocks += 1;
            parallel.extend_from_slice(samples);
        })
        .unwrap();
        let detail = measure_detail(&path).unwrap();
        let (waves, both_detail) = measure_waveforms_and_detail(&path).unwrap();
        let waves_alone = measure_waveforms(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert!(blocks > 2, "40 s of audio is several segments, got {blocks}");
        assert_eq!(parallel.len(), serial.len());
        assert!(parallel.iter().zip(&serial).all(|(a, b)| a.to_bits() == b.to_bits()));
        assert_eq!(detail, meter.unwrap().finish());
        assert_eq!(both_detail, detail);
        assert_eq!(waves.pwv7, waves_alone.pwv7);
        assert_eq!(waves.pwv6, waves_alone.pwv6);
    }

    #[test]
    fn a_parallel_decode_of_a_missing_file_is_an_error() {
        assert!(decode_blocks_parallel(Path::new("/nonexistent/track.mp3"), 2, |_, _, _| {}).is_err());
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        assert!(measure_waveforms(Path::new("/nonexistent/track.mp3")).is_err());
    }
}
