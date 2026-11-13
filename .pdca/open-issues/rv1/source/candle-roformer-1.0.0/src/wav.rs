use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::error::{AudioError, Error};

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    pub planar: Vec<f32>,
}

impl AudioBuffer {
    pub fn frames(&self) -> usize {
        self.planar.len() / self.channels.max(1) as usize
    }

    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.sample_rate as f64
        }
    }

    /// Reject buffers the rest of the pipeline cannot process faithfully:
    /// non-audio sample rates, more than two channels, an empty or
    /// channel-misaligned buffer, and non-finite samples.
    pub fn validate(&self) -> Result<(), Error> {
        if self.sample_rate == 0 || self.sample_rate > 384_000 {
            return Err(Error::Audio(AudioError::InvalidSampleRate(format!(
                "invalid sample rate {}",
                self.sample_rate
            ))));
        }
        if self.channels == 0 || self.channels > 2 {
            return Err(Error::Audio(AudioError::TooManyChannels(format!(
                "unsupported channel count {}",
                self.channels
            ))));
        }
        let ch = self.channels as usize;
        if self.planar.is_empty() || self.planar.len() % ch != 0 {
            return Err(Error::Audio(AudioError::UnsupportedFormat(
                "buffer length is not a whole number of frames".into(),
            )));
        }
        if self.planar.iter().any(|s| !s.is_finite()) {
            return Err(Error::Audio(AudioError::NonFinite(
                "buffer contains non-finite samples".into(),
            )));
        }
        Ok(())
    }
}

pub fn read_audio(path: impl AsRef<Path>) -> Result<AudioBuffer, Error> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| Error::Audio(AudioError::UnsupportedFormat(e.to_string())))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| Error::Audio(AudioError::UnsupportedFormat("no audio track".into())))?;

    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .ok_or_else(|| Error::Audio(AudioError::UnsupportedFormat("no codec parameters".into())))?
        .audio()
        .ok_or_else(|| Error::Audio(AudioError::UnsupportedFormat("not an audio codec".into())))?
        .clone();

    let sample_rate = codec_params
        .sample_rate
        .ok_or_else(|| Error::Audio(AudioError::InvalidSampleRate("missing sample rate".into())))?;
    if sample_rate == 0 || sample_rate > 384000 {
        return Err(Error::Audio(AudioError::InvalidSampleRate(format!(
            "absurd sample rate {}",
            sample_rate
        ))));
    }
    let channels = codec_params
        .channels
        .as_ref()
        .map(|c| c.count() as u16)
        .unwrap_or(1);
    if channels == 0 || channels > 2 {
        return Err(Error::Audio(AudioError::TooManyChannels(format!(
            "unsupported channel count {}",
            channels
        ))));
    }

    // FLAC-in-MP4 streams (rare, but real) are decoded incorrectly by
    // symphonia for some bitstreams (the second channel comes out wrong).
    // Route FLAC through claxon: rebuild a raw FLAC stream from the demuxed
    // frames plus the STREAMINFO block and decode that.
    if codec_params.codec == symphonia::core::codecs::audio::well_known::CODEC_ID_FLAC {
        return read_flac(path, &mut format, track_id);
    }

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
        .map_err(|e| Error::Audio(AudioError::UnsupportedFormat(e.to_string())))?;

    let mut planar: Vec<Vec<f32>> = vec![Vec::new(); channels as usize];

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => {
                return Err(Error::Audio(AudioError::UnsupportedFormat(e.to_string())));
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(Error::Audio(AudioError::UnsupportedFormat(e.to_string()))),
        };

        let ch = decoded.spec().channels().count();
        if ch == 0 {
            continue;
        }
        let mut interleaved = vec![0f32; decoded.frames() * ch];
        decoded.copy_to_slice_interleaved(&mut interleaved);

        let frames = interleaved.len() / ch;
        for f in 0..frames {
            for c in 0..channels as usize {
                // A packet with fewer channels than the stream (rare, malformed) is
                // refused rather than silently mis-interleaved.
                if c >= ch {
                    return Err(Error::Audio(AudioError::UnsupportedFormat(format!(
                        "decoder returned {} channels, expected {}",
                        ch, channels
                    ))));
                }
                let v = interleaved[f * ch + c];
                if !v.is_finite() {
                    return Err(Error::Audio(AudioError::NonFinite(
                        "non-finite sample".into(),
                    )));
                }
                planar[c].push(v);
            }
        }
    }

    let frames = planar[0].len();
    if frames == 0 {
        return Err(Error::Audio(AudioError::EmptyInput("empty audio".into())));
    }
    for c in 0..channels as usize {
        if planar[c].len() != frames {
            return Err(Error::Audio(AudioError::UnsupportedFormat(
                "channel length mismatch".into(),
            )));
        }
    }

    let mut planar_flat = Vec::with_capacity(channels as usize * frames);
    for c in 0..channels as usize {
        planar_flat.extend_from_slice(&planar[c]);
    }

    Ok(AudioBuffer {
        sample_rate,
        channels,
        planar: planar_flat,
    })
}

