//! Pad-driven duel input: the interactive sub-states of the original duel
//! loop 0x1D00F0 (0x18 field cursor, 0x1D move selection, 0x3E summon cell,
//! 0x43 hand menu, 0x46 summon confirmation, 0x30 unit pick and 0x2D
//! graveyard pick after a magic activation), turning one press at a time
//! into engine commands. Human pads and the computer's key queues
//! ([`super::ai::keys`]) drive the same machine, as in the original, where
//! 0x1CFF80 feeds either into it (research: `claude-rules/movement-
//! verified.md`, `claude-effects/FUSION.md` §3, and the handlers at
//! 0x1D386C, 0x1D41F4, 0x1D6414, 0x1D6D70, 0x1D7230).
//!
//! Animation and wait states (0x13–0x17, 0x19, 0x1E, 0x36–0x3B, 0x3F–0x42,
//! 0x44, 0x45, 0x47–0x50) are not modelled: the caller applies the returned
//! commands and the machine continues in the state the original reaches
//! after them. The card details view (Triangle → 0x20 → 0x21, closed with
//! Circle) is modelled as a polled state without its contents, and so is the
//! surrender prompt (Select → 0x23 → 0x24 → 0x25, handlers 0x1D4DAC–
//! 0x1D4F34), the settings panel (L1 → 0x27 → 0x28 → 0x29, 0x1D4F38–
//! 0x1D5180), the graveyard view (R1 → 0x2B → 0x2C → 0x2D with d66 bit3
//! clear), the hand view (0x51 → 0x52 → 0x53, 0x1D8188–0x1D84A8) and the
//! L2 display toggle (d66 bit7, 0x1D3BCC). Camera sticks (and L3/R3) and
//! SFX are left to the presentation.

use super::ai::pad;
use super::{terrain, CardData, Duel, DuelCommand, DuelError, Pending, Pick, Result};
use crate::{GridPos, UnitId};
use serde::{Deserialize, Serialize};

/// Cell mark (cell +0x17) no search reached.
pub const UNMARKED: u8 = 0xFF;
/// Mark left over from an earlier search of the same selection.
pub const STALE: u8 = 0xFE;
const CELLS: usize = 49;

/// A unit being moved (state 0x1D).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub unit: UnitId,
    /// The d59 cells stepped from, origin first (0x2DB86A / 0x2DB89B).
    pub path: Vec<GridPos>,
    /// Step allowance d5a (0x247E10), recomputed when the face changes.
    pub allowance: u8,
    /// d66 bit6: may enter empty Labyrinth cells.
    pub wall: bool,
    /// Pending +0x57 bit2 / bit3 (face-up, defence).
    pub face_up: bool,
    pub defence: bool,
    /// Cell marks +0x17 (0x1E3A50), row-major.
    pub marks: Vec<u8>,
}

/// The hand menu (0x43): cursor d4F and the marked hand indices in marking
/// order (d4E = their count).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandMenu {
    pub cursor: usize,
    pub marks: Vec<usize>,
}

/// The unit selection of state 0x30 (0x1D5AD8), opened by a Change of
/// Heart ([`Pending::ControlChange`]): state 0x2F (0x1D58B8) lists the
/// positions of the opposing group's slots 1..10 at 0x1ED0000 and parks
/// the cursor on the first; the d-pad walks the list with wrap-around
/// (0x1D5B04: Up/Right/L1 back, Down/Left/R1 forward, raw pad bits, no
/// side mirroring), Cross or Square confirms the unit under the cursor
/// (state 0x31, c4c = group << 8 | slot), Circle cancels (c4c = −1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitPick {
    pub units: Vec<UnitId>,
    /// d5e: the list index under the cursor.
    pub index: usize,
}

/// The graveyard pick of state 0x2D (0x1D5348), opened by Mimicat or
/// Monster Reborn ([`Pending::GraveyardCopy`]): state 0x2B (0x1D5184)
/// lists each side's eligible graveyard entries (0x1ED0000 / 0x1ED00F0);
/// d36 holds the shown list (bit 7: locked to one list, from the builder's
/// byte 1, 0x1D2604), d5e/d5f the cursor row inside the five-row window of
/// each list and c8a/c8e their scroll offsets. Up/Down move the row, then
/// the offset (no wrap); L1/R1 move the offset by five; Right shows side
/// 1's list and Left side 0's (absolute, the row carried over and clamped);
/// Cross/Square confirm list[offset + row] (state 0x2E, c4c = side << 16 |
/// card; error sound on an empty list), Circle cancels (c4c = 999). Raw pad
/// bits for every key (0x1D365C: the repeat word is unmirrored).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraveyardPick {
    /// Graveyard indices per side, oldest first (0x1E0080 order).
    /// Per side (two in a 2-player duel).
    pub lists: Vec<Vec<usize>>,
    pub shown: u8,
    pub locked: bool,
    pub row: Vec<u8>,
    pub scroll: Vec<u8>,
}

