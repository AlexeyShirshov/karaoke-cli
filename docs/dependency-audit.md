# Dependency and codec audit

This audit records the direct dependency changes made for the `candle-roformer`
1.0 migration and tickets #1–#7. `Cargo.lock` is the authoritative resolved
graph, including transitive crates; regenerate and review it whenever
`Cargo.toml` changes.

## Direct dependency changes

Compared with the project baseline (`a1ea0a4`):

| Dependency | Manifest / locked version | Change and use |
|---|---|---|
| `candle-roformer` | `1.0` / `1.0.0` | Replaces the previous pinned Git revision with the published crates.io release. Provides model inference, input decoding, WAV f32 writing, and the shared resampler. |
| `flacenc` | `0.5.1` (`default-features = false`) | New direct dependency for offline FLAC encoding. Pure Rust; no system FLAC library or FFmpeg process is used. The CLI converts f32 samples to signed 24-bit PCM for FLAC, so sample count/rate are lossless but the source f32 bit patterns are not preserved. Upstream metadata declares Apache-2.0. |
| `mp3lame-encoder` | `0.3.0` | New direct dependency for MP3 CBR encoding. Its `mp3lame-sys 0.2.0` dependency builds the bundled LAME C sources; a working C compiler is required at build time, but an installed system LAME library and FFmpeg are not. Upstream metadata declares LGPL-3.0; review and meet applicable LGPL obligations before distributing binaries. |
| `log` | `0.4` / `0.4.35` | New logging facade used by the CLI's timestamped, target-aware logger and `--log_level`/`--debug`. |
| `serde_yaml` | `0.9` / `0.9.34+deprecated` | New YAML support used to apply inference overrides to a temporary profile without editing the user's model configuration. |

The codec crate manifests and source snapshots used during this implementation
are retained under `.pdca/open-issues/rv1/source/` for review. The lockfile
captures all resolved transitive dependencies, including `mp3lame-sys 0.2.0`
and its build dependency `cc`; transitive versions must not be inferred from
the semver requirements above.

## Format and build decisions

- WAV is the default and is written as IEEE-f32 by `candle-roformer`; the CLI
  does not convert it to integer PCM or apply hidden dithering.
- FLAC is lossless 24-bit PCM. Synthetic round-trip tests use the project's
  `read_audio` decoder and require exact sample rate and sample count.
- MP3 is lossy CBR (default `192 kbps`). The same decoder must report the
  requested sample rate and a duration within `0.05 s` of the source to allow
  for encoder/decoder frame delay and padding.
- `--sample_rate` uses `candle-roformer's` Rust resampler before encoding.
  Invalid rates, unsupported MP3 rates, unsupported bitrates, and a bitrate
  supplied for a non-MP3 format are rejected before output work begins.
- M4A/AAC output is intentionally unsupported: this project has no bundled
  offline Rust AAC/M4A encoder and does not shell out to FFmpeg or system
  encoders. The CLI returns a clear error stating that no bundled offline
  AAC/M4A encoder is available. Input decoding support is separate from output
  encoding support.

All codec verification uses generated synthetic audio; no user media or model
weights are included in the dependency audit.
