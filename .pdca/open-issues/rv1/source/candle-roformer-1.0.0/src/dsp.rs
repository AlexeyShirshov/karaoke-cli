use std::f32::consts::PI;

use crate::error::DspError;

pub const STFT_N_FFT_V7: usize = 2048;
pub const STFT_HOP_V7: usize = 441;
pub const STFT_WIN_V7: usize = 2048;

#[derive(Debug, Clone)]
pub struct StftResult {
    pub re: Vec<f32>,
    pub im: Vec<f32>,
    pub frames: usize,
    pub bins: usize,
}

pub fn hann_window_periodic(n: usize) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / n as f32).cos()))
        .collect()
}

fn reflect_pad(input: &[f32], pad: usize) -> Vec<f32> {
    let n = input.len();
    let mut out = Vec::with_capacity(n + 2 * pad);
    for i in 0..pad {
        let r = (pad - i) as isize;
        let reflected = if n <= 1 {
            0usize
        } else {
            let n_i = n as isize;
            let two_n_minus_2 = 2 * n_i - 2;
            let idx = if r < n_i {
                r
            } else {
                (two_n_minus_2 - r).max(0).min(n_i - 1)
            };
            idx as usize
        };
        let reflected = reflected.min(n.saturating_sub(1));
        out.push(input[reflected]);
    }
    out.extend_from_slice(input);
    for i in 0..pad {
        let reflected = if n <= 1 {
            0usize
        } else {
            let n_i = n as isize;
            let r = (n + i) as isize;
            let two_n_minus_2 = 2 * n_i - 2;
            let m = two_n_minus_2 - r;
            let idx = m.max(0).min(n_i - 1);
            idx as usize
        };
        let reflected = reflected.min(n.saturating_sub(1));
        out.push(input[reflected]);
    }
    out
}

pub fn stft(
    input: &[f32],
    n_fft: usize,
    hop: usize,
    win_length: usize,
) -> Result<StftResult, DspError> {
    if n_fft == 0 || n_fft % 2 != 0 {
        return Err(DspError::InvalidParams(
            "n_fft must be positive even".into(),
        ));
    }
    if hop == 0 {
        return Err(DspError::InvalidParams("hop must be > 0".into()));
    }
    if win_length == 0 || win_length > n_fft {
        return Err(DspError::InvalidParams(
            "win_length must be in 1..=n_fft".into(),
        ));
    }
    if input.is_empty() {
        return Err(DspError::InvalidParams("input must not be empty".into()));
    }
    if win_length != n_fft {
        // Keep the spike focused on the v7 profile (win == n_fft).
        return Err(DspError::InvalidParams(
            "this spike requires win_length == n_fft".into(),
        ));
    }

    let pad = n_fft / 2;
    // Reflect padding (torch/NumPy 'reflect') requires input longer than pad;
    // otherwise it cannot be performed faithfully -> typed error, no silent substitution.
    if input.len() <= pad {
        return Err(DspError::InvalidParams(format!(
            "input too short for reflect padding: need length > {} (pad), got {}",
            pad,
            input.len()
        )));
    }
    let padded = reflect_pad(input, pad);
    if padded.len() < n_fft {
        return Err(DspError::InvalidParams("input too short".into()));
    }
    let frames = (padded.len() - n_fft) / hop + 1;
    let bins = n_fft / 2 + 1;

    let window = hann_window_periodic(win_length);
    let mut re = vec![0.0f32; frames * bins];
    let mut im = vec![0.0f32; frames * bins];

    let mut planner = rustfft::FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n_fft);

    for frame in 0..frames {
        let start = frame * hop;
        let mut buf = vec![rustfft::num_complex::Complex::new(0.0f32, 0.0f32); n_fft];
        for i in 0..n_fft {
            buf[i] = rustfft::num_complex::Complex::new(padded[start + i] * window[i], 0.0);
        }
        fft.process(&mut buf);
        for b in 0..bins {
            re[frame * bins + b] = buf[b].re;
            im[frame * bins + b] = buf[b].im;
        }
    }

    Ok(StftResult {
        re,
        im,
        frames,
        bins,
    })
}

