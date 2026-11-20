//! App-only high-resolution 3-band series for the zoomed waveform. Never written to the USB export.
//!
//! The same filters, gains and byte scale as `PWV7`, at [`DETAIL_COLUMNS_PER_SECOND`] columns per second with a
//! release of a few milliseconds instead of the `PWV7` release of about 190 ms, so a kick or a hat keeps its
//! sharp attack and its decay at any zoom. Column `j` covers samples `[round(j * rate / 2400), round((j + 1) * rate / 2400))`;
//! 16 columns make exactly one `PWV7` column, so the per-band maximum over 16 columns equals the raw `PWV7` band peak.
//!
//! Each column also holds the lowest and highest mono sample on the low band's byte scale (`signal_gain` bytes per
//! full scale), so the zoom view can draw the shape of the audio itself once a pixel is short enough to resolve it.
//!
//! [`measure_window`] measures any short stretch of the track at any resolution with the same filters, release and the
//! gains stored in the series, for zoom levels where even 2400 columns per second are wider than a pixel.

use super::{band_gain, larger, Bank, Biquad, Pair, Tuning, COLUMNS_PER_SECOND, PWV7_MAX};

pub const DETAIL_COLUMNS_PER_SECOND: f64 = 2400.0;
const COLUMNS_PER_PWV7_COLUMN: usize = (DETAIL_COLUMNS_PER_SECOND / COLUMNS_PER_SECOND) as usize;
const RELEASE_MS: [f64; 3] = [10.0, 5.0, 2.5];
const NO_EXTREMES: [f64; 2] = [f64::INFINITY, f64::NEG_INFINITY];
const MAGIC: &[u8; 4] = b"FFWD";
const FORMAT_VERSION: u8 = 3;
const HEADER_LEN: usize = 28;

/// Low, mid and high bytes (0..=127, the `PWV7` scale), one plane per band, `columns_per_second` columns per second.
/// `gains` are the track's `PWV7` band gains the bytes were scaled with; [`measure_window`] needs them to match.
/// `signal` holds the lowest and highest mono sample of each column times `signal_gain` (the low band's bytes per full
/// scale), clamped to -127..=127.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DetailSeries {
    pub columns_per_second: u32,
    pub gains: [u16; 3],
    pub signal_gain: f32,
    pub planes: [Vec<u8>; 3],
    pub signal: [Vec<i8>; 2],
}

impl DetailSeries {
    pub fn len(&self) -> usize {
        self.planes[0].len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `FFWD`, version, `columns_per_second` (u32 LE), column count (u32 LE), the three gains (u16 LE), two zero
    /// bytes, `signal_gain` (f32 LE), then the low, mid and high planes and the lowest and highest signal planes (i8).
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.len();
        let mut out = Vec::with_capacity(HEADER_LEN + 5 * n);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&[FORMAT_VERSION, 0, 0, 0]);
        out.extend_from_slice(&self.columns_per_second.to_le_bytes());
        out.extend_from_slice(&(n as u32).to_le_bytes());
        for gain in self.gains {
            out.extend_from_slice(&gain.to_le_bytes());
        }
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&self.signal_gain.to_le_bytes());
        for plane in &self.planes {
            out.extend_from_slice(plane);
        }
        for plane in &self.signal {
            out.extend(plane.iter().map(|&v| v as u8));
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < HEADER_LEN || &bytes[..4] != MAGIC || bytes[4] != FORMAT_VERSION {
            return Err("not a detail waveform file".to_string());
        }
        let columns_per_second = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
        let n = u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes")) as usize;
        if bytes.len() != HEADER_LEN + 5 * n {
            return Err(format!("detail waveform file is {} bytes, expected {}", bytes.len(), HEADER_LEN + 5 * n));
        }
        let gain = |k: usize| u16::from_le_bytes(bytes[16 + 2 * k..18 + 2 * k].try_into().expect("2 bytes"));
        let signal_gain = f32::from_le_bytes(bytes[24..28].try_into().expect("4 bytes"));
        let plane = |k: usize| &bytes[HEADER_LEN + k * n..HEADER_LEN + (k + 1) * n];
        let signed = |k: usize| plane(k).iter().map(|&v| v as i8).collect();
        Ok(DetailSeries {
            columns_per_second,
            gains: [gain(0), gain(1), gain(2)],
            signal_gain,
            planes: [plane(0).to_vec(), plane(1).to_vec(), plane(2).to_vec()],
            signal: [signed(3), signed(4)],
        })
    }
}

