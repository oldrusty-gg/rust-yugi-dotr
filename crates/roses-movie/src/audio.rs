//! Sony "SShd/SSbd" audio as carried in PSS private stream 1: a 40-byte
//! header followed by channel-interleaved blocks of either 16-bit
//! little-endian PCM (codec 1) or PS-ADPCM (codec 0x10, the SPU2 "VAG"
//! format). Output is interleaved signed 16-bit PCM.
use crate::{invalid, unsupported, Result};

pub const HEADER_BYTES: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Pcm16Le,
    PsAdpcm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioHeader {
    pub codec: Codec,
    pub sample_rate: u32,
    pub channels: u32,
    /// Bytes of one channel before the next channel's block starts.
    pub interleave: u32,
    /// Size of the sample data after the header, as declared.
    pub data_bytes: u32,
}

impl AudioHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES || &bytes[..4] != b"SShd" || &bytes[32..36] != b"SSbd" {
            return invalid("missing SShd/SSbd audio header");
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        if word(4) != 24 {
            return unsupported(format!("SShd header size {}", word(4)));
        }
        let codec = match word(8) {
            0x01 => Codec::Pcm16Le,
            0x10 => Codec::PsAdpcm,
            other => return unsupported(format!("SShd codec {other:#x}")),
        };
        let header = Self {
            codec,
            sample_rate: word(12),
            channels: word(16),
            interleave: word(20),
            data_bytes: word(36),
        };
        let unit = match codec {
            Codec::Pcm16Le => 2,
            Codec::PsAdpcm => 16,
        };
        if !(1..=8).contains(&header.channels)
            || header.interleave == 0
            || header.interleave % unit != 0
            || header.interleave > 1 << 20
            || !(1000..=192_000).contains(&header.sample_rate)
        {
            return invalid(format!("implausible SShd parameters {header:?}"));
        }
        Ok(header)
    }

    pub fn block_bytes(&self) -> usize {
        (self.interleave * self.channels) as usize
    }

    pub fn samples_per_channel_per_block(&self) -> usize {
        match self.codec {
            Codec::Pcm16Le => self.interleave as usize / 2,
            Codec::PsAdpcm => self.interleave as usize / 16 * 28,
        }
    }
}

/// PS-ADPCM prediction filters, in 1/64 units.
const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

#[derive(Clone, Copy, Default)]
struct AdpcmState {
    s1: i32,
    s2: i32,
}

