//! Public library API: load a local separator checkpoint once, then run
//! separation on in-memory audio buffers.
//!
//! ```no_run
//! use candle_roformer::{AudioBuffer, DeviceChoice, LocalSeparationAssets, Separator};
//!
//! # fn main() -> Result<(), candle_roformer::Error> {
//! let assets = LocalSeparationAssets::new("/models/voc.safetensors", "/models/voc.yaml");
//! let separator = Separator::load(&assets, DeviceChoice::Cpu)?;
//! let audio = candle_roformer::wav::read_audio("/music/song.wav")?;
//! let stems = separator.separate(&audio)?;
//! candle_roformer::wav::write_wav_f32("/out/vocals.wav", &stems.vocals)?;
//! candle_roformer::wav::write_wav_f32("/out/instrumental.wav", &stems.instrumental)?;
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use candle_core::safetensors::Load;
use candle_core::{DType, Device};
use candle_nn::VarBuilder;

use crate::assets::LocalSeparationAssets;
use crate::config::{InferenceConfig, SeparatorConfig};
use crate::device::DeviceChoice;
use crate::error::{AudioError, Error};
use crate::model::{expected_key_shapes, BSRoformer, MelBandRoformer, RoformerModel};
use crate::resample;
use crate::separate;
use crate::wav::AudioBuffer;

/// The stem set produced by the separator.
///
/// # Named stems
///
/// A separator may produce any number of named stems (`num_stems` in the
/// profile). They are all returned here in model order under [`Stems::stems`]
/// as `(name, AudioBuffer)` pairs, and are reachable by name through
/// [`Stems::get`]. The buffer's sample rate, channel count and length match
/// the input buffer exactly.
///
/// # Two-stem convenience
///
/// For the common vocal/instrumental ("rest of the mix") case the separator
/// still exposes [`Stems::vocals`] and [`Stems::instrumental`] as direct
/// fields, so existing two-stem callers keep working unchanged. In that mode
/// `stems` contains exactly the two entries `("vocals", ..)` and
/// `("instrumental", ..)`.
///
/// # Residual semantics
///
/// Two distinct modes exist and are *not* interchangeable:
///
/// * **Residual mode ("rest of the mix").** Used when the model estimates a
///   single target stem (`num_stems == 1`, e.g. a MelBandRoformer vocals
///   model). The model produces the target (`vocals`); the secondary stem
///   (`instrumental`) is the input residual. In **exact residual mode**
///   (level processing disabled: `normalization = 0.0`, `amplification = 0.0`,
///   `invert_spect = false`) this is the exact waveform residual
///   `input - vocals`, so `vocals + instrumental == input` pointwise (within
///   `f32` round-off). With normalization, amplification or `invert_spect`
///   enabled the identity no longer holds in the original input scale (see
///   "Level processing" / "Residual exactness" below).
/// * **Independent multi-stem mode.** Used when the model estimates several
///   stems (`num_stems > 1`, e.g. Demucs / MDX / VR karaoke 4-stem models).
///   Every stem is an independent estimate taken straight from the model's own
///   mask estimator; **no** stem is replaced by `input - stem` and the sum of
///   the stems **need not** (and generally does not) equal the input. Treating
///   the last stem as "the residual" in this mode would be wrong.
///
/// Which mode applies is determined by `num_stems` of the loaded profile, not
/// by a runtime flag: a single-stem profile is residual, a multi-stem profile
/// is independent.

/// # Level processing
///
/// The separator also applies the reference `audio-separator` level processing
/// controlled by [`InferenceConfig`] (`normalization`, `amplification`,
/// `invert_spect`):
///
/// * **Peak normalization** (`normalization`, default `0.9`). Each non-silent
///   buffer (input mix and every output stem) is scaled so its peak absolute
///   amplitude equals the threshold: `x *= threshold / peak(x)` when
///   `peak(x) > 0`. A threshold of `0.0` disables scaling; negative or
///   non-finite thresholds are rejected by [`InferenceConfig::validate`]
///   before reaching this point. A silent buffer (`peak == 0`) is left
///   untouched and never produces `NaN`/`Inf`.
/// * **Amplification** (`amplification`, default `0.0` = off). If the input
///   peak is strictly below the threshold (`0 < peak < threshold`), the input
///   is scaled up to that threshold before separation. A threshold of `0.0`
///   disables amplification.
/// * **Spectral inversion** (`invert_spect`, default `false`). In residual
///   mode the secondary stem is normally the waveform residual
///   `input - vocals`. With `invert_spect = true` it is instead computed by
///   subtracting the primary stem's complex STFT from the input's STFT and
///   inverting (`iSTFT`), which keeps the operation in the spectral domain.
///
/// ## Residual exactness (two modes, not interchangeable)
///
/// * **Exact residual mode** — disable level processing (`normalization = 0.0`,
///   `amplification = 0.0`, `invert_spect = false`). The secondary stem is the
///   exact waveform residual `input - vocals`, so `vocals + instrumental ==
///   input` holds pointwise (within `f32` round-off). Use this mode when an
///   exact, lossless reconstruction of the input from the two stems is
///   required.
/// * **Normalized mode** (reference default, `normalization = 0.9`) — the
///   input mix and each output stem are peak-normalized independently. The
///   pointwise identity `vocals + instrumental == input` no longer holds in the
///   original input scale (each stem is rescaled to its own peak), and with
///   `invert_spect = true` the secondary stem is a spectral residual rather
///   than a waveform one. Use this mode to match `audio-separator` output
///   levels.
#[derive(Debug, Clone)]
pub struct Stems {
    /// Two-stem convenience: the vocal (target) stem. In residual mode this
    /// is the model estimate; in independent mode it is the stem named
    /// `"vocals"` if present, otherwise the first stem.
    pub vocals: AudioBuffer,
    /// Two-stem convenience: the instrumental stem. In residual mode this is
    /// the input residual (exact `input - vocals` only in exact residual mode,
    /// i.e. level processing disabled; see "Residual exactness"); in
    /// independent mode it is the stem named `"instrumental"` if present,
    /// otherwise the second stem (or the first when only one exists).
    pub instrumental: AudioBuffer,
    /// All stems returned by the model, in model order, as `(name, buffer)`
    /// pairs. In residual mode this is `[("vocals", ..), ("instrumental", ..)]`;
    /// in independent mode it has one entry per `num_stems`.
    pub stems: Vec<(String, AudioBuffer)>,
}

