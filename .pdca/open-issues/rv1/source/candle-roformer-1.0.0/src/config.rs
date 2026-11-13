use std::path::Path;

use crate::error::ConfigError;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SeparatorConfig {
    pub dim: usize,
    pub depth: usize,
    pub stereo: bool,
    pub num_stems: usize,
    /// Mel-band band count. MelBandRoformer only; BS-RoFormer configs carry
    /// `freqs_per_bands` instead (default 0 = absent).
    #[serde(default)]
    pub num_bands: usize,
    /// Explicit family selector. When absent the family is inferred from the
    /// config shape: `freqs_per_bands` present => BS-RoFormer, otherwise
    /// MelBandRoformer.
    #[serde(default)]
    pub family: Option<ModelFamily>,
    /// BS-RoFormer splits the STFT frequency axis into explicit consecutive
    /// band widths, e.g. `(2, 2, ..., 128, 129)`. The widths must sum to
    /// `dim_freqs_in`.
    #[serde(default)]
    pub freqs_per_bands: Option<Vec<usize>>,
    pub dim_head: usize,
    pub heads: usize,
    #[serde(default = "default_true")]
    pub flash_attn: bool,
    pub dim_freqs_in: usize,
    /// Model sample rate. UVR profiles usually carry it in the `audio:` block;
    /// it is resolved from there when the `model:` block omits it.
    #[serde(default)]
    pub sample_rate: usize,
    pub stft_n_fft: usize,
    pub stft_hop_length: usize,
    pub stft_win_length: usize,
    #[serde(default)]
    pub stft_normalized: bool,
    pub mask_estimator_depth: usize,
    #[serde(default)]
    pub use_torch_checkpoint: bool,
    pub time_transformer_depth: usize,
    pub freq_transformer_depth: usize,
    #[serde(default = "default_mlp_expansion")]
    pub mlp_expansion_factor: usize,
    #[serde(default)]
    pub linear_transformer_depth: usize,
    #[serde(default = "default_true")]
    pub norm_output: bool,
    #[serde(default)]
    pub skip_connection: bool,
    #[serde(default = "default_true")]
    pub zero_dc: bool,
}

fn default_true() -> bool {
    true
}

fn default_mlp_expansion() -> usize {
    4
}

impl SeparatorConfig {
    /// Parse a v7-style YAML profile (`model:` section).
    ///
    /// The upstream UVR model configs use `!!python/tuple` tags for the
    /// `freqs_per_bands` sequence; the tag carries no information a YAML
    /// sequence does not already have, so it is stripped before parsing.
    pub fn from_yaml_str(text: &str) -> Result<Self, ConfigError> {
        #[derive(Deserialize)]
        struct AudioSection {
            #[serde(default)]
            sample_rate: Option<usize>,
        }
        #[derive(Deserialize)]
        struct Root {
            model: SeparatorConfig,
            #[serde(default)]
            audio: Option<AudioSection>,
        }
        let text = text.replace("!!python/tuple", "");
        let root: Root =
            serde_yaml::from_str(&text).map_err(|e| ConfigError::Invalid(e.to_string()))?;
        let mut model = root.model;
        // UVR profiles keep the sample rate in `audio:`; the reference loader
        // copies it into the model profile. Default to 44.1 kHz (the only rate
        // the published RoFormer profiles use).
        if model.sample_rate == 0 {
            model.sample_rate = root.audio.and_then(|a| a.sample_rate).unwrap_or(44_100);
        }
        Ok(model)
    }