/// STFT with librosa 1.x default padding (`pad_mode="constant"`).
///
/// The VR/UVR reference calls `librosa.stft` with its default `pad_mode`, which
/// zero-pads `n_fft / 2` samples on both sides. [`stft`] reflects instead, which
/// is what the RoFormer profiles need; the two are not interchangeable at the
/// signal boundaries.
pub fn stft_zeros(
    input: &[f32],
    n_fft: usize,
    hop: usize,
    win_length: usize,
) -> Result<StftResult, DspError> {
    if n_fft == 0 || n_fft % 2 != 0 {
        return Err(DspError::InvalidParams(
            "n_fft must be positive even".into(),
        ));
    }
    if hop == 0 {
        return Err(DspError::InvalidParams("hop must be > 0".into()));
    }
    if win_length == 0 || win_length > n_fft {
        return Err(DspError::InvalidParams(
            "win_length must be in 1..=n_fft".into(),
        ));
    }
    if input.is_empty() {
        return Err(DspError::InvalidParams("input must not be empty".into()));
    }
    if win_length != n_fft {
        return Err(DspError::InvalidParams(
            "this spike requires win_length == n_fft".into(),
        ));
    }

    let pad = n_fft / 2;
    let mut padded = vec![0.0f32; input.len() + 2 * pad];
    padded[pad..pad + input.len()].copy_from_slice(input);
    if padded.len() < n_fft {
        return Err(DspError::InvalidParams("input too short".into()));
    }
    let frames = (padded.len() - n_fft) / hop + 1;
    let bins = n_fft / 2 + 1;

    let window = hann_window_periodic(win_length);
    let mut re = vec![0.0f32; frames * bins];
    let mut im = vec![0.0f32; frames * bins];

    let mut planner = rustfft::FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n_fft);

    for frame in 0..frames {
        let start = frame * hop;
        let mut buf = vec![rustfft::num_complex::Complex::new(0.0f32, 0.0f32); n_fft];
        for i in 0..n_fft {
            buf[i] = rustfft::num_complex::Complex::new(padded[start + i] * window[i], 0.0);
        }
        fft.process(&mut buf);
        for b in 0..bins {
            re[frame * bins + b] = buf[b].re;
            im[frame * bins + b] = buf[b].im;
        }
    }

    Ok(StftResult {
        re,
        im,
        frames,
        bins,
    })
}
pub fn istft(
    spec: &StftResult,
    n_fft: usize,
    hop: usize,
    win_length: usize,
    length: usize,
) -> Result<Vec<f32>, DspError> {
    if n_fft == 0 || n_fft % 2 != 0 {
        return Err(DspError::InvalidParams(
            "n_fft must be positive even".into(),
        ));
    }
    if hop == 0 {
        return Err(DspError::InvalidParams("hop must be > 0".into()));
    }
    if win_length == 0 || win_length > n_fft {
        return Err(DspError::InvalidParams(
            "win_length must be in 1..=n_fft".into(),
        ));
    }
    if spec.bins != n_fft / 2 + 1 {
        return Err(DspError::ShapeMismatch(format!(
            "expected {} bins, got {}",
            n_fft / 2 + 1,
            spec.bins
        )));
    }
    if win_length != n_fft {
        return Err(DspError::InvalidParams(
            "this spike requires win_length == n_fft".into(),
        ));
    }
    if spec.frames == 0 {
        return Err(DspError::InvalidParams("spec has no frames".into()));
    }

    let window = hann_window_periodic(win_length);
    let mut window_full = vec![0.0f32; n_fft];
    window_full.copy_from_slice(&window);

    let padded_len = (spec.frames - 1) * hop + n_fft;
    let mut out = vec![0.0f32; padded_len];
    let mut wsum = vec![0.0f32; padded_len];

    let mut planner = rustfft::FftPlanner::<f32>::new();
    let ifft = planner.plan_fft_inverse(n_fft);

    for frame in 0..spec.frames {
        let mut buf = vec![rustfft::num_complex::Complex::new(0.0f32, 0.0f32); n_fft];
        for b in 0..spec.bins {
            buf[b] = rustfft::num_complex::Complex::new(
                spec.re[frame * spec.bins + b],
                spec.im[frame * spec.bins + b],
            );
        }
        // Hermitian symmetry for the upper half.
        for k in spec.bins..n_fft {
            let src = n_fft - k;
            buf[k] = rustfft::num_complex::Complex::new(
                spec.re[frame * spec.bins + src],
                -spec.im[frame * spec.bins + src],
            );
        }

        ifft.process(&mut buf);
        let scale = 1.0 / n_fft as f32;
        for i in 0..n_fft {
            let t = buf[i].re * scale;
            let w = window_full[i];
            let pos = frame * hop + i;
            out[pos] += t * w;
            wsum[pos] += w * w;
        }
    }

    for i in 0..padded_len {
        if wsum[i].abs() > 1e-8 {
            out[i] /= wsum[i];
        }
    }

    let pad = n_fft / 2;
    let start = pad.min(padded_len);
    let end = (start + length).min(padded_len);
    let mut result = out[start..end].to_vec();
    if result.len() < length {
        result.resize(length, 0.0);
    }
    Ok(result)
}

