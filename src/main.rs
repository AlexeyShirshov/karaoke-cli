use std::collections::HashMap;
use std::env;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use candle_roformer::device::DeviceChoice;
use candle_roformer::whisper::TranscribeOptions;
use candle_roformer::{LocalSeparationAssets, LocalWhisperAssets};
use karaoke_cli::catalog;
use karaoke_cli::pipeline::{
    expand_inputs, load_separator_with_options, process_batch, run_file_with_chunks, ChunkConfig,
    ChunkProgress, FileRequest, InferenceOverrides, OutputFormat,
};
use karaoke_cli::status::{format_duration, Activity};
use log::{LevelFilter, Log, Metadata, Record};

mod wizard;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const CANDLE_ROFORMER_VERSION: &str = env!("CANDLE_ROFORMER_VERSION");
const CANDLE_ROFORMER_SOURCE: &str = env!("CANDLE_ROFORMER_SOURCE");
const CANDLE_CORE_VERSION: &str = env!("CANDLE_CORE_VERSION");

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
        eprintln!(
            "{} {} {}: {}",
            chrono::Local::now().format("%H:%M:%S"),
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
    chunk_seconds: Option<f64>,
    chunk_overlap: Option<f64>,
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
    download_models: bool,
    whisper_size: String,
}

fn version_banner() -> String {
    format!("karaoke-cli {VERSION} (candle-roformer {CANDLE_ROFORMER_VERSION})")
}