    /// Read and parse a v7-style YAML profile from disk.
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Invalid(format!("cannot read config: {}", e)))?;
        Self::from_yaml_str(&text)
    }

    /// Resolve the model family: an explicit `family:` key wins; otherwise the
    /// config shape decides (`freqs_per_bands` => BS-RoFormer, else MelBand).
    pub fn family(&self) -> ModelFamily {
        self.family.unwrap_or(if self.freqs_per_bands.is_some() {
            ModelFamily::BSRoformer
        } else {
            ModelFamily::MelBandRoformer
        })
    }

    /// The exact profile from `v7.yaml` (MelBandRoformer vocals).
    pub fn v7() -> Self {
        Self {
            dim: 256,
            depth: 12,
            stereo: true,
            num_stems: 1,
            num_bands: 60,
            family: None,
            freqs_per_bands: None,
            dim_head: 64,
            heads: 8,
            flash_attn: true,
            dim_freqs_in: 1025,
            sample_rate: 44100,
            stft_n_fft: 2048,
            stft_hop_length: 441,
            stft_win_length: 2048,
            stft_normalized: false,
            mask_estimator_depth: 2,
            use_torch_checkpoint: true,
            time_transformer_depth: 1,
            freq_transformer_depth: 1,
            mlp_expansion_factor: 4,
            linear_transformer_depth: 0,
            norm_output: true,
            skip_connection: false,
            zero_dc: true,
        }
    }

    /// The profile of the pinned Demucs v4 (HTDemucs) checkpoint.
    ///
    /// The separator geometry lives in [`crate::model::demucs::DemucsConfig`];
    /// this envelope carries the fields the generic [`crate::separator::Separator`]
    /// API needs (sample rate, stem count, stereo, STFT geometry).
    pub fn demucs() -> Self {
        Self {
            dim: 48,
            depth: 4,
            stereo: true,
            num_stems: 4,
            num_bands: 1,
            family: Some(ModelFamily::Demucs),
            freqs_per_bands: None,
            dim_head: 8,
            heads: 8,
            flash_attn: false,
            dim_freqs_in: 2048,
            sample_rate: 44100,
            stft_n_fft: 4096,
            stft_hop_length: 1024,
            stft_win_length: 4096,
            stft_normalized: true,
            mask_estimator_depth: 0,
            use_torch_checkpoint: false,
            time_transformer_depth: 1,
            freq_transformer_depth: 1,
            mlp_expansion_factor: 4,
            linear_transformer_depth: 0,
            norm_output: true,
            skip_connection: false,
            zero_dc: false,
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.dim == 0 {
            return Err(ConfigError::Invalid("dim must be > 0".into()));
        }
        if self.depth == 0 {
            return Err(ConfigError::Invalid("depth must be > 0".into()));
        }
        if self.num_stems == 0 {
            return Err(ConfigError::Invalid("num_stems must be > 0".into()));
        }
        match self.family() {
            ModelFamily::MelBandRoformer => {
                if self.num_bands == 0 {
                    return Err(ConfigError::Invalid(
                        "num_bands must be > 0 for mel_band_roformer".into(),
                    ));
                }
            }
            ModelFamily::BSRoformer => {
                let freqs = self.freqs_per_bands.as_ref().ok_or_else(|| {
                    ConfigError::Invalid("freqs_per_bands is required for bs_roformer".into())
                })?;
                if freqs.len() < 2 {
                    return Err(ConfigError::Invalid(
                        "freqs_per_bands must have at least 2 bands".into(),
                    ));
                }
                if freqs.iter().any(|f| *f == 0) {
                    return Err(ConfigError::Invalid(
                        "freqs_per_bands entries must be > 0".into(),
                    ));
                }
                let total: usize = freqs.iter().sum();
                if total != self.dim_freqs_in {
                    return Err(ConfigError::Invalid(format!(
                        "freqs_per_bands sums to {} but dim_freqs_in is {}",
                        total, self.dim_freqs_in
                    )));
                }
            }
            // MDX-Net ONNX checkpoints carry their geometry in the pinned
            // `mdx::MdxProfile`, not in a RoFormer YAML profile; the generic
            // checks above still apply. Inference runs through
            // `mdx::MdxSeparator`, which re-validates the profile.
            ModelFamily::MdxNet => {}
            // VR Arch checkpoints carry their band table in the pinned
            // `vr_pipeline::VrProfile` (the `4band_v2` model parameters), not in
            // a RoFormer YAML profile. `VrProfile::from_separator_config`
            // re-validates the STFT geometry against that table.
            ModelFamily::VrArch => {}
            // Demucs v4 keeps its whole geometry in the pinned
            // `model::demucs::DemucsConfig`; the profile only carries the
            // envelope needed to reach `Separator::load_demucs`.
            ModelFamily::Demucs => {}
        }
        if self.dim_head == 0 || self.heads == 0 {
            return Err(ConfigError::Invalid(
                "dim_head and heads must be > 0".into(),
            ));
        }
        if self.dim_freqs_in == 0 {
            return Err(ConfigError::Invalid("dim_freqs_in must be > 0".into()));
        }
        if self.sample_rate == 0 {
            return Err(ConfigError::Invalid("sample_rate must be > 0".into()));
        }
        if self.stft_n_fft == 0 || self.stft_n_fft % 2 != 0 {
            return Err(ConfigError::Invalid(
                "stft_n_fft must be a positive even number".into(),
            ));
        }
        if self.stft_hop_length == 0 {
            return Err(ConfigError::Invalid("stft_hop_length must be > 0".into()));
        }
        if self.stft_win_length == 0 || self.stft_win_length > self.stft_n_fft {
            return Err(ConfigError::Invalid(
                "stft_win_length must be in 1..=stft_n_fft".into(),
            ));
        }
        if self.dim_freqs_in != self.stft_n_fft / 2 + 1 {
            return Err(ConfigError::Invalid(format!(
                "dim_freqs_in ({}) must equal stft_n_fft/2 + 1 ({})",
                self.dim_freqs_in,
                self.stft_n_fft / 2 + 1
            )));
        }
        if self.stereo != true {
            // The v7 profile is stereo; keep the check explicit but allow other profiles?
        }
        Ok(())
    }
}

