//! A PSS file as a whole: demultiplexes the program stream, decodes video
//! frames on demand and collects the audio decoded along the way.
use crate::audio::{AudioDecoder, AudioHeader};
use crate::ps::{PsReader, STREAM_PRIVATE_1, STREAM_VIDEO_0};
use crate::video::{Frame, SequenceInfo, Stats, VideoDecoder};
use crate::Result;
use std::io::Read;

/// What is known about the movie so far (complete after the first frame).
#[derive(Clone, Debug, Default)]
pub struct MovieInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: (u32, u32),
    pub audio: Option<AudioHeader>,
    pub first_video_pts: Option<u64>,
    pub first_audio_pts: Option<u64>,
}

pub struct MovieReader<R> {
    ps: PsReader<R>,
    video: VideoDecoder,
    audio: AudioDecoder,
    audio_substream: Option<u8>,
    audio_out: Vec<i16>,
    info: MovieInfo,
    done: bool,
}

impl<R: Read> MovieReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            ps: PsReader::new(reader),
            video: VideoDecoder::new(),
            audio: AudioDecoder::new(),
            audio_substream: None,
            audio_out: Vec::new(),
            info: MovieInfo::default(),
            done: false,
        }
    }

    pub fn set_strict(&mut self, strict: bool) {
        self.video.set_strict(strict);
    }

    pub fn info(&self) -> &MovieInfo {
        &self.info
    }

    pub fn video_stats(&self) -> &Stats {
        self.video.stats()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.video.last_error()
    }

    pub fn sequence(&self) -> Option<&SequenceInfo> {
        self.video.sequence()
    }

    /// True once the program stream ended with its end code.
    pub fn ended_cleanly(&self) -> bool {
        self.ps.saw_end_code()
    }

    /// Interleaved PCM decoded so far and not yet taken.
    pub fn take_audio(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.audio_out)
    }

    pub fn audio_available(&self) -> usize {
        self.audio_out.len()
    }

    /// Read one more packet. Returns false at the end of the stream.
    pub fn pump(&mut self) -> Result<bool> {
        if self.done {
            return Ok(false);
        }
        let Some(packet) = self.ps.next_packet()? else {
            self.video.flush()?;
            self.done = true;
            return Ok(false);
        };
        match packet.stream_id {
            STREAM_VIDEO_0 => {
                if self.info.first_video_pts.is_none() {
                    self.info.first_video_pts = packet.pts;
                }
                self.video.push(&packet.payload)?;
                if self.info.width == 0 {
                    if let Some(seq) = self.video.sequence() {
                        self.info.width = seq.width;
                        self.info.height = seq.height;
                        self.info.frame_rate = seq.frame_rate;
                    }
                }
            }
            STREAM_PRIVATE_1 => {
                let sub = packet.substream;
                // The first audio substream (0xA0..) is the one played.
                if self.audio_substream.is_none() && sub.is_some_and(|s| s & 0xF0 == 0xA0) {
                    self.audio_substream = sub;
                }
                if sub.is_some() && sub == self.audio_substream {
                    if self.info.first_audio_pts.is_none() {
                        self.info.first_audio_pts = packet.pts;
                    }
                    self.audio.push(&packet.payload, &mut self.audio_out)?;
                    if self.info.audio.is_none() {
                        self.info.audio = self.audio.header();
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }

    /// The next frame in display order, or `None` at the end.
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            if let Some(frame) = self.video.next_frame() {
                return Ok(Some(frame));
            }
            if !self.pump()? {
                return Ok(self.video.next_frame());
            }
        }
    }
}

/// Demultiplex and decode only the audio of a PSS (no video decoding).
pub fn decode_audio(reader: impl Read) -> Result<(Option<AudioHeader>, Vec<i16>)> {
    let mut ps = PsReader::new(reader);
    let mut audio = AudioDecoder::new();
    let mut out = Vec::new();
    let mut substream = None;
    while let Some(packet) = ps.next_packet()? {
        if packet.stream_id != STREAM_PRIVATE_1 {
            continue;
        }
        if substream.is_none() && packet.substream.is_some_and(|s| s & 0xF0 == 0xA0) {
            substream = packet.substream;
        }
        if packet.substream.is_some() && packet.substream == substream {
            audio.push(&packet.payload, &mut out)?;
        }
    }
    Ok((audio.header(), out))
}