impl Stems {
    /// Look up a stem by name.
    pub fn get(&self, name: &str) -> Option<&AudioBuffer> {
        self.stems.iter().find(|(n, _)| n == name).map(|(_, b)| b)
    }

    /// Look up a stem by name (mutable).
    pub fn get_mut(&mut self, name: &str) -> Option<&mut AudioBuffer> {
        self.stems
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b)
    }

    /// Number of named stems.
    pub fn len(&self) -> usize {
        self.stems.len()
    }

    /// Whether the stem set is empty.
    pub fn is_empty(&self) -> bool {
        self.stems.is_empty()
    }

    /// Names of the stems, in model order.
    pub fn names(&self) -> Vec<&str> {
        self.stems.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Iterate over `(name, buffer)` pairs.
    pub fn iter(&self) -> std::slice::Iter<'_, (String, AudioBuffer)> {
        self.stems.iter()
    }
}

/// A loaded RoFormer separator (MelBand or BS-RoFormer) bound to a device.
///
/// The family is taken from the config (`family:` key, or inferred from
/// `freqs_per_bands` / `num_bands`). Loading validates the checkpoint against
/// the config (full key set, shapes) before allocating the model, so a
/// mismatched pair fails with a typed error instead of producing garbage.
pub struct Separator {
    model: Box<dyn RoformerModel>,
    /// The Demucs v4 path has its own segment/overlap pipeline
    /// (`model::demucs_pipeline`), so a Demucs separator is not a
    /// [`RoformerModel`]; when present, [`Separator::separate`] delegates to it.
    demucs: Option<crate::model::DemucsSeparator>,
    config: SeparatorConfig,
    inference: InferenceConfig,
    device: DeviceChoice,
}

/// Placeholder model slot for pipelines that are not RoFormer families.
///
/// A Demucs separator owns its own forward pass (`model::demucs_pipeline`), so
/// [`Separator::separate`] dispatches to it before the RoFormer path is reached.
/// Every method below is therefore unreachable for that configuration and
/// reports a typed error instead of panicking.
struct NoRoformerModel {
    device: Device,
}

impl RoformerModel for NoRoformerModel {
    fn forward(&self, _raw_audio: &candle_core::Tensor) -> candle_core::Result<candle_core::Tensor> {
        Err(candle_core::Error::Msg(
            "this separator is not a RoFormer model".into(),
        ))
    }
    fn audio_channels(&self) -> usize {
        2
    }
    fn num_stems(&self) -> usize {
        4
    }
    fn hop(&self) -> usize {
        1024
    }
    fn device(&self) -> &Device {
        &self.device
    }
}

