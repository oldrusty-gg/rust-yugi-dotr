//! Computer opponents, traced from the PAL executable (research in progress).
//!
//! The original opponents play through the same pad input as a person: the
//! duel loop asks 0x1CFF80 for a side's pad word, which calls the opponent's
//! driver from table 0x2BA230 (Weevil: control byte 2 → 0x1117D0). When its
//! key queue is empty the driver plans one [`Action`] and 0x277BD0 turns it
//! into pad words; one word is returned per call. This module keeps that
//! split: decisions produce actions, and [`keys`] reproduces the presses so a
//! frontend can animate the cursor with the original timing.
//!
//! Only the key emission is implemented. The per-opponent planners are not,
//! so no computer turn can be played yet.

pub mod weevil;

use super::{terrain, CardData, Duel};
use crate::{GridPos, UnitId};
use serde::{Deserialize, Serialize};

/// Pad words (active-high, the game's accumulator layout).
pub mod pad {
    pub const L2: u16 = 0x0001;
    pub const R2: u16 = 0x0002;
    pub const L1: u16 = 0x0004;
    pub const R1: u16 = 0x0008;
    pub const TRIANGLE: u16 = 0x0010;
    pub const CIRCLE: u16 = 0x0020;
    pub const CROSS: u16 = 0x0040;
    pub const SQUARE: u16 = 0x0080;
    pub const SELECT: u16 = 0x0100;
    /// L3 / R3 (stick presses; the camera reset in 0x18).
    pub const L3: u16 = 0x0200;
    pub const R3: u16 = 0x0400;
    pub const START: u16 = 0x0800;
    pub const UP: u16 = 0x1000;
    pub const RIGHT: u16 = 0x2000;
    pub const DOWN: u16 = 0x4000;
    pub const LEFT: u16 = 0x8000;
}

/// Position request of a type-6 action (+0x10), used when the unit stays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    /// 0: leave committed defence (L1).
    Attack,
    /// 1: enter defence (R1).
    Defence,
    /// 2: no position key.
    Keep,
}

/// One planned action (the 0x277BD0 action record).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Type 0xA: Square at the leader, walk to `cell`, pick hand cards by
    /// index (marked with Up; several indices form a fusion list).
    Summon { cell: GridPos, hand: Vec<usize> },
    /// Type 6: select `unit`, optionally reveal it (R2) or change position
    /// in place (`target` = own cell; neither applies to leaders), then walk
    /// the cursor to `target` (a move or an attack) and confirm.
    Act {
        unit: UnitId,
        target: GridPos,
        reveal: bool,
        position: Position,
    },
    /// Types 0xC/0xD (0x2782F0 / 0x2783B4): select the set card on `at`,
    /// reveal it (R2) and confirm in place, then answer the selection its
    /// activation opens. Mimicat (0xC) first switches to `graveyard` when
    /// it is not the active side's (the menu starts on that one; Left when
    /// side 1 acts, Right otherwise, 0x278330) and presses Down `down`
    /// times (+4 & 0xF); Change of Heart (0xD) presses Down `down` times
    /// (+4, the whole word); both end with Cross.
    Activate {
        unit: UnitId,
        at: GridPos,
        graveyard: Option<u8>,
        down: u16,
    },
}

/// Column step keys for `side` (the game mirrors the d-pad for side 1).
fn column_key(side: u8, decrease: bool) -> u16 {
    if decrease == (side == 0) {
        pad::LEFT
    } else {
        pad::RIGHT
    }
}

fn row_key(side: u8, decrease: bool) -> u16 {
    if decrease == (side == 0) {
        pad::UP
    } else {
        pad::DOWN
    }
}

fn walk(keys: &mut Vec<u16>, side: u8, from: GridPos, to: GridPos, columns_first: bool) {
    let dc = i32::from(from.column) - i32::from(to.column);
    let dr = i32::from(from.row) - i32::from(to.row);
    let columns = std::iter::repeat_n(column_key(side, dc > 0), dc.unsigned_abs() as usize);
    let rows = std::iter::repeat_n(row_key(side, dr > 0), dr.unsigned_abs() as usize);
    if columns_first {
        keys.extend(columns.chain(rows));
    } else {
        keys.extend(rows.chain(columns));
    }
}

