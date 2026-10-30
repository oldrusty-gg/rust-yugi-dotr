//! Narrow, fingerprint-gated PAL opening PSS demux. MPEG video decoding is left
//! to an installed decoder; the Sony ADS audio is extracted without alteration.
use crate::{Error, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PAL_OPENING_MOVIE_SHA256: &str =
    "649bbb1683c1d3525990d939f1420ccefff946e0380109ff9b4c5cb0150d0c52";
const PAL_OPENING_MOVIE_BYTES: usize = 140_247_044;
const MAX_AUDIO_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct OpeningMovieMetadata {
    pub source_sha256: String,
    pub width: u16,
    pub height: u16,
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub video_frames: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub audio_samples_per_channel: u64,
    pub first_video_pts_90khz: u64,
    pub first_audio_pts_90khz: u64,
    pub last_video_pts_90khz: u64,
    pub last_audio_pts_90khz: u64,
    pub audio_ss2_sha256: String,
}

pub struct OpeningMovie {
    pub audio_ss2: Vec<u8>,
    pub metadata: OpeningMovieMetadata,
}

fn invalid(reason: &str) -> Error {
    Error::Invalid(format!("opening PSS: {reason}"))
}

/// Accepts exactly the researched original PAL opening file. Returned audio is
/// a standalone SShd/SSbd stream: each private PES's four-byte substream tag is
/// removed, with the remaining bytes concatenated in source order.
pub fn demux_pal_opening_movie(source: &[u8]) -> Result<OpeningMovie> {
    if source.len() != PAL_OPENING_MOVIE_BYTES {
        return Err(Error::Unsupported("opening PSS byte length".into()));
    }
    let hash = format!("{:x}", Sha256::digest(source));
    if hash != PAL_OPENING_MOVIE_SHA256 {
        return Err(Error::Unsupported(format!("opening PSS SHA256 {hash}")));
    }
    let mut movie = parse(source)?;
    if (
        movie.metadata.video_frames,
        movie.metadata.audio_samples_per_channel,
    ) != (2867, 5_504_768)
    {
        return Err(invalid("unexpected PAL stream duration"));
    }
    movie.metadata.source_sha256 = hash;
    Ok(movie)
}

fn pts(bytes: &[u8]) -> Result<u64> {
    if bytes.len() < 5 || bytes[0] & 1 == 0 || bytes[2] & 1 == 0 || bytes[4] & 1 == 0 {
        return Err(invalid("invalid PTS marker bits or length"));
    }
    Ok(u64::from((bytes[0] >> 1) & 7) << 30
        | u64::from(bytes[1]) << 22
        | u64::from(bytes[2] >> 1) << 15
        | u64::from(bytes[3]) << 7
        | u64::from(bytes[4] >> 1))
}

fn parse(source: &[u8]) -> Result<OpeningMovie> {
    let mut at = 0usize;
    let mut ended = false;
    let mut audio = Vec::new();
    let mut video_pts = BTreeSet::new();
    let mut audio_pts = BTreeSet::new();
    let mut sequence_seen = false;
    while at < source.len() {
        let head = source
            .get(at..at + 4)
            .ok_or_else(|| invalid("truncated start code"))?;
        if head[..3] != [0, 0, 1] {
            return Err(invalid("unexpected packet prefix"));
        }
        let stream = head[3];
        if stream == 0xB9 {
            at += 4;
            ended = at == source.len();
            if !ended {
                return Err(invalid("bytes after program end"));
            }
            break;
        }
        if stream == 0xBA {
            let pack = source
                .get(at..at + 14)
                .ok_or_else(|| invalid("truncated pack header"))?;
            if pack[4] & 0xC0 != 0x40 {
                return Err(invalid("unsupported pack header"));
            }
            at += 14 + usize::from(pack[13] & 7);
            if at > source.len() {
                return Err(invalid("truncated pack stuffing"));
            }
            continue;
        }
        let length = source
            .get(at + 4..at + 6)
            .ok_or_else(|| invalid("missing packet length"))?;
        let end = at + 6 + usize::from(u16::from_be_bytes([length[0], length[1]]));
        let packet = source
            .get(at..end)
            .ok_or_else(|| invalid("packet exceeds input"))?;
        match stream {
            0xBB | 0xBE => {}
            0xE0 | 0xBD => {
                if packet.len() < 9 || packet[6] & 0xC0 != 0x80 {
                    return Err(invalid("unsupported or truncated PES header"));
                }
                let payload_at = 9 + usize::from(packet[8]);
                let payload = packet
                    .get(payload_at..)
                    .ok_or_else(|| invalid("PES header exceeds packet"))?;
                if packet[7] & 0x80 != 0 {
                    let header = packet
                        .get(9..payload_at)
                        .ok_or_else(|| invalid("invalid PES header range"))?;
                    let timestamp = pts(header)?;
                    if stream == 0xE0 {
                        video_pts.insert(timestamp);
                    } else {
                        audio_pts.insert(timestamp);
                    }
                }
                if stream == 0xBD {
                    if payload.get(..4) != Some(&[0xFF, 0xA0, 0, 0]) {
                        return Err(invalid("unsupported private audio substream"));
                    }
                    if audio.len() + payload.len() - 4 > MAX_AUDIO_BYTES {
                        return Err(invalid("audio exceeds bound"));
                    }
                    audio.extend_from_slice(&payload[4..]);
                } else if !sequence_seen {
                    if let Some(offset) = payload
                        .windows(4)
                        .position(|bytes| bytes == [0, 0, 1, 0xB3])
                    {
                        let sequence = payload
                            .get(offset + 4..offset + 8)
                            .ok_or_else(|| invalid("truncated video sequence"))?;
                        // 640x448, square sample pixels, frame-rate code 3 (25 Hz).
                        if sequence != [0x28, 0x01, 0xC0, 0x13] {
                            return Err(invalid("unsupported video dimensions, aspect or rate"));
                        }
                        sequence_seen = true;
                    }
                }
            }
            _ => return Err(invalid("unexpected stream ID")),
        }
        at = end;
    }
    if !ended || !sequence_seen {
        return Err(invalid("missing end code or supported video sequence"));
    }
    let first_video = *video_pts
        .first()
        .ok_or_else(|| invalid("missing video PTS"))?;
    let last_video = *video_pts.last().expect("nonempty video PTS");
    let first_audio = *audio_pts
        .first()
        .ok_or_else(|| invalid("missing audio PTS"))?;
    let last_audio = *audio_pts.last().expect("nonempty audio PTS");
    if first_audio != first_video
        || video_pts
            .iter()
            .zip(video_pts.iter().skip(1))
            .any(|(a, b)| b - a != 3600)
    {
        return Err(invalid(
            "unsupported stream start offset or variable video timestamps",
        ));
    }
    if audio.len() < 40 || &audio[..4] != b"SShd" || &audio[32..36] != b"SSbd" {
        return Err(invalid("missing Sony ADS audio headers"));
    }
    let word = |offset: usize| {
        u32::from_le_bytes(
            audio[offset..offset + 4]
                .try_into()
                .expect("bounded header"),
        )
    };
    if (word(4), word(8), word(12), word(16), word(20)) != (24, 1, 48000, 2, 512) {
        return Err(invalid("unsupported ADS PCM format"));
    }
    let sample_bytes = usize::try_from(word(36)).map_err(|_| invalid("audio size overflow"))?;
    if sample_bytes != audio.len() - 40 || !sample_bytes.is_multiple_of(1024) {
        return Err(invalid("ADS data size or stereo block alignment disagrees"));
    }
    let metadata = OpeningMovieMetadata {
        source_sha256: String::new(),
        width: 640,
        height: 448,
        frame_rate_num: 25,
        frame_rate_den: 1,
        video_frames: video_pts.len() as u32,
        audio_sample_rate: 48000,
        audio_channels: 2,
        audio_samples_per_channel: (sample_bytes / 4) as u64,
        first_video_pts_90khz: first_video,
        first_audio_pts_90khz: first_audio,
        last_video_pts_90khz: last_video,
        last_audio_pts_90khz: last_audio,
        audio_ss2_sha256: format!("{:x}", Sha256::digest(&audio)),
    };
    Ok(OpeningMovie {
        audio_ss2: audio,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pes(stream: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0, 0, 1, stream];
        bytes.extend_from_slice(&u16::try_from(payload.len() + 8).unwrap().to_be_bytes());
        bytes.extend_from_slice(&[0x80, 0x80, 5, 0x21, 0, 1, 0, 1]);
        bytes.extend_from_slice(payload);
        bytes
    }
    fn audio() -> Vec<u8> {
        let mut bytes = b"SShd".to_vec();
        for value in [24u32, 1, 48000, 2, 512, u32::MAX, u32::MAX] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(b"SSbd");
        bytes.extend_from_slice(&1024u32.to_le_bytes());
        bytes.extend((0..1024).map(|n| (n % 251) as u8));
        bytes
    }
    fn program(audio: &[u8]) -> Vec<u8> {
        let mut bytes = pes(0xE0, &[0, 0, 1, 0xB3, 0x28, 0x01, 0xC0, 0x13]);
        for body in audio.chunks(73) {
            let mut payload = vec![0xFF, 0xA0, 0, 0];
            payload.extend_from_slice(body);
            bytes.extend_from_slice(&pes(0xBD, &payload));
        }
        bytes.extend_from_slice(&[0, 0, 1, 0xB9]);
        bytes
    }
    #[test]
    fn split_private_packets_reassemble_exact_audio_without_substream_tags() {
        let expected = audio();
        let source = program(&expected);
        let actual = parse(&source).unwrap();
        assert_eq!(actual.audio_ss2, expected);
        assert_eq!(actual.metadata.audio_samples_per_channel, 256);
        assert_eq!(actual.metadata.video_frames, 1);
        assert!(demux_pal_opening_movie(&source).is_err());
    }
    #[test]
    fn truncated_packets_missing_end_and_audio_size_mismatch_are_rejected() {
        let source = program(&audio());
        for length in [0, 1, 3, 5, 10, source.len() - 1] {
            assert!(parse(&source[..length]).is_err());
        }
        let mut changed = source.clone();
        changed[4] = 0xFF;
        changed[5] = 0xFF;
        assert!(parse(&changed).is_err());
        let mut bad_audio = audio();
        bad_audio[36..40].copy_from_slice(&2048u32.to_le_bytes());
        assert!(parse(&program(&bad_audio)).is_err());
        let mut trailing = source;
        trailing.push(0);
        assert!(parse(&trailing).is_err());
    }
    #[test]
    fn unsupported_codecs_substreams_and_timestamp_markers_are_rejected() {
        let mut bad_audio = audio();
        bad_audio[8] = 16;
        assert!(parse(&program(&bad_audio)).is_err());
        let mut source = program(&audio());
        source[9] &= !1;
        assert!(parse(&source).is_err());
        let mut source = program(&audio());
        let at = source
            .windows(4)
            .position(|v| v == [0xFF, 0xA0, 0, 0])
            .unwrap();
        source[at + 1] = 0xA1;
        assert!(parse(&source).is_err());
    }
    #[test]
    #[ignore = "requires private original PSS via ROSES_OPENING_PSS"]
    fn private_movie_matches_independently_demuxed_audio() {
        let path = std::env::var_os("ROSES_OPENING_PSS").expect("set ROSES_OPENING_PSS");
        let result = demux_pal_opening_movie(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            result.metadata.audio_ss2_sha256,
            "05fc902d9a6d792ada92ab79f1ccbb58027f90f00cd0604a6767ebfaf97a180c"
        );
        assert_eq!(result.metadata.video_frames, 2867);
        assert_eq!(result.metadata.audio_samples_per_channel, 5_504_768);
    }
}
