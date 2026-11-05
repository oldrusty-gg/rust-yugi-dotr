//! The reward slot machine after a won campaign duel (mode 0xF "Slot
//! Machine", MSG 98), as a pure frame-stepped model.
//!
//! Sources (PAL SLES_515.79, private research the project notes §3 and
//! the interpreter runs `run2.py`/`run3.py`; EE addresses): reel data
//! 0x252980/0x252C90, mode init 0x284E60, spin start 0x281580, per-frame
//! input 0x2852B0 (the mode update) followed by physics 0x2849A0 (the draw
//! pass), line evaluation 0x281930, flash idle test 0x281FB0, prize order
//! 0x2816C0, special mapping 0x252E10, popup/grant 0x1E5540 and popup close
//! 0x1E57B0. Provenance marks: [F] read from the code, [emu] confirmed in the
//! EE interpreter, [H] hypothesis.
//!
//! Driving it: the duel's result state 0x62 builds [`ReelData`] with
//! [`build_reel_data`] (nothing on a loss, a draw or an empty list: the game
//! pops straight back to the map). [`SlotMachine::new`] is 0x284E60 plus
//! 0x281580 (the reels start spinning at once, SFX 0x1E). Then, once per
//! frame, [`SlotMachine::step`] with the buttons newly pressed on that frame
//! (0x203380 mode 0: current & !previous of the pad word); it returns the
//! frame's [`SlotEvent`]s. The frontend draws three reels of three rows
//! ([`Reel::rows`] once stopped; while moving, the strip scrolled by
//! [`Reel::pos`]/[`Reel::scroll`], whose drawing routines 0x283910/0x2828C0
//! are not decoded [H]), the five pay lines ([`LINE_ROWS`]), the flashing
//! line/cells ([`FlashState`]) and the prize popups ("GET CARD" / "GET
//! SPECIAL CARD" banners 0xC/0xD with the card). The shared 64-bit LCG
//! ([`next_rand31`]) is consumed by the shuffle (8·count calls), every stop
//! with the random slip on (1 call), every random special prize (1 call) and
//! the grant (0x2157D0 draws at least once per card; done by the
//! [`PrizeGrant`] callback so the order is kept). The main loop also draws
//! two calls per frame after update and draw (DUEL-END §5.5); the caller
//! does that between `step`s.
//!
//! Test mode (mode parameter 15 ≠ 0: random reels, screen 0 with a start
//! key, no grants) is not modelled ([`SlotMachine::new_test_mode`] refuses).

use crate::duel::next_rand31;
use crate::CARD_COUNT;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 0x334 Fake Trap: pads short lists to 8 and never pays a line [F].
pub const FILLER_CARD: u16 = 0x334;
/// A reel's card before it stops; an absent prize (0x2816C0) [F].
pub const NO_CARD: u16 = 0xFFFF;
/// 0x252E10's answer when its random pool is empty (0x3E7) [F].
pub const NO_PRIZE: u16 = 0x3E7;
/// Card id refused by the graveyard append 0x1E00F0 and the record lookup
/// 0x1DE350 [F].
pub const REJECTED_CARD: u16 = 0x29F;
/// Graveyard entries (side +0x232) [F].
pub const GRAVEYARD_CAPACITY: usize = 0x78;
/// Reel position units per card (0x2849A0, 0x281930) [F].
pub const CARD_UNITS: i16 = 128;
/// The second position (+0x14) wraps at 0x160 by subtracting 0x15F [F].
pub const SCROLL_WRAP: i16 = 0x160;
pub const SCROLL_STEP: i16 = 0x15F;
/// Strip length bounds of 0x252C90 [F].
pub const MIN_STRIP: usize = 8;
pub const MAX_STRIP: usize = 16;
/// Full speed of a spinning reel [F].
pub const FULL_SPEED: i16 = 32;
pub const LINE_COUNT: usize = 5;
/// Row shown by each reel on each pay line (0x281930; reel 0's row is the
/// table u32[5] at 0x2B7B50 used by 0x2816C0): 0 middle, 1 bottom, 2 top,
/// 3 falling diagonal, 4 rising diagonal [F].
pub const LINE_ROWS: [[usize; 3]; LINE_COUNT] =
    [[1, 1, 1], [2, 2, 2], [0, 0, 0], [2, 1, 0], [0, 1, 2]];
