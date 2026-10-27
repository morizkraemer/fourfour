use super::*;

use std::f32::consts::PI;

use crate::tempo::{Path as TempoPath, TempoTarget};

pub(crate) const SR: u32 = 48_000;

pub(crate) fn sine(freq: f32, seconds: f32, amp: f32) -> Vec<f32> {
    let frames = (seconds * SR as f32) as usize;
    let mut out = Vec::with_capacity(frames * 2);
    for n in 0..frames {
        let v = amp * (2.0 * std::f64::consts::PI * freq as f64 * n as f64 / SR as f64).sin() as f32;
        out.push(v);
        out.push(v);
    }
    out
}

pub(crate) fn click_train(period_seconds: f32, seconds: f32) -> Vec<f32> {
    let frames = (seconds * SR as f32) as usize;
    let period = (period_seconds * SR as f32) as usize;
    let mut out = vec![0.0f32; frames * 2];
    let mut at = period / 2;
    while at + 64 < frames {
        for i in 0..64 {
            let w = (PI * i as f32 / 63.0).sin();
            let v = 0.8 * w * (2.0 * PI * 1500.0 * i as f32 / SR as f32).sin();
            out[(at + i) * 2] = v;
            out[(at + i) * 2 + 1] = v;
        }
        at += period;
    }
    out
}

pub(crate) fn click_centers(period_seconds: f32, seconds: f32) -> Vec<f64> {
    let frames = (seconds * SR as f32) as usize;
    let period = (period_seconds * SR as f32) as usize;
    let mut out = Vec::new();
    let mut at = period / 2;
    while at + 64 < frames {
        out.push(at as f64 + 31.5);
        at += period;
    }
    out
}

pub(crate) fn noise(seconds: f32, amp: f32) -> Vec<f32> {
    let frames = (seconds * SR as f32) as usize;
    let mut state = 0x1234_5678u32;
    (0..frames * 2)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            ((state >> 8) as f32 / 8_388_608.0 - 1.0) * amp
        })
        .collect()
}

pub(crate) struct Lab {
    pub state: RenderState,
    pub pipe: SamplePipe,
    pub tempo: Arc<TempoParams>,
    source: Vec<f32>,
    next_frame: usize,
    start_frame: usize,
    pub block: usize,
    pub timings_ns: Vec<u64>,
    pub positions: Vec<f64>,
    pub paths: Vec<TempoPath>,
}

impl Lab {
    pub fn new(source: Vec<f32>, block: usize, preset: Preset) -> Self {
        let (pipe, cons) = new_sample_pipe();
        let tempo = Arc::new(TempoParams::new());
        let state = RenderState::new(
            cons,
            pipe.clone(),
            Arc::new(AtomicU32::new(0)),
            Arc::new(AtomicU32::new(0)),
            tempo.clone(),
            SR,
            preset,
        );
        let mut lab = Self {
            state,
            pipe,
            tempo,
            source,
            next_frame: 0,
            start_frame: 0,
            block,
            timings_ns: Vec::new(),
            positions: Vec::new(),
            paths: Vec::new(),
        };
        lab.locate(0);
        lab
    }

    pub fn source_frames(&self) -> usize {
        self.source.len() / 2
    }

    pub fn locate(&mut self, frame: usize) {
        self.pipe.supersede();
        self.state.render(&mut [], 2);
        self.next_frame = frame;
        self.start_frame = frame;
        self.fill();
    }

    fn fill(&mut self) {
        let mut producer = lock_ignoring_poison(&self.pipe.producer);
        let free_frames = producer.vacant_len() / 2;
        let remaining = self.source_frames().saturating_sub(self.next_frame);
        let n = free_frames.min(remaining);
        if n > 0 {
            producer.push_slice(&self.source[self.next_frame * 2..(self.next_frame + n) * 2]);
            self.next_frame += n;
        }
        if self.next_frame >= self.source_frames() {
            let generation = self.pipe.decode_generation.load(Ordering::SeqCst);
            self.pipe.ended_generation.store(generation, Ordering::SeqCst);
        }
    }

    pub fn position_frames(&self) -> f64 {
        self.start_frame as f64 + self.pipe.frames_played.load(Ordering::Relaxed) as f64
    }

    pub fn drained(&self) -> bool {
        let generation = self.pipe.decode_generation.load(Ordering::SeqCst);
        self.pipe.drained_generation.load(Ordering::SeqCst) == generation
    }

