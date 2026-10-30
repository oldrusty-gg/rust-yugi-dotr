//! Narrow PAL opening-story music import. Format and track identity were
//! checked against the original conversion routine, EE/IOP buffers and current
//! SPU input bytes in a preserved scene. No original audio is embedded here.

use crate::{Error, Result, PROFILE_ID};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const PAL_STORY_MUSIC_ISO_PATH: &str = "SOUND/BGM/02/0024.PCM";
pub const PAL_STORY_MUSIC_SHA256: &str =
    "9513b87e505ad05c48f92f96813767b971930980e8230d6b2191022b2aabf5ed";
pub const PAL_SIMON_MUSIC_ISO_PATH: &str = "SOUND/BGM/02/0025.PCM";
pub const PAL_SIMON_MUSIC_SHA256: &str =
    "a8afd67f7eca68515de3e0df37b0edc1ff6b4b9557a52991e9bac5b4f2b79552";
pub const PAL_PCMLIST_SHA256: &str =
    "8c2d4bcd06f5b74a6095bbf8632dcb7f5957ca936a42bd7d6a6c3c57565dd18a";
const TRACK_BYTES: usize = 9_418_752;
const LIST_BYTES: usize = 4096;
const DATA_OFFSET: usize = 2048;
const SAMPLE_RATE: u32 = 24_000;
const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const FRAME_BYTES: u32 = 4;

#[derive(Clone, Debug, Serialize)]
pub struct StoryMusicMetadata {
    pub schema_version: u32,
    pub profile_id: String,
    pub track_iso_path: String,
    pub track_sha256: String,
    pub pcmlist_sha256: String,
    pub source_data_offset: usize,
    pub data_bytes: u32,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub frame_count: u32,
    pub loop_start_frame: u32,
    pub loop_end_frame_exclusive: u32,
    /// Import validates the source format, not the native playback/mixer.
    pub playback_validated: bool,
}

#[derive(Clone, Debug)]
pub struct StoryMusic {
    pub metadata: StoryMusicMetadata,
    /// Standard 44-byte RIFF/WAVE PCM header followed by the unchanged original
    /// interleaved samples. There is no smpl chunk: use metadata for looping.
    pub wav: Vec<u8>,
}

