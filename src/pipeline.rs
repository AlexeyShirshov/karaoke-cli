//! File orchestration on top of the `candle-roformer` crate: decode an input
//! file, hand the buffers to the library, write the stems and sidecars.
//!
//! Every path and naming decision of the CLI lives here; the library crate
//! itself never touches output file names.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use candle_roformer::assets::{LocalSeparationAssets, LocalWhisperAssets};
use candle_roformer::device::DeviceChoice;
use candle_roformer::error::{AudioError, Error};
use candle_roformer::separator::{Separator, Stems};
use candle_roformer::wav::{read_audio, write_wav_f32, AudioBuffer};
use candle_roformer::whisper::{TranscribeOptions, Transcriber, Transcript, TranscriptSegment};

/// Separate an in-memory buffer and return the complete named set supplied by
/// `candle-roformer` 1.0. `Stems::vocals` and `Stems::instrumental` remain
/// available as two-stem convenience fields; multi-stem models expose their
/// independent estimates through `Stems::stems` / `get` without synthesizing a
/// residual.
pub fn separate_audio(separator: &Separator, audio: &AudioBuffer) -> Result<Stems, Error> {
    separator.separate(audio)
}

/// Output file names (relative to the output directory).
pub struct OutputNames<'a> {
    pub vocals: &'a str,
    pub instrumental: Option<&'a str>,
}

/// Separate a file and write `vocals.wav` / `instrumental.wav`.
#[allow(clippy::too_many_arguments)]
pub fn run_pipeline(
    model_path: &str,
    config_path: &str,
    input_path: &str,
    output_dir: &str,
    device: DeviceChoice,
    overwrite: bool,
    start_seconds: Option<f64>,
    limit_seconds: Option<f64>,
) -> Result<(), Error> {
    run_pipeline_named(
        model_path,
        config_path,
        input_path,
        output_dir,
        device,
        overwrite,
        start_seconds,
        limit_seconds,
        &OutputNames {
            vocals: "vocals.wav",
            instrumental: Some("instrumental.wav"),
        },
    )
}

/// Separate a file with explicit output names.
#[allow(clippy::too_many_arguments)]
pub fn run_pipeline_named(
    model_path: &str,
    config_path: &str,
    input_path: &str,
    output_dir: &str,
    device: DeviceChoice,
    overwrite: bool,
    start_seconds: Option<f64>,
    limit_seconds: Option<f64>,
    names: &OutputNames,
) -> Result<(), Error> {
    let assets = LocalSeparationAssets::new(model_path, config_path);
    let separator = Separator::load(&assets, device)?;

    let mut audio = read_audio(input_path)?;
    trim_window(&mut audio, start_seconds, limit_seconds)?;
    let stems = separator.separate(&audio)?;

    write_stems(output_dir, names, &stems, overwrite)
}

/// Trim a decoded buffer to a `[start, start + duration)` window. Buffers are
/// channel-major, so both ends are cut per channel.
pub fn trim_window(
    audio: &mut AudioBuffer,
    start_seconds: Option<f64>,
    limit_seconds: Option<f64>,
) -> Result<(), Error> {
    let ch = audio.channels as usize;
    let frames = audio.planar.len() / ch;
    if let Some(start) = start_seconds {
        if start < 0.0 || !start.is_finite() {
            return Err(Error::Audio(AudioError::InvalidSampleRate(
                "invalid start offset".into(),
            )));
        }
        let skip = ((start * audio.sample_rate as f64).round() as usize).min(frames);
        if skip >= frames {
            return Err(Error::Audio(AudioError::EmptyInput(
                "start offset is beyond the input length".into(),
            )));
        }
        let mut cut = Vec::with_capacity(audio.planar.len() - skip * ch);
        for c in 0..ch {
            cut.extend_from_slice(&audio.planar[c * frames + skip..(c + 1) * frames]);
        }
        audio.planar = cut;
    }
    if let Some(secs) = limit_seconds {
        if secs <= 0.0 || !secs.is_finite() {
            return Err(Error::Audio(AudioError::InvalidSampleRate(
                "invalid duration".into(),
            )));
        }
        let frames = audio.planar.len() / ch;
        let want = ((secs * audio.sample_rate as f64).round() as usize).min(frames);
        if want == 0 {
            return Err(Error::Audio(AudioError::EmptyInput(
                "duration trims the whole input".into(),
            )));
        }
        let mut cut = Vec::with_capacity(want * ch);
        for c in 0..ch {
            cut.extend_from_slice(&audio.planar[c * frames..c * frames + want]);
        }
        audio.planar = cut;
    }
    Ok(())
}