fn default_stem_names(n: usize) -> Vec<String> {    match n {
        1 => vec!["vocals".to_string()],
        4 => vec!["drums", "bass", "other", "vocals"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        _ => (0..n).map(|i| format!("stem_{}", i)).collect(),
    }
}

fn named_or_index(stems: &[(String, AudioBuffer)], name: &str, idx: usize) -> AudioBuffer {
    if let Some((_, b)) = stems.iter().find(|(n, _)| n == name) {
        return b.clone();
    }
    if let Some((_, b)) = stems.get(idx) {
        return b.clone();
    }
    if let Some((_, b)) = stems.first() {
        return b.clone();
    }
    AudioBuffer {
        sample_rate: 44100,
        channels: 1,
        planar: Vec::new(),
    }
}

impl Separator {
    /// Load a local checkpoint (`safetensors`) and its YAML profile.
    pub fn load(assets: &LocalSeparationAssets, device: DeviceChoice) -> Result<Self, Error> {        assets.validate()?;
        let cfg = SeparatorConfig::from_yaml_file(&assets.config)?;
        cfg.validate()?;
        let inference = InferenceConfig::from_yaml_file(&assets.config)?;
        inference.validate()?;
        let dev = device.resolve()?;
        strict_check(
            assets.checkpoint.to_str().ok_or_else(|| {
                Error::Audio(AudioError::Io("checkpoint path is not UTF-8".into()))
            })?,
            &cfg,
            &dev,
        )?;
        let path = assets
            .checkpoint
            .to_str()
            .ok_or_else(|| Error::Audio(AudioError::Io("checkpoint path is not UTF-8".into())))?;
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[path], DType::F32, &dev) }
            .map_err(Error::Candle)?;
        let model: Box<dyn RoformerModel> = match cfg.family() {
            crate::config::ModelFamily::MelBandRoformer => {
                Box::new(MelBandRoformer::new(&cfg, vb).map_err(Error::Candle)?)
            }
            crate::config::ModelFamily::BSRoformer => {
                Box::new(BSRoformer::new(&cfg, vb).map_err(Error::Candle)?)
            }
            // MDX-Net is an ONNX graph loaded and driven by `crate::mdx`; it has
            // no safetensors key schema and never goes through `Separator::load`.
            crate::config::ModelFamily::MdxNet => {
                return Err(Error::Config(crate::error::ConfigError::Invalid(
                    "MDX-Net ONNX checkpoints are loaded by mdx::MdxSeparator, not Separator::load"
                        .into(),
                )));
            }
            // VR Arch drives its own multi-band STFT/ROI pipeline; load it with
            // `crate::model::vr_pipeline::VrSeparator::load`.
            crate::config::ModelFamily::VrArch => {
                return Err(Error::Config(crate::error::ConfigError::Invalid(
                    "VR Arch checkpoints are loaded by vr_pipeline::VrSeparator, not Separator::load"
                        .into(),
                )));
            }
            // Demucs v4 has no RoFormer YAML profile; load it with
            // `Separator::load_demucs`.
            crate::config::ModelFamily::Demucs => {
                return Err(Error::Config(crate::error::ConfigError::Invalid(
                    "Demucs checkpoints are loaded by Separator::load_demucs, not Separator::load"
                        .into(),
                )));
            }
        };
        Ok(Self {
            model,
            demucs: None,
            config: cfg,
            inference,
            device,
        })
    }

    /// Load a Demucs v4 (HTDemucs) `safetensors` artifact produced by
    /// `tools/convert_demucs.py`.
    ///
    /// Every tensor name and shape is validated against the pinned
    /// [`crate::model::demucs::DemucsConfig`] schema before the model is built;
    /// a mismatch is a typed error, never a silent skip. Separation then follows
    /// the reference `apply_model(shifts=0, split=True, overlap=0.25)` path and
    /// returns four stems named `drums`, `bass`, `other`, `vocals`.
    pub fn load_demucs(
        checkpoint: impl AsRef<Path>,
        device: DeviceChoice,
    ) -> Result<Self, Error> {
        let demucs = crate::model::DemucsSeparator::load(checkpoint, device)?;
        let config = SeparatorConfig::demucs();
        let inference = InferenceConfig::demucs();
        let dev = device.resolve()?;
        let model: Box<dyn RoformerModel> = Box::new(NoRoformerModel { device: dev });
        Ok(Self {
            model,
            demucs: Some(demucs),
            config,
            inference,
            device,
        })
    }

    /// Load a checkpoint with the config resolved as `<checkpoint>.yaml`.
    pub fn load_with_sidecar_config(
        checkpoint: impl AsRef<Path>,
        device: DeviceChoice,
    ) -> Result<Self, Error> {
        let checkpoint = checkpoint.as_ref();
        let assets = LocalSeparationAssets::new(checkpoint, checkpoint.with_extension("yaml"));
        Self::load(&assets, device)
    }

    pub fn config(&self) -> &SeparatorConfig {
        &self.config
    }

    pub fn inference_config(&self) -> &InferenceConfig {
        &self.inference
    }

    pub fn device_choice(&self) -> DeviceChoice {
        self.device
    }

    /// Separate an in-memory buffer. The input is resampled to the model's
    /// sample rate, processed in chunked overlap-add, then returned at the
    /// input's sample rate with the input's channel layout and exact length.
    ///
    /// The number of stems returned follows the loaded profile:
    /// - `num_stems == 1`: residual mode — `vocals` is the model estimate and
    ///   `instrumental` is the input residual (`input - vocals` exactly only in
    ///   exact residual mode; see "Residual exactness"); `stems` holds
    ///   `[("vocals", ..), ("instrumental", ..)]`.
    /// - `num_stems > 1`: independent mode — every stem comes from the model
    ///   and `stems` holds one named entry per `num_stems` (no residual).
    pub fn separate(&self, audio: &AudioBuffer) -> Result<Stems, Error> {
        audio.validate()?;
        if let Some(demucs) = &self.demucs {
            return demucs.separate(audio);
        }
        let channels = audio.channels as usize;
        let input_sr = audio.sample_rate;
        let n = audio.planar.len() / channels;

        // mono is duplicated to stereo for the model; stems are downmixed back
        let model_channels = if channels == 1 { 2 } else { channels };
        let model_input = if channels == 1 {
            let mut v = Vec::with_capacity(audio.planar.len() * 2);
            v.extend_from_slice(&audio.planar);
            v.extend_from_slice(&audio.planar);
            v
        } else {
            audio.planar.clone()
        };

        let model_sr = self.config.sample_rate as u32;
        let resampled = resample::resample_planar(&model_input, model_channels, input_sr, model_sr);
        // Processed (amplified/normalized) model-rate input, mirroring what
        // separate_all_with_config does internally, for spectral inversion.
        let mut processed_model_input = resampled.clone();
        if self.inference.amplification > 0.0 {
            crate::dsp::amplify_quiet(&mut processed_model_input, self.inference.amplification);
        }
        if self.inference.normalization > 0.0 {
            crate::dsp::normalize_peak(&mut processed_model_input, self.inference.normalization);
        }
        let stem_outputs = separate::separate_all_with_config(
            self.model.as_ref(),
            &resampled,
            model_channels,
            &self.inference,
        )?;

        // Convert each model stem back to the input sample rate / channel layout.
        let mut stem_audios: Vec<AudioBuffer> = Vec::with_capacity(stem_outputs.len());
        for out in &stem_outputs {
            let orig_sr = if channels == 1 {
                let m = out.len() / 2;
                let mut mono = vec![0f32; m];
                for i in 0..m {
                    mono[i] = (out[i] + out[m + i]) * 0.5;
                }
                mono
            } else {
                out.clone()
            };
            let resampled_back = resample::resample_planar(&orig_sr, channels, model_sr, input_sr);
            let mut final_buf = vec![0f32; channels * n];
            let vframes = resampled_back.len() / channels;
            for ch in 0..channels {
                for i in 0..n {
                    let src = ch * vframes + i;
                    if src < resampled_back.len() {
                        final_buf[ch * n + i] = resampled_back[src];
                    }
                }
            }
            stem_audios.push(AudioBuffer {
                sample_rate: input_sr,
                channels: audio.channels,
                planar: final_buf,
            });
        }

        let residual_mode = self.model.num_stems() <= 1;

        // Unnormalized (raw) primary stem in model rate and input rate, used for
        // the unified secondary-stem formula N(p - y): the secondary stem is
        // computed from the pre-scaled input p and the *unnormalized* primary
        // output y, then both stems are peak-normalized independently.
        let mut raw_model_vocals: Vec<f32> = Vec::new();
        let mut raw_vocals_input: Vec<f32> = vec![0f32; channels * n];
        if residual_mode {
            let mut cfg_raw = self.inference.clone();
            cfg_raw.normalization = 0.0;
            cfg_raw.amplification = 0.0;
            if let Ok(outs) = separate::separate_all_with_config(
                self.model.as_ref(),
                &processed_model_input,
                model_channels,
                &cfg_raw,
            ) {
                if let Some(first) = outs.into_iter().next() {
                    raw_model_vocals = first;
                }
            }
            if !raw_model_vocals.is_empty() {
                let orig_sr_buf: Vec<f32> = if channels == 1 {
                    let m = raw_model_vocals.len() / 2;
                    let mut mono = vec![0f32; m];
                    for i in 0..m {
                        mono[i] = (raw_model_vocals[i] + raw_model_vocals[m + i]) * 0.5;
                    }
                    mono
                } else {
                    raw_model_vocals.clone()
                };
                let resampled_back =
                    resample::resample_planar(&orig_sr_buf, channels, model_sr, input_sr);
                let vframes = resampled_back.len() / channels.max(1);
                for ch in 0..channels {
                    for i in 0..n {
                        let src = ch * vframes + i;
                        if src < resampled_back.len() {
                            raw_vocals_input[ch * n + i] = resampled_back[src];
                        }
                    }
                }
            }
        }

        let mut stems_vec: Vec<(String, AudioBuffer)> = Vec::new();
        let (vocals, instrumental) = if residual_mode {
            let mut vocals_audio = stem_audios
                .into_iter()
                .next()
                .unwrap_or_else(|| AudioBuffer {
                    sample_rate: input_sr,
                    channels: audio.channels,
                    planar: vec![0f32; channels * n],
                });
            if self.inference.normalization > 0.0 {
                crate::dsp::normalize_peak(&mut vocals_audio.planar, self.inference.normalization);
            }
            // Reference input for the residual, possibly level-processed.
            let mut input_ref = audio.planar.clone();
            if self.inference.amplification > 0.0 {
                crate::dsp::amplify_quiet(&mut input_ref, self.inference.amplification);
            }
            if self.inference.normalization > 0.0 {
                crate::dsp::normalize_peak(&mut input_ref, self.inference.normalization);
            }

            let instrumental_planar = if self.inference.invert_spect {
                // Spectral inversion: secondary stem via STFT subtraction.
                // Work in model-rate domain then resample.
                // Fallback: compute per-channel on input-rate buffers using
                // model STFT params is not sample-rate aligned; instead compute
                // at model rate from processed_model_input and the model vocals.
                let vocals_src = if raw_model_vocals.is_empty() {
                    stem_outputs.first().cloned().unwrap_or_default()
                } else {
                    raw_model_vocals.clone()
                };
                let mut model_vocals = vocals_src;
                // model vocals already normalized by separate_all; ensure length
                let mv_frames = model_vocals.len() / model_channels;
                let in_frames = processed_model_input.len() / model_channels;
                let frames = mv_frames.min(in_frames);
                let mut instr_model = vec![0f32; model_channels * frames];
                // We'll resample vocals_model? It is model-rate; keep.
                for ch in 0..model_channels {
                    let in_ch: Vec<f32> = (0..frames)
                        .map(|i| processed_model_input[ch * in_frames + i])
                        .collect();
                    let voc_ch: Vec<f32> = (0..frames)
                        .map(|i| model_vocals[ch * mv_frames + i])
                        .collect();
                    let res = crate::dsp::spectral_invert(
                        &voc_ch,
                        &in_ch,
                        self.config.stft_n_fft,
                        self.config.stft_hop_length,
                        self.config.stft_win_length,
                    )
                    .unwrap_or_else(|_| {
                        in_ch
                            .iter()
                            .zip(voc_ch.iter())
                            .map(|(a, b)| a - b)
                            .collect()
                    });
                    for i in 0..frames {
                        instr_model[ch * frames + i] = res[i];
                    }
                }
                // resample model-rate instrumental to input rate / channel layout
                let mut out_final = vec![0f32; channels * n];
                // Rebuild instr_model full length frames
                let mut full = vec![0f32; model_channels * frames];
                full.copy_from_slice(&instr_model);
                // downmix to channels
                let mut mono_model: Vec<f32> = if channels == 1 {
                    let mut v = vec![0f32; frames];
                    for i in 0..frames {
                        v[i] = (full[i] + full[frames + i]) * 0.5;
                    }
                    v
                } else {
                    full.clone()
                };
                // resample to input sr
                let rs = resample::resample_planar(&mono_model, channels, model_sr, input_sr);
                let vframes = rs.len() / channels;
                for ch in 0..channels {
                    for i in 0..n {
                        let src = ch * vframes + i;
                        if src < rs.len() {
                            out_final[ch * n + i] = rs[src];
                        }
                    }
                }
                if self.inference.normalization > 0.0 {
                    crate::dsp::normalize_peak(&mut out_final, self.inference.normalization);
                }
                out_final
            } else {
                let mut instr_final = vec![0f32; channels * n];
                for i in 0..channels * n {
                    instr_final[i] = input_ref[i] - raw_vocals_input[i];
                }
                if self.inference.normalization > 0.0 {
                    crate::dsp::normalize_peak(&mut instr_final, self.inference.normalization);
                }
                instr_final
            };
            let instrumental_audio = AudioBuffer {
                sample_rate: input_sr,
                channels: audio.channels,
                planar: instrumental_planar,
            };
            stems_vec.push(("vocals".to_string(), vocals_audio.clone()));
            stems_vec.push(("instrumental".to_string(), instrumental_audio.clone()));
            (vocals_audio, instrumental_audio)
        } else {
            let names = default_stem_names(stem_audios.len());
            for (name, mut audio_buf) in names.iter().zip(stem_audios.into_iter()) {
                if self.inference.normalization > 0.0 {
                    crate::dsp::normalize_peak(&mut audio_buf.planar, self.inference.normalization);
                }
                stems_vec.push((name.clone(), audio_buf));
            }
            // Two-stem convenience: prefer named stems, else fall back to
            // the first / second stem.
            let vocals_audio = named_or_index(&stems_vec, "vocals", 0);
            let instrumental_audio = named_or_index(&stems_vec, "instrumental", 1);
            (vocals_audio, instrumental_audio)
        };

        Ok(Stems {
            vocals,
            instrumental,
            stems: stems_vec,
        })
    }
}