/// Measures interleaved audio into raw band peaks per 1/2400 s as it arrives.
pub struct DetailMeter {
    rate: f64,
    channels: usize,
    tuning: Tuning,
    frames: u64,
    next_edge: u64,
    peaks: Vec<[f32; 3]>,
    current: [f64; 3],
    extremes: Vec<[f32; 2]>,
    current_extremes: [f64; 2],
    /// Track frame and column the meter starts at: 0 for a meter from the start of the track.
    start: u64,
    first_column: usize,
    /// Low, mid high-pass, mid low-pass, high.
    filters: [Biquad; 4],
}

impl DetailMeter {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        Self::with_tuning(sample_rate, channels, Tuning::default())
    }

    pub fn with_tuning(sample_rate: u32, channels: usize, tuning: Tuning) -> Self {
        let rate = sample_rate.max(1) as f64;
        DetailMeter {
            rate,
            channels,
            tuning,
            frames: 0,
            next_edge: (rate / DETAIL_COLUMNS_PER_SECOND).round() as u64,
            peaks: Vec::new(),
            current: [0.0; 3],
            extremes: Vec::new(),
            current_extremes: NO_EXTREMES,
            start: 0,
            first_column: 0,
            filters: band_filters(&tuning, rate),
        }
    }

    /// A meter for the audio from track frame `frame` on, so stretches of a track can be measured on separate threads
    /// and put together with [`DetailMeter::join`]. Its filters start empty: [`DetailMeter::warm`] them on audio before
    /// `frame` (a few thousand frames bring the short band filters to the state a meter from the start has there).
    pub fn starting_at(sample_rate: u32, channels: usize, frame: u64) -> Self {
        let mut meter = Self::new(sample_rate, channels);
        let edge = |column: usize| (column as f64 * meter.rate / DETAIL_COLUMNS_PER_SECOND).round() as u64;
        let mut column = (frame as f64 * DETAIL_COLUMNS_PER_SECOND / meter.rate).floor() as usize;
        while column > 0 && edge(column) > frame {
            column -= 1;
        }
        while edge(column + 1) <= frame {
            column += 1;
        }
        meter.first_column = column;
        meter.start = frame;
        meter.frames = frame;
        meter.next_edge = edge(column + 1);
        meter
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate as u32
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Runs the band filters over audio that comes before this meter's first frame, without measuring it.
    pub fn warm(&mut self, interleaved: &[f32]) {
        let channels = self.channels.max(1);
        let [mut low, mut mid_hp, mut mid_lp, mut high] = self.filters;
        for frame in interleaved.chunks_exact(channels) {
            let x = frame.iter().map(|&s| s as f64).sum::<f64>() / channels as f64;
            low.step(x);
            mid_lp.step(mid_hp.step(x));
            high.step(x);
        }
        self.filters = [low, mid_hp, mid_lp, high];
    }

    /// Appends the columns of `later`, a [`DetailMeter::starting_at`] the frame this meter has reached; a column the
    /// two share takes the peaks of both.
    pub fn join(&mut self, later: DetailMeter) -> Result<(), String> {
        if later.start != self.frames || later.rate != self.rate {
            return Err(format!(
                "a stretch from frame {} at {} Hz does not continue a meter at frame {} at {} Hz",
                later.start, later.rate, self.frames, self.rate
            ));
        }
        while self.first_column + self.peaks.len() < later.first_column {
            self.close_current();
        }
        let closed = later.peaks.iter().zip(&later.extremes).map(|(p, e)| {
            ([p[0] as f64, p[1] as f64, p[2] as f64], [e[0] as f64, e[1] as f64])
        });
        let mut columns = closed.chain(std::iter::once((later.current, later.current_extremes)));
        if let Some((c, e)) = columns.next() {
            for band in 0..3 {
                self.current[band] = self.current[band].max(c[band]);
            }
            self.current_extremes = [self.current_extremes[0].min(e[0]), self.current_extremes[1].max(e[1])];
        }
        for (c, e) in columns {
            self.close_current();
            self.current = c;
            self.current_extremes = e;
        }
        self.frames = later.frames;
        self.next_edge = later.next_edge;
        Ok(())
    }

    fn close_current(&mut self) {
        let (c, e) = (self.current, self.current_extremes);
        self.peaks.push([c[0] as f32, c[1] as f32, c[2] as f32]);
        self.extremes.push([e[0] as f32, e[1] as f32]);
        self.current = [0.0; 3];
        self.current_extremes = NO_EXTREMES;
    }

    pub fn push(&mut self, interleaved: &[f32]) {
        let channels = self.channels.max(1);
        let total = interleaved.len() / channels;
        let [low, mid_hp, mid_lp, high] = self.filters;
        let mut bank = Bank::new([low, high, mid_hp, mid_hp]);
        let mut mid_lp = mid_lp;
        let (mut frames, mut next_edge) = (self.frames, self.next_edge);
        let mut c = self.current;
        let mut e = self.current_extremes;
        let mut done = 0;
        while done < total {
            while frames >= next_edge {
                self.peaks.push([c[0] as f32, c[1] as f32, c[2] as f32]);
                self.extremes.push([e[0] as f32, e[1] as f32]);
                c = [0.0; 3];
                e = NO_EXTREMES;
                next_edge = ((self.first_column + self.peaks.len() + 1) as f64 * self.rate / DETAIL_COLUMNS_PER_SECOND)
                    .round() as u64;
            }
            let run = ((next_edge - frames) as usize).min(total - done);
            let block = &interleaved[done * channels..(done + run) * channels];
            match channels {
                1 => measure_detail(&mut bank, &mut mid_lp, &mut c, &mut e, block.iter().map(|&s| s as f64)),
                2 => measure_detail(
                    &mut bank,
                    &mut mid_lp,
                    &mut c,
                    &mut e,
                    block.chunks_exact(2).map(|f| (f[0] as f64 + f[1] as f64) / 2.0),
                ),
                _ => measure_detail(
                    &mut bank,
                    &mut mid_lp,
                    &mut c,
                    &mut e,
                    block.chunks_exact(channels).map(|f| f.iter().map(|&s| s as f64).sum::<f64>() / channels as f64),
                ),
            }
            done += run;
            frames += run as u64;
        }
        self.current = c;
        self.current_extremes = e;
        let [low, high, mid_hp, _] = bank.into_biquads();
        self.filters = [low, mid_hp, mid_lp, high];
        (self.frames, self.next_edge) = (frames, next_edge);
    }

    /// `ceil(frames * 2400 / rate)` raw band peaks, before any release or gain.
    pub fn raw_peaks(mut self) -> Vec<[f32; 3]> {
        self.close_columns();
        self.peaks
    }

    fn close_columns(&mut self) {
        let n = ((self.frames as f64 * DETAIL_COLUMNS_PER_SECOND / self.rate).ceil() as usize).max(1);
        let (c, e) = (self.current, self.current_extremes);
        self.peaks.push([c[0] as f32, c[1] as f32, c[2] as f32]);
        self.extremes.push([e[0] as f32, e[1] as f32]);
        self.peaks.resize(n, [0.0; 3]);
        self.extremes.resize(n, [0.0; 2]);
    }

    pub fn finish(mut self) -> DetailSeries {
        self.close_columns();
        let mut series = shape(&self.peaks, &self.tuning);
        let gain = series.signal_gain as f64;
        let limit = PWV7_MAX as f64;
        let to_signal = |v: f32| if v.is_finite() { (v as f64 * gain).round().clamp(-limit, limit) as i8 } else { 0 };
        series.signal = [
            self.extremes.iter().map(|e| to_signal(e[0])).collect(),
            self.extremes.iter().map(|e| to_signal(e[1])).collect(),
        ];
        series
    }
}