/// Write stems through staging files: both are written first, then renamed
/// (one by one, not a multi-file atomic transaction). On failure this removes
/// only its own staging files.
pub fn write_stems(
    output_dir: &str,
    names: &OutputNames,
    stems: &Stems,
    overwrite: bool,
) -> Result<(), Error> {
    let out_dir = Path::new(output_dir);
    fs::create_dir_all(out_dir).map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;

    let vocals_path = out_dir.join(names.vocals);
    let instr_path = names.instrumental.map(|n| out_dir.join(n));

    if !overwrite
        && (vocals_path.exists() || instr_path.as_ref().map(|p| p.exists()).unwrap_or(false))
    {
        return Err(Error::Audio(AudioError::Io(
            "output file exists; use --overwrite".into(),
        )));
    }

    let staging_v = out_dir.join(format!(".{}.tmp", names.vocals));
    let staging_i = names
        .instrumental
        .map(|n| out_dir.join(format!(".{}.tmp", n)));

    if let Err(e) = write_wav_f32(&staging_v, &stems.vocals) {
        let _ = fs::remove_file(&staging_v);
        return Err(e);
    }
    if let Some(instr_path) = &instr_path {
        let staging_i = staging_i.as_ref().expect("staging path for instrumental");
        if let Err(e) = write_wav_f32(staging_i, &stems.instrumental) {
            let _ = fs::remove_file(&staging_v);
            let _ = fs::remove_file(staging_i);
            return Err(e);
        }
        if let Err(e) = fs::rename(staging_i, instr_path) {
            let _ = fs::remove_file(staging_i);
            return Err(Error::Audio(AudioError::Io(e.to_string())));
        }
    }
    if let Err(e) = fs::rename(&staging_v, &vocals_path) {
        let _ = fs::remove_file(&staging_v);
        return Err(Error::Audio(AudioError::Io(e.to_string())));
    }
    Ok(())
}

/// High-level request: separate a file and transcribe the vocal stem.
///
/// The separator is dropped before the Whisper model is loaded, so the two
/// model sets are never resident at the same time (important on small GPUs and
/// memory-tight machines).
#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    pub separator: LocalSeparationAssets,
    pub whisper: LocalWhisperAssets,
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub device: DeviceChoice,
    pub transcribe_options: TranscribeOptions,
    pub overwrite: bool,
    pub start_seconds: Option<f64>,
    pub limit_seconds: Option<f64>,
    pub single_stem: bool,
}

impl TranscriptionRequest {
    pub fn new(
        separator: LocalSeparationAssets,
        whisper: LocalWhisperAssets,
        input: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            separator,
            whisper,
            input: input.into(),
            output_dir: output_dir.into(),
            device: DeviceChoice::Cpu,
            transcribe_options: TranscribeOptions::default(),
            overwrite: false,
            start_seconds: None,
            limit_seconds: None,
            single_stem: false,
        }
    }
}

/// Paths and data produced by [`separate_and_transcribe_file`].
#[derive(Debug, Clone)]
pub struct TranscriptionOutput {
    pub vocals: PathBuf,
    pub instrumental: Option<PathBuf>,
    pub text: PathBuf,
    pub json: PathBuf,
    /// Offset of the written stems on the original input timeline (`--start`).
    pub start_offset_seconds: f64,
    pub duration_seconds: f64,
    pub transcript: Transcript,
}

/// `MM:SS.mmm` for the text sidecar.
pub fn format_timestamp(seconds: f64) -> String {
    let total_ms = if seconds.is_finite() {
        (seconds.max(0.0) * 1000.0).round() as u64
    } else {
        0
    };
    let minutes = total_ms / 60_000;
    let rest = total_ms % 60_000;
    format!("{:02}:{:02}.{:03}", minutes, rest / 1000, rest % 1000)
}

/// Separate `input` into wav stems, transcribe the vocal stem, and write
/// `<input>_(Vocals)_<model>.wav`/`.txt`/`.json` plus the instrumental wav.
///
/// All final paths are preflighted before anything is written; without
/// `overwrite` the call fails when any output already exists. Each file is
/// written to a dot-prefixed staging name and renamed into place, but this is
/// not claimed to be a multi-file atomic transaction: a failure part-way leaves
/// the files already renamed and removes only its own staging files.
pub fn separate_and_transcribe_file(
    request: &TranscriptionRequest,
) -> Result<TranscriptionOutput, Error> {
    let stem = request
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audio");
    let model_stem = request
        .separator
        .checkpoint
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model");
    let vocals_name = format!("{}_(Vocals)_{}.wav", stem, model_stem);
    let instrumental_name = if request.single_stem {
        None
    } else {
        Some(format!("{}_(Instrumental)_{}.wav", stem, model_stem))
    };
    let base = format!("{}_(Vocals)_{}", stem, model_stem);
    let text_name = format!("{}.txt", base);
    let json_name = format!("{}.json", base);

    let out_dir = &request.output_dir;
    fs::create_dir_all(out_dir).map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
    let text_path = out_dir.join(&text_name);
    let json_path = out_dir.join(&json_name);
    let mut final_paths = vec![out_dir.join(&vocals_name)];
    if let Some(name) = &instrumental_name {
        final_paths.push(out_dir.join(name));
    }
    final_paths.push(text_path.clone());
    final_paths.push(json_path.clone());

    if !request.overwrite {
        let existing: Vec<String> = final_paths
            .iter()
            .filter(|p| p.exists())
            .map(|p| p.display().to_string())
            .collect();
        if !existing.is_empty() {
            return Err(Error::Audio(AudioError::Io(format!(
                "output exists; use --overwrite: {}",
                existing.join(", ")
            ))));
        }
    }

    let mut audio = read_audio(&request.input)?;
    trim_window(&mut audio, request.start_seconds, request.limit_seconds)?;
    let start_offset_seconds = request.start_seconds.unwrap_or(0.0);
    let duration_seconds = audio.duration_secs();

    // Scope the separator so its weights are released before Whisper loads.
    let stems = {
        let separator = Separator::load(&request.separator, request.device)?;
        separator.separate(&audio)?
    };

    // Load Whisper before writing anything: bad ASR assets then fail without
    // leaving half of the outputs behind. The separator is already dropped, so
    // the two model sets are still never resident together.
    let mut transcriber = Transcriber::load(&request.whisper, request.device)?;

    let names = OutputNames {
        vocals: &vocals_name,
        instrumental: instrumental_name.as_deref(),
    };
    let out_dir_str = out_dir.to_str().ok_or_else(|| {
        Error::Audio(AudioError::Io("output directory is not valid UTF-8".into()))
    })?;
    write_stems(out_dir_str, &names, &stems, request.overwrite)?;

    let transcript = transcriber.transcribe_with(&stems.vocals, &request.transcribe_options)?;

    write_text(&text_path, &text_sidecar(&transcript))?;
    write_text(
        &json_path,
        &json_sidecar(
            &request.input,
            &model_stem,
            start_offset_seconds,
            duration_seconds,
            &transcript,
        )?,
    )?;

    Ok(TranscriptionOutput {
        vocals: out_dir.join(&vocals_name),
        instrumental: instrumental_name.map(|name| out_dir.join(name)),
        text: text_path,
        json: json_path,
        start_offset_seconds,
        duration_seconds,
        transcript,
    })
}

