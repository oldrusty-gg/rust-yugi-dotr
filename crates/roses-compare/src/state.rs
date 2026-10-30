//! Bounded, read-only observation of one archived EE RAM buffer. No extraction
//! to the filesystem, state loading, or inference of game identity is performed.

use crate::pine::{validate_watches, MemoryObservation, MemoryWatch};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;
use zip::ZipArchive;

pub const MAX_STATE_BYTES: u64 = 128 * 1024 * 1024;
pub const EE_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 256;

#[derive(Debug)]
pub enum StateError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Invalid(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "savestate I/O: {error}"),
            Self::Zip(error) => write!(f, "savestate ZIP: {error}"),
            Self::Invalid(message) => write!(f, "invalid or unsupported savestate: {message}"),
        }
    }
}
impl std::error::Error for StateError {}
impl From<io::Error> for StateError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<zip::result::ZipError> for StateError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Zip(value)
    }
}
fn invalid(message: impl Into<String>) -> StateError {
    StateError::Invalid(message.into())
}

#[derive(Debug, Serialize)]
pub struct SavedMemoryCapture {
    pub schema_version: String,
    pub state_sha256: String,
    pub ee_memory_sha256: String,
    pub state_bytes: u64,
    pub ee_memory_bytes: usize,
    pub compression_method: String,
    pub observations: Vec<MemoryObservation>,
    /// Every observation comes from one immutable decoded byte buffer. This is
    /// NOT a claim about whole-machine consistency or emulator capture timing.
    pub atomic_saved_memory: bool,
    pub normalized_game_state: bool,
    pub scope: String,
}

/// Read exactly one 32 MiB `eeMemory.bin` entry from a bounded ZIP savestate.
/// Source/archive and decoded-RAM hashes identify the exact bytes observed.
pub fn capture_state(
    path: impl AsRef<Path>,
    watches: &[MemoryWatch],
) -> Result<SavedMemoryCapture, StateError> {
    validate_watches(watches).map_err(|error| invalid(error.to_string()))?;
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_STATE_BYTES {
        return Err(invalid("archive exceeds 128 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(invalid("archive grew beyond 128 MiB during reading"));
    }
    capture_bytes(&bytes, watches)
}

fn capture_bytes(bytes: &[u8], watches: &[MemoryWatch]) -> Result<SavedMemoryCapture, StateError> {
    let entries = validate_directory(bytes)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    if archive.len() != entries {
        return Err(invalid("ZIP parser normalized duplicate entry names"));
    }
    let mut entry = archive.by_name("eeMemory.bin")?;
    if entry.size() != EE_MEMORY_BYTES as u64 {
        return Err(invalid("eeMemory.bin must be exactly 32 MiB"));
    }
    if entry.encrypted() {
        return Err(invalid("encrypted EE memory is unsupported"));
    }
    let compression_method = match entry.compression() {
        zip::CompressionMethod::Stored => "stored",
        zip::CompressionMethod::Deflated => "deflate",
        zip::CompressionMethod::Zstd => "zstandard",
        _ => return Err(invalid("unsupported EE memory compression method")),
    }
    .to_owned();
    let mut memory = Vec::with_capacity(EE_MEMORY_BYTES);
    // The extra byte detects streams whose actual output exceeds their claimed
    // size. Reading through EOF also lets zip verify the entry's CRC.
    entry
        .by_ref()
        .take(EE_MEMORY_BYTES as u64 + 1)
        .read_to_end(&mut memory)?;
    if memory.len() != EE_MEMORY_BYTES {
        return Err(invalid("decoded EE memory is truncated or exceeds 32 MiB"));
    }
    let mut observations = Vec::with_capacity(watches.len());
    for watch in watches {
        let start = watch.address as usize;
        let count = watch.width.byte_count();
        let mut value = [0u8; 8];
        value[..count].copy_from_slice(&memory[start..start + count]);
        observations.push(MemoryObservation {
            name: watch.name.clone(),
            address: watch.address,
            width: watch.width,
            value: u64::from_le_bytes(value),
        });
    }
    Ok(SavedMemoryCapture {
        schema_version: "roses-saved-ee-memory-1".into(),
        state_sha256: format!("{:x}", Sha256::digest(bytes)),
        ee_memory_sha256: format!("{:x}", Sha256::digest(&memory)),
        state_bytes: bytes.len() as u64,
        ee_memory_bytes: memory.len(),
        compression_method,
        observations,
        atomic_saved_memory: true,
        normalized_game_state: false,
        scope: "Raw observations from one immutable archived EE RAM byte buffer. Atomicity applies only to this file view, not whole-machine consistency or capture timing. No game identity, address meaning, gameplay semantics or emulator compatibility is inferred.".into(),
    })
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed header bounds checked"),
    )
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed header bounds checked"),
    )
}

fn validate_extra_fields(bytes: &[u8]) -> Result<(), StateError> {
    let mut cursor = 0;
    while cursor < bytes.len() {
        let header = bytes
            .get(cursor..cursor + 4)
            .ok_or_else(|| invalid("truncated ZIP extra-field header"))?;
        let kind = u16_at(header, 0);
        let length = usize::from(u16_at(header, 2));
        if matches!(kind, 0x0001 | 0x7075 | 0x9901) {
            return Err(invalid(
                "ZIP64, Unicode-path and AES metadata overrides are unsupported",
            ));
        }
        cursor += 4 + length;
        if cursor > bytes.len() {
            return Err(invalid("truncated ZIP extra-field payload"));
        }
    }
    Ok(())
}