impl GraveyardPick {
    /// The graveyard index under the cursor of the shown list.
    pub fn selected(&self) -> Option<usize> {
        let i = usize::from(self.shown);
        self.lists[i]
            .get(usize::from(self.scroll[i]) + usize::from(self.row[i]))
            .copied()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// 0x18.
    Field,
    /// 0x1D.
    Move(Selection),
    /// 0x3E; `leader` is path[0], where Circle returns the cursor.
    SummonCell { leader: GridPos, menu: HandMenu },
    /// 0x43, after the summon was opened (and the hand refilled).
    Hand(HandMenu),
    /// 0x46, after the 0x44/0x45 preview.
    Confirm(HandMenu),
    /// 0x30, while [`Duel::pending`] is a control change.
    SelectUnit(UnitPick),
    /// 0x2D, while [`Duel::pending`] is a graveyard copy.
    Graveyard(GraveyardPick),
}

/// The card details view, state 0x21 (after the 0x20 transition), opened
/// with Triangle on the unit under the cursor in 0x18 and closed with
/// Circle, cursor unchanged. It overlays [`Mode::Field`] rather than
/// replacing it. Up/Down change `page` [H: what Down does inside the view
/// is not traced, TUTORIAL.md §7].
///
/// [F 0x1D4C04–0x1D4DA8] 0x20 slides the window in (d3a += 8 to 0x40),
/// 0x21 polls: Up/Down cycle the four pages (cbd − 1 / + 1 mod 4: basic,
/// special info, power-up cards, leader ability; SE 0x13); with the hand
/// menu (d35 = 0x43) or the hand view (0x53) under it, Left/Right move the
/// hand cursor d4F with wrap-around and show that card (SE 0x13); Circle
/// → 0x22 (SE 0x11), which slides out and returns to d35, the state it was
/// opened from (0x18, 0x43, 0x53 or 0x2D).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailsView {
    /// The field unit shown (`None` for a hand or graveyard card).
    pub unit: Option<UnitId>,
    /// The card shown.
    #[serde(default)]
    pub card: u16,
    /// cbd: 0..=3.
    pub page: u8,
}

/// Pages of the details view (cbd wraps mod 4).
pub const DETAILS_PAGES: u8 = 4;

/// The duel settings panel, state 0x29 (L1 in 0x18 → 0x27, which fills d36
/// bit i from `0x214490(0, i)` for i = 0..2 and sets d35 = 0; 0x28 slides it
/// in over 8 frames). Up/Down toggle the row d35 between 0 and 1 (SE 3);
/// Left sets and Right clears d36 bit (row + 1) (SE 3 on a change): the
/// panel offers options 1 and 2 only (bit 0, the card-name notation, is not
/// a row in the duel). Cross/Square write bits 0..2 to both sides' card
/// blocks (0x214340, +0xFF9C bits 2..4) with SE 4, Circle closes with SE 5
/// and writes nothing; both through 0x2A (8 frames) back to 0x18 [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsPanel {
    /// d35.
    pub row: u8,
    /// d36: option bits as edited.
    pub bits: u8,
}

/// The hand view, state 0x53: Square on the leader after the turn's summon
/// with cards in hand (0x18 → 0x51, which refills the hand display with
/// draw = 0, 0x1D8F90; 0x52 fades it in). Left/Right move the hand cursor
/// d4F with wrap-around (SE 3), Triangle opens the details of that card
/// (return state 0x53), Circle fades out (0x52) and returns through 0x16 to
/// 0x18 [F 0x1D8188–0x1D84A4].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandView {
    /// d4F.
    pub cursor: usize,
}

/// Option bits as the card block stores them in +0xFF9C bits 2..4 (bit i
/// here = bit i + 2 there): 0 card-name notation (the tutorial's text
/// variant), 1 monster battle, 2 sound [F 0x214340/0x214490; meaning of
/// bits 1/2 from the panel's rows, OTHER-MODES-AUDIT.md §3a].
pub const DEFAULT_OPTIONS: u8 = 0b111;

fn default_options() -> u8 {
    DEFAULT_OPTIONS
}

/// The surrender prompt, state 0x25 (Select in 0x18 → 0x23 sets D35 = 0,
/// 0x24 slides the window in over 8 frames, neither polled; 0x1D4DAC–
/// 0x1D4DFC). In 0x25 (0x1D4E00) Left sets D35 = 1 ("yes") and Right 0
/// ("no", SFX 3 on a change); Cross/Square confirm (SFX 4) and Circle
/// cancels with D35 = 0 (SFX 5), both through 0x26, which slides the window
/// out and either ends the duel (D35 = 1: the active side surrenders,
/// 0x1D4ED0) or returns to 0x18. It overlays [`Mode::Field`] like the
/// details view [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurrenderPrompt {
    /// D35: "yes" highlighted.
    pub yes: bool,
}

impl Mode {
    /// The original sub-state (0x2DB8F7) this mode stands for.
    pub fn state(&self) -> u8 {
        match self {
            Mode::Field => 0x18,
            Mode::Move(_) => 0x1D,
            Mode::SummonCell { .. } => 0x3E,
            Mode::Hand(_) => 0x43,
            Mode::Confirm(_) => 0x46,
            Mode::SelectUnit(_) => 0x30,
            Mode::Graveyard(_) => 0x2D,
        }
    }
}

/// What a press did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    /// Nothing handled the press (a masked step, no button of this state).
    Ignored,
    /// The cursor, a selection or a menu changed.
    Moved,
    /// The original's error sound 6: the press is refused.
    Refused,
    /// Apply these to the engine, in order, before the next press.
    Commands(Vec<DuelCommand>),
    /// A view or action this port does not model; nothing changed.
    Unsupported(&'static str),
}

/// Input state of both sides: each side's cursor (side +0x33D/+0x33E) and
/// the active side's mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelUi {
    /// Per side (two in a 2-player duel).
    pub cursors: Vec<GridPos>,
    pub mode: Mode,
    /// The details view open over [`Mode::Field`] (0x21).
    #[serde(default)]
    pub details: Option<DetailsView>,
    /// The surrender prompt open over [`Mode::Field`] (0x25).
    #[serde(default)]
    pub surrender: Option<SurrenderPrompt>,
    /// The settings panel open over [`Mode::Field`] (0x29).
    #[serde(default)]
    pub settings: Option<SettingsPanel>,
    /// The hand view open over [`Mode::Field`] (0x53).
    #[serde(default)]
    pub hand_view: Option<HandView>,
    /// d66 bit3 clear while [`Mode::Graveyard`] is open: R1's view-only
    /// graveyard lists (Cross does nothing, Circle closes).
    #[serde(default)]
    pub graveyard_view: bool,
    /// d66 bit7 (mirrored to d67 bit0): L2 switched the info windows off.
    #[serde(default)]
    pub hud_hidden: bool,
    /// The option bits the settings panel reads and writes
    /// ([`DEFAULT_OPTIONS`]; the caller seeds them from the card block).
    #[serde(default = "default_options")]
    pub options: u8,
    /// d64 bit6: a held diagonal alternates between its two axes.
    alternate: bool,
}

