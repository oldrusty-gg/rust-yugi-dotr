//! The memory-card dialogs of the map's SYSTEM → SAVE (op 1) and the title's
//! Continue (op 0), and SAVE ↔ the port's campaign state (Claude, 4 Oct
//! 2026). Notes, traces and captures: `work/research/story/claude-save/`.
//!
//! The original: 0x1AB8F0(op) starts an op of the card module, 0x1ABB20()
//! runs it once per update (the map calls it while W+543E = 1, the title
//! menu every update) and returns −1 idle, 0 busy, 1 loaded, 2 failed. Each
//! step issues one asynchronous libmc call (GetInfo, ChDir, GetDir, Open,
//! Seek, Read, Write, Close, Mkdir, Format) and polls sceMcSync on the next
//! update; the card layer 0x23BCE0..0x240DF0 maps results to message ids
//! (0x1ACBE0) and the module shows them in overlay slot 3 with 0x1E5540
//! kind 5 (centred box, busy), 6 (box + press icon, acknowledge) or 7 (YES /
//! NO) [F]. The timeline below is the one the game's own code produced in
//! the EE interpreter with a virtual card whose calls complete at once
//! (`tools/vcard.py`) [emu]; a real card's latency would lengthen the busy
//! boxes [H] — the port's card is a local file, so it completes at once too.
//!
//! The ending's save (op 2, event 11184 sub-step 3 at 0x221750; traced in
//! `work/research/story/claude-ending/`) runs the same steps behind a
//! first "Save?" question with YES preselected; its refusals, failures and
//! NO answers return to that question, and only NO / Circle there gives up.
//!
//! Op 3 (the title's Custom Duel, traced in
//! `work/research/modes/other-modes/` and `modes/custom-duel/`) loads
//! slot 1 as op 0 does, then checks MEMORY CARD slot 2: "Checking MEMORY
//! CARD slot 2" (3521), with a save of this game "Loading… slot 2" (3538),
//! then "Load complete" (3540); the result's high byte − 1 picks Custom
//! Duel's opponent (`custom_duel::selector_from_result`): 0x100 a valid
//! save (a second player), 0x280 no card (the computer with a copy of the
//! player's deck), 0x380 an empty, unformatted or unreadable card [emu
//! op3-*]. Its slot-1 failures are op 0's [H: not run].
//!
//! Op 4 (the title's Trade) and op 5 (Trade's save; traced in
//! `work/research/modes/two-player/`, `tools/opvar.py`, `op45.py`)
//! [emu]: op 4 loads slot 1 as op 0 with "Loading... slots" (3539), checks
//! slot 2 (3521) and loads it (3539 again), then "Load complete - do not
//! remove the MEMORY CARD while trading" (3560) returns 1 with both files;
//! slot 2 without a card -> 3524, without a save (empty or unformatted) ->
//! 3535, unreadable -> 3541, each acknowledged and returning 2. Op 5 checks
//! both slots (GetInfo x2): a slot gone -> 3561 "Save failed! The MEMORY
//! CARD was removed while trading"; else "Saving... slots" (3556) for eight
//! updates, both files written (each slot its own block and its own SAVE,
//! unchanged), "Save completed" (3557); op 5 returns 1 either way.
//!
//! What is explicit here and not traced: op 6 (boot check), removing a
//! card while a YES/NO waits (the module polls GetInfo then), op 5's
//! write failure (the port shows 3558 [H]), and the Icon.Mrg icon files
//! the original writes beside the data (the port's virtual card stores
//! only the data file).
use crate::campaign_map::{CampaignSave, NODE_COUNT};
use roses_core::memory_card::{MIN_FREE_KB, SAVE_BYTES};

// ------------------------------------------------------------------ SAVE
/// SAVE+0x24: the player's name, twelve MSG_E tokens (end bit on the last).
pub const NAME_OFFSET: usize = 0x24;
pub const NAME_WORDS: usize = 12;
/// SAVE+0x3C (s16): non-zero once the map was entered (0x21C5D8 stores 1;
/// the ending clears it at 0x2216E4). The Continue path 0x21C440 tests it
/// (`CampaignSave::map_entered`).
pub const MAP_ENTERED_OFFSET: usize = 0x3C;
/// Opponent counters per id (SAVE+0x42 / +0x82, s16 each).
pub const OPPONENTS: usize = 32;

fn rd16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn wr16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

