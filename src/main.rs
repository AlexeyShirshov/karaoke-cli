use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use candle_roformer::device::DeviceChoice;
use candle_roformer::whisper::TranscribeOptions;
use candle_roformer::{LocalSeparationAssets, LocalWhisperAssets};
use karaoke_cli::catalog;
use karaoke_cli::pipeline::{
    expand_inputs, load_separator_with_options, process_batch, run_file, FileRequest,
    InferenceOverrides, OutputFormat,
};
use log::{LevelFilter, Log, Metadata, Record};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const CANDLE_ROFORMER_VERSION: &str = env!("CANDLE_ROFORMER_VERSION");
const CANDLE_CORE_VERSION: &str = env!("CANDLE_CORE_VERSION");
const CANDLE_ROFORMER_GIT_REVISION: &str = env!("CANDLE_ROFORMER_GIT_REVISION");

struct StderrLogger;
static LOGGER: StderrLogger = StderrLogger;

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        log::max_level() >= metadata.level().to_level_filter()
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        eprintln!(
            "{}.{:03} {} {}: {}",
            elapsed.as_secs(),
            elapsed.subsec_millis(),
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {}
}

#[derive(Debug)]
struct Args {
    inputs: Vec<PathBuf>,
    model: Option<String>,
    config: Option<String>,
    output_dir: PathBuf,
    model_file_dir: Option<String>,
    format: OutputFormat,
    bitrate: Option<u16>,
    sample_rate: Option<u32>,
    single_stem: Option<String>,
    custom_output_names: HashMap<String, String>,
    device: String,
    overwrite: bool,
    start: Option<f64>,
    duration: Option<f64>,
    normalization: f32,
    amplification: f32,
    invert_spect: bool,
    transcribe: bool,
    whisper_model: Option<String>,
    whisper_config: Option<String>,
    whisper_tokenizer: Option<String>,
    whisper_model_dir: Option<String>,
    language: Option<String>,
    env_info: bool,
    debug: bool,
    log_level: LevelFilter,
    list_models: bool,
    list_filter: Option<String>,
    list_limit: Option<usize>,
    list_format: String,
    models_manifest: Option<PathBuf>,
}

fn print_help() {
    println!("karaoke-cli {}", VERSION);
    println!();
    println!("Usage:");
    println!("  karaoke-cli [audio files or directories...] --model_filename <file.safetensors> [options]");
    println!();
    println!("Options:");
    println!("  --model_filename <file>   Local checkpoint (alias: --model)");
    println!(
        "  --config <file.yaml>      Model profile (default: <model-stem>.yaml next to model)"
    );
    println!("  --model_file_dir <dir>    Directory for relative model/config paths");
    println!("  --output_dir <dir>        Output directory (default: .)");
    println!("  --output_format wav|flac|mp3 (default: wav; M4A/AAC are rejected)");
    println!("  --output_bitrate <kbps>   MP3 CBR bitrate (default: 192)");
    println!("  --sample_rate <Hz>        Resample output with candle-roformer's resampler");
    println!("  --single_stem <name>      Write one named stem, e.g. vocals or drums");
    println!("  --custom_output_names '<json>'  Stem-name to safe output basename mapping");
    println!("  --normalization <peak>    Peak normalization threshold (default: 0.9; 0 disables)");
    println!("  --amplification <peak>    Amplify quiet input below threshold (default: 0.0/off)");
    println!("  --invert_spect            Build residual in the spectral domain");
    println!("  --device cpu|cuda|metal   Device (default: cpu); unavailable device is an error");
    println!("  --start <seconds>         Start offset in the input");
    println!("  --duration <seconds>      Process only this many seconds");
    println!("  --overwrite               Replace existing outputs after preflight");
    println!(
        "  --env_info                Print local runtime/device information without model weights"
    );
    println!("  --debug                   Alias for --log_level debug");
    println!("  --log_level <level>       trace|debug|info|warn|error|off (default: info)");
    println!("  --list_models             List local model metadata only; never downloads weights");
    println!("  --list_filter <text>      Case-insensitive metadata substring filter");
    println!("  --list_limit <n>          Maximum number of model rows");
    println!("  --list_format pretty|json (default: pretty)");
    println!("  --models_manifest <file>  Optional local JSON/YAML catalog manifest");
    println!();
    println!("Transcription (Whisper, optional):");
    println!("  --transcribe              Transcribe the vocal stem to .txt and .json");
    println!(
        "  --whisper_model_dir <dir> Directory with config.json, tokenizer.json, model.safetensors"
    );
    println!("  --whisper_model <file>    Override model.safetensors (local path)");
    println!("  --whisper_config <file>   Override config.json (local path)");
    println!("  --whisper_tokenizer <file> Override tokenizer.json (local path)");
    println!("  --language <auto|code>    Whisper language (default: auto)");
    println!("  --version | --help");
}

