//! Custom Duel (title mode 9, "CUSTOM VS"): the set-up screen's model
//! (Claude, 5 Oct 2026). Bevy-free; the drawing is
//! `roses_game::custom_duel_screen`. Notes, disassembly and
//! interpreter runs: the project notes.
//!
//! Entry [F 0x1AD050.., emu]: the title's Custom Duel runs memory-card op 3
//! (`memory_card::CardOp::CustomDuelLoad`: slot 1 loaded, slot 2 checked);
//! its result's high byte − 1 is param[9], the *selector*, and mode 9 is
//! pushed. Init 0x1C6BC0(selector) seats the other side ([`Opponent`]),
//! sets the duel words of both sides (control +0x33C, LP +0x33A = 4000, SP
//! recharge +0x342 = 3) and the per-player block 0xA9C000 ([`PlayerSetup`]),
//! and builds the map list 0xA9C018 (the 24 default VS maps, then 5 own maps
//! per side whose card block has +0xFF9C bit 5 — the both-routes mark).
//!
//! Update 0x1C6F80 runs 0x1C6FC0 for player 0 and, only with a second human
//! (selector 0), for player 1. Each player's state byte ([2]/[3]) is a high
//! nibble mode and a low nibble row: 0x00 the row grid (table 0x2BDE40: two
//! columns of five rows, one table per selector kind; 0xFF = no row), 0x10
//! a row being edited (jump table 0x2BDE80), 0x20 ready (2P, waiting for the
//! other), 0x30 "Cancel settings and return to Title Screen" YES/NO, 0x40
//! the map list ("Select Map"; Cross starts the duel). Every branch below
//! follows the disassembly (`d_1c6fc0.txt`) [F] and the interpreter runs of
//! the game's own code reproduce it frame by frame [emu, private test].
//!
//! Start [F 0x1C7D64..0x1C7DF8]: param[2] = 0 (side 0 moves first), param[1]
//! = the map (default i → 0x16 + i; own map i of side s → 0x2E + 5·s + i),
//! param[3]/[4] = 1000 + each side's control (≥ 1000: the shuffled deck
//! insert), param[0x1C] = the leader-ability switches (bit s = player s's
//! pending row-3 value) and param[0x1D] the card-display switches (row 4);
//! the duel mode is pushed. param[0x1C] gates every leader-ability query of
//! the duel (0x1E1F00 via 0x273070, rules and computer alike); param[0x1D]
//! is read by the duel's hand/field shading 0x1D9B00 / 0x1DA6C0 [F]. After
//! the duel (or its reels) the mode pops back to 9 and init runs with −1
//! ([`CustomDuel::resume`]): the menu keeps its state (the map list is still
//! open), the duel words are set again from the committed LPs.
//!
//! Two players (selector 0: MEMORY CARD slot 2 holds a save) run player
//! 1's update on pad 2 (the project notes).
//! Edit Map (only with the both-routes mark) is an event the frontend turns
//! into the map editor (mode 0xD, `crate::map_editor`) on own map
//! (0xA9C08C + 2) % 0xA9C08D of the pad side's block.
use roses_core::card_block::{CardBlock, EMPTY_CARD};
use roses_core::deck_editor;
use serde::Serialize;

/// PS2 pad bits as 0x203380 tests them.
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
    pub const START: u16 = 0x0800;
    pub const UP: u16 = 0x1000;
    pub const RIGHT: u16 = 0x2000;
    pub const DOWN: u16 = 0x4000;
    pub const LEFT: u16 = 0x8000;
}

/// One player's pad this update: `pressed` = new presses (0x203380 mode
/// 0), `repeat` = new presses and auto-repeats (mode 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuPad {
    pub pressed: u16,
    pub repeat: u16,
}

impl MenuPad {
    /// A single new press (repeat = press).
    pub fn press(bits: u16) -> Self {
        Self { pressed: bits, repeat: bits }
    }
    fn new(&self, mask: u16) -> bool {
        self.pressed & mask != 0
    }
    fn rep(&self, mask: u16) -> bool {
        (self.repeat | self.pressed) & mask != 0
    }
}