fn text_sidecar(transcript: &Transcript) -> String {
    let mut out = String::new();
    for segment in &transcript.segments {
        out.push_str(&format!(
            "[{} --> {}] {}\n",
            format_timestamp(segment.start_seconds),
            format_timestamp(segment.end_seconds),
            segment.text.trim()
        ));
    }
    out
}

#[derive(serde::Serialize)]
struct JsonSidecar<'a> {
    input: &'a str,
    model: &'a str,
    language: Option<&'a str>,
    /// `--start` of the processed window; segment times are relative to it.
    start_offset_seconds: f64,
    duration_seconds: f64,
    segments: &'a [TranscriptSegment],
}

fn json_sidecar(
    input: &Path,
    model: &str,
    start_offset_seconds: f64,
    duration_seconds: f64,
    transcript: &Transcript,
) -> Result<String, Error> {
    let sidecar = JsonSidecar {
        input: input.to_str().unwrap_or_default(),
        model,
        language: transcript.language.as_deref(),
        start_offset_seconds,
        duration_seconds,
        segments: &transcript.segments,
    };
    serde_json::to_string_pretty(&sidecar).map_err(|e| {
        Error::Audio(AudioError::Io(format!(
            "cannot serialize transcript: {}",
            e
        )))
    })
}

/// Write a UTF-8 file through a staging file and a rename. A pre-existing file
/// is replaced only because the caller already preflighted and decided to.
fn write_text(path: &Path, contents: &str) -> Result<(), Error> {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Audio(AudioError::Io("invalid output file name".into())))?;
    let staging = path.with_file_name(format!(".{}.tmp", file_name));
    if let Err(e) = fs::write(&staging, contents) {
        let _ = fs::remove_file(&staging);
        return Err(Error::Audio(AudioError::Io(e.to_string())));
    }
    if let Err(e) = fs::rename(&staging, path) {
        let _ = fs::remove_file(&staging);
        return Err(Error::Audio(AudioError::Io(e.to_string())));
    }
    Ok(())
}

/// Supported output containers. WAV always remains IEEE-f32; it is never
/// silently converted to integer PCM or dithered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Wav,
    Flac,
    Mp3,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "wav" => Ok(Self::Wav),
            "flac" => Ok(Self::Flac),
            "mp3" => Ok(Self::Mp3),
            "m4a" | "aac" => Err(format!(
                "output format '{value}' is unsupported: no bundled offline AAC/M4A encoder is available"
            )),
            _ => Err(format!(
                "unsupported output format '{value}'; choose wav, flac, or mp3"
            )),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Mp3 => "mp3",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InferenceOverrides {
    pub normalization: f32,
    pub amplification: f32,
    pub invert_spect: bool,
}

impl Default for InferenceOverrides {
    fn default() -> Self {
        Self {
            normalization: 0.9,
            amplification: 0.0,
            invert_spect: false,
        }
    }
}

