//! Platform-neutral, read-only board presentation. This is not a rules engine.
//! Producers submit an explicit `SnapshotDocument` to `BoardSnapshot::new`;
//! renderers receive immutable validated data. Capture-specific pointer checks
//! belong to the adapter, never the renderer or a future game core.

mod capture;
pub use capture::{from_saved_capture, pal_watches};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const SIDE: u8 = 7;
pub const CELL_COUNT: usize = 49;
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
pub const SNAPSHOT_SCHEMA: &str = "roses-board-view-1";
pub const NATIVE_RULES_PROFILE: &str = "roses-private-observed-board-core-1";

#[derive(Debug)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn invalid(message: impl Into<String>) -> Error {
    Error(message.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridPos {
    pub column: u8,
    pub row: u8,
}
impl GridPos {
    pub fn index(self) -> Result<usize> {
        if self.column >= SIDE || self.row >= SIDE {
            return Err(invalid(format!(
                "Coordinate ({},{}) is outside the 7-by-7 board.",
                self.column, self.row
            )));
        }
        Ok(usize::from(self.row) * usize::from(SIDE) + usize::from(self.column))
    }
}

/// Known-empty cells are explicit. Missing observations are rejected upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Occupancy {
    Empty,
    Occupied { unit_key: u16 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewCell {
    pub position: GridPos,
    pub terrain_code: u8,
    pub occupancy: Occupancy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardMetadata {
    pub name: String,
    pub attack: u16,
    pub defense: u16,
    pub level: u8,
    pub deck_cost: u8,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawUnitLocation {
    pub array_group: u8,
    pub array_slot: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewUnit {
    /// Stable within this snapshot. The PAL adapter uses group*11+slot.
    pub key: u16,
    pub card_id: u16,
    pub position: GridPos,
    pub raw_location: Option<RawUnitLocation>,
    pub card: Option<CardMetadata>,
}

/// References the original observation used to initialize native computation.
/// Supplied hashes identify provenance; they do not authenticate game identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalCaptureReference {
    pub capture_json_sha256: String,
    pub state_sha256: String,
    pub ee_memory_sha256: String,
    pub address_profile: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewOrigin {
    Synthetic {
        description: String,
    },
    SavedCapture {
        capture_json_sha256: String,
        state_sha256: String,
        ee_memory_sha256: String,
        /// An address interpretation, not authenticated game identity.
        address_profile: String,
    },
    NativeSimulation {
        source: OriginalCaptureReference,
        engine_version: String,
        rules_profile: String,
        request_sha256: String,
        /// "before" is imported initialization; "after" is computed state.
        checkpoint: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnlinkedCandidate {
    pub raw_location: RawUnitLocation,
    pub card_id: u16,
    pub column_raw: u8,
    pub row_raw: u8,
    pub constant_info_pointer: u32,
    pub side_field_raw: u8,
    pub slot_field_raw: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCandidateFields {
    pub selected_map_index: u8,
    pub active_side_byte: u8,
    pub unknown_resource_bytes: [u8; 2],
    pub unknown_b8f2: u8,
    pub unknown_b8f3: u8,
    pub unknown_process_b8f7: u8,
}

/// Explicit producer document. Names/raw diagnostics/provenance must be removed
/// by a separately researched projection before gameplay checkpoint comparison.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDocument {
    pub schema_version: String,
    pub label: String,
    pub origin: ViewOrigin,
    pub cells: Vec<ViewCell>,
    pub units: Vec<ViewUnit>,
    pub lp_candidates_by_raw_group: [u16; 2],
    pub unlinked_candidates: Vec<UnlinkedCandidate>,
    pub raw_candidate_fields: Option<RawCandidateFields>,
}

/// Fields cannot be mutated without passing through validation again.
#[derive(Clone, Debug)]
pub struct BoardSnapshot {
    document: SnapshotDocument,
}
impl BoardSnapshot {
    pub fn new(mut document: SnapshotDocument) -> Result<Self> {
        if document.schema_version != SNAPSHOT_SCHEMA {
            return Err(invalid("Unsupported board-view schema."));
        }
        check_text(&document.label, 120, "snapshot label")?;
        match &document.origin {
            ViewOrigin::Synthetic { description } => {
                check_text(description, 500, "synthetic description")?
            }
            ViewOrigin::SavedCapture {
                capture_json_sha256,
                state_sha256,
                ee_memory_sha256,
                address_profile,
            } => {
                for hash in [capture_json_sha256, state_sha256, ee_memory_sha256] {
                    check_hash(hash)?;
                }
                if address_profile != roses_disc::PAL_PROFILE_ID {
                    return Err(invalid("Unsupported candidate address profile."));
                }
            }
            ViewOrigin::NativeSimulation {
                source,
                engine_version,
                rules_profile,
                request_sha256,
                checkpoint,
            } => {
                for hash in [
                    &source.capture_json_sha256,
                    &source.state_sha256,
                    &source.ee_memory_sha256,
                    request_sha256,
                ] {
                    check_hash(hash)?;
                }
                if source.address_profile != roses_disc::PAL_PROFILE_ID {
                    return Err(invalid("Unsupported initialization address profile."));
                }
                check_text(engine_version, 128, "native engine version")?;
                if rules_profile != NATIVE_RULES_PROFILE {
                    return Err(invalid("Unsupported native rules profile."));
                }
                if !matches!(checkpoint.as_str(), "before" | "after") {
                    return Err(invalid("Native checkpoint must be before or after."));
                }
                if document.raw_candidate_fields.is_some()
                    || !document.unlinked_candidates.is_empty()
                    || document
                        .units
                        .iter()
                        .any(|unit| unit.raw_location.is_some())
                {
                    return Err(invalid(
                        "Native snapshots must omit original raw fields, array locations and unlinked diagnostics.",
                    ));
                }
            }
        }
        if document.cells.len() != CELL_COUNT {
            return Err(invalid("Snapshot must explicitly contain all 49 cells."));
        }
        if document.units.len() > CELL_COUNT || document.unlinked_candidates.len() > 33 {
            return Err(invalid("Snapshot exceeds the bounded unit count."));
        }
        if document
            .lp_candidates_by_raw_group
            .iter()
            .any(|value| *value > 9999)
        {
            return Err(invalid(
                "LP candidate is outside the supported observed range.",
            ));
        }
        let mut positions = BTreeSet::new();
        let mut units = BTreeMap::new();
        for unit in &document.units {
            unit.position.index()?;
            if usize::from(unit.card_id) >= roses_assets::CARD_COUNT {
                return Err(invalid(format!(
                    "Unit {} has an unsupported card ID.",
                    unit.key
                )));
            }
            if !positions.insert(unit.position) || units.insert(unit.key, unit).is_some() {
                return Err(invalid("Duplicate unit key or occupied coordinate."));
            }
            if let Some(location) = unit.raw_location {
                check_raw_location(location)?;
            }
            if let Some(card) = &unit.card {
                check_text(&card.name, 256, "card name")?;
                if card.level > 12 || card.attack > 8191 || card.defense > 8191 {
                    return Err(invalid(
                        "Card metadata exceeds the supported numeric range.",
                    ));
                }
            }
        }
        let mut seen_cells = BTreeSet::new();
        let mut linked = BTreeSet::new();
        for cell in &document.cells {
            cell.position.index()?;
            if !seen_cells.insert(cell.position) {
                return Err(invalid(
                    "Duplicate cell coordinate; a board position is missing.",
                ));
            }
            if cell.terrain_code > 9 {
                return Err(invalid("Unsupported raw terrain code (expected 0..9)."));
            }
            if let Occupancy::Occupied { unit_key } = cell.occupancy {
                let unit = units
                    .get(&unit_key)
                    .ok_or_else(|| invalid(format!("Cell references missing unit {unit_key}.")))?;
                if unit.position != cell.position || !linked.insert(unit_key) {
                    return Err(invalid(
                        "Cell/unit occupancy is not reciprocal or a unit has duplicate cell links.",
                    ));
                }
            }
        }
        if linked.len() != units.len() {
            return Err(invalid("A board unit has no reciprocal occupied cell."));
        }
        let mut raw_locations = BTreeSet::new();
        for unit in &document.units {
            if let Some(location) = unit.raw_location {
                if !raw_locations.insert((location.array_group, location.array_slot)) {
                    return Err(invalid("Duplicate linked raw array location."));
                }
            }
        }
        for candidate in &document.unlinked_candidates {
            check_raw_location(candidate.raw_location)?;
            if !raw_locations.insert((
                candidate.raw_location.array_group,
                candidate.raw_location.array_slot,
            )) {
                return Err(invalid(
                    "Raw record is duplicated between linked/unlinked candidates.",
                ));
            }
            if candidate.column_raw > 7
                || candidate.row_raw > 7
                || candidate.side_field_raw > 2
                || candidate.slot_field_raw > 11
            {
                return Err(invalid(
                    "Unlinked candidate contains out-of-range raw coordinate/identity fields.",
                ));
            }
            capture::validate_card_pointer(candidate.card_id, candidate.constant_info_pointer)?;
        }
        document
            .cells
            .sort_by_key(|cell| (cell.position.row, cell.position.column));
        document.units.sort_by_key(|unit| unit.key);
        Ok(Self { document })
    }
    pub fn document(&self) -> &SnapshotDocument {
        &self.document
    }
    pub fn cell(&self, position: GridPos) -> Option<&ViewCell> {
        position
            .index()
            .ok()
            .and_then(|index| self.document.cells.get(index))
    }
    pub fn unit(&self, key: u16) -> Option<&ViewUnit> {
        self.document.units.iter().find(|unit| unit.key == key)
    }
    pub fn is_synthetic(&self) -> bool {
        matches!(self.document.origin, ViewOrigin::Synthetic { .. })
    }

    /// Optional local labels/statistics, joined by card ID through the existing
    /// importer cache loader. This cannot add units or change board coordinates.
    pub fn with_library(mut self, library: &roses_assets::library::Library) -> Result<Self> {
        for unit in &mut self.document.units {
            let card = library
                .cards
                .iter()
                .find(|card| card.id == unit.card_id)
                .ok_or_else(|| invalid(format!("Library is missing card {}.", unit.card_id)))?;
            unit.card = Some(CardMetadata {
                name: card.name.clone(),
                attack: card.attack,
                defense: card.defense,
                level: card.level,
                deck_cost: card.deck_cost,
            });
        }
        Self::new(self.document)
    }
}

fn check_raw_location(location: RawUnitLocation) -> Result<()> {
    if location.array_group > 2 || location.array_slot > 10 {
        return Err(invalid(
            "Raw array location exceeds 3 groups of 11 records.",
        ));
    }
    Ok(())
}
fn check_text(text: &str, limit: usize, label: &str) -> Result<()> {
    if text.trim().is_empty() || text.chars().count() > limit || text.chars().any(char::is_control)
    {
        return Err(invalid(format!("Invalid {label}.")));
    }
    Ok(())
}
pub(crate) fn check_hash(hash: &str) -> Result<()> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(
            "SHA256 metadata must contain exactly 64 hex digits.",
        ));
    }
    Ok(())
}

pub fn from_snapshot_json(bytes: &[u8]) -> Result<BoardSnapshot> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(invalid("Snapshot JSON exceeds the 1 MiB input limit."));
    }
    let document = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("Invalid snapshot JSON: {error}")))?;
    BoardSnapshot::new(document)
}
pub fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)
        .map_err(|error| invalid(format!("Cannot open {}: {error}", path.display())))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(invalid("Viewer input exceeds the 1 MiB limit."));
    }
    Ok(bytes)
}
pub fn demo() -> BoardSnapshot {
    from_snapshot_json(include_bytes!("../../../examples/viewer/demo.json"))
        .expect("checked synthetic demo fixture")
}

/// Screen geometry only; increasing raw rows are drawn down, columns right.
/// This convention does not establish the original camera orientation.
#[derive(Clone, Copy, Debug)]
pub struct BoardLayout {
    pub left: i32,
    pub top: i32,
    pub tile: i32,
}
impl BoardLayout {
    pub fn hit(self, x: i32, y: i32) -> Option<GridPos> {
        if self.tile <= 0 || x < self.left || y < self.top {
            return None;
        }
        let column = (i64::from(x) - i64::from(self.left)) / i64::from(self.tile);
        let row = (i64::from(y) - i64::from(self.top)) / i64::from(self.tile);
        if column >= 7 || row >= 7 {
            None
        } else {
            Some(GridPos {
                column: column as u8,
                row: row as u8,
            })
        }
    }
}

#[cfg(test)]
mod tests;