impl DuelUi {
    /// Cursors start on the leaders.
    pub fn new(duel: &Duel) -> Self {
        let cursor = |side| duel.leader_cell(side).unwrap_or(GridPos { column: 3, row: 3 });
        Self {
            cursors: (0..duel.side_count()).map(cursor).collect(),
            mode: Mode::Field,
            details: None,
            surrender: None,
            settings: None,
            hand_view: None,
            graveyard_view: false,
            hud_hidden: false,
            options: DEFAULT_OPTIONS,
            alternate: false,
        }
    }

    pub fn cursor(&self, duel: &Duel) -> GridPos {
        self.cursors[usize::from(duel.active)]
    }

    /// The original sub-state (0x2DB8F7): the mode's, 0x21 while the
    /// details view is open, 0x25 while the surrender prompt is, 0x29 for
    /// the settings panel, 0x53 for the hand view.
    pub fn state(&self) -> u8 {
        if self.details.is_some() {
            0x21
        } else if self.surrender.is_some() {
            0x25
        } else if self.settings.is_some() {
            0x29
        } else if self.hand_view.is_some() {
            0x53
        } else {
            self.mode.state()
        }
    }

    /// One press (or auto-repeat) of the active side's pad. `keys` holds
    /// original pad bits ([`pad`]); side 1's board d-pad is mirrored, its
    /// hand menu is not.
    pub fn press(&mut self, duel: &Duel, cards: &impl CardData, keys: u16) -> Result<Response> {
        if duel.outcome.is_some() || duel.pending_summon.is_some() != self.summon_open() {
            return Err(DuelError::InvalidCommand("input does not match the duel"));
        }
        let side = duel.active;
        self.open_selection(duel, cards)?;
        let board_keys = board_keys(duel, side, keys);
        if let Some(view) = self.details {
            return Ok(self.details(duel, keys, board_keys, view));
        }
        if let Some(prompt) = self.surrender {
            return Ok(self.surrender_prompt(board_keys, prompt));
        }
        if let Some(panel) = self.settings {
            return Ok(self.settings_panel(board_keys, panel));
        }
        if let Some(view) = self.hand_view {
            return Ok(self.hand_view(duel, board_keys, view));
        }
        match self.mode.clone() {
            Mode::Field => self.field(duel, cards, board_keys),
            Mode::Move(sel) => self.moving(duel, cards, board_keys, sel),
            Mode::SummonCell { leader, menu } => self.summon_cell(duel, board_keys, leader, menu),
            Mode::Hand(menu) => self.hand(duel, cards, keys, menu),
            Mode::Confirm(menu) => Ok(self.confirm(keys, menu)),
            // Both menus read the raw pad word (0x1D5AF8 / 0x1D365C).
            Mode::SelectUnit(pick) => self.select_unit(duel, keys, pick),
            Mode::Graveyard(pick) => Ok(self.graveyard(duel, keys, pick)),
        }
    }

    /// The transition out of 0x39 when the builder returned 2: into 0x2F
    /// (0x1D58B8, a unit list) or 0x2B (0x1D5184, the graveyard lists). The
    /// engine opens the selection as part of the activation command, so the
    /// machine catches up before the next press; a selection mode without
    /// a pending selection is a mismatch like a stray summon menu.
    fn open_selection(&mut self, duel: &Duel, cards: &impl CardData) -> Result<()> {
        let side = duel.active;
        match (&duel.pending, &self.mode) {
            (Some(Pending::ControlChange { units, .. }), mode)
                if !matches!(mode, Mode::SelectUnit(_)) =>
            {
                let first = units
                    .first()
                    .and_then(|&u| duel.unit(u))
                    .ok_or(DuelError::InvalidCommand("an empty unit selection"))?;
                // 0x1D5A2C: the cursor jumps to the first entry.
                self.cursors[usize::from(side)] = first.position;
                self.details = None;
                self.surrender = None;
                self.mode = Mode::SelectUnit(UnitPick {
                    units: units.clone(),
                    index: 0,
                });
            }
            (
                Some(Pending::GraveyardCopy {
                    graveyards,
                    monsters,
                    non_monsters,
                    ..
                }),
                mode,
            ) if !matches!(mode, Mode::Graveyard(_)) || self.graveyard_view => {
                let list = |g: u8| -> Vec<usize> {
                    if !graveyards.contains(&g) {
                        return Vec::new();
                    }
                    duel.sides[usize::from(g)]
                        .graveyard
                        .iter()
                        .enumerate()
                        .filter(|(_, &c)| {
                            (!*monsters || cards.card(c).is_some_and(|c| c.is_monster()))
                                // 0x251920 class code 2: card id >= 0x2AB.
                                && (!*non_monsters || c >= 0x2AB)
                        })
                        .map(|(i, _)| i)
                        .collect()
                };
                // 0x1D2610: d36 = the builder's graveyard nibble | 0x80 when
                // it names one side, else the active side (both lists).
                let locked = graveyards.len() == 1;
                let shown = if locked { graveyards[0] } else { side };
                self.details = None;
                self.surrender = None;
                self.graveyard_view = false;
                self.mode = Mode::Graveyard(GraveyardPick {
                    lists: (0..duel.side_count()).map(list).collect(),
                    shown,
                    locked,
                    row: vec![0; duel.sides.len()],
                    scroll: vec![0; duel.sides.len()],
                });
            }
            (None, Mode::Graveyard(_)) if self.graveyard_view => (),
            (None, Mode::SelectUnit(_) | Mode::Graveyard(_)) => {
                return Err(DuelError::InvalidCommand("input does not match the duel"));
            }
            _ => (),
        }
        Ok(())
    }

