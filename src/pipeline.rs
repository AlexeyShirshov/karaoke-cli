//! File orchestration on top of the `candle-roformer` crate: decode an input
//! file, hand the buffers to the library, write the stems and sidecars.
//!
//! Every path and naming decision of the CLI lives here; the library crate
//! itself never touches output file names.

use std::fs;
use std::path::{Path, PathBuf};

use candle_roformer::assets::{LocalSeparationAssets, LocalWhisperAssets};
use candle_roformer::device::DeviceChoice;
use candle_roformer::error::{AudioError, Error};
use candle_roformer::separator::{Separator, Stems};
use candle_roformer::wav::{read_audio, write_wav_f32, AudioBuffer};
use candle_roformer::whisper::{TranscribeOptions, Transcriber, Transcript, TranscriptSegment};

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
