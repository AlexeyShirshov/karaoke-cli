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

Outputs follow the `audio-separator` naming convention, one file per named
stem: `<input>_(Vocals)_<model>.<ext>`,
`<input>_(Instrumental)_<model>.<ext>`, or the names supplied by a multi-stem
model. The default extension is `.wav`.

Options:

| Flag | Meaning |
|---|---|
| `--model_filename`, `--model` | Local `candle-roformer`-supported safetensors checkpoint; family is selected by its profile |
| `--config` | Model profile YAML (default: `<model-stem>.yaml` next to the model) |
| `--model_file_dir` | Directory used to resolve the two paths above |
| `--output_dir`, `--output` | Output directory (default: `.`) |
| `--output_format wav, flac, or mp3` | Output container (default: WAV, IEEE-f32, no integer conversion or dithering); FLAC is lossless 24-bit PCM and MP3 is lossy |
| `--output_bitrate <kbps>` | MP3 CBR bitrate (default: 192 kbps); only valid with `--output_format mp3` |
| `--sample_rate <Hz>` | Resample output using `candle-roformer`'s resampler; omitted means preserve the input rate |
| `--single_stem <name>` | Write only the selected named stem, e.g. `vocals`, `bass`, or `drums` |
| `--custom_output_names '<json>'` | Partial JSON mapping from stem name to safe output basename; transcription sidecars use the vocal basename |
| `--device cpu\|cuda\|metal` | An unavailable device is an error, never a silent CPU fallback |
| `--start`, `--duration` | Process a window of the input, in seconds |
| `--overwrite` | Replace existing outputs |
| `--normalization <peak>` | Peak-normalize input and output (default: `0.9`; `0` disables) |
| `--amplification <peak>` | Raise non-silent input below this peak before separation (default: `0.0`, off) |
| `--invert_spect` | In residual mode, form the secondary stem by complex-STFT subtraction |

`--output_bitrate` accepts `8, 16, 24, 32, 40, 48, 64, 80, 96, 112, 128,
160, 192, 224, 256, or 320` kbps. General output sample rates must be in
`1..=384000` Hz; MP3 supports `8000, 11025, 12000, 16000, 22050, 24000,
32000, 44100, or 48000` Hz. Unsupported combinations are rejected before
outputs are written.

## Batch inputs and named stems

Pass one or more files and/or directories. Directories are scanned
non-recursively and supported audio files are sorted in a stable order. Each
file is preflighted independently; a failed or existing output is reported,
later inputs are still attempted, and the command returns nonzero if any input
failed. Existing outputs are left untouched unless `--overwrite` is set.

The public Rust API exposes `karaoke_cli::NamedStems` (the upstream named
`candle_roformer::Stems` type) and `karaoke_cli::separate_audio`. A single-target
model returns `vocals` plus an `instrumental` residual. A multi-stem model
returns independent named estimates such as `drums`, `bass`, `other`, and
`vocals`; they are not forced to sum to the input. In the CLI,
`--single_stem bass` writes only the matching stem. Stem selection is
case-insensitive. For custom names, for example:

```sh
karaoke-cli song.wav --model_filename model.safetensors \
  --custom_output_names '{"vocals":"lead-take","bass":"low-end"}'
```

Mappings may be partial. Unknown stem names, absolute paths, path separators,
and `..` traversal are rejected. When transcribing, `.txt` and `.json`
sidecars use the selected vocal stem's custom basename.

## Level processing and residual exactness

The CLI defaults match the upstream reference semantics: `--normalization 0.9`,
`--amplification 0.0`, and `--invert_spect` off. For each non-silent input and
each output stem, positive normalization independently scales the peak to the
selected threshold; silent buffers stay silent. Amplification raises a
non-silent input only when its peak is below the supplied threshold, and a
threshold of `0` disables it. `--invert_spect` applies to the secondary stem
of a single-target residual model; multi-stem models continue to return their
independent estimates.

There are two different residual contracts:

- **Exact residual mode:** pass `--normalization 0 --amplification 0` and omit
  `--invert_spect`. The waveform residual is `input - vocals`, so
  `vocals + instrumental = input` within f32 arithmetic (the synthetic test's
  maximum error is at most `1e-5`).