/// SE ids (0x216EA0(0, id)).
pub const SFX_CURSOR: u16 = 3;
pub const SFX_CONFIRM: u16 = 4;
pub const SFX_CANCEL: u16 = 5;
pub const SFX_BUZZER: u16 = 6;

/// The human pad's control byte (100 + port).
pub const PAD_CONTROL: u8 = 100;
/// The second human's control byte (pad 2).
pub const SECOND_PAD_CONTROL: u8 = 101;
/// Duel words a fresh init writes [F 0x1C6D0C..].
pub const START_LP: u16 = 4000;
pub const START_SP: u8 = 3;
/// LP Setting's range and step [F 0x1C72F0..0x1C73AC].
pub const LP_MIN: u16 = 1;
pub const LP_MAX: u16 = 9999;
pub const LP_STEP: i32 = 100;
/// SP Recharge Rate's range [F 0x1C7524..0x1C7570].
pub const SP_MIN: u8 = 1;
pub const SP_MAX: u8 = 12;
/// What the computer side gets when the player commits a value (vs the
/// computer): LP max(player's, 4000), SP the player's if ≥ 4 else 3
/// [F 0x1C7450, 0x1C760C].
pub const CPU_MIN_LP: u16 = 4000;
pub const CPU_MIN_SP: u8 = 4;
pub const CPU_LOW_SP: u8 = 3;
/// Decks per card block (Select Deck cycles 0..2).
pub const DECKS: u8 = 3;

/// Rows (low nibble of the state byte) and their labels, MSG 130 + row.
pub mod row {
    pub const SELECT_DECK: u8 = 0;
    pub const LP_SETTING: u8 = 1;
    pub const SP_RECHARGE: u8 = 2;
    pub const LEADER_ABILITY: u8 = 3;
    pub const CARD_DISPLAY: u8 = 4;
    pub const BUILD_DECK: u8 = 5;
    pub const SAVE: u8 = 6;
    pub const EDIT_MAP: u8 = 7;
}
pub const ROW_LABEL_BASE: u16 = 130;
/// MSG 138 "Select Map", 139 "Default Map", 140/141 "1P/2P User Setup".
pub const MSG_SELECT_MAP: u16 = 138;
pub const MSG_DEFAULT_MAP: u16 = 139;
pub const MSG_USER_SETUP: u16 = 140;
/// MSG 127 "Cancel settings and return to Title Screen", 128 "Are you sure?".
pub const MSG_CANCEL_SETTINGS: u16 = 127;
pub const MSG_ARE_YOU_SURE: u16 = 128;

/// State byte modes (high nibble).
pub mod mode {
    pub const GRID: u8 = 0x00;
    pub const EDIT: u8 = 0x10;
    pub const READY: u8 = 0x20;
    pub const EXIT: u8 = 0x30;
    pub const MAP: u8 = 0x40;
}

/// Table 0x2BDE40: the row at (column, row) of the grid, by selector kind
/// (0 = two players, else against the computer); 0xFF = none [F].
pub const GRID_TWO_PLAYERS: [u8; 10] = [0, 1, 2, 3, 4, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
pub const GRID_COMPUTER: [u8; 10] = [0, 1, 2, 0xFF, 0xFF, 5, 6, 7, 0xFF, 0xFF];
const GRID_ROWS: u8 = 5;

/// The default VS maps (map list entries 0x200 | i → duel map 0x16 + i).
pub const DEFAULT_MAPS: u16 = 24;
pub const DEFAULT_MAP_BASE: u8 = 0x16;
/// Own maps (map editor): 5 per side, entries (side << 8) | i → 0x2E + 5·side + i.
pub const OWN_MAPS: u16 = 5;
pub const OWN_MAP_BASE: u8 = 0x2E;
/// The map list shows five rows; the chosen one is the middle (top + 2).
pub const MAP_ROWS: u8 = 5;

/// param[9]: the result's high byte − 1 (0x1AD1xx) [F, emu op3-*].
pub fn selector_from_result(result: u16) -> Option<u8> {
    match result >> 8 {
        1..=5 => Some((result >> 8) as u8 - 1),
        _ => None,
    }
}

/// Who sits on side 1 (init 0x1C6BC0's switch on the selector) [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Opponent {
    /// Selector 0 (slot 2 holds a save of this game): a second human,
    /// control 101, side 1 = slot 2's card block.
    SecondPlayer,
    /// Selector 1 (no card in slot 2): control 22 with a copy of the
    /// player's block (0x20AA70, 0xFFA0 bytes; +0xFF9C bit 5 cleared).
    Mirror,
    /// Selectors 2..4: a static deck record loaded into side 1's block
    /// (0x214120(1, record)): 2 → control 25, deck 0x32 (empty, unformatted
    /// or unreadable slot 2); 3 → 24, 0x31; 4 → 23, 0x30 (no producer of
    /// those two results was found [H]).
    Deck { control: u8, record: u8 },
}

