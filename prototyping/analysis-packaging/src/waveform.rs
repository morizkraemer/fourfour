use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

const FFT_SIZE: usize = 2048;
pub const DETAIL_PER_SECOND: f64 = 150.0;
pub const OVERVIEW_ENTRIES: usize = 1200;
pub const PWAV_ENTRIES: usize = 400;

pub struct Waveforms {
    pub pwav: [u8; PWAV_ENTRIES],
    pub detail: Vec<[u8; 3]>,
    pub overview: Vec<[u8; 3]>,
}

pub fn compute(mono: &[f32], sr: u32, threads: usize) -> Waveforms {
    let duration_s = mono.len() as f64 / sr as f64;
    Waveforms {
        pwav: pwav_preview(mono),
        detail: band_entries(mono, sr, (duration_s * DETAIL_PER_SECOND) as usize, threads),
        overview: band_entries(mono, sr, OVERVIEW_ENTRIES, threads),
    }
}

pub fn checksum(w: &Waveforms) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    };
    w.pwav.iter().for_each(|b| eat(*b));
    w.detail.iter().chain(&w.overview).flatten().for_each(|b| eat(*b));
    h
}

pub fn band_entries(mono: &[f32], sr: u32, entries: usize, threads: usize) -> Vec<[u8; 3]> {
    let hop = (mono.len() / entries.max(1)).max(1);
    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
    let bin_hz = sr as f32 / FFT_SIZE as f32;
    let bin_of = |hz: f32| (hz / bin_hz).ceil() as usize;
    let (low_lo, low_hi, high_lo) = (bin_of(20.0), bin_of(250.0), bin_of(4000.0));
    let (mid_lo, mid_hi) = (low_hi, high_lo);
    let nbins = FFT_SIZE / 2 + 1;
    let count = entries.min(mono.len() / hop);

    let mut raw = vec![(0f32, 0f32, 0f32); count];
    let per = count.div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (ci, chunk_out) in raw.chunks_mut(per).enumerate() {
            let fft = &fft;
            s.spawn(move || {
                let mut buf = vec![Complex { re: 0f32, im: 0f32 }; FFT_SIZE];
                for (k, out) in chunk_out.iter_mut().enumerate() {
                    let i = ci * per + k;
                    let chunk = &mono[i * hop..(i + 1) * hop];
                    for (j, c) in buf.iter_mut().enumerate() {
                        *c = Complex { re: chunk.get(j).copied().unwrap_or(0.0), im: 0.0 };
                    }
                    fft.process(&mut buf);
                    let mean = |lo: usize, hi: usize| {
                        let hi = hi.min(nbins);
                        if hi <= lo { 0.0 } else { buf[lo..hi].iter().map(|c| c.norm()).sum::<f32>() / (hi - lo) as f32 }
                    };
                    *out = (mean(low_lo, low_hi), mean(mid_lo, mid_hi), mean(high_lo, nbins));
                }
            });
        }
    });
    let (mut max_l, mut max_m, mut max_h) = (0f32, 0f32, 0f32);
    for (l, m, h) in &raw {
        max_l = max_l.max(*l);
        max_m = max_m.max(*m);
        max_h = max_h.max(*h);
    }
    let scale = |v: f32, max: f32, top: f32| if max > 0.0 { ((v / max) * top) as u8 } else { 0 };
    let mut out: Vec<[u8; 3]> = raw.into_iter().map(|(l, m, h)| [scale(l, max_l, 96.0), scale(m, max_m, 64.0), scale(h, max_h, 48.0)]).collect();
    out.resize(entries, [0, 0, 0]);
    out
}

pub fn pwav_preview(mono: &[f32]) -> [u8; PWAV_ENTRIES] {
    let chunk = (mono.len() / PWAV_ENTRIES).max(1);
    let mut rms = [0f32; PWAV_ENTRIES];
    for (i, r) in rms.iter_mut().enumerate() {
        let start = i * chunk;
        if start >= mono.len() {
            break;
        }
        let end = (start + chunk).min(mono.len());
        let c = &mono[start..end];
        *r = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
    }
    let max = rms.iter().cloned().fold(0f32, f32::max);
    let mut out = [0u8; PWAV_ENTRIES];
    if max > 0.0 {
        for (o, r) in out.iter_mut().zip(rms) {
            let n = r / max;
            *o = (((n * 7.0) as u8) << 5) | ((n * 31.0) as u8 & 0x1F);
        }
    }
    out
}