/// Load a separator with the CLI's level settings without editing the user's
/// model profile. The temporary profile is removed immediately after loading.
pub fn load_separator_with_options(
    assets: &LocalSeparationAssets,
    device: DeviceChoice,
    overrides: InferenceOverrides,
) -> Result<Separator, Error> {
    if !overrides.normalization.is_finite() || overrides.normalization < 0.0 {
        return Err(Error::Audio(AudioError::InvalidSampleRate(
            "--normalization must be finite and >= 0".into(),
        )));
    }
    if !overrides.amplification.is_finite() || overrides.amplification < 0.0 {
        return Err(Error::Audio(AudioError::InvalidSampleRate(
            "--amplification must be finite and >= 0".into(),
        )));
    }

    static NEXT_CONFIG: AtomicU64 = AtomicU64::new(0);
    let original = fs::read_to_string(&assets.config).map_err(|error| {
        Error::Audio(AudioError::Io(format!("cannot read model config: {error}")))
    })?;
    let clean = original.replace("!!python/tuple", "");
    let mut root: serde_yaml::Value = serde_yaml::from_str(&clean).map_err(|error| {
        Error::Audio(AudioError::UnsupportedFormat(format!(
            "invalid model config: {error}"
        )))
    })?;
    let root_map = root.as_mapping_mut().ok_or_else(|| {
        Error::Audio(AudioError::UnsupportedFormat(
            "model config root must be a YAML mapping".into(),
        ))
    })?;
    let inference_key = serde_yaml::Value::String("inference".into());
    let inference = root_map
        .entry(inference_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let inference_map = inference.as_mapping_mut().ok_or_else(|| {
        Error::Audio(AudioError::UnsupportedFormat(
            "model config `inference` must be a YAML mapping".into(),
        ))
    })?;
    for (key, value) in [
        (
            "normalization",
            serde_yaml::to_value(overrides.normalization).unwrap(),
        ),
        (
            "amplification",
            serde_yaml::to_value(overrides.amplification).unwrap(),
        ),
        (
            "invert_spect",
            serde_yaml::Value::Bool(overrides.invert_spect),
        ),
    ] {
        inference_map.insert(serde_yaml::Value::String(key.into()), value);
    }
    let profile = serde_yaml::to_string(&root).map_err(|error| {
        Error::Audio(AudioError::UnsupportedFormat(format!(
            "cannot render model config: {error}"
        )))
    })?;
    let temp_path = std::env::temp_dir().join(format!(
        "karaoke-cli-config-{}-{}.yaml",
        std::process::id(),
        NEXT_CONFIG.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&temp_path, profile).map_err(|error| {
        Error::Audio(AudioError::Io(format!(
            "cannot create temporary model config: {error}"
        )))
    })?;
    let adjusted = LocalSeparationAssets::new(&assets.checkpoint, &temp_path);
    let result = Separator::load(&adjusted, device);
    let _ = fs::remove_file(temp_path);
    result
}

#[derive(Debug, Clone)]
pub struct FileRequest {
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub model_stem: String,
    pub overwrite: bool,
    pub custom_output_names: HashMap<String, String>,
    pub single_stem: Option<String>,
    pub format: OutputFormat,
    pub bitrate: Option<u16>,
    pub sample_rate: Option<u32>,
    pub start_seconds: Option<f64>,
    pub limit_seconds: Option<f64>,
    pub whisper: Option<(LocalWhisperAssets, TranscribeOptions)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StemFile {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct FileResult {
    pub stems: Vec<StemFile>,
    pub text: Option<PathBuf>,
    pub json: Option<PathBuf>,
    pub duration_seconds: f64,
}

/// Expand paths into a stable, non-recursive list. Explicit files are kept so
/// that unsupported/corrupt files receive a per-file decoder error; directory
/// entries are restricted to audio extensions.
#[derive(Debug, Default)]
pub struct InputExpansion {
    pub files: Vec<PathBuf>,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub struct BatchReport {
    pub outcomes: Vec<(PathBuf, Result<FileResult, String>)>,
    pub succeeded: usize,
    pub failed: usize,
}

/// Execute each input independently and retain every result. An error from one
/// input never short-circuits the remaining files in the batch.
pub fn process_batch<F>(inputs: &[PathBuf], mut process: F) -> BatchReport
where
    F: FnMut(&Path) -> Result<FileResult, String>,
{
    let outcomes: Vec<_> = inputs
        .iter()
        .map(|input| (input.clone(), process(input)))
        .collect();
    let succeeded = outcomes.iter().filter(|(_, result)| result.is_ok()).count();
    let failed = outcomes.len() - succeeded;
    BatchReport {
        outcomes,
        succeeded,
        failed,
    }
}

pub fn expand_inputs(inputs: &[PathBuf]) -> InputExpansion {
    let mut result = InputExpansion::default();
    let mut files = Vec::new();
    for input in inputs {
        match fs::metadata(input) {
            Ok(metadata) if metadata.is_file() => files.push(input.clone()),
            Ok(metadata) if metadata.is_dir() => match fs::read_dir(input) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => match entry.file_type() {
                                Ok(kind) if kind.is_file() && is_audio_path(&entry.path()) => {
                                    files.push(entry.path());
                                }
                                Ok(_) => {}
                                Err(error) => result.errors.push(format!(
                                    "{}: cannot inspect directory entry: {error}",
                                    entry.path().display()
                                )),
                            },
                            Err(error) => result.errors.push(format!(
                                "{}: cannot read directory entry: {error}",
                                input.display()
                            )),
                        }
                    }
                }
                Err(error) => result.errors.push(format!(
                    "{}: cannot read input directory: {error}",
                    input.display()
                )),
            },
            Ok(_) => result.errors.push(format!(
                "{}: input is not a file or directory",
                input.display()
            )),
            Err(error) => result
                .errors
                .push(format!("{}: cannot access input: {error}", input.display())),
        }
    }
    files.sort();
    files.dedup();
    result.files = files;
    result
}

fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "wav" | "flac" | "mp3" | "m4a" | "ogg" | "opus" | "aac" | "aiff" | "aif"
            )
        })
        .unwrap_or(false)
}

