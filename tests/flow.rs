//! File-level flow tests: output naming, sidecars, preflight, window trimming.
//!
//! The real checkpoints are not present in this environment, so the end-to-end
//! test builds a tiny randomly-initialised separator and Whisper model. That
//! proves the naming, staging/rename and sidecar plumbing executes; it says
//! nothing about separation or transcription quality (see the crate's
//! `docs/gates-report.md`).

use std::fs;
use std::path::{Path, PathBuf};

use candle_core::{DType, Device};
use candle_nn::{VarBuilder, VarMap};
use candle_roformer::config::SeparatorConfig;
use candle_roformer::model::MelBandRoformer;
use candle_roformer::wav::{read_audio, write_wav_f32, AudioBuffer};
use candle_roformer::whisper::TranscribeOptions;
use candle_roformer::{DeviceChoice, LocalSeparationAssets, LocalWhisperAssets};
use karaoke_cli::pipeline::{
    format_timestamp, separate_and_transcribe_file, trim_window, TranscriptionRequest,
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
    let cfg = SeparatorConfig {
        dim: 8,
        depth: 1,
        stereo: true,
        num_stems: 1,
        num_bands: 4,
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
        "model:\n  dim: {}\n  depth: {}\n  stereo: true\n  num_stems: 1\n  time_transformer_depth: 1\n  freq_transformer_depth: 1\n  num_bands: {}\n  dim_head: {}\n  heads: {}\n  flash_attn: false\n  dim_freqs_in: {}\n  sample_rate: 44100\n  stft_n_fft: {}\n  stft_hop_length: {}\n  stft_win_length: {}\n  stft_normalized: false\n  mask_estimator_depth: 2\n  use_torch_checkpoint: false\n  mlp_expansion_factor: 4\n  linear_transformer_depth: 0\n  norm_output: true\n  skip_connection: false\n  zero_dc: true\n",
        cfg.dim, cfg.depth, cfg.num_bands, cfg.dim_head, cfg.heads, cfg.dim_freqs_in,
        cfg.stft_n_fft, cfg.stft_hop_length, cfg.stft_win_length
    );
    let config = dir.join("tiny_sep.yaml");
    fs::write(&config, yaml).unwrap();
    LocalSeparationAssets::new(model, config)
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