/// Leader types (card byte 0 & 0x1F) without leader ability 0 "Slot
/// Machine": only line 0 pays (0x252B54..) [F].
pub const TYPES_WITHOUT_SLOT_ABILITY: [u8; 10] = [0, 2, 5, 6, 7, 9, 12, 16, 19, 20];
/// Stored leader rank needed for lines 1..=4 (0x252C08..) [F].
pub const LINE_RANKS: [i32; 4] = [4, 7, 10, 11];
/// The fixed pairs of 0x252E10 (reel card → special card) [F].
pub const SPECIAL_MAP: [(u16, u16); 8] = [
    (61, 733),
    (722, 811),
    (539, 700),
    (272, 702),
    (716, 715),
    (754, 699),
    (837, 83),
    (60, 87),
];
/// Countdown of the prize screen before the first prize (0x2858A8) and of
/// the close after the last one (0x284E08) [F].
pub const PRIZE_DELAY_FRAMES: u8 = 0x10;
pub const CLOSE_DELAY_FRAMES: u8 = 8;
/// Flash counter of a paying line and its cells (0x2858C8) [F].
pub const FLASH_FRAMES: u8 = 0x10;

/// Pad bits as 0x203380 sees them [F].
pub mod buttons {
    pub const TRIANGLE: u8 = 0x10;
    pub const CIRCLE: u8 = 0x20;
    pub const CROSS: u8 = 0x40;
    pub const SQUARE: u8 = 0x80;
    /// Any of the four face buttons (closes a prize popup).
    pub const ANY_FACE: u8 = 0xF0;
    /// The button that stops reel 0, 1, 2 (0x2853C8, 0x285540, 0x2856B8).
    pub const STOP: [u8; 3] = [SQUARE, TRIANGLE, CIRCLE];
}

/// Sound effects 0x216EA0(0, n) and the screen cue 0x217050(0, n) [F].
pub mod sfx {
    pub const SPIN_START: u8 = 0x1E;
    pub const REEL_STOP: u8 = 0x21;
    pub const ALL_STOPPED: u8 = 0x1E;
    pub const GET_CARD: u8 = 7;
    pub const GET_SPECIAL_CARD: u8 = 0x19;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotMachineError {
    /// The strip must hold 1..=16 cards (0x252C90 never makes another).
    InvalidStrip(usize),
    Unsupported(String),
}
impl fmt::Display for SlotMachineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStrip(n) => write!(f, "reel strip of {n} cards is impossible"),
            Self::Unsupported(m) => write!(f, "not yet supported: {m}"),
        }
    }
}
impl std::error::Error for SlotMachineError {}

/// Byte 0xB of the card records (roses-assets `parse_pal_reward_flags`).
pub trait RewardCardFlags {
    /// Bit 5: the card may appear on a reel.
    fn reel_eligible(&self, card: u16) -> bool;
    /// Bit 6: the card is in 0x252E10's random pool.
    fn special_pool(&self, card: u16) -> bool;
}

/// Flag bits per card id, imported from the executable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardCardTable {
    pub reel_eligible: Vec<bool>,
    pub special_pool: Vec<bool>,
}
impl RewardCardTable {
    /// From byte 0xB of each record in id order.
    pub fn from_record_flag_bytes(bytes: impl IntoIterator<Item = u8>) -> Self {
        let mut t = Self::default();
        for b in bytes {
            t.reel_eligible.push(b & 0x20 != 0);
            t.special_pool.push(b & 0x40 != 0);
        }
        t
    }
}
impl RewardCardFlags for RewardCardTable {
    fn reel_eligible(&self, card: u16) -> bool {
        self.reel_eligible
            .get(usize::from(card))
            .copied()
            .unwrap_or(false)
    }
    fn special_pool(&self, card: u16) -> bool {
        self.special_pool
            .get(usize::from(card))
            .copied()
            .unwrap_or(false)
    }
}

