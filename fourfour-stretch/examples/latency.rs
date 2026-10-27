use fourfour_stretch::{Preset, Stretch};

fn run(preset: Preset, rate: f64, shift: f32) {
    let sr = 48_000.0f32;
    let mut s = Stretch::new(2, sr, preset);
    s.set_transpose_factor(shift);
    let burst_at = 60_000usize;
    let total_in = 160_000usize;
    let mut source = vec![0.0f32; total_in * 2];
    for i in 0..64 {
        let w = (std::f32::consts::PI * i as f32 / 63.0).sin();
        let v = w * (2.0 * std::f32::consts::PI * 1500.0 * i as f32 / sr).sin();
        source[(burst_at + i) * 2] = v;
        source[(burst_at + i) * 2 + 1] = v;
    }
    let block = 256usize;
    let mut consumed = 0usize;
    let mut carry = 0.0f64;
    let mut peak = (0.0f32, 0usize);
    let mut out_index = 0usize;
    let mut out = vec![0.0f32; block * 2];
    while consumed + 400 < total_in {
        let want = block as f64 * rate + carry;
        let n_in = want.floor() as usize;
        carry = want - n_in as f64;
        let mut input = source[consumed * 2..(consumed + n_in) * 2].to_vec();
        s.process(&mut input, &mut out);
        consumed += n_in;
        for i in 0..block {
            let a = out[i * 2].abs();
            if a > peak.0 {
                peak = (a, out_index + i);
            }
        }
        out_index += block;
    }
    let lag_src = peak.1 as f64 * rate - (burst_at as f64 + 32.0);
    println!(
        "{:?} rate {:.3} shift {:.3}: peak at out {} lag_src {:.0} (in_lat {} out_lat {} predicted {:.0})",
        (preset.block_seconds, preset.interval_seconds),
        rate,
        shift,
        peak.1,
        lag_src,
        s.input_latency(),
        s.output_latency(),
        s.input_latency() as f64 + s.output_latency() as f64 * rate
    );
}

fn main() {
    for preset in [Preset::DEFAULT, Preset::CHEAPER] {
        for rate in [0.84, 0.92, 1.0, 1.08, 1.16, 1.5] {
            run(preset, rate, 1.0);
        }
        run(preset, 1.0, 1.122);
    }
}
