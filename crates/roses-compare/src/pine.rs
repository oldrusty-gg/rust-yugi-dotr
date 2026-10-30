//! A read-only PINE client for PCSX2's Windows loopback TCP transport.
//!
//! Wire format verified against upstream `pcsx2/PINE.cpp` and `PINE.h` on
//! 2026-09-27. See this crate's README for sources and observation limitations.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

pub const DEFAULT_PORT: u16 = 28011;
const MAX_REPLY: usize = 450_000;
const EE_RAM_BYTES: u32 = 32 * 1024 * 1024;
const MAX_WATCHES: usize = 4096;

#[derive(Debug)]
pub enum PineError {
    Io(io::Error),
    Protocol(String),
    Rejected,
    InvalidRequest(String),
    NotPaused(EmulatorStatus),
    Disconnected,
}

impl fmt::Display for PineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "PINE transport: {error}"),
            Self::Protocol(message) => write!(f, "PINE malformed response: {message}"),
            Self::Rejected => {
                f.write_str("PCSX2 rejected the PINE request (the game may have stopped)")
            }
            Self::InvalidRequest(message) => write!(f, "invalid PINE request: {message}"),
            Self::NotPaused(status) => write!(
                f,
                "memory observation requires a manually paused emulator; status is {status:?}"
            ),
            Self::Disconnected => {
                f.write_str("PINE connection is unusable after an earlier failure; reconnect")
            }
        }
    }
}