// ---------- Level processing helpers (issue #13) ----------

/// Peak absolute amplitude of a buffer.
pub fn peak_abs(samples: &[f32]) -> f32 {
    samples.iter().map(|x| x.abs()).fold(0.0f32, f32::max)
}

/// Peak-normalize a buffer in place to `threshold`. A threshold <= 0 disables
/// scaling; a zero/NaN-free? zero peak leaves the buffer untouched (no divide
/// by zero, no NaN/Inf).
pub fn normalize_peak(samples: &mut [f32], threshold: f32) {
    if !threshold.is_finite() || threshold <= 0.0 {
        return;
    }
    let peak = peak_abs(samples);
    if peak > 0.0 && peak.is_finite() {
        let scale = threshold / peak;
        if scale.is_finite() {
            for x in samples.iter_mut() {
                *x *= scale;
            }
        }
    }
}

/// Scale a buffer **down** so its peak absolute amplitude does not exceed
/// `max_peak`. Unlike [`normalize_peak`] a buffer already at or below the
/// threshold is left untouched: nothing is ever amplified, which mirrors
/// `audio-separator`'s `spec_utils.normalize(.., max_peak, min_peak = 0)`.
/// A non-finite or non-positive `max_peak` disables scaling; a silent buffer
/// is left untouched and never produces `NaN`/`Inf`.
pub fn normalize_max_peak(samples: &mut [f32], max_peak: f32) {
    if !max_peak.is_finite() || max_peak <= 0.0 {
        return;
    }
    let peak = peak_abs(samples);
    if peak > max_peak && peak.is_finite() {
        let scale = max_peak / peak;
        if scale.is_finite() {
            for x in samples.iter_mut() {
                *x *= scale;
            }
        }
    }
}

/// Amplify a quiet input in place: if `0 < peak < threshold`, scale up to the
/// threshold. Threshold <= 0 disables. Zero peak is left untouched.
pub fn amplify_quiet(samples: &mut [f32], threshold: f32) {
    if !threshold.is_finite() || threshold <= 0.0 {
        return;
    }
    let peak = peak_abs(samples);
    if peak > 0.0 && peak < threshold && peak.is_finite() {
        let scale = threshold / peak;
        if scale.is_finite() {
            for x in samples.iter_mut() {
                *x *= scale;
            }
        }
    }
}

/// Spectral inversion of the secondary stem: `secondary = input - primary` in
/// the complex STFT domain, followed by iSTFT. Operates on a single channel.
/// Returns the residual signal trimmed/padded to `input.len()`.
pub fn spectral_invert(
    primary: &[f32],
    input: &[f32],
    n_fft: usize,
    hop: usize,
    win_length: usize,
) -> Result<Vec<f32>, DspError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let n = input.len().min(primary.len());
    let input = &input[..n];
    let primary = &primary[..n];
    // Pad short inputs so STFT reflect-padding is valid.
    let pad = n_fft / 2;
    let mut in_buf = input.to_vec();
    let mut pr_buf = primary.to_vec();
    if in_buf.len() <= pad {
        let need = pad + 1 - in_buf.len();
        in_buf.resize(in_buf.len() + need, 0.0);
        pr_buf.resize(pr_buf.len() + need, 0.0);
    }
    let spec_in = stft(&in_buf, n_fft, hop, win_length)?;
    let spec_v = stft(&pr_buf, n_fft, hop, win_length)?;
    if spec_in.frames != spec_v.frames || spec_in.bins != spec_v.bins {
        return Err(DspError::ShapeMismatch("stft frame mismatch".into()));
    }
    let mut re = spec_in.re.clone();
    let mut im = spec_in.im.clone();
    for i in 0..re.len() {
        re[i] -= spec_v.re[i];
        im[i] -= spec_v.im[i];
    }
    let spec = StftResult {
        re,
        im,
        frames: spec_in.frames,
        bins: spec_in.bins,
    };
    let mut out = istft(&spec, n_fft, hop, win_length, in_buf.len())?;
    out.truncate(n);
    if out.len() < n {
        out.resize(n, 0.0);
    }
    Ok(out)
}

