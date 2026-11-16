//! File-level flow tests: output naming, sidecars, preflight, window trimming.
//!
//! The real checkpoints are not present in this environment, so the end-to-end
//! test builds a tiny randomly-initialised separator and Whisper model. That
//! proves the naming, staging/rename and sidecar plumbing executes; it says
//! nothing about separation or transcription quality (see the crate's
//! `docs/gates-report.md`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use candle_core::{DType, Device};
use candle_nn::{VarBuilder, VarMap};
use candle_roformer::config::SeparatorConfig;
use candle_roformer::model::MelBandRoformer;
use candle_roformer::wav::{read_audio, write_wav_f32, AudioBuffer};
use candle_roformer::whisper::TranscribeOptions;
use candle_roformer::{DeviceChoice, LocalSeparationAssets, LocalWhisperAssets};
use karaoke_cli::pipeline::{
    expand_inputs, format_timestamp, load_separator_with_options, process_batch, run_file,
    run_file_with_chunks, separate_and_transcribe_file, trim_window, ChunkConfig, FileRequest,
    InferenceOverrides, OutputFormat, TranscriptionRequest,
};

fn tmpdir(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("kc_test_{}", name));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

/// A minimal Whisper-shaped tokenizer: three text tokens, the required special
/// tokens, and 20 timestamp tokens. Text ids are below `timestamp_begin`, as in
/// the real tokenizer.
fn write_tokenizer(dir: &Path) -> PathBuf {
    use serde_json::json;

    let mut added: Vec<(u32, String)> = vec![
        (3, "<|endoftext|>".to_string()),
        (4, "<|startoftranscript|>".to_string()),
        (5, "<|transcribe|>".to_string()),
        (6, "<|nocaptions|>".to_string()),
        (7, "<|notimestamps|>".to_string()),
    ];
    for i in 0..20u32 {
        added.push((8 + i, format!("<|{:.2}|>", i as f32 / 50.0)));
    }
    let added_tokens: Vec<_> = added
        .iter()
        .map(|(id, content)| {
            json!({
                "id": id,
                "content": content,
                "single_word": false,
                "lstrip": false,
                "rstrip": false,
                "normalized": false,
                "special": true
            })
        })
        .collect();

    let tokenizer = json!({
        "version": "1.0",
        "truncation": null,
        "padding": null,
        "added_tokens": added_tokens,
        "normalizer": null,
        "pre_tokenizer": {"type": "Whitespace"},
        "post_processor": null,
        "decoder": null,
        "model": {
            "type": "WordLevel",
            "vocab": {"<unk>": 0, "hello": 1, "world": 2},
            "unk_token": "<unk>"
        }
    });
    let path = dir.join("tokenizer.json");
    fs::write(&path, serde_json::to_string(&tokenizer).unwrap()).unwrap();
    path
}

/// A tiny model with the same tensor layout as a real Whisper checkpoint.
fn write_model(dir: &Path) -> (PathBuf, PathBuf) {
    use candle_transformers::models::whisper::{model::Whisper, Config};

    let config = Config {
        num_mel_bins: 80,
        max_source_positions: 1500,
        d_model: 16,
        encoder_attention_heads: 2,
        encoder_layers: 1,
        vocab_size: 28,
        max_target_positions: 30,
        decoder_attention_heads: 2,
        decoder_layers: 1,
        suppress_tokens: vec![],
    };
    let config_path = dir.join("config.json");
    let config_json = serde_json::json!({
        "num_mel_bins": 80,
        "max_source_positions": 1500,
        "d_model": 16,
        "encoder_attention_heads": 2,
        "encoder_layers": 1,
        "vocab_size": 28,
        "max_target_positions": 30,
        "decoder_attention_heads": 2,
        "decoder_layers": 1,
        "suppress_tokens": []
    });
    fs::write(&config_path, serde_json::to_string(&config_json).unwrap()).unwrap();

    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let _ = Whisper::load(&vb, config).unwrap();
    let model_path = dir.join("model.safetensors");
    varmap.save(&model_path).unwrap();
    (config_path, model_path)
}

fn synth_whisper_assets(name: &str) -> LocalWhisperAssets {
    let dir = tmpdir(name);
    let config = write_model(&dir);
    let tokenizer = write_tokenizer(&dir);
    LocalWhisperAssets::new(config.0, tokenizer, config.1)
}

fn tiny_separator_assets(dir: &Path) -> LocalSeparationAssets {
    tiny_separator_assets_with_stems(dir, 1)
}

