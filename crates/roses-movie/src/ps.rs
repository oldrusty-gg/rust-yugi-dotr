//! MPEG program stream (ISO/IEC 13818-1) demultiplexing as used by PS2 PSS
//! files. Streaming: packets are read one at a time from any `Read`.
use crate::{invalid, unsupported, Result};
use std::io::{ErrorKind, Read};

pub const STREAM_PRIVATE_1: u8 = 0xBD;
pub const STREAM_VIDEO_0: u8 = 0xE0;

/// One PES packet's payload.
#[derive(Clone, Debug)]
pub struct Packet {
    /// Byte offset of the packet's start code in the program stream.
    pub offset: u64,
    pub stream_id: u8,
    /// For private stream 1 in a PSS: the Sony substream byte (0xA0 + n for
    /// audio stream n). The four-byte tag carrying it is removed from
    /// `payload`.
    pub substream: Option<u8>,
    /// 90 kHz presentation timestamp, when the packet carries one.
    pub pts: Option<u64>,
    pub dts: Option<u64>,
    pub payload: Vec<u8>,
}

pub struct PsReader<R> {
    reader: R,
    offset: u64,
    end_code: bool,
    done: bool,
}

fn timestamp(bytes: &[u8]) -> Result<u64> {
    if bytes.len() < 5 || bytes[0] & 1 == 0 || bytes[2] & 1 == 0 || bytes[4] & 1 == 0 {
        return invalid("PES timestamp marker bits");
    }
    Ok(u64::from((bytes[0] >> 1) & 7) << 30
        | u64::from(bytes[1]) << 22
        | u64::from(bytes[2] >> 1) << 15
        | u64::from(bytes[3]) << 7
        | u64::from(bytes[4] >> 1))
}

