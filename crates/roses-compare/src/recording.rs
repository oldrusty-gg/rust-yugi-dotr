//! Deterministic PCSX2 v2.8.2 input recording files, format version 1.
//!
//! Generates host files only. Does not contact PCSX2, load states, or write guest
//! memory. Playback requires an explicit copy of a compatible preserved state
//! at `<complete movie filename>_SaveState.p2s`. See `RECORDING.md`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const FILE_VERSION: u8 = 1;
pub const HEADER_BYTES: usize = 570;
pub const FRAME_BYTES: usize = 36;
pub const MAX_FRAMES: u32 = 1_000_000;
pub const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Start,
    Cross,
    Circle,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingMetadata {
    /// Informational header string, normally `PCSX2-v2.8.2`.
    pub emulator_version: String,
    pub author: String,
    pub game_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputSegment {
    pub frames: u32,
    /// Held continuously for every frame in this segment. Empty means neutral.
    pub buttons: Vec<Button>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SegmentManifest {
    pub start_frame: u32,
    pub end_frame_exclusive: u32,
    pub frames: u32,
    pub buttons: Vec<Button>,
    pub port_0_bytes: [u8; 18],
    pub port_1_bytes: [u8; 18],
}

#[derive(Clone, Debug, Serialize)]
pub struct RecordingManifest {
    pub schema_version: u32,
    pub format: String,
    pub file_version: u8,
    pub protocol_source_release: String,
    pub metadata: RecordingMetadata,
    pub total_frames: u32,
    pub total_bytes: usize,
    pub from_savestate: bool,
    pub companion_state_rule: String,
    pub segments: Vec<SegmentManifest>,
    /// A valid serialization is not evidence that a particular state/sequence
    /// is repeatable in the emulator.
    pub replay_verified: bool,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingError(pub String);

impl fmt::Display for RecordingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RecordingError {}

/// Encoded bytes and a description of exactly those bytes. No mutable accessor
/// is exposed. This does not make a generated file immune to PCSX2 rerecording.
pub struct EncodedRecording {
    bytes: Vec<u8>,
    manifest: RecordingManifest,
}

impl EncodedRecording {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn manifest(&self) -> &RecordingManifest {
        &self.manifest
    }

    /// Create the movie only, never replacing an existing file. The caller must
    /// separately copy its preserved savestate to `companion_state_path(path)`.
    /// An I/O failure can leave a partial new file; no existing file is removed.
    pub fn write_new(&self, path: &Path) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&self.bytes)?;
        file.sync_all()
    }
}

/// Appends to the complete filename, preserving its `.p2m2` extension. This
/// helper only computes a path; it never reads or copies a savestate.
pub fn companion_state_path(movie: &Path) -> PathBuf {
    let mut name = movie.as_os_str().to_os_string();
    name.push("_SaveState.p2s");
    PathBuf::from(name)
}

fn neutral_pad() -> [u8; 18] {
    let mut pad = [0u8; 18];
    pad[0] = 0xff;
    pad[1] = 0xff;
    pad[2..6].fill(127);
    pad
}

fn segment_pad(segment: &InputSegment, index: usize) -> Result<[u8; 18], RecordingError> {
    let mut buttons = BTreeSet::new();
    for button in &segment.buttons {
        if !buttons.insert(*button) {
            return Err(RecordingError(format!(
                "segment {index} repeats button {button:?}"
            )));
        }
    }
    if (buttons.contains(&Button::Up) && buttons.contains(&Button::Down))
        || (buttons.contains(&Button::Left) && buttons.contains(&Button::Right))
    {
        return Err(RecordingError(format!(
            "segment {index} contains opposing directions"
        )));
    }
    let mut pad = neutral_pad();
    for button in buttons {
        let (group, mask, pressure) = match button {
            Button::Start => (0, 0x08, None),
            Button::Cross => (1, 0x40, Some(12)),
            Button::Circle => (1, 0x20, Some(11)),
            Button::Up => (0, 0x10, Some(8)),
            Button::Down => (0, 0x40, Some(9)),
            Button::Left => (0, 0x80, Some(7)),
            Button::Right => (0, 0x20, Some(6)),
        };
        pad[group] &= !mask;
        if let Some(index) = pressure {
            pad[index] = 255;
        }
    }
    Ok(pad)
}

fn header_string(value: &str, field: &str, capacity: usize) -> Result<(), RecordingError> {
    if value.trim().is_empty() || value.as_bytes().contains(&0) || value.len() >= capacity {
        return Err(RecordingError(format!(
            "{field} must be nonempty, contain no NUL, and fit in {} UTF-8 bytes",
            capacity - 1
        )));
    }
    Ok(())
}

/// Encode exact frame intervals, with no inferred/inserted frames. The last
/// segment must explicitly release every button. PCSX2 requests a pause when its
/// frame counter reaches total_frames. Stored rows are 0..total_frames-1; actual
/// first-frame alignment and stopping must be checked in a reference replay.
/// Port 1 is always neutral; multitap and analog actions are unsupported.
pub fn encode_recording(
    metadata: &RecordingMetadata,
    segments: &[InputSegment],
) -> Result<EncodedRecording, RecordingError> {
    header_string(&metadata.emulator_version, "emulator_version", 50)?;
    header_string(&metadata.author, "author", 255)?;
    header_string(&metadata.game_name, "game_name", 255)?;
    if segments.is_empty() || segments.len() > MAX_SEGMENTS {
        return Err(RecordingError(format!(
            "provide between 1 and {MAX_SEGMENTS} segments"
        )));
    }
    if !segments
        .last()
        .expect("nonempty segments")
        .buttons
        .is_empty()
    {
        return Err(RecordingError("the final segment must explicitly hold no buttons, releasing all input before replay ends".into()));
    }
    let neutral = neutral_pad();
    let mut total_frames = 0u32;
    let mut segment_manifests = Vec::with_capacity(segments.len());
    for (index, segment) in segments.iter().enumerate() {
        if segment.frames == 0 {
            return Err(RecordingError(format!(
                "segment {index} must contain at least one frame"
            )));
        }
        let end = total_frames
            .checked_add(segment.frames)
            .filter(|frames| *frames <= MAX_FRAMES)
            .ok_or_else(|| {
                RecordingError(format!("recording exceeds the {MAX_FRAMES}-frame limit"))
            })?;
        let pad = segment_pad(segment, index)?;
        let mut buttons = segment.buttons.clone();
        buttons.sort();
        segment_manifests.push(SegmentManifest {
            start_frame: total_frames,
            end_frame_exclusive: end,
            frames: segment.frames,
            buttons,
            port_0_bytes: pad,
            port_1_bytes: neutral,
        });
        total_frames = end;
    }
    // The practical frame bound keeps multiplication safe even on 32-bit hosts.
    let byte_count = HEADER_BYTES + total_frames as usize * FRAME_BYTES;
    let mut bytes = vec![0u8; byte_count];
    bytes[0] = FILE_VERSION;
    for (offset, value) in [
        (1, &metadata.emulator_version),
        (51, &metadata.author),
        (306, &metadata.game_name),
    ] {
        bytes[offset..offset + value.len()].copy_from_slice(value.as_bytes());
    }
    bytes[561..565].copy_from_slice(&total_frames.to_le_bytes());
    // Bytes 565..569 remain zero: no rerecords in a freshly generated file.
    bytes[569] = 1; // Starts from the explicitly supplied companion savestate.
    let mut offset = HEADER_BYTES;
    for segment in &segment_manifests {
        for _ in 0..segment.frames {
            bytes[offset..offset + 18].copy_from_slice(&segment.port_0_bytes);
            bytes[offset + 18..offset + FRAME_BYTES].copy_from_slice(&segment.port_1_bytes);
            offset += FRAME_BYTES;
        }
    }
    let manifest = RecordingManifest {
        schema_version: 1,
        format: "pcsx2-p2m2".into(),
        file_version: FILE_VERSION,
        protocol_source_release: "PCSX2 v2.8.2".into(),
        metadata: metadata.clone(),
        total_frames,
        total_bytes: bytes.len(),
        from_savestate: true,
        companion_state_rule: "Append _SaveState.p2s to the complete movie filename, including .p2m2; copy the compatible preserved savestate there without modifying the source.".into(),
        segments: segment_manifests,
        replay_verified: false,
        scope: "Deterministic input-file serialization only. State compatibility, game outcomes and replay repeatability require separate emulator checks. Preserve archival originals and replay working copies: PCSX2 opens input movies for writing and can rerecord past their end.".into(),
    };
    Ok(EncodedRecording { bytes, manifest })
}
