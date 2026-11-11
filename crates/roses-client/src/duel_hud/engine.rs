//! The HUD's state ([`HudState`]) from the rules engine: a
//! [`roses_core::duel::Duel`], the rule getters of
//! [`roses_core::duel::queries`] and a plain snapshot of the input-state
//! bytes ([`HudUi`]) the original keeps next to the duel in EE RAM.
//!
//! Field by field ("S" = side struct EE 0x2DABA0 + side·0x350, "dXX" =
//! 0x2DABA0 + 0xDXX; private spec `hud-spec.json` → `state_fields`):
//!
//! | HUD field | original | from |
//! |---|---|---|
//! | `sides[s].lp`, `stars`, `deck`, `hand` | S+0x33A, +0x341, +0x230, +0x32E | [`Duel::sides`] |
//! | `sides[s].controller` | S+0x33C | [`Context::controllers`] |
//! | `sides[s].cursor` | S+0x33D/+0x33E | [`HudUi::cursors`] |
//! | `sides[s].summon_flags` | S+0x346 (bit 0 read) | `can_summon` |
//! | `sides[s].summon_counter` | S+0x344 | [`HudExtra::summon_counters`] |
//! | `sides[s].monsters`, `magic_traps` | 0x1E13D0 / 0x1E1470 | units of the side in slots 1–5 / 6–10 |
//! | `d3c`, `d3d`, `d3e`, `d3f`, `d40`–`d43` | the HUD's own bytes | `previous` ([`HudUpdate`] of the last frame, through [`after_skipped_states`]) |
//! | `d53`, `d54` | 0x2DB8F3 / 0x2DB8F4 | [`Duel::round`] / [`Duel::active`] |
//! | `d35`, `d44`, `d45`, `d47`, `d48`, `d4f`, `d57`, `d65`–`d67`, `flash` | input-state bytes | [`HudUi`] ([`ui_from_input`], [`FlashWave`]) |
//! | `units` | records 0x2DB990 + group·0x3F4 + slot·0x5C | field units, then the hand records |
//! | `board`, `terrain` | 0x1E4960 / cell +0x16 | [`Duel::unit_at`] / [`Duel::terrain`] |
//! | `unit_a`, `unit_b` | 0x2DB268 / 0x2DB26C | [`HudUi::unit_a`] / [`HudUi::unit_b`] |
//! | `hand_card` | 0x1E07C0(2, d4f + 1) | hand record `d4f` |
//! | `advantage` | 0x2692E0(A, B) | [`Query::advantage`] |
//! | `legend_1e3760` | 0x1E3760(cursor) != 0 | not a legal summon cell ([`Duel::summon_area`]) |
//! | `legend_1fb900`, `option_30`, `overlay_mode`, `terrain_model_ready` | 0x1FB900, 0x1A8C50(30), 0x1AB8B0(1), 0x1F2580 | [`HudExtra`] |
//! | `hand_box_values` | 0x1E1B70(2, d4f + 1, 0) / 0x270280() | [`Query::hand_card_cost`] / [`Query::marked_cost`] |
//! | `hand_overlay` | 0x1AFF00: record +4 object floats, 0x1E0340 words | [`HudExtra::hand_positions`], hand + [`HudUi::marks`] |
//! | `message_box` | overlay slot 3 (0x2DB855, 0x2DB846/48/50) | [`HudUi::message_box`] |
//! | `graveyard` | overlay slot 0 code 0x0E: lists 0x1ED0000/0x1ED00F0, d36, c8a/c8e, d5e/d5f, c9c | [`HudUi::graveyard`] (card ids) + the card data (icon byte) |
//!
//! Unit records: +0x3C card, +0x3E/+0x3F kind and attribute of the card,
//! +0x40/+0x41 cell, +0x44 side, +0x45 slot, +0x49 = `spellbind`, +0x57 =
//! acted | control change << 1 | face-up << 2 | defence << 3 | committed
//! face-up << 4 | committed defence << 5 | selected << 6, +0x58 bit 3 =
//! `spellbind_fresh`. What the engine does not keep is passed as the
//! original holds it in every recorded state:
//! - +0x4A (second turn counter): 0 on every unit of every saved state.
//! - +0x58 bit 2 (no turn counter): clear on field units, set on hand
//!   records (0xF5) in every saved state.
//! - Hand records (group 2, slot i + 1 for hand card i of the active side,
//!   built by 0x1D9C70 for the hand menu): +0x57 = 0x80, off the board,
//!   no counters; only present while [`HudUi::hand_records`].

use super::{
    GraveyardEntry, GraveyardPanel, HandFace, HandOverlayCard, HudSide, HudState, HudUnit,
    HudUpdate, UnitStats, GRAVEYARD_REST,
};
use roses_core::duel::queries::{Context, Query, Record, Stats};
use roses_core::duel::ui::{DuelUi, GraveyardPick, Mode};
use roses_core::duel::{CardData, Duel, DuelUnit};
use roses_core::{GridPos, UnitId};
use std::collections::BTreeMap;

/// A unit record one of the HUD's unit pointers (0x2DB268 / 0x2DB26C) holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudRecord {
    /// A field unit (record groups 0 and 1).
    Field(UnitId),
    /// The hand record of hand card `i` of the active side (group 2, slot
    /// i + 1); absent unless [`HudUi::hand_records`].
    Hand(usize),
}

/// The unit a move selection holds (+0x57 bit 6), with the face and
/// position the selection has written into its record (+0x57 bits 2 and 3,
/// changed by L2/R2 and L1/R1 before the first step).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selected {
    pub unit: UnitId,
    pub face_up: bool,
    pub defence: bool,
}