impl<R: Read> PsReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            offset: 0,
            end_code: false,
            done: false,
        }
    }

    /// True once the program end code (0x000001B9) has been read.
    pub fn saw_end_code(&self) -> bool {
        self.end_code
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    fn exact(&mut self, buf: &mut [u8]) -> Result<()> {
        match self.reader.read_exact(buf) {
            Ok(()) => {
                self.offset += buf.len() as u64;
                Ok(())
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => invalid(format!(
                "program stream truncated near byte {}",
                self.offset
            )),
            Err(e) => Err(e.into()),
        }
    }

    /// Read the first byte of a start code; `None` at a clean end of input.
    fn first_byte(&mut self) -> Result<Option<u8>> {
        let mut one = [0u8];
        loop {
            return match self.reader.read(&mut one) {
                Ok(0) => Ok(None),
                Ok(_) => {
                    self.offset += 1;
                    Ok(Some(one[0]))
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => Err(e.into()),
            };
        }
    }

    /// The next PES packet carrying elementary stream data, skipping pack
    /// headers, system headers and padding. `None` after the end code or at
    /// the end of input.
    pub fn next_packet(&mut self) -> Result<Option<Packet>> {
        while !self.done {
            let start = self.offset;
            let Some(first) = self.first_byte()? else {
                self.done = true;
                return Ok(None);
            };
            let mut rest = [0u8; 3];
            rest[0] = first;
            self.exact(&mut rest[1..])?;
            let mut code = [0u8; 1];
            self.exact(&mut code)?;
            if rest != [0, 0, 1] {
                return invalid(format!("no start code at byte {start}"));
            }
            let id = code[0];
            match id {
                0xB9 => {
                    self.end_code = true;
                    self.done = true;
                    return Ok(None);
                }
                0xBA => {
                    let mut first = [0u8; 1];
                    self.exact(&mut first)?;
                    if first[0] & 0xC0 == 0x40 {
                        let mut pack = [0u8; 9];
                        self.exact(&mut pack)?;
                        let mut stuffing = vec![0u8; usize::from(pack[8] & 7)];
                        self.exact(&mut stuffing)?;
                    } else if first[0] & 0xF0 == 0x20 {
                        let mut pack = [0u8; 7];
                        self.exact(&mut pack)?;
                    } else {
                        return invalid(format!("unknown pack header at byte {start}"));
                    }
                }
                0xBB..=0xFF => {
                    let mut len = [0u8; 2];
                    self.exact(&mut len)?;
                    let mut body = vec![0u8; usize::from(u16::from_be_bytes(len))];
                    self.exact(&mut body)?;
                    let pes = matches!(id, 0xBD | 0xC0..=0xEF);
                    if !pes {
                        continue; // system header, padding, private 2, ...
                    }
                    return self.pes(start, id, body).map(Some);
                }
                _ => return invalid(format!("unexpected start code {id:02X} at byte {start}")),
            }
        }
        Ok(None)
    }

    fn pes(&self, offset: u64, stream_id: u8, body: Vec<u8>) -> Result<Packet> {
        if body.len() < 3 || body[0] & 0xC0 != 0x80 {
            return unsupported(format!("non-MPEG-2 PES header at byte {offset}"));
        }
        let flags = body[1] >> 6;
        let header_end = 3 + usize::from(body[2]);
        if header_end > body.len() {
            return invalid(format!("PES header exceeds packet at byte {offset}"));
        }
        let (pts, dts) = match flags {
            0b10 => (Some(timestamp(&body[3..header_end])?), None),
            0b11 => (
                Some(timestamp(&body[3..header_end])?),
                Some(timestamp(body.get(8..header_end).unwrap_or(&[]))?),
            ),
            0b00 => (None, None),
            _ => return invalid(format!("forbidden PTS_DTS_flags at byte {offset}")),
        };
        let mut payload = body;
        payload.drain(..header_end);
        let mut substream = None;
        if stream_id == STREAM_PRIVATE_1 {
            if payload.len() >= 4 && payload[0] == 0xFF {
                substream = Some(payload[1]);
                payload.drain(..4);
            } else if let Some(&first) = payload.first() {
                substream = Some(first);
            }
        }
        Ok(Packet {
            offset,
            stream_id,
            substream,
            pts,
            dts,
            payload,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic MPEG-2 PES packet with a PTS.
    pub fn pes(stream: u8, pts: Option<u64>, payload: &[u8]) -> Vec<u8> {
        let mut header = Vec::new();
        if let Some(t) = pts {
            header.extend_from_slice(&[
                0x21 | ((t >> 29) as u8 & 0x0E),
                (t >> 22) as u8,
                ((t >> 14) as u8 & 0xFE) | 1,
                (t >> 7) as u8,
                ((t << 1) as u8) | 1,
            ]);
        }
        let mut bytes = vec![0, 0, 1, stream];
        let len = 3 + header.len() + payload.len();
        bytes.extend_from_slice(&u16::try_from(len).unwrap().to_be_bytes());
        bytes.extend_from_slice(&[0x81, if pts.is_some() { 0x80 } else { 0 }]);
        bytes.push(header.len() as u8);
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(payload);
        bytes
    }

    pub fn pack() -> Vec<u8> {
        vec![0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 1, 0x89, 0xC3, 0xF8]
    }

    #[test]
    fn packets_timestamps_and_substreams() {
        let mut s = pack();
        s.extend(pes(0xE0, Some(0x1_2345_6789 & 0x1_FFFF_FFFF), b"video"));
        s.extend([0, 0, 1, 0xBE, 0, 3, 0xFF, 0xFF, 0xFF]);
        s.extend(pes(0xBD, None, &[0xFF, 0xA0, 0, 0, 9, 8, 7]));
        s.extend([0, 0, 1, 0xB9]);
        let mut r = PsReader::new(&s[..]);
        let v = r.next_packet().unwrap().unwrap();
        assert_eq!((v.stream_id, v.pts, &v.payload[..]), (0xE0, Some(0x1_2345_6789), &b"video"[..]));
        let a = r.next_packet().unwrap().unwrap();
        assert_eq!((a.substream, &a.payload[..]), (Some(0xA0), &[9u8, 8, 7][..]));
        assert!(r.next_packet().unwrap().is_none());
        assert!(r.saw_end_code());
    }

    #[test]
    fn truncation_and_garbage_are_errors() {
        let mut s = pack();
        s.extend(pes(0xE0, Some(7), b"abcdef"));
        for cut in [1, 5, 13, 16, 20, s.len() - 1] {
            let mut r = PsReader::new(&s[..cut]);
            let mut failed = false;
            for _ in 0..4 {
                match r.next_packet() {
                    Err(_) => {
                        failed = true;
                        break;
                    }
                    Ok(None) => break,
                    Ok(Some(_)) => {}
                }
            }
            assert!(failed, "cut {cut}");
        }
        let mut r = PsReader::new(&[1u8, 2, 3, 4][..]);
        assert!(r.next_packet().is_err());
    }
}