/// Verify the checkpoint against the config without materialising a model.
fn strict_check(model_path: &str, cfg: &SeparatorConfig, dev: &Device) -> Result<(), Error> {
    use std::collections::HashSet;

    let expected = expected_key_shapes(cfg);
    let expected_names: HashSet<String> = expected.iter().map(|(n, _)| n.clone()).collect();

    let ms = unsafe { candle_core::safetensors::MmapedSafetensors::multi(&[model_path]) }
        .map_err(Error::Candle)?;
    let actual_names: HashSet<String> = ms.tensors().iter().map(|(n, _)| n.clone()).collect();

    let mut missing: Vec<String> = expected_names.difference(&actual_names).cloned().collect();
    let mut extra: Vec<String> = actual_names.difference(&expected_names).cloned().collect();
    missing.sort();
    extra.sort();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(Error::Audio(AudioError::UnsupportedFormat(format!(
            "checkpoint key mismatch: missing {:?}, extra {:?}",
            missing, extra
        ))));
    }

    let mut shape_errors: Vec<String> = Vec::new();
    for (name, shape) in &expected {
        let tv = ms.get(name).map_err(Error::Candle)?;
        let t = tv.load(dev).map_err(Error::Candle)?;
        if t.dims() != shape.as_slice() {
            shape_errors.push(format!(
                "{}: file {:?} vs expected {:?}",
                name,
                t.dims(),
                shape
            ));
        }
    }
    if !shape_errors.is_empty() {
        return Err(Error::Audio(AudioError::UnsupportedFormat(format!(
            "checkpoint shape mismatch: {:?}",
            shape_errors
        ))));
    }
    Ok(())
}