impl std::error::Error for PineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for PineError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmulatorStatus {
    Running,
    Paused,
    Shutdown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameInfo {
    pub title: String,
    pub serial: String,
    /// PINE calls this UUID, but PCSX2 returns its eight-digit disc CRC.
    /// This is not a SHA-256 digest and does not replace importer hashes.
    pub disc_crc: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmulatorInfo {
    pub emulator_version: String,
    pub status: EmulatorStatus,
    pub game: Option<GameInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum ReadWidth {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
}

impl TryFrom<u8> for ReadWidth {
    type Error = String;

    fn try_from(bits: u8) -> Result<Self, Self::Error> {
        match bits {
            8 => Ok(Self::Bits8),
            16 => Ok(Self::Bits16),
            32 => Ok(Self::Bits32),
            64 => Ok(Self::Bits64),
            _ => Err("memory width must be 8, 16, 32 or 64 bits".into()),
        }
    }
}

impl From<ReadWidth> for u8 {
    fn from(width: ReadWidth) -> Self {
        match width {
            ReadWidth::Bits8 => 8,
            ReadWidth::Bits16 => 16,
            ReadWidth::Bits32 => 32,
            ReadWidth::Bits64 => 64,
        }
    }
}

impl ReadWidth {
    pub(crate) fn byte_count(self) -> usize {
        usize::from(u8::from(self)) / 8
    }

    fn opcode(self) -> u8 {
        match self {
            Self::Bits8 => 0,
            Self::Bits16 => 1,
            Self::Bits32 => 2,
            Self::Bits64 => 3,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryWatch {
    pub name: String,
    /// Physical EE RAM address, expressed as an unsigned integer in JSON.
    pub address: u32,
    pub width: ReadWidth,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryObservation {
    pub name: String,
    pub address: u32,
    pub width: ReadWidth,
    pub value: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryCapture {
    pub status_before: EmulatorStatus,
    pub status_after: EmulatorStatus,
    pub observations: Vec<MemoryObservation>,
    /// Always false: PINE reads on a service thread without a snapshot lock.
    pub atomic: bool,
    pub scope: String,
}

pub struct PineClient {
    stream: TcpStream,
    timeout: Duration,
    usable: bool,
}

impl PineClient {
    /// Connect only to IPv4 loopback. PCSX2 uses TCP on Windows and Unix
    /// sockets elsewhere; this client intentionally implements Windows TCP.
    pub fn connect_local(port: u16, timeout: Duration) -> Result<Self, PineError> {
        if port == 0 || timeout.is_zero() || timeout > Duration::from_secs(60) {
            return Err(PineError::InvalidRequest(
                "port must be nonzero and timeout must be within (0, 60 seconds]".into(),
            ));
        }
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let stream = TcpStream::connect_timeout(&address, timeout)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            timeout,
            usable: true,
        })
    }

    pub fn status(&mut self) -> Result<EmulatorStatus, PineError> {
        let body = self.exchange(&[15])?;
        let mut cursor = Cursor::new(&body);
        let status = cursor.status();
        let parsed = status.and_then(|status| {
            cursor.finish()?;
            Ok(status)
        });
        self.checked(parsed)
    }

    /// Get emulator version/status, and game identity when a VM is active.
    /// If the VM changes while querying, failures are returned, not concealed.
    pub fn identify(&mut self) -> Result<EmulatorInfo, PineError> {
        let body = self.exchange(&[8, 15])?;
        let parsed = (|| {
            let mut cursor = Cursor::new(&body);
            let emulator_version = cursor.string()?;
            let status = cursor.status()?;
            cursor.finish()?;
            Ok((emulator_version, status))
        })();
        let (emulator_version, status) = self.checked(parsed)?;
        let game = if status == EmulatorStatus::Shutdown {
            None
        } else {
            let body = self.exchange(&[11, 12, 13, 14])?;
            let parsed = (|| {
                let mut cursor = Cursor::new(&body);
                let game = GameInfo {
                    title: cursor.string()?,
                    serial: cursor.string()?,
                    disc_crc: cursor.string()?,
                    version: cursor.string()?,
                };
                cursor.finish()?;
                Ok(game)
            })();
            Some(self.checked(parsed)?)
        };
        Ok(EmulatorInfo {
            emulator_version,
            status,
            game,
        })
    }

    /// Read a caller-supplied list of physical EE RAM addresses. The emulator
    /// must already be paused and must still be paused afterward. These checks
    /// reduce timing ambiguity but cannot prove an atomic or stable snapshot.
    /// Values remain raw observations until an independently verified profile
    /// gives their addresses meaning. No built-in game addresses are assumed.
    pub fn read_watches(&mut self, watches: &[MemoryWatch]) -> Result<MemoryCapture, PineError> {
        validate_watches(watches)?;
        let mut commands = Vec::with_capacity(watches.len() * 5);
        for watch in watches {
            commands.push(watch.width.opcode());
            commands.extend_from_slice(&watch.address.to_le_bytes());
        }
        let status_before = self.status()?;
        if status_before != EmulatorStatus::Paused {
            return Err(PineError::NotPaused(status_before));
        }
        let body = self.exchange(&commands)?;
        let parsed = (|| {
            let mut cursor = Cursor::new(&body);
            let mut observations = Vec::with_capacity(watches.len());
            for watch in watches {
                let mut value = [0u8; 8];
                let width = watch.width.byte_count();
                value[..width].copy_from_slice(cursor.take(width)?);
                observations.push(MemoryObservation {
                    name: watch.name.clone(),
                    address: watch.address,
                    width: watch.width,
                    value: u64::from_le_bytes(value),
                });
            }
            cursor.finish()?;
            Ok(observations)
        })();
        let observations = self.checked(parsed)?;
        let status_after = self.status()?;
        if status_after != EmulatorStatus::Paused {
            return Err(PineError::NotPaused(status_after));
        }
        Ok(MemoryCapture {
            status_before,
            status_after,
            observations,
            atomic: false,
            scope: "Raw memory values at caller-supplied addresses; address meanings and repeatability remain unverified. Paused status checks do not prove atomicity.".into(),
        })
    }

    fn checked<T>(&mut self, result: Result<T, PineError>) -> Result<T, PineError> {
        if result.is_err() {
            self.usable = false;
            let _ = self.stream.shutdown(Shutdown::Both);
        }
        result
    }

    fn exchange(&mut self, commands: &[u8]) -> Result<Vec<u8>, PineError> {
        if !self.usable {
            return Err(PineError::Disconnected);
        }
        let result = self.exchange_inner(commands);
        self.checked(result)
    }

    fn exchange_inner(&mut self, commands: &[u8]) -> Result<Vec<u8>, PineError> {
        let deadline = Instant::now() + self.timeout;
        let mut request = Vec::with_capacity(commands.len() + 4);
        request.extend_from_slice(&((commands.len() + 4) as u32).to_le_bytes());
        request.extend_from_slice(commands);
        let mut remaining = request.as_slice();
        while !remaining.is_empty() {
            self.stream.set_write_timeout(Some(time_left(deadline)?))?;
            match self.stream.write(remaining) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "PINE peer closed while writing",
                    )
                    .into())
                }
                Ok(count) => remaining = &remaining[count..],
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let mut header = [0u8; 4];
        self.read_deadline(&mut header, deadline)?;
        let length = u32::from_le_bytes(header) as usize;
        if !(5..=MAX_REPLY).contains(&length) {
            return Err(PineError::Protocol(format!(
                "frame length {length} is outside 5..={MAX_REPLY}"
            )));
        }
        let mut body = vec![0; length - 4];
        self.read_deadline(&mut body, deadline)?;
        match body[0] {
            0 => Ok(body[1..].to_vec()),
            255 if body.len() == 1 => Err(PineError::Rejected),
            status => Err(PineError::Protocol(format!(
                "invalid reply status or payload: {status}"
            ))),
        }
    }

    fn read_deadline(
        &mut self,
        mut remaining: &mut [u8],
        deadline: Instant,
    ) -> Result<(), PineError> {
        while !remaining.is_empty() {
            self.stream.set_read_timeout(Some(time_left(deadline)?))?;
            match self.stream.read(remaining) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "PINE response ended early",
                    )
                    .into())
                }
                Ok(count) => remaining = &mut remaining[count..],
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_watches(watches: &[MemoryWatch]) -> Result<(), PineError> {
    if watches.is_empty() || watches.len() > MAX_WATCHES {
        return Err(PineError::InvalidRequest(format!(
            "provide between 1 and {MAX_WATCHES} explicit memory watches"
        )));
    }
    let mut names = BTreeSet::new();
    for watch in watches {
        if watch.name.trim().is_empty() || !names.insert(&watch.name) {
            return Err(PineError::InvalidRequest(
                "watch names must be nonempty and unique".into(),
            ));
        }
        let end = watch.address.checked_add(watch.width.byte_count() as u32);
        if end.is_none_or(|end| end > EE_RAM_BYTES) {
            return Err(PineError::InvalidRequest(format!(
                "watch {:?} is outside physical EE RAM (0x00000000..0x02000000)",
                watch.name
            )));
        }
    }
    Ok(())
}

fn time_left(deadline: Instant) -> Result<Duration, PineError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::TimedOut, "PINE request deadline exceeded").into()
        })
}