impl StoryMusic {
    /// Return a 48kHz stereo WAV using the original CPU's exact 2x frame
    /// repetition: [left,right] becomes [left,right,left,right]. The source
    /// `wav` and metadata remain unchanged at 24kHz. Playback must double the
    /// metadata frame count and both loop indices for this returned file.
    /// This preserves the original upsampling step, not the full SPU mixer.
    pub fn original_rate_wav(&self) -> Result<Vec<u8>> {
        repeat_original_wave(&self.wav)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Header {
    data_bytes: u32,
    frames: u32,
    loop_start: u32,
    loop_end: u32,
}

fn invalid(message: &str) -> Error {
    Error::Invalid(format!("PAL story music: {message}"))
}

/// Import the identified historical-scene track from the supported PAL ISO.
/// Both the music file and `SOUND/BGM/PCMLIST.BIN` must match exact known hashes.
/// The 24kHz source rate follows the original routine's 2x sample repetition
/// before 48kHz SPU input, verified against saved converted buffers. A native
/// renderer can instead repeat each source frame twice at 48kHz to preserve
/// that original upsampling behavior; resampling filters are not validated.
pub fn decode_pal_story_music(track: &[u8], pcmlist: &[u8]) -> Result<StoryMusic> {
    decode_track(
        track,
        pcmlist,
        24,
        TRACK_BYTES,
        PAL_STORY_MUSIC_ISO_PATH,
        PAL_STORY_MUSIC_SHA256,
        Header {
            data_bytes: 9_415_744,
            frames: 2_353_936,
            loop_start: 413_728,
            loop_end: 2_353_936,
        },
    )
}

/// Event10040 starts track25 with the original music routine at0x21E394.
/// This uses the same verified PCM conversion as the preceding history track.
pub fn decode_pal_simon_music(track: &[u8], pcmlist: &[u8]) -> Result<StoryMusic> {
    decode_track(
        track,
        pcmlist,
        25,
        7_929_856,
        PAL_SIMON_MUSIC_ISO_PATH,
        PAL_SIMON_MUSIC_SHA256,
        Header {
            data_bytes: 7_926_080,
            frames: 1_981_520,
            loop_start: 599_120,
            loop_end: 1_981_520,
        },
    )
}

fn decode_track(
    track: &[u8],
    pcmlist: &[u8],
    track_id: u32,
    track_bytes: usize,
    track_path: &str,
    expected_hash: &str,
    expected_header: Header,
) -> Result<StoryMusic> {
    // Bound work before hashing or allocating the output.
    if track.len() != track_bytes || pcmlist.len() != LIST_BYTES {
        return Err(Error::Unsupported("PAL story music source lengths".into()));
    }
    let track_sha256 = format!("{:x}", Sha256::digest(track));
    let pcmlist_sha256 = format!("{:x}", Sha256::digest(pcmlist));
    if track_sha256 != expected_hash || pcmlist_sha256 != PAL_PCMLIST_SHA256 {
        return Err(Error::Unsupported(format!(
            "PAL story music fingerprints: track {track_sha256}, PCMLIST {pcmlist_sha256}"
        )));
    }
    let header = parse_header(track)?;
    validate_list(pcmlist, header, track_id)?;
    if header != expected_header {
        return Err(invalid("source values differ from the supported profile"));
    }
    let payload = &track[DATA_OFFSET..DATA_OFFSET + header.data_bytes as usize];
    Ok(StoryMusic {
        metadata: StoryMusicMetadata {
            schema_version: 1,
            profile_id: PROFILE_ID.into(),
            track_iso_path: track_path.into(),
            track_sha256,
            pcmlist_sha256,
            source_data_offset: DATA_OFFSET,
            data_bytes: header.data_bytes,
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
            bits_per_sample: BITS_PER_SAMPLE,
            frame_count: header.frames,
            loop_start_frame: header.loop_start,
            loop_end_frame_exclusive: header.loop_end,
            playback_validated: false,
        },
        wav: wrap_pcm_wave(payload)?,
    })
}

fn parse_header(track: &[u8]) -> Result<Header> {
    if track.len() < DATA_OFFSET || track.len() > TRACK_BYTES || !track.len().is_multiple_of(2048) {
        return Err(invalid("track length is truncated, unaligned or oversized"));
    }
    let header = Header {
        data_bytes: word(track, 0)?,
        frames: word(track, 4)?,
        loop_start: word(track, 8)?,
        loop_end: word(track, 12)?,
    };
    if header.data_bytes == 0
        || !header.data_bytes.is_multiple_of(FRAME_BYTES)
        || header.frames != header.data_bytes / FRAME_BYTES
    {
        return Err(invalid("data length does not match stereo frame count"));
    }
    if header.loop_start >= header.loop_end || header.loop_end != header.frames {
        return Err(invalid("loop range is empty, reversed or outside the data"));
    }
    let end = DATA_OFFSET
        .checked_add(header.data_bytes as usize)
        .filter(|&end| end <= track.len())
        .ok_or_else(|| invalid("declared sample data exceeds the track"))?;
    if track.len() - end >= 2048
        || track[16..DATA_OFFSET].iter().any(|&v| v != 0)
        || track[end..].iter().any(|&v| v != 0)
    {
        return Err(invalid("unrecognized header fields or trailing padding"));
    }
    Ok(header)
}

fn validate_list(list: &[u8], header: Header, track_id: u32) -> Result<()> {
    if list.len() != LIST_BYTES {
        return Err(invalid("PCMLIST has an unsupported byte length"));
    }
    let mut found = false;
    let mut padding = false;
    for record in list.chunks_exact(16) {
        if record.iter().all(|&v| v == 0) {
            padding = true;
            continue;
        }
        if padding {
            return Err(invalid("PCMLIST has data after its zero padding"));
        }
        if word(record, 4)? != track_id {
            continue;
        }
        if found {
            return Err(invalid("PCMLIST repeats the requested track"));
        }
        found = true;
        if word(record, 0)? != header.data_bytes
            || word(record, 8)? != header.loop_start
            || word(record, 12)? != header.loop_end
        {
            return Err(invalid("PCMLIST and track disagree on data or loop bounds"));
        }
    }
    if !found {
        return Err(invalid("PCMLIST lacks the requested track"));
    }
    Ok(())
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| invalid("field offset overflow"))?;
    let bytes = bytes
        .get(offset..end)
        .ok_or_else(|| invalid("truncated field"))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("checked field length"),
    ))
}