struct FrameBuf {
    channels: usize,
    data: Vec<f32>,
    base: usize,
}

impl FrameBuf {
    fn frames(&self) -> usize {
        self.data.len() / self.channels.max(1)
    }
    fn available(&self) -> usize {
        self.base + self.frames()
    }
    fn append(&mut self, planar: &[f32]) {
        let ch = self.channels.max(1);
        let new_frames = planar.len() / ch;
        if new_frames == 0 {
            return;
        }
        let old_frames = self.frames();
        let mut merged = vec![0f32; ch * (old_frames + new_frames)];
        for c in 0..ch {
            for i in 0..old_frames {
                merged[c * (old_frames + new_frames) + i] = self.data[c * old_frames + i];
            }
            for i in 0..new_frames {
                merged[c * (old_frames + new_frames) + (old_frames + i)] =
                    planar[c * new_frames + i];
            }
        }
        self.data = merged;
    }
    fn slice(&self, global_start: usize, n: usize) -> Vec<f32> {
        let ch = self.channels.max(1);
        let frames = self.frames();
        let off = global_start.saturating_sub(self.base);
        let n = n.min(frames.saturating_sub(off));
        let mut out = vec![0f32; ch * n];
        for c in 0..ch {
            for i in 0..n {
                out[c * n + i] = self.data[c * frames + (off + i)];
            }
        }
        out
    }
    fn drop_before(&mut self, idx: usize) {
        let ch = self.channels.max(1);
        if idx <= self.base {
            return;
        }
        let frames = self.frames();
        let drop = (idx - self.base).min(frames);
        let keep = frames - drop;
        let mut new = vec![0f32; ch * keep];
        for c in 0..ch {
            for i in 0..keep {
                new[c * keep + i] = self.data[c * frames + (drop + i)];
            }
        }
        self.data = new;
        self.base = self.base + drop;
    }
}

