//! Legacy DAB MPEG Audio Layer II decoder.
//!
//! After UEP/EEP channel decoding and energy dispersal, legacy DAB audio
//! produces a byte stream of MPEG-1/2 Layer II frames. At MPEG-1/48 kHz one
//! 24 ms DAB logical frame is exactly one 1152-sample MP2 frame; buffering and
//! sync recovery also cover other legal Layer II rates and damaged frames.

use oxideav_mp2::frame::{FrameDecodeState, decode_frame_with};
use oxideav_mp2::header::{FrameHeader, find_sync};
use tracing::{debug, info, warn};

/// PCM produced from one or more complete MP2 frames.
#[derive(Debug, Default)]
pub struct Mp2DecodeOutput {
    /// Interleaved normalized PCM samples.
    pub pcm: Vec<f32>,
    pub sample_rate: u32,
    pub channels: usize,
}

impl Mp2DecodeOutput {
    /// Convert the DAB-legal 48 kHz or 24 kHz mono/stereo PCM formats to the
    /// application's fixed 48 kHz stereo output. MPEG-2/24 kHz samples are
    /// repeated by 2; mono is duplicated without changing level.
    pub fn into_stereo_48k(self) -> Option<Vec<f32>> {
        if self.pcm.is_empty() {
            return Some(Vec::new());
        }
        if !matches!(self.channels, 1 | 2) || !matches!(self.sample_rate, 24_000 | 48_000) {
            return None;
        }

        let mut stereo = Vec::with_capacity(self.pcm.len() * 2 / self.channels);
        for frame in self.pcm.chunks_exact(self.channels) {
            let left = frame[0];
            let right = if self.channels == 2 { frame[1] } else { left };
            stereo.extend_from_slice(&[left, right]);
        }
        if self.sample_rate == 48_000 {
            return Some(stereo);
        }

        let mut upsampled = Vec::with_capacity(stereo.len() * 2);
        for frame in stereo.chunks_exact(2) {
            upsampled.extend_from_slice(frame);
            upsampled.extend_from_slice(frame);
        }
        Some(upsampled)
    }
}

/// Stateful Layer II frame synchronizer and decoder.
#[derive(Debug)]
pub struct Mp2Decoder {
    buffer: Vec<u8>,
    state: FrameDecodeState,
    configured: bool,
    pub frames_decoded: usize,
    pub decode_errors: usize,
    pub sync_bytes_skipped: usize,
    pub sample_rate: u32,
    pub channels: usize,
}

impl Mp2Decoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            state: FrameDecodeState::new(),
            configured: false,
            frames_decoded: 0,
            decode_errors: 0,
            sync_bytes_skipped: 0,
            sample_rate: 0,
            channels: 0,
        }
    }

    /// Feed one 24 ms DAB logical frame and decode every complete MPEG Layer II
    /// frame now available.
    pub fn feed_frame(&mut self, logical_frame: &[u8]) -> Mp2DecodeOutput {
        self.buffer.extend_from_slice(logical_frame);
        let mut pcm = Vec::new();

        loop {
            let Some(sync) = find_sync(&self.buffer) else {
                // Preserve one trailing 0xFF so a split syncword survives.
                let keep = usize::from(self.buffer.last() == Some(&0xFF));
                let drop_n = self.buffer.len().saturating_sub(keep);
                self.sync_bytes_skipped += drop_n;
                if drop_n > 0 {
                    self.buffer.drain(..drop_n);
                }
                break;
            };
            if sync > 0 {
                self.sync_bytes_skipped += sync;
                self.buffer.drain(..sync);
            }
            if self.buffer.len() < 4 {
                break;
            }

            let header = match FrameHeader::parse(&self.buffer) {
                Ok(header) => header,
                Err(_) => {
                    self.sync_bytes_skipped += 1;
                    self.buffer.drain(..1);
                    continue;
                }
            };
            let frame_size = header.frame_size_bytes();
            if self.buffer.len() < frame_size {
                break;
            }

            match decode_frame_with(&self.buffer[..frame_size], &mut self.state) {
                Ok(decoded) => {
                    let channels = decoded.header.channels();
                    let samples_per_channel = decoded.pcm.first().map_or(0, Vec::len);
                    pcm.reserve(samples_per_channel * channels);
                    for i in 0..samples_per_channel {
                        for ch in 0..channels {
                            pcm.push(decoded.pcm[ch][i].clamp(-1.0, 1.0) as f32);
                        }
                    }
                    self.frames_decoded += 1;
                    self.sample_rate = decoded.header.sample_rate;
                    self.channels = channels;
                    if !self.configured {
                        self.configured = true;
                        info!(
                            sample_rate = self.sample_rate,
                            channels = self.channels,
                            bitrate_bps = decoded.header.bit_rate,
                            "Legacy DAB MPEG Layer II audio configured"
                        );
                    }
                    self.buffer.drain(..frame_size);
                }
                Err(error) => {
                    self.decode_errors += 1;
                    if self.decode_errors <= 5 {
                        warn!(error = %error, "MP2 frame decode failed; searching for next sync");
                    } else {
                        debug!(error = %error, "MP2 frame decode failed");
                    }
                    // Advance one byte rather than discarding a potentially valid
                    // following frame after a false sync or corrupted frame.
                    self.buffer.drain(..1);
                }
            }
        }

        Mp2DecodeOutput {
            pcm,
            sample_rate: self.sample_rate,
            channels: self.channels,
        }
    }
}

impl Default for Mp2Decoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_split_syncword() {
        let mut decoder = Mp2Decoder::new();
        let out = decoder.feed_frame(&[1, 2, 3, 0xFF]);
        assert!(out.pcm.is_empty());
        assert_eq!(decoder.buffer, vec![0xFF]);
        assert_eq!(decoder.sync_bytes_skipped, 3);
    }

    #[test]
    fn skips_bytes_before_sync_and_waits_for_complete_header() {
        let mut decoder = Mp2Decoder::new();
        let out = decoder.feed_frame(&[7, 8, 0xFF, 0xFD]);
        assert!(out.pcm.is_empty());
        assert_eq!(&decoder.buffer[..2], &[0xFF, 0xFD]);
        assert_eq!(decoder.sync_bytes_skipped, 2);
    }

    #[test]
    fn converts_24khz_mono_to_48khz_stereo() {
        let out = Mp2DecodeOutput {
            pcm: vec![0.0, 1.0],
            sample_rate: 24_000,
            channels: 1,
        };
        assert_eq!(
            out.into_stereo_48k().unwrap(),
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]
        );
    }
}