    /// State 0x30 (0x1D5AD8).
    fn select_unit(&mut self, duel: &Duel, keys: u16, mut pick: UnitPick) -> Result<Response> {
        let side = duel.active;
        const BACK: u16 = pad::UP | pad::RIGHT | pad::L1;
        const FORWARD: u16 = pad::DOWN | pad::LEFT | pad::R1;
        if keys & (BACK | FORWARD) != 0 {
            let n = pick.units.len();
            pick.index = if keys & BACK != 0 {
                (pick.index + n - 1) % n
            } else {
                (pick.index + 1) % n
            };
            let unit = duel
                .unit(pick.units[pick.index])
                .ok_or(DuelError::InvalidCommand("a listed unit is gone"))?;
            self.cursors[usize::from(side)] = unit.position;
            self.mode = Mode::SelectUnit(pick);
            return Ok(Response::Moved);
        }
        // 0x1D5C30: no unit under the cursor, nothing happens.
        let Some(here) = duel.unit_at(self.cursors[usize::from(side)]) else {
            return Ok(Response::Ignored);
        };
        if keys & pad::TRIANGLE != 0 {
            return Ok(Response::Unsupported(
                "card details from a selection (0x20)",
            ));
        }
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            self.return_to_field();
            return Ok(Response::Commands(vec![DuelCommand::Pick {
                pick: Pick::Unit { unit: here.id },
            }]));
        }
        if keys & pad::CIRCLE != 0 {
            self.return_to_field();
            return Ok(Response::Commands(vec![DuelCommand::Pick {
                pick: Pick::Cancel,
            }]));
        }
        Ok(Response::Ignored)
    }

    /// State 0x2D (0x1D5348), key priority as the handler tests them. In
    /// R1's view (d66 bit3 clear) Cross/Square do nothing and Circle closes
    /// (0x2E → 0x18); Triangle opens the details of the entry in both
    /// (0x1D5740: return state 0x2D; error sound on an empty list).
    fn graveyard(&mut self, duel: &Duel, keys: u16, mut pick: GraveyardPick) -> Response {
        let i = usize::from(pick.shown);
        let count = pick.lists[i].len() as i32;
        let max_scroll = (count - 5).max(0);
        let clamp = |v: i32| v.clamp(0, max_scroll) as u8;
        if keys & (pad::L1 | pad::R1) != 0 {
            // 0x1D5374 / 0x1D5670: the offset moves by five, the row stays.
            let delta = if keys & pad::L1 != 0 { -5 } else { 5 };
            let next = clamp(i32::from(pick.scroll[i]) + delta);
            let changed = next != pick.scroll[i];
            pick.scroll[i] = next;
            self.mode = Mode::Graveyard(pick);
            return if changed {
                Response::Moved
            } else {
                Response::Ignored
            };
        }
        if keys & (pad::UP | pad::DOWN) != 0 {
            let (row, scroll) = (i32::from(pick.row[i]), i32::from(pick.scroll[i]));
            let (next_row, next_scroll) = if keys & pad::UP != 0 {
                if row > 0 {
                    (row - 1, scroll)
                } else {
                    (row, (scroll - 1).max(0))
                }
            } else if row < (count - 1).min(4) {
                (row + 1, scroll)
            } else {
                (row, scroll + 1)
            };
            let next_scroll = clamp(next_scroll);
            let changed = next_row != row || next_scroll != pick.scroll[i];
            pick.row[i] = next_row as u8;
            pick.scroll[i] = next_scroll;
            self.mode = Mode::Graveyard(pick);
            return if changed {
                Response::Moved
            } else {
                Response::Ignored
            };
        }
        // N-player duels (a port addition): Right / Left step through
        // every side's list.
        let n = pick.lists.len() as u8;
        let target = if duel.multi().is_some() {
            if keys & pad::RIGHT != 0 && pick.shown + 1 < n {
                Some(pick.shown + 1)
            } else if keys & pad::LEFT != 0 && pick.shown > 0 {
                Some(pick.shown - 1)
            } else {
                None
            }
        } else {
            let switch = if pick.shown == 0 {
                keys & pad::RIGHT != 0
            } else {
                keys & pad::LEFT != 0
            };
            switch.then_some(pick.shown ^ 1)
        };
        if let Some(to) = target {
            if pick.locked {
                return Response::Ignored;
            }
            // 0x1D5410 / 0x1D5598: the row carries over, clamped to the
            // other list's last entry.
            let other = usize::from(to);
            let last = (pick.lists[other].len() as i32 - 1).max(0) as u8;
            pick.row[other] = pick.row[i].min(last);
            pick.shown = to;
            self.mode = Mode::Graveyard(pick);
            return Response::Moved;
        }
        if keys & pad::TRIANGLE != 0 {
            let Some(index) = pick.selected() else {
                return Response::Refused;
            };
            let card = duel.sides[usize::from(pick.shown)].graveyard[index];
            self.details = Some(DetailsView {
                unit: None,
                card,
                page: 0,
            });
            return Response::Moved;
        }
        if self.graveyard_view {
            if keys & pad::CIRCLE != 0 {
                self.graveyard_view = false;
                self.return_to_field();
                return Response::Moved;
            }
            return Response::Ignored;
        }
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            let Some(index) = pick.selected() else {
                return Response::Refused;
            };
            self.return_to_field();
            return Response::Commands(vec![DuelCommand::Pick {
                pick: Pick::Graveyard {
                    side: pick.shown,
                    index,
                },
            }]);
        }
        if keys & pad::CIRCLE != 0 {
            self.return_to_field();
            return Response::Commands(vec![DuelCommand::Pick { pick: Pick::Cancel }]);
        }
        Response::Ignored
    }

    fn summon_open(&self) -> bool {
        matches!(self.mode, Mode::Hand(_) | Mode::Confirm(_))
    }

    /// State 0x18 (0x1D386C).
    fn field(&mut self, duel: &Duel, cards: &impl CardData, keys: u16) -> Result<Response> {
        let side = duel.active;
        let cursor = self.cursors[usize::from(side)];
        let here = duel.unit_at(cursor);
        if keys & pad::TRIANGLE != 0 {
            return Ok(match here {
                Some(unit) => {
                    self.details = Some(DetailsView {
                        unit: Some(unit.id),
                        card: unit.card,
                        page: 0,
                    });
                    Response::Moved
                }
                None => Response::Refused,
            });
        }
        if keys & pad::SELECT != 0 {
            // 0x1D3930 → 0x23: D35 = 0 ("no" highlighted), then 0x24
            // slides the window in and 0x25 polls.
            self.surrender = Some(SurrenderPrompt { yes: false });
            return Ok(Response::Moved);
        }
        if keys & pad::L1 != 0 {
            // 0x1D3954 → 0x27 (SE 4): d35 = 0, d36 = the option bits.
            self.settings = Some(SettingsPanel {
                row: 0,
                bits: self.options & 0b111,
            });
            return Ok(Response::Moved);
        }
        if keys & pad::R1 != 0 {
            // 0x1D3978 → 0x2B (SE 4): d35 = 0 (list filter: every card,
            // 0x251920 mode 0), d36 = the active side (not locked).
            let list = |g: usize| (0..duel.sides[g].graveyard.len()).collect::<Vec<usize>>();
            self.graveyard_view = true;
            self.mode = Mode::Graveyard(GraveyardPick {
                lists: (0..duel.sides.len()).map(list).collect(),
                shown: side,
                locked: false,
                row: vec![0; duel.sides.len()],
                scroll: vec![0; duel.sides.len()],
            });
            return Ok(Response::Moved);
        }
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            let Some(unit) = here.filter(|u| u.controller() == side) else {
                return Ok(Response::Refused);
            };
            if unit.slot == 0 && keys & pad::SQUARE != 0 {
                let s = &duel.sides[usize::from(side)];
                if !s.can_summon {
                    return Ok(if s.hand.is_empty() {
                        Response::Refused
                    } else {
                        // 0x1D3AB0 → 0x51; d4F is not reset there [H: the
                        // port starts the view on the first card].
                        self.hand_view = Some(HandView { cursor: 0 });
                        Response::Moved
                    });
                }
                if s.hand.len() + s.deck.len() == 0 {
                    return Ok(Response::Refused);
                }
                self.mode = Mode::SummonCell {
                    leader: cursor,
                    menu: HandMenu::default(),
                };
                return Ok(Response::Moved);
            }
            if unit.acted {
                return Ok(Response::Refused);
            }
            let (allowance, wall) = duel.move_range(unit, cards)?;
            let mut sel = Selection {
                unit: unit.id,
                path: Vec::new(),
                allowance,
                wall,
                face_up: unit.face_up,
                defence: unit.defence,
                marks: vec![UNMARKED; CELLS],
            };
            let leader = unit.slot == 0;
            mark_cells(duel, &mut sel, true, leader, side, cursor);
            self.mode = Mode::Move(sel);
            return Ok(Response::Moved);
        }
        if keys & pad::L2 != 0 {
            // 0x1D3BCC: d66 bit7 toggled, d67 bit0 := it (SE 4).
            self.hud_hidden = !self.hud_hidden;
            return Ok(Response::Moved);
        }
        if keys & pad::START != 0 {
            // The next turn starts with state 0x16, which clears d64 bit6.
            self.alternate = false;
            return Ok(Response::Commands(vec![DuelCommand::EndTurn]));
        }
        Ok(match self.step(cursor, keys) {
            Some(next) => {
                self.cursors[usize::from(side)] = next;
                Response::Moved
            }
            None => Response::Ignored,
        })
    }

    /// State 0x1D (0x1D41F4).
    fn moving(
        &mut self,
        duel: &Duel,
        cards: &impl CardData,
        keys: u16,
        mut sel: Selection,
    ) -> Result<Response> {
        let side = duel.active;
        let cursor = self.cursors[usize::from(side)];
        // Circle needs no unit: the original leaves 0x1D whatever the slot
        // holds (lesson 3 step 143 cancels a selection whose unit a script
        // edit replaced, TUTORIAL.md §4).
        if keys & pad::CIRCLE != 0 {
            if let Some(&origin) = sel.path.first() {
                self.cursors[usize::from(side)] = origin;
            }
            self.return_to_field();
            return Ok(Response::Moved);
        }
        let unit = duel
            .unit(sel.unit)
            .ok_or(DuelError::InvalidCommand("selected unit is gone"))?
            .clone();
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            let path = if sel.path.is_empty() {
                Vec::new()
            } else {
                let mut p = sel.path[1..].to_vec();
                p.push(cursor);
                p
            };
            let hold = path.is_empty() && unit.slot != 0;
            self.return_to_field();
            return Ok(Response::Commands(vec![DuelCommand::Move {
                unit: sel.unit,
                face_up: sel.face_up && !unit.face_up,
                defence: (hold && sel.defence != unit.defence).then_some(sel.defence),
                path,
            }]));
        }
        let dpad = keys & (pad::UP | pad::DOWN | pad::LEFT | pad::RIGHT);
        if dpad != 0 {
            let Some(next) = self.step(cursor, keys) else {
                return Ok(Response::Ignored);
            };
            let d59 = sel.path.len();
            if d59 >= 2 && sel.path[..d59 - 1].contains(&next) {
                return Ok(Response::Ignored);
            }
            if sel.marks[index(next)] > sel.allowance {
                return Ok(Response::Ignored);
            }
            if d59 > 0 && sel.path[d59 - 1] == next {
                sel.path.pop();
            } else if d59 < usize::from(sel.allowance) {
                sel.path.push(cursor);
            } else {
                return Ok(Response::Ignored);
            }
            self.cursors[usize::from(side)] = next;
            mark_cells(duel, &mut sel, false, unit.slot == 0, side, next);
            // A step takes attack position (0x1D4678).
            sel.defence = false;
            self.mode = Mode::Move(sel);
            return Ok(Response::Moved);
        }
        // Shoulder buttons: non-leaders before the first step (0x1D46E0).
        if unit.slot == 0 || !sel.path.is_empty() || keys & 0xF == 0 {
            return Ok(Response::Ignored);
        }
        let mut changed = false;
        if keys & (pad::L1 | pad::R1) != 0 {
            let defence = keys & pad::L1 == 0;
            changed |= sel.defence != defence;
            sel.defence = defence;
        }
        if keys & (pad::L2 | pad::R2) != 0 && !unit.committed_face_up {
            let face_up = keys & pad::L2 == 0;
            if sel.face_up != face_up {
                sel.face_up = face_up;
                changed = true;
                let mut probe = unit.clone();
                probe.face_up = face_up;
                let (allowance, wall) = duel.move_range(&probe, cards)?;
                sel.allowance = allowance;
                sel.wall = wall;
                mark_cells(duel, &mut sel, true, false, side, cursor);
            }
        }
        self.mode = Mode::Move(sel);
        Ok(if changed {
            Response::Moved
        } else {
            Response::Ignored
        })
    }

    /// State 0x3E (0x1D6414).
    fn summon_cell(
        &mut self,
        duel: &Duel,
        keys: u16,
        leader: GridPos,
        menu: HandMenu,
    ) -> Result<Response> {
        let side = duel.active;
        let cursor = self.cursors[usize::from(side)];
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            if !duel.summon_area(side, cursor) {
                return Ok(Response::Refused);
            }
            self.mode = Mode::Hand(menu);
            return Ok(Response::Commands(vec![DuelCommand::OpenSummon {
                cell: cursor,
            }]));
        }
        if keys & pad::CIRCLE != 0 {
            self.cursors[usize::from(side)] = leader;
            self.return_to_field();
            return Ok(Response::Moved);
        }
        Ok(match self.step(cursor, keys) {
            Some(next) => {
                self.cursors[usize::from(side)] = next;
                Response::Moved
            }
            None => Response::Ignored,
        })
    }

    /// State 0x43 (0x1D6D70). Keys are not mirrored here.
    fn hand(
        &mut self,
        duel: &Duel,
        cards: &impl CardData,
        keys: u16,
        mut menu: HandMenu,
    ) -> Result<Response> {
        let side = duel.active;
        let hand = &duel.sides[usize::from(side)].hand;
        let stars = duel.sides[usize::from(side)].stars;
        let n = hand.len();
        if n == 0 {
            return Err(DuelError::InvalidCommand("the hand menu has no cards"));
        }
        menu.cursor = menu.cursor.min(n - 1);
        menu.marks.retain(|&i| i < n);
        let cost = |i: usize| duel.hand_cost(side, hand[i], cards).unwrap_or(u8::MAX);
        if keys & (pad::LEFT | pad::RIGHT) != 0 {
            let before = menu.cursor;
            menu.cursor = if keys & pad::LEFT != 0 {
                (menu.cursor + n - 1) % n
            } else {
                (menu.cursor + 1) % n
            };
            let moved = menu.cursor != before;
            self.mode = Mode::Hand(menu);
            return Ok(if moved {
                Response::Moved
            } else {
                Response::Ignored
            });
        }
        if keys & (pad::UP | pad::DOWN) != 0 {
            if stars < cost(menu.cursor) {
                return Ok(Response::Ignored);
            }
            let marked = menu.marks.contains(&menu.cursor);
            let changed = if keys & pad::UP != 0 {
                !marked && {
                    menu.marks.push(menu.cursor);
                    true
                }
            } else {
                marked && {
                    menu.marks.retain(|&i| i != menu.cursor);
                    true
                }
            };
            self.mode = Mode::Hand(menu);
            return Ok(if changed {
                Response::Moved
            } else {
                Response::Ignored
            });
        }
        if keys & pad::TRIANGLE != 0 {
            // 0x43 → 0x20 with d35 = 0x43: the card under the hand cursor.
            self.details = Some(DetailsView {
                unit: None,
                card: hand[menu.cursor],
                page: 0,
            });
            self.mode = Mode::Hand(menu);
            return Ok(Response::Moved);
        }
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            let needed = if menu.marks.is_empty() {
                cost(menu.cursor)
            } else {
                menu.marks.iter().map(|&i| cost(i)).max().unwrap_or(0)
            };
            if stars < needed {
                return Ok(Response::Refused);
            }
            self.mode = Mode::Confirm(menu);
            return Ok(Response::Moved);
        }
        if keys & pad::CIRCLE != 0 {
            let leader = duel.leader(side).position;
            self.mode = Mode::SummonCell { leader, menu };
            return Ok(Response::Commands(vec![DuelCommand::CancelSummon]));
        }
        Ok(Response::Ignored)
    }

    /// State 0x21: the details view (its contents are the presentation's).
    /// `keys` are the raw pad bits (the hand cursor), `board_keys` the
    /// mirrored ones (the page).
    fn details(&mut self, duel: &Duel, keys: u16, board_keys: u16, view: DetailsView) -> Response {
        let side = usize::from(duel.active);
        if board_keys & (pad::UP | pad::DOWN) != 0 {
            let page = if board_keys & pad::UP != 0 {
                (view.page + DETAILS_PAGES - 1) % DETAILS_PAGES
            } else {
                (view.page + 1) % DETAILS_PAGES
            };
            self.details = Some(DetailsView { page, ..view });
            return Response::Moved;
        }
        let hand = &duel.sides[side].hand;
        if keys & (pad::LEFT | pad::RIGHT) != 0 && !hand.is_empty() {
            let n = hand.len();
            let step = |c: usize| {
                if keys & pad::LEFT != 0 {
                    (c + n - 1) % n
                } else {
                    (c + 1) % n
                }
            };
            let cursor = match (&mut self.mode, &mut self.hand_view) {
                (_, Some(v)) => {
                    v.cursor = step(v.cursor.min(n - 1));
                    v.cursor
                }
                (Mode::Hand(menu), None) => {
                    menu.cursor = step(menu.cursor.min(n - 1));
                    menu.cursor
                }
                _ => return Response::Ignored,
            };
            self.details = Some(DetailsView {
                card: hand[cursor],
                ..view
            });
            return Response::Moved;
        }
        if keys & pad::CIRCLE != 0 {
            self.details = None;
            if matches!(self.mode, Mode::Field) && self.hand_view.is_none() {
                self.return_to_field();
            }
            return Response::Moved;
        }
        Response::Ignored
    }

    /// State 0x29 (0x1D4FD0): the settings panel.
    fn settings_panel(&mut self, keys: u16, mut panel: SettingsPanel) -> Response {
        if keys & (pad::UP | pad::DOWN) != 0 {
            panel.row ^= 1;
            self.settings = Some(panel);
            return Response::Moved;
        }
        if keys & (pad::LEFT | pad::RIGHT) != 0 {
            let bit = 1 << (panel.row + 1);
            let bits = if keys & pad::LEFT != 0 {
                panel.bits | bit
            } else {
                panel.bits & !bit
            };
            let changed = bits != panel.bits;
            panel.bits = bits;
            self.settings = Some(panel);
            return if changed {
                Response::Moved
            } else {
                Response::Ignored
            };
        }
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            self.options = panel.bits;
            self.settings = None;
            return Response::Moved;
        }
        if keys & pad::CIRCLE != 0 {
            self.settings = None;
            return Response::Moved;
        }
        Response::Ignored
    }

    /// State 0x53 (0x1D83B0): the hand view.
    fn hand_view(&mut self, duel: &Duel, keys: u16, mut view: HandView) -> Response {
        let hand = &duel.sides[usize::from(duel.active)].hand;
        let n = hand.len();
        if keys & (pad::LEFT | pad::RIGHT) != 0 {
            if n == 0 {
                return Response::Ignored;
            }
            let before = view.cursor;
            view.cursor = if keys & pad::LEFT != 0 {
                (view.cursor.min(n - 1) + n - 1) % n
            } else {
                (view.cursor.min(n - 1) + 1) % n
            };
            self.hand_view = Some(view);
            return if view.cursor != before {
                Response::Moved
            } else {
                Response::Ignored
            };
        }
        if keys & pad::TRIANGLE != 0 {
            let Some(&card) = hand.get(view.cursor) else {
                return Response::Refused;
            };
            self.details = Some(DetailsView {
                unit: None,
                card,
                page: 0,
            });
            return Response::Moved;
        }
        if keys & pad::CIRCLE != 0 {
            self.hand_view = None;
            self.return_to_field();
            return Response::Moved;
        }
        Response::Ignored
    }

    /// State 0x25 (0x1D4E00): the surrender prompt. Left/Right pick "yes"/
    /// "no"; Cross or Square confirm (a "yes" ends the duel through 0x26:
    /// the active side surrenders); Circle cancels with "no". The window's
    /// slide (0x24, 0x26) is not modelled.
    fn surrender_prompt(&mut self, keys: u16, prompt: SurrenderPrompt) -> Response {
        if keys & (pad::LEFT | pad::RIGHT) != 0 {
            let yes = keys & pad::LEFT != 0;
            self.surrender = Some(SurrenderPrompt { yes });
            return if yes != prompt.yes {
                Response::Moved
            } else {
                Response::Ignored
            };
        }
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            self.surrender = None;
            return if prompt.yes {
                Response::Commands(vec![DuelCommand::Surrender])
            } else {
                Response::Moved
            };
        }
        if keys & pad::CIRCLE != 0 {
            self.surrender = None;
            return Response::Moved;
        }
        Response::Ignored
    }

    /// State 0x46 (0x1D7230).
    fn confirm(&mut self, keys: u16, menu: HandMenu) -> Response {
        if keys & (pad::CROSS | pad::SQUARE) != 0 {
            let hand = if menu.marks.is_empty() {
                vec![menu.cursor]
            } else {
                menu.marks.clone()
            };
            self.return_to_field();
            return Response::Commands(vec![DuelCommand::ChooseSummon { hand }]);
        }
        if keys & pad::CIRCLE != 0 {
            self.mode = Mode::Hand(menu);
            return Response::Moved;
        }
        Response::Ignored
    }

    /// One board step (0x1D3F40 / 0x1D4390 / 0x1D6638): presses past an
    /// edge are dropped, and a held diagonal alternates axes via d64 bit6.
    /// Back to 0x18 through state 0x16, which clears d64 bit6 (0x1D37F8).
    fn return_to_field(&mut self) {
        self.mode = Mode::Field;
        self.alternate = false;
    }

    fn step(&mut self, at: GridPos, keys: u16) -> Option<GridPos> {
        let mut k = keys & (pad::UP | pad::DOWN | pad::LEFT | pad::RIGHT);
        // No d-pad bits: the states leave through 0x1D8F38 without touching
        // d64 (0x1D3EE8, 0x1D662C, 0x1D4368).
        if k == 0 {
            return None;
        }
        let (mut c, mut r) = (at.column, at.row);
        if (k & pad::LEFT != 0 && c == 0) || (k & pad::RIGHT != 0 && c >= 6) {
            k &= !(pad::LEFT | pad::RIGHT);
        }
        if (k & pad::UP != 0 && r == 0) || (k & pad::DOWN != 0 && r >= 6) {
            k &= !(pad::UP | pad::DOWN);
        }
        let vertical = pad::UP | pad::DOWN;
        if self.alternate {
            if k & vertical != 0 {
                k &= vertical;
            } else {
                self.alternate = false;
            }
        }
        if k & (pad::LEFT | pad::RIGHT) != 0 {
            c = if k & pad::LEFT != 0 { c - 1 } else { c + 1 };
            if k & vertical != 0 {
                self.alternate = true;
            }
        } else if k & vertical != 0 {
            r = if k & pad::UP != 0 { r - 1 } else { r + 1 };
            self.alternate = false;
        } else {
            self.alternate = false;
            return None;
        }
        Some(GridPos { column: c, row: r })
    }
}