impl Opponent {
    pub fn of(selector: u8) -> Option<Self> {
        Some(match selector {
            0 => Self::SecondPlayer,
            1 => Self::Mirror,
            2 => Self::Deck { control: 25, record: 0x32 },
            3 => Self::Deck { control: 24, record: 0x31 },
            4 => Self::Deck { control: 23, record: 0x30 },
            _ => return None,
        })
    }

    pub fn control(self) -> u8 {
        match self {
            Self::SecondPlayer => SECOND_PAD_CONTROL,
            Self::Mirror => 22,
            Self::Deck { control, .. } => control,
        }
    }
}

/// The per-player block at 0xA9C000 (bytes [p], [2+p], … for player p).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PlayerSetup {
    /// [0]: grid cursor, column · 5 + row.
    pub cursor: u8,
    /// [2]: mode | row.
    pub state: u8,
    /// [4]: the deck shown in Select Deck (committed by Cross).
    pub deck: u8,
    /// [6]: SP Recharge Rate (1..=12; committed to +0x342 by Cross).
    pub sp: u8,
    /// [8] / [0xA]: Leader Ability shown / committed (1 = Left choice off? see NOTES).
    pub leader: u8,
    pub leader_set: u8,
    /// [0xC] / [0xE]: Card Display shown / committed.
    pub display: u8,
    pub display_set: u8,
    /// [0x10] (s16): LP Setting shown; [0x14]: committed (the resume LP).
    pub lp: u16,
    pub lp_set: u16,
}

/// A duel side's words in the duel's side record (G + 0x350·side).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DuelSide {
    /// +0x33C.
    pub control: u8,
    /// +0x33A: the LP the duel starts with.
    pub lp: u16,
    /// +0x342: the SP recharge rate (read at every turn start, 0x269570).
    pub sp: u8,
}

/// What a Start hands to the duel mode (params 1..4, 0x1C, 0x1D).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DuelStart {
    /// param[1]: the duel map (terrain board), 0x16.. default, 0x2E.. own.
    pub map: u8,
    /// param[2]: the side moving first (always 0).
    pub first: u8,
    /// param[3] / [4]: 1000 + control.
    pub identities: [i32; 2],
    /// param[0x1C]: bit s = side s keeps its leader abilities.
    pub leader_abilities: u8,
    /// param[0x1D]: bit s = card display of side s.
    pub card_display: u8,
    /// The duel words (+0x33C, +0x33A, +0x342) of both sides.
    pub sides: [DuelSide; 2],
}

impl DuelStart {
    pub fn controls(&self) -> [u8; 2] {
        self.sides.map(|s| s.control)
    }
    pub fn leader_ability(&self, side: u8) -> bool {
        self.leader_abilities & (1 << side) != 0
    }
}