    pub fn step(&mut self, out: &mut Vec<f32>) {
        self.fill();
        let mut buf = vec![0.0f32; self.block * 2];
        let started = Instant::now();
        self.state.render(&mut buf, 2);
        self.timings_ns.push(started.elapsed().as_nanos() as u64);
        out.extend_from_slice(&buf);
        self.positions.push(self.position_frames());
    }

    pub fn run_blocks(&mut self, blocks: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(blocks * self.block * 2);
        for _ in 0..blocks {
            self.step(&mut out);
        }
        out
    }

    pub fn run_seconds(&mut self, seconds: f32) -> Vec<f32> {
        let blocks = (seconds * SR as f32 / self.block as f32).ceil() as usize;
        self.run_blocks(blocks)
    }

    pub fn run_until_drained(&mut self, max_seconds: f32) -> Vec<f32> {
        let max_blocks = (max_seconds * SR as f32 / self.block as f32) as usize;
        let mut out = Vec::new();
        for _ in 0..max_blocks {
            self.step(&mut out);
            if self.drained() {
                break;
            }
        }
        out
    }

    pub fn set(&self, ratio: f64, semitones: f64, key_lock: bool) {
        self.tempo.set_ratio(ratio);
        self.tempo.set_semitones(semitones);
        self.tempo.set_key_lock(key_lock);
    }

    pub fn settle(&mut self) {
        self.run_seconds(1.0);
    }
}

pub(crate) fn left(stereo: &[f32]) -> Vec<f32> {
    stereo.chunks(2).map(|f| f[0]).collect()
}

pub(crate) fn zero_cross_freq(x: &[f32], from: usize, to: usize) -> f64 {
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0usize;
    for i in from.max(1)..to.min(x.len()) {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = i as f64 - 1.0 + (-x[i - 1] as f64) / (x[i] as f64 - x[i - 1] as f64);
            if first.is_none() {
                first = Some(t);
            }
            last = t;
            count += 1;
        }
    }
    let first = first.expect("no zero crossings");
    (count - 1) as f64 * SR as f64 / (last - first)
}

pub(crate) fn cents(measured: f64, expected: f64) -> f64 {
    1200.0 * (measured / expected).log2()
}

pub(crate) fn peak_positions(x: &[f32], threshold: f32, min_gap: usize) -> Vec<f64> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < x.len() {
        if x[i].abs() > threshold {
            let end = (i + min_gap).min(x.len());
            let mut best = i;
            let mut weight = 0.0f64;
            let mut moment = 0.0f64;
            for j in i..end {
                if x[j].abs() > x[best].abs() {
                    best = j;
                }
            }
            for j in best.saturating_sub(48)..(best + 48).min(x.len()) {
                let e = (x[j] as f64) * (x[j] as f64);
                weight += e;
                moment += e * j as f64;
            }
            out.push(moment / weight);
            i = end.max(best + min_gap / 2);
        } else {
            i += 1;
        }
    }
    out
}

pub(crate) fn max_second_difference(x: &[f32], from: usize) -> (f32, usize) {
    let mut best = (0.0f32, 0usize);
    for i in from.max(1)..x.len() - 1 {
        let d = (x[i + 1] - 2.0 * x[i] + x[i - 1]).abs();
        if d > best.0 {
            best = (d, i);
        }
    }
    best
}

#[test]
fn identity_is_bit_exact_with_key_lock_on_and_off() {
    for lock in [true, false] {
        for block in [64usize, 100, 256, 512, 1000] {
            let source = noise(2.0, 0.6);
            let mut lab = Lab::new(source.clone(), block, STRETCH_PRESET);
            lab.set(1.0, 0.0, lock);
            let out = lab.run_seconds(1.5);
            let frames = out.len() / 2;
            for f in 256..frames {
                assert_eq!(
                    out[f * 2].to_bits(),
                    source[f * 2].to_bits(),
                    "lock {lock} block {block} frame {f}"
                );
                assert_eq!(out[f * 2 + 1].to_bits(), source[f * 2 + 1].to_bits());
            }
        }
    }
}

#[test]
fn key_lock_keeps_pitch_across_ratios() {
    for ratio in [0.84, 0.92, 0.98, 1.02, 1.08, 1.16, 1.5] {
        let mut lab = Lab::new(sine(1000.0, 8.0, 0.5), 256, STRETCH_PRESET);
        lab.set(ratio, 0.0, true);
        let out = left(&lab.run_seconds(5.0));
        let f = zero_cross_freq(&out, 2 * SR as usize, 5 * SR as usize);
        let error = cents(f, 1000.0);
        assert!(error.abs() < 1.0, "ratio {ratio}: {error:.3} cents");
    }
}