/// Side 1 sees the d-pad mirrored (0x1D35D4–0x1D3648).
/// The board d-pad of `side`: side 1's is mirrored (the original); in an
/// N-player duel (a port addition) the east and west seats' boards are
/// turned a quarter (east looks west: Up is the column below, Left the row
/// below; west the other way).
fn board_keys(duel: &Duel, side: u8, keys: u16) -> u16 {
    if duel.multi().is_none() {
        return if side == 1 { mirror(keys) } else { keys };
    }
    match side {
        1 => mirror(keys),
        2 => turn(keys, [pad::LEFT, pad::RIGHT, pad::DOWN, pad::UP]),
        3 => turn(keys, [pad::RIGHT, pad::LEFT, pad::UP, pad::DOWN]),
        _ => keys,
    }
}

/// Up, Down, Left, Right become `to[0..4]`.
fn turn(keys: u16, to: [u16; 4]) -> u16 {
    let from = [pad::UP, pad::DOWN, pad::LEFT, pad::RIGHT];
    let mut out = keys & !(pad::UP | pad::DOWN | pad::LEFT | pad::RIGHT);
    for (f, t) in from.into_iter().zip(to) {
        if keys & f != 0 {
            out |= t;
        }
    }
    out
}

fn mirror(keys: u16) -> u16 {
    let swap = |k: u16, a: u16, b: u16| {
        let mut out = k & !(a | b);
        if k & a != 0 {
            out |= b;
        }
        if k & b != 0 {
            out |= a;
        }
        out
    };
    swap(swap(keys, pad::LEFT, pad::RIGHT), pad::UP, pad::DOWN)
}

