# karaoke-cli

Offline command-line tool for vocal/instrumental separation and lyric
transcription. It is the file-level front end of the
[`candle-roformer`](https://github.com/AlexeyShirshov/candle-roformer) crate:
the crate does the DSP and the models, this repository decides output names,
writes timestamped sidecars and drives batches.

No Python at runtime, no ffmpeg, no hidden model downloads.

## Install

```sh
cargo build --release
```

CUDA: `cargo build --release --features cuda` (NVIDIA + CUDA toolkit).
Metal: `cargo build --release --features metal` (macOS).

The binary is `target/release/karaoke-cli`. The crate is consumed from
crates.io, so a checkout needs network access for the first build; bump the
version in `Cargo.toml` after a release of `candle-roformer`.

## Separate

```sh
karaoke-cli "/path/to/song.m4a" \
  --model_filename /path/to/MelBandRoformer_fp32.safetensors \
  --config /path/to/kijai_melband_384.yaml \
  --output_dir out/
```

Outputs follow the `audio-separator` naming convention:
`<input>_(Vocals)_<model>.wav` and `<input>_(Instrumental)_<model>.wav`.

Options:

| Flag | Meaning |
|---|---|
| `--model_filename`, `--model` | MelBandRoformer safetensors checkpoint |
| `--config` | Model profile YAML (default: `<model-stem>.yaml` next to the model) |
| `--model_file_dir` | Directory used to resolve the two paths above |
| `--output_dir`, `--output` | Output directory (default: `.`) |
| `--output_format wav` | Only `wav` is implemented |
| `--single_stem` | Write only the vocals stem |
| `--device cpu\|cuda\|metal` | An unavailable device is an error, never a silent CPU fallback |
| `--start`, `--duration` | Process a window of the input, in seconds |
| `--overwrite` | Replace existing outputs |

## Transcribe

```sh
karaoke-cli "song.m4a" \
  --model_filename /models/MelBandRoformer_fp32.safetensors \
  --config configs/kijai_melband_384.yaml --output_dir out/ \
  --transcribe --whisper_model_dir /models/whisper-small-ft \
  --language ru
```

`--whisper_model_dir` must contain `config.json`, `tokenizer.json` and
`model.safetensors` (override individually with `--whisper_config`,
`--whisper_tokenizer`, `--whisper_model`). Only the vocal stem is transcribed,
never the mix.

`--language auto` (the default) lets Whisper detect the language; on singing the
detection is unreliable, so forcing a code is usually better.

Outputs: the two WAV stems plus `<input>_(Vocals)_<model>.txt` (timestamped
lines) and `.json` (input, model, language, duration, segments,
`start_offset_seconds`). All paths are preflighted before any work starts;
without `--overwrite` existing outputs are refused. Each file goes through a
dot-prefixed staging name and a rename, but this is not a multi-file atomic
transaction. The separator is dropped before Whisper loads, so both model sets
are never resident at once.

## Input formats

m4a (AAC, ALAC, FLAC-in-MP4), mp3, flac, ogg/vorbis, wav, decoded by the crate.
Mono input is duplicated to stereo for the model and the separated vocal is
downmixed back to mono; stereo stays stereo.

## Model files

Weights are supplied locally. This project never downloads, bundles, caches or
redistributes them.

| Model | Config | Weights |
|---|---|---|
| GaboxR67 `voc_fv7` | `configs/voc_fv7.yaml` in the crate | convert `voc_fv7.ckpt` with `candle-roformer`'s `tools/convert_voc_fv7.py` |
| KimberleyJSN / Kijai mirror | `configs/kijai_melband_384.yaml` in the crate | `MelBandRoformer_fp32.safetensors` as published |
| Whisper (tiny..large, `*.en` or multilingual) | `config.json` from the checkpoint | `model.safetensors` + `tokenizer.json` |

Whisper assets must be in the Candle-compatible layout; f32 only, no
GGUF/quantized path.

## Status and limits

- CPU inference works but is slow: on a machine without a GPU a MelBandRoformer
  384/6 pass runs at roughly a quarter of real time, and a 30 s transcription
  window on `whisper-small` took several minutes end to end.
  `CANDLE_NUM_THREADS=8` uses more cores.
- CUDA/Metal execution has **not** been tested here (no NVIDIA GPU, no macOS
  host). Nothing is claimed for GPU runs.
- Whisper on singing can hallucinate or mis-transcribe; timestamps are
  segment-level, not word alignment. There is no WER or lyric-accuracy claim.
- Only MelBandRoformer checkpoints are supported so far; `audio-separator` also
  covers MDX-Net, VR/UVR, Demucs and MDX23C, which are not ported.
- Weights licensing is not resolved by this repository.

## Tests

```sh
cargo test --offline
```

The suite builds a tiny randomly-initialised separator and Whisper checkpoint
and exercises the full file flow (naming, stems, sidecars, preflight) plus
window trimming and timestamp formatting. It says nothing about model quality;
real-checkpoint evidence lives in the crate's `docs/gates-report.md`.