fn value(argv: &[String], index: usize, name: &str) -> Result<String, String> {
    argv.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn parse_level(value: &str) -> Result<LevelFilter, String> {
    match value.to_ascii_lowercase().as_str() {
        "trace" => Ok(LevelFilter::Trace),
        "debug" => Ok(LevelFilter::Debug),
        "info" => Ok(LevelFilter::Info),
        "warn" | "warning" => Ok(LevelFilter::Warn),
        "error" => Ok(LevelFilter::Error),
        "off" => Ok(LevelFilter::Off),
        _ => Err(format!(
            "invalid --log_level '{value}'; choose trace, debug, info, warn, error, or off"
        )),
    }
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut args = Args {
        inputs: Vec::new(),
        model: None,
        config: None,
        output_dir: PathBuf::from("."),
        model_file_dir: None,
        format: OutputFormat::Wav,
        bitrate: None,
        sample_rate: None,
        single_stem: None,
        custom_output_names: HashMap::new(),
        device: "cpu".into(),
        overwrite: false,
        start: None,
        duration: None,
        normalization: 0.9,
        amplification: 0.0,
        invert_spect: false,
        transcribe: false,
        whisper_model: None,
        whisper_config: None,
        whisper_tokenizer: None,
        whisper_model_dir: None,
        language: None,
        env_info: false,
        debug: false,
        log_level: LevelFilter::Info,
        list_models: false,
        list_filter: None,
        list_limit: None,
        list_format: "pretty".into(),
        models_manifest: None,
    };
    let mut positional_only = false;
    let mut index = 0;
    while index < argv.len() {
        let arg = argv[index].as_str();
        if positional_only {
            args.inputs.push(PathBuf::from(arg));
            index += 1;
            continue;
        }
        match arg {
            "--" => {
                positional_only = true;
                index += 1;
            }
            "--model_filename" | "--model" => {
                args.model = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--config" => {
                args.config = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--output_dir" | "--output-dir" | "--output" => {
                args.output_dir = PathBuf::from(value(argv, index, arg)?);
                index += 2;
            }
            "--model_file_dir" | "--model-file-dir" => {
                args.model_file_dir = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--output_format" | "--output-format" => {
                args.format = OutputFormat::parse(&value(argv, index, arg)?)?;
                index += 2;
            }
            "--output_bitrate" | "--output-bitrate" => {
                args.bitrate = Some(
                    value(argv, index, arg)?
                        .parse()
                        .map_err(|_| "bad --output_bitrate".to_string())?,
                );
                index += 2;
            }
            "--sample_rate" | "--sample-rate" => {
                args.sample_rate = Some(
                    value(argv, index, arg)?
                        .parse()
                        .map_err(|_| "bad --sample_rate".to_string())?,
                );
                index += 2;
            }
            "--single_stem" | "--single-stem" => {
                args.single_stem = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--custom_output_names" | "--custom-output-names" => {
                let json = value(argv, index, arg)?;
                args.custom_output_names =
                    serde_json::from_str::<HashMap<String, String>>(&json)
                        .map_err(|error| format!("invalid --custom_output_names JSON: {error}"))?;
                index += 2;
            }
            "--device" => {
                args.device = value(argv, index, arg)?;
                index += 2;
            }
            "--start" => {
                args.start = Some(
                    value(argv, index, arg)?
                        .parse()
                        .map_err(|_| "bad --start".to_string())?,
                );
                index += 2;
            }
            "--duration" => {
                args.duration = Some(
                    value(argv, index, arg)?
                        .parse()
                        .map_err(|_| "bad --duration".to_string())?,
                );
                index += 2;
            }
            "--normalization" | "--normalize" => {
                args.normalization = value(argv, index, arg)?
                    .parse()
                    .map_err(|_| "bad --normalization".to_string())?;
                index += 2;
            }
            "--amplification" => {
                args.amplification = value(argv, index, arg)?
                    .parse()
                    .map_err(|_| "bad --amplification".to_string())?;
                index += 2;
            }
            "--invert_spect" | "--invert-spect" => {
                args.invert_spect = true;
                index += 1;
            }
            "--transcribe" => {
                args.transcribe = true;
                index += 1;
            }
            "--whisper_model" => {
                args.whisper_model = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--whisper_config" => {
                args.whisper_config = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--whisper_tokenizer" => {
                args.whisper_tokenizer = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--whisper_model_dir" => {
                args.whisper_model_dir = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--language" => {
                args.language = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--env_info" | "--env-info" => {
                args.env_info = true;
                index += 1;
            }
            "--debug" => {
                args.debug = true;
                args.log_level = LevelFilter::Debug;
                index += 1;
            }
            "--log_level" | "--log-level" => {
                args.log_level = parse_level(&value(argv, index, arg)?)?;
                index += 2;
            }
            "--list_models" | "--list-models" => {
                args.list_models = true;
                index += 1;
            }
            "--list_filter" | "--list-filter" => {
                args.list_filter = Some(value(argv, index, arg)?);
                index += 2;
            }
            "--list_limit" | "--list-limit" => {
                args.list_limit = Some(
                    value(argv, index, arg)?
                        .parse()
                        .map_err(|_| "bad --list_limit".to_string())?,
                );
                index += 2;
            }
            "--list_format" | "--list-format" => {
                args.list_format = value(argv, index, arg)?;
                index += 2;
            }
            "--models_manifest" | "--models-manifest" => {
                args.models_manifest = Some(PathBuf::from(value(argv, index, arg)?));
                index += 2;
            }
            "--overwrite" => {
                args.overwrite = true;
                index += 1;
            }
            "--version" | "-V" => {
                println!("{}", VERSION);
                process::exit(0);
            }
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            other if other.starts_with('-') => return Err(format!("unknown argument: {other}")),
            other => {
                args.inputs.push(PathBuf::from(other));
                index += 1;
            }
        }
    }
    if args.list_format != "pretty" && args.list_format != "json" {
        return Err("--list_format must be pretty or json".into());
    }
    Ok(args)
}

fn resolve_in(dir: Option<&str>, name: &str) -> PathBuf {
    let path = Path::new(name);
    if path.is_absolute()
        || path
            .parent()
            .map(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(false)
    {
        return path.to_path_buf();
    }
    dir.map(|directory| Path::new(directory).join(name))
        .unwrap_or_else(|| path.to_path_buf())
}

fn env_info_report(
    model_dir: Option<&str>,
    model_file: Option<&str>,
    config_file: Option<&str>,
) -> String {
    let configured_model_dir = model_dir
        .map(str::to_owned)
        .or_else(|| env::var("CANDLE_ROFORMER_MODEL_DIR").ok());
    let resolved_model = model_file.map(|path| resolve_in(model_dir, path));
    let resolved_config = config_file
        .map(|path| resolve_in(model_dir, path))
        .or_else(|| {
            resolved_model
                .as_ref()
                .map(|path| path.with_extension("yaml"))
        });
    let parallelism = std::thread::available_parallelism()
        .map(|value| value.get().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let cpu = match DeviceChoice::Cpu.resolve() {
        Ok(_) => "available",
        Err(_) => "unavailable",
    };
    let mut lines = vec![
        format!("CLI version: {VERSION}"),
        format!("karaoke-cli package version: {VERSION}"),
        format!("candle-roformer version: {CANDLE_ROFORMER_VERSION}"),
        format!("candle-roformer revision: {CANDLE_ROFORMER_GIT_REVISION}"),
        format!("candle-core version: {CANDLE_CORE_VERSION}"),
        format!(
            "compiled features: cuda={}, metal={}",
            cfg!(feature = "cuda"),
            cfg!(feature = "metal")
        ),
        format!("CPU device: {cpu}"),
        format!("available parallelism: {parallelism}"),
        format!(
            "model search path: {}",
            configured_model_dir
                .as_deref()
                .unwrap_or("explicit local paths only; no implicit model downloads")
        ),
        format!(
            "resolved model checkpoint: {}",
            resolved_model
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "not specified (not required for --env_info)".into())
        ),
        format!(
            "resolved model config: {}",
            resolved_config
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "not specified (not required for --env_info)".into())
        ),
    ];
    if cfg!(feature = "cuda") {
        lines.push(format!(
            "CUDA device: {}",
            match DeviceChoice::Cuda.resolve() {
                Ok(_) => "available",
                Err(_) => "unavailable",
            }
        ));
    }
    if cfg!(feature = "metal") {
        lines.push(format!(
            "Metal device: {}",
            match DeviceChoice::Metal.resolve() {
                Ok(_) => "available",
                Err(_) => "unavailable",
            }
        ));
    }
    lines.join("\n") + "\n"
}

fn print_env_info(model_dir: Option<&str>, model_file: Option<&str>, config_file: Option<&str>) {
    print!("{}", env_info_report(model_dir, model_file, config_file));
}

fn list_models(args: &Args) -> Result<(), String> {
    let all = match &args.models_manifest {
        Some(path) => catalog::load_models(path)?,
        None => catalog::builtin_models()?,
    };
    let matches = catalog::list_models(&all, args.list_filter.as_deref(), args.list_limit);
    if matches.is_empty() {
        return Err("model catalog query selected zero models".into());
    }
    match args.list_format.as_str() {
        "pretty" => print!("{}", catalog::render_pretty(&matches)),
        "json" => println!("{}", catalog::render_json(&matches)?),
        _ => unreachable!(),
    }
    Ok(())
}

fn run() -> Result<i32, String> {
    let mut argv: Vec<String> = env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("separate") {
        argv.remove(0);
    }
    let args = parse_args(&argv)?;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(args.log_level);

    if args.env_info {
        print_env_info(
            args.model_file_dir.as_deref(),
            args.model.as_deref(),
            args.config.as_deref(),
        );
        return Ok(0);
    }
    if args.list_models {
        list_models(&args)?;
        return Ok(0);
    }
    if args.inputs.is_empty() {
        print_help();
        return Err("no input audio files or directories".into());
    }
    if args.transcribe
        && args.whisper_model_dir.is_none()
        && args.whisper_model.is_none()
        && args.whisper_config.is_none()
        && args.whisper_tokenizer.is_none()
    {
        return Err(
            "--transcribe requires --whisper_model_dir (or explicit Whisper asset paths)".into(),
        );
    }
    karaoke_cli::pipeline::validate_output_options(args.format, args.bitrate, args.sample_rate)?;

    let expansion = expand_inputs(&args.inputs);
    if expansion.files.is_empty() {
        let detail = if expansion.errors.is_empty() {
            "input expansion selected zero audio files".to_string()
        } else {
            expansion.errors.join("\n")
        };
        return Err(detail);
    }

    let model_name = args
        .model
        .as_deref()
        .ok_or_else(|| "--model_filename is required".to_string())?;
    let model_path = resolve_in(args.model_file_dir.as_deref(), model_name);
    let model_stem = model_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model")
        .to_string();
    let config_path = match &args.config {
        Some(config) => resolve_in(args.model_file_dir.as_deref(), config),
        None => model_path.with_extension("yaml"),
    };
    let device = match args.device.to_ascii_lowercase().as_str() {
        "cpu" => DeviceChoice::Cpu,
        "cuda" => DeviceChoice::Cuda,
        "metal" => DeviceChoice::Metal,
        other => return Err(format!("unknown device '{other}'")),
    };
    let separator_assets = LocalSeparationAssets::new(model_path.clone(), config_path);
    let separator = load_separator_with_options(
        &separator_assets,
        device,
        InferenceOverrides {
            normalization: args.normalization,
            amplification: args.amplification,
            invert_spect: args.invert_spect,
        },
    )
    .map_err(|error| format!("model {}: {error}", model_path.display()))?;

    let whisper = if args.transcribe {
        let dir = args.whisper_model_dir.as_deref();
        let resolve = |explicit: &Option<String>, default: &str| match explicit {
            Some(path) => resolve_in(dir, path),
            None => resolve_in(dir, default),
        };
        let assets = LocalWhisperAssets::new(
            resolve(&args.whisper_config, "config.json"),
            resolve(&args.whisper_tokenizer, "tokenizer.json"),
            resolve(&args.whisper_model, "model.safetensors"),
        );
        let language = match args.language.as_deref() {
            None | Some("auto") => None,
            Some(language) => Some(language.to_string()),
        };
        Some((
            assets,
            TranscribeOptions {
                language,
                ..Default::default()
            },
        ))
    } else {
        None
    };

    let selected_count = expansion.files.len();
    let discovery_failure_count = expansion.errors.len();
    let mut failures = expansion.errors;
    let batch = process_batch(&expansion.files, |input| {
        println!("separating {}", input.display());
        let request = FileRequest {
            input: input.to_path_buf(),
            output_dir: args.output_dir.clone(),
            model_stem: model_stem.clone(),
            overwrite: args.overwrite,
            custom_output_names: args.custom_output_names.clone(),
            single_stem: args.single_stem.clone(),
            format: args.format,
            bitrate: args.bitrate,
            sample_rate: args.sample_rate,
            start_seconds: args.start,
            limit_seconds: args.duration,
            whisper: whisper.clone(),
        };
        run_file(&separator, &request).map_err(|error| error.to_string())
    });
    let success_count = batch.succeeded;
    for (input, outcome) in batch.outcomes {
        match outcome {
            Ok(result) => {
                for stem in result.stems {
                    println!("{}: {}", stem.name, stem.path.display());
                }
                if let Some(text) = result.text {
                    println!("transcript: {}", text.display());
                }
                if let Some(json) = result.json {
                    println!("json: {}", json.display());
                }
            }
            Err(error) => {
                let failure = format!("{}: {error}", input.display());
                eprintln!("error: {failure}");
                failures.push(failure);
            }
        }
    }
    println!(
        "batch summary: {} input(s), {} succeeded, {} failed",
        selected_count + discovery_failure_count,
        success_count,
        failures.len()
    );
    if failures.is_empty() && success_count == 0 {
        return Err("batch executed zero files".into());
    }
    if !failures.is_empty() {
        eprintln!("failed inputs:");
        for failure in failures {
            eprintln!("  - {failure}");
        }
        return Ok(1);
    }
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(error) => {
            eprintln!("error: {error}");
            process::exit(2);
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn exact_ticket_flags_parse_and_custom_names_are_json() {
        let argv = vec![
            "--list_models".into(),
            "--list_filter".into(),
            "vocals".into(),
            "--list_limit".into(),
            "2".into(),
            "--list_format".into(),
            "json".into(),
            "--custom_output_names".into(),
            r#"{"vocals":"my-song"}"#.into(),
            "--single_stem".into(),
            "Vocals".into(),
        ];
        let parsed = parse_args(&argv).unwrap();
        assert!(parsed.list_models);
        assert_eq!(parsed.list_filter.as_deref(), Some("vocals"));
        assert_eq!(parsed.list_limit, Some(2));
        assert_eq!(parsed.list_format, "json");
        assert_eq!(parsed.custom_output_names["vocals"], "my-song");
        assert_eq!(parsed.single_stem.as_deref(), Some("Vocals"));
    }

    #[test]
    fn malformed_custom_names_json_is_rejected_with_context() {
        let error =
            parse_args(&["--custom_output_names".into(), r#"{"vocals":}"#.into()]).unwrap_err();
        assert!(error.contains("invalid --custom_output_names JSON"));
    }

    #[test]
    fn output_codec_and_resample_flags_parse_with_wav_default() {
        let defaults = parse_args(&[]).unwrap();
        assert_eq!(defaults.format, OutputFormat::Wav);
        assert_eq!(defaults.bitrate, None);
        assert_eq!(defaults.sample_rate, None);

        let parsed = parse_args(&[
            "--output_format".into(),
            "mp3".into(),
            "--output_bitrate".into(),
            "256".into(),
            "--sample_rate".into(),
            "48000".into(),
        ])
        .unwrap();
        assert_eq!(parsed.format, OutputFormat::Mp3);
        assert_eq!(parsed.bitrate, Some(256));
        assert_eq!(parsed.sample_rate, Some(48_000));
        assert!(karaoke_cli::pipeline::validate_output_options(
            parsed.format,
            parsed.bitrate,
            parsed.sample_rate
        )
        .is_ok());
    }

    #[test]
    fn level_flags_match_reference_defaults_and_parse_explicit_values() {
        let defaults = parse_args(&[]).unwrap();
        assert_eq!(defaults.normalization, 0.9);
        assert_eq!(defaults.amplification, 0.0);
        assert!(!defaults.invert_spect);
        let inference_defaults = InferenceOverrides::default();
        assert_eq!(inference_defaults.normalization, 0.9);
        assert_eq!(inference_defaults.amplification, 0.0);
        assert!(!inference_defaults.invert_spect);

        let explicit = parse_args(&[
            "--normalization".into(),
            "0".into(),
            "--amplification".into(),
            "0.25".into(),
            "--invert_spect".into(),
        ])
        .unwrap();
        assert_eq!(explicit.normalization, 0.0);
        assert_eq!(explicit.amplification, 0.25);
        assert!(explicit.invert_spect);
    }

    #[test]
    fn debug_is_an_info_backend_alias_and_invalid_levels_fail() {
        assert_eq!(parse_level("info").unwrap(), LevelFilter::Info);
        assert!(parse_level("verboseish").is_err());
        assert_eq!(parse_args(&[]).unwrap().log_level, LevelFilter::Info);
        assert_eq!(
            parse_args(&["--log_level".into(), "warn".into()])
                .unwrap()
                .log_level,
            LevelFilter::Warn
        );
        let parsed = parse_args(&["--debug".into()]).unwrap();
        assert!(parsed.debug);
        assert_eq!(parsed.log_level, LevelFilter::Debug);
    }

    #[test]
    fn env_info_reports_versions_features_and_resolved_paths_without_loading_assets() {
        let report = env_info_report(Some("/tmp/model-root"), Some("separator.safetensors"), None);
        assert!(report.contains(&format!("CLI version: {VERSION}")));
        assert!(report.contains(&format!(
            "candle-roformer version: {CANDLE_ROFORMER_VERSION}"
        )));
        assert!(report.contains(&format!("candle-core version: {CANDLE_CORE_VERSION}")));
        assert!(report.contains(&format!(
            "compiled features: cuda={}, metal={}",
            cfg!(feature = "cuda"),
            cfg!(feature = "metal")
        )));
        assert!(report.contains("available parallelism:"));
        assert!(report.contains("CPU device: available"));
        assert!(report.contains("resolved model checkpoint: /tmp/model-root/separator.safetensors"));
        assert!(report.contains("resolved model config: /tmp/model-root/separator.yaml"));
        assert!(!report.contains("API_KEY"));
    }
}
