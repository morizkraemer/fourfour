use pioneer_usb_writer::waveform::{analyze, Meter};

pub use pioneer_usb_writer::waveform::Waveforms;

pub fn compute(mono: &[f32], sample_rate: u32) -> Waveforms {
    let mut meter = Meter::new(sample_rate, 1);
    meter.push(mono);
    analyze(&meter.finish())
}

pub fn checksum(w: &Waveforms) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    };
    w.pwav.iter().for_each(|b| eat(*b));
    w.pwv7.iter().chain(&w.pwv6).flatten().for_each(|b| eat(*b));
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, rate: u32, seconds: f64) -> Vec<f32> {
        (0..(rate as f64 * seconds) as usize)
            .map(|i| (0.5 * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin()) as f32)
            .collect()
    }

    #[test]
    fn bass_tone_is_low_band_and_columns_follow_150_per_second() {
        let w = compute(&sine(60.0, 44_100, 4.0), 44_100);
        assert_eq!(w.pwv7.len(), 600);
        assert_eq!(w.pwav.len(), 400);
        let mid = w.pwv7[300];
        assert!(mid[0] > mid[2], "{mid:?}");
    }

    #[test]
    fn silence_has_zero_bands() {
        let w = compute(&vec![0.0; 44_100], 44_100);
        assert!(w.pwv7.iter().all(|c| *c == [0, 0, 0]));
    }
}