/// What an update asks of the frontend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum MenuEvent {
    Sfx(u16),
    /// Build Deck: 0x1A8C90(6, 0), push mode 6 (the deck editor on the
    /// player's block; the menu resumes at row 5).
    BuildDeck,
    /// Save: memory-card op 1 (the row waits until the card module is idle).
    Save,
    /// Edit Map on `side`'s own map `map` (0..4): push mode 0xD.
    EditMap { side: u8, map: u8 },
    /// YES on "return to Title Screen?": 0x1A9290 (pop to the title).
    Title,
    /// Cross on the map list: push the duel mode.
    Start(DuelStart),
}

/// The card blocks as the menu reads them.
pub trait SetupBlocks {
    /// 0x215040.
    fn current_deck(&self, side: u8) -> u8;
    /// 0x2150B0.
    fn set_current_deck(&mut self, side: u8, deck: u8);
    /// 0x215720(side, deck) ≥ 40 and 0x215160(side, deck) ≠ 999.
    fn deck_playable(&self, side: u8, deck: u8) -> bool;
    /// 0x216940: +0xFF9C bit 5 (both routes cleared).
    fn has_own_maps(&self, side: u8) -> bool;
}

impl SetupBlocks for [CardBlock; 2] {
    fn current_deck(&self, side: u8) -> u8 {
        deck_editor::current_deck(&self[usize::from(side & 1)])
    }
    fn set_current_deck(&mut self, side: u8, deck: u8) {
        deck_editor::set_current_deck(&mut self[usize::from(side & 1)], deck);
    }
    fn deck_playable(&self, side: u8, deck: u8) -> bool {
        let r = deck_editor::deck_record(&self[usize::from(side & 1)], deck);
        r.count() >= deck_editor::DECK_SIZE && r.leader != EMPTY_CARD
    }
    fn has_own_maps(&self, side: u8) -> bool {
        crate::ending::both_routes_marked(&self[usize::from(side & 1)])
    }
}

/// Side 1's block as init 0x1C6BC0 prepares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideOneBlock {
    /// Slot 2's block (selector 0).
    SecondCard,
    /// A copy of the player's block with +0xFF9C bit 5 cleared.
    PlayerCopy,
    /// 0x214120(1, record): a static deck record.
    StaticDeck(u8),
}

/// Prepare side 1's block in `blocks` (selector 1: the copy). Static decks
/// are the caller's (the deck table is in the executable).
pub fn side_one_block(selector: u8) -> Option<SideOneBlock> {
    Some(match Opponent::of(selector)? {
        Opponent::SecondPlayer => SideOneBlock::SecondCard,
        Opponent::Mirror => SideOneBlock::PlayerCopy,
        Opponent::Deck { record, .. } => SideOneBlock::StaticDeck(record),
    })
}

/// 0x20AA70 + the bit-5 clear of init's selector-1 branch.
pub fn mirror_block(player: &CardBlock) -> CardBlock {
    let mut copy = player.clone();
    if let Some(b) = copy.raw_tail.get_mut(crate::ending::BLOCK_FLAGS_TAIL_OFFSET) {
        *b &= !crate::ending::BOTH_ROUTES_BLOCK_BIT;
    }
    copy
}

/// The duel map of a map-list entry [F 0x1C7D3C..0x1C7D60].
pub fn entry_map(entry: u16) -> u8 {
    let hi = entry >> 8;
    let lo = (entry & 0xFF) as u8;
    if hi < 2 {
        lo.wrapping_add((hi as u8) * 5).wrapping_add(OWN_MAP_BASE)
    } else {
        lo.wrapping_add(DEFAULT_MAP_BASE)
    }
}

/// The whole menu state (0xA9C000..0xA9C08E and the two duel sides).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CustomDuel {
    /// 0xA9C088 (param[9] of the fresh init).
    pub selector: u8,
    pub players: [PlayerSetup; 2],
    pub sides: [DuelSide; 2],
    /// 0xA9C018: the map list (0x200 | i default, (side << 8) | i own).
    pub maps: Vec<u16>,
    /// 0xA9C089: the list's top row (the chosen entry is top + 2).
    pub map_top: u8,
    /// 0xA9C08B (cleared by init; no reader in the update).
    pub x8b: u8,
    /// 0xA9C08C / 0xA9C08D: Edit Map's own-map list (top, count).
    pub own_top: u8,
    pub own_count: u8,
    /// 0xA9C08E: YES chosen on the return-to-title question.
    pub exit_yes: bool,
    /// 0x2DB8D2/3: the deck graphics of a side changed (the next update
    /// reloads its graphics set, 0x200320(5 + side, 1)).
    pub reload: [bool; 2],
}