fn tiny_separator_assets_with_stems(dir: &Path, num_stems: usize) -> LocalSeparationAssets {
    let cfg = SeparatorConfig {
        dim: 8,
        depth: 1,
        stereo: true,
        num_stems,
        num_bands: 4,
        family: None,
        freqs_per_bands: None,
        dim_head: 4,
        heads: 1,
        flash_attn: false,
        dim_freqs_in: 9,
        sample_rate: 44100,
        stft_n_fft: 16,
        stft_hop_length: 4,
        stft_win_length: 16,
        stft_normalized: false,
        mask_estimator_depth: 2,
        use_torch_checkpoint: false,
        time_transformer_depth: 1,
        freq_transformer_depth: 1,
        mlp_expansion_factor: 4,
        linear_transformer_depth: 0,
        norm_output: true,
        skip_connection: false,
        zero_dc: true,
    };
    cfg.validate().unwrap();
    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let _ = MelBandRoformer::new(&cfg, vb).unwrap();
    let model = dir.join("tiny_sep.safetensors");
    varmap.save(&model).unwrap();

    let yaml = format!(
        "model:\n  dim: {}\n  depth: {}\n  stereo: true\n  num_stems: {}\n  time_transformer_depth: 1\n  freq_transformer_depth: 1\n  num_bands: {}\n  dim_head: {}\n  heads: {}\n  flash_attn: false\n  dim_freqs_in: {}\n  sample_rate: 44100\n  stft_n_fft: {}\n  stft_hop_length: {}\n  stft_win_length: {}\n  stft_normalized: false\n  mask_estimator_depth: 2\n  use_torch_checkpoint: false\n  mlp_expansion_factor: 4\n  linear_transformer_depth: 0\n  norm_output: true\n  skip_connection: false\n  zero_dc: true\ninference:\n  normalization: 0.0\n  amplification: 0.0\n  invert_spect: false\n",
        cfg.dim, cfg.depth, cfg.num_stems, cfg.num_bands, cfg.dim_head, cfg.heads, cfg.dim_freqs_in,
        cfg.stft_n_fft, cfg.stft_hop_length, cfg.stft_win_length
    );
    let config = dir.join("tiny_sep.yaml");
    fs::write(&config, yaml).unwrap();
    LocalSeparationAssets::new(model, config)
}

