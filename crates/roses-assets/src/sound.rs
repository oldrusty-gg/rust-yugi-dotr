//! The game's music tracks and sound-effect banks, decoded natively
//! (`claude-audio/DUEL-AUDIO-SCOPE.md`) [F]:
//! - music: `SOUND/BGM/NN/XXXX.PCM`, a 2048-byte header {data bytes,
//!   frames, loop start, loop end} then interleaved s16le stereo at 24 kHz;
//!   `PCMLIST.BIN` holds 16-byte records {data bytes, track id, loop start,
//!   loop end}; loop start == loop end (0, 0) plays once;
//! - sound effects: KCE "GRP" banks: i32 count, i32 data bytes, 255 x 8-byte
//!   entries {volume, vtime, priority, voice group, u16 pitch (note << 8 |
//!   fine), u16 offset in 16-byte blocks}, the PS-ADPCM data from 2048;
//! - pitch: the driver's `MIDI_Note2Pitch` with its 12 x 128 table (from
//!   `IOP/KCEJEAST.IRX`), centre note 60 = 44.1 kHz.
use crate::{Error, Result};

fn invalid(message: &str) -> Error {
    Error::Invalid(format!("PAL sound: {message}"))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| invalid("truncated"))
}

/// A `PCMLIST.BIN` record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackRecord {
    pub data_bytes: u32,
    pub track: u32,
    pub loop_start: u32,
    pub loop_end: u32,
}

/// All records of `PCMLIST.BIN` (zero padding ends the list).
pub fn parse_pcmlist(bytes: &[u8]) -> Result<Vec<TrackRecord>> {
    let mut out = Vec::new();
    for at in (0..bytes.len().saturating_sub(15)).step_by(16) {
        let r = TrackRecord {
            data_bytes: u32_at(bytes, at)?,
            track: u32_at(bytes, at + 4)?,
            loop_start: u32_at(bytes, at + 8)?,
            loop_end: u32_at(bytes, at + 12)?,
        };
        if r.data_bytes == 0 && r.track == 0 {
            break;
        }
        out.push(r);
    }
    Ok(out)
}

/// The disc path of a track (`SOUND/BGM/<(id % 1000) / 10>/<id>.PCM`).
pub fn track_path(track: u32) -> String {
    format!("SOUND/BGM/{:02}/{:04}.PCM", (track % 1000) / 10, track)
}

/// A decoded music track: 24 kHz stereo frames and its loop.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub track: u32,
    pub sample_rate: u32,
    /// Interleaved left/right samples.
    pub samples: Vec<i16>,
    /// (loop start, loop end) in frames; `None` plays once.
    pub looped: Option<(u32, u32)>,
}

/// Decode a track file against its `PCMLIST.BIN` record.
pub fn decode_track(bytes: &[u8], record: &TrackRecord) -> Result<Track> {
    let (data_bytes, frames, loop_start, loop_end) =
        (u32_at(bytes, 0)?, u32_at(bytes, 4)?, u32_at(bytes, 8)?, u32_at(bytes, 12)?);
    if data_bytes != record.data_bytes || loop_start != record.loop_start || loop_end != record.loop_end {
        return Err(invalid("track header differs from its PCMLIST record"));
    }
    if u64::from(frames) * 4 > u64::from(data_bytes) {
        return Err(invalid("track frame count beyond its data"));
    }
    let data = bytes
        .get(2048..2048 + frames as usize * 4)
        .ok_or_else(|| invalid("track data truncated"))?;
    let samples = data.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]])).collect();
    let looped = (loop_start < loop_end && loop_end <= frames).then_some((loop_start, loop_end));
    Ok(Track { track: record.track, sample_rate: 24_000, samples, looped })
}

/// One sound-effect entry (`_tagSD_SEVAG`) with its decoded sample.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundEffect {
    pub volume: u8,
    pub vtime: u8,
    pub priority: u8,
    pub voice_group: u8,
    pub note: u8,
    pub fine: u8,
    /// Mono samples at the note's pitch.
    pub samples: Vec<i16>,
    /// Loop start sample when the data loops.
    pub loop_start: Option<usize>,
}

/// A GRP bank.
pub fn decode_grp(bytes: &[u8]) -> Result<Vec<SoundEffect>> {
    let count = u32_at(bytes, 0)? as usize;
    if count > 255 || bytes.len() < 2048 {
        return Err(invalid("GRP header out of range"));
    }
    (0..count)
        .map(|i| {
            let e = bytes.get(8 + 8 * i..16 + 8 * i).ok_or_else(|| invalid("GRP entry"))?;
            let pitch = u16::from_le_bytes([e[4], e[5]]);
            let offset = usize::from(u16::from_le_bytes([e[6], e[7]]));
            let (samples, loop_start) = decode_adpcm(bytes, 2048 + offset * 16)?;
            Ok(SoundEffect {
                volume: e[0],
                vtime: e[1],
                priority: e[2],
                voice_group: e[3],
                note: (pitch >> 8) as u8,
                fine: (pitch & 0x7F) as u8,
                samples,
                loop_start,
            })
        })
        .collect()
}