pub fn write_wav_f32(path: impl AsRef<Path>, buf: &AudioBuffer) -> Result<(), Error> {
    use std::io::Write;
    let path = path.as_ref();
    let channels = buf.channels as u16;
    let sr = buf.sample_rate;
    let bits_per_sample: u16 = 32;
    let byte_rate = sr * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = (buf.planar.len() * 4) as u32;
    let file_size = 36 + data_size;
    let channels_usize = channels as usize;
    let frames = buf.planar.len() / channels_usize;

    let mut f = File::create(path).map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
    // RIFF header
    f.write_all(b"RIFF").unwrap();
    f.write_all(&file_size.to_le_bytes()).unwrap();
    f.write_all(b"WAVE").unwrap();
    // fmt chunk
    f.write_all(b"fmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    f.write_all(&3u16.to_le_bytes()).unwrap(); // audio format IEEE float
    f.write_all(&channels.to_le_bytes()).unwrap();
    f.write_all(&sr.to_le_bytes()).unwrap();
    f.write_all(&byte_rate.to_le_bytes()).unwrap();
    f.write_all(&block_align.to_le_bytes()).unwrap();
    f.write_all(&bits_per_sample.to_le_bytes()).unwrap();
    // data chunk
    f.write_all(b"data").unwrap();
    f.write_all(&data_size.to_le_bytes()).unwrap();
    // WAV data is interleaved: frame-major, channel-minor.
    for frame in 0..frames {
        for c in 0..channels_usize {
            let s = buf.planar[c * frames + frame];
            if !s.is_finite() {
                return Err(Error::Audio(AudioError::NonFinite(
                    "non-finite output".into(),
                )));
            }
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }
    Ok(())
}

fn read_flac(
    path: &Path,
    format: &mut Box<dyn symphonia::core::formats::FormatReader>,
    track_id: u32,
) -> Result<AudioBuffer, Error> {
    use std::io::Cursor;
    let bytes = std::fs::read(path).map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
    let mut stream: Vec<u8> = Vec::new();
    if bytes.starts_with(b"fLaC") {
        stream = bytes;
    } else {
        // MP4 (dfLa box): 'dfLa' + version/flags (4) + FLAC metadata block
        // header (4, 0x80|type and 24-bit length) + STREAMINFO payload.
        let needle = b"dfLa";
        let pos = bytes
            .windows(needle.len())
            .position(|w| w == needle)
            .ok_or_else(|| {
                Error::Audio(AudioError::UnsupportedFormat(
                    "FLAC stream without dfLa box".into(),
                ))
            })?;
        let hdr_start = pos + 4 + 4;
        if hdr_start + 4 > bytes.len() {
            return Err(Error::Audio(AudioError::UnsupportedFormat(
                "truncated dfLa box".into(),
            )));
        }
        let block_hdr = &bytes[hdr_start..hdr_start + 4];
        let len = ((block_hdr[1] as usize) << 16)
            | ((block_hdr[2] as usize) << 8)
            | (block_hdr[3] as usize);
        let data_start = hdr_start + 4;
        if data_start + len > bytes.len() {
            return Err(Error::Audio(AudioError::UnsupportedFormat(
                "truncated STREAMINFO".into(),
            )));
        }
        stream.extend_from_slice(b"fLaC");
        stream.extend_from_slice(block_hdr);
        stream.extend_from_slice(&bytes[data_start..data_start + len]);
        loop {
            match format.next_packet() {
                Ok(Some(packet)) => {
                    if packet.track_id == track_id {
                        stream.extend_from_slice(&packet.data);
                    }
                }
                Ok(None) => break,
                Err(SymphoniaError::ResetRequired) => break,
                Err(e) => return Err(Error::Audio(AudioError::UnsupportedFormat(e.to_string()))),
            }
        }
    }

    let mut reader = claxon::FlacReader::new(Cursor::new(stream))
        .map_err(|e| Error::Audio(AudioError::UnsupportedFormat(format!("flac: {}", e))))?;
    let info = reader.streaminfo();
    let channels = info.channels as usize;
    if channels == 0 || channels > 2 {
        return Err(Error::Audio(AudioError::TooManyChannels(format!(
            "unsupported channel count {}",
            channels
        ))));
    }
    let bits = if info.bits_per_sample == 0 || info.bits_per_sample > 32 {
        16
    } else {
        info.bits_per_sample
    };
    let scale = 1.0f32 / (1u64 << (bits - 1)) as f32;
    let mut planar: Vec<Vec<f32>> = vec![Vec::new(); channels];
    let mut idx = 0usize;
    for sample in reader.samples() {
        let v = sample
            .map_err(|e| Error::Audio(AudioError::UnsupportedFormat(format!("flac: {}", e))))?
            as f32
            * scale;
        if !v.is_finite() {
            return Err(Error::Audio(AudioError::NonFinite(
                "non-finite sample".into(),
            )));
        }
        planar[idx % channels].push(v);
        idx += 1;
    }
    let frames = planar[0].len();
    if frames == 0 {
        return Err(Error::Audio(AudioError::EmptyInput("empty audio".into())));
    }
    for c in 0..channels {
        if planar[c].len() != frames {
            return Err(Error::Audio(AudioError::UnsupportedFormat(
                "channel length mismatch".into(),
            )));
        }
    }
    let mut planar_flat = Vec::with_capacity(channels * frames);
    for c in 0..channels {
        planar_flat.extend_from_slice(&planar[c]);
    }
    Ok(AudioBuffer {
        sample_rate: info.sample_rate,
        channels: channels as u16,
        planar: planar_flat,
    })
}

use std::io::{Seek, SeekFrom, Write};

/// A WAV writer that appends f32 frames incrementally and finalises the RIFF
/// size fields on close. Used by the bounded-memory file separation path so
/// that only the current chunk (not the whole output) is held in memory.
pub struct StreamingWavWriter {
    file: File,
    channels: u16,
    data_bytes: u64,
}

impl StreamingWavWriter {
    pub fn create(
        path: impl AsRef<std::path::Path>,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, Error> {
        let mut f =
            File::create(path.as_ref()).map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
        let bits_per_sample: u16 = 32;
        let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
        let block_align = channels * (bits_per_sample / 8);
        // Placeholder header; sizes patched in `finalize`.
        f.write_all(b"RIFF").unwrap();
        f.write_all(&0u32.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&3u16.to_le_bytes()).unwrap(); // IEEE float
        f.write_all(&channels.to_le_bytes()).unwrap();
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        f.write_all(&byte_rate.to_le_bytes()).unwrap();
        f.write_all(&block_align.to_le_bytes()).unwrap();
        f.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&0u32.to_le_bytes()).unwrap();
        Ok(Self {
            file: f,
            channels,
            data_bytes: 0,
        })
    }

    /// Append `planar` samples (layout `channels * frames`, planar) to the file.
    pub fn write_planar(&mut self, planar: &[f32]) -> Result<(), Error> {
        let channels = self.channels as usize;
        if planar.len() % channels != 0 {
            return Err(Error::Audio(AudioError::UnsupportedFormat(
                "planar length is not a whole number of frames".into(),
            )));
        }
        let frames = planar.len() / channels;
        for frame in 0..frames {
            for c in 0..channels {
                let s = planar[c * frames + frame];
                if !s.is_finite() {
                    return Err(Error::Audio(AudioError::NonFinite(
                        "non-finite output".into(),
                    )));
                }
                self.file
                    .write_all(&s.to_le_bytes())
                    .map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
            }
        }
        self.data_bytes += (planar.len() * 4) as u64;
        Ok(())
    }

    pub fn finalize(mut self) -> Result<(), Error> {
        let file_size = 36u32 + self.data_bytes as u32;
        let data_size = self.data_bytes as u32;
        self.file
            .seek(SeekFrom::Start(4))
            .map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
        self.file
            .write_all(&file_size.to_le_bytes())
            .map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
        // data chunk size is at offset 40
        self.file
            .seek(SeekFrom::Start(40))
            .map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
        self.file
            .write_all(&data_size.to_le_bytes())
            .map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
        self.file
            .flush()
            .map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
        Ok(())
    }
}

/// Incremental decoder used by the bounded-memory file separation path.
pub struct AudioStreamReader {
    inner: ReaderInner,
}

enum ReaderInner {
    Sym {
        format: Box<dyn symphonia::core::formats::FormatReader>,
        track_id: u32,
        decoder: Box<dyn symphonia::core::codecs::audio::AudioDecoder>,
        channels: usize,
        sample_rate: u32,
        pending: Vec<f32>, // interleaved
        eof: bool,
    },
    Buffered {
        data: Vec<f32>, // planar
        channels: usize,
        sample_rate: u32,
        pos: usize,
    },
}

impl AudioStreamReader {
    pub fn open(path: &std::path::Path) -> Result<Self, Error> {
        let file = File::open(path).map_err(|e| Error::Audio(AudioError::Io(e.to_string())))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| Error::Audio(AudioError::UnsupportedFormat(e.to_string())))?;

        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| Error::Audio(AudioError::UnsupportedFormat("no audio track".into())))?;

        let track_id = track.id;
        let codec_params = track
            .codec_params
            .as_ref()
            .ok_or_else(|| Error::Audio(AudioError::UnsupportedFormat("no codec params".into())))?
            .audio()
            .ok_or_else(|| Error::Audio(AudioError::UnsupportedFormat("not audio".into())))?
            .clone();

        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| Error::Audio(AudioError::InvalidSampleRate("missing".into())))?;
        let channels = codec_params
            .channels
            .as_ref()
            .map(|c| c.count())
            .unwrap_or(1);
        if channels == 0 || channels > 2 {
            return Err(Error::Audio(AudioError::TooManyChannels(format!(
                "unsupported channel count {}",
                channels
            ))));
        }

        // FLAC: decode whole via read_audio and serve buffered.
        if codec_params.codec == symphonia::core::codecs::audio::well_known::CODEC_ID_FLAC {
            let buf = read_audio(path)?;
            return Ok(Self {
                inner: ReaderInner::Buffered {
                    data: buf.planar,
                    channels: buf.channels as usize,
                    sample_rate: buf.sample_rate,
                    pos: 0,
                },
            });
        }

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
            .map_err(|e| Error::Audio(AudioError::UnsupportedFormat(e.to_string())))?;

        Ok(Self {
            inner: ReaderInner::Sym {
                format,
                track_id,
                decoder,
                channels,
                sample_rate,
                pending: Vec::new(),
                eof: false,
            },
        })
    }

    pub fn sample_rate(&self) -> u32 {
        match &self.inner {
            ReaderInner::Sym { sample_rate, .. } => *sample_rate,
            ReaderInner::Buffered { sample_rate, .. } => *sample_rate,
        }
    }

    pub fn channels(&self) -> u16 {
        match &self.inner {
            ReaderInner::Sym { channels, .. } => *channels as u16,
            ReaderInner::Buffered { channels, .. } => *channels as u16,
        }
    }

    pub fn read_frames(&mut self, n: usize) -> Result<Vec<f32>, Error> {
        match &mut self.inner {
            ReaderInner::Buffered {
                data,
                channels,
                pos,
                ..
            } => {
                let ch = *channels;
                let frames_avail = (data.len() / ch).saturating_sub(*pos);
                let take = n.min(frames_avail);
                if take == 0 {
                    return Ok(Vec::new());
                }
                let mut out = vec![0f32; ch * take];
                for c in 0..ch {
                    for i in 0..take {
                        out[c * take + i] = data[c * (data.len() / ch) + (*pos + i)];
                    }
                }
                *pos += take;
                Ok(out)
            }
            ReaderInner::Sym {
                format,
                track_id,
                decoder,
                channels,
                pending,
                eof,
                ..
            } => {
                let ch = *channels;
                // Collect enough interleaved samples.
                while !*eof && pending.len() < n * ch {
                    let packet = match format.next_packet() {
                        Ok(Some(p)) => p,
                        Ok(None) => {
                            *eof = true;
                            break;
                        }
                        Err(SymphoniaError::ResetRequired) => {
                            decoder.reset();
                            continue;
                        }
                        Err(e) => {
                            return Err(Error::Audio(AudioError::UnsupportedFormat(e.to_string())));
                        }
                    };
                    if packet.track_id != *track_id {
                        continue;
                    }
                    let decoded = match decoder.decode(&packet) {
                        Ok(d) => d,
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(e) => {
                            return Err(Error::Audio(AudioError::UnsupportedFormat(e.to_string())));
                        }
                    };
                    let pc = decoded.spec().channels().count();
                    if pc == 0 {
                        continue;
                    }
                    let mut interleaved = vec![0f32; decoded.frames() * pc];
                    decoded.copy_to_slice_interleaved(&mut interleaved);
                    let frames = interleaved.len() / pc;
                    for f in 0..frames {
                        for c in 0..ch {
                            if c >= pc {
                                return Err(Error::Audio(AudioError::UnsupportedFormat(format!(
                                    "decoder returned {} channels, expected {}",
                                    pc, ch
                                ))));
                            }
                            let v = interleaved[f * pc + c];
                            if !v.is_finite() {
                                return Err(Error::Audio(AudioError::NonFinite(
                                    "non-finite sample".into(),
                                )));
                            }
                            pending.push(v);
                        }
                    }
                }
                let frames_avail = pending.len() / ch;
                let take = n.min(frames_avail);
                if take == 0 {
                    return Ok(Vec::new());
                }
                // Convert first take frames interleaved -> planar.
                let mut out = vec![0f32; ch * take];
                for f in 0..take {
                    for c in 0..ch {
                        out[c * take + f] = pending[f * ch + c];
                    }
                }
                pending.drain(0..take * ch);
                Ok(out)
            }
        }
    }
}