pub fn validate_output_options(
    format: OutputFormat,
    bitrate: Option<u16>,
    sample_rate: Option<u32>,
) -> Result<(), String> {
    if let Some(rate) = sample_rate {
        if rate == 0 || rate > 384_000 {
            return Err(format!(
                "unsupported --sample_rate {rate}; choose 1..=384000 Hz"
            ));
        }
    }
    match (format, bitrate) {
        (OutputFormat::Mp3, Some(rate)) if !supported_mp3_bitrate(rate) => Err(format!(
            "unsupported MP3 bitrate {rate} kbps; supported values: 8, 16, 24, 32, 40, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320"
        )),
        (OutputFormat::Mp3, _) => {
            if let Some(rate) = sample_rate {
                if !matches!(rate, 8_000 | 11_025 | 12_000 | 16_000 | 22_050 | 24_000 | 32_000 | 44_100 | 48_000) {
                    return Err(format!("MP3 does not support sample rate {rate} Hz"));
                }
            }
            Ok(())
        }
        (_, Some(_)) => Err("--output_bitrate is only valid with --output_format mp3".into()),
        _ => Ok(()),
    }
}

fn supported_mp3_bitrate(bitrate: u16) -> bool {
    matches!(
        bitrate,
        8 | 16 | 24 | 32 | 40 | 48 | 64 | 80 | 96 | 112 | 128 | 160 | 192 | 224 | 256 | 320
    )
}

/// Separate and write all requested named stems. The complete set of output
/// paths is preflighted before creating the output directory or writing files.
pub fn run_file(separator: &Separator, request: &FileRequest) -> Result<FileResult, Error> {
    validate_output_options(request.format, request.bitrate, request.sample_rate)
        .map_err(|message| Error::Audio(AudioError::UnsupportedFormat(message)))?;
    let decode_started = Instant::now();
    let mut input = read_audio(&request.input)?;
    trim_window(&mut input, request.start_seconds, request.limit_seconds)?;
    let duration_seconds = input.duration_secs();
    log::debug!(
        "stage=decode input={} elapsed_ms={}",
        request.input.display(),
        decode_started.elapsed().as_millis()
    );

    let separate_started = Instant::now();
    let stems = separator.separate(&input)?;
    log::debug!(
        "stage=separate input={} stem_count={} elapsed_ms={}",
        request.input.display(),
        stems.len(),
        separate_started.elapsed().as_millis()
    );
    if stems.is_empty() {
        return Err(Error::Audio(AudioError::UnsupportedFormat(
            "separator returned no named stems".into(),
        )));
    }

    let all_names: Vec<String> = stems.stems.iter().map(|(name, _)| name.clone()).collect();
    let custom_names = validate_custom_names(&request.custom_output_names, &all_names)
        .map_err(|message| Error::Audio(AudioError::UnsupportedFormat(message)))?;
    let selected: Vec<(&str, &AudioBuffer)> = if let Some(selection) = &request.single_stem {
        let (name, buffer) = stems
            .stems
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(selection))
            .ok_or_else(|| {
                Error::Audio(AudioError::UnsupportedFormat(format!(
                    "unknown stem '{}'; available stems: {}",
                    selection,
                    all_names.join(", ")
                )))
            })?;
        vec![(name.as_str(), buffer)]
    } else {
        stems
            .stems
            .iter()
            .map(|(name, buffer)| (name.as_str(), buffer))
            .collect()
    };

    let mut staged_contents = Vec::<(PathBuf, StagedContent)>::new();
    let mut written = Vec::with_capacity(selected.len());
    let mut vocals_base = None;
    let mut duplicate_paths = HashSet::new();
    for (name, buffer) in &selected {
        let base = custom_names
            .get(&name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_else(|| {
                let input_stem = request
                    .input
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("audio");
                format!(
                    "{}_({})_{}",
                    input_stem,
                    stem_label(name),
                    request.model_stem
                )
            });
        if name.eq_ignore_ascii_case("vocals") || (vocals_base.is_none() && stems.len() == 1) {
            vocals_base = Some(base.clone());
        }
        let final_path =
            request
                .output_dir
                .join(format!("{}.{}", base, request.format.extension()));
        if !duplicate_paths.insert(final_path.clone()) {
            return Err(Error::Audio(AudioError::UnsupportedFormat(format!(
                "multiple outputs resolve to {}",
                final_path.display()
            ))));
        }
        let output = resample_for_output(buffer, request.sample_rate)?;
        staged_contents.push((
            final_path.clone(),
            StagedContent::Audio(output, request.format, request.bitrate),
        ));
        written.push(StemFile {
            name: (*name).to_string(),
            path: final_path,
        });
    }

    let mut transcript = None;
    let mut sidecars = None;
    if let Some((assets, options)) = &request.whisper {
        let vocal = stems
            .get("vocals")
            .or_else(|| selected.first().map(|(_, buffer)| *buffer))
            .ok_or_else(|| {
                Error::Audio(AudioError::UnsupportedFormat(
                    "no vocal stem to transcribe".into(),
                ))
            })?;
        if let Some(selected_name) = &request.single_stem {
            if !selected_name.eq_ignore_ascii_case("vocals") {
                return Err(Error::Audio(AudioError::UnsupportedFormat(
                    "--transcribe requires --single_stem vocals when selecting one stem".into(),
                )));
            }
        }
        let base = vocals_base.unwrap_or_else(|| {
            let input_stem = request
                .input
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("audio");
            format!(
                "{}_({})_{}",
                input_stem,
                stem_label("vocals"),
                request.model_stem
            )
        });
        let text_path = request.output_dir.join(format!("{base}.txt"));
        let json_path = request.output_dir.join(format!("{base}.json"));
        if !duplicate_paths.insert(text_path.clone()) || !duplicate_paths.insert(json_path.clone())
        {
            return Err(Error::Audio(AudioError::UnsupportedFormat(
                "transcription sidecar path collides with another output".into(),
            )));
        }
        let transcription_started = Instant::now();
        let mut transcriber = Transcriber::load(assets, DeviceChoice::Cpu)?;
        let transcript_value = transcriber.transcribe_with(vocal, options)?;
        log::debug!(
            "stage=transcribe input={} elapsed_ms={}",
            request.input.display(),
            transcription_started.elapsed().as_millis()
        );
        let text = text_sidecar(&transcript_value);
        let json = json_sidecar(
            &request.input,
            &request.model_stem,
            request.start_seconds.unwrap_or(0.0),
            duration_seconds,
            &transcript_value,
        )?;
        staged_contents.push((text_path.clone(), StagedContent::Text(text)));
        staged_contents.push((json_path.clone(), StagedContent::Text(json)));
        sidecars = Some((text_path, json_path));
        transcript = Some(transcript_value);
    }

    let final_paths: Vec<PathBuf> = staged_contents
        .iter()
        .map(|(path, _)| path.clone())
        .collect();
    preflight_outputs(&request.output_dir, &final_paths, request.overwrite)?;
    let write_started = Instant::now();
    write_staged_outputs(&request.output_dir, staged_contents)?;
    log::debug!(
        "stage=write input={} outputs={} elapsed_ms={}",
        request.input.display(),
        final_paths.len(),
        write_started.elapsed().as_millis()
    );
    let _ = transcript;
    Ok(FileResult {
        stems: written,
        text: sidecars.as_ref().map(|(text, _)| text.clone()),
        json: sidecars.map(|(_, json)| json),
        duration_seconds,
    })
}

