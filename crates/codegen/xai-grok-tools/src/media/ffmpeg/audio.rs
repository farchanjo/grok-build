//! Normalized bounded PCM output from the native FFmpeg layer.
//!
//! The C shim decodes the whole audio stream to interleaved float32,
//! normalized to `[-1, 1]`, capped by `FfmpegLimits::max_audio_samples`.

use super::abi::GrokAvPcm;

/// Bounded normalized PCM produced by [`crate::media::ffmpeg::DecodeSession`].
///
/// `samples` is interleaved `(frames * channels)` float32 values normalized
/// to `[-1, 1]`. This is a plain Rust-owned buffer copied out of the native
/// context immediately after decoding.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedPcm {
    /// Interleaved samples, `len = frames * channels`.
    pub samples: Vec<f32>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u32,
    /// True when the audio cap stopped output early (`max_audio_samples`).
    pub truncated: bool,
}

impl DecodedPcm {
    /// Convert a native `GrokAvPcm` into an owned Rust buffer. The native
    /// buffer is released by the caller via `grok_av_pcm_free`.
    pub(crate) fn from_native(c: &GrokAvPcm) -> Self {
        let len = c.len;
        let samples = if len > 0 && !c.data.is_null() {
            // SAFETY: the shim guarantees `data` points to `len` float32
            // samples for a successful decode.
            unsafe { std::slice::from_raw_parts(c.data, len) }.to_vec()
        } else {
            Vec::new()
        };
        DecodedPcm {
            samples,
            sample_rate: c.sample_rate.max(0) as u32,
            channels: c.channels.max(0) as u32,
            truncated: c.truncated != 0,
        }
    }

    /// Number of sample frames (`samples.len() / channels`).
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }

    /// Duration in seconds as a float.
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.sample_rate as f64
        }
    }

    /// Downmix to a single mono channel by averaging channels per frame.
    pub fn to_mono(&self) -> Vec<f32> {
        let channels = self.channels as usize;
        if channels == 0 {
            return Vec::new();
        }
        if channels == 1 {
            return self.samples.clone();
        }
        let frames = self.frames();
        let mut mono = Vec::with_capacity(frames);
        for frame in self.samples.chunks_exact(channels) {
            let sum: f32 = frame.iter().sum();
            mono.push(sum / channels as f32);
        }
        mono
    }
}