/// PS-ADPCM (SPU) from `start` to the block with the end flag; the loop
/// start is the last block flagged 4 when the end block repeats (flag 2).
pub fn decode_adpcm(bytes: &[u8], start: usize) -> Result<(Vec<i16>, Option<usize>)> {
    const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];
    let (mut s1, mut s2) = (0i32, 0i32);
    let mut out = Vec::new();
    let mut loop_at = None;
    let mut at = start;
    loop {
        let block = bytes.get(at..at + 16).ok_or_else(|| invalid("ADPCM without an end block"))?;
        let shift = block[0] & 15;
        let (f0, f1) = FILTERS[usize::from((block[0] >> 4).min(4))];
        let flags = block[1];
        if flags & 4 != 0 {
            loop_at = Some(out.len());
        }
        for b in &block[2..16] {
            for nibble in [b & 15, b >> 4] {
                let mut v = i32::from(((u16::from(nibble) << 12) as i16) >> shift.min(12));
                v += (s1 * f0 + s2 * f1 + 32) >> 6;
                let v = v.clamp(-32768, 32767);
                out.push(v as i16);
                s2 = s1;
                s1 = v;
            }
        }
        at += 16;
        if flags & 1 != 0 {
            return Ok((out, if flags & 2 != 0 { loop_at } else { None }));
        }
        if out.len() > 8 * 1024 * 1024 {
            return Err(invalid("ADPCM sample too long"));
        }
    }
}

/// The driver's note table (`MIDI_Note2Pitch`, 12 x 128 u16, T[0][0] =
/// 0x1000) from `KCEJEAST.IRX` (.rodata 0x13160 of its loadable segment).
pub fn parse_pitch_table(irx: &[u8]) -> Result<Vec<u16>> {
    if irx.get(..4) != Some(b"\x7fELF") {
        return Err(invalid("KCEJEAST.IRX is not an ELF image"));
    }
    let phoff = u32_at(irx, 0x1C)? as usize;
    let size = usize::from(u16::from_le_bytes([irx[0x2A], irx[0x2B]]));
    let count = usize::from(u16::from_le_bytes([irx[0x2C], irx[0x2D]]));
    for i in 0..count {
        let h = phoff + i * size;
        if u32_at(irx, h)? == 1 {
            let offset = u32_at(irx, h + 4)? as usize;
            let vaddr = u32_at(irx, h + 8)? as usize;
            let at = offset + 0x13160 - vaddr;
            let table: Vec<u16> = irx
                .get(at..at + 12 * 128 * 2)
                .ok_or_else(|| invalid("pitch table outside the IRX"))?
                .chunks_exact(2)
                .map(|s| u16::from_le_bytes([s[0], s[1]]))
                .collect();
            if table[0] != 0x1000 || table[128] <= table[127] {
                return Err(invalid("unexpected pitch table"));
            }
            return Ok(table);
        }
    }
    Err(invalid("KCEJEAST.IRX has no loadable segment"))
}

/// `MIDI_Note2Pitch(note, fine, centre 60)` * 147 / 160: the SPU pitch
/// (0x1000 = 48 kHz output rate).
pub fn note_pitch(table: &[u16], note: u8, fine: u8) -> u32 {
    let d = i32::from(note) - 60;
    let fine = usize::from(fine & 0x7F);
    let base = if d >= 0 {
        u32::from(table[(d % 12) as usize * 128 + fine]) << (d / 12)
    } else {
        let row = ((12 - (-d) % 12) % 12) as usize;
        u32::from(table[row * 128 + fine]) >> ((-d + 11) / 12)
    };
    base * 147 / 160
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_60_is_44100_and_note_36_is_11025() {
        let mut table = vec![0u16; 12 * 128];
        for r in 0..12 {
            for f in 0..128 {
                table[r * 128 + f] = (4096.0 * 2f64.powf((r as f64 + f as f64 / 128.0) / 12.0)).round() as u16;
            }
        }
        // Integer pitch: 0x1000 * 147 / 160 = 3763 (44097 Hz).
        assert_eq!(note_pitch(&table, 60, 0), 3763);
        assert_eq!(note_pitch(&table, 36, 0) * 48_000 / 0x1000, 11_015);
    }

    #[test]
    fn adpcm_end_block_stops_the_sample() {
        let mut data = vec![0u8; 32];
        data[1] = 0;
        data[16 + 1] = 1;
        let (s, l) = decode_adpcm(&data, 0).unwrap();
        assert_eq!((s.len(), l), (56, None));
    }
}