/// Input-state bytes the HUD reads (`dXX` = EE 0x2DABA0 + 0xDXX), as the
/// duel loop 0x1D00F0 leaves them before the HUD routine runs.
#[derive(Clone, Debug, PartialEq)]
pub struct HudUi {
    /// d57 (0x2DB8F7): the duel sub-state (`ui::Mode::state`).
    pub d57: u8,
    /// d35 (0x2DB8D5): read only with d57 0x20–0x22 (details view).
    pub d35: u8,
    /// Each side's board cursor (S+0x33D/+0x33E).
    pub cursors: [GridPos; 2],
    /// 0x2DB268: the unit under the cursor (0x16, 0x18), the mover (from
    /// 0x19 on), the leader that opened a summon (0x3E–0x46).
    pub unit_a: Option<HudRecord>,
    /// 0x2DB26C: the unit under the cursor in 0x1D and 0x3E, the unit on
    /// the chosen summon cell from 0x3F on; cleared by 0x16.
    pub unit_b: Option<HudRecord>,
    /// The unit with +0x57 bit 6 (a move selection).
    pub selected: Option<Selected>,
    /// d4f (0x2DB8EF): hand cursor.
    pub d4f: u8,
    /// Marked hand indices in marking order (d4e = their count; the order
    /// is the hand word's bits 12–15).
    pub marks: Vec<usize>,
    /// The hand records of the active side's hand exist (group 2, built for
    /// the hand menu 0x43 and read by the hand boxes and the overlay).
    pub hand_records: bool,
    /// d44: banner alpha.
    pub d44: u8,
    /// d45 / d47 / d48: hand-menu slide, closing, hand boxes.
    pub d45: u8,
    pub d47: u8,
    pub d48: u8,
    /// d65 / d66 / d67: HUD control bits (d67 as the duel loop leaves it;
    /// the HUD's own write-back of the last frame is `previous.d67`).
    pub d65: u8,
    pub d66: u8,
    pub d67: u8,
    /// u16 0x2DB828: the flashing overlay's alpha (value >> 8).
    pub flash: u16,
    /// Original state outside the engine and the input states.
    pub extra: HudExtra,
    /// Overlay window slot 3 (0x1E5540 kind 0), `None` when closed.
    pub message_box: Option<super::MessageBox>,
    /// Overlay window slot 0 with code 0x0E (the GRAVEYARD panel of
    /// Mimicat / Monster Reborn), `None` while slot 0 holds anything else.
    pub graveyard: Option<GraveyardUi>,
}

/// What the game keeps for the GRAVEYARD panel (states 0x2B..0x2E; B =
/// EE 0x2DABA0). [`ui_from_input`] fills it from `ui::Mode::Graveyard` at
/// rest; the caller animates `slide` (0x2C: +8 per update from 0 up to
/// [`GRAVEYARD_REST`] while opening, input read from the update it
/// reaches 64; 0x2E: −8 per update down to 0 after Cross/Circle, then the
/// slot closes and the pick's event runs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraveyardUi {
    /// Card ids of list 0 (side 0's graveyard) and list 1 (side 1's), the
    /// candidates only (class filter applied), oldest first: 0x1ED0000 /
    /// 0x1ED00F0. A list the card does not offer is empty.
    pub lists: [Vec<u16>; 2],
    /// d36 & 0x7F: the list shown (0 or 1).
    pub shown: u8,
    /// d36 bit 7: only one list offered.
    pub locked: bool,
    /// B+0xC8A / B+0xC8E: first entry of the five-row window per list.
    pub offsets: [u16; 2],
    /// d5e / d5f: cursor row (0..=4) per list.
    pub rows: [u8; 2],
    /// s16 B+0xC9C: 0 (closed, below the screen) .. 64 (at rest).
    pub slide: i16,
}

impl GraveyardUi {
    /// The panel at rest for the port's graveyard pick: list entries are
    /// the cards at the pick's graveyard indices.
    pub fn from_pick(duel: &Duel, pick: &GraveyardPick) -> Self {
        let cards = |s: usize| -> Vec<u16> {
            pick.lists[s]
                .iter()
                .filter_map(|&i| duel.sides[s].graveyard.get(i).copied())
                .collect()
        };
        Self {
            lists: [cards(0), cards(1)],
            shown: pick.shown,
            locked: pick.locked,
            offsets: [0, 1].map(|s| u16::from(pick.scroll.get(s).copied().unwrap_or(0))),
            rows: [0, 1].map(|s| pick.row.get(s).copied().unwrap_or(0)),
            slide: GRAVEYARD_REST,
        }
    }
}

/// 0x1DE350(card): byte 0 of the card record (class << 5 | race), `None`
/// for card 671 (read as 999) and ids >= 854 [F 0x1DE350].
fn record_kind<C: CardData + ?Sized>(cards: &C, card: u16) -> Option<u8> {
    if card == 671 || card >= 854 {
        return None;
    }
    cards.card(card).map(|c| c.kind)
}

/// Original state the HUD reads that neither the engine nor the input
/// states keep. [`HudExtra::default`] is what the original holds in every
/// situation the port models (first campaign duel, states 0x18, 0x1D, 0x3E,
/// 0x43, 0x46; checked on the saved states of that duel).
#[derive(Clone, Debug, PartialEq)]
pub struct HudExtra {
    /// 0x1A8C50(30) != 0 (word 0x2DAB08): 1 in every saved duel state.
    pub option_30: bool,
    /// 0x1AB8B0(1): 1 in every saved duel state (the hand overlay needs 1).
    pub overlay_mode: i8,
    /// 0x1F2580(T + 74): the terrain tile model is loaded (true in every
    /// saved state with the HUD shown).
    pub terrain_model_ready: bool,
    /// S+0x344 per side: the special-draw flag, 0 unless a side with the
    /// special draw fell below 1000 LP (never for the player at rank 1 or
    /// for Weevil, controller 2; the project notes).
    pub summon_counters: [u8; 2],
    /// 0x1FB900(−1, −1) != 0: read only in state 0x30 (not modelled).
    pub legend_1fb900: bool,
    /// Screen position floats of hand card i's 3D object (record +4 →
    /// object +0/+4); `None` when the object is absent. See
    /// [`settled_hand_positions`].
    pub hand_positions: Vec<Option<[f32; 2]>>,
    /// Online (a port addition): the side this screen belongs to. The
    /// original's 2-player duel has one shared screen, so the card box hides
    /// a player's own cards too (Card Display HIDE needs R2, and d3c follows
    /// the acting side). A seat with a screen of its own always reads the
    /// cards it controls. `None` offline: the original's rule, unchanged.
    pub own_side: Option<u8>,
}

impl Default for HudExtra {
    fn default() -> Self {
        Self {
            option_30: true,
            overlay_mode: 1,
            terrain_model_ready: true,
            summon_counters: [0; 2],
            legend_1fb900: false,
            hand_positions: Vec::new(),
            own_side: None,
        }
    }
}

/// Hand-card object positions once the hand menu (0x43) has settled: x =
/// −202 + 101·i, y = 8 for five cards, as saved states 377, 683 and 708
/// hold them. The fly-in and fly-out are not modelled; fewer than five
/// cards (an empty deck) are centred the same way [H].
pub fn settled_hand_positions(count: usize) -> Vec<Option<[f32; 2]>> {
    let first = -50.5 * count.saturating_sub(1) as f32;
    (0..count)
        .map(|i| Some([first + 101.0 * i as f32, 8.0]))
        .collect()
}