#[test]
fn key_shift_lands_on_the_semitone() {
    for semitones in [-12.0, -5.0, -2.0, 2.0, 7.0, 12.0] {
        let mut lab = Lab::new(sine(500.0, 8.0, 0.5), 256, STRETCH_PRESET);
        lab.set(1.0, semitones, true);
        let out = left(&lab.run_seconds(5.0));
        let expected = 500.0 * 2f64.powf(semitones / 12.0);
        let f = zero_cross_freq(&out, 2 * SR as usize, 5 * SR as usize);
        let error = cents(f, expected);
        assert!(error.abs() < 1.0, "{semitones} st: {error:.3} cents");
    }
}

#[test]
fn key_shift_with_tempo_change_stays_independent() {
    let mut lab = Lab::new(sine(500.0, 8.0, 0.5), 256, STRETCH_PRESET);
    lab.set(1.08, 3.0, true);
    let out = left(&lab.run_seconds(5.0));
    let expected = 500.0 * 2f64.powf(3.0 / 12.0);
    let f = zero_cross_freq(&out, 2 * SR as usize, 5 * SR as usize);
    assert!(cents(f, expected).abs() < 1.0, "{} cents", cents(f, expected));
}

#[test]
fn varispeed_moves_pitch_with_tempo() {
    for ratio in [0.84, 0.92, 1.08, 1.16, 1.5] {
        let mut lab = Lab::new(sine(1000.0, 8.0, 0.5), 256, STRETCH_PRESET);
        lab.set(ratio, 0.0, false);
        let out = left(&lab.run_seconds(5.0));
        let f = zero_cross_freq(&out, 2 * SR as usize, 5 * SR as usize);
        let error = cents(f, 1000.0 * ratio);
        assert!(error.abs() < 0.2, "ratio {ratio}: {error:.3} cents");
    }
}

#[test]
fn playing_ten_seconds_at_ratio_takes_ten_over_ratio() {
    for (ratio, lock) in [(0.92, true), (1.08, true), (1.16, true), (1.08, false), (0.84, false)] {
        let mut lab = Lab::new(sine(440.0, 10.0, 0.5), 256, STRETCH_PRESET);
        lab.set(ratio, 0.0, lock);
        lab.settle();
        lab.locate(0);
        let out = lab.run_until_drained(20.0);
        let seconds = out.len() as f64 / 2.0 / SR as f64;
        let expected = 10.0 / ratio;
        assert!(
            (seconds - expected).abs() < 0.03,
            "ratio {ratio} lock {lock}: {seconds:.4}s vs {expected:.4}s"
        );
    }
}

pub(crate) fn heard_error_ms(period: f32, ratio: f64, semitones: f64, lock: bool, start_frame: usize) -> f64 {
    let seconds = 12.0;
    let mut lab = Lab::new(click_train(period, seconds), 256, STRETCH_PRESET);
    lab.set(ratio, semitones, lock);
    lab.settle();
    lab.locate(start_frame);
    lab.positions.clear();
    let out = left(&lab.run_seconds(8.0));
    let clicks = click_centers(period, seconds);
    let peaks = peak_positions(&out, 0.03, SR as usize / 4);
    let mut worst = 0.0f64;
    let mut matched = 0;
    for peak in peaks {
        let block = (peak as usize / lab.block).min(lab.positions.len() - 1);
        let block_start = if block == 0 { start_frame as f64 } else { lab.positions[block - 1] };
        let block_end = lab.positions[block];
        let within = (peak - (block * lab.block) as f64) / lab.block as f64;
        let heard = block_start + (block_end - block_start) * within;
        let nearest = clicks
            .iter()
            .copied()
            .min_by(|a, b| (a - heard).abs().total_cmp(&(b - heard).abs()))
            .unwrap();
        worst = worst.max((heard - nearest).abs() * 1000.0 / SR as f64);
        matched += 1;
    }
    assert!(matched >= 8, "only {matched} clicks heard");
    worst
}