/// SAVE as the map leaves it, from the port's state: the fields the port
/// models are written over `base` (the SAVE last loaded or saved, so the
/// bytes the port does not model are carried; zeros when there is none).
/// SAVE+0x3C = `map.map_entered` (1 for every map save; 0 for the ending's).
pub fn save_record(
    base: Option<&[u8; SAVE_BYTES]>,
    map: &CampaignSave,
    name_codes: &[u16],
) -> [u8; SAVE_BYTES] {
    let mut s = base.copied().unwrap_or([0u8; SAVE_BYTES]);
    s[0] = map.side;
    s[1] = u8::from(map.red_route_cleared);
    // SAVE[2] is only ever set here (the ending's record carries it in
    // `base`): a map save keeps the card's flag.
    s[2] |= u8::from(map.white_route_cleared);
    for (n, &state) in map.node_states.iter().take(NODE_COUNT).enumerate() {
        s[3 + n] = state;
    }
    if !name_codes.is_empty() {
        for k in 0..NAME_WORDS {
            wr16(&mut s, NAME_OFFSET + 2 * k, name_codes.get(k).copied().unwrap_or(0));
        }
    }
    wr16(&mut s, MAP_ENTERED_OFFSET, u16::from(map.map_entered));
    wr16(&mut s, 0x3E, map.wins);
    wr16(&mut s, 0x40, map.losses);
    for k in 0..OPPONENTS {
        wr16(&mut s, 0x42 + 2 * k, map.wins_by_opponent.get(k).copied().unwrap_or(0));
        wr16(&mut s, 0x82 + 2 * k, map.losses_by_opponent.get(k).copied().unwrap_or(0));
    }
    wr16(&mut s, 0xC2, map.event as u16);
    wr16(&mut s, 0xC4, map.opponent as u16);
    s[0xC6] = map.hovered;
    s[0xC7] = map.location;
    s[0xC8] = map.destination;
    s[0xC9] = map.c9;
    s[0xCA] = map.duel_count;
    s
}

/// What the port reads back from SAVE.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedRecord {
    pub map: CampaignSave,
    /// SAVE+0x24, the twelve name words (end bit kept).
    pub name_codes: Vec<u16>,
    /// SAVE[2]: the white route was cleared (also `map.white_route_cleared`).
    pub white_route_cleared: bool,
    /// SAVE+0x3C ≠ 0.
    pub map_entered: bool,
}

pub fn load_record(s: &[u8; SAVE_BYTES]) -> LoadedRecord {
    let words = |at: usize| (0..OPPONENTS).map(|k| rd16(s, at + 2 * k)).collect::<Vec<_>>();
    LoadedRecord {
        map: CampaignSave {
            side: s[0],
            node_states: s[3..3 + NODE_COUNT].to_vec(),
            event: rd16(s, 0xC2) as i16,
            opponent: rd16(s, 0xC4) as i16,
            hovered: s[0xC6],
            location: s[0xC7],
            destination: s[0xC8],
            c9: s[0xC9],
            duel_count: s[0xCA],
            wins: rd16(s, 0x3E),
            losses: rd16(s, 0x40),
            wins_by_opponent: words(0x42),
            losses_by_opponent: words(0x82),
            red_route_cleared: s[1] != 0,
            map_entered: rd16(s, MAP_ENTERED_OFFSET) != 0,
            white_route_cleared: s[2] != 0,
        },
        name_codes: (0..NAME_WORDS).map(|k| rd16(s, NAME_OFFSET + 2 * k)).collect(),
        white_route_cleared: s[2] != 0,
        map_entered: rd16(s, MAP_ENTERED_OFFSET) != 0,
    }
}

