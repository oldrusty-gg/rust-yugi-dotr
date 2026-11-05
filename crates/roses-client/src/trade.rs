//! Trade (title mode 8): the two-player card exchange's model (Claude, 6
//! Oct 2026). Bevy-free; the scene is `trade_render.rs` in the game binary.
//! Notes, disassembly and interpreter runs: the project notes §2 (`d_trade_1c3050.txt`, `tools/run_trade.py`).
//!
//! Entry: the title's Trade runs memory-card op 4 (both slots loaded,
//! `memory_card::CardDialog::trade_load`); on 1 mode 8 is pushed. Init
//! 0x1C3050 clears both export lists, sets each side's sort keys to [0, 4,
//! 10], the state to 1 and builds both lists (0x1C3230). Update 0x1C3470
//! advances the background's two scroll phases, then — only while the card
//! module is idle — runs 0x1C3650(0) and, unless that started the save,
//! 0x1C3650(1): each player drives his own side with his own pad (port =
//! side) [F].
//!
//! Each side ([`TradeSide`]): a list of the 854 card ids (present = the id,
//! else 999) in one of three views — 0 the side's trunk copies (0x216250(s,
//! c, 3) > 0), 1 the cards it exports (its export counts), 2 the cards it
//! imports (the other side's export counts) — sorted with the deck
//! editor's comparators; a 6-row window (top, cursor); the export count per
//! card (0..= trunk copies). YES on "Trade card data?" (state 6) nets the
//! two export lists card by card, moves side 0's copies, then side 1's
//! (0x1C42E0): a copy moves only when the destination holds fewer than 9
//! copies and no instance with its 30-bit id (MSG 129's two conditions); the
//! copy keeps its owner key and id (0x215A10), the source slot is freed
//! (0x214760). Anything left unmoved shows MSG 129 and leaves without
//! saving; otherwise op 5 writes both cards, then the mode pops [F].
use roses_core::card_block::{CardBlock, CardInstance, INSTANCE_ID_MASK, INSTANCE_SLOTS, OWNED_BIT, OWNER_KEY_MASK};
use roses_core::deck_editor::{self, StaticCards, TRUNK_CATEGORY};
use serde::Serialize;

/// PS2 pad bits (0x203230).
pub use crate::custom_duel::pad;
/// L3 / R3 (switch the list's view).
pub const L3: u16 = 0x0200;
pub const R3: u16 = 0x0400;

/// Card ids per list (0x356).
pub const CARDS: usize = roses_core::CARD_COUNT as usize;
pub const EMPTY: u16 = roses_core::card_block::EMPTY_CARD;
/// Rows the list shows.
pub const ROWS: u8 = 6;
/// Copies a card record holds (the destination limit).
pub const MAX_COPIES: usize = INSTANCE_SLOTS;
/// Detail pane slide step and end (0x1C37B8, 0x1C3E24).
pub const SLIDE_STEP: i16 = 0x2D;
pub const SLIDE_END: i16 = 0x100;
/// Idle timer value set while any button is held (0x1C375C).
pub const IDLE_FRAMES: u16 = 0x5A;
/// MSG 126 "Trade card data", 127 "Cancel settings and return to Title
/// Screen", 128 "Are you sure?", 129 "Cannot trade due to …".
pub const MSG_TRADE: u16 = 126;
pub const MSG_CANCEL: u16 = 127;
pub const MSG_SURE: u16 = 128;
pub const MSG_CANNOT: u16 = 129;

/// Per-side state byte (0x2DB85E + side), jump table 0x2BDE10.
pub mod state {
    pub const SLIDE_IN: u8 = 0;
    pub const LIST: u8 = 1;
    pub const SLIDE_OUT: u8 = 2;
    pub const DETAIL: u8 = 3;
    pub const READY: u8 = 4;
    pub const CANCEL_ASK: u8 = 5;
    pub const TRADE_ASK: u8 = 6;
    pub const SAVING: u8 = 7;
    pub const REFUSED: u8 = 8;
}

/// SE ids.
pub mod sfx {
    pub const CURSOR: u16 = 3;
    pub const CONFIRM: u16 = 4;
    pub const CANCEL: u16 = 5;
    pub const BUZZER: u16 = 6;
    pub const OPEN: u16 = 0x10;
    pub const CLOSE: u16 = 0x11;
    pub const PAGE: u16 = 0x13;
}