impl CustomDuel {
    /// Init 0x1C6BC0 with param[9] = `selector` ≥ 0 (side 1's block must
    /// already be prepared, [`side_one_block`]).
    pub fn new(selector: u8, blocks: &impl SetupBlocks) -> Self {
        let control = Opponent::of(selector).map_or(SECOND_PAD_CONTROL, Opponent::control);
        let player = |side: u8| PlayerSetup {
            cursor: 0,
            state: 0,
            deck: blocks.current_deck(side),
            sp: START_SP,
            leader: 1,
            leader_set: 1,
            display: 1,
            display_set: 1,
            lp: START_LP,
            lp_set: START_LP,
        };
        let mut maps: Vec<u16> = (0..DEFAULT_MAPS).map(|i| 0x200 | i).collect();
        let (mut own_top, mut own_count) = (0, 0);
        if blocks.has_own_maps(0) {
            maps.extend(0..OWN_MAPS);
            own_count = OWN_MAPS as u8;
            own_top = own_count - 2;
        }
        if blocks.has_own_maps(1) {
            maps.extend((0..OWN_MAPS).map(|i| 0x100 | i));
        }
        let map_top = (maps.len() as u8).wrapping_sub(2);
        Self {
            selector,
            players: [player(0), player(1)],
            sides: [
                DuelSide { control: PAD_CONTROL, lp: START_LP, sp: START_SP },
                DuelSide { control, lp: START_LP, sp: START_SP },
            ],
            maps,
            map_top,
            x8b: 0,
            own_top,
            own_count,
            exit_yes: false,
            reload: [false; 2],
        }
    }

    /// Init with −1 after the duel, its reels or the deck editor (the mode
    /// switch 0x1AB77C / 0x1AB7B4 / 0x1AB874): the menu as it was, the duel
    /// words set again from the committed LPs (+0x342 is not touched).
    pub fn resume(&mut self) {
        let control = Opponent::of(self.selector).map_or(SECOND_PAD_CONTROL, Opponent::control);
        self.sides[0].control = PAD_CONTROL;
        self.sides[0].lp = self.players[0].lp_set;
        self.sides[1].control = control;
        self.sides[1].lp = self.players[1].lp_set;
        self.reload = [false; 2];
    }

