//! A narrow deterministic board transition model, not a complete game rules engine.
//!
//! The caller must explicitly supply an isolated movement context. This model
//! does not discover whose turn it is, choose an eligible unit, or rule out card
//! effects. Its supported operation consumes one caller-supplied permission.

use serde::{Deserialize, Serialize};

pub mod card_block;
pub mod deck_editor;
pub mod duel;
pub mod duel_end;
pub mod memory_card;
pub mod practice;
pub mod slot_machine;
use std::collections::BTreeSet;
use std::fmt;

pub const STATE_SCHEMA: &str = "roses-core-state-1";
pub const BOARD_SIDE: u8 = 7;
pub const CELL_COUNT: usize = 49;
pub const CARD_COUNT: u16 = 854;
pub const NORMAL_TERRAIN: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnitId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridPos {
    pub column: u8,
    pub row: u8,
}

impl GridPos {
    pub fn index(self) -> Option<usize> {
        (self.column < BOARD_SIDE && self.row < BOARD_SIDE)
            .then_some(usize::from(self.row) * usize::from(BOARD_SIDE) + usize::from(self.column))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    /// Identifies an instance, not a card type. Distinct units may share a card ID.
    pub id: UnitId,
    pub card_id: u16,
    /// Preserved caller-supplied side value; never used to infer authority or turns.
    pub side: u8,
    pub position: GridPos,
}

/// Unvalidated transport type. Construct `CoreState` before using it as a state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateDocument {
    pub schema_version: String,
    /// Exactly 49 raw terrain codes in row-major order.
    pub terrain: Vec<u8>,
    pub units: Vec<Unit>,
    pub life_points: [u16; 2],
}

/// Immutable, canonical state. Both construction and deserialization validate it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StateDocument", into = "StateDocument")]
pub struct CoreState {
    document: StateDocument,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    UnsupportedSchema,
    TerrainCount,
    TerrainCode { index: usize, value: u8 },
    TooManyUnits,
    DuplicateUnitId(UnitId),
    DuplicatePosition(GridPos),
    InvalidPosition { unit: UnitId, position: GridPos },
    InvalidCardId { unit: UnitId, card_id: u16 },
    InvalidSide { unit: UnitId, side: u8 },
    InvalidLifePoints { side: usize, value: u16 },
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid core state: {self:?}")
    }
}
impl std::error::Error for StateError {}

impl CoreState {
    pub fn new(mut document: StateDocument) -> Result<Self, StateError> {
        if document.schema_version != STATE_SCHEMA {
            return Err(StateError::UnsupportedSchema);
        }
        if document.terrain.len() != CELL_COUNT {
            return Err(StateError::TerrainCount);
        }
        for (index, &value) in document.terrain.iter().enumerate() {
            if value > 9 {
                return Err(StateError::TerrainCode { index, value });
            }
        }
        if document.units.len() > CELL_COUNT {
            return Err(StateError::TooManyUnits);
        }
        let mut identities = BTreeSet::new();
        let mut positions = BTreeSet::new();
        for unit in &document.units {
            if unit.position.index().is_none() {
                return Err(StateError::InvalidPosition {
                    unit: unit.id,
                    position: unit.position,
                });
            }
            if unit.card_id >= CARD_COUNT {
                return Err(StateError::InvalidCardId {
                    unit: unit.id,
                    card_id: unit.card_id,
                });
            }
            if unit.side > 2 {
                return Err(StateError::InvalidSide {
                    unit: unit.id,
                    side: unit.side,
                });
            }
            if !identities.insert(unit.id) {
                return Err(StateError::DuplicateUnitId(unit.id));
            }
            if !positions.insert(unit.position) {
                return Err(StateError::DuplicatePosition(unit.position));
            }
        }
        for (side, &value) in document.life_points.iter().enumerate() {
            if value > 9999 {
                return Err(StateError::InvalidLifePoints { side, value });
            }
        }
        document.units.sort_by_key(|unit| unit.id);
        Ok(Self { document })
    }

    pub fn document(&self) -> &StateDocument {
        &self.document
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.document.units.iter().find(|unit| unit.id == id)
    }

    pub fn unit_at(&self, position: GridPos) -> Option<&Unit> {
        self.document
            .units
            .iter()
            .find(|unit| unit.position == position)
    }