// ---------- Inference configuration (issue #9) ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ModelFamily {
    #[serde(rename = "mel_band_roformer", alias = "melband", alias = "mel_band")]
    MelBandRoformer,
    #[serde(rename = "bs_roformer", alias = "bsroformer", alias = "bs-roformer")]
    BSRoformer,
    #[serde(
        rename = "mdx_net",
        alias = "MdxNet",
        alias = "mdxnet",
        alias = "mdx-net"
    )]
    MdxNet,
    #[serde(
        rename = "vr_arch",
        alias = "VrArch",
        alias = "vrarch",
        alias = "vr_arch_uvr",
        alias = "uvr"
    )]
    VrArch,
    /// Demucs v4 (HTDemucs). Its geometry is pinned in
    /// `model::demucs::DemucsConfig`; the profile is used only to reach
    /// `Separator::load_demucs`.
    #[serde(
        rename = "demucs",
        alias = "Demucs",
        alias = "htdemucs",
        alias = "demucs_v4"
    )]
    Demucs,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct InferenceConfig {
    pub segment_size: Option<usize>,
    pub chunk_hops: usize,
    pub num_overlap: usize,
    pub step_size: Option<usize>,
    pub overlap_size: Option<usize>,
    pub fade_size: Option<usize>,
    pub batch_size: usize,
    pub pitch_shift: f32,
    pub denoise: bool,
    pub tta: bool,
    pub sample_rate: usize,
    #[serde(default = "default_normalization")]
    pub normalization: f32,
    #[serde(default = "default_amplification")]
    pub amplification: f32,
    #[serde(default = "default_invert_spect")]
    pub invert_spect: bool,
}

fn default_normalization() -> f32 {
    0.9
}
fn default_amplification() -> f32 {
    0.0
}
fn default_invert_spect() -> bool {
    false
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self::melband()
    }
}

impl InferenceConfig {
    pub fn melband() -> Self {
        Self {
            segment_size: None,
            chunk_hops: 800,
            num_overlap: 2,
            step_size: None,
            overlap_size: None,
            fade_size: None,
            batch_size: 1,
            pitch_shift: 0.0,
            denoise: false,
            tta: false,
            sample_rate: 44100,
            normalization: 0.9,
            amplification: 0.0,
            invert_spect: false,
        }
    }

    pub fn bsroformer() -> Self {
        // Same streaming granularity as the MelBand profile: 800 STFT hops per
        // chunk (352800 samples = 8 s at 44.1 kHz) and the reference's
        // 4-way overlap for BS-RoFormer profiles.
        Self {
            num_overlap: 4,
            ..Self::melband()
        }
    }

    /// MDX-Net (ONNX) architecture parameters as used by `audio-separator`
    /// 0.48.0 for the `UVR-MDX-NET-Inst_HQ_3` profile. The MDX-Net runtime
    /// itself (`crate::mdx`) reads the authoritative window/segment geometry
    /// from its [`crate::mdx::MdxProfile`]; this config carries the same
    /// user-facing level and segmentation choices for family dispatch.
    pub fn mdx() -> Self {
        Self {
            segment_size: Some(256),
            chunk_hops: 255,
            num_overlap: 4,
            step_size: None,
            overlap_size: None,
            fade_size: None,
            batch_size: 1,
            pitch_shift: 0.0,
            denoise: false,
            tta: false,
            sample_rate: 44100,
            normalization: 0.9,
            amplification: 0.0,
            invert_spect: false,
        }
    }