    fn grid(&self) -> &'static [u8; 10] {
        if self.selector == 0 { &GRID_TWO_PLAYERS } else { &GRID_COMPUTER }
    }

    /// The map list entry the list's middle row holds (Cross starts it).
    pub fn chosen_entry(&self) -> u16 {
        let n = self.maps.len().max(1);
        self.maps[(usize::from(self.map_top) + 2) % n]
    }

    /// Update 0x1C6F80: player 0, then player 1 when two humans play
    /// (`card_idle`: 0x1ABB20 < 0, the card module has nothing running).
    pub fn update(
        &mut self,
        pads: [MenuPad; 2],
        blocks: &mut impl SetupBlocks,
        card_idle: bool,
    ) -> Vec<MenuEvent> {
        let mut ev = Vec::new();
        let started = self.player(0, pads[0], blocks, card_idle, &mut ev);
        if !started && self.selector == 0 {
            self.player(1, pads[1], blocks, card_idle, &mut ev);
        }
        ev
    }

    /// 0x1C6FC0(side): true when the duel starts.
    fn player(
        &mut self,
        side: u8,
        p: MenuPad,
        blocks: &mut impl SetupBlocks,
        card_idle: bool,
        ev: &mut Vec<MenuEvent>,
    ) -> bool {
        let s = usize::from(side);
        let current = blocks.current_deck(side);
        // 0x1C7000: no fade running → a pending graphics reload.
        if self.reload[s] {
            self.reload[s] = false;
        }
        let st = self.players[s].state;
        match st & 0xF0 {
            mode::GRID if st < 8 => self.grid_update(side, p, blocks, ev),
            mode::MAP => return self.map_update(side, p, ev),
            mode::EXIT => self.exit_update(p, ev),
            mode::READY => {
                if p.new(pad::CIRCLE) {
                    ev.push(MenuEvent::Sfx(SFX_CANCEL));
                    self.players[s].state &= 0xF;
                }
            }
            mode::EDIT => self.edit_update(side, st & 0xF, p, current, blocks, card_idle, ev),
            _ => {}
        }
        false
    }

    fn grid_update(&mut self, side: u8, p: MenuPad, blocks: &impl SetupBlocks, ev: &mut Vec<MenuEvent>) {
        let s = usize::from(side);
        if p.rep(pad::UP | pad::DOWN | pad::LEFT | pad::RIGHT) {
            let cur = self.players[s].cursor;
            let (mut col, mut r) = (cur / GRID_ROWS, cur % GRID_ROWS);
            let grid = self.grid();
            // 0x1C7F2C: step in the pressed direction until a row exists
            // (Edit Map only with own maps); Up first, then Down, Left,
            // Right. Bounded: a full loop comes back to the current row.
            let mut target = self.players[s].state;
            for _ in 0..(2 * GRID_ROWS + 2) {
                if p.rep(pad::UP) {
                    r = (r + 4) % GRID_ROWS;
                } else if p.rep(pad::DOWN) {
                    r = (r + 1) % GRID_ROWS;
                } else {
                    // Left and Right both toggle the column.
                    col = (col + 1) & 1;
                }
                let t = grid[usize::from(col * GRID_ROWS + r)];
                if t == 0xFF || (t == row::EDIT_MAP && !blocks.has_own_maps(side)) {
                    continue;
                }
                target = t;
                break;
            }
            if target != self.players[s].state {
                self.players[s].cursor = r + col * GRID_ROWS;
                ev.push(MenuEvent::Sfx(SFX_CURSOR));
                self.players[s].state = target;
            }
        } else if p.new(pad::START) {
            let o = s ^ 1;
            if self.players[o].state & 0xF0 == mode::READY {
                ev.push(MenuEvent::Sfx(SFX_CONFIRM));
                self.players[o].state &= 0xF;
                self.players[0].state |= mode::MAP;
                self.players[1].state |= mode::MAP;
            } else {
                ev.push(MenuEvent::Sfx(SFX_CONFIRM));
                if self.selector != 0 {
                    self.players[0].state |= mode::MAP;
                    self.players[1].state |= mode::MAP;
                } else {
                    self.players[s].state |= mode::READY;
                }
            }
        } else if p.new(pad::CROSS | pad::SQUARE) {
            ev.push(MenuEvent::Sfx(SFX_CONFIRM));
            match self.players[s].state {
                row::SAVE => ev.push(MenuEvent::Save),
                row::BUILD_DECK => ev.push(MenuEvent::BuildDeck),
                _ => {}
            }
            self.players[s].state |= mode::EDIT;
        } else if p.new(pad::CIRCLE) {
            if self.players[s ^ 1].state < 8 {
                ev.push(MenuEvent::Sfx(SFX_CANCEL));
                self.players[0].state |= mode::EXIT;
                self.players[1].state |= mode::EXIT;
                self.exit_yes = false;
            } else {
                ev.push(MenuEvent::Sfx(SFX_BUZZER));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn edit_update(
        &mut self,
        side: u8,
        r: u8,
        p: MenuPad,
        current: u8,
        blocks: &mut impl SetupBlocks,
        card_idle: bool,
        ev: &mut Vec<MenuEvent>,
    ) {
        let s = usize::from(side);
        let o = s ^ 1;
        let back = |me: &mut Self, ev: &mut Vec<MenuEvent>| {
            ev.push(MenuEvent::Sfx(SFX_CANCEL));
            me.players[s].state &= 0xF;
        };
        match r {
            row::SELECT_DECK => {
                if p.rep(pad::LEFT | pad::RIGHT) || p.rep(pad::L1 | pad::R1) {
                    let d = self.players[s].deck;
                    self.players[s].deck = if p.rep(pad::LEFT | pad::L1) { (d + 2) % DECKS } else { (d + 4) % DECKS };
                    ev.push(MenuEvent::Sfx(SFX_CURSOR));
                } else if p.new(pad::CROSS | pad::SQUARE) {
                    let d = self.players[s].deck;
                    if !blocks.deck_playable(side, d) {
                        ev.push(MenuEvent::Sfx(SFX_BUZZER));
                    } else if d != current {
                        ev.push(MenuEvent::Sfx(SFX_CONFIRM));
                        blocks.set_current_deck(side, d);
                        self.reload[s] = true;
                        if self.selector == 1 {
                            // The mirror's block follows the player's choice.
                            blocks.set_current_deck(1, d);
                        }
                    }
                } else if p.new(pad::CIRCLE) {
                    back(self, ev);
                }
            }
            row::LP_SETTING => {
                if p.rep(pad::LEFT | pad::RIGHT) || p.rep(pad::L1 | pad::R1) {
                    let old = self.players[s].lp;
                    let hundreds = (i32::from(old as i16) + 50) / LP_STEP;
                    let v = if p.rep(pad::LEFT | pad::L1) { hundreds - 1 } else { hundreds + 1 } * LP_STEP;
                    // 0x1C7384: below 2 → 1, from 9999 → 9999.
                    let v = v.clamp(i32::from(LP_MIN), i32::from(LP_MAX));
                    self.players[s].lp = v as u16;
                    ev.push(MenuEvent::Sfx(if self.players[s].lp == old { SFX_BUZZER } else { SFX_CURSOR }));
                } else if p.new(pad::CROSS | pad::SQUARE) {
                    let lp = self.players[s].lp;
                    if lp != self.sides[s].lp {
                        ev.push(MenuEvent::Sfx(SFX_CONFIRM));
                        self.sides[s].lp = lp;
                        self.players[s].lp_set = lp;
                        if self.selector != 0 {
                            let cpu = lp.max(CPU_MIN_LP);
                            self.sides[o].lp = cpu;
                            self.players[o].lp_set = cpu;
                        }
                    }
                } else if p.new(pad::CIRCLE) {
                    back(self, ev);
                }
            }
            row::SP_RECHARGE => {
                if p.rep(pad::LEFT | pad::RIGHT) || p.rep(pad::L1 | pad::R1) {
                    let old = self.players[s].sp;
                    self.players[s].sp = if p.rep(pad::LEFT | pad::L1) {
                        old.saturating_sub(1).max(SP_MIN)
                    } else {
                        (old + 1).min(SP_MAX)
                    };
                    ev.push(MenuEvent::Sfx(if self.players[s].sp == old { SFX_BUZZER } else { SFX_CURSOR }));
                } else if p.new(pad::CROSS | pad::SQUARE) {
                    let sp = self.players[s].sp;
                    if sp != self.sides[s].sp {
                        ev.push(MenuEvent::Sfx(SFX_CONFIRM));
                        self.sides[s].sp = sp;
                        if self.selector != 0 {
                            self.sides[o].sp = if sp >= CPU_MIN_SP { sp } else { CPU_LOW_SP };
                        }
                    }
                } else if p.new(pad::CIRCLE) {
                    back(self, ev);
                }
            }
            row::LEADER_ABILITY | row::CARD_DISPLAY => {
                let leader = r == row::LEADER_ABILITY;
                let pl = &mut self.players[s];
                let (shown, set) = if leader { (&mut pl.leader, &mut pl.leader_set) } else { (&mut pl.display, &mut pl.display_set) };
                if p.rep(pad::LEFT | pad::RIGHT) {
                    let old = *shown;
                    *shown = u8::from(!p.rep(pad::LEFT));
                    if *shown != old {
                        ev.push(MenuEvent::Sfx(SFX_CURSOR));
                    }
                } else if p.new(pad::CROSS | pad::SQUARE) {
                    if *shown != *set {
                        *set = *shown;
                        ev.push(MenuEvent::Sfx(SFX_CONFIRM));
                    }
                } else if p.new(pad::CIRCLE) {
                    back(self, ev);
                }
            }
            row::BUILD_DECK => {
                // 0x1C789C: back to the grid as soon as the editor returns.
                self.players[s].state = r;
            }
            row::SAVE => {
                if card_idle {
                    self.players[s].state &= 0xF;
                }
            }
            row::EDIT_MAP => {
                let n = self.own_count.max(1);
                if p.rep(pad::UP | pad::DOWN) {
                    self.own_top = if p.rep(pad::UP) { (self.own_top + n - 1) % n } else { (self.own_top + 1) % n };
                    ev.push(MenuEvent::Sfx(SFX_CURSOR));
                } else if p.new(pad::CROSS | pad::SQUARE) {
                    ev.push(MenuEvent::Sfx(SFX_CONFIRM));
                    ev.push(MenuEvent::EditMap { side, map: (self.own_top + 2) % n });
                } else if p.new(pad::CIRCLE) {
                    back(self, ev);
                }
            }
            _ => {}
        }
    }

    fn exit_update(&mut self, p: MenuPad, ev: &mut Vec<MenuEvent>) {
        if p.new(pad::LEFT | pad::RIGHT) {
            let old = self.exit_yes;
            self.exit_yes = p.new(pad::LEFT);
            if self.exit_yes != old {
                ev.push(MenuEvent::Sfx(SFX_CURSOR));
            }
        } else if p.new(pad::CROSS | pad::SQUARE) {
            ev.push(MenuEvent::Sfx(SFX_CONFIRM));
            if self.exit_yes {
                ev.push(MenuEvent::Title);
            } else {
                self.players[0].state &= 0xF;
                self.players[1].state &= 0xF;
            }
        } else if p.new(pad::CIRCLE) {
            ev.push(MenuEvent::Sfx(SFX_CANCEL));
            self.players[0].state &= 0xF;
            self.players[1].state &= 0xF;
        }
    }

    fn map_update(&mut self, side: u8, p: MenuPad, ev: &mut Vec<MenuEvent>) -> bool {
        let n = self.maps.len().max(1) as u16;
        let top = u16::from(self.map_top);
        if p.rep(pad::L1 | pad::R1) {
            self.map_top = if p.rep(pad::L1) { (top + n - 5) % n } else { (top + 5) % n } as u8;
            ev.push(MenuEvent::Sfx(SFX_CURSOR));
        } else if p.rep(pad::UP | pad::DOWN) {
            self.map_top = if p.rep(pad::UP) { (top + n - 1) % n } else { (top + 1) % n } as u8;
            ev.push(MenuEvent::Sfx(SFX_CURSOR));
        } else if p.new(pad::CROSS | pad::SQUARE) {
            let map = entry_map(self.chosen_entry());
            let flags = |a: u8, b: u8| u8::from(a != 0) | (u8::from(b != 0) << 1);
            ev.push(MenuEvent::Start(DuelStart {
                map,
                first: 0,
                identities: self.sides.map(|s| 1000 + i32::from(s.control)),
                leader_abilities: flags(self.players[0].leader, self.players[1].leader),
                card_display: flags(self.players[0].display, self.players[1].display),
                sides: self.sides,
            }));
            ev.push(MenuEvent::Sfx(SFX_CONFIRM));
            return true;
        } else if p.new(pad::CIRCLE) {
            ev.push(MenuEvent::Sfx(SFX_CANCEL));
            self.players[0].state &= 0xF;
            self.players[1].state &= 0xF;
            if self.selector == 0 {
                self.players[usize::from(side) ^ 1].state |= mode::READY;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