// ------------------------------------------------------------------ messages
/// MSG_E message ids of the dialogs (slot 1 variants; 0x1ACBE0) [F, emu].
pub mod msg {
    pub const CHECKING: u16 = 3520;
    /// Op 3: "Checking MEMORY CARD slot 2" / "Loading… slot 2" [emu op3-*].
    pub const CHECKING_SLOT2: u16 = 3521;
    pub const LOADING_SLOT2: u16 = 3538;
    pub const NO_CARD: u16 = 3523;
    pub const NO_SPACE: u16 = 3528;
    pub const NO_SAVE_DATA: u16 = 3533;
    pub const LOADING: u16 = 3537;
    pub const LOAD_COMPLETE: u16 = 3540;
    pub const LOAD_FAILED: u16 = 3541;
    pub const FORMAT_ASK: u16 = 3544;
    pub const FORMATTING: u16 = 3548;
    pub const FORMAT_DONE: u16 = 3550;
    pub const FORMAT_FAILED: u16 = 3552;
    pub const SAVING: u16 = 3554;
    pub const SAVE_COMPLETE: u16 = 3557;
    pub const SAVE_FAILED: u16 = 3558;
    pub const OVERWRITE_ASK: u16 = 3559;
    /// "Save?": the ending save's (op 2) first question [emu op2-*].
    pub const SAVE_ASK: u16 = 3543;
    /// Ops 4 / 5 (Trade) [emu op4-*, op45-*].
    pub const NO_CARD_SLOT2: u16 = 3524;
    pub const NO_SAVE_SLOT2: u16 = 3535;
    pub const LOADING_SLOTS: u16 = 3539;
    pub const SAVING_SLOTS: u16 = 3556;
    pub const TRADE_LOADED: u16 = 3560;
    pub const TRADE_CARD_REMOVED: u16 = 3561;
    /// Every id a dialog may show (the HUD must compose them).
    pub const ALL: [u16; 24] = [
        CHECKING, NO_CARD, NO_SPACE, NO_SAVE_DATA, LOADING, LOAD_COMPLETE, LOAD_FAILED, FORMAT_ASK,
        FORMATTING, FORMAT_DONE, FORMAT_FAILED, SAVING, SAVE_COMPLETE, SAVE_FAILED, OVERWRITE_ASK,
        SAVE_ASK, CHECKING_SLOT2, LOADING_SLOT2, NO_CARD_SLOT2, NO_SAVE_SLOT2, LOADING_SLOTS,
        SAVING_SLOTS, TRADE_LOADED, TRADE_CARD_REMOVED,
    ];
}

/// SE ids (0x216EA0(0, id)) [F, emu].
pub const SFX_CURSOR: u16 = 3;
pub const SFX_CONFIRM: u16 = 4;
pub const SFX_CANCEL: u16 = 5;

// ------------------------------------------------------------------ card
/// The card in MEMORY CARD slot 1 as GetInfo sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardStatus {
    Missing,
    Unformatted,
    Formatted { free_kb: u32 },
}

/// The memory card the dialogs talk to (the port: a local file).
pub trait CardIo {
    fn status(&self) -> CardStatus;
    /// The save directory exists (ChDir succeeds).
    fn has_save(&self) -> bool;
    /// The data file's bytes.
    fn read(&mut self) -> Result<Vec<u8>, String>;
    /// Create the directory if needed and write the data file.
    fn write(&mut self, file: &[u8]) -> Result<(), String>;
    fn format(&mut self) -> Result<(), String>;
}

// ------------------------------------------------------------------ dialog
/// The overlay box a dialog shows this frame (0x1E5540 kinds 5/6/7, x 0,
/// y 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardBox {
    /// Kind 5 (state 0x14): centred box, no input.
    Busy(u16),
    /// Kind 6 (state 0x15): centred box with the press icon.
    Acknowledge(u16),
    /// Kind 7 (state 0x16): YES / NO; `yes` = D35.
    YesNo { text: u16, yes: bool },
}

impl CardBox {
    pub fn text(&self) -> u16 {
        match *self {
            Self::Busy(t) | Self::Acknowledge(t) | Self::YesNo { text: t, .. } => t,
        }
    }
}

/// New presses this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardPad {
    pub left: bool,
    pub right: bool,
    pub cross: bool,
    pub circle: bool,
    pub square: bool,
    pub triangle: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardOp {
    /// Op 1: the map's SYSTEM → SAVE.
    Save,
    /// Op 0: the title's Continue.
    Load,
    /// Op 2: the save after a route's ending (0x1AB8F0(2) at 0x221750;
    /// bit 3 of +0x23A = YES preselected on the first question).
    EndingSave,
    /// Op 3: the title's Custom Duel (slot 1 loaded, slot 2 checked).
    CustomDuelLoad,
    /// Op 4: the title's Trade (both slots loaded).
    TradeLoad,
    /// Op 5: Trade's YES (both slots written).
    TradeSave,
}

/// Op 3's results (0x23C820) [F 0x23C918..0x23C9B8, emu op3-*].
pub mod op3 {
    /// Slot 2 holds a valid save of this game (selector 0: two players).
    pub const SECOND_SAVE: u16 = 0x100;
    /// No card in slot 2 (selector 1: the mirror computer).
    pub const NO_CARD: u16 = 0x280;
    /// An empty, unformatted or unreadable slot 2 (selector 2).
    pub const NO_SAVE: u16 = 0x380;
}