struct Cursor<'a> {
    bytes: &'a [u8],
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], PineError> {
        if self.bytes.len() < count {
            return Err(PineError::Protocol("truncated payload".into()));
        }
        let (head, tail) = self.bytes.split_at(count);
        self.bytes = tail;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, PineError> {
        let bytes: [u8; 4] = self.take(4)?.try_into().expect("length checked by take");
        Ok(u32::from_le_bytes(bytes))
    }

    fn status(&mut self) -> Result<EmulatorStatus, PineError> {
        match self.u32()? {
            0 => Ok(EmulatorStatus::Running),
            1 => Ok(EmulatorStatus::Paused),
            2 => Ok(EmulatorStatus::Shutdown),
            status => Err(PineError::Protocol(format!(
                "unknown emulator status {status}"
            ))),
        }
    }

    fn string(&mut self) -> Result<String, PineError> {
        let count = self.u32()? as usize;
        let bytes = self.take(count)?;
        if bytes.last() != Some(&0) || bytes[..bytes.len() - 1].contains(&0) {
            return Err(PineError::Protocol(
                "string must have exactly one trailing NUL".into(),
            ));
        }
        String::from_utf8(bytes[..bytes.len() - 1].to_vec())
            .map_err(|_| PineError::Protocol("string is not valid UTF-8".into()))
    }

    fn finish(&self) -> Result<(), PineError> {
        if !self.bytes.is_empty() {
            return Err(PineError::Protocol(
                "unexpected trailing payload bytes".into(),
            ));
        }
        Ok(())
    }
}