/// 0x2784A0: walk columns first unless the target is an L-shaped two-step
/// away with |Δcolumn| = 1 and the column-first corner is not passable
/// (0x27B200 one-step rule: Labyrinth, a Crush cell that would kill the
/// unit, or any occupant other than the target).
fn columns_first(
    duel: &Duel,
    unit: UnitId,
    from: GridPos,
    to: GridPos,
    cards: &impl CardData,
) -> bool {
    let dc = from.column.abs_diff(to.column);
    let dr = from.row.abs_diff(to.row);
    if dc != 1 || dc + dr != 2 {
        return true;
    }
    let corner = GridPos {
        column: to.column,
        row: from.row,
    };
    let target = duel.unit_at(to).map(|u| u.id);
    let occupant = duel.unit_at(corner).map(|u| u.id);
    match occupant {
        Some(id) => Some(id) == target,
        None => match duel.terrain_at(corner) {
            terrain::LABYRINTH => false,
            terrain::CRUSH => duel.unit(unit).is_none_or(|u| {
                let mut moved = u.clone();
                moved.position = corner;
                !duel.crushes(&moved, cards)
            }),
            _ => true,
        },
    }
}

fn toward(value: u8, target: u8) -> u8 {
    if value < target {
        value + 1
    } else {
        value - 1
    }
}

/// Cells a type-6 action walks through, in key order (for [`DuelCommand::Move`]).
///
/// [`DuelCommand::Move`]: super::DuelCommand::Move
pub fn path(duel: &Duel, unit: UnitId, target: GridPos, cards: &impl CardData) -> Vec<GridPos> {
    let Some(from) = duel.unit(unit).map(|u| u.position) else {
        return Vec::new();
    };
    let mut cells = Vec::new();
    let mut here = from;
    for columns in if columns_first(duel, unit, from, target, cards) {
        [true, false]
    } else {
        [false, true]
    } {
        if columns {
            while here.column != target.column {
                here.column = toward(here.column, target.column);
                cells.push(here);
            }
        } else {
            while here.row != target.row {
                here.row = toward(here.row, target.row);
                cells.push(here);
            }
        }
    }
    cells
}

/// 0x277BD0: the pad words for `action`, starting from `cursor`.
pub fn keys(
    duel: &Duel,
    side: u8,
    cursor: GridPos,
    action: &Action,
    cards: &impl CardData,
) -> Vec<u16> {
    let mut out = Vec::new();
    match action {
        Action::Summon { cell, hand } => {
            let leader = duel.leader(side).position;
            walk(&mut out, side, cursor, leader, true);
            out.push(pad::SQUARE);
            walk(&mut out, side, leader, *cell, true);
            out.push(pad::CROSS);
            // Hand scrolling is not mirrored: Right is always the next card.
            let mut at = 0usize;
            for &index in hand {
                let key = if index >= at { pad::RIGHT } else { pad::LEFT };
                out.extend(std::iter::repeat_n(key, index.abs_diff(at)));
                out.push(pad::UP);
                at = index;
            }
            out.extend([pad::CROSS, pad::CROSS]);
        }
        Action::Act {
            unit,
            target,
            reveal,
            position,
        } => {
            let Some(u) = duel.unit(*unit) else {
                return out;
            };
            walk(&mut out, side, cursor, u.position, true);
            out.push(pad::CROSS);
            // Leaders skip straight to the path keys (0x277DA4).
            if u.slot != 0 {
                if *reveal {
                    out.push(pad::R2);
                }
                if *target == u.position {
                    match position {
                        Position::Attack if u.committed_defence => out.push(pad::L1),
                        Position::Defence if !u.committed_defence => out.push(pad::R1),
                        _ => (),
                    }
                }
            }
            let first = columns_first(duel, *unit, u.position, *target, cards);
            walk(&mut out, side, u.position, *target, first);
            out.push(pad::CROSS);
        }
        Action::Activate {
            at,
            graveyard,
            down,
            ..
        } => {
            walk(&mut out, side, cursor, *at, true);
            out.extend([pad::CROSS, pad::R2, pad::CROSS]);
            if let Some(g) = graveyard {
                if *g != side {
                    out.push(if side != 0 { pad::LEFT } else { pad::RIGHT });
                }
            }
            out.extend(std::iter::repeat_n(pad::DOWN, usize::from(*down)));
            out.push(pad::CROSS);
        }
    }
    out
}

/// Where the cursor rests after `action` (the summon cell or the target;
/// for an activation the card's cell, although a Change of Heart leaves
/// it on the picked unit — the session reads the real cursor instead).
pub fn cursor_after(action: &Action) -> GridPos {
    match action {
        Action::Summon { cell, .. } => *cell,
        Action::Act { target, .. } => *target,
        Action::Activate { at, .. } => *at,
    }
}
