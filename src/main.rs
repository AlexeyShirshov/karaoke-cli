use std::env;
use std::path::{Path, PathBuf};
use std::process;

use candle_roformer::device::DeviceChoice;
use candle_roformer::whisper::TranscribeOptions;
use candle_roformer::{LocalSeparationAssets, LocalWhisperAssets};
use karaoke_cli::pipeline::{
    run_pipeline_named, separate_and_transcribe_file, OutputNames, TranscriptionRequest,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!("karaoke-cli {}", VERSION);
    println!();
    println!("Usage:");
    println!("  karaoke-cli [audio files...] --model_filename <file.safetensors> [options]");
    println!();
    println!("Options:");
    println!("  --model_filename <file>   MelBandRoformer safetensors checkpoint (or use --model)");
    println!(
        "  --config <file.yaml>      Model profile (default: <model-stem>.yaml next to the model)"
    );
    println!("  --model_file_dir <dir>    Directory to resolve --model_filename and --config in");
    println!("  --output_dir <dir>        Output directory (default: .)");
    println!("  --output_format <fmt>     wav (default; other formats are not implemented yet)");
    println!("  --single_stem             Write only the vocals stem");
    println!("  --device cpu|cuda|metal   Device (default: cpu); unavailable device is an error");
    println!("  --start <seconds>         Start offset in the input");
    println!("  --duration <seconds>      Process only this many seconds");
    println!("  --overwrite               Replace existing outputs");
    println!();
    println!("Transcription (Whisper, optional):");
    println!("  --transcribe              Also transcribe the vocal stem (.txt + .json)");
    println!("  --whisper_model_dir <dir>  Directory with config.json, tokenizer.json, model.safetensors");
    println!("  --whisper_model <file>    Override model.safetensors (local path)");
    println!("  --whisper_config <file>   Override config.json (local path)");
    println!("  --whisper_tokenizer <file> Override tokenizer.json (local path)");
    println!("  --language <auto|code>    Whisper language (default: auto)");
    println!("  --version | --help");
}

struct Args {
    files: Vec<String>,
    model: Option<String>,
    config: Option<String>,
    output_dir: String,
    model_file_dir: Option<String>,
    format: String,
    single_stem: bool,
    device: String,
    overwrite: bool,
    start: Option<f64>,
    duration: Option<f64>,
    transcribe: bool,
    whisper_model: Option<String>,
    whisper_config: Option<String>,
    whisper_tokenizer: Option<String>,
    whisper_model_dir: Option<String>,
    language: Option<String>,
}

fn value(argv: &[String], i: usize, name: &str) -> Result<String, String> {
    argv.get(i + 1)
        .cloned()
        .ok_or_else(|| format!("{} requires a value", name))
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut a = Args {
        files: Vec::new(),
        model: None,
        config: None,
        output_dir: ".".to_string(),
        model_file_dir: None,
        format: "wav".to_string(),
        single_stem: false,
        device: "cpu".to_string(),
        overwrite: false,
        start: None,
        duration: None,
        transcribe: false,
        whisper_model: None,
        whisper_config: None,
        whisper_tokenizer: None,
        whisper_model_dir: None,
        language: None,
    };
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        match arg {
            "--model_filename" | "--model" => {
                a.model = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--config" => {
                a.config = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--output_dir" | "--output" => {
                a.output_dir = value(argv, i, arg)?;
                i += 2;
            }
            "--model_file_dir" => {
                a.model_file_dir = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--output_format" => {
                a.format = value(argv, i, arg)?;
                i += 2;
            }
            "--device" => {
                a.device = value(argv, i, arg)?;
                i += 2;
            }
            "--start" => {
                a.start = Some(
                    value(argv, i, arg)?
                        .parse()
                        .map_err(|_| "bad --start".to_string())?,
                );
                i += 2;
            }
            "--duration" => {
                a.duration = Some(
                    value(argv, i, arg)?
                        .parse()
                        .map_err(|_| "bad --duration".to_string())?,
                );
                i += 2;
            }
            "--transcribe" => {
                a.transcribe = true;
                i += 1;
            }
            "--whisper_model" => {
                a.whisper_model = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--whisper_config" => {
                a.whisper_config = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--whisper_tokenizer" => {
                a.whisper_tokenizer = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--whisper_model_dir" => {
                a.whisper_model_dir = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--language" => {
                a.language = Some(value(argv, i, arg)?);
                i += 2;
            }
            "--single_stem" => {
                a.single_stem = true;
                i += 1;
            }
            "--overwrite" => {
                a.overwrite = true;
                i += 1;
            }
            "--version" | "-V" => {
                println!("{}", VERSION);
                process::exit(0);
            }
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            other if other.starts_with('-') => return Err(format!("unknown arg: {}", other)),
            other => {
                a.files.push(other.to_string());
                i += 1;
            }
        }
    }
    Ok(a)
}