/// Folds mono samples into the column peaks and extremes: `low` and `high`, then `mid_hp` twice, are the [`Bank`].
#[inline(never)]
fn measure_detail(bank: &mut Bank, mid_lp: &mut Biquad, c: &mut [f64; 3], e: &mut [f64; 2], mono: impl Iterator<Item = f64>) {
    let [b0, b1, b2] = bank.b;
    let [a0, a1] = bank.a;
    let [mut x1, mut x2] = bank.x;
    let [mut y1, mut y2] = bank.y;
    let mut held = Pair([c[0], c[2]]);
    let (mut mid, mut lowest, mut highest) = (c[1], e[0], e[1]);
    let mut lowpass = *mid_lp;
    for x in mono {
        let (xv, x1v, x2v) = (Pair::splat(x), Pair::splat(x1), Pair::splat(x2));
        let next = |h: usize| b0[h] * xv + b1[h] * x1v + b2[h] * x2v - a0[h] * y1[h] - a1[h] * y2[h];
        let y = [next(0), next(1)];
        (x2, x1, y2, y1) = (x1, x, y1, y);
        lowest = if x < lowest { x } else { lowest };
        highest = larger(highest, x);
        held = held.larger(y[0].abs());
        mid = larger(mid, lowpass.step(y[1].0[0]).abs());
    }
    *mid_lp = lowpass;
    (bank.x, bank.y) = ([x1, x2], [y1, y2]);
    *c = [held.0[0], mid, held.0[1]];
    *e = [lowest, highest];
}