fn index(p: GridPos) -> usize {
    p.index().expect("cursor on the board")
}

/// 0x1E3A50 → 0x1E3CE0: marks for the selection from `from` with the
/// remaining allowance and the path so far as prefix; afterwards prefix cell
/// i is marked i. `reset` clears all marks, otherwise earlier marks turn
/// stale.
fn mark_cells(
    duel: &Duel,
    sel: &mut Selection,
    reset: bool,
    leader: bool,
    side: u8,
    from: GridPos,
) {
    for m in sel.marks.iter_mut() {
        *m = if reset || *m == UNMARKED {
            UNMARKED
        } else {
            STALE
        };
    }
    let search = Search {
        duel,
        wall: sel.wall,
        leader,
        side,
        prefix: &sel.path,
        max: sel.allowance - sel.path.len() as u8,
    };
    let mut pathbuf = Vec::new();
    search.visit(
        &mut sel.marks,
        &mut pathbuf,
        i32::from(from.column),
        i32::from(from.row),
        0,
    );
    for (i, &p) in sel.path.iter().enumerate() {
        sel.marks[index(p)] = i as u8;
    }
}

struct Search<'a> {
    duel: &'a Duel,
    wall: bool,
    leader: bool,
    side: u8,
    prefix: &'a [GridPos],
    max: u8,
}

