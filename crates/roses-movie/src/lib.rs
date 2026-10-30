//! Pure-Rust decoding of PlayStation 2 PSS movies from a user-supplied disc.
//!
//! A PSS file is an MPEG-2 program stream: video is an MPEG-2 elementary
//! stream on stream 0xE0, audio is a Sony "SShd/SSbd" stream carried in
//! private stream 1 (0xBD) behind a four-byte substream tag. This crate has
//! no dependencies and no unsafe code; it never touches the network or any
//! system codec. Nothing from a game disc is embedded here: callers pass the
//! bytes they read from the player's own ISO.
//!
//! * [`ps`] — program stream demultiplexing (streaming, from any `Read`).
//! * [`audio`] — SShd/SSbd headers, 16-bit PCM and PS-ADPCM to interleaved PCM.
//! * [`video`] — MPEG-2 Main Profile video (I/P/B, frame and field pictures).
//! * [`movie`] — a file-level reader joining the two for playback.

pub mod audio;
mod bits;
mod idct;
pub mod movie;
pub mod ps;
mod tables;
pub mod video;

pub use movie::{MovieInfo, MovieReader};
pub use video::{Frame, VideoDecoder};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    /// Bytes that do not follow the format.
    Invalid(String),
    /// Valid syntax this decoder deliberately does not implement.
    Unsupported(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Invalid(s) => write!(f, "invalid movie data: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported movie feature: {s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn invalid<T>(reason: impl Into<String>) -> Result<T> {
    Err(Error::Invalid(reason.into()))
}
pub(crate) fn unsupported<T>(reason: impl Into<String>) -> Result<T> {
    Err(Error::Unsupported(reason.into()))
}