/// Low, mid high-pass, mid low-pass, high.
fn band_filters(t: &Tuning, rate: f64) -> [Biquad; 4] {
    [
        Biquad::lowpass(t.low_hz, rate),
        Biquad::highpass(t.mid_low_hz, rate),
        Biquad::lowpass(t.mid_high_hz, rate),
        Biquad::highpass(t.high_hz, rate),
    ]
}

fn to_byte(t: &Tuning, band: usize, gain: u16, env: f64) -> u8 {
    let linear = (t.band_scale[band] * gain as f64 * env).min(PWV7_MAX as f64);
    (PWV7_MAX as f64 * (linear / PWV7_MAX as f64).powf(t.curve[band])).round() as u8
}

/// The `PWV7` gains (from the `PWV7` envelope rebuilt out of the raw peaks), then the fast envelope and the byte scale.
fn shape(raw: &[[f32; 3]], t: &Tuning) -> DetailSeries {
    let mut gains = [0u16; 3];
    for band in 0..3 {
        let mut env = 0.0f64;
        let mut max = 0.0f64;
        for group in raw.chunks(COLUMNS_PER_PWV7_COLUMN) {
            let peak = group.iter().map(|c| c[band] as f64).fold(0.0, f64::max);
            env = peak.max(env * t.release[band]);
            max = max.max(env);
        }
        gains[band] = band_gain(t, band, max);
    }
    let hop_ms = 1000.0 / DETAIL_COLUMNS_PER_SECOND;
    let mut planes = [Vec::with_capacity(raw.len()), Vec::with_capacity(raw.len()), Vec::with_capacity(raw.len())];
    for band in 0..3 {
        let release = (-hop_ms / RELEASE_MS[band]).exp();
        let mut env = 0.0f64;
        for c in raw {
            env = (c[band] as f64).max(env * release);
            planes[band].push(to_byte(t, band, gains[band], env));
        }
    }
    DetailSeries {
        columns_per_second: DETAIL_COLUMNS_PER_SECOND as u32,
        gains,
        signal_gain: (t.band_scale[0] * gains[0] as f64) as f32,
        planes,
        signal: Default::default(),
    }
}

const WINDOW_MAGIC: &[u8; 4] = b"FFWW";
const WINDOW_FORMAT_VERSION: u8 = 1;
const WINDOW_HEADER_LEN: usize = 32;

/// One stretch of the track at `len()` evenly spaced columns from `start_s` to `end_s`: the band bytes on the detail
/// scale, and the lowest and highest mono sample of each column (full scale `i16::MAX`). A column narrower than one
/// sample holds the signal interpolated at its centre (lowest equals highest), so the samples can be drawn as a line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindowSeries {
    pub sample_rate: u32,
    pub start_s: f64,
    pub end_s: f64,
    pub planes: [Vec<u8>; 3],
    pub lowest: Vec<i16>,
    pub highest: Vec<i16>,
}