fn validate_custom_names(
    custom: &HashMap<String, String>,
    known_stems: &[String],
) -> Result<BTreeMap<String, String>, String> {
    let known: HashMap<String, &str> = known_stems
        .iter()
        .map(|name| (name.to_ascii_lowercase(), name.as_str()))
        .collect();
    let mut result = BTreeMap::new();
    for (name, base) in custom {
        let key = name.to_ascii_lowercase();
        if !known.contains_key(&key) {
            return Err(format!(
                "unknown stem '{}' in --custom_output_names; available stems: {}",
                name,
                known_stems.join(", ")
            ));
        }
        validate_output_base(base)
            .map_err(|reason| format!("invalid output name for stem '{}': {}", name, reason))?;
        result.insert(key, base.clone());
    }
    Ok(result)
}

pub fn validate_output_base(base: &str) -> Result<(), String> {
    if base.trim().is_empty() {
        return Err("name must not be empty".into());
    }
    let path = Path::new(base);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
        || base.contains('/')
        || base.contains('\\')
        || base == "."
        || base == ".."
    {
        return Err(
            "absolute paths, directory separators, and '..' traversal are not allowed".into(),
        );
    }
    if base.chars().any(char::is_control) {
        return Err("control characters are not allowed".into());
    }
    Ok(())
}