/// What 0x252980(loser, buffer) reads at result state 0x62 (human won).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardListInput {
    /// The loser's graveyard in removal order (side +0x232).
    pub loser_graveyard: Vec<u16>,
    /// The loser's deck leader card (0x215160(loser, 0x215040(loser))).
    pub loser_deck_leader: u16,
    /// The winner's deck leader card byte 0 (0x271610 keeps bits 0..=4).
    pub winner_leader_kind: u8,
    /// The winner's leader's stored rank (0x2154C0; `None` = −1, no rank).
    pub winner_leader_rank: Option<u8>,
    /// Winner's side byte +0x344 bit 7 (0x1DF9C0): the special draw was
    /// announced, which turns the random slip off.
    pub winner_special_draw_announced: bool,
}

/// The reel buffer at 0x1ED0000: B[0] count, B[1] slip flag, the strip
/// (three identical copies at B+8+r·0x2A) and the line words B[0x86+2k].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReelData {
    /// 8..=16 cards; reel 0 shows it in this order.
    pub strip: Vec<u16>,
    /// B[1] = 2: each stop adds 0, 1 or 2 random extra cards.
    pub random_slip: bool,
    /// B[0x86+2k] == 0xFFFE. Line 0 is forced on again by 0x284E60, so the
    /// value here can be false only for a rank ≤ 0 leader with the ability.
    pub enabled_lines: [bool; LINE_COUNT],
}
impl ReelData {
    pub fn count(&self) -> u8 {
        self.strip.len() as u8
    }
}

/// 0x252980 + 0x252C90. `None` is the return value 0: no slot machine, the
/// duel mode pops back to the map. [F][emu: 767, list 817 406 757 809 397
/// padded with 820 to 8].
pub fn build_reel_data(input: &RewardListInput, flags: &impl RewardCardFlags) -> Option<ReelData> {
    // 0x1E00F0 appends the leader: card 0x29F and ids ≥ 0x356 are refused,
    // as is a full graveyard (count ≥ 0x78).
    let mut graveyard = input.loser_graveyard.clone();
    let leader = input.loser_deck_leader;
    if leader != REJECTED_CARD
        && usize::from(leader) < usize::from(CARD_COUNT)
        && graveyard.len() < GRAVEYARD_CAPACITY
    {
        graveyard.push(leader);
    }
    if graveyard.is_empty() {
        return None;
    }
    // Removal 0x1E0280 leaves holes that the next append compacts; every
    // original entry is visited once, so this keeps the order.
    graveyard.retain(|&c| flags.reel_eligible(c));
    if graveyard.is_empty() {
        return None;
    }
    // 0x252C90: ≥ 16 → the first 16; 8..15 → all; < 8 → padded with 0x334.
    let mut strip = graveyard;
    strip.truncate(MAX_STRIP);
    while strip.len() < MIN_STRIP {
        strip.push(FILLER_CARD);
    }
    let mut enabled_lines = [true; LINE_COUNT];
    let kind = input.winner_leader_kind & 0x1F;
    if TYPES_WITHOUT_SLOT_ABILITY.contains(&kind) {
        enabled_lines[1..].fill(false);
    } else {
        let rank = input.winner_leader_rank.map_or(-1, i32::from);
        enabled_lines[0] = rank > 0;
        for (line, need) in enabled_lines[1..].iter_mut().zip(LINE_RANKS) {
            *line = rank >= need;
        }
    }
    Some(ReelData {
        strip,
        random_slip: !input.winner_special_draw_announced,
        enabled_lines,
    })
}

/// 0x252E10's random pool: every card with record bit 6 that is not one of
/// the eight fixed results (card 0x29F has no record and is skipped) [F].
pub fn special_pool(flags: &impl RewardCardFlags) -> Vec<u16> {
    (0..CARD_COUNT)
        .filter(|&c| c != REJECTED_CARD && flags.special_pool(c))
        .filter(|c| !SPECIAL_MAP.iter().any(|&(_, result)| result == *c))
        .collect()
}