/// zip 2.4.2 stores entries in a name-keyed map, silently replacing duplicates.
/// Inspect the bounded standard central directory before constructing ZipArchive.
/// ZIP64, split archives, prefixes and nonstandard central trailers are outside
/// this deliberately narrow PCSX2 observer's supported format.
fn validate_directory(bytes: &[u8]) -> Result<usize, StateError> {
    if bytes.len() < 22 || bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(invalid("archive size is outside supported bounds"));
    }
    let first = bytes.len().saturating_sub(22 + usize::from(u16::MAX));
    let end = (first..=bytes.len() - 22)
        .rev()
        .find(|offset| {
            bytes[*offset..*offset + 4] == *b"PK\x05\x06"
                && *offset + 22 + usize::from(u16_at(bytes, *offset + 20)) == bytes.len()
        })
        .ok_or_else(|| invalid("missing or truncated ZIP end record"))?;
    let footer = &bytes[end..end + 22];
    let count = usize::from(u16_at(footer, 10));
    let size = u32_at(footer, 12);
    let start = u32_at(footer, 16);
    if u16_at(footer, 4) != 0 || u16_at(footer, 6) != 0 || usize::from(u16_at(footer, 8)) != count {
        return Err(invalid("multi-disk archives are unsupported"));
    }
    if count == usize::from(u16::MAX) || size == u32::MAX || start == u32::MAX {
        return Err(invalid("ZIP64 archives are unsupported"));
    }
    if count == 0 || count > MAX_ENTRIES || u64::from(start) + u64::from(size) != end as u64 {
        return Err(invalid("central directory count or bounds are unsupported"));
    }
    let mut cursor = start as usize;
    let mut names = BTreeSet::new();
    let mut ee_count = 0;
    for _ in 0..count {
        let header = bytes
            .get(cursor..cursor + 46)
            .filter(|_| cursor + 46 <= end)
            .ok_or_else(|| invalid("truncated central directory entry"))?;
        if &header[..4] != b"PK\x01\x02" {
            return Err(invalid("bad central directory signature"));
        }
        let name_len = usize::from(u16_at(header, 28));
        let extra_len = usize::from(u16_at(header, 30));
        let comment_len = usize::from(u16_at(header, 32));
        let next = cursor + 46 + name_len + extra_len + comment_len;
        if next > end {
            return Err(invalid("truncated central name or extra data"));
        }
        let name = &bytes[cursor + 46..cursor + 46 + name_len];
        validate_extra_fields(&bytes[cursor + 46 + name_len..cursor + 46 + name_len + extra_len])?;
        if !names.insert(name) {
            return Err(invalid("duplicate ZIP entry name"));
        }
        if name == b"eeMemory.bin" {
            ee_count += 1;
        }
        if u16_at(header, 34) != 0 {
            return Err(invalid("entry refers to a different disk"));
        }
        let compressed = u32_at(header, 20);
        let uncompressed = u32_at(header, 24);
        let local_offset = u32_at(header, 42);
        if [compressed, uncompressed, local_offset].contains(&u32::MAX) {
            return Err(invalid("ZIP64 entry fields are unsupported"));
        }
        if u64::from(local_offset) + 30 > u64::from(start) {
            return Err(invalid(
                "local entry header lies outside the bounded data region",
            ));
        }
        let local_offset = local_offset as usize;
        let local = bytes
            .get(local_offset..local_offset + 30)
            .filter(|_| local_offset + 30 <= start as usize)
            .ok_or_else(|| invalid("truncated local entry header"))?;
        if &local[..4] != b"PK\x03\x04"
            || u16_at(local, 6) != u16_at(header, 8)
            || u16_at(local, 8) != u16_at(header, 10)
        {
            return Err(invalid("local and central entry headers disagree"));
        }
        let local_name_len = usize::from(u16_at(local, 26));
        let data_start = local_offset + 30 + local_name_len + usize::from(u16_at(local, 28));
        if data_start as u64 + u64::from(compressed) > u64::from(start)
            || bytes.get(local_offset + 30..local_offset + 30 + local_name_len) != Some(name)
        {
            return Err(invalid(
                "local name or compressed-data bounds disagree with directory",
            ));
        }
        validate_extra_fields(&bytes[local_offset + 30 + local_name_len..data_start])?;
        let descriptor = u16_at(local, 6) & 8 != 0;
        for (local_field, central_field) in [(14, 16), (18, 20), (22, 24)] {
            let value = u32_at(local, local_field);
            if value != u32_at(header, central_field) && !(descriptor && value == 0) {
                return Err(invalid("local and central CRC or size fields disagree"));
            }
        }
        cursor = next;
    }
    if cursor != end || ee_count != 1 {
        return Err(invalid(
            "archive must contain exactly one eeMemory.bin and a complete central directory",
        ));
    }
    Ok(count)
}

#[cfg(test)]
mod tests;