/// What an update ended with (0x1ABB20's return).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardOutcome {
    /// 0: still running.
    Running,
    /// −1: the dialog closed (save done, refused or cancelled).
    Closed,
    /// 1: loaded; the file's bytes (checksum good).
    Loaded(Vec<u8>),
    /// 2: the load failed (no card, no data, bad checksum).
    LoadFailed,
    /// Op 2 returned 1: the ending's save was written and acknowledged
    /// (the update after returns −1 = `Closed`) [emu op2-fresh u72].
    Saved,
    /// Op 2 returned 2: NO or Circle on "Save?" (nothing written; −1 the
    /// update after) [emu op2-no u10, op2-circle u6].
    NotSaved,
    /// Op 3 returned 1: slot 1's file, the result word (`op3`) and slot 2's
    /// file when it held a valid save.
    CustomDuel {
        slot1: Vec<u8>,
        result: u16,
        slot2: Option<Vec<u8>>,
    },
    /// Op 4 returned 1: both slots' files.
    Trade { slot1: Vec<u8>, slot2: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// No box; after `left` more updates `then` runs.
    Blank { left: u8, then: Action },
    /// A busy box; after `left` more updates `then` runs.
    Busy { text: u16, left: u8, then: Action },
    Ack { text: u16, then: AckThen },
    Ask { text: u16, yes: bool, on_yes: Action },
    /// The box of the confirming frame stays; `then` runs next update.
    Hold { shown: CardBox, then: Action },
    Finished,
}

/// What runs at the update a busy (or blank) stretch ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    /// Op 2: (back to) "Save?", YES preselected.
    AskSave,
    /// Save u2: GetInfo.
    SaveCheck,
    /// Save u4: ChDir's result (and, with a save, its read).
    SaveDirectory,
    AskFormat,
    AskOverwrite,
    Overwrite,
    Write,
    Format,
    FormatDone,
    /// After a format's acknowledge: two more "Checking…", then u2.
    Recheck,
    /// Load u0: GetInfo.
    LoadCheck,
    /// Load u1: ChDir.
    LoadDirectory,
    LoadStart,
    LoadRead,
    NoData,
    AckDone,
    /// Op 3: slot 2's GetInfo / ChDir answered with this result.
    Slot2Result(u16),
    /// Op 3: slot 2 has the save directory: read it.
    LoadSlot2,
    /// Op 4: slot 2 refused with this message (acknowledged, returns 2).
    Slot2Fail(u16),
    /// Op 5's first update: GetInfo on both slots.
    TradeSaveCheck,
    /// Op 5: write both files.
    TradeWrite,
    /// Op 5: a slot was removed (acknowledged, returns 1).
    TradeRemoved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AckThen {
    Close,
    /// Op 2: "Save completed" acknowledged → return 1.
    Saved,
    /// Op 2: a refusal acknowledged → back to "Save?" (the box stays on the
    /// press frame) [emu op2-nocard u17, op2-full u17].
    AskAgain,
    LoadDone,
    LoadFailed,
    Recheck,
}

/// One memory-card dialog.
#[derive(Clone, Debug)]
pub struct CardDialog {
    op: CardOp,
    phase: Phase,
    file: Vec<u8>,
    loaded: Option<Vec<u8>>,
    /// Op 3: the result word and slot 2's file.
    result: u16,
    loaded2: Option<Vec<u8>>,
    /// Op 5: slot 2's file.
    file2: Vec<u8>,
    /// Updates since the start (the press icon's cel counter [H: the
    /// original uses the global update counter 0x2DA954]).
    pub counter: u32,
    /// SE requests of the last update.
    pub sfx: Vec<u16>,
    /// What went wrong at the card (for the log), if anything.
    pub error: Option<String>,
}

/// Updates of "Saving…" after its first: a first save makes 19 more card
/// calls (Mkdir, ChDir, icon.sys and three icons x open/write/close,
/// create + close, open, seek, write) and an overwrite 3 (open, seek,
/// write) before the close shows "Save completed" [emu].
const SAVE_NEW_MORE: u8 = 19;
const SAVE_OVERWRITE_MORE: u8 = 3;
/// With a save on the card the overwrite check reads it: "Checking…" for
/// GetDir, Open, Seek, Read after the ChDir update [emu].
const CHECK_READ_MORE: u8 = 3;
/// "Loading…" for GetDir, Open, Seek, Read [emu].
const LOAD_MORE: u8 = 3;