fn stem_label(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "vocals" => "Vocals".into(),
        "instrumental" => "Instrumental".into(),
        _ => {
            let mut chars = name.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

fn resample_for_output(
    buffer: &AudioBuffer,
    sample_rate: Option<u32>,
) -> Result<AudioBuffer, Error> {
    let Some(target_rate) = sample_rate else {
        return Ok(buffer.clone());
    };
    if target_rate == buffer.sample_rate {
        return Ok(buffer.clone());
    }
    let planar = candle_roformer::resample::resample_planar(
        &buffer.planar,
        buffer.channels as usize,
        buffer.sample_rate,
        target_rate,
    );
    let output = AudioBuffer {
        sample_rate: target_rate,
        channels: buffer.channels,
        planar,
    };
    output.validate()?;
    Ok(output)
}

fn preflight_outputs(output_dir: &Path, paths: &[PathBuf], overwrite: bool) -> Result<(), Error> {
    let mut unique = HashSet::new();
    for path in paths {
        if !unique.insert(path.clone()) {
            return Err(Error::Audio(AudioError::UnsupportedFormat(format!(
                "duplicate output path {}",
                path.display()
            ))));
        }
        if !overwrite && path.exists() {
            return Err(Error::Audio(AudioError::Io(format!(
                "output file exists ({}); use --overwrite",
                path.display()
            ))));
        }
    }
    if output_dir.exists() && !output_dir.is_dir() {
        return Err(Error::Audio(AudioError::Io(format!(
            "output path {} is not a directory",
            output_dir.display()
        ))));
    }
    Ok(())
}

enum StagedContent {
    Audio(AudioBuffer, OutputFormat, Option<u16>),
    Text(String),
}

fn write_staged_outputs(
    output_dir: &Path,
    outputs: Vec<(PathBuf, StagedContent)>,
) -> Result<(), Error> {
    static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);
    fs::create_dir_all(output_dir)
        .map_err(|error| Error::Audio(AudioError::Io(error.to_string())))?;
    let mut staged = Vec::with_capacity(outputs.len());
    for (final_path, content) in outputs {
        let id = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let stage_path = output_dir.join(format!(".karaoke-{}-{id}.tmp", std::process::id()));
        let result = match content {
            StagedContent::Audio(buffer, format, bitrate) => {
                write_audio(&stage_path, &buffer, format, bitrate)
            }
            StagedContent::Text(text) => fs::write(&stage_path, text)
                .map_err(|error| Error::Audio(AudioError::Io(error.to_string()))),
        };
        if let Err(error) = result {
            let _ = fs::remove_file(&stage_path);
            for (_, path) in &staged {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        staged.push((final_path, stage_path));
    }
    for (final_path, stage_path) in &staged {
        if let Err(error) = fs::rename(stage_path, final_path) {
            for (_, path) in &staged {
                let _ = fs::remove_file(path);
            }
            return Err(Error::Audio(AudioError::Io(format!(
                "cannot install output {}: {error}",
                final_path.display()
            ))));
        }
    }
    Ok(())
}

fn write_audio(
    path: &Path,
    audio: &AudioBuffer,
    format: OutputFormat,
    bitrate: Option<u16>,
) -> Result<(), Error> {
    audio.validate()?;
    match format {
        OutputFormat::Wav => write_wav_f32(path, audio),
        OutputFormat::Flac => write_flac(path, audio),
        OutputFormat::Mp3 => write_mp3(path, audio, bitrate.unwrap_or(192)),
    }
}

fn write_flac(path: &Path, audio: &AudioBuffer) -> Result<(), Error> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;

    let channels = audio.channels as usize;
    let frames = audio.planar.len() / channels;
    let bits_per_sample = 24usize;
    let scale = ((1i64 << (bits_per_sample - 1)) - 1) as f32;
    let mut interleaved = Vec::with_capacity(audio.planar.len());
    for frame in 0..frames {
        for channel in 0..channels {
            let sample = audio.planar[channel * frames + frame].clamp(-1.0, 1.0);
            interleaved.push((sample * scale).round() as i32);
        }
    }
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|error| {
            Error::Audio(AudioError::UnsupportedFormat(format!(
                "invalid FLAC encoder config: {error:?}"
            )))
        })?;
    let source = flacenc::source::MemSource::from_samples(
        &interleaved,
        channels,
        bits_per_sample,
        audio.sample_rate as usize,
    );
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|error| {
            Error::Audio(AudioError::UnsupportedFormat(format!(
                "FLAC encoding failed: {error}"
            )))
        })?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(|error| {
        Error::Audio(AudioError::UnsupportedFormat(format!(
            "cannot write FLAC stream: {error:?}"
        )))
    })?;
    fs::write(path, sink.as_slice())
        .map_err(|error| Error::Audio(AudioError::Io(error.to_string())))
}

fn write_mp3(path: &Path, audio: &AudioBuffer, bitrate: u16) -> Result<(), Error> {
    use mp3lame_encoder::{Bitrate, DualPcm, FlushNoGap, MonoPcm, Quality};

    let bitrate = match bitrate {
        8 => Bitrate::Kbps8,
        16 => Bitrate::Kbps16,
        24 => Bitrate::Kbps24,
        32 => Bitrate::Kbps32,
        40 => Bitrate::Kbps40,
        48 => Bitrate::Kbps48,
        64 => Bitrate::Kbps64,
        80 => Bitrate::Kbps80,
        96 => Bitrate::Kbps96,
        112 => Bitrate::Kbps112,
        128 => Bitrate::Kbps128,
        160 => Bitrate::Kbps160,
        192 => Bitrate::Kbps192,
        224 => Bitrate::Kbps224,
        256 => Bitrate::Kbps256,
        320 => Bitrate::Kbps320,
        _ => {
            return Err(Error::Audio(AudioError::UnsupportedFormat(format!(
                "unsupported MP3 bitrate {bitrate} kbps"
            ))));
        }
    };
    let mut builder = mp3lame_encoder::Builder::new()
        .ok_or_else(|| Error::Audio(AudioError::Io("cannot allocate LAME encoder".into())))?;
    builder
        .set_num_channels(audio.channels as u8)
        .and_then(|_| builder.set_sample_rate(audio.sample_rate))
        .and_then(|_| builder.set_output_sample_rate(NonZeroU32::new(audio.sample_rate)))
        .and_then(|_| builder.set_brate(bitrate))
        .and_then(|_| builder.set_quality(Quality::Best))
        .map_err(|error| {
            Error::Audio(AudioError::UnsupportedFormat(format!(
                "MP3 encoder configuration failed: {error}"
            )))
        })?;
    let mut encoder = builder.build().map_err(|error| {
        Error::Audio(AudioError::UnsupportedFormat(format!(
            "cannot initialize MP3 encoder: {error}"
        )))
    })?;
    let frames = audio.planar.len() / audio.channels as usize;
    let capacity = mp3lame_encoder::max_required_buffer_size(frames).saturating_add(7200);
    let mut encoded = Vec::with_capacity(capacity);
    if audio.channels == 1 {
        let samples: Vec<f32> = audio
            .planar
            .iter()
            .map(|sample| sample.clamp(-1.0, 1.0))
            .collect();
        encoder
            .encode_to_vec(MonoPcm(&samples), &mut encoded)
            .map_err(|error| {
                Error::Audio(AudioError::UnsupportedFormat(format!(
                    "MP3 encoding failed: {error}"
                )))
            })?;
    } else {
        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for frame in 0..frames {
            left.push(audio.planar[frame].clamp(-1.0, 1.0));
            right.push(audio.planar[frames + frame].clamp(-1.0, 1.0));
        }
        encoder
            .encode_to_vec(
                DualPcm {
                    left: &left,
                    right: &right,
                },
                &mut encoded,
            )
            .map_err(|error| {
                Error::Audio(AudioError::UnsupportedFormat(format!(
                    "MP3 encoding failed: {error}"
                )))
            })?;
    }
    encoder
        .flush_to_vec::<FlushNoGap>(&mut encoded)
        .map_err(|error| {
            Error::Audio(AudioError::UnsupportedFormat(format!(
                "MP3 finalization failed: {error}"
            )))
        })?;
    fs::write(path, encoded).map_err(|error| Error::Audio(AudioError::Io(error.to_string())))
}