/// Alias expected by some callers.
pub fn invert_spect(
    primary: &[f32],
    input: &[f32],
    n_fft: usize,
    hop: usize,
    win_length: usize,
) -> Result<Vec<f32>, DspError> {
    spectral_invert(primary, input, n_fft, hop, win_length)
}

/// Alias for amplification helper (reference name).
pub fn amplify(samples: &mut [f32], threshold: f32) {
    amplify_quiet(samples, threshold)
}

/// Alias for peak normalization helper (reference name).
pub fn normalize(samples: &mut [f32], threshold: f32) {
    normalize_peak(samples, threshold)
}

#[cfg(test)]
mod level_tests {
    use super::*;

    #[test]
    fn peak_abs_empty_and_zero() {
        assert_eq!(peak_abs(&[]), 0.0);
        assert_eq!(peak_abs(&[0.0, 0.0]), 0.0);
    }

    #[test]
    fn peak_abs_basic() {
        assert!((peak_abs(&[0.5, -0.25, 0.1]) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn normalize_peak_scales_to_threshold() {
        let mut v = vec![0.5f32, -0.25, 0.1];
        normalize_peak(&mut v, 0.9);
        assert!((peak_abs(&v) - 0.9).abs() < 1e-5);
    }

    #[test]
    fn normalize_peak_disabled_thresholds() {
        let orig = vec![0.5f32, -0.25];
        for thr in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
            let mut v = orig.clone();
            normalize_peak(&mut v, thr);
            assert_eq!(v, orig);
        }
    }

    #[test]
    fn normalize_peak_zero_peak_no_nan() {
        let mut v = vec![0.0f32; 4];
        normalize_peak(&mut v, 0.9);
        assert!(v.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn normalize_max_peak_scales_down_only() {
        let mut loud = vec![1.8f32, -1.2];
        normalize_max_peak(&mut loud, 0.9);
        assert!((peak_abs(&loud) - 0.9).abs() < 1e-5);

        let quiet = vec![0.2f32, -0.1];
        let mut v = quiet.clone();
        normalize_max_peak(&mut v, 0.9);
        assert_eq!(v, quiet, "quiet buffers must not be amplified");
    }

    #[test]
    fn normalize_max_peak_disabled_and_zero_peak() {
        let orig = vec![2.0f32, -1.0];
        for thr in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
            let mut v = orig.clone();
            normalize_max_peak(&mut v, thr);
            assert_eq!(v, orig);
        }
        let mut z = vec![0.0f32; 4];
        normalize_max_peak(&mut z, 0.9);
        assert!(z.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn amplify_quiet_scales_quiet_input() {
        let mut v = vec![0.1f32, -0.05];
        amplify_quiet(&mut v, 0.5);
        assert!((peak_abs(&v) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn amplify_quiet_noop_when_loud_or_disabled() {
        let orig = vec![0.8f32, -0.4];
        // peak >= threshold -> no change
        let mut v = orig.clone();
        amplify_quiet(&mut v, 0.5);
        assert_eq!(v, orig);
        // threshold 0 disables
        let mut v2 = vec![0.1f32];
        amplify_quiet(&mut v2, 0.0);
        assert_eq!(v2[0], 0.1);
        // zero peak untouched
        let mut z = vec![0.0f32; 3];
        amplify_quiet(&mut z, 0.5);
        assert!(z.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn spectral_invert_length_and_finite() {
        let n = 128usize;
        let input: Vec<f32> = (0..n).map(|i| (i as f32 * 0.1).sin()).collect();
        let vocals: Vec<f32> = input.iter().map(|x| x * 0.5).collect();
        let res = spectral_invert(&vocals, &input, 64, 16, 64).unwrap();
        assert_eq!(res.len(), n);
        assert!(res.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn spectral_invert_empty_input() {
        let res = spectral_invert(&[], &[], 64, 16, 64).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn invert_spect_alias_works() {
        let n = 64usize;
        let input: Vec<f32> = (0..n).map(|i| (i as f32 * 0.2).sin()).collect();
        let vocals: Vec<f32> = input.iter().map(|x| x * 0.3).collect();
        let a = spectral_invert(&vocals, &input, 32, 8, 32).unwrap();
        let b = invert_spect(&vocals, &input, 32, 8, 32).unwrap();
        assert_eq!(a, b);
    }
}