    /// VR Arch (UVR) architecture parameters as used by `audio-separator`
    /// 0.48.0 for the `4band_v2` profile. The band table, window and ROI live in
    /// the authoritative [`crate::model::VrProfile`]; this config carries the
    /// user-facing level and segmentation choices for family dispatch.
    pub fn vr_arch() -> Self {
        Self {
            segment_size: None,
            // The VR pipeline does not chunk; it windows the magnitude
            // spectrogram itself (512-frame window, 256-frame ROI). The value
            // keeps `validate()` satisfied and matches the MelBand default.
            chunk_hops: 800,
            num_overlap: 2,
            step_size: None,
            overlap_size: None,
            fade_size: None,
            batch_size: 1,
            pitch_shift: 0.0,
            denoise: false,
            tta: false,
            sample_rate: 44100,
            normalization: 0.9,
            amplification: 0.0,
            invert_spect: false,
        }
    }

    /// Demucs v4 (HTDemucs) level and segmentation choices.
    ///
    /// The reference path segments at `int(44100 * 39/5)` samples with
    /// `overlap = 0.25` and `shifts = 0`, and applies its own per-track
    /// normalisation, so the generic writer level processing is disabled here.
    pub fn demucs() -> Self {
        Self {
            segment_size: Some(343_980),
            chunk_hops: 343_980,
            num_overlap: 2,
            step_size: None,
            overlap_size: None,
            fade_size: None,
            batch_size: 1,
            pitch_shift: 0.0,
            denoise: false,
            tta: false,
            sample_rate: 44100,
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        }
    }

    pub fn for_family(family: ModelFamily) -> Self {
        match family {
            ModelFamily::MelBandRoformer => Self::melband(),
            ModelFamily::BSRoformer => Self::bsroformer(),
            ModelFamily::MdxNet => Self::mdx(),
            ModelFamily::VrArch => Self::vr_arch(),
            ModelFamily::Demucs => Self::demucs(),
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(s) = self.segment_size {
            if s == 0 {
                return Err(ConfigError::Invalid("segment must be > 0".into()));
            }
        }
        if self.segment_size.is_none() && self.chunk_hops == 0 {
            return Err(ConfigError::Invalid("chunk_hops must be > 0".into()));
        }
        if let Some(s) = self.step_size {
            if s == 0 {
                return Err(ConfigError::Invalid("step must be > 0".into()));
            }
        }
        if let Some(o) = self.overlap_size {
            if o == 0 {
                return Err(ConfigError::Invalid("overlap must be > 0".into()));
            }
        }
        if self.num_overlap == 0 {
            return Err(ConfigError::Invalid("overlap must be > 0".into()));
        }
        if let Some(f) = self.fade_size {
            if f == 0 {
                return Err(ConfigError::Invalid("fade must be > 0".into()));
            }
        }
        if let (Some(seg), Some(fade)) = (self.segment_size, self.fade_size) {
            if fade > seg {
                return Err(ConfigError::Invalid("fade must be <= chunk size".into()));
            }
        }
        if self.batch_size == 0 {
            return Err(ConfigError::Invalid("batch must be > 0".into()));
        }
        if !self.pitch_shift.is_finite() {
            return Err(ConfigError::Invalid("pitch_shift must be finite".into()));
        }
        if self.sample_rate == 0 {
            return Err(ConfigError::Invalid("sample_rate must be > 0".into()));
        }
        if !self.normalization.is_finite() || self.normalization < 0.0 {
            return Err(ConfigError::Invalid(
                "normalization must be finite and >= 0".into(),
            ));
        }
        if !self.amplification.is_finite() || self.amplification < 0.0 {
            return Err(ConfigError::Invalid(
                "amplification must be finite and >= 0".into(),
            ));
        }
        Ok(())
    }

    pub fn chunk_size(&self, model: &dyn crate::model::RoformerModel) -> usize {
        if let Some(c) = self.segment_size {
            c.max(1)
        } else {
            let hops = std::env::var("CANDLE_ROFORMER_CHUNK_HOPS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(self.chunk_hops);
            model.hop().max(1) * hops.max(1)
        }
    }

    pub fn step_for(&self, chunk: usize) -> usize {
        if let Some(s) = self.step_size {
            return s.max(1);
        }
        if let Some(k) = self.overlap_size {
            return (chunk / k.max(1)).max(1);
        }
        (chunk / self.num_overlap.max(1)).max(1)
    }

    pub fn fade_for(&self, chunk: usize) -> usize {
        self.fade_size.unwrap_or((chunk / 10).max(1)).max(1)
    }

    pub fn from_yaml_str(text: &str) -> Result<Self, ConfigError> {
        #[derive(serde::Deserialize)]
        struct Root {
            #[serde(default)]
            inference: Option<InferenceConfig>,
        }
        let root: Root =
            serde_yaml::from_str(text).map_err(|e| ConfigError::Invalid(e.to_string()))?;
        let cfg = root.inference.unwrap_or_default();
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn from_yaml_file(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Invalid(format!("cannot read config: {}", e)))?;
        Self::from_yaml_str(&text)
    }
}

/// Ensemble operator ids of the reference implementation.
///
/// The ids are exactly the `audio-separator` 0.48.0 names
/// (`separator/ensembler.py`, `Separator.VALID_ENSEMBLE_ALGORITHMS`). Only the
/// two waveform operators this crate implements are accepted; every other id —
/// including reference operators such as `avg_fft` or `uvr_max_spec` — is
/// rejected with [`crate::error::EnsembleError::UnknownAlgorithm`] rather than
/// being mapped onto a different computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnsembleAlgorithm {
    /// Per-sample arithmetic mean of the model waveforms.
    AvgWave,
    /// Per-sample median across the model waveforms.
    MedianWave,
}

impl EnsembleAlgorithm {
    /// Ids accepted by [`EnsembleAlgorithm::from_id`], in documented order.
    pub const SUPPORTED: [&'static str; 2] = ["avg_wave", "median_wave"];

    /// The reference id of this operator (`avg_wave` / `median_wave`).
    pub fn as_str(self) -> &'static str {
        match self {
            EnsembleAlgorithm::AvgWave => "avg_wave",
            EnsembleAlgorithm::MedianWave => "median_wave",
        }
    }

