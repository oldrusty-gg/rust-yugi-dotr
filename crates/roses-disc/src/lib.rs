//! Read-only ISO9660 inventory and fingerprinting for user-supplied disc images.
//!
//! This crate accepts plain, cooked 2048-byte-sector ISO images. It does not
//! unpack archives, mount images, or modify the input. Joliet/Rock Ridge naming,
//! multi-extent files, interleaving, and extended attributes are deliberately
//! unsupported. A fingerprint match identifies a retail candidate, not verified
//! game behavior or legal provenance.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const SECTOR: u64 = 2048;
const MAX_DIRECTORY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_DIRECTORY_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const MAX_DEPTH: usize = 128;
pub const PAL_PROFILE_ID: &str = "dotr-europe-sles-51579";
pub const PAL_ISO_SHA256: &str = "dba0ac673efe8eeee58a462ecfed1a55bcca594dcd6ba59afb24805e886aecbe";
pub const PAL_EXECUTABLE_SHA256: &str =
    "0b23138866f478cd6ef69190388925b32b1e105635b09298b3a1ddd91549c1a2";

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(String),
    Unsupported(String),
    NotFound(String),
    Limit(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "disc I/O: {e}"),
            Self::Invalid(s) => write!(f, "invalid ISO9660: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported ISO9660: {s}"),
            Self::NotFound(s) => write!(f, "disc file not found: {s}"),
            Self::Limit(s) => write!(f, "disc inspection limit: {s}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DiscEntry {
    /// Slash-separated primary-volume path, without a leading slash or `;1`.
    pub path: String,
    pub is_directory: bool,
    pub extent_lba: u32,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BootConfig {
    pub boot_executable: Option<String>,
    pub video_mode: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationStatus {
    KnownRetailCandidate,
    Unrecognized,
}

#[derive(Clone, Debug, Serialize)]
pub struct Identification {
    pub status: IdentificationStatus,
    pub candidate_profile: Option<String>,
    pub iso_hash_matches_known_pal: bool,
    pub executable_hash_matches_known_pal: bool,
    /// Always false: inspection does not run or compare the game.
    pub gameplay_validated: bool,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscReport {
    pub schema_version: u32,
    pub iso_size: u64,
    pub iso_sha256: String,
    pub volume_id: String,
    pub volume_size: u64,
    pub boot: Option<BootConfig>,
    pub executable_sha256: Option<String>,
    pub identification: Identification,
    pub entries: Vec<DiscEntry>,
}

/// An open, read-only ISO. Keep its source unchanged while inspecting it.
pub struct Disc {
    file: File,
    iso_size: u64,
    volume_size: u64,
    volume_id: String,
    entries: Vec<DiscEntry>,
}

impl Disc {
    /// Open an ISO and validate/inventory its primary directory tree.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut file = File::open(path)?;
        let iso_size = file.metadata()?.len();
        let inventory = parse_image(&mut file, iso_size)?;
        Ok(Self {
            file,
            iso_size,
            volume_size: inventory.volume_size,
            volume_id: inventory.volume_id,
            entries: inventory.entries,
        })
    }

    pub fn entries(&self) -> &[DiscEntry] {
        &self.entries
    }

    /// Read one file, rejecting it before allocation if it exceeds `max_bytes`.
    /// Paths are case-insensitive and accept slash/backslash separators and `;1`.
    pub fn read_file(&mut self, path: &str, max_bytes: u64) -> Result<Vec<u8>> {
        let entry = self.find_file(path)?.clone();
        read_extent(&mut self.file, &entry, self.volume_size, max_bytes)
    }

    /// Hash the entire ISO and its boot executable, returning metadata only.
    /// This reads all ISO bytes and can take time on large or remote images.
    pub fn inspect(&mut self) -> Result<DiscReport> {
        let mut notes = vec![
            "Fingerprints identify a candidate build; gameplay, asset decoding, and provenance are not validated.".into(),
        ];
        let boot = if self
            .entries
            .iter()
            .any(|e| e.path.eq_ignore_ascii_case("SYSTEM.CNF") && !e.is_directory)
        {
            Some(parse_system_cnf(&self.read_file("SYSTEM.CNF", 16 * 1024)?)?)
        } else {
            notes.push("No SYSTEM.CNF was found in the primary volume.".into());
            None
        };
        let executable_sha256 = match boot.as_ref().and_then(|b| b.boot_executable.as_ref()) {
            Some(path) => {
                let entry = self.find_file(path)?.clone();
                Some(hash_range(
                    &mut self.file,
                    u64::from(entry.extent_lba) * SECTOR,
                    entry.size,
                )?)
            }
            None => None,
        };
        let iso_sha256 = hash_range(&mut self.file, 0, self.iso_size)?;
        if self.file.metadata()?.len() != self.iso_size {
            return Err(Error::Invalid(
                "ISO length changed during inspection".into(),
            ));
        }
        let iso_match = iso_sha256 == PAL_ISO_SHA256;
        let executable_match = executable_sha256.as_deref() == Some(PAL_EXECUTABLE_SHA256);
        let named_pal = boot.as_ref().is_some_and(|b| {
            b.boot_executable
                .as_deref()
                .is_some_and(|p| p.eq_ignore_ascii_case("SLES_515.79"))
                && b.video_mode
                    .as_deref()
                    .is_some_and(|v| v.eq_ignore_ascii_case("PAL"))
        });
        let known = iso_match && executable_match && named_pal;
        if named_pal && !known {
            notes.push("The boot configuration resembles the PAL edition, but the complete known fingerprints do not match.".into());
        }
        Ok(DiscReport {
            schema_version: 1,
            iso_size: self.iso_size,
            iso_sha256,
            volume_id: self.volume_id.clone(),
            volume_size: self.volume_size,
            boot,
            executable_sha256,
            identification: Identification {
                status: if known {
                    IdentificationStatus::KnownRetailCandidate
                } else {
                    IdentificationStatus::Unrecognized
                },
                candidate_profile: known.then(|| PAL_PROFILE_ID.into()),
                iso_hash_matches_known_pal: iso_match,
                executable_hash_matches_known_pal: executable_match,
                gameplay_validated: false,
                notes,
            },
            entries: self.entries.clone(),
        })
    }

    fn find_file(&self, path: &str) -> Result<&DiscEntry> {
        let path = normalize_lookup(path)?;
        self.entries
            .iter()
            .find(|e| !e.is_directory && e.path.eq_ignore_ascii_case(&path))
            .ok_or(Error::NotFound(path))
    }
}

struct Inventory {
    volume_size: u64,
    volume_id: String,
    entries: Vec<DiscEntry>,
}

fn parse_image(reader: &mut (impl Read + Seek), iso_size: u64) -> Result<Inventory> {
    if iso_size < 18 * SECTOR || !iso_size.is_multiple_of(SECTOR) {
        return Err(Error::Invalid(
            "expected a cooked image containing whole 2048-byte sectors".into(),
        ));
    }
    let mut primary = None;
    let mut terminated = false;
    for lba in 16..272 {
        if (lba + 1) * SECTOR > iso_size {
            return Err(Error::Invalid(
                "truncated volume descriptor sequence".into(),
            ));
        }
        reader.seek(SeekFrom::Start(lba * SECTOR))?;
        let mut descriptor = [0u8; SECTOR as usize];
        reader.read_exact(&mut descriptor)?;
        if &descriptor[1..6] != b"CD001" || descriptor[6] != 1 {
            return Err(Error::Invalid(format!(
                "bad volume descriptor at sector {lba}"
            )));
        }
        match descriptor[0] {
            1 => {
                if primary.is_some() {
                    return Err(Error::Unsupported(
                        "multiple primary volume descriptors".into(),
                    ));
                }
                primary = Some(descriptor);
            }
            255 => {
                terminated = true;
                break;
            }
            0 | 2 | 3 => {} // Boot/supplementary/partition descriptors are not used.
            other => {
                return Err(Error::Unsupported(format!(
                    "volume descriptor type {other}"
                )))
            }
        }
    }
    if !terminated {
        return Err(Error::Limit(
            "volume descriptor terminator absent in first 256 descriptors".into(),
        ));
    }
    let pvd = primary.ok_or_else(|| Error::Invalid("no primary volume descriptor".into()))?;
    if both_u16(&pvd[128..132], "logical block size")? != SECTOR as u16 {
        return Err(Error::Unsupported("logical block size is not 2048".into()));
    }
    if both_u16(&pvd[120..124], "volume set size")? != 1
        || both_u16(&pvd[124..128], "volume sequence")? != 1
    {
        return Err(Error::Unsupported("multi-volume disc".into()));
    }
    let volume_size = u64::from(both_u32(&pvd[80..88], "volume space size")?) * SECTOR;
    if volume_size > iso_size || volume_size < 18 * SECTOR {
        return Err(Error::Invalid(
            "declared volume exceeds image or is too small".into(),
        ));
    }
    let root_len = usize::from(pvd[156]);
    if root_len < 34 || 156 + root_len > pvd.len() {
        return Err(Error::Invalid(
            "invalid root directory record length".into(),
        ));
    }
    let root = parse_record(&pvd[156..156 + root_len], volume_size)?;
    if !root.is_directory || root.name != RecordName::SelfEntry {
        return Err(Error::Invalid(
            "root record must be the self directory".into(),
        ));
    }
    let volume_id = std::str::from_utf8(&pvd[40..72])
        .map_err(|_| Error::Unsupported("non-UTF8 primary volume identifier".into()))?
        .trim_end_matches([' ', '\0'])
        .to_owned();
    let mut entries = Vec::new();
    let mut pending = vec![(String::new(), root.extent_lba, root.size, 0usize)];
    let mut directories = HashSet::new();
    let mut paths = BTreeSet::new();
    let mut directory_bytes = 0u64;
    while let Some((path, extent_lba, size, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(Error::Limit("directory nesting exceeds 128".into()));
        }
        if !directories.insert(extent_lba) {
            return Err(Error::Invalid(format!(
                "directory cycle or alias at {path}"
            )));
        }
        directory_bytes = directory_bytes
            .checked_add(size)
            .ok_or_else(|| Error::Limit("directory byte total overflow".into()))?;
        if directory_bytes > MAX_TOTAL_DIRECTORY_BYTES {
            return Err(Error::Limit("total directory data exceeds 256 MiB".into()));
        }
        let dir = DiscEntry {
            path: path.clone(),
            is_directory: true,
            extent_lba,
            size,
        };
        let bytes = read_extent(reader, &dir, volume_size, MAX_DIRECTORY_BYTES)?;
        let mut offset = 0usize;
        while offset < bytes.len() {
            let record_len = usize::from(bytes[offset]);
            let sector_end = ((offset / SECTOR as usize + 1) * SECTOR as usize).min(bytes.len());
            if record_len == 0 {
                if bytes[offset..sector_end].iter().any(|b| *b != 0) {
                    return Err(Error::Invalid(format!(
                        "nonzero directory padding in {path}"
                    )));
                }
                offset = sector_end;
                continue;
            }
            if record_len < 34 || offset + record_len > sector_end {
                return Err(Error::Invalid(format!(
                    "directory record crosses sector or is truncated in {path}"
                )));
            }
            let record = parse_record(&bytes[offset..offset + record_len], volume_size)?;
            offset += record_len;
            let name = match record.name {
                RecordName::Name(n) => n,
                RecordName::SelfEntry | RecordName::ParentEntry => {
                    if !record.is_directory {
                        return Err(Error::Invalid(
                            "self/parent entry is not a directory".into(),
                        ));
                    }
                    continue;
                }
            };
            let full_path = if path.is_empty() {
                name
            } else {
                format!("{path}/{name}")
            };
            if !paths.insert(full_path.to_ascii_uppercase()) {
                return Err(Error::Invalid(format!(
                    "ambiguous duplicate path {full_path}"
                )));
            }
            if entries.len() >= MAX_ENTRIES {
                return Err(Error::Limit("entry count exceeds 100,000".into()));
            }
            let entry = DiscEntry {
                path: full_path.clone(),
                is_directory: record.is_directory,
                extent_lba: record.extent_lba,
                size: record.size,
            };
            if entry.is_directory {
                pending.push((full_path, entry.extent_lba, entry.size, depth + 1));
            }
            entries.push(entry);
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Inventory {
        volume_size,
        volume_id,
        entries,
    })
}

#[derive(PartialEq)]
enum RecordName {
    SelfEntry,
    ParentEntry,
    Name(String),
}
struct Record {
    name: RecordName,
    extent_lba: u32,
    size: u64,
    is_directory: bool,
}

fn parse_record(bytes: &[u8], volume_size: u64) -> Result<Record> {
    if bytes.len() < 34 || usize::from(bytes[0]) != bytes.len() {
        return Err(Error::Invalid(
            "short or inconsistent directory record".into(),
        ));
    }
    let identifier_len = usize::from(bytes[32]);
    let minimum = 33 + identifier_len + usize::from(identifier_len % 2 == 0);
    if identifier_len == 0 || bytes.len() < minimum {
        return Err(Error::Invalid(
            "directory identifier extends past record".into(),
        ));
    }
    if bytes[1] != 0 {
        return Err(Error::Unsupported("extended attribute records".into()));
    }
    if bytes[25] & 0x80 != 0 {
        return Err(Error::Unsupported("multi-extent file".into()));
    }
    if bytes[25] & 0x60 != 0 {
        return Err(Error::Unsupported("reserved directory flags".into()));
    }
    if bytes[25] & 0x04 != 0 {
        return Err(Error::Unsupported("associated file".into()));
    }
    if bytes[26] != 0 || bytes[27] != 0 {
        return Err(Error::Unsupported("interleaved file".into()));
    }
    if both_u16(&bytes[28..32], "record volume sequence")? != 1 {
        return Err(Error::Unsupported("record on another volume".into()));
    }
    let extent_lba = both_u32(&bytes[2..10], "extent location")?;
    let size = u64::from(both_u32(&bytes[10..18], "data length")?);
    check_extent(extent_lba, size, volume_size)?;
    let is_directory = bytes[25] & 0x02 != 0;
    if is_directory && size == 0 {
        return Err(Error::Invalid("empty directory extent".into()));
    }
    let identifier = &bytes[33..33 + identifier_len];
    let name = match identifier {
        [0] => RecordName::SelfEntry,
        [1] => RecordName::ParentEntry,
        _ => {
            let raw = std::str::from_utf8(identifier)
                .map_err(|_| Error::Unsupported("non-UTF8 primary file name".into()))?;
            RecordName::Name(normalize_component(raw)?)
        }
    };
    Ok(Record {
        name,
        extent_lba,
        size,
        is_directory,
    })
}

fn both_u16(bytes: &[u8], label: &str) -> Result<u16> {
    let le = u16::from_le_bytes([bytes[0], bytes[1]]);
    let be = u16::from_be_bytes([bytes[2], bytes[3]]);
    if le != be {
        return Err(Error::Invalid(format!("{label} endian copies disagree")));
    }
    Ok(le)
}
fn both_u32(bytes: &[u8], label: &str) -> Result<u32> {
    let le = u32::from_le_bytes(bytes[0..4].try_into().expect("fixed length"));
    let be = u32::from_be_bytes(bytes[4..8].try_into().expect("fixed length"));
    if le != be {
        return Err(Error::Invalid(format!("{label} endian copies disagree")));
    }
    Ok(le)
}
fn check_extent(lba: u32, size: u64, volume_size: u64) -> Result<()> {
    let start = u64::from(lba) * SECTOR;
    let end = start
        .checked_add(size)
        .ok_or_else(|| Error::Invalid("extent overflow".into()))?;
    if start > volume_size || end > volume_size {
        return Err(Error::Invalid(format!(
            "extent at sector {lba} exceeds volume"
        )));
    }
    Ok(())
}
fn read_extent(
    reader: &mut (impl Read + Seek),
    entry: &DiscEntry,
    volume_size: u64,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    check_extent(entry.extent_lba, entry.size, volume_size)?;
    if entry.size > max_bytes {
        return Err(Error::Limit(format!(
            "{} is {} bytes, limit is {max_bytes}",
            entry.path, entry.size
        )));
    }
    let length = usize::try_from(entry.size)
        .map_err(|_| Error::Limit("file does not fit address space".into()))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Error::Limit("unable to allocate file buffer".into()))?;
    bytes.resize(length, 0);
    reader.seek(SeekFrom::Start(u64::from(entry.extent_lba) * SECTOR))?;
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}
fn hash_range(reader: &mut (impl Read + Seek), offset: u64, length: u64) -> Result<String> {
    reader.seek(SeekFrom::Start(offset))?;
    let mut remaining = length;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        let amount = remaining.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..amount])?;
        digest.update(&buffer[..amount]);
        remaining -= amount as u64;
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn normalize_component(raw: &str) -> Result<String> {
    let name = raw.strip_suffix(";1").unwrap_or(raw);
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | ';'))
    {
        return Err(Error::Invalid(format!(
            "unsafe or unsupported path component {raw:?}"
        )));
    }
    Ok(name.into())
}
fn normalize_lookup(path: &str) -> Result<String> {
    let path = path.replace('\\', "/");
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return Err(Error::Invalid("empty file path".into()));
    }
    path.split('/')
        .map(normalize_component)
        .collect::<Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}
fn parse_system_cnf(bytes: &[u8]) -> Result<BootConfig> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| Error::Invalid("SYSTEM.CNF is not UTF8/ASCII".into()))?;
    let mut boot = None;
    let mut video_mode = None;
    for line in source.trim_end_matches('\0').lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.eq_ignore_ascii_case("BOOT2") {
            if boot.is_some() {
                return Err(Error::Invalid("duplicate BOOT2 in SYSTEM.CNF".into()));
            }
            let (device, path) = value
                .split_once(':')
                .ok_or_else(|| Error::Invalid("BOOT2 has no device prefix".into()))?;
            if !device.eq_ignore_ascii_case("cdrom0") && !device.eq_ignore_ascii_case("cdrom") {
                return Err(Error::Unsupported(format!("BOOT2 device {device:?}")));
            }
            boot = Some(normalize_lookup(path)?);
        } else if key.eq_ignore_ascii_case("VMODE") {
            if video_mode.is_some() {
                return Err(Error::Invalid("duplicate VMODE in SYSTEM.CNF".into()));
            }
            video_mode = Some(value.to_ascii_uppercase());
        }
    }
    Ok(BootConfig {
        boot_executable: boot,
        video_mode,
    })
}

#[cfg(test)]
mod tests;