fn wrap_pcm_wave(payload: &[u8]) -> Result<Vec<u8>> {
    if payload.is_empty()
        || !payload.len().is_multiple_of(FRAME_BYTES as usize)
        || payload.len() > TRACK_BYTES - DATA_OFFSET
    {
        return Err(invalid("WAV payload is empty, unaligned or oversized"));
    }
    let data_bytes = u32::try_from(payload.len()).map_err(|_| invalid("WAV size overflow"))?;
    let mut wav = Vec::with_capacity(44 + payload.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(data_bytes + 36).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&CHANNELS.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * FRAME_BYTES).to_le_bytes());
    wav.extend_from_slice(&(FRAME_BYTES as u16).to_le_bytes());
    wav.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    wav.extend_from_slice(payload);
    Ok(wav)
}

fn repeat_original_wave(source: &[u8]) -> Result<Vec<u8>> {
    // StoryMusic's fields are public, so validate the canonical source header
    // and bound allocation even if a caller has replaced its WAV bytes.
    if source.len() < 48 || source.len() > 44 + TRACK_BYTES - DATA_OFFSET {
        return Err(invalid("original-rate WAV source length is unsupported"));
    }
    let bytes = source.len() - 44;
    if !bytes.is_multiple_of(FRAME_BYTES as usize)
        || source[..4] != *b"RIFF"
        || source[8..16] != *b"WAVEfmt "
        || word(source, 4)? as usize != source.len() - 8
        || word(source, 16)? != 16
        || source[20..24] != [1, 0, 2, 0]
        || word(source, 24)? != SAMPLE_RATE
        || word(source, 28)? != SAMPLE_RATE * FRAME_BYTES
        || source[32..36] != [4, 0, 16, 0]
        || source[36..40] != *b"data"
        || word(source, 40)? as usize != bytes
    {
        return Err(invalid(
            "original-rate conversion requires canonical 24kHz stereo PCM",
        ));
    }
    let output_bytes = bytes
        .checked_mul(2)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| invalid("original-rate WAV output size overflow"))?;
    let mut output = Vec::with_capacity(44 + output_bytes as usize);
    output.extend_from_slice(&source[..44]);
    output[4..8].copy_from_slice(&(output_bytes + 36).to_le_bytes());
    output[24..28].copy_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    output[28..32].copy_from_slice(&(SAMPLE_RATE * 2 * FRAME_BYTES).to_le_bytes());
    output[40..44].copy_from_slice(&output_bytes.to_le_bytes());
    for frame in source[44..].chunks_exact(FRAME_BYTES as usize) {
        output.extend_from_slice(frame);
        output.extend_from_slice(frame);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn fixtures() -> (Vec<u8>, Vec<u8>) {
        let mut track = vec![0; 4096];
        for (offset, value) in [(0, 16), (4, 4), (8, 1), (12, 4)] {
            put(&mut track, offset, value);
        }
        // Invented signed stereo samples, including minimum/maximum lanes.
        for (index, sample) in [-32768i16, 32767, -1, 1, 2, -2, 500, -500]
            .into_iter()
            .enumerate()
        {
            track[DATA_OFFSET + index * 2..DATA_OFFSET + index * 2 + 2]
                .copy_from_slice(&sample.to_le_bytes());
        }
        let mut list = vec![0; LIST_BYTES];
        for (offset, value) in [(0, 16), (4, 24), (8, 1), (12, 4)] {
            put(&mut list, offset, value);
        }
        (track, list)
    }

    #[test]
    fn wav_keeps_signed_stereo_lanes_and_declares_correct_rate_and_lengths() {
        let (track, list) = fixtures();
        let h = parse_header(&track).unwrap();
        validate_list(&list, h, 24).unwrap();
        let payload = &track[DATA_OFFSET..DATA_OFFSET + 16];
        let wav = wrap_pcm_wave(payload).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(word(&wav, 4).unwrap(), 52);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(word(&wav, 24).unwrap(), 24000);
        assert_eq!(word(&wav, 28).unwrap(), 96000);
        assert_eq!(&wav[20..24], &[1, 0, 2, 0]);
        assert_eq!(&wav[32..36], &[4, 0, 16, 0]);
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(word(&wav, 40).unwrap(), 16);
        assert_eq!(&wav[44..], payload);
    }

    #[test]
    fn original_rate_repeats_frames_without_swapping_or_repeating_whole_channels() {
        let payload = [0, 128, 255, 127, 255, 255, 1, 0];
        let source = wrap_pcm_wave(&payload).unwrap();
        let repeated = repeat_original_wave(&source).unwrap();
        assert_eq!(word(&repeated, 24).unwrap(), 48_000);
        assert_eq!(word(&repeated, 28).unwrap(), 192_000);
        assert_eq!(word(&repeated, 4).unwrap(), 52);
        assert_eq!(word(&repeated, 40).unwrap(), 16);
        assert_eq!(
            &repeated[44..],
            &[0, 128, 255, 127, 0, 128, 255, 127, 255, 255, 1, 0, 255, 255, 1, 0,]
        );
        assert_eq!(&source[44..], &payload);
        // Both files represent exactly the same duration.
        assert_eq!(
            word(&source, 40).unwrap() * word(&repeated, 24).unwrap(),
            word(&repeated, 40).unwrap() * word(&source, 24).unwrap()
        );
        for (offset, value) in [
            (4, 0),
            (16, 18),
            (20, 1),
            (24, 48_000),
            (28, 192_000),
            (32, 0),
            (36, 0),
            (40, 4),
        ] {
            let mut bad = source.clone();
            put(&mut bad, offset, value);
            assert!(repeat_original_wave(&bad).is_err());
        }
        assert!(repeat_original_wave(&source[..43]).is_err());
        assert!(repeat_original_wave(&source[..source.len() - 1]).is_err());
    }

    #[test]
    fn malformed_sizes_and_loop_ranges_never_reach_sample_copy() {
        let (track, _) = fixtures();
        for (offset, value) in [
            (0, 0),
            (0, 15),
            (0, u32::MAX - 3),
            (4, 5),
            (8, 4),
            (8, 5),
            (12, 3),
            (12, 5),
        ] {
            let mut bad = track.clone();
            put(&mut bad, offset, value);
            assert!(parse_header(&bad).is_err());
        }
        assert!(parse_header(&track[..15]).is_err());
        assert!(parse_header(&track[..track.len() - 1]).is_err());
        let mut bad = track.clone();
        bad[16] = 1;
        assert!(parse_header(&bad).is_err());
        let mut bad = track;
        *bad.last_mut().unwrap() = 1;
        assert!(parse_header(&bad).is_err());
        assert!(wrap_pcm_wave(&[]).is_err());
        assert!(wrap_pcm_wave(&[0; 3]).is_err());
    }

    #[test]
    fn list_mismatch_duplicate_missing_and_nonpadding_tail_are_rejected() {
        let (track, list) = fixtures();
        let h = parse_header(&track).unwrap();
        for (offset, value) in [(0, 20), (4, 25), (8, 2), (12, 3)] {
            let mut bad = list.clone();
            put(&mut bad, offset, value);
            assert!(validate_list(&bad, h, 24).is_err());
        }
        assert!(validate_list(&list[..list.len() - 1], h, 24).is_err());
        let mut duplicate = list.clone();
        duplicate[16..32].copy_from_slice(&list[..16]);
        assert!(validate_list(&duplicate, h, 24).is_err());
        let mut gap = list.clone();
        gap[32..48].copy_from_slice(&list[..16]);
        assert!(validate_list(&gap, h, 24).is_err());
        assert!(validate_list(&vec![0; LIST_BYTES], h, 24).is_err());
    }

    #[test]
    fn exact_fingerprints_are_required_before_import() {
        assert!(decode_pal_story_music(&[], &[]).is_err());
        assert!(decode_pal_story_music(&vec![0; TRACK_BYTES], &vec![0; LIST_BYTES]).is_err());
        assert!(decode_pal_simon_music(&[], &[]).is_err());
        assert!(decode_pal_simon_music(&vec![0; 7_929_856], &vec![0; LIST_BYTES]).is_err());
    }

    #[test]
    #[ignore = "requires private Simon track and independent WAVs; set ROSES_TEST_SIMON_PCM_PATH, ROSES_TEST_PCMLIST_PATH, ROSES_TEST_SIMON_WAV_PATH and ROSES_TEST_SIMON_48K_WAV_PATH"]
    fn private_simon_music_matches_independent_source_and_upsampling() {
        let read = |key| std::fs::read(std::env::var_os(key).expect(key)).unwrap();
        let track = read("ROSES_TEST_SIMON_PCM_PATH");
        let list = read("ROSES_TEST_PCMLIST_PATH");
        let music = decode_pal_simon_music(&track, &list).unwrap();
        assert_eq!(music.wav, read("ROSES_TEST_SIMON_WAV_PATH"));
        assert_eq!(
            music.original_rate_wav().unwrap(),
            read("ROSES_TEST_SIMON_48K_WAV_PATH")
        );
        assert_eq!(music.metadata.frame_count, 1_981_520);
        assert_eq!(music.metadata.loop_start_frame, 599_120);
        assert!(decode_pal_story_music(&track, &list).is_err());
    }

    #[test]
    #[ignore = "requires private track, PCMLIST and independent WAV; set ROSES_TEST_STORY_PCM_PATH, ROSES_TEST_PCMLIST_PATH, ROSES_TEST_STORY_WAV_PATH"]
    fn private_story_wave_matches_independent_header_and_all_payload_bytes() {
        let read = |key| std::fs::read(std::env::var_os(key).expect(key)).unwrap();
        let track = read("ROSES_TEST_STORY_PCM_PATH");
        let list = read("ROSES_TEST_PCMLIST_PATH");
        let oracle = read("ROSES_TEST_STORY_WAV_PATH");
        let imported = decode_pal_story_music(&track, &list).unwrap();
        assert_eq!(imported.wav, oracle);
        assert_eq!(imported.metadata.frame_count, 2_353_936);
        assert_eq!(imported.metadata.loop_start_frame, 413_728);
        assert_eq!(imported.metadata.loop_end_frame_exclusive, 2_353_936);
        assert!(!imported.metadata.playback_validated);
    }

    #[test]
    #[ignore = "requires private track, PCMLIST and independent repeated-frame WAV; set ROSES_TEST_STORY_PCM_PATH, ROSES_TEST_PCMLIST_PATH, ROSES_TEST_STORY_48K_WAV_PATH"]
    fn private_original_rate_wave_matches_independent_repeated_sample_oracle() {
        let read = |key| std::fs::read(std::env::var_os(key).expect(key)).unwrap();
        let music = decode_pal_story_music(
            &read("ROSES_TEST_STORY_PCM_PATH"),
            &read("ROSES_TEST_PCMLIST_PATH"),
        )
        .unwrap();
        let source_hash = Sha256::digest(&music.wav);
        let repeated = music.original_rate_wav().unwrap();
        assert_eq!(repeated, read("ROSES_TEST_STORY_48K_WAV_PATH"));
        assert_eq!(word(&repeated, 40).unwrap() / FRAME_BYTES, 4_707_872);
        assert_eq!(music.metadata.loop_start_frame * 2, 827_456);
        assert_eq!(Sha256::digest(&music.wav), source_hash);
        assert_eq!(music.metadata.sample_rate, SAMPLE_RATE);
    }
}
