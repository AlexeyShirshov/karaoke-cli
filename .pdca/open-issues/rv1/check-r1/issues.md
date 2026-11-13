# GitHub issues #1–#7

Retrieved at (UTC): 2026-11-13T22:34:38Z

## #1 — feat: accept directories as input and aggregate batch failures

- State: OPEN
- Updated at: 2026-11-08T06:44:11Z
- Source: https://github.com/AlexeyShirshov/karaoke-cli/issues/1

### Body

The CLI only accepts explicit file paths, and the first failure aborts the whole run.

`audio-separator` accepts files or a directory, processes a batch, writes every stem it could, and raises `BatchSeparationError` at the end carrying `successful_files` and per-input causes (`InvalidAudioDataError`, `AudioExportError`).

**Missing**
- Directory input: discover audio files (non-recursive? define and document), stable ordering.
- Per-input error collection; exit code and a summary that names each failed input.
- Continue-on-error semantics: a failed input must not abort the remaining ones.
- Preflight per input (existing `--overwrite` behaviour) without writing anything for a refused file.

**Done when**
- `karaoke-cli <dir>/ ...` separates every supported file and exits non-zero if any failed, printing the per-file reasons.
- A test with one valid and one corrupt file proves the valid one is still produced.

## #2 — feat: --custom_output_names

- State: OPEN
- Updated at: 2026-11-08T06:44:15Z
- Source: https://github.com/AlexeyShirshov/karaoke-cli/issues/2

### Body

Output names are fixed as `<input>_(Vocals|Instrumental)_<model>.wav`. `audio-separator` accepts a per-stem override map, e.g. `{"Vocals": "vocals_output"}`, leaving unnamed stems on the default convention.

**Missing**
- `--custom_output_names '<json>'` parsed into a stem -> base name map.
- Partial maps (only one stem renamed), unknown stem names rejected, path traversal rejected (no absolute paths or `..`).
- The same names used for the `.txt`/`.json` sidecars of the vocal stem.
- Cross-repo dependency: named stems arrive with the multi-stem API in `candle-roformer`.

**Done when**
- The flag works for the two current stems, with tests for partial maps and invalid names.

## #3 — feat: --list_models (filter, limit, JSON)

- State: OPEN
- Updated at: 2026-11-08T06:44:19Z
- Source: https://github.com/AlexeyShirshov/karaoke-cli/issues/3

### Body

`audio-separator -l` prints a model catalogue with architecture, available stems and SDR, supports `--list_filter` (by stem), `--list_limit` and `--list_format json`. This CLI has no catalogue at all and only resolves whatever path the user passes.

**Missing**
- A model catalogue data file (name, filename, architecture, stems, optional SDR, checksum) shipped with the CLI or read from a user-supplied manifest.
- `--list_models`, `--list_filter`, `--list_limit`, `--list_format pretty|json`.
- Honest scope: the catalogue is metadata only. Downloading weights stays out of scope for this project (the crate must not download or redistribute models), so the list explains how to obtain each file instead of fetching it.

**Done when**
- The command prints the documented columns and a stable JSON schema, with tests over a small fixture catalogue.

## #4 — feat: CLI diagnostics (--env_info, --debug/--log_level)

- State: OPEN
- Updated at: 2026-11-08T06:44:30Z
- Source: https://github.com/AlexeyShirshov/karaoke-cli/issues/4

### Body

The CLI has no logging or environment reporting: progress is a single `println!` per file and failures are a bare `error: ...` line.

`audio-separator` offers `--debug`, `--log_level` and `--env_info` (device/provider availability, FFmpeg presence, versions), which is what users paste into bug reports.

**Missing**
- A logging backend (`tracing`/`log`) with `--log_level`, `--debug` alias, timestamps and module targets, default `info`.
- `--env_info`: crate and CLI versions, Candle revision, features compiled in (`cuda`/`metal`), device availability probe, available parallelism, and the resolved model paths. No secrets, no network calls.
- Errors that carry the input path and the underlying cause.

**Done when**
- `--env_info` output is sufficient to reproduce a report and is exercised by a test (no model weights required).
- Verbose mode shows per-stage timings (decode, separate, write, transcribe) without changing default output.

## #5 — feat: expose multi-stem output (drums, bass, piano, guitar, other)

- State: OPEN
- Updated at: 2026-11-09T05:01:03Z
- Source: https://github.com/AlexeyShirshov/karaoke-cli/issues/5

### Body

`Separator::separate` returns `Stems { vocals, instrumental }`; the instrumental is the input residual. `audio-separator` models can emit arbitrary named stems (Demucs: drums/bass/other/vocals, `htdemucs_6s` adds guitar/piano; MDX/VR karaoke models: lead/backing vocals).

The model already builds one mask estimator per `num_stems`, but the public API and the CLI throw everything away except the vocal stem.

**Missing**
- A stem set in the public API (named stems, not two fixed fields), while keeping the two-stem convenience.
- Residual semantics: only meaningful for "rest of the mix"; multi-stem models return independent stems and their sum need not equal the input.
- CLI: `--single_stem <name>` (the current flag is a bool meaning "skip the instrumental") and per-stem file naming (`<input>_(Drums)_<model>.wav`, ...).

**Done when**
- A 4-stem model returns four named `AudioBuffer`s through the library API.
- Naming and `--single_stem <name>` work in `karaoke-cli` (cross-repo follow-up).

## #6 — feat: output encoders beyond WAV f32 (FLAC/MP3/M4A, bitrate, sample rate)

- State: OPEN
- Updated at: 2026-11-09T05:01:09Z
- Source: https://github.com/AlexeyShirshov/karaoke-cli/issues/6

### Body

Today the only writer is `wav::write_wav_f32` and the only output is f32 WAV at the input sample rate. `audio-separator` writes any ffmpeg format (FLAC by default in the CLI) with `--output_bitrate` and resamples to `--sample_rate` (default 44100).

**Missing**
- Encoders: FLAC (e.g. `flacenc`), MP3 (`mp3lame-encoder`), and a decision for M4A/AAC (pure-Rust encoder vs documented unsupported).
- `--output_bitrate` for lossy formats.
- An output sample-rate option, reusing the existing `resample` module.
- WAV should stay f32 by default; no silent dithering/quantisation.

**Done when**
- FLAC and MP3 outputs round-trip through our own decoder (`read_audio`) and match the reference tool's duration/sample rate.
- `--output_format`, `--output_bitrate`, `--sample_rate` exist in `karaoke-cli` and are rejected with a clear error when unsupported.
- New dependencies are added to `docs/dependency-audit.md`.

## #7 — feat: level processing (normalization, amplification, spectral inversion)

- State: OPEN
- Updated at: 2026-11-09T05:01:14Z
- Source: https://github.com/AlexeyShirshov/karaoke-cli/issues/7

### Body

`audio-separator` applies `--normalization 0.9` by default, plus `--amplification` and `--invert_spect`. This crate touches neither the input level nor the output level, so levels and peak values differ from the reference tool even when the separation is identical.

**Missing**
- Peak normalization of input and output to a configurable threshold (default 0.9 to match the reference).
- Amplification of quiet inputs below a threshold (default 0.0 = off).
- `--invert_spect`: spectral inversion of the secondary stem instead of the waveform residual.
- A decision on residual exactness: normalization must be applied consistently to both stems or the `vocals + instrumental = input` invariant breaks (currently tested and documented).

**Done when**
- The invariant is re-stated precisely, with a test for both "exact residual" and "normalized" modes.
- CLI flags match the reference semantics and defaults.