    /// Resolve a reference operator id; unknown ids are a typed error.
    pub fn from_id(id: &str) -> Result<Self, crate::error::EnsembleError> {
        match id {
            "avg_wave" => Ok(EnsembleAlgorithm::AvgWave),
            "median_wave" => Ok(EnsembleAlgorithm::MedianWave),
            other => Err(crate::error::EnsembleError::UnknownAlgorithm {
                id: other.to_string(),
                supported: Self::SUPPORTED.join(", "),
            }),
        }
    }
}

/// One model of an ensemble preset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EnsembleMember {
    /// Published checkpoint file name the member is built from (`.ckpt`); the
    /// runtime loads the offline-converted `.safetensors` of that checkpoint.
    pub checkpoint: String,
    /// Repository profile for that checkpoint (`configs/*.yaml`).
    pub profile: String,
    /// Stem this member contributes to the ensemble (the member's target stem).
    pub role: String,
}

/// A documented ensemble preset: the models, their weights, the operator and
/// the stem set a run produces.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EnsemblePreset {
    /// Stable id used on the command line.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// What the preset is for, including anything it is *not* (see
    /// `docs/ensemble-presets.md`).
    #[serde(default)]
    pub description: String,
    /// Operator applied across the members' waveforms.
    pub algorithm: EnsembleAlgorithm,
    /// Per-member weights; `null`/absent means equal weights.
    #[serde(default)]
    pub weights: Option<Vec<f32>>,
    /// Members, in application order.
    pub models: Vec<EnsembleMember>,
    /// The documented stem set the preset produces, in output order.
    pub stems: Vec<String>,
}