impl AdpcmState {
    /// One 16-byte frame to 28 samples.
    fn frame(&mut self, frame: &[u8], out: &mut impl FnMut(i16)) {
        let shift = u32::from(frame[0] & 0x0F);
        let filter = usize::from(frame[0] >> 4).min(4);
        let (f0, f1) = FILTERS[filter];
        for i in 0..28 {
            let byte = frame[2 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
            // Sign-extend the nibble into the top of 16 bits, then shift.
            let raw = i32::from(((u16::from(nibble) << 12) as i16) >> shift.min(12));
            let sample = (raw + ((self.s1 * f0 + self.s2 * f1 + 32) >> 6)).clamp(-32768, 32767);
            self.s2 = self.s1;
            self.s1 = sample;
            out(sample as i16);
        }
    }
}

/// Streaming SShd/SSbd decoder: push the private-stream payload bytes in
/// order, collect interleaved PCM.
pub struct AudioDecoder {
    pending: Vec<u8>,
    header: Option<AudioHeader>,
    states: Vec<AdpcmState>,
    consumed_data: u64,
}

impl Default for AudioDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDecoder {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            header: None,
            states: Vec::new(),
            consumed_data: 0,
        }
    }

    pub fn header(&self) -> Option<AudioHeader> {
        self.header
    }

    /// Append bytes and decode every complete interleave block into `out`.
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<i16>) -> Result<()> {
        self.pending.extend_from_slice(bytes);
        if self.header.is_none() {
            if self.pending.len() < HEADER_BYTES {
                return Ok(());
            }
            let header = AudioHeader::parse(&self.pending)?;
            self.pending.drain(..HEADER_BYTES);
            self.states = vec![AdpcmState::default(); header.channels as usize];
            self.header = Some(header);
        }
        let header = self.header.unwrap();
        let block = header.block_bytes();
        let mut at = 0;
        while self.pending.len() - at >= block {
            if header.data_bytes != 0 && self.consumed_data >= u64::from(header.data_bytes) {
                break; // trailing padding after the declared data
            }
            self.block(&header, at, out);
            at += block;
            self.consumed_data += block as u64;
        }
        self.pending.drain(..at);
        Ok(())
    }

    fn block(&mut self, header: &AudioHeader, at: usize, out: &mut Vec<i16>) {
        let channels = header.channels as usize;
        let interleave = header.interleave as usize;
        let per = header.samples_per_channel_per_block();
        let base = out.len();
        out.resize(base + per * channels, 0);
        for ch in 0..channels {
            let src = &self.pending[at + ch * interleave..at + (ch + 1) * interleave];
            match header.codec {
                Codec::Pcm16Le => {
                    for (i, pair) in src.chunks_exact(2).enumerate() {
                        out[base + i * channels + ch] = i16::from_le_bytes([pair[0], pair[1]]);
                    }
                }
                Codec::PsAdpcm => {
                    let mut i = 0;
                    let state = &mut self.states[ch];
                    for frame in src.chunks_exact(16) {
                        state.frame(frame, &mut |s| {
                            out[base + i * channels + ch] = s;
                            i += 1;
                        });
                    }
                }
            }
        }
    }

    /// Bytes left over that do not fill a whole block (normally zero).
    pub fn leftover(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn header(codec: u32, rate: u32, channels: u32, interleave: u32, data: u32) -> Vec<u8> {
        let mut b = b"SShd".to_vec();
        for v in [24u32, codec, rate, channels, interleave, u32::MAX, u32::MAX] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(b"SSbd");
        b.extend_from_slice(&data.to_le_bytes());
        b
    }

    #[test]
    fn pcm_blocks_deinterleave_in_any_push_sizes() {
        let mut s = header(1, 48000, 2, 8, 32);
        let left: Vec<i16> = (0..8).map(|n| n * 100 - 300).collect();
        let right: Vec<i16> = (0..8).map(|n| -n * 7).collect();
        for half in 0..2 {
            for v in &left[half * 4..half * 4 + 4] {
                s.extend_from_slice(&v.to_le_bytes());
            }
            for v in &right[half * 4..half * 4 + 4] {
                s.extend_from_slice(&v.to_le_bytes());
            }
        }
        for chunk in [1, 3, 7, 40, 1000] {
            let mut d = AudioDecoder::new();
            let mut out = Vec::new();
            for c in s.chunks(chunk) {
                d.push(c, &mut out).unwrap();
            }
            let expect: Vec<i16> = left.iter().zip(&right).flat_map(|(l, r)| [*l, *r]).collect();
            assert_eq!(out, expect, "chunk {chunk}");
            assert_eq!(d.leftover(), 0);
        }
    }

    #[test]
    fn adpcm_filters_and_shifts() {
        // Filter 0, shift 0: nibbles are samples << 12.
        let mut frame = [0u8; 16];
        frame[0] = 0x00;
        frame[2] = 0x71; // samples 1, 7
        frame[3] = 0x8F; // samples -1, -8
        let mut st = AdpcmState::default();
        let mut got = Vec::new();
        st.frame(&frame, &mut |s| got.push(s));
        assert_eq!(&got[..4], &[4096, 28672, -4096, -32768]);
        // Filter 1 (60/64) with shift 12 and zero nibbles decays the history.
        let mut st = AdpcmState { s1: 6400, s2: 0 };
        let mut got = Vec::new();
        let mut f = [0u8; 16];
        f[0] = 0x1C;
        st.frame(&f, &mut |s| got.push(s));
        assert_eq!(got[0], ((6400 * 60 + 32) >> 6) as i16);
        assert_eq!(got[1], ((i32::from(got[0]) * 60 + 32) >> 6) as i16);
    }

    #[test]
    fn rejects_bad_headers() {
        assert!(AudioHeader::parse(&header(2, 48000, 2, 512, 0)).is_err());
        assert!(AudioHeader::parse(&header(1, 48000, 0, 512, 0)).is_err());
        assert!(AudioHeader::parse(&header(0x10, 48000, 2, 100, 0)).is_err());
        let ok = AudioHeader::parse(&header(0x10, 48000, 2, 512, 0)).unwrap();
        assert_eq!(ok.samples_per_channel_per_block(), 896);
    }
}