/// 0x252E10(card): the fixed pair, else `pool[rand() % len]` (one LCG
/// call), else [`NO_PRIZE`] for an empty pool [F][emu: 809 → pool[18]].
pub fn special_prize(card: u16, pool: &[u16], rng: &mut u64) -> u16 {
    if let Some(&(_, result)) = SPECIAL_MAP.iter().find(|&&(from, _)| from == card) {
        return result;
    }
    if pool.is_empty() {
        return NO_PRIZE;
    }
    pool[(next_rand31(rng) % pool.len() as u32) as usize]
}

/// Reel state byte (+0xA) [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReelState {
    /// Before 0x281580 (only reachable in test mode).
    Idle = 0,
    /// Speed +1 per frame up to 32.
    Accelerating = 1,
    /// 32 units per frame; a stop button is accepted.
    Spinning = 2,
    /// Counting the stop counter down.
    Stopping = 3,
    Stopped = 4,
}

/// One reel (0x314940 + r·0x3A) [F].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reel {
    pub state: ReelState,
    /// +0xB: the stop counter of state 3.
    pub counter: u8,
    /// +0x12: strip position in 1/128 cards, 0..=128·count (wraps).
    pub pos: i16,
    /// +0x14: a second position advanced by the same speed that wraps at
    /// 0x160 by 0x15F; read by the reel drawing only (meaning untraced) [H].
    pub scroll: i16,
    /// +0x16: units per frame.
    pub speed: i16,
    /// +0x18: the middle row once stopped, else [`NO_CARD`].
    pub card: u16,
    /// +0x1A: the cards of this reel in strip order.
    pub strip: Vec<u16>,
}
impl Reel {
    fn new(strip: Vec<u16>) -> Self {
        Self {
            state: ReelState::Idle,
            counter: 0,
            pos: 0,
            scroll: 0,
            speed: 0,
            card: NO_CARD,
            strip,
        }
    }
    /// The strip index at the top row (0x281930: trunc(pos / 128) mod count).
    pub fn top_index(&self) -> usize {
        (ftoi(self.pos as f32 / CARD_UNITS as f32) as usize) % self.strip.len()
    }
    /// The three rows (top, middle, bottom) of a stopped reel, else
    /// [`NO_CARD`] each (0x281930).
    pub fn rows(&self) -> [u16; 3] {
        if self.state != ReelState::Stopped {
            return [NO_CARD; 3];
        }
        let i = self.top_index();
        let n = self.strip.len();
        [
            self.strip[i % n],
            self.strip[(i + 1) % n],
            self.strip[(i + 2) % n],
        ]
    }
}

/// Screen byte 0x314940 [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    /// Waiting for the start key (test mode only; never entered here).
    Start = 0,
    Spinning = 1,
    /// Every paying line flashes once, one after the other.
    LineFlash = 2,
    /// The prize popups.
    Prizes = 3,
    /// The mode pops on the next update.
    Done = 4,
}

/// Sub-state byte 0x314878 of the prize screen [F].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrizePhase {
    /// 16-frame wait before the first prize.
    Delay = 0,
    /// The draw pass of this frame picks the next prize.
    Draw = 1,
    /// A popup is shown; any face button continues.
    Popup = 2,
    /// 8-frame wait after the last popup, then [`Screen::Done`].
    Closing = 6,
}

/// The flash counters 0x314910.. : a line and its three cells count down
/// from 16 to 1; the flash is over when every counter is below 2 [F].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlashState {
    /// 0x314910: set by the first flash, never cleared.
    pub started: bool,
    /// 0x314914 + k. Stays 1 after the flash: 0x2816C0 reads it as "line
    /// k paid".
    pub lines: [u8; LINE_COUNT],
    /// 0x314919 + 3·reel + row.
    pub cells: [[u8; 3]; 3],
}
impl FlashState {
    /// 0x281FB0.
    pub fn idle(&self) -> bool {
        self.lines.iter().all(|&c| c < 2) && self.cells.iter().flatten().all(|&c| c < 2)
    }
    fn decrement(&mut self) {
        for c in self.lines.iter_mut().chain(self.cells.iter_mut().flatten()) {
            if *c >= 2 {
                *c -= 1;
            }
        }
    }
}