fn print_help() {
    println!("{}", version_banner());
    println!();
    println!("Usage:");
    println!("  karaoke-cli [audio files or directories...] --model_filename <file.safetensors> [options]");
    println!("  karaoke-cli              Interactive wizard (terminal only)");
    println!("  karaoke-cli --wizard     Explicit interactive wizard");
    println!();
    println!("Options:");
    println!(
        "  --model_filename <name|file>  Catalog model name or local checkpoint (alias: --model)"
    );
    println!(
        "  --config <file.yaml>      Model profile (default: <model-stem>.yaml next to model)"
    );
    println!("  --model_file_dir <dir>    Directory for relative model/config paths");
    println!("  --download_models        Download missing catalog assets into ./models (opt-in)");
    println!("                           With no audio inputs: download/prepare models only");
    println!("  --output_dir <dir>        Output directory (default: out)");
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
    println!(
        "  --chunk_seconds <seconds> Separate in overlapping chunks and report per-chunk progress"
    );
    println!("  --chunk_overlap <seconds> Crossfade width (default: 10% of the chunk, 0.25..5 s)");
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
    println!(
        "  --whisper_size <size>    Download variant (default: small); requires --download_models"
    );
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
        output_dir: PathBuf::from("out"),
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
        chunk_seconds: None,
        chunk_overlap: None,
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
        download_models: false,
        whisper_size: "small".into(),
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
            "--download_models" | "--download-models" => {
                args.download_models = true;
                index += 1;
            }
            "--whisper_size" | "--whisper-size" => {
                args.whisper_size = value(argv, index, arg)?;
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
            "--chunk_seconds" | "--chunk-seconds" => {
                args.chunk_seconds = Some(
                    value(argv, index, arg)?
                        .parse()
                        .map_err(|_| "bad --chunk_seconds".to_string())?,
                );
                index += 2;
            }
            "--chunk_overlap" | "--chunk-overlap" => {
                args.chunk_overlap = Some(
                    value(argv, index, arg)?
                        .parse()
                        .map_err(|_| "bad --chunk_overlap".to_string())?,
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
                println!("{}", version_banner());
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

fn separation_model_dir(args: &Args) -> Option<&str> {
    args.model_file_dir.as_deref().or(if args.download_models {
        Some("models")
    } else {
        None
    })
}

fn separation_paths(
    args: &Args,
    models: &[catalog::ModelInfo],
) -> Result<(PathBuf, PathBuf), String> {
    let requested = args
        .model
        .as_deref()
        .ok_or("--model_filename is required")?;
    let path = Path::new(requested);
    let bare = path
        .parent()
        .is_none_or(|parent| parent.as_os_str().is_empty());
    let known = catalog::find_model(models, requested)?;
    let short_name = bare && known.is_some_and(|model| requested != model.filename);
    let filename = if short_name {
        known.expect("resolved short name").filename.as_str()
    } else {
        requested
    };
    let directory = separation_model_dir(args).or(if short_name { Some("models") } else { None });
    let model_path = resolve_in(directory, filename);
    let config_path = match &args.config {
        Some(config) => resolve_in(directory, config),
        None => match known.and_then(|model| model.config_filename.as_deref()) {
            Some(config) => model_path.with_file_name(config),
            None => model_path.with_extension("yaml"),
        },
    };
    Ok((model_path, config_path))
}

fn whisper_paths(args: &Args) -> (PathBuf, PathBuf, PathBuf) {
    let default_dir = format!("models/whisper-{}", args.whisper_size);
    let dir = args
        .whisper_model_dir
        .as_deref()
        .or(if args.download_models {
            Some(default_dir.as_str())
        } else {
            None
        });
    let resolve = |explicit: &Option<String>, default: &str| {
        resolve_in(dir, explicit.as_deref().unwrap_or(default))
    };
    (
        resolve(&args.whisper_config, "config.json"),
        resolve(&args.whisper_tokenizer, "tokenizer.json"),
        resolve(&args.whisper_model, "model.safetensors"),
    )
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
        format!("candle-roformer version: {CANDLE_ROFORMER_VERSION} ({CANDLE_ROFORMER_SOURCE})"),
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
    let interactive = argv == ["--wizard"]
        || (argv.is_empty() && io::stdin().is_terminal() && io::stdout().is_terminal());
    if interactive {
        let executable = env::current_exe().map_err(|error| error.to_string())?;
        argv = match wizard::run(
            io::stdin().lock(),
            io::stdout().lock(),
            &version_banner(),
            &executable.to_string_lossy(),
        )? {
            Some(argv) => argv,
            None => return Ok(0),
        };
    }
    if argv.first().map(String::as_str) == Some("separate") {
        argv.remove(0);
    }
    let args = parse_args(&argv)?;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(args.log_level);

    if args.env_info {
        let paths = if args.model.is_some() {
            let models = match &args.models_manifest {
                Some(path) => catalog::load_models(path)?,
                None => catalog::builtin_models()?,
            };
            Some(separation_paths(&args, &models)?)
        } else {
            None
        };
        print_env_info(
            separation_model_dir(&args),
            paths.as_ref().and_then(|paths| paths.0.to_str()),
            paths
                .as_ref()
                .and_then(|paths| paths.1.to_str())
                .or(args.config.as_deref()),
        );
        return Ok(0);
    }
    if args.list_models {
        list_models(&args)?;
        return Ok(0);
    }
    if args.inputs.is_empty() && !args.download_models {
        print_help();
        return Err("no input audio files or directories".into());
    }
    if args.transcribe
        && !args.download_models
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
    if expansion.files.is_empty() && !args.inputs.is_empty() {
        let detail = if expansion.errors.is_empty() {
            "input expansion selected zero audio files".to_string()
        } else {
            expansion.errors.join("\n")
        };
        return Err(detail);
    }

    let models = match &args.models_manifest {
        Some(path) => catalog::load_models(path)?,
        None => catalog::builtin_models()?,
    };
    let (model_path, config_path) = separation_paths(&args, &models)?;
    let model_stem = model_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model")
        .to_string();
    let device = match args.device.to_ascii_lowercase().as_str() {
        "cpu" => DeviceChoice::Cpu,
        "cuda" => DeviceChoice::Cuda,
        "metal" => DeviceChoice::Metal,
        other => return Err(format!("unknown device '{other}'")),
    };
    let chunk_config = match args.chunk_seconds {
        Some(chunk_seconds) => {
            let config = ChunkConfig::new(
                chunk_seconds,
                args.chunk_overlap,
                args.normalization,
                args.amplification,
            );
            config.validate()?;
            Some(config)
        }
        None => None,
    };
    // With chunking the CLI applies amplification and normalization once over
    // the whole file; leaving them in the model would renormalise every chunk.
    let model_levels = chunk_config.is_none();
    let whisper_paths = args.transcribe.then(|| whisper_paths(&args));
    if args.download_models {
        if args.transcribe
            && !karaoke_cli::download::WHISPER_SIZES.contains(&args.whisper_size.as_str())
        {
            return Err(format!(
                "unsupported --whisper_size '{}'",
                args.whisper_size
            ));
        }
        let downloader = karaoke_cli::download::Downloader::new()?;
        downloader.separator(&models, &model_path, &config_path)?;
        if let Some((config, tokenizer, weights)) = &whisper_paths {
            downloader.whisper(&args.whisper_size, config, tokenizer, weights)?;
        }
    }
    if args.inputs.is_empty() {
        log::info!("requested models are ready");
        return Ok(0);
    }
    let separator_assets = LocalSeparationAssets::new(model_path.clone(), config_path);
    log::info!(
        "loading separation model (device={device:?}): {}",
        model_path.display()
    );
    let loading_activity = Activity::start("loading separation model");
    let separator = load_separator_with_options(
        &separator_assets,
        device,
        InferenceOverrides {
            normalization: if model_levels {
                args.normalization
            } else {
                0.0
            },
            amplification: if model_levels {
                args.amplification
            } else {
                0.0
            },
            invert_spect: args.invert_spect,
        },
    )
    .map_err(|error| format!("model {}: {error}", model_path.display()))?;
    drop(loading_activity);
    log::info!("separation model loaded");

    let whisper = if let Some((config, tokenizer, weights)) = whisper_paths {
        let assets = LocalWhisperAssets::new(config, tokenizer, weights);
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
        let mut reporter = |chunk: ChunkProgress| {
            let percent = if chunk.total == 0 {
                100.0
            } else {
                chunk.index as f64 * 100.0 / chunk.total as f64
            };
            eprintln!(
                "  chunk {}/{} done ({percent:.0}%, {} elapsed)",
                chunk.index,
                chunk.total,
                format_duration(Duration::from_secs_f64(chunk.elapsed_seconds))
            );
        };
        run_file_with_chunks(&separator, &request, chunk_config.as_ref(), &mut reporter)
            .map_err(|error| error.to_string())
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
    #[cfg(all(windows, feature = "cuda"))]
    if let Err(error) = configure_cuda_dll_directory() {
        eprintln!("error: {error}");
        process::exit(2);
    }

    match run() {
        Ok(code) => process::exit(code),
        Err(error) => {
            eprintln!("error: {error}");
            process::exit(2);
        }
    }
}

// nvcc registers its fat binaries from C++ static constructors, before main.
// MSVC sorts XCT before the normal XCU constructors, so delay-loaded cudart can
// already find the portable bin directory during that registration.
#[cfg(all(windows, target_env = "msvc", feature = "cuda"))]
#[used]
#[link_section = ".CRT$XCT"]
static EARLY_CUDA_DLL_DIRECTORY: extern "C" fn() = configure_cuda_dll_directory_early;

#[cfg(all(windows, target_env = "msvc", feature = "cuda"))]
extern "C" fn configure_cuda_dll_directory_early() {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleFileNameW(module: *mut std::ffi::c_void, path: *mut u16, size: u32) -> u32;
        fn GetFileAttributesW(path: *const u16) -> u32;
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }
    // No allocation or Rust runtime facilities before main. The buffer covers
    // Windows' maximum extended-length path, including the appended bin name.
    let mut path = [0_u16; 32768];
    // SAFETY: the writable buffer has the stated size; NULL selects this EXE.
    let len =
        unsafe { GetModuleFileNameW(std::ptr::null_mut(), path.as_mut_ptr(), path.len() as u32) }
            as usize;
    if len == 0 || len >= path.len() {
        return;
    }
    let Some(separator) = path[..len]
        .iter()
        .rposition(|unit| *unit == 92 || *unit == 47)
    else {
        return;
    };
    let start = separator + 1;
    if start + 4 > path.len() {
        return;
    }
    path[start..start + 4].copy_from_slice(&[98, 105, 110, 0]); // bin\0
                                                                // SAFETY: path is an absolute, NUL-terminated UTF-16 string. These calls
                                                                // neither load CUDA nor rely on Rust initialization.
    unsafe {
        let attributes = GetFileAttributesW(path.as_ptr());
        if attributes != u32::MAX && attributes & 0x10 != 0 {
            SetDllDirectoryW(path.as_ptr());
        }
    }
}

#[cfg(all(windows, feature = "cuda"))]
fn configure_cuda_dll_directory() -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }

    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let bin = executable
        .parent()
        .ok_or("cannot locate executable directory")?
        .join("bin");
    // Source builds may use the installed Toolkit instead of a portable bundle.
    if !bin.is_dir() {
        return Ok(());
    }
    let path: Vec<u16> = bin.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: path is an absolute, NUL-terminated UTF-16 string that remains
    // alive for the call. Early initialization handles nvcc's constructors;
    // this also reports errors normally after the Rust runtime is initialized.
    if unsafe { SetDllDirectoryW(path.as_ptr()) } == 0 {
        return Err(format!(
            "cannot configure CUDA DLL directory {}: {}",
            bin.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn output_directory_defaults_to_out_and_allows_explicit_override() {
        assert_eq!(parse_args(&[]).unwrap().output_dir, PathBuf::from("out"));
        for flag in ["--output_dir", "--output-dir", "--output"] {
            let args = parse_args(&[flag.into(), ".".into()]).unwrap();
            assert_eq!(args.output_dir, PathBuf::from("."));
        }
    }

    #[test]
    fn catalog_names_resolve_weights_and_matching_profiles_automatically() {
        let models = catalog::builtin_models().unwrap();
        for flag in [false, true] {
            let mut argv = vec!["--model".into(), "mbr_instfv6n_gabox".into()];
            if flag {
                argv.push("--download_models".into());
            }
            let mut args = parse_args(&argv).unwrap();
            let paths = separation_paths(&args, &models).unwrap();
            assert_eq!(
                paths.0,
                PathBuf::from("models/mbr_instfv6n_gabox.safetensors")
            );
            assert_eq!(
                paths.1,
                PathBuf::from("models/mbr_instfv6n_gabox_config.yaml")
            );
            args.model_file_dir = Some("custom".into());
            let paths = separation_paths(&args, &models).unwrap();
            assert_eq!(
                paths.0,
                PathBuf::from("custom/mbr_instfv6n_gabox.safetensors")
            );
            assert_eq!(
                paths.1,
                PathBuf::from("custom/mbr_instfv6n_gabox_config.yaml")
            );
            args.model = Some("explicit/mbr_instfv6n_gabox.safetensors".into());
            let paths = separation_paths(&args, &models).unwrap();
            assert_eq!(
                paths.0,
                PathBuf::from("explicit/mbr_instfv6n_gabox.safetensors")
            );
            assert_eq!(
                paths.1,
                PathBuf::from("explicit/mbr_instfv6n_gabox_config.yaml")
            );
        }
    }

    #[test]
    fn download_flags_default_to_models_without_changing_offline_paths() {
        let offline = parse_args(&[]).unwrap();
        assert!(!offline.download_models);
        assert_eq!(separation_model_dir(&offline), None);
        assert_eq!(whisper_paths(&offline).0, PathBuf::from("config.json"));

        let mut args = parse_args(&[
            "--download_models".into(),
            "--whisper_size".into(),
            "tiny".into(),
        ])
        .unwrap();
        assert_eq!(separation_model_dir(&args), Some("models"));
        let paths = whisper_paths(&args);
        assert_eq!(paths.0, PathBuf::from("models/whisper-tiny/config.json"));
        assert_eq!(paths.1, PathBuf::from("models/whisper-tiny/tokenizer.json"));
        assert_eq!(
            paths.2,
            PathBuf::from("models/whisper-tiny/model.safetensors")
        );
        args.model_file_dir = Some("custom-models".into());
        args.whisper_model_dir = Some("custom-whisper".into());
        assert_eq!(separation_model_dir(&args), Some("custom-models"));
        assert_eq!(
            whisper_paths(&args).0,
            PathBuf::from("custom-whisper/config.json")
        );
        args.whisper_config = Some("explicit/config.json".into());
        assert_eq!(
            whisper_paths(&args).0,
            PathBuf::from("explicit/config.json")
        );
    }

    fn cargo_lock_field(package_name: &str, field: &str) -> String {
        let prefix = format!("{field} = \"");
        include_str!("../Cargo.lock")
            .split("[[package]]")
            .skip(1)
            .find(|package| {
                package
                    .lines()
                    .any(|line| line.trim() == format!("name = \"{package_name}\""))
            })
            .and_then(|package| {
                package.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix(&prefix)
                        .and_then(|value| value.strip_suffix('\"'))
                        .map(str::to_owned)
                })
            })
            .unwrap_or_else(|| panic!("Cargo.lock has no {field} for {package_name}"))
    }

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
    fn chunk_flags_parse_and_reject_unsafe_geometry() {
        let parsed = parse_args(&[
            "song.wav".into(),
            "--chunk_seconds".into(),
            "45".into(),
            "--chunk_overlap".into(),
            "2.5".into(),
        ])
        .unwrap();
        assert_eq!(parsed.chunk_seconds, Some(45.0));
        assert_eq!(parsed.chunk_overlap, Some(2.5));

        let default_overlap = parse_args(&["--chunk_seconds".into(), "60".into()]).unwrap();
        assert_eq!(default_overlap.chunk_overlap, None);
        assert_eq!(
            karaoke_cli::pipeline::default_chunk_overlap(60.0),
            5.0,
            "default crossfade must be capped"
        );

        assert!(parse_args(&["--chunk_seconds".into(), "fast".into()]).is_err());
        assert!(parse_args(&["--chunk_overlap".into()]).is_err());
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
        let locked_roformer_version = cargo_lock_field("candle-roformer", "version");
        let locked_roformer_source = cargo_lock_field("candle-roformer", "source");
        let locked_core_version = cargo_lock_field("candle-core", "version");
        let source_label =
            if locked_roformer_source == "registry+https://github.com/rust-lang/crates.io-index" {
                "crates.io registry"
            } else if locked_roformer_source.starts_with("registry+") {
                "registry"
            } else if locked_roformer_source.starts_with("git+") {
                "git"
            } else {
                "other source"
            };
        assert!(report.contains(&format!("CLI version: {VERSION}")));
        assert!(report.contains(&format!(
            "candle-roformer version: {locked_roformer_version} ({source_label})"
        )));
        assert_eq!(CANDLE_ROFORMER_VERSION, locked_roformer_version);
        assert_eq!(CANDLE_ROFORMER_SOURCE, source_label);
        assert!(report.contains(&format!("candle-core version: {locked_core_version}")));
        assert_eq!(CANDLE_CORE_VERSION, locked_core_version);
        assert!(!report.contains("candle-roformer revision:"));
        assert!(report.contains(&format!(
            "compiled features: cuda={}, metal={}",
            cfg!(feature = "cuda"),
            cfg!(feature = "metal")
        )));
        assert!(report.contains("available parallelism:"));
        assert!(report.contains("CPU device: available"));
        let expected_checkpoint = std::path::Path::new("/tmp/model-root")
            .join("separator.safetensors")
            .display()
            .to_string();
        let expected_config = std::path::Path::new("/tmp/model-root")
            .join("separator.yaml")
            .display()
            .to_string();
        assert!(report.contains(&format!("resolved model checkpoint: {expected_checkpoint}")));
        assert!(report.contains(&format!("resolved model config: {expected_config}")));
        assert!(!report.contains("API_KEY"));
    }
}