#[test]
fn reported_position_matches_the_audio_after_seek_at_every_ratio() {
    for (ratio, semitones, lock) in [
        (1.0, 0.0, true),
        (0.84, 0.0, true),
        (0.92, 0.0, true),
        (1.08, 0.0, true),
        (1.16, 0.0, true),
        (1.5, 0.0, true),
        (1.08, 3.0, true),
        (1.08, 0.0, false),
        (0.84, 0.0, false),
    ] {
        let error = heard_error_ms(0.5, ratio, semitones, lock, 48_000 * 2 + 777);
        assert!(error < 5.0, "ratio {ratio} st {semitones} lock {lock}: {error:.2} ms");
    }
}

#[test]
fn no_discontinuities_while_tempo_and_key_move() {
    let theory = |f: f32| 0.5 * (2.0 * PI * f / SR as f32).powi(2);
    let mut lab = Lab::new(sine(220.0, 30.0, 0.5), 256, STRETCH_PRESET);
    lab.settle();
    let mut out = Vec::new();
    for b in 0..(10 * SR as usize / 256) {
        let t = b as f64 * 256.0 / SR as f64;
        lab.set(1.0 + 0.08 * (t / 10.0 * 2.0 - 1.0).abs() * (t / 10.0 * 2.0 - 1.0).signum(), 0.0, true);
        lab.step(&mut out);
    }
    let x = left(&out);
    let (spike, at) = max_second_difference(&x, SR as usize / 2);
    assert!(spike < theory(220.0) * 3.0, "tempo sweep spike {spike} at {at}");

    let mut lab = Lab::new(sine(220.0, 30.0, 0.5), 256, STRETCH_PRESET);
    lab.settle();
    let mut out = Vec::new();
    for b in 0..(6 * SR as usize / 256) {
        let seconds = b as f64 * 256.0 / SR as f64;
        lab.set(1.0, if (seconds as usize / 2) % 2 == 0 { 3.0 } else { -2.0 }, true);
        lab.step(&mut out);
    }
    let x = left(&out);
    let (spike, at) = max_second_difference(&x, SR as usize / 2);
    assert!(spike < theory(220.0 * 1.2) * 3.0, "key change spike {spike} at {at}");
}

#[test]
fn switching_between_paths_never_clicks() {
    let theory = |f: f32| 0.5 * (2.0 * PI * f / SR as f32).powi(2);
    let mut lab = Lab::new(sine(220.0, 30.0, 0.5), 256, STRETCH_PRESET);
    lab.settle();
    let script: [(f64, f64, bool); 8] = [
        (1.0, 0.0, true),
        (1.06, 0.0, true),
        (1.06, 0.0, false),
        (1.06, 2.0, true),
        (1.0, 0.0, true),
        (1.0, 0.0, false),
        (0.94, 0.0, false),
        (1.0, 0.0, true),
    ];
    let mut out = Vec::new();
    for (ratio, semitones, lock) in script {
        lab.set(ratio, semitones, lock);
        for _ in 0..(1.2 * SR as f32 / 256.0) as usize {
            lab.step(&mut out);
        }
    }
    let x = left(&out);
    let (spike, at) = max_second_difference(&x, SR as usize / 2);
    assert!(
        spike < theory(220.0 * 1.3) * 3.0,
        "path switch spike {spike} at {at} ({:.2} s)",
        at as f64 / SR as f64
    );
}

#[test]
fn engine_reports_the_path_it_plays() {
    let mut lab = Lab::new(sine(220.0, 10.0, 0.5), 256, STRETCH_PRESET);
    lab.settle();
    lab.run_seconds(0.2);
    lab.set(1.08, 0.0, true);
    let mut seen = Vec::new();
    for _ in 0..200 {
        let mut out = Vec::new();
        lab.step(&mut out);
        seen.push(lab.state.engine.rate());
    }
    assert!((seen.last().unwrap() - 1.08).abs() < 1e-3);
    assert_eq!(lab.tempo.target(), TempoTarget { ratio: 1.08f32 as f64, semitones: 0.0, key_lock: true });
}

#[test]
#[ignore]
fn lab_pitch_table() {
    for (block, interval) in [(0.12f32, 0.03f32)] {
        let preset = Preset { block_seconds: block, interval_seconds: interval, split_computation: false };
        for freq in [110.0f32, 220.0, 440.0] {
            let mut row = format!("{block}/{interval} {freq:>6} Hz:");
            for ratio in [1.35, 1.4, 1.45, 1.48, 1.5] {
                let mut lab = Lab::new(sine(freq, 8.0, 0.5), 256, preset);
                lab.set(ratio, 0.0, true);
                let out = left(&lab.run_seconds(6.0));
                let f = zero_cross_freq(&out, 2 * SR as usize, 6 * SR as usize);
                row += &format!(" r{ratio} {:+.2}c", cents(f, freq as f64));
            }
            println!("{row}");
        }
    }
}