/// The prize screen words 0x314878..0x31487C [F].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrizeStage {
    pub phase: PrizePhase,
    /// 0x314879.
    pub wait: u8,
    /// 0x31487A: the next entry of the prize order (0..=8).
    pub index: u8,
    /// 0x31487C: the card of the popup being shown.
    pub card: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrizeKind {
    /// 0x1E5540 kind 2, banner 0xC "GET CARD" (SFX 7): a reel's card.
    GetCard,
    /// Kind 3, banner 0xD "GET SPECIAL CARD" (SFX 0x19): a line's mapped
    /// card; the grant also cycles the alternate-art counter (0x215D10).
    GetSpecialCard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prize {
    pub kind: PrizeKind,
    /// May be [`NO_PRIZE`] (0x3E7) when the special pool is empty; the
    /// grant ignores invalid ids.
    pub card: u16,
    /// Position in the prize order: 0..=2 the reels, 3..=7 lines 0..=4.
    pub order: u8,
}

/// The card grant 0x1E5540 → 0x2157D0(winner, 0, card) (plus 0x215D10 for
/// [`PrizeKind::GetSpecialCard`]), which belongs to the persistent card
/// block. It draws the LCG (instance key, re-rolled on a duplicate), so it
/// runs inside [`SlotMachine::step`] to keep the call order.
pub trait PrizeGrant {
    fn grant(&mut self, prize: Prize, rng: &mut u64);
}
/// Collects the prizes without touching the LCG (not the original's RNG
/// use; for callers that grant later).
impl PrizeGrant for Vec<Prize> {
    fn grant(&mut self, prize: Prize, _rng: &mut u64) {
        self.push(prize);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlotEvent {
    /// 0x281580: all reels accelerate; SFX 0x1E.
    SpinStarted,
    /// A stop button was accepted (SFX 0x21); the reel stops `counter`
    /// frames later, `slip` of them random extra cards (0, 1 or 2).
    ReelStopping { reel: u8, counter: u8, slip: u8 },
    /// The reel came to rest showing `rows`; `live_lines` are the pay
    /// lines still possible.
    ReelStopped {
        reel: u8,
        rows: [u16; 3],
        live_lines: u8,
    },
    /// Screen 2: 0x217050(0, 0x1E).
    AllStopped { paying_lines: u8 },
    /// Line `line` and its cells flash for 16 frames.
    LineFlash { line: u8 },
    /// Screen 3 (after the 16-frame delay the first popup follows).
    PrizeScreen,
    /// A popup opened (banner 0xC/0xD, SFX 7/0x19) and the card was granted.
    Prize(Prize),
    /// 0x1E57B0: the banner closes (CB2 = 0x18); 8 frames later screen 4.
    PopupClosed,
    /// 0x1A9290: the mode pops back to the campaign map.
    Finished,
}

/// The whole mode state (0x314878..0x3149F0) [F].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotMachine {
    pub screen: Screen,
    /// 0x314941: bit k = line k pays.
    pub enabled_lines: u8,
    /// 0x314942: enabled lines not yet killed by a stopped reel; during
    /// screen 2, the lines still to flash.
    pub live_lines: u8,
    /// 0x1ED0000: cards per strip.
    pub count: u8,
    /// 0x314948: 128·count, the position wrap.
    pub cycle: i16,
    /// 0x314946: frames since the spin started.
    pub frame: u16,
    /// B[1] & 2.
    pub random_slip: bool,
    pub reels: [Reel; 3],
    pub flash: FlashState,
    pub prizes: PrizeStage,
    /// 0x252E10's random pool ([`special_pool`]).
    pub special_pool: Vec<u16>,
    pub finished: bool,
}

/// Float→int truncation toward zero (0x1A2B80 / 0x1A2C70 for the values
/// used here).
fn ftoi(x: f32) -> i32 {
    x as i32
}

impl SlotMachine {
    /// 0x284E60(0) then 0x281580: reel 0 in list order, reels 1 and 2 each
    /// shuffled by 2·count swaps of `(rand() >> 4) % count` pairs (reel 1
    /// first, 8·count LCG calls), line 0 forced on, then the spin starts
    /// [F][emu 767: 64 calls, strips as in the tests].
    pub fn new(
        data: &ReelData,
        special_pool: Vec<u16>,
        rng: &mut u64,
    ) -> Result<(Self, Vec<SlotEvent>), SlotMachineError> {
        let count = data.strip.len();
        if count == 0 || count > MAX_STRIP {
            return Err(SlotMachineError::InvalidStrip(count));
        }
        let mut enabled = data.enabled_lines;
        enabled[0] = true;
        let enabled_lines = enabled
            .iter()
            .enumerate()
            .fold(0u8, |m, (k, &on)| if on { m | (1 << k) } else { m });
        let cycle = ftoi(CARD_UNITS as f32 * count as f32) as i16;
        let mut reels = [
            Reel::new(data.strip.clone()),
            Reel::new(data.strip.clone()),
            Reel::new(data.strip.clone()),
        ];
        for reel in &mut reels[1..] {
            for _ in 0..2 * count {
                let i = ((next_rand31(rng) >> 4) % count as u32) as usize;
                let j = ((next_rand31(rng) >> 4) % count as u32) as usize;
                reel.strip.swap(i, j);
            }
        }
        let mut machine = Self {
            screen: Screen::Start,
            enabled_lines,
            live_lines: 0,
            count: count as u8,
            cycle,
            frame: 0,
            random_slip: data.random_slip,
            reels,
            flash: FlashState::default(),
            prizes: PrizeStage {
                phase: PrizePhase::Delay,
                wait: 0,
                index: 0,
                card: 0,
            },
            special_pool,
            finished: false,
        };
        let mut events = Vec::new();
        machine.start_spin(&mut events);
        Ok((machine, events))
    }

    /// Mode parameter 15 ≠ 0: random strips of 16, all lines on, no grant.
    pub fn new_test_mode() -> Result<Self, SlotMachineError> {
        Err(SlotMachineError::Unsupported(
            "slot machine test mode (0x284E60 with param 15 set)".into(),
        ))
    }

    /// 0x281580.
    fn start_spin(&mut self, events: &mut Vec<SlotEvent>) {
        for reel in &mut self.reels {
            reel.state = ReelState::Accelerating;
        }
        self.screen = Screen::Spinning;
        self.live_lines = self.enabled_lines;
        self.frame = 0;
        events.push(SlotEvent::SpinStarted);
    }

    /// Bit k of `live_lines`/`enabled_lines` as a list of line numbers.
    pub fn lines(mask: u8) -> Vec<u8> {
        (0..LINE_COUNT as u8)
            .filter(|k| mask & (1 << k) != 0)
            .collect()
    }

    /// One frame: the mode update 0x2852B0 with `pressed` (the pad bits that
    /// went down this frame, see [`buttons`]) followed by the draw pass
    /// 0x2849A0. Nothing happens after [`SlotEvent::Finished`].
    pub fn step(
        &mut self,
        pressed: u8,
        rng: &mut u64,
        grant: &mut dyn PrizeGrant,
    ) -> Vec<SlotEvent> {
        let mut events = Vec::new();
        if self.finished {
            return events;
        }
        self.update(pressed, rng, &mut events);
        if self.finished {
            return events;
        }
        self.draw(rng, grant, &mut events);
        events
    }

    fn update(&mut self, pressed: u8, rng: &mut u64, events: &mut Vec<SlotEvent>) {
        match self.screen {
            // Only test mode gets here (start key / stick); nothing to do.
            Screen::Start => {}
            Screen::Spinning => {
                for r in 0..3 {
                    if self.reels[r].state == ReelState::Spinning && pressed & buttons::STOP[r] != 0
                    {
                        let reel = &mut self.reels[r];
                        reel.state = ReelState::Stopping;
                        // c = trunc(5 + (128 − pos % 128) / speed)
                        let m = reel.pos % CARD_UNITS;
                        let mut counter =
                            ftoi(5.0_f32 + (CARD_UNITS as f32 - m as f32) / reel.speed as f32)
                                as u8;
                        let mut slip = 0;
                        if self.random_slip {
                            slip = ((next_rand31(rng) >> 4) % 3) as u8;
                            // c = trunc(c + (128 / speed) · k)
                            counter = ftoi(
                                counter as f32
                                    + (CARD_UNITS as f32 / reel.speed as f32) * slip as f32,
                            ) as u8;
                        }
                        reel.counter = counter;
                        events.push(SlotEvent::ReelStopping {
                            reel: r as u8,
                            counter,
                            slip,
                        });
                    }
                }
                if self.reels.iter().all(|r| r.state == ReelState::Stopped) {
                    self.screen = Screen::LineFlash;
                    events.push(SlotEvent::AllStopped {
                        paying_lines: self.live_lines,
                    });
                }
            }
            Screen::LineFlash => {
                if self.flash.idle() {
                    if self.live_lines & 0x1F == 0 {
                        self.prizes.phase = PrizePhase::Delay;
                        self.screen = Screen::Prizes;
                        self.prizes.wait = PRIZE_DELAY_FRAMES;
                        events.push(SlotEvent::PrizeScreen);
                    } else {
                        let k = (0..LINE_COUNT)
                            .find(|k| self.live_lines & (1 << k) != 0)
                            .unwrap();
                        self.flash.started = true;
                        self.flash.lines[k] = FLASH_FRAMES;
                        for (reel, cells) in self.flash.cells.iter_mut().enumerate() {
                            cells[LINE_ROWS[k][reel]] = FLASH_FRAMES;
                        }
                        self.live_lines &= !(1 << k);
                        events.push(SlotEvent::LineFlash { line: k as u8 });
                    }
                }
            }
            Screen::Prizes => match self.prizes.phase {
                PrizePhase::Delay => {
                    if self.prizes.wait > 0 {
                        self.prizes.wait -= 1;
                    } else {
                        self.prizes.phase = PrizePhase::Draw;
                        self.prizes.index = 0;
                        self.prizes.wait = 8;
                    }
                }
                PrizePhase::Draw => {}
                PrizePhase::Popup => {
                    if pressed & buttons::ANY_FACE != 0 {
                        self.prizes.phase = PrizePhase::Draw;
                        self.prizes.wait = 8;
                    }
                }
                PrizePhase::Closing => {
                    if self.prizes.wait > 0 {
                        self.prizes.wait -= 1;
                    } else {
                        self.screen = Screen::Done;
                    }
                }
            },
            Screen::Done => {
                self.finished = true;
                events.push(SlotEvent::Finished);
            }
        }
    }

    fn draw(&mut self, rng: &mut u64, grant: &mut dyn PrizeGrant, events: &mut Vec<SlotEvent>) {
        for r in 0..3 {
            self.physics(r, events);
        }
        if self.flash.started {
            self.flash.decrement();
        }
        if self.screen == Screen::Prizes && self.prizes.phase == PrizePhase::Draw {
            self.next_prize(rng);
            if self.prizes.card != NO_CARD {
                let prize = Prize {
                    kind: if self.prizes.index < 4 {
                        PrizeKind::GetCard
                    } else {
                        PrizeKind::GetSpecialCard
                    },
                    card: self.prizes.card,
                    order: self.prizes.index - 1,
                };
                grant.grant(prize, rng);
                self.prizes.phase = PrizePhase::Popup;
                self.prizes.wait = 0;
                events.push(SlotEvent::Prize(prize));
            } else {
                self.prizes.phase = PrizePhase::Closing;
                self.prizes.wait = CLOSE_DELAY_FRAMES;
                events.push(SlotEvent::PopupClosed);
            }
        }
        self.frame = self.frame.wrapping_add(1);
    }

    /// The per-reel part of 0x2849A0.
    fn physics(&mut self, r: usize, events: &mut Vec<SlotEvent>) {
        let cycle = self.cycle;
        let reel = &mut self.reels[r];
        let advance = |reel: &mut Reel| {
            reel.pos += reel.speed;
            if reel.pos > cycle {
                reel.pos -= cycle;
            } else if reel.pos < 0 {
                reel.pos += cycle;
            }
            reel.scroll += reel.speed;
            if reel.scroll >= SCROLL_WRAP {
                reel.scroll -= SCROLL_STEP;
            } else if reel.scroll < 0 {
                reel.scroll += SCROLL_STEP;
            }
        };
        match reel.state {
            ReelState::Accelerating => {
                if reel.speed < FULL_SPEED {
                    reel.speed += 1;
                } else {
                    reel.state = ReelState::Spinning;
                }
                advance(reel);
            }
            ReelState::Spinning => advance(reel),
            ReelState::Stopping => {
                match reel.counter {
                    5 => {
                        // The next card boundary, then 16 units past it.
                        let next = ftoi(reel.pos as f32 / CARD_UNITS as f32) + 1;
                        reel.pos = ftoi(CARD_UNITS as f32 * next as f32) as i16;
                        reel.speed = 16;
                    }
                    4 | 3 => reel.speed = -12,
                    2 | 1 => reel.speed = 4,
                    _ => {}
                }
                advance(reel);
                reel.counter = reel.counter.wrapping_sub(1);
                if reel.counter == 0 {
                    reel.speed = 0;
                    reel.state = ReelState::Stopped;
                    self.evaluate_lines(r);
                    events.push(SlotEvent::ReelStopped {
                        reel: r as u8,
                        rows: self.reels[r].rows(),
                        live_lines: self.live_lines,
                    });
                }
            }
            ReelState::Idle | ReelState::Stopped => {}
        }
    }

    /// 0x281930(reel): a live line dies when this reel's cell on it is the
    /// filler or differs from the cell of any reel already stopped; the
    /// reel's card becomes its middle row.
    fn evaluate_lines(&mut self, r: usize) {
        let rows = [
            self.reels[0].rows(),
            self.reels[1].rows(),
            self.reels[2].rows(),
        ];
        for (k, line) in LINE_ROWS.iter().enumerate() {
            if self.live_lines & (1 << k) == 0 {
                continue;
            }
            let own = rows[r][line[r]];
            let mut dead = own == FILLER_CARD;
            for (other, cells) in rows.iter().enumerate() {
                if other == r {
                    continue;
                }
                let cell = cells[line[other]];
                if cell != NO_CARD && cell != own {
                    dead = true;
                }
            }
            if dead {
                self.live_lines &= !(1 << k);
            }
        }
        self.reels[r].card = rows[r][1];
    }

    /// 0x2816C0: reel 0's card; reel 1's if different; reel 2's if
    /// different from both; then every line that flashed, mapped through
    /// 0x252E10 from reel 0's cell on it. Leaves [`NO_CARD`] when the order
    /// is exhausted.
    fn next_prize(&mut self, rng: &mut u64) {
        self.prizes.card = NO_CARD;
        let c0 = self.reels[0].card;
        let c1 = self.reels[1].card;
        while self.prizes.index < 8 {
            let index = self.prizes.index;
            self.prizes.index += 1;
            let card = match index {
                0 => Some(c0).filter(|&c| c != NO_CARD),
                1 => Some(c1).filter(|&c| c != NO_CARD && c != c0),
                2 => Some(self.reels[2].card).filter(|&c| c != NO_CARD && c != c0 && c != c1),
                _ => {
                    let k = usize::from(index) - 3;
                    if self.flash.lines[k] == 0 {
                        None
                    } else {
                        // A flashed line is always enabled; the code's other
                        // branch (B[0x80+2k], a stale strip word) is unreachable.
                        let reel0 = &self.reels[0];
                        let cell =
                            reel0.strip[(reel0.top_index() + LINE_ROWS[k][0]) % reel0.strip.len()];
                        Some(special_prize(cell, &self.special_pool, rng))
                    }
                }
            };
            if let Some(card) = card {
                self.prizes.card = card;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests;