#[test]
fn cli_transcription_reports_device_and_completion() {
    let dir = tmpdir("cli_transcription_device");
    tiny_separator_assets(&dir);
    let whisper = synth_whisper_assets("cli_transcription_device_whisper");
    let whisper_dir = whisper.config.parent().unwrap();
    let input = dir.join("song.wav");
    write_wav_f32(
        &input,
        &AudioBuffer {
            sample_rate: 44_100,
            channels: 1,
            planar: (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.4).collect(),
        },
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_karaoke-cli"))
        .arg(&input)
        .arg("--model_filename")
        .arg(dir.join("tiny_sep.safetensors"))
        .arg("--config")
        .arg(dir.join("tiny_sep.yaml"))
        .arg("--output_dir")
        .arg(dir.join("out"))
        .args(["--device", "cpu", "--transcribe", "--whisper_model_dir"])
        .arg(whisper_dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{stderr}");
    let started = stderr
        .find("transcription started (device=Cpu)")
        .expect("transcription must report the selected device");
    let finished = stderr
        .find("transcription finished (")
        .expect("transcription must report completion");
    let initial = stderr
        .find("transcription 0% (")
        .expect("transcription must report initial progress");
    let complete = stderr
        .find("transcription 100% (")
        .expect("transcription must report completed progress");
    assert!(
        started < initial && initial < complete && complete < finished,
        "{stderr}"
    );
    assert!(dir.join("out/song_(Vocals)_tiny_sep.txt").is_file());
    assert!(dir.join("out/song_(Vocals)_tiny_sep.json").is_file());
}

#[test]
fn cli_saves_instrumental_and_lyrics_without_a_vocal_audio_file() {
    let dir = tmpdir("instrumental_and_lyrics");
    let assets = tiny_separator_assets(&dir);
    let profile = fs::read_to_string(&assets.config).unwrap();
    fs::write(
        &assets.config,
        format!("{profile}\ntraining:\n  target_instrument: instrument\n"),
    )
    .unwrap();
    let whisper = synth_whisper_assets("instrumental_and_lyrics_whisper");
    let input = dir.join("song.wav");
    write_wav_f32(&input, &stereo_source(44_100, 2000)).unwrap();
    for chunked in [false, true] {
        let output_dir = dir.join(format!("out-{chunked}"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_karaoke-cli"));
        command
            .arg(&input)
            .arg("--model_filename")
            .arg(&assets.checkpoint)
            .arg("--config")
            .arg(&assets.config)
            .arg("--output_dir")
            .arg(&output_dir)
            .args(["--single_stem", "INSTRUMENTAL", "--transcribe"])
            .arg("--whisper_model_dir")
            .arg(whisper.config.parent().unwrap())
            .args([
                "--custom_output_names",
                r#"{"instrumental":"backing","vocals":"unused"}"#,
            ]);
        if chunked {
            command.args(["--chunk_seconds", "1"]);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        let clock_line = stderr
            .lines()
            .find(|line| line.contains("loading separation model (device=Cpu)"))
            .expect("model loading must be visible without debug logging");
        let clock = &clock_line.as_bytes()[..8];
        assert_eq!(clock[2], b':');
        assert_eq!(clock[5], b':');
        assert!(clock
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || index == 5 || byte.is_ascii_digit()));
        let mut previous = stderr.find("transcription finished (").unwrap();
        for marker in [
            "output saving started (3 file(s))",
            "writing output 1/3 (wav):",
            "output prepared 1/3:",
            "writing output 2/3 (text):",
            "output prepared 2/3:",
            "writing output 3/3 (text):",
            "output prepared 3/3:",
            "installing 3 output file(s)",
            "output saved 1/3:",
            "output saved 2/3:",
            "output saved 3/3:",
            "output saving finished (3 file(s),",
        ] {
            let position = stderr
                .find(marker)
                .unwrap_or_else(|| panic!("missing console message {marker}: {stderr}"));
            assert!(previous < position, "{marker}: {stderr}");
            previous = position;
        }
        let mut files: Vec<_> = fs::read_dir(output_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(files, ["backing.json", "backing.txt", "backing.wav"]);
    }
}

#[test]
fn separate_and_transcribe_writes_stems_and_sidecars() {
    let dir = tmpdir("flow");
    let separator = tiny_separator_assets(&dir);
    let whisper = synth_whisper_assets("flow_whisper");

    let frames = 2000usize;
    let samples: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    let input_audio = AudioBuffer {
        sample_rate: 44100,
        channels: 1,
        planar: samples.clone(),
    };
    let input = dir.join("song.wav");
    write_wav_f32(&input, &input_audio).unwrap();

    let out_dir = dir.join("out");
    let request = TranscriptionRequest {
        separator,
        whisper,
        input: input.clone(),
        output_dir: out_dir.clone(),
        device: DeviceChoice::Cpu,
        transcribe_options: TranscribeOptions::default(),
        overwrite: false,
        start_seconds: None,
        limit_seconds: None,
        single_stem: false,
    };

    let out = separate_and_transcribe_file(&request).unwrap();
    assert_eq!(
        out.vocals.file_name().unwrap().to_str().unwrap(),
        "song_(Vocals)_tiny_sep.wav"
    );
    assert_eq!(
        out.instrumental
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
        "song_(Instrumental)_tiny_sep.wav"
    );
    assert_eq!(
        out.text.file_name().unwrap().to_str().unwrap(),
        "song_(Vocals)_tiny_sep.txt"
    );
    assert_eq!(
        out.json.file_name().unwrap().to_str().unwrap(),
        "song_(Vocals)_tiny_sep.json"
    );

    let vocals = read_audio(&out.vocals).unwrap();
    let instrumental = read_audio(out.instrumental.as_ref().unwrap()).unwrap();
    assert_eq!(vocals.sample_rate, 44100);
    assert_eq!(vocals.planar.len(), samples.len());
    for i in 0..samples.len() {
        assert!(
            (samples[i] - (vocals.planar[i] + instrumental.planar[i])).abs() < 1e-4,
            "stems must sum back to the input at frame {i}"
        );
    }

    let text = fs::read_to_string(&out.text).unwrap();
    for segment in &out.transcript.segments {
        assert!(text.contains(segment.text.trim()));
        assert!(segment.end_seconds <= out.duration_seconds + 1e-9);
    }
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&out.json).unwrap()).unwrap();
    assert_eq!(json["language"], "en");
    assert_eq!(json["start_offset_seconds"].as_f64().unwrap(), 0.0);
    assert!(json["duration_seconds"].as_f64().unwrap() > 0.0);
    assert_eq!(
        json["segments"].as_array().unwrap().len(),
        out.transcript.segments.len()
    );

    // Preflight: a second run without --overwrite must refuse before doing work.
    let err = separate_and_transcribe_file(&request)
        .err()
        .expect("existing outputs must be refused");
    assert!(err.to_string().contains("output exists"));
}

#[test]
fn timestamp_format_is_stable() {
    assert_eq!(format_timestamp(0.0), "00:00.000");
    assert_eq!(format_timestamp(61.5), "01:01.500");
    assert_eq!(format_timestamp(-1.0), "00:00.000");
}

#[test]
fn trim_window_cuts_each_channel_independently() {
    // 2 channels x 10 frames at 10 Hz; channel 0 counts 0..10, channel 1 100..110.
    let planar: Vec<f32> = (0..10)
        .map(|i| i as f32)
        .chain((100..110).map(|i| i as f32))
        .collect();
    let mut audio = AudioBuffer {
        sample_rate: 10,
        channels: 2,
        planar,
    };

    trim_window(&mut audio, Some(0.2), Some(0.5)).unwrap();
    assert_eq!(audio.planar.len(), 2 * 5);
    assert_eq!(&audio.planar[..5], &[2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(&audio.planar[5..], &[102.0, 103.0, 104.0, 105.0, 106.0]);

    // A window past the end of the buffer is a loud error, not silence.
    let mut audio = AudioBuffer {
        sample_rate: 10,
        channels: 1,
        planar: vec![0.0; 10],
    };
    assert!(trim_window(&mut audio, Some(1.0), None).is_err());
}

#[test]
fn batch_runs_valid_audio_after_corrupt_audio_and_aggregates_failures() {
    let dir = tmpdir("batch_continue");
    let separator_assets = tiny_separator_assets(&dir);
    let separator = load_separator_with_options(
        &separator_assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();

    let input_dir = dir.join("inputs");
    fs::create_dir_all(&input_dir).unwrap();
    let valid = input_dir.join("a-valid.wav");
    let damaged = input_dir.join("b-damaged.wav");
    let valid_after_failure = input_dir.join("c-valid.wav");
    write_wav_f32(
        &valid,
        &AudioBuffer {
            sample_rate: 44_100,
            channels: 1,
            planar: (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.4).collect(),
        },
    )
    .unwrap();
    fs::copy(&valid, &valid_after_failure).unwrap();
    fs::write(&damaged, b"not an audio file").unwrap();

    let expanded = expand_inputs(std::slice::from_ref(&input_dir));
    assert!(expanded.errors.is_empty());
    assert_eq!(
        expanded.files,
        vec![valid.clone(), damaged.clone(), valid_after_failure.clone()]
    );
    let output_dir = dir.join("out");
    fs::create_dir_all(&output_dir).unwrap();
    let existing_vocal = output_dir.join("a-valid_(Vocals)_tiny_sep.wav");
    fs::write(&existing_vocal, b"preserve this file").unwrap();
    let report = process_batch(&expanded.files, |input| {
        run_file(
            &separator,
            &FileRequest {
                input: input.to_path_buf(),
                output_dir: output_dir.clone(),
                model_stem: "tiny_sep".into(),
                overwrite: false,
                custom_output_names: HashMap::new(),
                single_stem: None,
                format: OutputFormat::Wav,
                bitrate: None,
                sample_rate: None,
                start_seconds: None,
                limit_seconds: None,
                whisper: None,
            },
        )
        .map_err(|error| error.to_string())
    });

    assert_eq!(report.succeeded, 1);
    assert_eq!(report.failed, 2);
    assert!(report.outcomes[0]
        .1
        .as_ref()
        .unwrap_err()
        .contains("output file exists"));
    assert!(report.outcomes[1]
        .1
        .as_ref()
        .unwrap_err()
        .contains("unsupported"));
    assert!(report.outcomes[2].1.is_ok());
    assert_eq!(fs::read(existing_vocal).unwrap(), b"preserve this file");
    assert!(!output_dir
        .join("a-valid_(Instrumental)_tiny_sep.wav")
        .exists());
    assert!(!output_dir.join("b-damaged_(Vocals)_tiny_sep.wav").exists());
    assert!(output_dir.join("c-valid_(Vocals)_tiny_sep.wav").exists());
}

#[test]
fn custom_names_cover_partial_outputs_sidecars_and_path_rejection() {
    let dir = tmpdir("custom_names");
    let separator_assets = tiny_separator_assets(&dir);
    let separator = load_separator_with_options(
        &separator_assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();
    let input = dir.join("song.wav");
    write_wav_f32(
        &input,
        &AudioBuffer {
            sample_rate: 44_100,
            channels: 1,
            planar: (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.4).collect(),
        },
    )
    .unwrap();
    let whisper = synth_whisper_assets("custom_names_whisper");
    let output_dir = dir.join("custom-out");
    let result = run_file(
        &separator,
        &FileRequest {
            input: input.clone(),
            output_dir: output_dir.clone(),
            model_stem: "tiny_sep".into(),
            overwrite: false,
            custom_output_names: HashMap::from([("vocals".into(), "lead-take".into())]),
            single_stem: None,
            format: OutputFormat::Wav,
            bitrate: None,
            sample_rate: None,
            start_seconds: None,
            limit_seconds: None,
            whisper: Some((whisper, TranscribeOptions::default())),
        },
    )
    .unwrap();

    assert!(result
        .stems
        .iter()
        .any(|stem| stem.name.eq_ignore_ascii_case("vocals")
            && stem.path.file_name().unwrap() == "lead-take.wav"));
    assert!(result
        .stems
        .iter()
        .any(|stem| stem.name.eq_ignore_ascii_case("instrumental")
            && stem.path.file_name().unwrap() == "song_(Instrumental)_tiny_sep.wav"));
    assert_eq!(
        result.text.as_ref().unwrap().file_name().unwrap(),
        "lead-take.txt"
    );
    assert_eq!(
        result.json.as_ref().unwrap().file_name().unwrap(),
        "lead-take.json"
    );
    assert!(result.text.as_ref().unwrap().is_file());
    assert!(result.json.as_ref().unwrap().is_file());

    let invalid_mappings = [
        (
            "unknown stem",
            HashMap::from([("guitar".into(), "mix".into())]),
            "unknown stem",
        ),
        (
            "absolute path",
            HashMap::from([("vocals".into(), "/tmp/mix".into())]),
            "invalid output name",
        ),
        (
            "parent traversal",
            HashMap::from([("vocals".into(), "../mix".into())]),
            "invalid output name",
        ),
        (
            "directory separator",
            HashMap::from([("vocals".into(), "sub/mix".into())]),
            "invalid output name",
        ),
    ];
    for (case, mapping, expected_error) in invalid_mappings {
        let rejected_output = dir.join(format!("rejected-{case}"));
        let error = run_file(
            &separator,
            &FileRequest {
                input: input.clone(),
                output_dir: rejected_output.clone(),
                model_stem: "tiny_sep".into(),
                overwrite: false,
                custom_output_names: mapping,
                single_stem: None,
                format: OutputFormat::Wav,
                bitrate: None,
                sample_rate: None,
                start_seconds: None,
                limit_seconds: None,
                whisper: None,
            },
        )
        .unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "{case}: {error}"
        );
        assert!(
            !rejected_output.exists(),
            "{case} must not create output directories"
        );
    }
}

#[test]
fn four_stem_public_api_and_cli_keep_independent_named_outputs() {
    let dir = tmpdir("four_stem_api");
    let assets = tiny_separator_assets_with_stems(&dir, 4);
    let separator = load_separator_with_options(
        &assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();
    let audio = AudioBuffer {
        sample_rate: 44_100,
        channels: 1,
        planar: (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.4).collect(),
    };

    // This is the actual public candle-roformer 1.0 Stems object, returned by
    // the karaoke-cli wrapper without converting independent stems to residuals.
    let named = karaoke_cli::separate_audio(&separator, &audio).unwrap();
    assert_eq!(named.names(), vec!["drums", "bass", "other", "vocals"]);
    assert_eq!(named.len(), 4);
    assert!(named.get("instrumental").is_none());
    assert_eq!(
        named.get("vocals").unwrap().planar,
        named.vocals.planar,
        "the two-stem convenience vocal field remains available"
    );
    assert_eq!(
        named.get("bass").unwrap().planar,
        named.instrumental.planar,
        "without an instrumental stem, the convenience field falls back to the second named estimate"
    );

    let two_stem_dir = dir.join("two-stem-model");
    fs::create_dir_all(&two_stem_dir).unwrap();
    let two_stem_assets = tiny_separator_assets(&two_stem_dir);
    let two_stem_separator = load_separator_with_options(
        &two_stem_assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();
    let residual = karaoke_cli::separate_audio(&two_stem_separator, &audio).unwrap();
    assert_eq!(residual.names(), vec!["vocals", "instrumental"]);
    for index in 0..audio.planar.len() {
        assert!(
            (audio.planar[index]
                - residual.vocals.planar[index]
                - residual.instrumental.planar[index])
                .abs()
                <= 1e-5,
            "exact two-stem residual must reconstruct input within f32 tolerance at frame {index}"
        );
    }

    let input = dir.join("song.wav");
    write_wav_f32(&input, &audio).unwrap();
    let model = dir.join("tiny_sep.safetensors");
    let config = dir.join("tiny_sep.yaml");
    let all_outputs = dir.join("all-stems");
    let all_result = Command::new(env!("CARGO_BIN_EXE_karaoke-cli"))
        .arg(&input)
        .arg("--model_filename")
        .arg(&model)
        .arg("--config")
        .arg(&config)
        .arg("--output_dir")
        .arg(&all_outputs)
        .arg("--normalization")
        .arg("0")
        .arg("--amplification")
        .arg("0")
        .output()
        .unwrap();
    assert!(
        all_result.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&all_result.stdout),
        String::from_utf8_lossy(&all_result.stderr)
    );
    let mut all_names: Vec<String> = fs::read_dir(&all_outputs)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    all_names.sort();
    assert_eq!(
        all_names,
        vec![
            "song_(Bass)_tiny_sep.wav",
            "song_(Drums)_tiny_sep.wav",
            "song_(Other)_tiny_sep.wav",
            "song_(Vocals)_tiny_sep.wav",
        ]
    );

    let single_output = dir.join("one-stem");
    let single_result = Command::new(env!("CARGO_BIN_EXE_karaoke-cli"))
        .arg(&input)
        .arg("--model_filename")
        .arg(&model)
        .arg("--config")
        .arg(&config)
        .arg("--output_dir")
        .arg(&single_output)
        .arg("--single_stem")
        .arg("bass")
        .arg("--normalization")
        .arg("0")
        .arg("--amplification")
        .arg("0")
        .output()
        .unwrap();
    assert!(
        single_result.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&single_result.stdout),
        String::from_utf8_lossy(&single_result.stderr)
    );
    let single_names: Vec<String> = fs::read_dir(&single_output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(single_names, vec!["song_(Bass)_tiny_sep.wav"]);
}

#[test]
fn cli_encodes_flac_and_mp3_at_requested_sample_rate() {
    let dir = tmpdir("cli_codecs");
    let assets = tiny_separator_assets(&dir);
    let input_audio = AudioBuffer {
        sample_rate: 44_100,
        channels: 1,
        planar: (0..2000)
            .map(|index| (index as f32 * 0.041).sin() * 0.2)
            .collect(),
    };
    let input = dir.join("codec-input.wav");
    write_wav_f32(&input, &input_audio).unwrap();
    let expected_resampled = candle_roformer::resample::resample_planar(
        &input_audio.planar,
        input_audio.channels as usize,
        input_audio.sample_rate,
        48_000,
    );

    for (format, bitrate) in [("flac", None), ("mp3", Some("128"))] {
        let output_dir = dir.join(format!("out-{format}"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_karaoke-cli"));
        command
            .arg(&input)
            .arg("--model_filename")
            .arg(&assets.checkpoint)
            .arg("--config")
            .arg(&assets.config)
            .arg("--output_dir")
            .arg(&output_dir)
            .arg("--output_format")
            .arg(format)
            .arg("--sample_rate")
            .arg("48000")
            .arg("--normalization")
            .arg("0")
            .arg("--amplification")
            .arg("0");
        if let Some(bitrate) = bitrate {
            command.arg("--output_bitrate").arg(bitrate);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{format} CLI failed: stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let output_path = output_dir.join(format!("codec-input_(Vocals)_tiny_sep.{format}"));
        assert!(output_path.is_file(), "missing {} output", format);
        let decoded = read_audio(&output_path).unwrap();
        assert_eq!(decoded.sample_rate, 48_000, "{format}");
        let duration_delta =
            (decoded.duration_secs() - expected_resampled.len() as f64 / 48_000.0).abs();
        assert!(
            duration_delta <= 0.05,
            "{format}: duration delta {duration_delta}"
        );
        if format == "flac" {
            assert_eq!(decoded.planar.len(), expected_resampled.len());
        }
    }
}

#[test]
fn level_processing_defaults_and_residual_modes_match_reference_semantics() {
    fn peak(samples: &[f32]) -> f32 {
        samples
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0, f32::max)
    }

    fn max_delta(left: &[f32], right: &[f32]) -> f32 {
        assert_eq!(left.len(), right.len());
        left.iter()
            .zip(right)
            .map(|(left, right)| (left - right).abs())
            .fold(0.0, f32::max)
    }

    let dir = tmpdir("level_semantics");
    let assets = tiny_separator_assets(&dir);
    let input = AudioBuffer {
        sample_rate: 44_100,
        channels: 1,
        planar: (0..2000)
            .map(|index| (index as f32 * 0.043).sin() * 0.18)
            .collect(),
    };

    // Exact-residual mode disables all level transformations and spectral
    // inversion; summing the convenience stems restores the original samples.
    let exact_separator = load_separator_with_options(
        &assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();
    let exact = karaoke_cli::separate_audio(&exact_separator, &input).unwrap();
    let exact_sum: Vec<f32> = exact
        .vocals
        .planar
        .iter()
        .zip(&exact.instrumental.planar)
        .map(|(vocals, instrumental)| vocals + instrumental)
        .collect();
    assert!(
        max_delta(&exact_sum, &input.planar) <= 1e-5,
        "exact residual mode must reconstruct the input within f32 tolerance"
    );

    // Normalized mode's invariant is per-buffer peak normalization, not an
    // original-scale sum. Scaling the source input does not change the
    // normalized inference result because the input is normalized to 0.9 too.
    let defaults = InferenceOverrides::default();
    assert_eq!(defaults.normalization, 0.9);
    assert_eq!(defaults.amplification, 0.0);
    assert!(!defaults.invert_spect);
    let normalized_separator =
        load_separator_with_options(&assets, DeviceChoice::Cpu, defaults).unwrap();
    let normalized = karaoke_cli::separate_audio(&normalized_separator, &input).unwrap();
    let half_scale_input = AudioBuffer {
        planar: input.planar.iter().map(|sample| sample * 0.5).collect(),
        ..input.clone()
    };
    let normalized_half =
        karaoke_cli::separate_audio(&normalized_separator, &half_scale_input).unwrap();
    assert_eq!(normalized.names(), normalized_half.names());
    for ((name, stem), (half_name, half_stem)) in
        normalized.stems.iter().zip(&normalized_half.stems)
    {
        assert_eq!(name, half_name);
        let stem_peak = peak(&stem.planar);
        assert!(stem_peak > 0.0, "synthetic stem {name} must be non-silent");
        assert!(
            (stem_peak - 0.9).abs() <= 1e-5,
            "normalized stem {name} peak {stem_peak} is not 0.9"
        );
        assert!(
            max_delta(&stem.planar, &half_stem.planar) <= 1e-5,
            "input normalization should make the inferred {name} scale-invariant"
        );
    }

    // With normalization off, amplification alone maps a quiet (< 0.1 peak)
    // input to 0.1 before inference; the residual pair reconstructs that
    // amplified reference, not the unamplified source.
    let quiet = AudioBuffer {
        planar: input.planar.iter().map(|sample| sample * 0.05).collect(),
        ..input.clone()
    };
    let quiet_peak = peak(&quiet.planar);
    assert!(quiet_peak > 0.0 && quiet_peak < 0.1);
    let amplification_threshold = 0.1f32;
    let gain = amplification_threshold / quiet_peak;
    let amplified_input: Vec<f32> = quiet.planar.iter().map(|sample| sample * gain).collect();
    let amplified_separator = load_separator_with_options(
        &assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: amplification_threshold,
            invert_spect: false,
        },
    )
    .unwrap();
    let amplified = karaoke_cli::separate_audio(&amplified_separator, &quiet).unwrap();
    let amplified_sum: Vec<f32> = amplified
        .vocals
        .planar
        .iter()
        .zip(&amplified.instrumental.planar)
        .map(|(vocals, instrumental)| vocals + instrumental)
        .collect();
    assert!(
        max_delta(&amplified_sum, &amplified_input) <= 1e-5,
        "amplification should set the quiet-input reference peak to 0.1"
    );
    assert!((peak(&amplified_sum) - amplification_threshold).abs() <= 1e-5);

    // Spectral inversion produces the secondary stem via the public upstream
    // complex-STFT subtraction operation rather than waveform subtraction.
    let inverted_separator = load_separator_with_options(
        &assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: true,
        },
    )
    .unwrap();
    let inverted = karaoke_cli::separate_audio(&inverted_separator, &input).unwrap();
    let expected_spectral_residual =
        candle_roformer::dsp::spectral_invert(&inverted.vocals.planar, &input.planar, 16, 4, 16)
            .unwrap();
    assert!(
        max_delta(&inverted.instrumental.planar, &expected_spectral_residual) <= 1e-5,
        "--invert_spect secondary stem must match complex-STFT subtraction"
    );
}

fn chunked_request(input: PathBuf, output_dir: PathBuf) -> FileRequest {
    FileRequest {
        input,
        output_dir,
        model_stem: "tiny_sep".into(),
        overwrite: false,
        custom_output_names: HashMap::new(),
        single_stem: None,
        format: OutputFormat::Wav,
        bitrate: None,
        sample_rate: None,
        start_seconds: None,
        limit_seconds: None,
        whisper: None,
    }
}

#[test]
fn existing_outputs_fail_before_decode_separation_or_transcription() {
    let dir = tmpdir("early_output_conflicts");
    // Deliberately invalid audio and absent Whisper assets: output conflicts
    // must take precedence over both decoding and model loading errors.
    let input = dir.join("song.wav");
    fs::write(&input, b"invalid audio").unwrap();
    let cases = [
        (
            1,
            OutputFormat::Wav,
            None,
            false,
            false,
            "song_(Vocals)_tiny_sep.wav",
        ),
        (
            1,
            OutputFormat::Flac,
            None,
            false,
            false,
            "song_(Instrumental)_tiny_sep.flac",
        ),
        (
            1,
            OutputFormat::Mp3,
            Some("VOCALS"),
            true,
            false,
            "lead-take.mp3",
        ),
        (1, OutputFormat::Wav, None, true, true, "lead-take.txt"),
        (1, OutputFormat::Wav, None, true, true, "lead-take.json"),
        (
            1,
            OutputFormat::Wav,
            Some("INSTRUMENTAL"),
            false,
            true,
            "song_(Instrumental)_tiny_sep.txt",
        ),
        (
            1,
            OutputFormat::Wav,
            Some("instrumental"),
            false,
            true,
            "song_(Instrumental)_tiny_sep.json",
        ),
        (
            4,
            OutputFormat::Wav,
            Some("BASS"),
            true,
            false,
            "low-end.wav",
        ),
        (
            4,
            OutputFormat::Wav,
            None,
            false,
            false,
            "song_(Drums)_tiny_sep.wav",
        ),
        (
            2,
            OutputFormat::Wav,
            None,
            false,
            false,
            "song_(Stem_0)_tiny_sep.wav",
        ),
    ];
    for (index, (count, format, selection, custom, transcribe, filename)) in
        cases.into_iter().enumerate()
    {
        let model_dir = dir.join(format!("model-{index}"));
        fs::create_dir_all(&model_dir).unwrap();
        let assets = tiny_separator_assets_with_stems(&model_dir, count);
        let separator =
            load_separator_with_options(&assets, DeviceChoice::Cpu, InferenceOverrides::default())
                .unwrap();
        let output_dir = dir.join(format!("out-{index}"));
        fs::create_dir_all(&output_dir).unwrap();
        let existing = output_dir.join(filename);
        fs::write(&existing, b"preserve").unwrap();
        let mut request = chunked_request(input.clone(), output_dir.clone());
        request.format = format;
        request.single_stem = selection.map(String::from);
        if custom {
            let stem = if count == 4 { "bass" } else { "vocals" };
            let base = if count == 4 { "low-end" } else { "lead-take" };
            request.custom_output_names.insert(stem.into(), base.into());
        }
        if transcribe {
            request.whisper = Some((
                LocalWhisperAssets::new("missing-config", "missing-tokenizer", "missing-weights"),
                TranscribeOptions::default(),
            ));
        }
        let mut chunks_finished = 0;
        let error = run_file_with_chunks(
            &separator,
            &request,
            Some(&ChunkConfig::new(1.0, Some(0.25), 0.0, 0.0)),
            &mut |_| chunks_finished += 1,
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("output file exists"),
            "case {index}: {error}"
        );
        assert!(error.contains(filename), "case {index}: {error}");
        assert_eq!(chunks_finished, 0);
        assert_eq!(fs::read(&existing).unwrap(), b"preserve");
        assert_eq!(fs::read_dir(output_dir).unwrap().count(), 1);
    }
}

#[test]
fn early_preflight_respects_overwrite_and_only_checks_requested_outputs() {
    let dir = tmpdir("early_output_overwrite");
    let assets = tiny_separator_assets(&dir);
    let separator =
        load_separator_with_options(&assets, DeviceChoice::Cpu, InferenceOverrides::default())
            .unwrap();
    let input = dir.join("song.wav");
    write_wav_f32(&input, &stereo_source(44_100, 2000)).unwrap();
    let output_dir = dir.join("out");
    fs::create_dir_all(&output_dir).unwrap();
    let vocal = output_dir.join("song_(Vocals)_tiny_sep.wav");
    let instrumental = output_dir.join("song_(Instrumental)_tiny_sep.wav");
    let sidecar = output_dir.join("song_(Vocals)_tiny_sep.txt");
    fs::write(&instrumental, b"preserve instrumental").unwrap();
    fs::write(&sidecar, b"preserve transcript").unwrap();
    let mut request = chunked_request(input, output_dir);
    request.single_stem = Some("vocals".into());
    // An unselected stem and an unrequested sidecar must not block processing.
    run_file(&separator, &request).unwrap();
    fs::write(&vocal, b"replace vocal").unwrap();
    request.overwrite = true;
    run_file(&separator, &request).unwrap();
    assert_ne!(fs::read(vocal).unwrap(), b"replace vocal");
    assert_eq!(fs::read(instrumental).unwrap(), b"preserve instrumental");
    assert_eq!(fs::read(sidecar).unwrap(), b"preserve transcript");
}

#[test]
fn instrumental_target_keeps_correct_output_labels_with_and_without_chunks() {
    let dir = tmpdir("instrumental_target_labels");
    let assets = tiny_separator_assets(&dir);
    let overrides = InferenceOverrides {
        normalization: 0.0,
        amplification: 0.0,
        invert_spect: false,
    };
    let vocal_separator =
        load_separator_with_options(&assets, DeviceChoice::Cpu, overrides).unwrap();
    let profile = fs::read_to_string(&assets.config).unwrap();
    fs::write(
        &assets.config,
        format!("{profile}\ntraining:\n  target_instrument: instrument\n"),
    )
    .unwrap();
    // The CLI's temporary-profile overrides must preserve target_instrument.
    let instrumental_separator =
        load_separator_with_options(&assets, DeviceChoice::Cpu, overrides).unwrap();
    let input = dir.join("song.wav");
    write_wav_f32(&input, &stereo_source(44_100, 2000)).unwrap();
    let chunk_config = ChunkConfig::new(1.0, Some(0.25), 0.0, 0.0);
    for (index, chunks) in [None, Some(&chunk_config)].into_iter().enumerate() {
        let vocal_request =
            chunked_request(input.clone(), dir.join(format!("vocals-target-{index}")));
        let instrumental_request = chunked_request(
            input.clone(),
            dir.join(format!("instrumental-target-{index}")),
        );
        let reference =
            run_file_with_chunks(&vocal_separator, &vocal_request, chunks, &mut |_| {}).unwrap();
        let result = run_file_with_chunks(
            &instrumental_separator,
            &instrumental_request,
            chunks,
            &mut |_| {},
        )
        .unwrap();
        for (output_name, reference_name) in
            [("instrumental", "vocals"), ("vocals", "instrumental")]
        {
            let actual_path = &result
                .stems
                .iter()
                .find(|stem| stem.name == output_name)
                .unwrap()
                .path;
            let reference_path = &reference
                .stems
                .iter()
                .find(|stem| stem.name == reference_name)
                .unwrap()
                .path;
            let actual = read_audio(actual_path).unwrap();
            let expected = read_audio(reference_path).unwrap();
            assert_eq!(actual.planar.len(), expected.planar.len());
            let max_error = actual
                .planar
                .iter()
                .zip(&expected.planar)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            assert!(max_error < 1e-5, "{output_name}: {max_error}");
        }
    }
}

fn stereo_source(sample_rate: u32, frames: usize) -> AudioBuffer {
    let mut planar = Vec::with_capacity(frames * 2);
    planar.extend((0..frames).map(|i| (i as f32 * 0.031).sin() * 0.4));
    planar.extend((0..frames).map(|i| (i as f32 * 0.017).cos() * 0.3));
    AudioBuffer {
        sample_rate,
        channels: 2,
        planar,
    }
}

#[test]
fn chunked_separation_crossfades_and_preserves_length_and_residual() {
    let dir = tmpdir("chunk_residual");
    let assets = tiny_separator_assets(&dir);
    let separator = load_separator_with_options(
        &assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();

    let input_audio = stereo_source(44_100, 52_920);
    let input = dir.join("long.wav");
    write_wav_f32(&input, &input_audio).unwrap();

    let out_dir = dir.join("chunked");
    let chunk = ChunkConfig {
        chunk_seconds: 1.0,
        overlap_seconds: 0.2,
        normalization: 0.0,
        amplification: 0.0,
    };
    let mut events = Vec::new();
    let result = run_file_with_chunks(
        &separator,
        &chunked_request(input, out_dir.clone()),
        Some(&chunk),
        &mut |progress| events.push((progress.index, progress.total)),
    )
    .unwrap();

    assert_eq!(result.stems.len(), 2);
    assert!(
        events.len() >= 2,
        "a 1.2 s input in 1 s chunks must report more than one chunk, got {}",
        events.len()
    );
    for (position, (index, total)) in events.iter().enumerate() {
        assert_eq!(*index, position + 1, "chunk events must be sequential");
        assert_eq!(*total, events.len(), "every event reports the same total");
    }

    let vocals = read_audio(out_dir.join("long_(Vocals)_tiny_sep.wav")).unwrap();
    let instrumental = read_audio(out_dir.join("long_(Instrumental)_tiny_sep.wav")).unwrap();
    assert_eq!(vocals.planar.len(), input_audio.planar.len());
    assert_eq!(instrumental.planar.len(), input_audio.planar.len());

    let mut worst = 0f32;
    for (frame, source) in input_audio.planar.iter().enumerate() {
        let delta = (source - vocals.planar[frame] - instrumental.planar[frame]).abs();
        if delta > worst {
            worst = delta;
        }
    }
    assert!(
        worst <= 1e-5,
        "a linear crossfade must keep vocals + instrumental = input, worst delta {worst}"
    );
}

#[test]
fn chunked_separation_applies_the_level_threshold_once_over_every_stem() {
    let dir = tmpdir("chunk_levels");
    let assets = tiny_separator_assets(&dir);
    let separator = load_separator_with_options(
        &assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();

    let input_audio = stereo_source(44_100, 52_920);
    let input = dir.join("levels.wav");
    write_wav_f32(&input, &input_audio).unwrap();

    let out_dir = dir.join("normalized");
    let chunk = ChunkConfig {
        chunk_seconds: 1.0,
        overlap_seconds: 0.2,
        normalization: 0.9,
        amplification: 0.0,
    };
    run_file_with_chunks(
        &separator,
        &chunked_request(input, out_dir.clone()),
        Some(&chunk),
        &mut |_| {},
    )
    .unwrap();

    for stem in ["Vocals", "Instrumental"] {
        let decoded = read_audio(out_dir.join(format!("levels_({stem})_tiny_sep.wav"))).unwrap();
        let peak = decoded
            .planar
            .iter()
            .fold(0f32, |peak, sample| peak.max(sample.abs()));
        assert!(
            (peak - 0.9).abs() <= 1e-4,
            "{stem} peak {peak} must be the whole-file threshold, not a per-chunk one"
        );
    }
}

#[test]
fn chunked_mode_keeps_level_processing_when_the_input_fits_one_chunk() {
    let dir = tmpdir("chunk_short");
    let assets = tiny_separator_assets(&dir);
    let separator = load_separator_with_options(
        &assets,
        DeviceChoice::Cpu,
        InferenceOverrides {
            normalization: 0.0,
            amplification: 0.0,
            invert_spect: false,
        },
    )
    .unwrap();

    let input_audio = stereo_source(44_100, 13_230);
    let input = dir.join("short.wav");
    write_wav_f32(&input, &input_audio).unwrap();

    let out_dir = dir.join("short-out");
    let chunk = ChunkConfig {
        chunk_seconds: 5.0,
        overlap_seconds: 1.0,
        normalization: 0.9,
        amplification: 0.0,
    };
    let mut events = Vec::new();
    run_file_with_chunks(
        &separator,
        &chunked_request(input, out_dir.clone()),
        Some(&chunk),
        &mut |progress| events.push((progress.index, progress.total)),
    )
    .unwrap();

    assert_eq!(events, vec![(1, 1)], "a short input is one chunk");
    let vocals = read_audio(out_dir.join("short_(Vocals)_tiny_sep.wav")).unwrap();
    assert_eq!(vocals.planar.len(), input_audio.planar.len());
    let peak = vocals
        .planar
        .iter()
        .fold(0f32, |peak, sample| peak.max(sample.abs()));
    assert!(
        (peak - 0.9).abs() <= 1e-4,
        "chunked mode must still normalise a single-chunk input, peak {peak}"
    );
}