impl Separator {
    pub fn separate_file(
        &self,
        input_path: impl AsRef<std::path::Path>,
        vocals_path: impl AsRef<std::path::Path>,
        instrumental_path: impl AsRef<std::path::Path>,
        chunk_duration: f64,
    ) -> Result<(), crate::error::Error> {
        use std::fs::File;
        use std::io::{Read, Write};

        let input_path = input_path.as_ref();
        let vocals_path = vocals_path.as_ref();
        let instrumental_path = instrumental_path.as_ref();

        let mut reader = crate::wav::AudioStreamReader::open(input_path)?;
        let input_sr = reader.sample_rate();
        let channels = reader.channels() as usize;
        let model_sr = self.config.sample_rate as u32;
        let residual_mode = self.model.num_stems() <= 1;

        // Fallback for non-matching sample rates, non-residual (multi-stem)
        // profiles, or spectral inversion: decode the whole input correctly and
        // run the in-memory separator. Correct but not bounded-memory; the
        // bounded streaming path below requires input_sr == model_sr, a residual
        // profile, and waveform (non-spectral) residual.
        if input_sr != model_sr || !residual_mode || self.inference.invert_spect {
            let audio = crate::wav::read_audio(input_path)?;
            let stems = self.separate(&audio)?;
            crate::wav::write_wav_f32(vocals_path, &stems.vocals)?;
            crate::wav::write_wav_f32(instrumental_path, &stems.instrumental)?;
            return Ok(());
        }

        let inference = &self.inference;
        let norm_thr = inference.normalization;
        let amp_thr = inference.amplification;

        let model_channels = if channels == 1 { 2 } else { channels };
        let chunk = ((chunk_duration * model_sr as f64).round() as usize).max(1);
        let step = inference.step_for(chunk).max(1);
        let fade = inference.fade_for(chunk).max(1).min(chunk);

        let mut fade_w = vec![1f32; chunk];
        for (t, w) in fade_w.iter_mut().enumerate() {
            let mut fi = 1.0f32;
            if t < fade {
                fi = (t as f32 + 1.0) / fade as f32;
            }
            let mut fo = 1.0f32;
            if t >= chunk - fade {
                fo = (chunk - t) as f32 / fade as f32;
            }
            *w = fi.min(fo).max(1.0 / fade as f32);
        }

        // --- Pass 0: global input peak (bounded, streaming) ---
        let mut peak0 = 0f32;
        let mut total_frames: usize = 0;
        loop {
            let block = reader.read_frames(65536)?;
            if block.is_empty() {
                break;
            }
            for &s in &block {
                if s.abs() > peak0 {
                    peak0 = s.abs();
                }
            }
            total_frames += block.len() / channels;
        }
        if total_frames == 0 {
            return Err(crate::error::Error::Audio(
                crate::error::AudioError::EmptyInput("empty input".into()),
            ));
        }

        let mut amp_scale = 1f32;
        if amp_thr > 0.0 && peak0 > 0.0 && peak0 < amp_thr {
            amp_scale = amp_thr / peak0;
        }
        let peak_after_amp = peak0 * amp_scale;
        let mut norm_scale = 1f32;
        if norm_thr > 0.0 && peak_after_amp > 0.0 {
            norm_scale = norm_thr / peak_after_amp;
        }
        let total_scale = amp_scale * norm_scale;

        // Temp files for raw (scaled-space) stem outputs, frame-major interleaved.
        let tmpdir = std::env::temp_dir();
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        let uid = CTR.fetch_add(1, Ordering::Relaxed);
        let vraw_path = tmpdir.join(format!("cr_vraw_{}_{}.bin", std::process::id(), uid));
        let iraw_path = tmpdir.join(format!("cr_iraw_{}_{}.bin", std::process::id(), uid));

        let mut vraw_file = File::create(&vraw_path)
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;
        let mut iraw_file = File::create(&iraw_path)
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;

        let mut peak_v: f32 = 0.0;
        let mut peak_i: f32 = 0.0;

        // Window schedule for the streaming pass: tail-aligned, the same rule
        // as the in-memory path (`separate::chunk_starts`), so both paths
        // produce the same stems and the final window always has full context.
        let mut starts: Vec<usize> = Vec::new();
        {
            let mut s = 0usize;
            while s < total_frames {
                if s + chunk >= total_frames {
                    let tail = total_frames.saturating_sub(chunk);
                    if starts.last() != Some(&tail) {
                        starts.push(tail);
                    }
                    break;
                }
                starts.push(s);
                s += step;
            }
        }

        // --- Pass 1: streaming OLA inference ---
        let mut reader = crate::wav::AudioStreamReader::open(input_path)?;
        let capacity = chunk + step + 4;
        let mut acc_v = vec![0f32; model_channels * capacity];
        let mut acc_w = vec![0f32; capacity];
        let mut frontier: usize = 0;
        let mut current_max: usize = 0;
        let mut idx: usize = 0;
        let mut eof = false;

        let mut orig_buf = FrameBuf {
            channels,
            data: Vec::new(),
            base: 0,
        };
        let mut model_buf = FrameBuf {
            channels: model_channels,
            data: Vec::new(),
            base: 0,
        };

        loop {
            if idx >= starts.len() {
                break;
            }
            let start = starts[idx];

            while !eof && model_buf.available() < start + chunk {
                let need = (start + chunk).saturating_sub(model_buf.available()).max(1);
                let block = reader.read_frames(need.min(65536))?;
                if block.is_empty() {
                    eof = true;
                    break;
                }
                let frames = block.len() / channels;
                if frames == 0 {
                    eof = true;
                    break;
                }
                // scale input block
                let mut scaled = block.clone();
                for s in &mut scaled {
                    *s *= total_scale;
                }
                orig_buf.append(&scaled);
                let mut model_block = vec![0f32; model_channels * frames];
                for mc in 0..model_channels {
                    let oc = if channels == 1 { 0 } else { mc };
                    for f in 0..frames {
                        model_block[mc * frames + f] = scaled[oc * frames + f];
                    }
                }
                model_buf.append(&model_block);
            }

            if model_buf.available() <= start {
                break;
            }

            let seg_len = (model_buf.available() - start).min(chunk);
            if seg_len == 0 {
                break;
            }

            let mut pcm = vec![0f32; model_channels * chunk];
            let seg = model_buf.slice(start, seg_len);
            for mc in 0..model_channels {
                for i in 0..seg_len {
                    pcm[mc * chunk + i] = seg[mc * seg_len + i];
                }
            }
            let model_out = crate::separate::infer_chunk(
                self.model.as_ref(),
                &self.inference,
                &pcm,
                model_channels,
                chunk,
            )?;

            for i in 0..seg_len {
                let t = start + i;
                let local = t % capacity;
                if local >= capacity {
                    continue;
                }
                for mc in 0..model_channels {
                    acc_v[mc * capacity + local] += model_out[mc * chunk + i] * fade_w[i];
                }
                acc_w[local] += fade_w[i];
            }
            current_max = current_max.max(start + seg_len);

            let mut up_to = starts.get(idx + 1).copied().unwrap_or(total_frames);
            if eof {
                up_to = up_to.min(total_frames);
            }
            if up_to > current_max {
                up_to = current_max;
            }
            if up_to > frontier {
                for t in frontier..up_to {
                    let local = t % capacity;
                    let w = acc_w[local];
                    let mut vocals_model = vec![0f32; model_channels];
                    for mc in 0..model_channels {
                        vocals_model[mc] = if w > 1e-8 {
                            acc_v[mc * capacity + local] / w
                        } else {
                            0.0
                        };
                    }
                    let mut vocals_orig = vec![0f32; channels];
                    if channels == 1 {
                        vocals_orig[0] = (vocals_model[0] + vocals_model[1]) * 0.5;
                    } else {
                        for c in 0..channels {
                            vocals_orig[c] = vocals_model[c];
                        }
                    }
                    let orig_frame = orig_buf.slice(t, 1);
                    let mut instr_orig = vec![0f32; channels];
                    for c in 0..channels {
                        instr_orig[c] = orig_frame[c * 1 + 0] - vocals_orig[c];
                    }
                    // update peaks
                    for c in 0..channels {
                        if vocals_orig[c].abs() > peak_v {
                            peak_v = vocals_orig[c].abs();
                        }
                        if instr_orig[c].abs() > peak_i {
                            peak_i = instr_orig[c].abs();
                        }
                    }
                    // write raw frame-major interleaved to temp files
                    let mut vb = Vec::with_capacity(channels);
                    let mut ib = Vec::with_capacity(channels);
                    for c in 0..channels {
                        vb.push(vocals_orig[c]);
                        ib.push(instr_orig[c]);
                    }
                    // write channels values (frame-major: all channels of this frame)
                    let mut vbytes = Vec::with_capacity(channels * 4);
                    let mut ibytes = Vec::with_capacity(channels * 4);
                    for c in 0..channels {
                        vbytes.extend_from_slice(&vb[c].to_le_bytes());
                        ibytes.extend_from_slice(&ib[c].to_le_bytes());
                    }
                    vraw_file.write_all(&vbytes).map_err(|e| {
                        crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
                    })?;
                    iraw_file.write_all(&ibytes).map_err(|e| {
                        crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
                    })?;
                }
                for t in frontier..up_to {
                    let local = t % capacity;
                    for mc in 0..model_channels {
                        acc_v[mc * capacity + local] = 0.0;
                    }
                    acc_w[local] = 0.0;
                }
                frontier = up_to;
            }

            model_buf.drop_before(frontier);
            orig_buf.drop_before(frontier);

            idx += 1;

            if eof && start + chunk >= total_frames {
                let mut up_to = total_frames;
                if up_to > current_max {
                    up_to = current_max;
                }
                if up_to > frontier {
                    for t in frontier..up_to {
                        let local = t % capacity;
                        let w = acc_w[local];
                        let mut vocals_model = vec![0f32; model_channels];
                        for mc in 0..model_channels {
                            vocals_model[mc] = if w > 1e-8 {
                                acc_v[mc * capacity + local] / w
                            } else {
                                0.0
                            };
                        }
                        let mut vocals_orig = vec![0f32; channels];
                        if channels == 1 {
                            vocals_orig[0] = (vocals_model[0] + vocals_model[1]) * 0.5;
                        } else {
                            for c in 0..channels {
                                vocals_orig[c] = vocals_model[c];
                            }
                        }
                        let orig_frame = orig_buf.slice(t, 1);
                        let mut instr_orig = vec![0f32; channels];
                        for c in 0..channels {
                            instr_orig[c] = orig_frame[c * 1 + 0] - vocals_orig[c];
                        }
                        for c in 0..channels {
                            if vocals_orig[c].abs() > peak_v {
                                peak_v = vocals_orig[c].abs();
                            }
                            if instr_orig[c].abs() > peak_i {
                                peak_i = instr_orig[c].abs();
                            }
                        }
                        let mut vb = Vec::with_capacity(channels);
                        let mut ib = Vec::with_capacity(channels);
                        for c in 0..channels {
                            vb.push(vocals_orig[c]);
                            ib.push(instr_orig[c]);
                        }
                        let mut vbytes = Vec::with_capacity(channels * 4);
                        let mut ibytes = Vec::with_capacity(channels * 4);
                        for c in 0..channels {
                            vbytes.extend_from_slice(&vb[c].to_le_bytes());
                            ibytes.extend_from_slice(&ib[c].to_le_bytes());
                        }
                        vraw_file.write_all(&vbytes).map_err(|e| {
                            crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
                        })?;
                        iraw_file.write_all(&ibytes).map_err(|e| {
                            crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
                        })?;
                    }
                    for t in frontier..up_to {
                        let local = t % capacity;
                        for mc in 0..model_channels {
                            acc_v[mc * capacity + local] = 0.0;
                        }
                        acc_w[local] = 0.0;
                    }
                    frontier = up_to;
                }
                break;
            }
        }

        // finalize temp files
        vraw_file
            .flush()
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;
        iraw_file
            .flush()
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;
        drop(vraw_file);
        drop(iraw_file);

        // --- Pass 2: normalize (if needed) and write destination WAVs ---
        let scale_v = if norm_thr > 0.0 && peak_v > 0.0 {
            norm_thr / peak_v
        } else {
            1.0f32
        };
        let scale_i = if norm_thr > 0.0 && peak_i > 0.0 {
            norm_thr / peak_i
        } else {
            1.0f32
        };

        let mut vw =
            crate::wav::StreamingWavWriter::create(vocals_path, input_sr, channels as u16)?;
        let mut iw =
            crate::wav::StreamingWavWriter::create(instrumental_path, input_sr, channels as u16)?;

        let mut vf = File::open(&vraw_path)
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;
        let mut inf = File::open(&iraw_path)
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;

        let chunk_frames = 4096usize;
        let mut frame_idx = 0usize;
        while frame_idx < total_frames {
            let take = chunk_frames.min(total_frames - frame_idx);
            // read take frames (frame-major interleaved) for vocals
            let mut vbuf = vec![0f32; take * channels];
            let mut ibuf = vec![0f32; take * channels];
            // read exact bytes
            let mut vbytes = vec![0u8; take * channels * 4];
            let mut ibytes = vec![0u8; take * channels * 4];
            vf.read_exact(&mut vbytes).map_err(|e| {
                crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
            })?;
            inf.read_exact(&mut ibytes).map_err(|e| {
                crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
            })?;
            for k in 0..(take * channels) {
                vbuf[k] = f32::from_le_bytes(vbytes[k * 4..(k + 1) * 4].try_into().unwrap());
                ibuf[k] = f32::from_le_bytes(ibytes[k * 4..(k + 1) * 4].try_into().unwrap());
            }
            // normalize
            for k in 0..(take * channels) {
                vbuf[k] *= scale_v;
                ibuf[k] *= scale_i;
            }
            // convert frame-major -> planar
            let mut vplanar = vec![0f32; channels * take];
            let mut iplanar = vec![0f32; channels * take];
            for fr in 0..take {
                for c in 0..channels {
                    vplanar[c * take + fr] = vbuf[fr * channels + c];
                    iplanar[c * take + fr] = ibuf[fr * channels + c];
                }
            }
            vw.write_planar(&vplanar).map_err(|e| {
                crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
            })?;
            iw.write_planar(&iplanar).map_err(|e| {
                crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string()))
            })?;
            frame_idx += take;
        }

        vw.finalize()
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;
        iw.finalize()
            .map_err(|e| crate::error::Error::Audio(crate::error::AudioError::Io(e.to_string())))?;

        // cleanup temp files (best effort)
        let _ = std::fs::remove_file(&vraw_path);
        let _ = std::fs::remove_file(&iraw_path);

        Ok(())
    }

    pub fn separate_streaming(
        &self,
        input_path: impl AsRef<std::path::Path>,
        vocals_path: impl AsRef<std::path::Path>,
        instrumental_path: impl AsRef<std::path::Path>,
        chunk_duration: f64,
    ) -> Result<(), crate::error::Error> {
        self.separate_file(input_path, vocals_path, instrumental_path, chunk_duration)
    }
}