fn resolve_in(dir: Option<&str>, name: &str) -> PathBuf {
    let p = Path::new(name);
    if p.is_absolute()
        || p.parent()
            .map(|x| !x.as_os_str().is_empty())
            .unwrap_or(false)
    {
        return p.to_path_buf();
    }
    match dir {
        Some(d) => Path::new(d).join(name),
        None => p.to_path_buf(),
    }
}

fn main() {
    // `separate` is accepted as a legacy prefix.
    let mut argv: Vec<String> = env::args().skip(1).collect();
    if argv.first().map(|s| s.as_str()) == Some("separate") {
        argv.remove(0);
    }
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {}", e);
            print_help();
            process::exit(2);
        }
    };
    if argv.is_empty() {
        print_help();
        process::exit(0);
    }
    if args.files.is_empty() {
        eprintln!("error: no input audio files");
        process::exit(2);
    }
    if args.format.to_ascii_lowercase() != "wav" {
        eprintln!(
            "error: output format '{}' is not implemented yet; only wav is supported",
            args.format
        );
        process::exit(2);
    }
    let model_name = match &args.model {
        Some(m) => m.clone(),
        None => {
            eprintln!("error: --model_filename is required");
            process::exit(2);
        }
    };
    let model_path = resolve_in(args.model_file_dir.as_deref(), &model_name);
    let model_stem = model_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model")
        .to_string();
    let config_path = match &args.config {
        Some(c) => resolve_in(args.model_file_dir.as_deref(), c),
        None => model_path.with_extension("yaml"),
    };
    let device = match args.device.to_ascii_lowercase().as_str() {
        "cpu" => DeviceChoice::Cpu,
        "cuda" => DeviceChoice::Cuda,
        "metal" => DeviceChoice::Metal,
        other => {
            eprintln!("error: unknown device '{}'", other);
            process::exit(2);
        }
    };

    let whisper_assets = if args.transcribe {
        let dir = args.whisper_model_dir.as_deref();
        if dir.is_none()
            && args.whisper_model.is_none()
            && args.whisper_config.is_none()
            && args.whisper_tokenizer.is_none()
        {
            eprintln!(
                "error: --transcribe requires --whisper_model_dir (or explicit whisper asset paths)"
            );
            process::exit(2);
        }
        let resolve = |explicit: &Option<String>, default: &str| match explicit {
            Some(path) => resolve_in(dir, path),
            None => resolve_in(dir, default),
        };
        Some(LocalWhisperAssets::new(
            resolve(&args.whisper_config, "config.json"),
            resolve(&args.whisper_tokenizer, "tokenizer.json"),
            resolve(&args.whisper_model, "model.safetensors"),
        ))
    } else {
        None
    };
    let language = match args.language.as_deref() {
        None => None,
        Some(code) if code.eq_ignore_ascii_case("auto") => None,
        Some(code) => Some(code.to_string()),
    };

    for input in &args.files {
        let stem = Path::new(input)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let vocals = format!("{}_(Vocals)_{}.wav", stem, model_stem);
        let instrumental = if args.single_stem {
            None
        } else {
            Some(format!("{}_(Instrumental)_{}.wav", stem, model_stem))
        };
        println!("separating {} -> {}/{}", input, args.output_dir, vocals);
        if let Some(whisper) = &whisper_assets {
            let request = TranscriptionRequest {
                separator: LocalSeparationAssets::new(model_path.clone(), config_path.clone()),
                whisper: whisper.clone(),
                input: PathBuf::from(input),
                output_dir: PathBuf::from(&args.output_dir),
                device,
                transcribe_options: TranscribeOptions {
                    language: language.clone(),
                    ..Default::default()
                },
                overwrite: args.overwrite,
                start_seconds: args.start,
                limit_seconds: args.duration,
                single_stem: args.single_stem,
            };
            match separate_and_transcribe_file(&request) {
                Ok(out) => {
                    let language = out.transcript.language.as_deref().unwrap_or("unknown");
                    println!(
                        "transcribed {} segments (language {}, {:.1}s) -> {}",
                        out.transcript.segments.len(),
                        language,
                        out.duration_seconds,
                        out.text.display()
                    );
                    if let Some(instrumental) = &out.instrumental {
                        println!("instrumental: {}", instrumental.display());
                    }
                    println!("json:         {}", out.json.display());
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    process::exit(1);
                }
            }
        } else {
            let names = OutputNames {
                vocals: &vocals,
                instrumental: instrumental.as_deref(),
            };
            if let Err(e) = run_pipeline_named(
                model_path.to_str().unwrap_or_default(),
                config_path.to_str().unwrap_or_default(),
                input,
                &args.output_dir,
                device,
                args.overwrite,
                args.start,
                args.duration,
                &names,
            ) {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
    }
}