impl WindowSeries {
    pub fn len(&self) -> usize {
        self.lowest.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `FFWW`, version, three zero bytes, column count (u32 LE), sample rate (u32 LE), start and end in seconds
    /// (f64 LE), then the lowest and highest sample planes (i16 LE) and the low, mid and high band planes (u8).
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.len();
        let mut out = Vec::with_capacity(WINDOW_HEADER_LEN + 7 * n);
        out.extend_from_slice(WINDOW_MAGIC);
        out.extend_from_slice(&[WINDOW_FORMAT_VERSION, 0, 0, 0]);
        out.extend_from_slice(&(n as u32).to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&self.start_s.to_le_bytes());
        out.extend_from_slice(&self.end_s.to_le_bytes());
        for plane in [&self.lowest, &self.highest] {
            for v in plane {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        for plane in &self.planes {
            out.extend_from_slice(plane);
        }
        out
    }
}

/// Measures `[start_s, end_s)` of mono audio into `columns` columns. `mono[0]` is the track's frame `first_frame`;
/// the audio before `start_s` warms the filters and envelopes up (a quarter second is plenty). `gains` come from the
/// track's [`DetailSeries`], so the bytes match it where both exist.
pub fn measure_window(
    mono: &[f32],
    first_frame: u64,
    sample_rate: u32,
    gains: [u16; 3],
    start_s: f64,
    end_s: f64,
    columns: usize,
) -> WindowSeries {
    let t = Tuning::default();
    let rate = sample_rate.max(1) as f64;
    let [mut low, mut mid_hp, mut mid_lp, mut high] = band_filters(&t, rate);
    let release: [f64; 3] = std::array::from_fn(|b| (-1000.0 / (rate * RELEASE_MS[b])).exp());
    let mut env = [0.0f64; 3];
    let mut envelopes: [Vec<f32>; 3] = std::array::from_fn(|_| Vec::with_capacity(mono.len()));
    for &x in mono {
        let x = x as f64;
        let band = [low.step(x).abs(), mid_lp.step(mid_hp.step(x)).abs(), high.step(x).abs()];
        for b in 0..3 {
            env[b] = band[b].max(env[b] * release[b]);
            envelopes[b].push(env[b] as f32);
        }
    }

    let columns = columns.max(1);
    let frames_per_column = (end_s - start_s) * rate / columns as f64;
    let interpolate = |frame: f64, values: &[f32]| -> f64 {
        let local = frame - first_frame as f64;
        if local < 0.0 || values.is_empty() || local > (values.len() - 1) as f64 {
            return 0.0;
        }
        let i = local.floor() as usize;
        let next = values[(i + 1).min(values.len() - 1)] as f64;
        values[i] as f64 + (next - values[i] as f64) * (local - i as f64)
    };
    let to_sample = |v: f64| (v.clamp(-1.0, 1.0) * i16::MAX as f64).round() as i16;
    let local_index = |frame: f64| ((frame.ceil() as i64 - first_frame as i64).max(0) as usize).min(mono.len());

    let mut out = WindowSeries {
        sample_rate,
        start_s,
        end_s,
        planes: std::array::from_fn(|_| Vec::with_capacity(columns)),
        lowest: Vec::with_capacity(columns),
        highest: Vec::with_capacity(columns),
    };
    for c in 0..columns {
        let from = start_s * rate + c as f64 * frames_per_column;
        let to = from + frames_per_column;
        let (i0, i1) = (local_index(from), local_index(to));
        if i0 < i1 {
            let span = &mono[i0..i1];
            let lo = span.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = span.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            out.lowest.push(to_sample(lo as f64));
            out.highest.push(to_sample(hi as f64));
            for b in 0..3 {
                let peak = envelopes[b][i0..i1].iter().copied().fold(0.0f32, f32::max);
                out.planes[b].push(to_byte(&t, b, gains[b], peak as f64));
            }
        } else {
            let centre = (from + to) / 2.0;
            let v = to_sample(interpolate(centre, mono));
            out.lowest.push(v);
            out.highest.push(v);
            for b in 0..3 {
                out.planes[b].push(to_byte(&t, b, gains[b], interpolate(centre, &envelopes[b])));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waveform::{analyze, Meter};

    const RATE: u32 = 44_100;

    fn kick_track(seconds: f64) -> Vec<f32> {
        let frames = (RATE as f64 * seconds) as usize;
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let t = i as f64 / RATE as f64;
            let beat = (t * 2.0).fract() / 2.0;
            let sweep = 55.0 + 120.0 * (-beat * 40.0).exp();
            let kick = (-beat * 30.0).exp() * (2.0 * std::f64::consts::PI * sweep * beat).sin();
            let hat = if (t * 4.0).fract() < 0.01 { ((i * 7919 % 1000) as f64 / 500.0 - 1.0) * 0.4 } else { 0.0 };
            let x = (0.7 * kick + hat) as f32;
            out.extend_from_slice(&[x, x]);
        }
        out
    }

    fn measure(samples: &[f32]) -> (DetailMeter, Meter) {
        let mut detail = DetailMeter::new(RATE, 2);
        let mut reference = Meter::new(RATE, 2);
        for block in samples.chunks(2 * 4096) {
            detail.push(block);
            reference.push(block);
        }
        (detail, reference)
    }

    fn scalar_columns(rate: u32, channels: usize, chunks: &[&[f32]]) -> (Vec<[f32; 3]>, Vec<[f32; 2]>) {
        let rate_f = rate.max(1) as f64;
        let [mut low, mut mid_hp, mut mid_lp, mut high] = band_filters(&Tuning::default(), rate_f);
        let (mut peaks, mut extremes) = (Vec::new(), Vec::new());
        let (mut c, mut e) = ([0.0f64; 3], NO_EXTREMES);
        let mut frames = 0u64;
        let mut next_edge = (rate_f / DETAIL_COLUMNS_PER_SECOND).round() as u64;
        for chunk in chunks {
            for frame in chunk.chunks_exact(channels) {
                while frames >= next_edge {
                    peaks.push([c[0] as f32, c[1] as f32, c[2] as f32]);
                    extremes.push([e[0] as f32, e[1] as f32]);
                    c = [0.0; 3];
                    e = NO_EXTREMES;
                    next_edge = ((peaks.len() + 1) as f64 * rate_f / DETAIL_COLUMNS_PER_SECOND).round() as u64;
                }
                let x = frame.iter().map(|&s| s as f64).sum::<f64>() / channels as f64;
                e = [e[0].min(x), e[1].max(x)];
                c[0] = c[0].max(low.step(x).abs());
                let mid_high_passed = mid_hp.step(x);
                c[1] = c[1].max(mid_lp.step(mid_high_passed).abs());
                c[2] = c[2].max(high.step(x).abs());
                frames += 1;
            }
        }
        peaks.push([c[0] as f32, c[1] as f32, c[2] as f32]);
        extremes.push([e[0] as f32, e[1] as f32]);
        let n = ((frames as f64 * DETAIL_COLUMNS_PER_SECOND / rate_f).ceil() as usize).max(1);
        peaks.resize(n, [0.0; 3]);
        extremes.resize(n, [0.0; 2]);
        (peaks, extremes)
    }

    #[test]
    fn the_detail_meter_matches_a_sample_by_sample_reference_bit_for_bit() {
        for (rate, channels) in [(44100, 1), (44100, 2), (48000, 2), (22050, 1), (96000, 3), (8000, 6), (1000, 1), (1, 2)] {
            let audio = crate::waveform::tests::noisy(channels * rate.min(40_000) as usize, rate as u64 + channels as u64);
            let sizes = [channels * 777, channels, channels * 4096 + channels];
            let mut pieces: Vec<&[f32]> = Vec::new();
            let (mut at, mut k) = (0, 0);
            while at < audio.len() {
                let end = (at + sizes[k % 3]).min(audio.len());
                pieces.push(&audio[at..end]);
                (at, k) = (end, k + 1);
            }
            let mut meter = DetailMeter::new(rate, channels);
            for piece in &pieces {
                meter.push(piece);
            }
            meter.close_columns();
            let (peaks, extremes) = scalar_columns(rate, channels, &pieces);
            assert_eq!(format!("{:?}", (&meter.peaks, &meter.extremes)), format!("{:?}", (&peaks, &extremes)), "{rate} Hz, {channels} ch");
        }
    }

    #[test]
    fn resolution_is_2400_columns_per_second() {
        let (detail, _) = measure(&kick_track(3.0));
        let series = detail.finish();
        assert_eq!(series.columns_per_second, 2400);
        assert_eq!(series.len(), 7200);
        assert!(series.planes.iter().all(|p| p.len() == 7200));
    }

    #[test]
    fn the_peak_of_sixteen_columns_is_the_raw_pwv7_band_peak() {
        let (detail, reference) = measure(&kick_track(4.0));
        let raw = detail.raw_peaks();
        let columns = reference.finish().band_columns();
        assert_eq!(raw.len(), columns.len() * COLUMNS_PER_PWV7_COLUMN);
        for (i, column) in columns.iter().enumerate() {
            for band in 0..3 {
                let group = &raw[i * COLUMNS_PER_PWV7_COLUMN..(i + 1) * COLUMNS_PER_PWV7_COLUMN];
                let peak = group.iter().map(|c| c[band]).fold(0.0f32, f32::max) as f64;
                assert!((peak - column[band]).abs() < 1e-6, "column {i} band {band}: {peak} vs {}", column[band]);
            }
        }
    }

    #[test]
    fn downsampled_with_the_pwv7_release_it_follows_the_pwv7_low_and_mid_series() {
        let (detail, reference) = measure(&kick_track(8.0));
        let series = detail.finish();
        let pwv7 = analyze(&reference.finish()).pwv7;
        let tuning = Tuning::default();
        for band in 0..2 {
            let mut held = 0.0f64;
            let down: Vec<f64> = series.planes[band]
                .chunks(COLUMNS_PER_PWV7_COLUMN)
                .map(|g| {
                    held = (*g.iter().max().unwrap() as f64).max(held * tuning.release[band]);
                    held
                })
                .collect();
            let expected: Vec<f64> = pwv7.iter().map(|c| c[band] as f64).collect();
            let n = down.len().min(expected.len());
            let (a, b) = (&down[..n], &expected[..n]);
            let mean_a = a.iter().sum::<f64>() / n as f64;
            let mean_b = b.iter().sum::<f64>() / n as f64;
            let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - mean_a) * (y - mean_b)).sum();
            let var_a: f64 = a.iter().map(|x| (x - mean_a).powi(2)).sum();
            let var_b: f64 = b.iter().map(|y| (y - mean_b).powi(2)).sum();
            let r = cov / (var_a.sqrt() * var_b.sqrt());
            assert!(r > 0.97, "band {band}: correlation {r}");
            let max_a = a.iter().cloned().fold(0.0, f64::max);
            let max_b = b.iter().cloned().fold(0.0, f64::max);
            assert!((max_a - max_b).abs() <= 2.0, "band {band}: maxima {max_a} vs {max_b}");
        }
    }

    #[test]
    fn a_kick_attacks_within_a_few_columns_and_decays_faster_than_pwv7() {
        let (detail, reference) = measure(&kick_track(3.0));
        let series = detail.finish();
        let pwv7 = analyze(&reference.finish()).pwv7;
        let onset = 2400;
        let low = &series.planes[0];
        let before = low[onset - 4] as i32;
        let after = low[onset + 24..onset + 40].iter().map(|&v| v as i32).max().unwrap();
        assert!(after - before > 40, "attack: {before} -> {after}");
        let tail_detail = low[onset + 600] as f64;
        let tail_pwv7 = pwv7[(onset + 600) / COLUMNS_PER_PWV7_COLUMN][0] as f64;
        assert!(tail_detail < tail_pwv7, "decay {tail_detail} vs PWV7 {tail_pwv7}");
    }

    #[test]
    fn stretches_measured_apart_after_a_warm_up_join_into_the_series_of_one_meter() {
        let samples = kick_track(9.0);
        let (one, _) = measure(&samples);
        let one = one.finish();

        let stretch = 2 * 100_003;
        let mut meter = DetailMeter::new(RATE, 2);
        for (k, chunk) in samples.chunks(stretch).enumerate() {
            let start = k * stretch;
            let mut part = DetailMeter::starting_at(RATE, 2, (start / 2) as u64);
            part.warm(&samples[start.saturating_sub(2 * 4096)..start]);
            part.push(chunk);
            meter.join(part).unwrap();
        }
        assert_eq!(meter.finish(), one);
    }

    #[test]
    fn a_stretch_joins_only_where_the_meter_is() {
        let mut meter = DetailMeter::new(RATE, 2);
        meter.push(&[0.1; 2 * 1000]);
        assert!(meter.join(DetailMeter::starting_at(RATE, 2, 999)).is_err());
        assert!(meter.join(DetailMeter::starting_at(48_000, 2, 1000)).is_err());
        assert!(meter.join(DetailMeter::starting_at(RATE, 2, 1000)).is_ok());
    }

    #[test]
    fn a_stretch_that_starts_on_a_column_edge_closes_the_column_before_it() {
        let samples = kick_track(1.0);
        let (one, _) = measure(&samples);
        let one = one.finish();
        let edge = (40.0 * RATE as f64 / DETAIL_COLUMNS_PER_SECOND).round() as usize;
        let mut meter = DetailMeter::new(RATE, 2);
        meter.push(&samples[..2 * edge]);
        let mut part = DetailMeter::starting_at(RATE, 2, edge as u64);
        part.warm(&samples[..2 * edge]);
        part.push(&samples[2 * edge..]);
        meter.join(part).unwrap();
        assert_eq!(meter.finish(), one);
    }

    #[test]
    fn bytes_round_trip_and_reject_damage() {
        let (detail, _) = measure(&kick_track(1.0));
        let series = detail.finish();
        let bytes = series.to_bytes();
        assert_eq!(bytes.len(), HEADER_LEN + 5 * 2400);
        assert_eq!(DetailSeries::from_bytes(&bytes).unwrap(), series);
        assert!(DetailSeries::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(DetailSeries::from_bytes(b"nope").is_err());
    }

    #[test]
    fn silence_is_all_zero() {
        let mut meter = DetailMeter::new(RATE, 2);
        meter.push(&vec![0.0; 2 * RATE as usize]);
        let series = meter.finish();
        assert_eq!(series.len(), 2400);
        assert!(series.planes.iter().all(|p| p.iter().all(|&v| v == 0)));
        assert!(series.signal.iter().all(|p| p.len() == 2400 && p.iter().all(|&v| v == 0)));
    }

    #[test]
    fn the_signal_planes_hold_the_lowest_and_highest_sample_of_each_column_on_the_low_band_scale() {
        let rate = 48_000u32;
        let mono: Vec<f32> = (0..rate as usize)
            .map(|i| (i as f64 * 2.0 * std::f64::consts::PI * 50.0 / rate as f64).sin() as f32 * 0.3)
            .collect();
        let stereo: Vec<f32> = mono.iter().flat_map(|&x| [x, x]).collect();
        let mut meter = DetailMeter::new(rate, 2);
        meter.push(&stereo);
        let series = meter.finish();
        let [lowest, highest] = &series.signal;
        assert_eq!(lowest.len(), series.len());
        let per_column = rate as usize / 2400;
        let gain = series.signal_gain as f64;
        for c in [1, 7, 13, 101, 777] {
            let span = &mono[c * per_column..(c + 1) * per_column];
            let lo = (span.iter().cloned().fold(f32::INFINITY, f32::min) as f64 * gain).clamp(-127.0, 127.0);
            let hi = (span.iter().cloned().fold(f32::NEG_INFINITY, f32::max) as f64 * gain).clamp(-127.0, 127.0);
            assert!((lowest[c] as f64 - lo).abs() <= 1.0, "column {c}: {} vs {lo}", lowest[c]);
            assert!((highest[c] as f64 - hi).abs() <= 1.0, "column {c}: {} vs {hi}", highest[c]);
        }
        let low_peak = *series.planes[0][2400 / 4..].iter().max().unwrap() as f64;
        let signal_peak = *highest.iter().max().unwrap() as f64;
        assert!((low_peak - signal_peak).abs() <= 6.0, "a bass tone is as tall as its low band: {low_peak} vs {signal_peak}");
    }

    fn mono(stereo: &[f32]) -> Vec<f32> {
        stereo.chunks_exact(2).map(|f| (f[0] + f[1]) / 2.0).collect()
    }

    #[test]
    fn the_gains_survive_the_bytes() {
        let (detail, _) = measure(&kick_track(2.0));
        let series = detail.finish();
        assert!(series.gains.iter().all(|&g| g > 0));
        assert_eq!(DetailSeries::from_bytes(&series.to_bytes()).unwrap().gains, series.gains);
    }

    #[test]
    fn a_window_at_2400_columns_per_second_matches_the_detail_series() {
        let samples = kick_track(6.0);
        let (detail, _) = measure(&samples);
        let series = detail.finish();
        let audio = mono(&samples);
        let first_frame = RATE as u64 * 2;
        let (start_s, end_s) = (2.5, 3.5);
        let window = measure_window(&audio[first_frame as usize..], first_frame, RATE, series.gains, start_s, end_s, 2400);
        assert_eq!(window.len(), 2400);
        for band in 0..2 {
            let expected = &series.planes[band][6000..8400];
            let got = &window.planes[band];
            let close = got.iter().zip(expected).filter(|(a, b)| (**a as i32 - **b as i32).abs() <= 6).count();
            assert!(close * 100 >= 95 * got.len(), "band {band}: {close} of {} columns within 6", got.len());
        }
    }

    #[test]
    fn columns_narrower_than_a_sample_follow_the_signal() {
        let rate = 48_000u32;
        let audio: Vec<f32> = (0..rate as usize).map(|i| (i as f64 * 2.0 * std::f64::consts::PI * 100.0 / rate as f64).sin() as f32 * 0.5).collect();
        let (start_s, end_s, columns) = (0.5, 0.501, 480);
        let window = measure_window(&audio, 0, rate, [1, 1, 1], start_s, end_s, columns);
        assert!(window.lowest.iter().zip(&window.highest).all(|(a, b)| a == b));
        for (c, &v) in window.lowest.iter().enumerate() {
            let t = start_s + (c as f64 + 0.5) * (end_s - start_s) / columns as f64;
            let expected = (t * 2.0 * std::f64::consts::PI * 100.0).sin() * 0.5 * i16::MAX as f64;
            assert!((v as f64 - expected).abs() < 40.0, "column {c}: {v} vs {expected}");
        }
    }

    #[test]
    fn a_wide_column_holds_the_lowest_and_highest_sample() {
        let audio: Vec<f32> = (0..1000).map(|i| if i == 500 { 0.9 } else if i == 510 { -0.4 } else { 0.0 }).collect();
        let window = measure_window(&audio, 1000, 1000, [1, 1, 1], 1.0, 2.0, 10);
        assert_eq!(window.highest[5], (0.9 * i16::MAX as f64).round() as i16);
        assert_eq!(window.lowest[5], (-0.4 * i16::MAX as f64).round() as i16);
        assert_eq!(window.highest[4], 0);
        assert_eq!(window.to_bytes().len(), WINDOW_HEADER_LEN + 7 * 10);
    }
}