    /// Computes a single isolated board move from the command. Neither this
    /// method nor its context accepts an expected endpoint or a reference trace.
    /// Both inputs remain unchanged on success and on error.
    pub fn apply(
        &self,
        command: &Command,
        context: Option<&MoveContext>,
    ) -> Result<Transition, ActionError> {
        let Some(MoveContext::Restricted {
            eligible_unit,
            direction,
            permission,
        }) = context
        else {
            return Err(ActionError::UnsupportedContext);
        };
        if *permission == MovePermission::Consumed {
            return Err(ActionError::PermissionConsumed);
        }
        let Command::MoveUnit { unit, destination } = *command;
        if unit != *eligible_unit {
            return Err(ActionError::UnitNotEligible { unit });
        }
        let mover = self.unit(unit).ok_or(ActionError::UnknownUnit { unit })?;
        let destination_index = destination
            .index()
            .ok_or(ActionError::OutOfBounds { destination })?;
        let source = mover.position;
        if source.column.abs_diff(destination.column) + source.row.abs_diff(destination.row) != 1 {
            return Err(ActionError::NotOneCardinalStep);
        }
        if !direction.matches(source, destination) {
            return Err(ActionError::DirectionNotPermitted);
        }
        if let Some(occupant) = self.unit_at(destination) {
            return Err(ActionError::DestinationOccupied {
                occupant: occupant.id,
            });
        }
        // Source coordinates are valid by CoreState's construction invariant.
        let source_index = source.index().expect("validated core position");
        if self.document.terrain[source_index] != NORMAL_TERRAIN
            || self.document.terrain[destination_index] != NORMAL_TERRAIN
        {
            return Err(ActionError::UnsupportedTerrain);
        }
        let mut state = self.clone();
        state
            .document
            .units
            .iter_mut()
            .find(|mover| mover.id == unit)
            .expect("the selected mover was validated above")
            .position = destination;
        Ok(Transition {
            state,
            context: MoveContext::Restricted {
                eligible_unit: *eligible_unit,
                direction: *direction,
                permission: MovePermission::Consumed,
            },
            events: vec![Event::UnitMoved {
                unit,
                from: source,
                to: destination,
            }],
        })
    }
}

impl TryFrom<StateDocument> for CoreState {
    type Error = StateError;
    fn try_from(value: StateDocument) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<CoreState> for StateDocument {
    fn from(value: CoreState) -> Self {
        value.document
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    MoveUnit { unit: UnitId, destination: GridPos },
}

/// Raw grid directions, not claims about the original game's camera controls.
/// Supporting these candidate policies does not establish original-game legality
/// for every direction. A caller explicitly chooses the one being exercised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardinalDirection {
    DecreasingRow,
    IncreasingRow,
    DecreasingColumn,
    IncreasingColumn,
}
impl CardinalDirection {
    fn matches(self, from: GridPos, to: GridPos) -> bool {
        match self {
            Self::DecreasingRow => to.row < from.row && to.column == from.column,
            Self::IncreasingRow => to.row > from.row && to.column == from.column,
            Self::DecreasingColumn => to.column < from.column && to.row == from.row,
            Self::IncreasingColumn => to.column > from.column && to.row == from.row,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovePermission {
    Available,
    Consumed,
}

/// Caller assertion that no battle, terrain interaction, card effect, turn
/// change, or other rule needs evaluation for this isolated operation. This is
/// deliberately not inferred from side, card ID, or a guessed emulator phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MoveContext {
    Unknown,
    Restricted {
        eligible_unit: UnitId,
        direction: CardinalDirection,
        /// A model token, not a researched once-per-turn counter.
        permission: MovePermission,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    UnitMoved {
        unit: UnitId,
        from: GridPos,
        to: GridPos,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub state: CoreState,
    pub context: MoveContext,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionError {
    UnsupportedContext,
    PermissionConsumed,
    UnitNotEligible { unit: UnitId },
    UnknownUnit { unit: UnitId },
    OutOfBounds { destination: GridPos },
    NotOneCardinalStep,
    DirectionNotPermitted,
    DestinationOccupied { occupant: UnitId },
    UnsupportedTerrain,
}
impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move not applied: {self:?}")
    }
}
impl std::error::Error for ActionError {}

#[cfg(test)]
mod tests;
