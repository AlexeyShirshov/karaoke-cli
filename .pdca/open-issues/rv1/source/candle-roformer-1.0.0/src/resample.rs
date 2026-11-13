pub fn resample(input: &[f32], from_sr: u32, to_sr: u32) -> Vec<f32> {
    resample_planar(input, 1, from_sr, to_sr)
}

pub fn resample_planar(input: &[f32], channels: usize, from_sr: u32, to_sr: u32) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    if from_sr == to_sr {
        return input.to_vec();
    }
    let frames = input.len() / channels;
    let ratio = to_sr as f64 / from_sr as f64;
    let out_frames = ((frames as f64) * ratio).round() as usize;
    let mut out = vec![0f32; out_frames * channels];
    let half = 16i64;
    // Low-pass cutoff in input-sample units: for downsampling this both
    // prevents aliasing and restores unity gain; weights are normalised per
    // output sample so DC gain is exactly 1.
    let cutoff = ratio.min(1.0);
    for ch in 0..channels {
        for n in 0..out_frames {
            let t = n as f64 / to_sr as f64;
            let src_pos = t * from_sr as f64;
            let i0 = src_pos.floor() as i64;
            let mut acc = 0.0f64;
            let mut wsum = 0.0f64;
            for k in -half..=half {
                let i = i0 + k;
                if i < 0 || i >= frames as i64 {
                    continue;
                }
                let x = src_pos - i as f64;
                let w = sinc(x * cutoff) * cutoff * hann_window(x, half as f64);
                acc += input[ch * frames + i as usize] as f64 * w;
                wsum += w;
            }
            out[ch * out_frames + n] = if wsum.abs() > 1e-9 {
                (acc / wsum) as f32
            } else {
                0.0
            };
        }
    }
    out
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        px.sin() / px
    }
}

fn hann_window(x: f64, half: f64) -> f64 {
    let ax = x.abs();
    if ax >= half {
        0.0
    } else {
        0.5 * (1.0 + (std::f64::consts::PI * x / half).cos())
    }
}
