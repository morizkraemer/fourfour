use fourfour_stretch::{Preset, Stretch};

fn freq(x: &[f32]) -> f64 {
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0usize;
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let t = i as f64 - 1.0 + (-x[i - 1] as f64) / (x[i] as f64 - x[i - 1] as f64);
            if first.is_none() {
                first = Some(t);
            }
            last = t;
            count += 1;
        }
    }
    (count - 1) as f64 * 48000.0 / (last - first.unwrap())
}

fn run(target: f64, hz: f64, block: usize, ramp: bool) {
    let mut s = Stretch::new(2, 48000.0, Preset::DEFAULT);
    let total = 48000 * 8;
    let source: Vec<f32> = (0..total)
        .flat_map(|n| {
            let v = (0.5 * (2.0 * std::f64::consts::PI * hz * n as f64 / 48000.0).sin()) as f32;
            [v, v]
        })
        .collect();
    let mut consumed = 0usize;
    let mut carry = 0.0f64;
    let mut left = Vec::new();
    let mut out = vec![0.0f32; block * 2];
    let mut rate = if ramp { 1.0 } else { target };
    while consumed + 2000 < total {
        if ramp {
            rate += (target - rate) * (1.0 - (-(block as f64) / 48000.0 / 0.03).exp());
        }
        let want = block as f64 * rate + carry;
        let n_in = want.floor() as usize;
        carry = want - n_in as f64;
        let mut input = source[consumed * 2..(consumed + n_in) * 2].to_vec();
        s.process(&mut input, &mut out);
        consumed += n_in;
        left.extend(out.chunks(2).map(|f| f[0]));
    }
    let f = freq(&left[48000..48000 * 4]);
    println!("ramp {ramp} rate {target} {hz} Hz -> {:+.2} cents", 1200.0 * (f / hz).log2());
}

fn main() {
    for ramp in [false, true] {
        for rate in [1.3, 1.4, 1.5] {
            for hz in [110.0, 220.0, 440.0] {
                run(rate, hz, 256, ramp);
            }
        }
    }
}