impl CardDialog {
    /// The op starts on its button press (the map's Cross on icon 1, the
    /// title's Continue; SE 4 is the caller's); the first
    /// [`CardDialog::update`] is the next frame's. `file` = the save file
    /// to write.
    pub fn save(file: Vec<u8>) -> Self {
        Self::new(CardOp::Save, file, Phase::Blank { left: 1, then: Action::SaveCheck })
    }

    pub fn load() -> Self {
        Self::new(CardOp::Load, Vec::new(), Phase::Blank { left: 0, then: Action::LoadCheck })
    }

    /// Op 2, the ending's save: 11184 starts it and calls the first
    /// [`CardDialog::update`] in the same frame, which already shows
    /// "Save?" with YES selected [emu ending-fresh f979].
    pub fn ending_save(file: Vec<u8>) -> Self {
        Self::new(CardOp::EndingSave, file, Phase::Blank { left: 0, then: Action::AskSave })
    }

    /// Op 3, the title's Custom Duel: the first update is the next frame's
    /// (as op 0), then slot 2 is checked after slot 1's read.
    pub fn custom_duel() -> Self {
        Self::new(CardOp::CustomDuelLoad, Vec::new(), Phase::Blank { left: 0, then: Action::LoadCheck })
    }

    /// Op 4, the title's Trade: the first update is the next frame's (as
    /// op 0) [emu op4-save u0].
    pub fn trade_load() -> Self {
        Self::new(CardOp::TradeLoad, Vec::new(), Phase::Blank { left: 0, then: Action::LoadCheck })
    }

    /// Op 5, Trade's save of both files: started by the YES (0x1C4104),
    /// updated from the next frame (the trade update calls 0x1ABB20 first)
    /// [emu op45-save u0 = "Saving... slots"].
    pub fn trade_save(slot1: Vec<u8>, slot2: Vec<u8>) -> Self {
        let mut d = Self::new(CardOp::TradeSave, slot1, Phase::Blank { left: 0, then: Action::TradeSaveCheck });
        d.file2 = slot2;
        d
    }

    fn new(op: CardOp, file: Vec<u8>, phase: Phase) -> Self {
        Self {
            op,
            phase,
            file,
            file2: Vec::new(),
            loaded: None,
            result: 0,
            loaded2: None,
            counter: 0,
            sfx: Vec::new(),
            error: None,
        }
    }

    pub fn op(&self) -> CardOp {
        self.op
    }

    pub fn finished(&self) -> bool {
        self.phase == Phase::Finished
    }

    /// The box drawn this frame (after the update), if any.
    pub fn overlay(&self) -> Option<CardBox> {
        match self.phase {
            Phase::Blank { .. } | Phase::Finished => None,
            Phase::Busy { text, .. } => Some(CardBox::Busy(text)),
            Phase::Ack { text, .. } => Some(CardBox::Acknowledge(text)),
            Phase::Ask { text, yes, .. } => Some(CardBox::YesNo { text, yes }),
            Phase::Hold { shown, .. } => Some(shown),
        }
    }

    /// One update (0x1ABB20) with MEMORY CARD slot 2 absent.
    pub fn update(&mut self, pad: CardPad, card: &mut dyn CardIo) -> CardOutcome {
        self.update_slots(pad, card, &mut NoCard)
    }

    /// One update (0x1ABB20); `slot2` is only read by op 3.
    pub fn update_slots(&mut self, pad: CardPad, card: &mut dyn CardIo, slot2: &mut dyn CardIo) -> CardOutcome {
        self.sfx.clear();
        self.counter = self.counter.wrapping_add(1);
        let phase = std::mem::replace(&mut self.phase, Phase::Finished);
        let (phase, outcome) = match phase {
            Phase::Finished => (Phase::Finished, CardOutcome::Closed),
            Phase::Blank { left, then } if left > 0 => {
                (Phase::Blank { left: left - 1, then }, CardOutcome::Running)
            }
            Phase::Busy { text, left, then } if left > 0 => {
                (Phase::Busy { text, left: left - 1, then }, CardOutcome::Running)
            }
            Phase::Blank { then, .. } | Phase::Busy { then, .. } | Phase::Hold { then, .. } => {
                (self.run(then, card, slot2), CardOutcome::Running)
            }
            Phase::Ack { text, then } => self.acknowledge(text, then, pad),
            Phase::Ask { text, yes, on_yes } => self.ask(text, yes, on_yes, pad),
        };
        self.phase = phase;
        outcome
    }