#[test]
#[ignore]
fn lab_spike_debug() {
    let theory = 0.5 * (2.0 * PI * 220.0 / SR as f32).powi(2);
    for ratio in [1.3f64, 1.35, 1.4, 1.45, 1.5] {
        let mut lab = Lab::new(sine(220.0, 30.0, 0.5), 256, STRETCH_PRESET);
        lab.set(ratio, 0.0, true);
        let out = left(&lab.run_seconds(6.0));
        let (spike, at) = max_second_difference(&out, SR as usize);
        let peak = out[SR as usize..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let floor = out[SR as usize..].iter().fold(1.0f32, |m, v| m.min(v.abs()));
        println!("static {ratio}: spike/theory {:.2} at {:.3}s, peak {peak:.4}", spike / theory, at as f64 / SR as f64);
        let _ = floor;
    }
    let mut lab = Lab::new(sine(220.0, 30.0, 0.5), 256, STRETCH_PRESET);
    lab.settle();
    let mut out = Vec::new();
    let mut targets = Vec::new();
    for b in 0..(10 * SR as usize / 256) {
        let t = b as f64 * 256.0 / SR as f64;
        let r = 0.92 + 0.016 * t;
        targets.push(r);
        lab.set(r, 0.0, true);
        lab.step(&mut out);
    }
    let x = left(&out);
    let mut spikes = Vec::new();
    for i in SR as usize / 2..x.len() - 1 {
        let d = (x[i + 1] - 2.0 * x[i] + x[i - 1]).abs();
        if d > theory * 1.8 {
            spikes.push((i, d / theory));
        }
    }
    println!("sweep spikes>1.8x: {}", spikes.len());
    for (i, d) in spikes.iter().take(20) {
        println!("  at {:.4}s block {} d/theory {:.2}", *i as f64 / SR as f64, i / 256, d);
    }
}

#[test]
#[ignore]
fn lab_switch_debug() {
    let mut lab = Lab::new(sine(220.0, 30.0, 0.5), 256, STRETCH_PRESET);
    lab.settle();
    let script: [(f64, f64, bool); 8] = [
        (1.0, 0.0, true), (1.06, 0.0, true), (1.06, 0.0, false), (1.06, 2.0, true),
        (1.0, 0.0, true), (1.0, 0.0, false), (0.94, 0.0, false), (1.0, 0.0, true),
    ];
    let mut out = Vec::new();
    let mut log = Vec::new();
    for (ratio, semitones, lock) in script {
        lab.set(ratio, semitones, lock);
        for _ in 0..(1.2 * SR as f32 / 256.0) as usize {
            lab.step(&mut out);
            log.push((lab.state.engine.rate(), lab.state.engine.last_path()));
        }
    }
    let x = left(&out);
    let theory = 0.5 * (2.0 * PI * 220.0 / SR as f32).powi(2);
    let mut worst: Vec<(f32, usize)> = Vec::new();
    for i in SR as usize / 2..x.len() - 1 {
        let d = (x[i + 1] - 2.0 * x[i] + x[i - 1]).abs();
        if d > theory * 3.0 { worst.push((d / theory, i)); }
    }
    println!("spikes: {}", worst.len());
    let mut last_block = usize::MAX;
    for (d, i) in worst.iter().take(30) {
        let b = i / 256;
        if b != last_block {
            println!("block {b} ({:.3}s) d/theory {d:.1} rate {:.4} path {:?}", *i as f64 / SR as f64, log[b].0, log[b].1);
            last_block = b;
        }
    }
    for b in 222..236 { println!("  b{b} rate {:.4} {:?}", log[b].0, log[b].1); }
}

#[test]
#[ignore]
fn lab_trace() {
    for ratio in [1.4, 1.3] {
        let mut lab = Lab::new(sine(220.0, 12.0, 0.5), 256, STRETCH_PRESET);
        lab.set(ratio, 0.0, true);
        let out = left(&lab.run_seconds(8.0));
        let mut row = format!("ratio {ratio}:");
        let w = SR as usize / 2;
        let mut at = SR as usize;
        while at + w < out.len() {
            row += &format!(" {:+.1}", cents(zero_cross_freq(&out, at, at + w), 220.0));
            at += w;
        }
        println!("{row}");
    }
}