impl Search<'_> {
    fn visit(&self, marks: &mut [u8], pathbuf: &mut Vec<GridPos>, c: i32, r: i32, depth: u8) {
        if !(0..7).contains(&c) || !(0..7).contains(&r) {
            return;
        }
        let cell = GridPos {
            column: c as u8,
            row: r as u8,
        };
        let unit = self.duel.unit_at(cell);
        let wall_cell = self.duel.terrain_at(cell) == terrain::LABYRINTH;
        if depth > 0 {
            if self.prefix.contains(&cell) {
                return;
            }
            if let Some(u) = unit {
                if (self.leader && u.controller() != self.side)
                    || (u.slot == 0 && u.controller() == self.side)
                {
                    return;
                }
            }
        }
        pathbuf.truncate(usize::from(depth));
        pathbuf.push(cell);
        let total = self.prefix.len() as u8 + depth;
        if unit.is_some() || !wall_cell || self.wall {
            let m = &mut marks[index(cell)];
            *m = (*m).min(total);
        }
        // The counter passed by value is 1: an occupied cell (or an empty
        // wall without the flag) ends the branch unless it is the start.
        if (unit.is_some() || (wall_cell && !self.wall)) && total > 0 {
            return;
        }
        let depth = depth + 1;
        if depth > self.max {
            return;
        }
        for (dc, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = (c + dc, r + dr);
            if pathbuf[..usize::from(depth)]
                .iter()
                .any(|p| (i32::from(p.column), i32::from(p.row)) == next)
            {
                continue;
            }
            self.visit(marks, pathbuf, next.0, next.1, depth);
        }
    }
}
