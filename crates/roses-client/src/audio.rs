//! Bounded RIFF/WAVE PCM16 parsing for explicit intro-plus-loop playback.
//! This does not infer loop points from audio or interpret optional `smpl` data.
use crate::Result;
use std::ops::Range;

pub const MAX_PCM_WAV_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug)]
pub struct Pcm16Wave {
    channels: u16,
    sample_rate: u32,
    data: Box<[u8]>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct LoopRegions {
    /// Played once; empty when the loop begins at frame zero.
    pub intro: Range<usize>,
    /// Played immediately after the intro, then repeatedly.
    pub repeated: Range<usize>,
}
impl Pcm16Wave {
    pub fn parse(wav: &[u8]) -> Result<Self> {
        if !(12..=MAX_PCM_WAV_BYTES).contains(&wav.len())
            || wav.get(..4) != Some(b"RIFF")
            || wav.get(8..12) != Some(b"WAVE")
        {
            return Err("Loop audio requires a bounded RIFF/WAVE file.".into());
        }
        if u32_at(wav, 4) as u64 + 8 != wav.len() as u64 {
            return Err("RIFF size does not match the WAV file.".into());
        }
        let mut cursor = 12usize;
        let mut chunks = 0usize;
        let mut format = None;
        let mut data = None;
        while cursor < wav.len() {
            chunks += 1;
            if chunks > 1024 || wav.len() - cursor < 8 {
                return Err("WAV chunk count or header is invalid.".into());
            }
            let size = u32_at(wav, cursor + 4) as usize;
            let start = cursor + 8;
            let end = start
                .checked_add(size)
                .filter(|end| *end <= wav.len())
                .ok_or("WAV chunk exceeds the RIFF boundary.")?;
            match &wav[cursor..cursor + 4] {
                b"fmt " => {
                    if format.is_some() || !matches!(size, 16 | 18) {
                        return Err("WAV requires one PCM format chunk of 16 or 18 bytes.".into());
                    }
                    let f = &wav[start..end];
                    let channels = u16_at(f, 2);
                    let rate = u32_at(f, 4);
                    if u16_at(f, 0) != 1
                        || !(1..=2).contains(&channels)
                        || !(8000..=192000).contains(&rate)
                        || u16_at(f, 14) != 16
                        || u16_at(f, 12) != channels * 2
                        || u32_at(f, 8) != rate * u32::from(channels) * 2
                        || (size == 18 && u16_at(f, 16) != 0)
                    {
                        return Err("Loop audio requires PCM16 mono/stereo, 8..192 kHz, and consistent byte/block rates.".into());
                    }
                    format = Some((channels, rate));
                }
                b"data" => {
                    if data.is_some() || size == 0 {
                        return Err("WAV requires one nonempty data chunk.".into());
                    }
                    data = Some(start..end);
                }
                _ => {}
            }
            cursor = end
                .checked_add(size & 1)
                .filter(|next| *next <= wav.len())
                .ok_or("WAV is missing an odd-sized chunk's padding byte.")?;
        }
        let (channels, sample_rate) = format.ok_or("WAV has no PCM format chunk.")?;
        let data = data.ok_or("WAV has no data chunk.")?;
        if !data.len().is_multiple_of(usize::from(channels) * 2) {
            return Err("PCM data ends within a sample frame.".into());
        }
        Ok(Self {
            channels,
            sample_rate,
            data: wav[data].into(),
        })
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn block_align(&self) -> u16 {
        self.channels * 2
    }
    pub fn frame_count(&self) -> u32 {
        // The parser's byte cap is smaller than u32::MAX even for mono PCM.
        (self.data.len() / usize::from(self.block_align())) as u32
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn loop_regions(&self, start_frame: u32) -> Result<LoopRegions> {
        if start_frame >= self.frame_count() {
            return Err("Loop start must precede the WAV's final sample frame.".into());
        }
        let split = start_frame as usize * usize::from(self.block_align());
        Ok(LoopRegions {
            intro: 0..split,
            repeated: split..self.data.len(),
        })
    }
}
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(tag: &[u8; 4], contents: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend_from_slice(&(contents.len() as u32).to_le_bytes());
        out.extend_from_slice(contents);
        if !contents.len().is_multiple_of(2) {
            out.push(0);
        }
        out
    }
    fn fmt() -> Vec<u8> {
        [
            1u16.to_le_bytes().as_slice(),
            &2u16.to_le_bytes(),
            &24000u32.to_le_bytes(),
            &96000u32.to_le_bytes(),
            &4u16.to_le_bytes(),
            &16u16.to_le_bytes(),
        ]
        .concat()
    }
    fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
        let body = chunks.concat();
        [
            b"RIFF".as_slice(),
            &((body.len() + 4) as u32).to_le_bytes(),
            b"WAVE",
            &body,
        ]
        .concat()
    }
    fn valid() -> Vec<u8> {
        // Four distinguishable stereo frames, including signed sample extremes.
        riff(&[
            chunk(b"fmt ", &fmt()),
            chunk(
                b"data",
                &[0, 128, 255, 127, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0],
            ),
        ])
    }
    #[test]
    fn explicit_loop_preserves_frames_without_resampling_or_repeating_intro() {
        let wave = Pcm16Wave::parse(&valid()).unwrap();
        assert_eq!(
            (wave.channels(), wave.sample_rate(), wave.frame_count()),
            (2, 24000, 4)
        );
        let regions = wave.loop_regions(2).unwrap();
        assert_eq!(
            regions,
            LoopRegions {
                intro: 0..8,
                repeated: 8..16
            }
        );
        let sequence = [
            &wave.data()[regions.intro],
            &wave.data()[regions.repeated.clone()],
            &wave.data()[regions.repeated],
        ]
        .concat();
        assert_eq!(
            sequence,
            [0, 128, 255, 127, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 3, 0, 4, 0, 5, 0, 6, 0]
        );
        assert_eq!(wave.loop_regions(0).unwrap().intro, 0..0);
        assert!(wave.loop_regions(4).is_err());
        assert!(wave.loop_regions(u32::MAX).is_err());
    }
    #[test]
    fn ignores_bounded_padded_ancillary_chunks_and_handles_data_before_format() {
        let wav = riff(&[
            chunk(b"JUNK", &[9, 8, 7]),
            chunk(b"data", &[1, 2, 3, 4]),
            chunk(b"fmt ", &fmt()),
        ]);
        let wave = Pcm16Wave::parse(&wav).unwrap();
        assert_eq!(wave.data(), [1, 2, 3, 4]);
        assert_eq!(wave.frame_count(), 1);
        let mut extended = fmt();
        extended.extend_from_slice(&[0, 0]);
        assert!(
            Pcm16Wave::parse(&riff(&[chunk(b"fmt ", &extended), chunk(b"data", &[0; 4])])).is_ok()
        );
    }
    #[test]
    fn rejects_every_truncated_prefix_and_declared_size_mismatch() {
        let wav = valid();
        for end in 0..wav.len() {
            assert!(Pcm16Wave::parse(&wav[..end]).is_err(), "prefix {end}");
        }
        let mut extra = wav.clone();
        extra.push(0);
        assert!(Pcm16Wave::parse(&extra).is_err());
        let mut huge_chunk = wav;
        huge_chunk[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Pcm16Wave::parse(&huge_chunk).is_err());
        assert!(Pcm16Wave::parse(&riff(&[b"JUNK\x01\0\0\0X".to_vec()])).is_err());
    }
    #[test]
    fn rejects_inconsistent_or_unsupported_formats_and_partial_frames() {
        for (at, bytes) in [
            (0, 3u16.to_le_bytes().to_vec()),
            (2, 0u16.to_le_bytes().to_vec()),
            (2, 3u16.to_le_bytes().to_vec()),
            (4, 0u32.to_le_bytes().to_vec()),
            (4, 192001u32.to_le_bytes().to_vec()),
            (8, 42u32.to_le_bytes().to_vec()),
            (12, 2u16.to_le_bytes().to_vec()),
            (14, 8u16.to_le_bytes().to_vec()),
        ] {
            let mut f = fmt();
            f[at..at + bytes.len()].copy_from_slice(&bytes);
            assert!(
                Pcm16Wave::parse(&riff(&[chunk(b"fmt ", &f), chunk(b"data", &[0; 4])])).is_err()
            );
        }
        for size in [0, 1, 2, 3, 5, 6, 7] {
            assert!(Pcm16Wave::parse(&riff(&[
                chunk(b"fmt ", &fmt()),
                chunk(b"data", &vec![0; size])
            ]))
            .is_err());
        }
    }
    #[test]
    fn rejects_duplicate_missing_and_excessive_chunks() {
        let f = chunk(b"fmt ", &fmt());
        let d = chunk(b"data", &[0; 4]);
        for chunks in [
            vec![f.clone()],
            vec![d.clone()],
            vec![f.clone(), f.clone(), d.clone()],
            vec![f.clone(), d.clone(), d.clone()],
        ] {
            assert!(Pcm16Wave::parse(&riff(&chunks)).is_err());
        }
        let mut chunks = vec![chunk(b"JUNK", &[]); 1023];
        chunks.extend([f, d]);
        assert!(Pcm16Wave::parse(&riff(&chunks)).is_err());
    }
}