    fn acknowledge(&mut self, text: u16, then: AckThen, pad: CardPad) -> (Phase, CardOutcome) {
        // 0x203380(0, 0, 0xF0): a new press of triangle, circle, cross or
        // square; SE 4.
        if !(pad.cross || pad.circle || pad.square || pad.triangle) {
            return (Phase::Ack { text, then }, CardOutcome::Running);
        }
        self.sfx.push(SFX_CONFIRM);
        match then {
            AckThen::Close => (Phase::Finished, CardOutcome::Closed),
            AckThen::Saved => (Phase::Finished, CardOutcome::Saved),
            AckThen::AskAgain => (
                Phase::Hold { shown: CardBox::Acknowledge(text), then: Action::AskSave },
                CardOutcome::Running,
            ),
            AckThen::LoadFailed => (Phase::Finished, CardOutcome::LoadFailed),
            AckThen::LoadDone => match (self.loaded.take(), self.op) {
                (Some(slot1), CardOp::CustomDuelLoad) => (
                    Phase::Finished,
                    CardOutcome::CustomDuel { slot1, result: self.result, slot2: self.loaded2.take() },
                ),
                (Some(slot1), CardOp::TradeLoad) => match self.loaded2.take() {
                    Some(slot2) => (Phase::Finished, CardOutcome::Trade { slot1, slot2 }),
                    None => (Phase::Finished, CardOutcome::LoadFailed),
                },
                (Some(bytes), _) => (Phase::Finished, CardOutcome::Loaded(bytes)),
                (None, _) => (Phase::Finished, CardOutcome::LoadFailed),
            },
            // After a format the box stays on the press frame [emu].
            AckThen::Recheck => (
                Phase::Hold { shown: CardBox::Acknowledge(text), then: Action::Recheck },
                CardOutcome::Running,
            ),
        }
    }

    fn ask(&mut self, text: u16, yes: bool, on_yes: Action, pad: CardPad) -> (Phase, CardOutcome) {
        // Left|Right (0xA000): YES = Left; SE 3 when it changes.
        let mut now = yes;
        if pad.left || pad.right {
            now = pad.left;
            if now != yes {
                self.sfx.push(SFX_CURSOR);
            }
        }
        // Cross / square (0xC0) confirm with SE 4; circle (0x20) cancels
        // with SE 5. Op 1 closes on NO or circle [F 0x1AC0DC, 0x1AC170].
        // Op 2: NO / circle on "Save?" returns 2 (no box that frame); on a
        // later question they go back to step 1 with bit 3 set: the box
        // shows YES on that frame, "Save?" the next [emu op2-exists-no u22,
        // op2-exists-circle u19, op2-unformatted-no u20].
        let give_up = match self.op {
            CardOp::EndingSave if text == msg::SAVE_ASK => (Phase::Finished, CardOutcome::NotSaved),
            CardOp::EndingSave => (
                Phase::Hold { shown: CardBox::YesNo { text, yes: true }, then: Action::AskSave },
                CardOutcome::Running,
            ),
            _ => (Phase::Finished, CardOutcome::Closed),
        };
        if pad.cross || pad.square {
            self.sfx.push(SFX_CONFIRM);
            return if now {
                (
                    Phase::Hold { shown: CardBox::YesNo { text, yes: true }, then: on_yes },
                    CardOutcome::Running,
                )
            } else {
                give_up
            };
        }
        if pad.circle {
            self.sfx.push(SFX_CANCEL);
            return give_up;
        }
        (Phase::Ask { text, yes: now, on_yes }, CardOutcome::Running)
    }