impl EnsemblePreset {
    /// Ids of the presets built into the crate.
    pub const BUILTINS: [&'static str; 1] = ["vocal_balanced"];

    /// The crate's `vocal_balanced` preset.
    ///
    /// MelBand (`voc_fv7`) + BS-RoFormer (`model_bs_roformer_ep_317_sdr_12.9755`)
    /// vocals models averaged with `avg_wave`, producing `vocals` and
    /// `instrumental`. It is this project's own preset: the `audio-separator`
    /// 0.48.0 preset of the same id uses different models and `avg_fft`, so the
    /// two are not interchangeable — see `docs/ensemble-presets.md`.
    pub fn vocal_balanced() -> Self {
        Self {
            id: "vocal_balanced".to_string(),
            name: "Vocal Balanced".to_string(),
            description: "MelBand (voc_fv7) + BS-RoFormer (Viperx 1297) vocals averaged with \
                          avg_wave; stems vocals + instrumental. Not the audio-separator preset \
                          of the same id (that one uses avg_fft and different models)."
                .to_string(),
            algorithm: EnsembleAlgorithm::AvgWave,
            weights: None,
            models: vec![
                EnsembleMember {
                    checkpoint: "model_bs_roformer_ep_317_sdr_12.9755.ckpt".to_string(),
                    profile: "configs/bs_roformer_viperx_1297.yaml".to_string(),
                    role: "vocals".to_string(),
                },
                EnsembleMember {
                    checkpoint: "voc_fv7.ckpt".to_string(),
                    profile: "configs/voc_fv7.yaml".to_string(),
                    role: "vocals".to_string(),
                },
            ],
            stems: vec!["vocals".to_string(), "instrumental".to_string()],
        }
    }

    /// Resolve a built-in preset id; unknown ids are a typed error.
    pub fn builtin(id: &str) -> Result<Self, crate::error::EnsembleError> {
        match id {
            "vocal_balanced" => Ok(Self::vocal_balanced()),
            other => Err(crate::error::EnsembleError::UnknownPreset {
                id: other.to_string(),
                known: Self::BUILTINS.join(", "),
            }),
        }
    }

    /// Reject presets the runner cannot honour.
    pub fn validate(&self) -> Result<(), crate::error::EnsembleError> {
        if self.id.trim().is_empty() {
            return Err(crate::error::EnsembleError::InvalidPreset(
                "preset id is empty".into(),
            ));
        }
        if self.models.is_empty() {
            return Err(crate::error::EnsembleError::InvalidPreset(format!(
                "preset `{}` has no models",
                self.id
            )));
        }
        if self.stems.is_empty() {
            return Err(crate::error::EnsembleError::InvalidPreset(format!(
                "preset `{}` declares no stems",
                self.id
            )));
        }
        for (i, stem) in self.stems.iter().enumerate() {
            if stem.trim().is_empty() {
                return Err(crate::error::EnsembleError::InvalidPreset(format!(
                    "preset `{}` has an empty stem name",
                    self.id
                )));
            }
            if self.stems[..i].contains(stem) {
                return Err(crate::error::EnsembleError::InvalidPreset(format!(
                    "preset `{}` repeats stem `{}`",
                    self.id, stem
                )));
            }
        }
        for member in &self.models {
            if member.role.trim().is_empty() {
                return Err(crate::error::EnsembleError::InvalidPreset(format!(
                    "preset `{}` has a member without a role",
                    self.id
                )));
            }
            if !self.stems.contains(&member.role) {
                return Err(crate::error::EnsembleError::InvalidPreset(format!(
                    "preset `{}` member role `{}` is not one of its stems",
                    self.id, member.role
                )));
            }
        }
        if let Some(weights) = &self.weights {
            if weights.len() != self.models.len() {
                return Err(crate::error::EnsembleError::InvalidWeights(format!(
                    "preset `{}` has {} weights for {} models",
                    self.id,
                    weights.len(),
                    self.models.len()
                )));
            }
            if weights.iter().any(|w| !w.is_finite()) {
                return Err(crate::error::EnsembleError::InvalidWeights(format!(
                    "preset `{}` weights must be finite",
                    self.id
                )));
            }
            let sum: f64 = weights.iter().map(|w| *w as f64).sum();
            if sum == 0.0 {
                return Err(crate::error::EnsembleError::InvalidWeights(format!(
                    "preset `{}` weights sum to zero",
                    self.id
                )));
            }
        }
        Ok(())
    }

    /// Parse a preset from YAML (the reference `ensemble_presets.json` fields
    /// plus `stems` and member roles).
    pub fn from_yaml_str(text: &str) -> Result<Self, crate::error::EnsembleError> {
        let preset: EnsemblePreset = serde_yaml::from_str(text).map_err(|e| {
            crate::error::EnsembleError::InvalidPreset(format!("cannot parse preset: {}", e))
        })?;
        preset.validate()?;
        Ok(preset)
    }

    /// Read and parse a preset file from disk.
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, crate::error::EnsembleError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            crate::error::EnsembleError::InvalidPreset(format!("cannot read preset: {}", e))
        })?;
        Self::from_yaml_str(&text)
    }
}