#[cfg(test)]
mod output_tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "karaoke-cli-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn synthetic_audio(sample_rate: u32, frames: usize, channels: u16) -> AudioBuffer {
        let mut planar = Vec::with_capacity(frames * channels as usize);
        for channel in 0..channels as usize {
            planar.extend(
                (0..frames)
                    .map(|frame| ((frame as f32 * 0.037 + channel as f32) * 0.7).sin() * 0.6),
            );
        }
        AudioBuffer {
            sample_rate,
            channels,
            planar,
        }
    }

    #[test]
    fn custom_name_validation_rejects_unknown_and_traversal_without_paths() {
        let known = vec!["vocals".to_string(), "drums".to_string()];
        assert!(
            validate_custom_names(&HashMap::from([("vocals".into(), "mix".into())]), &known)
                .is_ok()
        );
        assert!(
            validate_custom_names(&HashMap::from([("guitar".into(), "mix".into())]), &known)
                .is_err()
        );
        assert!(validate_custom_names(
            &HashMap::from([("vocals".into(), "../mix".into())]),
            &known
        )
        .is_err());
        assert!(validate_custom_names(
            &HashMap::from([("vocals".into(), "/tmp/mix".into())]),
            &known
        )
        .is_err());
    }

    #[test]
    fn directory_discovery_is_non_recursive_filtered_and_sorted() {
        let dir = temp_dir("discover");
        fs::write(dir.join("z.wav"), []).unwrap();
        fs::write(dir.join("a.MP3"), []).unwrap();
        fs::write(dir.join("ignore.txt"), []).unwrap();
        fs::create_dir(dir.join("nested")).unwrap();
        fs::write(dir.join("nested").join("deep.flac"), []).unwrap();
        let expanded = expand_inputs(&[dir.clone()]);
        assert!(expanded.errors.is_empty());
        assert_eq!(expanded.files.len(), 2);
        assert!(expanded.files[0].ends_with("a.MP3"));
        assert!(expanded.files[1].ends_with("z.wav"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn flac_and_mp3_round_trip_through_project_decoder() {
        let dir = temp_dir("codecs");
        let source = synthetic_audio(44_100, 8_192, 1);
        for (format, bitrate) in [(OutputFormat::Flac, None), (OutputFormat::Mp3, Some(192))] {
            let path = dir.join(format!("roundtrip.{}", format.extension()));
            write_audio(&path, &source, format, bitrate).unwrap();
            let decoded = read_audio(&path).unwrap();
            assert_eq!(decoded.sample_rate, source.sample_rate);
            let delta = (decoded.duration_secs() - source.duration_secs()).abs();
            let tolerance = if format == OutputFormat::Mp3 {
                0.05
            } else {
                0.0
            };
            assert!(delta <= tolerance, "{format:?}: duration delta {delta}");
            if format == OutputFormat::Flac {
                assert_eq!(decoded.planar.len(), source.planar.len());
            }
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn output_format_and_sample_rate_validation_is_explicit() {
        assert!(OutputFormat::parse("m4a")
            .unwrap_err()
            .contains("offline AAC/M4A"));
        assert!(validate_output_options(OutputFormat::Mp3, Some(192), Some(44_100)).is_ok());
        assert!(validate_output_options(OutputFormat::Mp3, Some(123), None).is_err());
        assert!(validate_output_options(OutputFormat::Wav, Some(192), None).is_err());
        assert!(validate_output_options(OutputFormat::Flac, None, Some(0)).is_err());
    }
}