    fn run(&mut self, action: Action, card: &mut dyn CardIo, slot2: &mut dyn CardIo) -> Phase {
        let busy = |text, left, then| Phase::Busy { text, left, then };
        let ack = |text, then| Phase::Ack { text, then };
        // A save that cannot go on: op 1 closes after the acknowledge, op 2
        // asks "Save?" again [emu op2-nocard, op2-full].
        let refuse = if self.op == CardOp::EndingSave { AckThen::AskAgain } else { AckThen::Close };
        match action {
            Action::AskSave => Phase::Ask { text: msg::SAVE_ASK, yes: true, on_yes: Action::SaveCheck },
            Action::Recheck => busy(msg::CHECKING, 1, Action::SaveCheck),
            Action::SaveCheck => match card.status() {
                CardStatus::Missing => ack(msg::NO_CARD, refuse),
                CardStatus::Unformatted => busy(msg::CHECKING, 1, Action::AskFormat),
                CardStatus::Formatted { .. } => busy(msg::CHECKING, 1, Action::SaveDirectory),
            },
            Action::SaveDirectory => {
                let free = match card.status() {
                    CardStatus::Formatted { free_kb } => free_kb,
                    _ => 0,
                };
                if card.has_save() {
                    busy(msg::CHECKING, CHECK_READ_MORE, Action::AskOverwrite)
                } else if free < MIN_FREE_KB {
                    ack(msg::NO_SPACE, refuse)
                } else {
                    busy(msg::SAVING, SAVE_NEW_MORE, Action::Write)
                }
            }
            Action::AskFormat => Phase::Ask { text: msg::FORMAT_ASK, yes: false, on_yes: Action::Format },
            Action::AskOverwrite => {
                Phase::Ask { text: msg::OVERWRITE_ASK, yes: false, on_yes: Action::Overwrite }
            }
            Action::Overwrite => busy(msg::SAVING, SAVE_OVERWRITE_MORE, Action::Write),
            Action::Write => match card.write(&self.file) {
                Ok(()) => busy(msg::SAVE_COMPLETE, 0, Action::AckDone),
                Err(e) => {
                    self.error = Some(e);
                    ack(msg::SAVE_FAILED, refuse)
                }
            },
            Action::Format => busy(msg::FORMATTING, 0, Action::FormatDone),
            Action::FormatDone => match card.format() {
                Ok(()) => ack(msg::FORMAT_DONE, AckThen::Recheck),
                Err(e) => {
                    self.error = Some(e);
                    ack(msg::FORMAT_FAILED, refuse)
                }
            },
            Action::LoadCheck => match card.status() {
                CardStatus::Missing => ack(msg::NO_CARD, AckThen::LoadFailed),
                CardStatus::Unformatted => busy(msg::CHECKING, 1, Action::NoData),
                CardStatus::Formatted { .. } => busy(msg::CHECKING, 0, Action::LoadDirectory),
            },
            Action::LoadDirectory => {
                if card.has_save() {
                    busy(msg::CHECKING, 0, Action::LoadStart)
                } else {
                    ack(msg::NO_SAVE_DATA, AckThen::LoadFailed)
                }
            }
            Action::NoData => ack(msg::NO_SAVE_DATA, AckThen::LoadFailed),
            Action::LoadStart => {
                let text = if self.op == CardOp::TradeLoad { msg::LOADING_SLOTS } else { msg::LOADING };
                busy(text, LOAD_MORE, Action::LoadRead)
            }
            Action::LoadRead => {
                let parsed = card.read().and_then(|bytes| {
                    roses_core::memory_card::SaveFile::parse(&bytes)
                        .map(|p| (p.checksum_ok, bytes))
                        .map_err(|e| e.to_string())
                });
                match parsed {
                    Ok((true, bytes)) => {
                        self.loaded = Some(bytes);
                        if self.op == CardOp::TradeLoad {
                            // u6: slot 2's GetInfo / ChDir as op 3; no card,
                            // no save or unformatted refuse the trade [emu
                            // op4-none u7, op4-empty u8, op4-unformatted u9].
                            match slot2.status() {
                                CardStatus::Missing => busy(msg::CHECKING_SLOT2, 0, Action::Slot2Fail(msg::NO_CARD_SLOT2)),
                                CardStatus::Unformatted => busy(msg::CHECKING_SLOT2, 2, Action::Slot2Fail(msg::NO_SAVE_SLOT2)),
                                CardStatus::Formatted { .. } if slot2.has_save() => busy(msg::CHECKING_SLOT2, 2, Action::LoadSlot2),
                                CardStatus::Formatted { .. } => busy(msg::CHECKING_SLOT2, 1, Action::Slot2Fail(msg::NO_SAVE_SLOT2)),
                            }
                        } else if self.op == CardOp::CustomDuelLoad {
                            // u6: slot 2's GetInfo, then its ChDir: one
                            // more "Checking…" per call that answered [emu
                            // op3-nocard2 f6, op3-empty f6-7, op3-unformatted
                            // and op3-save f6-8].
                            match slot2.status() {
                                CardStatus::Missing => busy(msg::CHECKING_SLOT2, 0, Action::Slot2Result(op3::NO_CARD)),
                                CardStatus::Unformatted => busy(msg::CHECKING_SLOT2, 2, Action::Slot2Result(op3::NO_SAVE)),
                                CardStatus::Formatted { .. } if slot2.has_save() => {
                                    busy(msg::CHECKING_SLOT2, 2, Action::LoadSlot2)
                                }
                                CardStatus::Formatted { .. } => busy(msg::CHECKING_SLOT2, 1, Action::Slot2Result(op3::NO_SAVE)),
                            }
                        } else {
                            busy(msg::LOAD_COMPLETE, 0, Action::AckDone)
                        }
                    }
                    Ok((false, _)) => {
                        self.error = Some("the save file's checksum does not match".into());
                        ack(msg::LOAD_FAILED, AckThen::LoadFailed)
                    }
                    Err(e) => {
                        self.error = Some(e);
                        ack(msg::LOAD_FAILED, AckThen::LoadFailed)
                    }
                }
            }
            Action::LoadSlot2 => {
                // "Loading… slot 2" for GetDir, Open, Seek, Read [emu op3-save f9-12].
                let valid = slot2.read().ok().filter(|bytes| {
                    roses_core::memory_card::SaveFile::parse(bytes).is_ok_and(|p| p.checksum_ok)
                });
                let result = if valid.is_some() { op3::SECOND_SAVE } else { op3::NO_SAVE };
                let ok = valid.is_some();
                self.loaded2 = valid;
                if self.op == CardOp::TradeLoad {
                    // "Loading... slots" again; an unreadable file -> "Load
                    // failed!" [emu op4-save u9-12, op4-corrupt u13].
                    let then = if ok { Action::Slot2Result(result) } else { Action::Slot2Fail(msg::LOAD_FAILED) };
                    return busy(msg::LOADING_SLOTS, LOAD_MORE, then);
                }
                busy(msg::LOADING_SLOT2, LOAD_MORE, Action::Slot2Result(result))
            }
            Action::Slot2Fail(text) => ack(text, AckThen::LoadFailed),
            Action::TradeSaveCheck => {
                let ready = |c: &dyn CardIo| matches!(c.status(), CardStatus::Formatted { .. });
                if ready(card) && ready(slot2) {
                    // Eight updates of "Saving... slots" (GetInfo x2, Open
                    // x2, Seek x2, Write x2, the closes) [emu op45-save u0-7].
                    busy(msg::SAVING_SLOTS, 7, Action::TradeWrite)
                } else {
                    self.error = Some("a MEMORY CARD was removed while trading".into());
                    busy(msg::TRADE_CARD_REMOVED, 0, Action::TradeRemoved)
                }
            }
            Action::TradeRemoved => ack(msg::TRADE_CARD_REMOVED, AckThen::Saved),
            Action::TradeWrite => match card.write(&self.file).and_then(|()| slot2.write(&self.file2)) {
                Ok(()) => busy(msg::SAVE_COMPLETE, 0, Action::AckDone),
                Err(e) => {
                    self.error = Some(e);
                    ack(msg::SAVE_FAILED, AckThen::Saved)
                }
            },
            Action::Slot2Result(result) => {
                self.result = result;
                let text = if self.op == CardOp::TradeLoad { msg::TRADE_LOADED } else { msg::LOAD_COMPLETE };
                busy(text, 0, Action::AckDone)
            }
            Action::AckDone => match self.op {
                CardOp::Load | CardOp::CustomDuelLoad => ack(msg::LOAD_COMPLETE, AckThen::LoadDone),
                CardOp::TradeLoad => ack(msg::TRADE_LOADED, AckThen::LoadDone),
                CardOp::Save => ack(msg::SAVE_COMPLETE, AckThen::Close),
                CardOp::EndingSave | CardOp::TradeSave => ack(msg::SAVE_COMPLETE, AckThen::Saved),
            },
        }
    }
}

/// A MEMORY CARD slot with no card.
pub struct NoCard;

impl CardIo for NoCard {
    fn status(&self) -> CardStatus {
        CardStatus::Missing
    }
    fn has_save(&self) -> bool {
        false
    }
    fn read(&mut self) -> Result<Vec<u8>, String> {
        Err("no memory card".into())
    }
    fn write(&mut self, _file: &[u8]) -> Result<(), String> {
        Err("no memory card".into())
    }
    fn format(&mut self) -> Result<(), String> {
        Err("no memory card".into())
    }
}

#[cfg(test)]
mod tests;