/// One player's pad this update (0x203380 kinds 0 new, 1 held, 2 repeat;
/// 0x203500 / 0x203580(port, 3): the left stick's Y when analog).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TradePad {
    pub pressed: u16,
    pub held: u16,
    /// New presses and auto-repeats.
    pub repeat: u16,
    pub analog: Option<u8>,
}

impl TradePad {
    /// A single new press (held and repeated this update).
    pub fn press(bits: u16) -> Self {
        Self { pressed: bits, held: bits, repeat: bits, analog: None }
    }
    fn new(&self, m: u16) -> bool {
        self.pressed & m != 0
    }
    fn rep(&self, m: u16) -> bool {
        (self.repeat | self.pressed) & m != 0
    }
    fn hold(&self, m: u16) -> bool {
        self.held & m != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TradeSide {
    /// 0x2DB85E + s.
    pub state: u8,
    /// 0x2DB860 + s: 0 trunk, 1 exports, 2 imports.
    pub view: u8,
    /// 0x2DB8FE + s: the row (0..=5).
    pub cursor: u8,
    /// 0x2DB85C + s: the detail page (0..=3).
    pub page: u8,
    /// 0x2DB856 / 0x2DB859: the three sort keys.
    pub keys: [u8; 3],
    /// 0x2DB82A / 0x2DB82E: the window's top entry.
    pub top: u16,
    /// 0x2DB82C / 0x2DB830: entries present.
    pub count: u16,
    /// 0x2DB832 / 0x2DB834: the detail pane's slide (0..=0x100).
    pub slide: i16,
    /// 0x2DB836 / 0x2DB838.
    pub idle: u16,
    /// 0x1ED0000 / 0x1ED06AC: 854 entries.
    pub list: Vec<u16>,
    /// 0x1ED0D58 / 0x1ED10AE: export count per card.
    pub exports: Vec<u8>,
}

impl TradeSide {
    fn new() -> Self {
        Self {
            state: state::LIST,
            view: 0,
            cursor: 0,
            page: 0,
            keys: [0, 4, 10],
            top: 0,
            count: 0,
            slide: 0,
            idle: 0,
            list: vec![EMPTY; CARDS],
            exports: vec![0; CARDS],
        }
    }

    /// The card under the cursor (999 for an empty row).
    pub fn card(&self) -> u16 {
        self.list.get(usize::from(self.top) + usize::from(self.cursor)).copied().unwrap_or(EMPTY)
    }

    /// The total exported copies.
    pub fn exported(&self) -> u32 {
        self.exports.iter().map(|&n| u32::from(n)).sum()
    }
}

/// What an update asks of the frontend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum TradeEvent {
    Sfx(u16),
    /// 0x1A9290: back to the title.
    Title,
    /// YES with every copy moved: memory-card op 5 (both cards); the mode
    /// pops once it is idle again (state 7).
    Save,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Trade {
    pub sides: [TradeSide; 2],
    /// 0x2DB8D6: YES chosen on the open question.
    pub yes: bool,
    /// 0x2DB826 / 0x2DB828: the background's scroll phases (0..=0x8000).
    pub scroll: [u16; 2],
    /// 0x2DB904 bits 0..1 / 2..3: their directions (+1 / −1).
    pub scroll_dir: [i8; 2],
    /// d63 (0x2DB903) as the previous mode left it: non-zero holds 0x2DB826
    /// rising (0x1C3484); Trade never writes it [F].
    #[serde(skip)]
    pub d63: u8,
}

/// 0x216250(side, card, 3): trunk copies.
pub fn trunk_copies(block: &CardBlock, card: u16) -> usize {
    deck_editor::count_copies(block, card, Some(TRUNK_CATEGORY))
}

impl Trade {
    /// Init 0x1C3050 (both blocks loaded by op 4). The scroll phases are
    /// not reset by the mode [F]; they start at 0, rising [H].
    pub fn new(blocks: &[CardBlock; 2], cards: &StaticCards) -> Self {
        let mut t = Self {
            sides: [TradeSide::new(), TradeSide::new()],
            yes: false,
            scroll: [0, 0],
            scroll_dir: [1, 1],
            d63: 0,
        };
        t.rebuild(0, blocks, cards);
        t.rebuild(1, blocks, cards);
        t
    }

    /// 0x1C3230(side): the list of the current view, the window clamped,
    /// then sorted (0x1C4460).
    pub fn rebuild(&mut self, side: usize, blocks: &[CardBlock; 2], cards: &StaticCards) {
        let other = self.sides[side ^ 1].exports.clone();
        let s = &mut self.sides[side];
        s.count = 0;
        for c in 0..CARDS {
            let present = match s.view {
                0 => trunk_copies(&blocks[side], c as u16) > 0,
                1 => s.exports[c] > 0,
                _ => other[c] > 0,
            };
            s.list[c] = if present { c as u16 } else { EMPTY };
            s.count += u16::from(present);
        }
        if s.count < s.top + u16::from(s.cursor) {
            s.top = s.count.saturating_sub(u16::from(ROWS));
        }
        self.sort(side, blocks, cards);
    }

    /// 0x1C4460(side): qsort with 0x1BB320 (side 0) / 0x1BB400 (side 1),
    /// the side's keys and block (D5C = side).
    pub fn sort(&mut self, side: usize, blocks: &[CardBlock; 2], cards: &StaticCards) {
        let keys = self.sides[side].keys;
        self.sides[side]
            .list
            .sort_by(|&a, &b| deck_editor::compare_keys(&keys, &blocks[side], cards, a, b));
    }

    /// Update 0x1C3470. `card_idle`: 0x1ABB20 < 0.
    pub fn update(
        &mut self,
        pads: [TradePad; 2],
        blocks: &mut [CardBlock; 2],
        cards: &StaticCards,
        card_idle: bool,
    ) -> Vec<TradeEvent> {
        // The two triangle waves (±0x200 and ±0x800, 0..=0x8000); d63 first
        // forces the first one rising (0x1C3484..0x1C34A4).
        if self.d63 != 0 {
            self.scroll_dir[0] = 1;
        }
        for (k, step) in [(0usize, 0x200i32), (1, 0x800)] {
            let v = (i32::from(self.scroll[k]) + i32::from(self.scroll_dir[k]) * step).clamp(0, 0x8000);
            self.scroll[k] = v as u16;
            if v == 0 || v >= 0x8000 {
                self.scroll_dir[k] = -self.scroll_dir[k];
            }
        }
        let mut ev = Vec::new();
        if card_idle && !self.player(0, pads[0], blocks, cards, &mut ev) {
            self.player(1, pads[1], blocks, cards, &mut ev);
        }
        ev
    }

    /// 0x1C3650(side): true when the save was started.
    fn player(&mut self, side: usize, p: TradePad, blocks: &mut [CardBlock; 2], cards: &StaticCards, ev: &mut Vec<TradeEvent>) -> bool {
        let o = side ^ 1;
        let se = |ev: &mut Vec<TradeEvent>, id| ev.push(TradeEvent::Sfx(id));
        if self.sides[side].state < 4 {
            let s = &mut self.sides[side];
            if p.held != 0 {
                s.idle = IDLE_FRAMES;
            } else if s.idle > 0 {
                s.idle -= 1;
            }
        }
        match self.sides[side].state {
            state::SLIDE_IN => {
                let s = &mut self.sides[side];
                s.slide = (s.slide - SLIDE_STEP).max(0);
                if s.slide <= 0 {
                    s.state = state::LIST;
                }
            }
            state::LIST => self.list_update(side, p, blocks, cards, ev),
            state::SLIDE_OUT => {
                let s = &mut self.sides[side];
                s.slide = (s.slide + SLIDE_STEP).min(SLIDE_END);
                if s.slide >= SLIDE_END {
                    s.state = state::DETAIL;
                }
            }
            state::DETAIL => {
                let s = &mut self.sides[side];
                if p.rep(pad::UP | pad::DOWN) {
                    let old = s.page;
                    s.page = if p.rep(pad::UP) { (s.page + 3) % 4 } else { (s.page + 1) % 4 };
                    if s.page != old {
                        se(ev, sfx::PAGE);
                    }
                } else if p.new(pad::CIRCLE | pad::TRIANGLE) {
                    s.state = state::SLIDE_IN;
                    se(ev, sfx::CLOSE);
                }
            }
            state::READY => {
                if p.new(pad::CIRCLE) {
                    se(ev, sfx::CANCEL);
                    self.sides[side].state = state::LIST;
                }
            }
            st @ (state::CANCEL_ASK | state::TRADE_ASK) => {
                if p.new(pad::LEFT | pad::RIGHT) {
                    let old = self.yes;
                    self.yes = p.new(pad::LEFT);
                    if self.yes != old {
                        se(ev, sfx::CURSOR);
                    }
                } else if p.new(pad::CROSS | pad::SQUARE) {
                    se(ev, sfx::CONFIRM);
                    match (st, self.yes) {
                        (state::CANCEL_ASK, true) => ev.push(TradeEvent::Title),
                        (state::CANCEL_ASK, false) => self.both(state::LIST),
                        (_, true) => {
                            let left = commit(&mut self.sides, blocks);
                            if left > 0 {
                                self.both(state::REFUSED);
                            } else {
                                self.both(state::SAVING);
                                ev.push(TradeEvent::Save);
                                return true;
                            }
                        }
                        (_, false) => {
                            self.sides[side].state = state::LIST;
                            self.sides[o].state = state::READY;
                        }
                    }
                } else if p.new(pad::CIRCLE) {
                    se(ev, sfx::CANCEL);
                    if st == state::CANCEL_ASK {
                        self.both(state::LIST);
                    } else {
                        self.sides[side].state = state::LIST;
                        self.sides[o].state = state::READY;
                    }
                }
            }
            state::SAVING => ev.push(TradeEvent::Title),
            state::REFUSED => {
                if p.new(pad::TRIANGLE | pad::CIRCLE | pad::CROSS | pad::SQUARE) {
                    se(ev, sfx::CONFIRM);
                    ev.push(TradeEvent::Title);
                }
            }
            _ => {}
        }
        false
    }

    fn both(&mut self, st: u8) {
        self.sides[0].state = st;
        self.sides[1].state = st;
    }

    fn list_update(&mut self, side: usize, p: TradePad, blocks: &mut [CardBlock; 2], cards: &StaticCards, ev: &mut Vec<TradeEvent>) {
        let o = side ^ 1;
        let se = |ev: &mut Vec<TradeEvent>, id| ev.push(TradeEvent::Sfx(id));
        let card = self.sides[side].card();
        let fp = usize::from(p.hold(pad::SELECT));
        if self.sides[side].count != 0 {
            let s = &mut self.sides[side];
            let old_cursor = s.cursor;
            let mut step: i32 = 0;
            if p.rep(pad::L1 | pad::R1) {
                step = if p.rep(pad::L1) { -6 } else { 6 };
            } else if p.rep(pad::UP | pad::DOWN) {
                if p.rep(pad::UP) {
                    if s.cursor > 0 {
                        s.cursor -= 1;
                    } else {
                        step = -1;
                    }
                } else if s.cursor < ROWS - 1 {
                    s.cursor += 1;
                } else {
                    step = 1;
                }
            }
            if s.cursor != old_cursor {
                se(ev, sfx::CURSOR);
                return;
            }
            if step == 0 {
                // 0x1C3918: the left stick past ±0x30 scrolls.
                if let Some(a) = p.analog {
                    let v = i32::from(a) - 0x80;
                    if v.abs() > 0x30 {
                        let v = if v <= 0 { v + 0x30 } else { v - 0x30 };
                        step = (v * 18) >> 7;
                    }
                }
            }
            if step != 0 {
                let max_top = (i32::from(s.count) - 6).max(0);
                let old = s.top;
                s.top = (i32::from(s.top) + step).clamp(0, max_top) as u16;
                if s.top != old {
                    se(ev, sfx::CURSOR);
                }
                s.idle = IDLE_FRAMES;
                return;
            }
            if p.rep(pad::CROSS | pad::SQUARE) {
                if card == EMPTY || s.view == 2 {
                    se(ev, sfx::BUZZER);
                    return;
                }
                let max = trunk_copies(&blocks[side], card) as u8;
                let up = if side == 0 { pad::CROSS } else { pad::SQUARE };
                let g = &mut s.exports[usize::from(card)];
                let old = *g;
                if p.rep(up) {
                    if *g < max {
                        *g += 1;
                    }
                } else if *g > 0 {
                    *g -= 1;
                }
                se(ev, if *g != old { sfx::CURSOR } else { sfx::BUZZER });
                return;
            }
            if p.new(pad::TRIANGLE) {
                if card == EMPTY {
                    se(ev, sfx::BUZZER);
                } else {
                    s.state = state::SLIDE_OUT;
                    se(ev, sfx::OPEN);
                }
                return;
            }
        }
        if p.new(L3 | R3) {
            let s = &mut self.sides[side];
            s.view = if p.new(L3) { (s.view + 2) % 3 } else { (s.view + 4) % 3 };
            self.rebuild(side, blocks, cards);
            self.sides[side].top = 0;
            se(ev, sfx::PAGE);
        } else if p.new(pad::START) {
            if self.sides[o].state != state::READY {
                se(ev, sfx::CONFIRM);
                self.sides[side].state = state::READY;
            } else if self.sides[side].exported() > 0 || self.sides[o].exported() > 0 {
                se(ev, sfx::CONFIRM);
                self.both(state::TRADE_ASK);
                self.yes = false;
            } else {
                se(ev, sfx::BUZZER);
            }
        } else if p.rep(pad::L2 | pad::R2) {
            let s = &mut self.sides[side];
            let old = s.keys[fp];
            s.keys[fp] = if p.rep(pad::L2) { (old + 9) % 10 } else { (old + 11) % 10 };
            if s.keys[fp] != old {
                self.sort(side, blocks, cards);
                se(ev, sfx::PAGE);
            }
        } else if p.new(pad::CIRCLE) {
            if self.sides[o].state == state::LIST {
                se(ev, sfx::CANCEL);
                self.both(state::CANCEL_ASK);
                self.yes = false;
            } else {
                se(ev, sfx::BUZZER);
            }
        }
    }
}

/// 0x1C41F0: net the export counts card by card, move side 0's copies then
/// side 1's (0x1C42E0); returns the copies left unmoved.
pub fn commit(sides: &mut [TradeSide; 2], blocks: &mut [CardBlock; 2]) -> u32 {
    for c in 0..CARDS {
        let (a, b) = (sides[0].exports[c], sides[1].exports[c]);
        if a != 0 && b != 0 {
            if b < a {
                sides[0].exports[c] = a - b;
                sides[1].exports[c] = 0;
            } else {
                sides[1].exports[c] = b - a;
                sides[0].exports[c] = 0;
            }
        }
    }
    for s in 0..2 {
        let [b0, b1] = blocks;
        let (from, to) = if s == 0 { (b0, b1) } else { (b1, b0) };
        move_copies(&mut sides[s].exports, from, to);
    }
    sides.iter().map(TradeSide::exported).sum()
}

/// 0x1C42E0(side) on the two blocks.
fn move_copies(exports: &mut [u8], from: &mut CardBlock, to: &mut CardBlock) {
    for c in 0..CARDS {
        let card = c as u16;
        while exports[c] != 0 {
            if deck_editor::count_copies(to, card, None) >= MAX_COPIES {
                break;
            }
            let found = from.records[c].instances.iter().position(|i| {
                !i.is_free() && deck_editor::instance_category(*i) == TRUNK_CATEGORY && !to.is_duplicate(card, i.random_id())
            });
            let Some(k) = found else { break };
            let inst = from.records[c].instances[k];
            add_instance(to, card, inst.owner & OWNER_KEY_MASK, inst.random_id());
            // 0x214760: owner 0, word 1 = 0xC0000000.
            from.records[c].instances[k] = CardInstance { owner: 0, id: 0xC000_0000 };
            exports[c] -= 1;
        }
    }
}

/// 0x215A10(side, key, id, card): the first free slot takes the key (0 =
/// the block's) and the id, keeping the slot's top bits; the owned bit is
/// set and the card moves to the front of the recent list.
fn add_instance(block: &mut CardBlock, card: u16, key: u32, id: u32) -> bool {
    let key = if key == 0 { block.owner_key & OWNER_KEY_MASK } else { key };
    if key == 0 || block.is_duplicate(card, id) {
        return false;
    }
    let Some(record) = block.records.get_mut(usize::from(card)) else { return false };
    let Some(slot) = record.instances.iter_mut().find(|i| i.is_free()) else { return false };
    slot.id = (slot.id & !INSTANCE_ID_MASK) | (id & INSTANCE_ID_MASK);
    slot.owner = (slot.owner & !OWNER_KEY_MASK) | key;
    record.rank_word |= OWNED_BIT;
    block.touch_recent(card);
    true
}

#[cfg(test)]
mod tests;
