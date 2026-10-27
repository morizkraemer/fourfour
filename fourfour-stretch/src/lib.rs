//! Safe wrapper around Signalsmith Stretch (MIT, vendored unmodified in `vendor/`).
//!
//! `process` takes N input frames and produces M output frames, so the time ratio can change on
//! every call. All buffers are interleaved and owned by the caller; nothing allocates after
//! `new`.

use std::os::raw::{c_int, c_long};

#[repr(C)]
struct Handle {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn ff_stretch_create(
        channels: c_int,
        block: c_int,
        interval: c_int,
        split_computation: c_int,
        seed: c_long,
    ) -> *mut Handle;
    fn ff_stretch_destroy(h: *mut Handle);
    fn ff_stretch_reset(h: *mut Handle);
    fn ff_stretch_block_samples(h: *const Handle) -> c_int;
    fn ff_stretch_interval_samples(h: *const Handle) -> c_int;
    fn ff_stretch_input_latency(h: *const Handle) -> c_int;
    fn ff_stretch_output_latency(h: *const Handle) -> c_int;
    fn ff_stretch_set_transpose_factor(h: *mut Handle, factor: f32);
    fn ff_stretch_seek(h: *mut Handle, input: *mut f32, frames: c_int, playback_rate: f64);
    fn ff_stretch_process(
        h: *mut Handle,
        input: *mut f32,
        in_frames: c_int,
        output: *mut f32,
        out_frames: c_int,
    );
    fn ff_stretch_count_arm(on: c_int);
    fn ff_stretch_count_read(allocs: *mut usize, frees: *mut usize);
}

/// Window and hop length as fractions of a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub block_seconds: f32,
    pub interval_seconds: f32,
    pub split_computation: bool,
}

impl Preset {
    pub const DEFAULT: Preset = Preset {
        block_seconds: 0.12,
        interval_seconds: 0.03,
        split_computation: false,
    };
    /// The default window with the work spread over the hop, so no single callback pays for a
    /// whole spectrum.
    pub const REALTIME: Preset = Preset {
        block_seconds: 0.12,
        interval_seconds: 0.03,
        split_computation: true,
    };
    pub const CHEAPER: Preset = Preset {
        block_seconds: 0.10,
        interval_seconds: 0.04,
        split_computation: true,
    };
}

pub struct Stretch {
    handle: *mut Handle,
    channels: usize,
}

unsafe impl Send for Stretch {}

impl Stretch {
    pub fn new(channels: usize, sample_rate: f32, preset: Preset) -> Self {
        let block = (sample_rate * preset.block_seconds) as usize;
        let interval = (sample_rate * preset.interval_seconds) as usize;
        Self::with_block(channels, block, interval, preset.split_computation)
    }

    pub fn with_block(channels: usize, block: usize, interval: usize, split: bool) -> Self {
        assert!(channels > 0);
        let handle = unsafe {
            ff_stretch_create(
                channels as c_int,
                block as c_int,
                interval as c_int,
                split as c_int,
                0x5eed,
            )
        };
        assert!(!handle.is_null(), "signalsmith stretch allocation failed");
        Self { handle, channels }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn block_samples(&self) -> usize {
        unsafe { ff_stretch_block_samples(self.handle) as usize }
    }

    pub fn interval_samples(&self) -> usize {
        unsafe { ff_stretch_interval_samples(self.handle) as usize }
    }

    pub fn input_latency(&self) -> usize {
        unsafe { ff_stretch_input_latency(self.handle) as usize }
    }

    pub fn output_latency(&self) -> usize {
        unsafe { ff_stretch_output_latency(self.handle) as usize }
    }

    pub fn reset(&mut self) {
        unsafe { ff_stretch_reset(self.handle) }
    }

    /// Frequency multiplier applied to the output (2.0 = one octave up).
    pub fn set_transpose_factor(&mut self, factor: f32) {
        unsafe { ff_stretch_set_transpose_factor(self.handle, factor) }
    }

    /// Feeds previous input without producing output (pre-roll after a reset).
    pub fn seek(&mut self, input: &mut [f32], playback_rate: f64) {
        let frames = input.len() / self.channels;
        unsafe { ff_stretch_seek(self.handle, input.as_mut_ptr(), frames as c_int, playback_rate) }
    }

    /// Consumes `input` (interleaved) and fills `output` (interleaved). The time ratio of this
    /// call is `input frames / output frames`.
    pub fn process(&mut self, input: &mut [f32], output: &mut [f32]) {
        let in_frames = input.len() / self.channels;
        let out_frames = output.len() / self.channels;
        unsafe {
            ff_stretch_process(
                self.handle,
                input.as_mut_ptr(),
                in_frames as c_int,
                output.as_mut_ptr(),
                out_frames as c_int,
            )
        }
    }
}

impl Drop for Stretch {
    fn drop(&mut self) {
        unsafe { ff_stretch_destroy(self.handle) }
    }
}

/// C++ allocation counting for the calling thread. Only counts when built with the
/// `count-allocs` feature (the Rust global allocator cannot see C++ `operator new`).
pub mod alloc_count {
    use super::*;

    pub fn arm() {
        unsafe { ff_stretch_count_arm(1) }
    }

    pub fn disarm() -> (usize, usize) {
        let (mut a, mut f) = (0usize, 0usize);
        unsafe {
            ff_stretch_count_read(&mut a, &mut f);
            ff_stretch_count_arm(0);
        }
        (a, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_stream_keeps_level() {
        let sr = 48_000.0;
        let mut s = Stretch::new(2, sr, Preset::DEFAULT);
        let n = 480usize;
        let mut t = 0usize;
        let mut peak = 0.0f32;
        for block in 0..400 {
            let mut input = vec![0.0f32; n * 2];
            for i in 0..n {
                let v = (2.0 * std::f32::consts::PI * 440.0 * (t + i) as f32 / sr).sin() * 0.5;
                input[i * 2] = v;
                input[i * 2 + 1] = v;
            }
            t += n;
            let mut out = vec![0.0f32; n * 2];
            s.process(&mut input, &mut out);
            if block > 100 {
                peak = out.iter().fold(peak, |p, v| p.max(v.abs()));
            }
        }
        assert!((peak - 0.5).abs() < 0.03, "peak {peak}");
    }
}