- **Normalized mode (default):** input, vocals, and instrumental are normalized
  independently. Its invariant is that every non-silent buffer has peak
  `0.9` (within `1e-5`); the sum of the output stems is **not** guaranteed to
  reconstruct the input in its original scale. With `--invert_spect`, the
  secondary stem is instead produced by complex-STFT subtraction and inverse
  STFT, so it is not the exact waveform residual.

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

Transcription writes `<input>_(Vocals)_<model>.txt` (timestamped lines) and
`.json` (input, model, language, duration, segments, `start_offset_seconds`).
The stem files use the selected `--output_format`; sidecars remain UTF-8 text
and JSON and follow the vocal custom basename when one is configured. Output
paths are preflighted before files are installed; without `--overwrite`,
existing outputs are refused. Files use dot-prefixed staging names and rename,
but installation of several outputs is not a multi-file atomic transaction.
The separator stays loaded across the batch; Whisper is loaded for the
transcription stage, so both model sets can be resident at the same time.

## Input formats

m4a (AAC, ALAC, FLAC-in-MP4), mp3, flac, ogg/vorbis, wav, decoded by the crate.
As an **output** format, only WAV, FLAC and MP3 are supported. WAV is IEEE-f32
with no integer conversion or hidden dithering. FLAC is lossless 24-bit PCM
(sample rate and sample count round-trip exactly; f32 values are quantized to
24-bit integers). MP3 is lossy and may differ in duration by up to `0.05 s`
because of frame delay/padding. Output `m4a`/`aac` is rejected with
`no bundled offline AAC/M4A encoder is available`; the CLI does not shell out to
FFmpeg or rely on a system encoder. See
[`docs/dependency-audit.md`](docs/dependency-audit.md) for codec/build and
dependency details.
Mono input is duplicated to stereo for the model and the separated vocal is
downmixed back to mono; stereo stays stereo.

## Local model catalog

`--list_models` prints the built-in metadata catalog and does not download
weights. `--list_filter <text>` searches model name, filename, architecture,
and stem names case-insensitively; `--list_limit <n>` limits results;
`--list_format pretty|json` selects a fixed-column table or stable JSON rows
with `name`, `filename`, `architecture`, `stems`, `SDR`, `checksum`, and
`weights` fields. To use a local JSON or YAML manifest, pass
`--models_manifest <path>`. It may be a top-level array or an object containing
`models: [...]`; each row requires `name`, `filename`, `architecture`, and a
non-empty `stems` list. `SDR`, `checksum`, and `weights` are optional metadata.
No weights are fetched or included.

## Diagnostics

`--env_info` reports CLI/Candle versions, compiled CUDA/Metal features, device
availability, available parallelism, and resolved model paths without loading
weights or accessing the network. Logging uses timestamped records with module
targets; `--log_level trace|debug|info|warn|error|off` controls the level
(default `info`), and `--debug` aliases `--log_level debug`. Debug logs include
decode, separation, transcription, and output-write timings. Normal output is
unchanged at the default level.

## Model files

Weights are supplied locally. This project never downloads, bundles, caches or
redistributes them.

| Model | Config | Weights |
|---|---|---|
| BS-RoFormer Viperx 1297 | `configs/bs_roformer_viperx_1297.yaml` in the crate | convert the source `.ckpt` with `candle-roformer`'s `tools/convert_bs_roformer.py`; the catalog checksum describes the source checkpoint |
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
- Catalog rows are metadata, not a loadability guarantee; checkpoint support
  follows the model families and config compatibility provided by
  `candle-roformer` 1.0.
- Weights licensing is not resolved by this repository.

## Tests

```sh
cargo test --offline
```

The suite uses generated audio, tiny synthetic separator and Whisper weights,
and local catalog fixtures. It covers directory batches and error handling,
custom names and sidecars, one- and four-stem flows, WAV/FLAC/MP3 round-trips
through `read_audio`, resampling, level defaults and residual invariants,
diagnostics, model listing, window trimming, and timestamp formatting. It uses
no real user audio or downloaded model weights and says nothing about model
quality; real-checkpoint evidence lives in the crate's `docs/gates-report.md`.