/// The flashing overlay's alpha word (u16 0x2DB828), stepped once per
/// update by the duel loop (0x1D08C8): ±0x800 with the direction in d64
/// bits 2–3, clamped to 0..=0x8000, the direction turning at either end (a
/// 32-update triangle). Its phase at duel start is not established: the
/// wave starts at 0, rising [H].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashWave {
    pub value: u16,
    pub rising: bool,
}

impl Default for FlashWave {
    fn default() -> Self {
        Self {
            value: 0,
            rising: true,
        }
    }
}

impl FlashWave {
    /// One update; returns the new value.
    pub fn step(&mut self) -> u16 {
        self.step_by(0x800)
    }

    /// One update of the second scroll word 0x2DB826 (direction 0x2DB904
    /// bits 0-1): d63 (0x2DB903) != 0 first forces the rising direction, then
    /// ±0x200 as 0x2DB828 [F: the same block in every stepper, the duel
    /// 0x1D081C, the deck editor 0x1B9558, Trade 0x1C3484, the map editor
    /// 0x1CD8E4]. With d63 set the word sits at 0x8000.
    pub fn step_p1(&mut self, d63: u8) -> u16 {
        if d63 != 0 {
            self.rising = true;
        }
        self.step_by(0x200)
    }

    fn step_by(&mut self, step: i32) -> u16 {
        let v = i32::from(self.value) + if self.rising { step } else { -step };
        self.value = v.clamp(0, 0x8000) as u16;
        if self.value == 0 || self.value >= 0x8000 {
            self.rising = !self.rising;
        }
        self.value
    }
}

/// The HUD bytes at the start of a duel as the first campaign duel's
/// saved start state holds them: shown (d3e 0, d67 bit 0), the active
/// side's turn latched, its cursor in d40/d41, no upper box.
pub fn initial_update(duel: &Duel, input: &DuelUi) -> HudUpdate {
    let cursor = input.cursors[usize::from(duel.active & 1)];
    HudUpdate {
        d3c: duel.active,
        d3d: duel.round,
        d3e: 0,
        d3f: 0,
        d40: cursor.column,
        d41: cursor.row,
        d42: 0,
        d43: 0,
        d67: 1,
    }
}

/// The HUD's write-backs once the states the port's input machine skips
/// between the last update (state `from`) and this one (state `to`) have
/// run. The port leaves the hand menu at once, but the original first runs
/// states that keep the HUD's hand path (d45 or d47 != 0) going until the
/// hand card's upper box has slid away (d3f 0):
/// - 0x43 Circle (0x1D6F28) sets d67 bit 3 and goes to 0x40, which lowers
///   d45 by 16 per update (0x1D6970) and resumes 0x3E only once it is 0
///   (0x1D6AE8); with bit 3 set the hand path lowers d3f by 16 per update.
/// - 0x46 Cross (0x1D7230) goes to 0x47 (0x4C for a fusion) with d47 still
///   30 (saved state 755), which lowers d3f the same way; d47 is cleared on
///   the way to 0x16 (0x1D8148).
///
/// Without this the normal card-box path would take over with d3f still up
/// and slide an upper box away for the stale cell d42/d43. The hand path
/// writes only d3f, so the other bytes are kept.
pub fn after_skipped_states(previous: &HudUpdate, from: u8, to: u8) -> HudUpdate {
    let hand_menu = |d57: u8| matches!(d57, 0x43 | 0x46);
    let mut next = *previous;
    if hand_menu(from) && !hand_menu(to) {
        next.d3f = 0;
    }
    next
}

/// The input-state bytes the original holds in each state the port's
/// input machine ([`DuelUi`]) models, from the handlers of 0x1D00F0 and the
/// saved states of the first campaign duel:
/// - unit A (0x2DB268): the unit under the active cursor in 0x18
///   (0x1D3878); the mover in 0x1D; the active leader (A when Square opened
///   the summon) in 0x3E, 0x43 and 0x46.
/// - unit B (0x2DB26C): none in 0x18 (cleared by 0x16, 0x1D37F0); the unit
///   under the cursor in 0x1D (0x1D4200) and 0x3E (0x1D6420); the unit on the
///   chosen summon cell ([`Duel::pending_summon`]) in 0x43 and 0x46.
/// - the mover's +0x57 bit 6 and pending face/position in 0x1D.
/// - d4f / marks: the hand menu's cursor and marks; hand records exist in
///   0x43 and 0x46, their 3D cards at [`settled_hand_positions`].
/// - d45 / d47 / d48: 128 / 0 / 128 in 0x43 and 0 / 30 / 128 in 0x46 (the
///   settled values of saved states 377, 683, 708 and 753; the slides of the
///   unmodelled states 0x40–0x42 and 0x44–0x45 are skipped), else 0.
/// - d67: the last write-back with bit 2 (fast slide) set in 0x43 only,
///   and bit 0 (show) cleared while the latched turn (d3c, d3d) differs from
///   the engine's (the turn-end state 0x15 clears it at 0x1D3758; the HUD
///   then slides out, latches the new turn and slides back in).
/// - d35 0, d44 0 (no banner), d65 0, d66 0x80 (re-show after the latch).
///
/// `previous` should already have been through [`after_skipped_states`].
pub fn ui_from_input(duel: &Duel, input: &DuelUi, previous: &HudUpdate, flash: u16) -> HudUi {
    let active = duel.active;
    let cursor = input.cursors[usize::from(active & 1)];
    let at = |p: GridPos| duel.unit_at(p).map(|u| HudRecord::Field(u.id));
    let leader = duel
        .units
        .iter()
        .find(|u| u.side == active && u.slot == 0)
        .map(|u| HudRecord::Field(u.id));
    let hand = duel.sides[usize::from(active & 1)].hand.len();
    let mut ui = HudUi {
        d57: input.mode.state(),
        d35: 0,
        cursors: [0, 1].map(|s| input.cursors.get(s).copied().unwrap_or(GridPos { column: 3, row: 3 })),
        unit_a: None,
        unit_b: None,
        selected: None,
        d4f: 0,
        marks: Vec::new(),
        hand_records: false,
        d44: 0,
        d45: 0,
        d47: 0,
        d48: 0,
        d65: 0,
        d66: 0x80,
        d67: 0,
        flash,
        extra: HudExtra::default(),
        message_box: None,
        graveyard: None,
    };
    match &input.mode {
        // The unit-selection menu (0x30) is not drawn yet: the cursor
        // record only.
        Mode::Field | Mode::SelectUnit(_) => ui.unit_a = at(cursor),
        Mode::Graveyard(pick) => {
            ui.unit_a = at(cursor);
            // d66 bit 3 ("magic selection") is set through the pick [emu
            // golden-graveyard: d66 0x88 in 0x2C/0x2D]: legend set 16. R1's
            // view-only lists keep it clear (0x1D3978; legend Dtl/Ref/Cncl
            // [emu modes/tutorial45 l5-step019]).
            if !input.graveyard_view {
                ui.d66 |= 8;
            }
            ui.graveyard = Some(GraveyardUi::from_pick(duel, pick));
        }
        Mode::Move(sel) => {
            ui.unit_a = Some(HudRecord::Field(sel.unit));
            ui.unit_b = at(cursor);
            ui.selected = Some(Selected {
                unit: sel.unit,
                face_up: sel.face_up,
                defence: sel.defence,
            });
        }
        Mode::SummonCell { .. } => {
            ui.unit_a = leader;
            ui.unit_b = at(cursor);
        }
        Mode::Hand(menu) | Mode::Confirm(menu) => {
            ui.unit_a = leader;
            ui.unit_b = duel.pending_summon.and_then(at);
            ui.d4f = menu.cursor.min(usize::from(u8::MAX)) as u8;
            ui.marks = menu.marks.clone();
            ui.hand_records = true;
            ui.extra.hand_positions = settled_hand_positions(hand);
            (ui.d45, ui.d47, ui.d48) = if matches!(input.mode, Mode::Hand(_)) {
                (128, 0, 128)
            } else {
                (0, 30, 128)
            };
        }
    }
    // Overlays over the field (`DuelUi::state`): the settings panel (0x29,
    // the OPTION panel of overlay slot 0, drawn by the caller) keeps the
    // field's records; the hand view (0x53) shows the hand row at rest
    // with the leader's box (0x1B1D50 reads unit_a in 0x51..=0x53) and the
    // hand cursor d4F [F 0x1D8188..0x1D84A4].
    let hand_view = input.hand_view.filter(|_| matches!(input.mode, Mode::Field));
    if input.settings.is_some() && matches!(input.mode, Mode::Field) {
        ui.d57 = 0x29;
    } else if let Some(view) = hand_view {
        ui.d57 = 0x53;
        ui.unit_a = leader;
        ui.unit_b = None;
        ui.d4f = view.cursor.min(usize::from(u8::MAX)) as u8;
        ui.hand_records = true;
        ui.extra.hand_positions = settled_hand_positions(hand);
        (ui.d45, ui.d47, ui.d48) = (128, 0, 128);
    }
    // The details view (Triangle) keeps the state it was opened from in d35
    // and the records of that state (the hand view's included); the caller
    // steps 0x20 / 0x21 / 0x22 with the slide [F 0x1D38F0, 0x1D57D8,
    // 0x1D6EA8, 0x1D8440; emu modes/ui3 dd-*.json].
    if input.details.is_some() {
        ui.d35 = ui.d57;
        ui.d57 = 0x21;
    }
    let mut d67 = previous.d67 & !4;
    if (previous.d3c, previous.d3d) != (active, duel.round) {
        d67 &= !1;
    }
    if matches!(input.mode, Mode::Hand(_)) || hand_view.is_some() {
        d67 |= 4;
    }
    ui.d67 = d67;
    ui
}

fn stats(s: Stats) -> UnitStats {
    UnitStats {
        attack: s.attack,
        defence: s.defence,
        terrain_effect: s.terrain_effect,
        allowance: s.allowance,
    }
}

/// The HUD state of one frame. `rank(side, card)` is 0x2154C0 (the save
/// data's rank, −1 for none); `previous` holds the HUD bytes the routine
/// wrote back last frame (its `d67` is not read: `ui.d67` is the value the
/// duel loop leaves, usually `previous.d67` with the loop's own bits
/// applied). The move selection's pending face and position
/// ([`HudUi::selected`]) are applied to a copy of the duel.
pub fn from_duel<C: CardData + ?Sized>(
    duel: &Duel,
    ui: &HudUi,
    cards: &C,
    context: &Context,
    rank: &dyn Fn(u8, u16) -> i32,
    previous: &HudUpdate,
) -> HudState {
    let patched;
    let duel = match ui.selected {
        Some(s)
            if duel
                .unit(s.unit)
                .is_some_and(|u| (u.face_up, u.defence) != (s.face_up, s.defence)) =>
        {
            let mut copy = duel.clone();
            for u in copy.units.iter_mut().filter(|u| u.id == s.unit) {
                (u.face_up, u.defence) = (s.face_up, s.defence);
            }
            patched = copy;
            &patched
        }
        _ => duel,
    };
    let q = Query::new(duel, cards, context);
    let d3c = previous.d3c;
    let cursor = ui.cursors[usize::from(d3c & 1)];
    let active = duel.active;
    let selected = ui.selected.map(|s| s.unit);

    let sides = [0u8, 1].map(|s| {
        let side = &duel.sides[usize::from(s)];
        let count = |slots: std::ops::RangeInclusive<u8>| {
            duel.units
                .iter()
                .filter(|u| u.side == s && slots.contains(&u.slot))
                .count() as u8
        };
        let c = ui.cursors[usize::from(s)];
        HudSide {
            lp: side.lp,
            controller: context.controllers[usize::from(s)],
            cursor: [c.column, c.row],
            stars: side.stars,
            deck: side.deck.len() as u8,
            hand: side.hand.len() as u8,
            summon_flags: u8::from(side.can_summon),
            summon_counter: ui.extra.summon_counters[usize::from(s)],
            monsters: count(1..=5),
            magic_traps: count(6..=10),
        }
    });

    let mut units = Vec::new();
    let mut field = BTreeMap::new();
    for u in &duel.units {
        field.insert(u.id, units.len());
        units.push(field_unit(&q, u, d3c, ui.extra.own_side, cursor, rank, selected == Some(u.id)));
    }
    let hand = &duel.sides[usize::from(active & 1)].hand;
    let first_hand = units.len();
    let hand_count = if ui.hand_records {
        hand.len().min(10)
    } else {
        0
    };
    let summon_cell = duel.pending_summon.map_or([0; 2], |c| [c.column, c.row]);
    for (i, &card) in hand.iter().take(hand_count).enumerate() {
        let slot = i as u8 + 1;
        let r = Record::Hand {
            side: active,
            slot,
            card,
        };
        let info = cards.card(card);
        units.push(HudUnit {
            has_card: true,
            card,
            kind: info.map_or(0, |c| c.kind),
            attribute: info.map_or(0, |c| c.attribute),
            cell: summon_cell,
            side: active,
            slot,
            turns: [0; 2],
            flags: 0x80,
            flags2: 0xF5,
            controller: q.controller(active, slot),
            rank: rank(active, card),
            face_down_hidden: ui.extra.own_side != Some(active) && q.hidden(r, d3c),
            effect: q.has_effect(r),
            abilities: q.abilities(r, rank),
            legend_query: q.legend_query(r, d3c),
            at_cursor: q.stats(r, cursor).map(stats).unwrap_or_default(),
            at_own_cell: UnitStats::default(),
        });
    }
    let record = |r: Option<HudRecord>| match r? {
        HudRecord::Field(id) => field.get(&id).copied(),
        HudRecord::Hand(i) => (i < hand_count).then_some(first_hand + i),
    };

    let mut board = [None; 49];
    for (i, cell) in board.iter_mut().enumerate() {
        let p = GridPos {
            column: (i % 7) as u8,
            row: (i / 7) as u8,
        };
        *cell = duel.unit_at(p).and_then(|u| field.get(&u.id).copied());
    }
    let mut terrain = [0; 49];
    for (t, &v) in terrain.iter_mut().zip(&duel.terrain) {
        *t = v;
    }

    let unit_a = record(ui.unit_a);
    let unit_b = record(ui.unit_b);
    let hand_card = record(Some(HudRecord::Hand(usize::from(ui.d4f))));
    let advantage = match (unit_a, unit_b) {
        (Some(a), Some(b)) => {
            let (a, b) = (&units[a], &units[b]);
            Some(q.advantage((a.side, a.slot), (b.side, b.slot)))
        }
        _ => None,
    };
    let left = hand_card.map_or(-1, |i| q.hand_card_cost(active, units[i].card));
    let right = if ui.marks.is_empty() {
        left
    } else {
        q.marked_cost(&ui.marks)
    };
    let hand_overlay = (0..hand.len())
        .map(|i| {
            let order = ui
                .marks
                .iter()
                .position(|&m| m == i)
                .map_or(0, |p| p as i32 + 1);
            HandOverlayCard {
                position: if i < hand_count {
                    ui.extra.hand_positions.get(i).copied().flatten()
                } else {
                    None
                },
                word: if i < 5 {
                    i32::from(hand[i] & 0xFFF) | order << 12
                } else {
                    -1
                },
                face: (i < hand_count).then(|| {
                    let unit = &units[first_hand + i];
                    let r = Record::Hand {
                        side: active,
                        slot: unit.slot,
                        card: unit.card,
                    };
                    let cell = GridPos {
                        column: unit.cell[0],
                        row: unit.cell[1],
                    };
                    let values = q.stats(r, cell).unwrap_or_default();
                    HandFace {
                        card: unit.card,
                        level: q.hand_card_cost(active, unit.card).max(0) as u8,
                        attack: values.attack,
                        defence: values.defence,
                    }
                }),
            }
        })
        .collect();

    HudState {
        sides,
        d35: ui.d35,
        d3c,
        d3d: previous.d3d,
        d3e: previous.d3e,
        d3f: previous.d3f,
        d40: previous.d40,
        d41: previous.d41,
        d42: previous.d42,
        d43: previous.d43,
        d44: ui.d44,
        d45: ui.d45,
        d47: ui.d47,
        d48: ui.d48,
        d4f: ui.d4f,
        d53: duel.round,
        d54: active,
        d57: ui.d57,
        d65: ui.d65,
        d66: ui.d66,
        d67: ui.d67,
        flash: ui.flash,
        option_30: ui.extra.option_30,
        own_side: ui.extra.own_side,
        units,
        board,
        terrain,
        unit_a,
        unit_b,
        hand_card,
        advantage,
        legend_1fb900: ui.extra.legend_1fb900,
        // Only state 0x3E reads it; its marks are 0x1E37F0's summon cells.
        legend_1e3760: ui.d57 == 0x3E && !duel.summon_area(active, cursor),
        terrain_model_ready: ui.extra.terrain_model_ready,
        #[cfg(feature = "online")]
        names: [None, None],
        #[cfg(feature = "online")]
        notes: Vec::new(),
        hand_box_values: [left, right],
        hand_box_hidden: false,
        overlay_mode: ui.extra.overlay_mode,
        hand_overlay,
        message_box: ui.message_box,
        graveyard: ui.graveyard.as_ref().map(|g| GraveyardPanel {
            lists: [0, 1].map(|l| {
                g.lists[l]
                    .iter()
                    .map(|&card| GraveyardEntry {
                        card,
                        kind: record_kind(cards, card),
                    })
                    .collect()
            }),
            shown: g.shown,
            locked: g.locked,
            offsets: g.offsets,
            rows: g.rows,
            slide: g.slide,
        }),
    }
}

fn field_unit<C: CardData + ?Sized>(
    q: &Query<'_, C>,
    u: &DuelUnit,
    d3c: u8,
    own_side: Option<u8>,
    cursor: GridPos,
    rank: &dyn Fn(u8, u16) -> i32,
    selected: bool,
) -> HudUnit {
    let r = Record::Field(u);
    let info = q.cards.card(u.card);
    let changed = q.ctx.control_changed.contains(&u.id);
    let flags = u8::from(u.acted)
        | u8::from(changed) << 1
        | u8::from(u.face_up) << 2
        | u8::from(u.defence) << 3
        | u8::from(u.committed_face_up) << 4
        | u8::from(u.committed_defence) << 5
        | u8::from(selected) << 6;
    // The cell holding the unit (0x1E4960 of its +0x40/+0x41).
    let own = q
        .duel
        .unit_at(u.position)
        .filter(|o| o.id == u.id)
        .map(|_| u.position);
    HudUnit {
        has_card: true,
        card: u.card,
        kind: info.map_or(0, |c| c.kind),
        attribute: info.map_or(0, |c| c.attribute),
        cell: [u.position.column, u.position.row],
        side: u.side,
        slot: u.slot,
        turns: [u.spellbind, 0],
        flags,
        flags2: u8::from(u.spellbind_fresh) << 3,
        controller: q.controller(u.side, u.slot),
        rank: rank(u.side, u.card),
        face_down_hidden: own_side != Some(q.controller(u.side, u.slot)) && q.hidden(r, d3c),
        effect: q.has_effect(r),
        abilities: q.abilities(r, rank),
        legend_query: q.legend_query(r, d3c),
        at_cursor: q.stats(r, cursor).map(stats).unwrap_or_default(),
        at_own_cell: own
            .and_then(|cell| q.stats(r, cell))
            .map(stats)
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    //! Synthetic cards and positions (invented numbers, not retail data).
    use super::*;
    use crate::duel_hud::{frame, Element};
    use roses_core::duel::{kind, terrain, CardInfo, Catalogue, SideState};

    fn card(id: u16, kind: u8, level: u8, attribute: u8, attack: u16) -> CardInfo {
        CardInfo {
            id,
            kind,
            level,
            attribute,
            attack,
            defense: 500,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        }
    }

    fn unit(id: u16, card: u16, side: u8, slot: u8, column: u8, row: u8) -> DuelUnit {
        DuelUnit {
            id: UnitId(id),
            card,
            side,
            slot,
            position: GridPos { column, row },
            face_up: true,
            defence: false,
            committed_face_up: true,
            committed_defence: false,
            acted: false,
            spellbind: 0,
            summon_order: 0,
            attack_bonus: 0,
            defense_bonus: 0,
            spellbind_fresh: false,
            evo_counter: 0,
            stolen: false,
            discount: 0,
            stolen_by: None,
        }
    }

    fn setup() -> (Duel, Catalogue) {
        let cards = Catalogue::new(
            vec![
                card(0, kind::DRAGON, 6, 0, 2000),
                card(1, kind::FIEND, 6, 1, 1800),
                card(2, kind::BEAST, 4, 3, 1200),
                card(3, kind::MACHINE, 3, 2, 900),
            ],
            [],
        );
        let mut terrain = vec![terrain::NORMAL; 49];
        terrain[5 * 7 + 3] = terrain::FOREST;
        let mut enemy = unit(3, 3, 1, 1, 3, 4);
        enemy.committed_face_up = false;
        enemy.face_up = false;
        let units = vec![
            unit(0, 0, 0, 0, 3, 6),
            unit(1, 1, 1, 0, 3, 0),
            unit(2, 2, 0, 1, 3, 5),
            enemy,
        ];
        let side = |hand: Vec<u16>| SideState {
            lp: 4000,
            stars: 7,
            can_summon: true,
            hand,
            deck: vec![3; 30],
            leader_abilities: vec![],
            summons: 0,
            graveyard: vec![],
            destiny: 0,
        };
        let duel = Duel::from_observed(
            terrain,
            units,
            [side(vec![2, 3, 2]), side(vec![3; 4])],
            1,
            0,
            0,
            &cards,
        )
        .unwrap();
        (duel, cards)
    }

    fn ui(duel: &Duel) -> HudUi {
        let cursor = GridPos { column: 3, row: 5 };
        HudUi {
            d57: 0x18,
            d35: 0,
            cursors: [cursor, GridPos { column: 3, row: 0 }],
            unit_a: duel.unit_at(cursor).map(|u| HudRecord::Field(u.id)),
            unit_b: None,
            selected: None,
            d4f: 0,
            marks: vec![],
            hand_records: false,
            d44: 0,
            d45: 0,
            d47: 0,
            d48: 0,
            d65: 0,
            d66: 0x80,
            d67: 1,
            flash: 0x4000,
            extra: HudExtra::default(),
            message_box: None,
            graveyard: None,
        }
    }

    fn ctx() -> Context {
        Context {
            option_28: 3,
            option_29: 3,
            controllers: [100, 2],
            ..Context::default()
        }
    }

    fn shown() -> HudUpdate {
        HudUpdate {
            d3c: 0,
            d3d: 1,
            d67: 1,
            ..HudUpdate::default()
        }
    }

    #[test]
    fn graveyard_pick_gives_the_panel_lists_and_icon_bytes() {
        use roses_core::duel::ui::GraveyardPick;
        let (mut duel, cards) = setup();
        duel.sides[0].graveyard = vec![2, 0, 3];
        duel.sides[1].graveyard = vec![1, 900];
        let mut input = DuelUi::new(&duel);
        input.mode = Mode::Graveyard(GraveyardPick {
            lists: vec![vec![0, 2], vec![0, 1]],
            shown: 1,
            locked: false,
            row: vec![1, 0],
            scroll: vec![0, 0],
        });
        let previous = initial_update(&duel, &input);
        let ui = ui_from_input(&duel, &input, &previous, 0);
        assert_eq!(ui.d57, 0x2D);
        // d66 bit 3 (magic selection) -> legend set 16.
        assert_eq!(ui.d66, 0x88);
        let g = ui.graveyard.clone().unwrap();
        assert_eq!(
            g,
            GraveyardUi {
                lists: [vec![2, 3], vec![1, 900]],
                shown: 1,
                locked: false,
                offsets: [0, 0],
                rows: [1, 0],
                slide: GRAVEYARD_REST,
            }
        );
        let rank = |_side: u8, _card: u16| 1;
        let st = from_duel(&duel, &ui, &cards, &ctx(), &rank, &previous);
        let panel = st.graveyard.unwrap();
        let kinds: Vec<Vec<Option<u8>>> = panel
            .lists
            .iter()
            .map(|l| l.iter().map(|e| e.kind).collect())
            .collect();
        // Card 900 has no record (0x1DE350 returns 0 for ids >= 854).
        assert_eq!(
            kinds,
            [vec![Some(kind::BEAST), Some(kind::MACHINE)], vec![Some(kind::FIEND), None]]
        );
        assert_eq!((panel.shown, panel.rows, panel.slide), (1, [1, 0], 64));
        assert_eq!(record_kind(&cards, 671), None);
    }

    /// Who reads a face-down card's name in the card box. `Some(seat)` is an
    /// online seat on its own screen; `None` keeps the original's rule.
    #[test]
    fn a_seat_reads_its_own_cards_and_offline_keeps_the_original_rule() {
        let (mut duel, cards) = setup();
        let own = duel.units.iter_mut().find(|u| u.id == UnitId(2)).unwrap();
        (own.face_up, own.committed_face_up) = (false, false);
        let rank = |_side: u8, _card: u16| 1;
        // (side 0's face-down, side 1's face-down, the acting side's hand).
        let hidden = |active: u8, ctx: &Context, own_side: Option<u8>| {
            let mut duel = duel.clone();
            duel.active = active;
            let mut ui = ui(&duel);
            ui.hand_records = true;
            ui.extra.own_side = own_side;
            let previous = HudUpdate { d3c: active, ..shown() };
            let st = from_duel(&duel, &ui, &cards, ctx, &rank, &previous);
            let (field, hand) = st.units.split_at(duel.units.len());
            assert!(!hand.is_empty());
            assert!(hand.iter().all(|h| h.face_down_hidden == hand[0].face_down_hidden));
            (field[2].face_down_hidden, field[3].face_down_hidden, hand[0].face_down_hidden)
        };
        let pads = |option_29| Context {
            option_28: 3,
            option_29,
            controllers: [100, 101],
            ..Context::default()
        };
        // Online, Card Display HIDE on both seats, on either side's turn:
        // own names shown, the other seat's face-down cards and hand not.
        assert_eq!(hidden(0, &pads(1), Some(0)), (false, true, false));
        assert_eq!(hidden(1, &pads(1), Some(0)), (false, true, true));
        assert_eq!(hidden(1, &pads(2), Some(1)), (true, false, false));
        assert_eq!(hidden(0, &pads(2), Some(1)), (true, false, true));
        // A spectator owns nothing: everything face-down stays hidden.
        assert_eq!(hidden(0, &pads(0), None), (true, true, true));
        // Offline against the computer: unchanged. On the computer's turn
        // the viewer d3c is the computer, so the player's own face-down card
        // is hidden as in the original.
        assert_eq!(hidden(0, &ctx(), None), (false, true, false));
        assert!(hidden(1, &ctx(), None).0);
        // Offline 2-player, one shared screen, HIDE: unchanged, own cards
        // need R2 so the other player cannot read them.
        let held = Context { r2_held: [true, false], ..pads(0) };
        assert_eq!(hidden(0, &held, None), (false, true, false));
    }

    #[test]
    fn field_state_maps_sides_units_board_and_pointers() {
        let (duel, cards) = setup();
        let rank = |_side: u8, _card: u16| 1;
        let st = from_duel(&duel, &ui(&duel), &cards, &ctx(), &rank, &shown());
        assert_eq!(st.sides[0].lp, 4000);
        assert_eq!((st.sides[0].hand, st.sides[1].hand), (3, 4));
        assert_eq!((st.sides[0].deck, st.sides[0].monsters), (30, 1));
        assert_eq!(st.sides[1].controller, 2);
        assert_eq!(st.sides[0].summon_flags, 1);
        assert_eq!((st.d53, st.d54, st.d57), (1, 0, 0x18));
        assert_eq!(st.units.len(), 4);
        let a = st.unit_a.unwrap();
        assert_eq!(st.board[5 * 7 + 3], Some(a));
        let beast = &st.units[a];
        assert_eq!((beast.card, beast.slot, beast.kind), (2, 1, kind::BEAST));
        // +500 on its forest cell (the cursor), not at its own cell? Both
        // are the forest cell here.
        assert_eq!(beast.at_cursor.attack, 1700);
        assert_eq!(beast.at_own_cell.attack, 1700);
        assert_eq!(st.terrain[5 * 7 + 3], terrain::FOREST);
        // The enemy's face-down unit is hidden from side 0.
        let enemy = st.board[4 * 7 + 3].unwrap();
        assert!(st.units[enemy].face_down_hidden);
        assert_eq!(st.hand_box_values, [-1, -1]);
        assert!(st.hand_card.is_none());

        let hud = frame(&st, &crate::duel_hud::tests::tables());
        assert!(hud.prims.iter().any(|p| p.element == Element::CardBox));
        assert_eq!(hud.next.d3e, 0);
    }

    #[test]
    fn move_selection_patches_the_mover_and_reads_the_advantage() {
        let (duel, cards) = setup();
        let rank = |_side: u8, _card: u16| 1;
        let mut ui = ui(&duel);
        ui.d57 = 0x1D;
        ui.cursors[0] = GridPos { column: 3, row: 4 };
        ui.unit_a = Some(HudRecord::Field(UnitId(2)));
        ui.unit_b = Some(HudRecord::Field(UnitId(3)));
        ui.selected = Some(Selected {
            unit: UnitId(2),
            face_up: true,
            defence: true,
        });
        let st = from_duel(&duel, &ui, &cards, &ctx(), &rank, &shown());
        let mover = &st.units[st.unit_a.unwrap()];
        assert_eq!(mover.flags & 0x48, 0x48);
        // B is face-down and not committed: no advantage shown.
        assert_eq!(st.advantage, Some(-1));
        assert_ne!(st.units[st.unit_b.unwrap()].controller, mover.controller);
    }

    #[test]
    fn hand_records_give_the_card_box_costs_and_overlay_words() {
        let (duel, cards) = setup();
        let rank = |_side: u8, _card: u16| 1;
        let mut ui = ui(&duel);
        ui.d57 = 0x43;
        ui.hand_records = true;
        ui.d4f = 1;
        ui.marks = vec![2, 0];
        ui.extra.hand_positions = settled_hand_positions(3);
        let st = from_duel(&duel, &ui, &cards, &ctx(), &rank, &shown());
        let h = st.hand_card.unwrap();
        assert_eq!((st.units[h].card, st.units[h].slot), (3, 2));
        assert_eq!(st.units[h].flags2 & 4, 4);
        // Left: the cursor card's level; right: the highest marked level.
        assert_eq!(st.hand_box_values, [3, 4]);
        let words: Vec<i32> = st.hand_overlay.iter().map(|c| c.word).collect();
        assert_eq!(words, [2 | 2 << 12, 3, 2 | 1 << 12]);
        assert_eq!(st.hand_overlay[0].position, Some([-101.0, 8.0]));
        assert_eq!(settled_hand_positions(5)[0], Some([-202.0, 8.0]));
        // The small faces show the hand records' cost and values.
        let face = st.hand_overlay[1].face.unwrap();
        assert_eq!(face.card, st.units[h].card);
        assert_eq!(i32::from(face.level), st.hand_box_values[0]);
    }

    #[test]
    fn flash_wave_is_a_32_update_triangle() {
        let mut wave = FlashWave::default();
        let values: Vec<u16> = (0..40).map(|_| wave.step()).collect();
        assert_eq!(values[0], 0x800);
        assert_eq!(values[15], 0x8000);
        assert_eq!(values[16], 0x7800);
        assert_eq!(values[31], 0);
        assert_eq!(values[32], 0x800);
        assert!(values.iter().all(|v| v % 0x800 == 0 && *v <= 0x8000));
    }

    #[test]
    fn input_states_give_the_unit_pointers_and_hand_bytes() {
        use roses_core::duel::ui::{HandMenu, Selection};
        let (mut duel, _) = setup();
        let mut input = DuelUi::new(&duel);
        input.cursors[0] = GridPos { column: 3, row: 5 };
        let previous = initial_update(&duel, &input);
        assert_eq!(
            (previous.d3c, previous.d3d, previous.d40, previous.d41),
            (0, 1, 3, 5)
        );
        let beast = Some(HudRecord::Field(UnitId(2)));
        let ui = ui_from_input(&duel, &input, &previous, 0x1800);
        assert_eq!(
            (ui.d57, ui.unit_a, ui.unit_b, ui.d67),
            (0x18, beast, None, 1)
        );
        assert_eq!(ui.flash, 0x1800);

        input.mode = Mode::Move(Selection {
            unit: UnitId(2),
            path: vec![],
            allowance: 1,
            wall: false,
            face_up: false,
            defence: true,
            marks: vec![0xFF; 49],
        });
        input.cursors[0] = GridPos { column: 3, row: 4 };
        let ui = ui_from_input(&duel, &input, &previous, 0);
        assert_eq!(ui.unit_a, beast);
        assert_eq!(ui.unit_b, Some(HudRecord::Field(UnitId(3))));
        assert_eq!(
            ui.selected.map(|s| (s.face_up, s.defence)),
            Some((false, true))
        );

        let leader = Some(HudRecord::Field(UnitId(0)));
        input.mode = Mode::SummonCell {
            leader: GridPos { column: 3, row: 6 },
            menu: HandMenu::default(),
        };
        let ui = ui_from_input(&duel, &input, &previous, 0);
        assert_eq!(
            (ui.d57, ui.unit_a, ui.unit_b),
            (0x3E, leader, Some(HudRecord::Field(UnitId(3))))
        );

        duel.pending_summon = Some(GridPos { column: 2, row: 5 });
        let menu = HandMenu {
            cursor: 2,
            marks: vec![1],
        };
        input.mode = Mode::Hand(menu.clone());
        let ui = ui_from_input(&duel, &input, &previous, 0);
        assert_eq!((ui.unit_a, ui.unit_b, ui.d4f), (leader, None, 2));
        assert_eq!((ui.d45, ui.d47, ui.d48, ui.d67), (128, 0, 128, 5));
        assert!(ui.hand_records && ui.marks == [1]);
        assert_eq!(ui.extra.hand_positions.len(), 3);
        input.mode = Mode::Confirm(menu);
        let ui = ui_from_input(&duel, &input, &previous, 0);
        assert_eq!((ui.d45, ui.d47, ui.d48, ui.d67), (0, 30, 128, 1));

        // A new turn the HUD has not latched: bit 0 cleared until it has.
        input.mode = Mode::Field;
        duel.active = 1;
        let ui = ui_from_input(&duel, &input, &previous, 0);
        assert_eq!(ui.d67 & 1, 0);
    }

    #[test]
    fn a_turn_change_slides_the_hud_out_latches_and_slides_back() {
        let (mut duel, cards) = setup();
        let rank = |_side: u8, _card: u16| 1;
        let tables = crate::duel_hud::tests::tables();
        let input = DuelUi::new(&duel);
        let mut previous = initial_update(&duel, &input);
        duel.active = 1;
        let mut shown = Vec::new();
        for _ in 0..40 {
            let ui = ui_from_input(&duel, &input, &previous, 0);
            let st = from_duel(&duel, &ui, &cards, &ctx(), &rank, &previous);
            let hud = frame(&st, &tables);
            previous = hud.next;
            shown.push((previous.d3e, previous.d3c, !hud.prims.is_empty()));
        }
        // 16 updates out (the last draws nothing and latches side 1), 16 in.
        assert_eq!(shown[14], (120, 0, true));
        assert_eq!(shown[15], (128, 1, false));
        assert_eq!(shown[16], (120, 1, true));
        assert_eq!(shown[31], (0, 1, true));
        assert_eq!(previous.d67 & 1, 1);
    }

    #[test]
    fn leaving_the_hand_menu_does_not_slide_a_box_for_a_stale_cell() {
        use roses_core::duel::ui::HandMenu;
        let (mut duel, cards) = setup();
        let rank = |_side: u8, _card: u16| 1;
        let tables = crate::duel_hud::tests::tables();
        let cell = GridPos { column: 2, row: 5 };
        let mut input = DuelUi::new(&duel);
        input.cursors[0] = cell;
        duel.pending_summon = Some(cell);
        // An earlier move selection left the enemy's cell (3, 4) in d42/d43.
        let start = HudUpdate {
            d42: 3,
            d43: 4,
            ..initial_update(&duel, &input)
        };
        let run = |duel: &Duel, input: &DuelUi, previous: HudUpdate| {
            let ui = ui_from_input(duel, input, &previous, 0);
            frame(
                &from_duel(duel, &ui, &cards, &ctx(), &rank, &previous),
                &tables,
            )
        };
        let boxes = |hud: &crate::duel_hud::HudFrame| {
            hud.prims
                .iter()
                .filter(|p| p.element == Element::CardBox)
                .count()
        };
        let exits = [
            (
                Mode::Hand(HandMenu::default()),
                Mode::SummonCell {
                    leader: GridPos { column: 3, row: 6 },
                    menu: HandMenu::default(),
                },
            ),
            (Mode::Confirm(HandMenu::default()), Mode::Field),
        ];
        for (menu, exit) in exits {
            input.mode = Mode::Hand(HandMenu::default());
            let mut previous = start;
            for _ in 0..8 {
                previous = run(&duel, &input, previous).next;
            }
            assert_eq!(previous.d3f, 128, "the hand card's box is up");
            input.mode = menu;
            let from = input.mode.state();
            previous = run(&duel, &input, previous).next;
            input.mode = exit;
            let to = input.mode.state();
            // Straight into the normal path: an upper box for (3, 4).
            let stale = run(&duel, &input, previous);
            assert!(stale.next.d3f > 0);
            let hud = run(&duel, &input, after_skipped_states(&previous, from, to));
            assert_eq!(hud.next.d3f, 0, "{from:#04x} -> {to:#04x}");
            assert!(boxes(&hud) < boxes(&stale));
            assert_eq!((hud.next.d42, hud.next.d43), (3, 4));
        }
        // Moves within the hand menu and outside it keep d3f.
        let up = HudUpdate { d3f: 128, ..start };
        assert_eq!(after_skipped_states(&up, 0x43, 0x46).d3f, 128);
        assert_eq!(after_skipped_states(&up, 0x46, 0x43).d3f, 128);
        assert_eq!(after_skipped_states(&up, 0x1D, 0x18).d3f, 128);
    }

    #[test]
    fn summon_cell_legend_reads_the_summon_area() {
        let (duel, cards) = setup();
        let rank = |_side: u8, _card: u16| 1;
        let mut ui = ui(&duel);
        ui.d57 = 0x3E;
        ui.cursors[0] = GridPos { column: 2, row: 5 };
        let st = from_duel(&duel, &ui, &cards, &ctx(), &rank, &shown());
        assert!(!st.legend_1e3760);
        ui.cursors[0] = GridPos { column: 3, row: 6 };
        let st = from_duel(&duel, &ui, &cards, &ctx(), &rank, &shown());
        assert!(st.legend_1e3760);
    }
}
