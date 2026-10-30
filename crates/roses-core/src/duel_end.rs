//! The result sequence of a duel: sub-states 0x54..0x62 of the duel state
//! machine 0x1D00F0 (private research `claude-end/DUEL-END.md` §0–§3,
//! handlers 0x1D84A8–0x1D8F38), stepped one frame at a time with the pad.
//!
//! What the original does after the outcome (D50 winner, D51 reason) is
//! decided: choose the banner (§0), play the music cue, wait for the sound
//! and the HUD panel, slide the banner in (64 frames; a loss also darkens
//! the scene), wait for a new △○×□ press, slide it out (64 frames) while
//! applying the duel-end experience (0x2745F0) and, for one human in a
//! campaign duel, the save counters (0x227F00, applied by the campaign map
//! model — this module only reports it), then recompute every rank of the
//! human's block and show one PROMOTION/DEMOTION banner per changed resident
//! card (a press each), and finally either start the slot machine (a human
//! won: reel data from the loser's graveyard) or pop back to the map.
//!
//! The dispatch loop matters for frame counts: a handler either returns for
//! the frame (0x1D8F54 with 0), asks for another run in the same frame
//! (0x1D8F54 with 1, looped by the caller at 0x1AAF5C: 0x58 and the end of
//! 0x59–0x5B), goes to the check at
//! 0x1D8F38 (which re-dispatches in the same frame iff the state changed
//! since the last check) or falls straight into the next state's code.
//! [`DuelEnd::step`] reproduces that, so e.g. the banner is already at
//! c9c = 1 in the frame 0x58 ran and the first PROMOTION frame slides by
//! 16, not 8 [emu run1, run4].
//!
//! Waits on things outside the state machine (sound idle 0x217150, HUD
//! panel D3E ≥ 0x80 slid by 0x1B1D50, camera 0x1FB900) are frame counts in
//! [`EndTimings`]; their real values are not traced ([H]), the interpreter
//! runs used 0. The mode switch starts a fade (0x1FDE80(0,1,1,0,0x3C,0))
//! whose length in frames is likewise untraced: [`MODE_FADE_ARG`].
//!
//! Provenance as in `card_block.rs`: [F] code, [emu] `run1.py`/`run1b.py`
//! (win, loss, draw) and `run4.py` (rank lists), [H] hypothesis.

use crate::card_block::{CardBlock, RankChange};
use crate::duel::{
    CardData, Duel, EndReason, ExperienceChange, Outcome, PAD_CONTROLLER, RANK_COUNT,
};
use crate::slot_machine::{build_reel_data, ReelData, RewardCardTable, RewardListInput};
use crate::CARD_COUNT;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Banner string ids (overlay slot CB2, table 0x2903B0).
pub mod banner {
    pub const YOU_WIN: u8 = 2;
    pub const YOU_LOSE: u8 = 3;
    pub const P1_WIN: u8 = 4;
    pub const P2_WIN: u8 = 5;
    pub const DRAW: u8 = 6;
    pub const PROMOTION: u8 = 9;
    pub const DEMOTION: u8 = 0xA;
    pub const TREASURE: u8 = 0xB;
    pub const GET_CARD: u8 = 0xC;
    pub const GET_SPECIAL_CARD: u8 = 0xD;
    /// No banner.
    pub const OFF: u8 = 0x18;
}

/// Message ids the rank banners use (roses-assets `parse_pal_messages`).
pub mod message {
    /// "has been Promoted!"
    pub const PROMOTED: u16 = 96;
    /// "has been Demoted!"
    pub const DEMOTED: u16 = 97;
    /// Rank names NCO .. SD are 83 + rank.
    pub const RANK_NAME_FIRST: u16 = 83;
    /// Mode 0xF's name, "Slot Machine".
    pub const SLOT_MACHINE: u16 = 98;
}

/// 0x217110 track ids: the win jingle (codes 2/4/5) and the loss one (3).
pub const MUSIC_WIN: u8 = 0x17;
pub const MUSIC_LOSE: u8 = 0x2A;
/// 0x216EA0(0, 4): the confirm sound of the presses.
pub const SFX_CONFIRM: u8 = 4;
/// Pad bits 4–7 = △○×□ (mask 0xF0 on the new-press word).
pub const PRESS_MASK: u16 = 0xF0;
/// Banner slide: x = 0x200 − 8·c9c.
pub const BANNER_X_ORIGIN: i32 = 0x200;
/// c9c at the banner's resting position (states 0x59–0x5B, 0x60).
pub const SLIDE_IN: i16 = 0x40;
/// c9c when the banner has left (state 0x5D).
pub const SLIDE_OUT: i16 = 0x80;
/// D3A darkening limit (+2 per frame in 0x5A and 0x5D).
pub const DARKEN_MAX: u8 = 0x80;
/// The last argument of the mode-switch fade 0x1FDE80(0,1,1,0,0x3C,0). [F]
/// as an argument; its meaning in frames is untraced [H].
pub const MODE_FADE_ARG: u32 = 0x3C;
/// Maps with an index below this are campaign duels (D37 < 22 → save).
pub const CAMPAIGN_MAPS: u8 = 22;

/// Frame counts of the waits outside the state machine. All [H]: the
/// interpreter runs (and the frame numbers quoted in DUEL-END.md) had the
/// sound and camera stubs answer at once (0) and poked the HUD panel to
/// 0x80 at the first frame that started in 0x56 (= `hud_slide_frames` 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndTimings {
    /// State 0x55: frames until 0x217150 reports the sound idle.
    pub sound_idle_frames: u32,
    /// State 0x56: frames until the HUD panel D3E reaches 0x80 (0x1B1D50
    /// slides it by 8 or 16 per frame from where it was).
    pub hud_slide_frames: u32,
    /// State 0x59: frames after the camera move started (state 0x58) until
    /// 0x1FB900(−1, −1) reports it done.
    pub camera_wait_frames: u32,
}

/// Everything the sequence reads from the ended duel and its parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelEndSetup {
    pub outcome: Outcome,
    /// Side +0x33C controller >= 100 (pad-driven).
    pub human: [bool; 2],
    /// D52, the first player (= SAVE[0] in campaign duels).
    pub first: u8,
    /// D37, the map index; < 22 is a campaign duel (save counters written).
    pub map: u8,
    /// param[22] != 0 (99 in campaign duels): play the win/loss track.
    pub sound: bool,
    /// The experience the duel recorded plus its end award
    /// (`Duel::final_experience`), applied to the blocks at state 0x5D.
    pub experience: [BTreeMap<u16, ExperienceChange>; 2],
    /// Each side's deck leader card (0x215160(s, 0x215040(s))).
    pub leaders: [u16; 2],
    /// Each side's graveyard in removal order (side +0x232).
    pub graveyards: [Vec<u16>; 2],
    /// Each side's leader type (card byte 0 & 0x1F, 0x271610).
    pub leader_types: [u8; 2],
    /// Side +0x344 bit 7 of each side (special draw announced, 0x27133C):
    /// the winner's clears the reels' random slip. Not modelled by `Duel`;
    /// the caller states it.
    pub special_draw_announced: [bool; 2],
    /// Byte 0xB of the card records (`roses_assets::parse_pal_reward_flags`
    /// → `RewardCardTable::from_record_flag_bytes`): bit 5 = the card may
    /// appear on the reels (0x252A38).
    pub reward_flags: RewardCardTable,
    pub timings: EndTimings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DuelEndError {
    /// The duel has no outcome yet.
    NotEnded,
    /// A card without a record in `cards`.
    UnknownCard(u16),
    /// `reward_flags` must have one entry per card id.
    RewardFlagsLength(usize),
    /// A side index outside 0..=1.
    InvalidSide(u8),
}
impl fmt::Display for DuelEndError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "duel end: {self:?}")
    }
}
impl std::error::Error for DuelEndError {}

impl DuelEndSetup {
    /// Read an ended duel. `map`, `sound`, `special_draw_announced`,
    /// `reward_flags` and `timings` are not in `Duel` and are passed in.
    pub fn from_duel(
        duel: &Duel,
        cards: &impl CardData,
        map: u8,
        sound: bool,
        special_draw_announced: [bool; 2],
        reward_flags: RewardCardTable,
        timings: EndTimings,
    ) -> Result<Self, DuelEndError> {
        let outcome = duel.outcome.ok_or(DuelEndError::NotEnded)?;
        if reward_flags.reel_eligible.len() != usize::from(CARD_COUNT) {
            return Err(DuelEndError::RewardFlagsLength(
                reward_flags.reel_eligible.len(),
            ));
        }
        let leader = |side: u8| {
            duel.units
                .iter()
                .find(|u| u.side == side && u.slot == 0)
                .map(|u| u.card)
                .ok_or(DuelEndError::InvalidSide(side))
        };
        let leaders = [leader(0)?, leader(1)?];
        let mut leader_types = [0; 2];
        for (t, &card) in leader_types.iter_mut().zip(&leaders) {
            *t = cards
                .card(card)
                .ok_or(DuelEndError::UnknownCard(card))?
                .kind
                & 0x1F;
        }
        Ok(Self {
            outcome,
            human: [
                duel.controllers[0] >= PAD_CONTROLLER,
                duel.controllers[1] >= PAD_CONTROLLER,
            ],
            first: duel.first,
            map,
            sound,
            experience: duel.final_experience(),
            leaders,
            graveyards: [
                duel.sides[0].graveyard.clone(),
                duel.sides[1].graveyard.clone(),
            ],
            leader_types,
            special_draw_announced,
            reward_flags,
            timings,
        })
    }

    /// D50: 0/1 the winner, 2 a draw.
    pub fn winner_code(&self) -> u8 {
        self.outcome.winner.unwrap_or(2)
    }

    /// D51 (turn-verified §7).
    pub fn reason_code(&self) -> u8 {
        match self.outcome.reason {
            EndReason::LifePoints | EndReason::Surrender => 0,
            EndReason::TurnLimit => 1,
            EndReason::Exodia => 2,
            EndReason::LeaderTrapped => 3,
        }
    }

    /// State 0x62's test: a pad-driven side won.
    pub fn human_won(&self) -> bool {
        matches!(self.outcome.winner, Some(w) if w < 2 && self.human[usize::from(w)])
    }
}

/// How the sequence ends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EndOutcome {
    /// 0x1A9130(0xF, −1): the duel mode is replaced by the slot machine
    /// (`slot_machine::SlotMachine::new` takes the data).
    SlotMachine(ReelData),
    /// 0x1A9290 with one human: back to the map (the save event, if any,
    /// was reported at state 0x5D).
    ReturnToMap { human_won: bool },
    /// 0x1A9290 at state 0x5D when both or neither side is pad-driven: no
    /// save, no rank list, no reels.
    Return,
}

/// What a frame did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EndEvent {
    /// 0x217110(track, arg): arg 0 at state 0x54, 1 at 0x55.
    Music { track: u8, arg: u8 },
    /// 0x216EA0(0, id).
    Sfx { id: u8 },
    /// State 0x58: 0x1DA360(0x40, …) moves the camera to this side's leader.
    CameraToLeader { side: u8 },
    /// State 0x5D: 0x2745F0 ran and the blocks were changed.
    ExperienceApplied,
    /// State 0x5D: 0x227F00(won) — the campaign save counters; apply with
    /// `campaign_map::CampaignSave::apply_duel_end`.
    SaveDuelEnd { won: bool },
    /// State 0x5E: the ranks were recomputed and stored; the list (may be
    /// empty) is shown one entry per press.
    RankList { side: u8, changes: Vec<RankChange> },
    /// The mode call: nothing runs after it.
    Finished(EndOutcome),
}

/// The visible state after a frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndView {
    /// D57.
    pub state: u8,
    /// CB2: banner string id ([`banner`]), 0x18 = none.
    pub banner: u8,
    /// c9c (s16 0x2DB83C); the banner's x is 0x200 − 8·c9c.
    pub slide: i16,
    /// c9a (0x2DB83A), cleared with c9c; no reader traced in 0x54..0x62.
    pub slide_aux: i16,
    /// D3A: scene darkening 0..=0x80.
    pub darken: u8,
    /// 0x1FC8B0(0, 0, 1) is called this frame (states 0x5C/0x5D with a win
    /// banner).
    pub camera_orbit: bool,
    /// CAA: the rank entry shown (states 0x5F–0x61) and its new rank.
    pub rank_entry: Option<RankEntryView>,
    pub finished: Option<EndOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankEntryView {
    pub side: u8,
    pub card: u16,
    pub demotion: bool,
    /// The stored rank after the recompute (MSG 83 + rank).
    pub new_rank: u16,
}
impl EndView {
    pub fn banner_x(&self) -> i32 {
        BANNER_X_ORIGIN - 8 * i32::from(self.slide)
    }
}

enum Flow {
    /// 0x1D8F54 with v0 = 0: the frame ends.
    Return,
    /// 0x1D8F54 with v0 = 1: the caller runs the machine again in the same
    /// frame (0x1AAF44/0x1AAF5C: `jal 0x1D00F0; bnez v0`), a fresh dispatch;
    /// the emu shows c9c = 1 in the frame 0x58 ran.
    Again,
    /// 0x1D8F38: re-dispatch iff the state changed since the last check.
    Check,
    /// Straight into the next state's code.
    Fallthrough,
}

/// The stepped sequence. Create with [`DuelEnd::new`] once the duel has an
/// outcome and call [`DuelEnd::step`] every frame until the view reports
/// `finished`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelEnd {
    setup: DuelEndSetup,
    thresholds: [u16; RANK_COUNT],
    frame: u64,
    state: u8,
    /// D35: the banner code chosen at 0x54, then a countdown at 0x57.
    d35: u8,
    /// D36 / D54: the side whose leader the camera shows.
    d36: u8,
    d54: u8,
    banner: u8,
    c9c: i16,
    c9a: i16,
    d3a: u8,
    /// c8a / c8c: rank list cursor and count.
    c8a: u16,
    c8c: u16,
    caa: u16,
    rank_side: u8,
    rank_list: Vec<RankChange>,
    sound_until: u64,
    hud_until: u64,
    camera_until: u64,
    camera_orbit: bool,
    finished: Option<EndOutcome>,
}

impl DuelEnd {
    /// `thresholds` = `roses_assets::parse_pal_rank_thresholds`.
    pub fn new(setup: DuelEndSetup, thresholds: [u16; RANK_COUNT]) -> Self {
        Self {
            setup,
            thresholds,
            frame: 0,
            state: 0x54,
            d35: 0,
            d36: 0,
            d54: 0,
            banner: banner::OFF,
            c9c: 0,
            c9a: 0,
            d3a: 0,
            c8a: 0,
            c8c: 0,
            caa: 0,
            rank_side: 0,
            rank_list: Vec::new(),
            sound_until: 0,
            hud_until: 0,
            camera_until: 0,
            camera_orbit: false,
            finished: None,
        }
    }

    pub fn setup(&self) -> &DuelEndSetup {
        &self.setup
    }

    /// Frames stepped so far.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn view(&self) -> EndView {
        let rank_entry = (self.state >= 0x5F && self.state <= 0x61)
            .then(|| self.rank_list.get(usize::from(self.c8a)))
            .flatten()
            .map(|change| RankEntryView {
                side: self.rank_side,
                card: change.card,
                demotion: change.demotion(),
                new_rank: change.new,
            });
        EndView {
            state: self.state,
            banner: self.banner,
            slide: self.c9c,
            slide_aux: self.c9a,
            darken: self.d3a,
            camera_orbit: self.camera_orbit,
            rank_entry,
            finished: self.finished.clone(),
        }
    }

    /// One frame of 0x1D00F0 in the result states. `pad_new` is the OR of
    /// the pad-driven sides' new-press words (bits 4–7 = △○×□; 0x1CFF80 /
    /// 0x203380(port, 0, 0xFFFF)). `blocks` are the two sides' card blocks
    /// (index = side; the pad-driven side's is the persistent one). After
    /// the mode call the sequence is finished and further steps do nothing.
    pub fn step(&mut self, pad_new: u16, blocks: &mut [CardBlock; 2]) -> Vec<EndEvent> {
        let mut events = Vec::new();
        if self.finished.is_some() {
            return events;
        }
        self.camera_orbit = false;
        let pressed = pad_new & PRESS_MASK != 0;
        let mut checked = self.state;
        loop {
            let flow = self.run_state(pressed, blocks, &mut events);
            match flow {
                Flow::Return => break,
                Flow::Again => checked = self.state,
                Flow::Fallthrough => {}
                Flow::Check => {
                    if self.state == checked || self.finished.is_some() {
                        break;
                    }
                    checked = self.state;
                }
            }
        }
        self.frame += 1;
        events
    }

    fn win_banner(&self) -> bool {
        matches!(
            self.banner,
            banner::YOU_WIN | banner::P1_WIN | banner::P2_WIN
        )
    }

    fn run_state(
        &mut self,
        pressed: bool,
        blocks: &mut [CardBlock; 2],
        events: &mut Vec<EndEvent>,
    ) -> Flow {
        let [h0, h1] = self.setup.human;
        let d50 = self.setup.winner_code();
        match self.state {
            // 0x1D84A8: banner and camera side (§0), the music cue.
            0x54 => {
                if h0 == h1 {
                    if d50 < 2 {
                        self.d35 = d50 + 4;
                        self.d36 = d50;
                    } else {
                        self.d35 = banner::DRAW;
                    }
                } else if h0 {
                    self.d35 = if d50 == 0 {
                        banner::YOU_WIN
                    } else {
                        banner::YOU_LOSE
                    };
                    self.d36 = 0;
                } else {
                    self.d35 = if d50 == 1 {
                        banner::YOU_WIN
                    } else {
                        banner::YOU_LOSE
                    };
                    self.d36 = 1;
                }
                if self.setup.sound {
                    if let Some(track) = music_for(self.d35) {
                        events.push(EndEvent::Music { track, arg: 0 });
                    }
                }
                self.state = 0x55;
                self.sound_until = self.frame + u64::from(self.setup.timings.sound_idle_frames);
                Flow::Fallthrough
            }
            // 0x1D8600: wait for the sound, restart the track, arm the banner.
            0x55 => {
                if self.frame < self.sound_until {
                    return Flow::Check;
                }
                if self.setup.sound {
                    if let Some(track) = music_for(self.d35) {
                        events.push(EndEvent::Music { track, arg: 1 });
                    }
                }
                self.d54 = self.d36;
                self.banner = self.d35;
                self.c9c = 0;
                self.c9a = 0;
                self.state = 0x56;
                self.hud_until = self.frame + u64::from(self.setup.timings.hud_slide_frames);
                Flow::Fallthrough
            }
            // 0x1D86B0: wait for the HUD panel.
            0x56 => {
                if self.frame < self.hud_until {
                    return Flow::Check;
                }
                self.d35 = 0;
                self.state = 0x57;
                Flow::Fallthrough
            }
            // 0x1D86C8: D35 countdown (always 0 here).
            0x57 => {
                if self.d35 != 0 {
                    self.d35 -= 1;
                    return Flow::Return;
                }
                self.state = 0x58;
                Flow::Fallthrough
            }
            // 0x1D86E8: camera, choose the slide state.
            0x58 => {
                self.state = match self.banner {
                    banner::YOU_WIN | banner::P1_WIN | banner::P2_WIN => {
                        events.push(EndEvent::CameraToLeader { side: self.d54 });
                        self.camera_until =
                            self.frame + u64::from(self.setup.timings.camera_wait_frames);
                        0x59
                    }
                    banner::YOU_LOSE => 0x5A,
                    _ => 0x5B,
                };
                self.d3a = 0;
                Flow::Again
            }
            // 0x1D87AC / 0x1D87FC / 0x1D8868: slide in (and darken on a loss).
            0x59..=0x5B => {
                if self.state == 0x5A {
                    self.d3a = self.d3a.saturating_add(2).min(DARKEN_MAX);
                }
                self.c9c = (self.c9c + 1).min(SLIDE_IN);
                if self.c9c < SLIDE_IN {
                    return Flow::Check;
                }
                if self.state == 0x59 && self.frame < self.camera_until {
                    return Flow::Check;
                }
                if self.state == 0x5A && self.d3a < DARKEN_MAX {
                    return Flow::Check;
                }
                self.state = 0x5C;
                Flow::Again
            }
            // 0x1D88A8: orbit the camera, wait for a new △○×□ press.
            0x5C => {
                self.camera_orbit = self.win_banner();
                if !pressed {
                    return Flow::Check;
                }
                self.state = 0x5D;
                events.push(EndEvent::Sfx { id: SFX_CONFIRM });
                Flow::Return
            }
            // 0x1D8A88: slide out and darken, then experience, save, pop or
            // the rank list.
            0x5D => {
                self.camera_orbit = self.win_banner();
                self.c9c = (self.c9c + 1).min(SLIDE_OUT);
                self.d3a = self.d3a.saturating_add(2).min(DARKEN_MAX);
                if self.c9c < SLIDE_OUT || self.d3a < DARKEN_MAX {
                    return Flow::Check;
                }
                self.banner = banner::OFF;
                for (side, changes) in self.setup.experience.iter().enumerate() {
                    for (&card, change) in changes {
                        blocks[side].apply_change(card, change);
                    }
                }
                events.push(EndEvent::ExperienceApplied);
                if h0 == h1 {
                    self.finished = Some(EndOutcome::Return);
                    events.push(EndEvent::Finished(EndOutcome::Return));
                    return Flow::Check;
                }
                if self.setup.map < CAMPAIGN_MAPS {
                    events.push(EndEvent::SaveDuelEnd {
                        won: self.setup.first == d50,
                    });
                }
                self.state = 0x5E;
                Flow::Fallthrough
            }
            // 0x1D8CB0: recompute the human's ranks, list the changes.
            0x5E => {
                self.c8a = 0;
                self.c8c = 0;
                // Exactly one side is pad-driven here (h0 == h1 popped).
                let side = if h0 { 0 } else { 1 };
                self.rank_side = side;
                self.rank_list =
                    blocks[usize::from(side)].recompute_all_ranks(&self.thresholds, |_| true);
                self.c8c = self.rank_list.len() as u16;
                events.push(EndEvent::RankList {
                    side,
                    changes: self.rank_list.clone(),
                });
                if self.rank_list.is_empty() {
                    self.state = 0x62;
                } else {
                    self.banner = banner::PROMOTION;
                    self.c9a = 0;
                    self.c9c = 0;
                    self.state = 0x5F;
                }
                Flow::Check
            }
            // 0x1D8E08: show the entry.
            0x5F => {
                self.caa = self.rank_list[usize::from(self.c8a)].entry_code(self.rank_side);
                events.push(EndEvent::Sfx { id: SFX_CONFIRM });
                self.state = 0x60;
                Flow::Fallthrough
            }
            // 0x1D8E38: slide by 8 per pass.
            0x60 => {
                self.c9c = (self.c9c + 8).min(SLIDE_IN);
                if self.c9c < SLIDE_IN {
                    return Flow::Check;
                }
                self.state = 0x61;
                Flow::Fallthrough
            }
            // 0x1D8E70: wait for a press, next entry or done.
            0x61 => {
                if !pressed {
                    return Flow::Return;
                }
                self.c8a += 1;
                if self.c8a < self.c8c {
                    self.state = 0x5F;
                } else {
                    self.banner = banner::OFF;
                    events.push(EndEvent::Sfx { id: SFX_CONFIRM });
                    self.state = 0x62;
                }
                Flow::Return
            }
            // 0x1D8ED0: reels or pop.
            0x62 => {
                let outcome = if self.setup.human_won() {
                    let winner = usize::from(d50);
                    let loser = winner ^ 1;
                    let input = RewardListInput {
                        loser_graveyard: self.setup.graveyards[loser].clone(),
                        loser_deck_leader: self.setup.leaders[loser],
                        winner_leader_kind: self.setup.leader_types[winner],
                        winner_leader_rank: blocks[winner]
                            .stored_rank(self.setup.leaders[winner])
                            .map(|r| r as u8),
                        winner_special_draw_announced: self.setup.special_draw_announced[winner],
                    };
                    match build_reel_data(&input, &self.setup.reward_flags) {
                        Some(data) => EndOutcome::SlotMachine(data),
                        None => EndOutcome::ReturnToMap { human_won: true },
                    }
                } else {
                    EndOutcome::ReturnToMap { human_won: false }
                };
                self.finished = Some(outcome.clone());
                events.push(EndEvent::Finished(outcome));
                Flow::Check
            }
            _ => Flow::Return,
        }
    }
}

/// 0x1D8588 / 0x1D8610: the track for a banner code (none for DRAW).
fn music_for(code: u8) -> Option<u8> {
    match code {
        banner::YOU_WIN | banner::P1_WIN | banner::P2_WIN => Some(MUSIC_WIN),
        banner::YOU_LOSE => Some(MUSIC_LOSE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card_block::{CardRecord, OWNED_BIT};

    const T: [u16; RANK_COUNT] = [
        0, 500, 1000, 2000, 4000, 7000, 11000, 16000, 22000, 30000, 40000, 52000, 65535,
    ];

    /// The 767 capture as run1.py sees it: player (side 0, human) vs Weevil,
    /// player's graveyard 13 cards, Weevil's [817, 406, 757, 809], leaders
    /// 392 / 397 (types 8 / 9), reel-eligible everything but 409.
    fn setup(winner: Option<u8>) -> DuelEndSetup {
        let mut experience: [BTreeMap<u16, ExperienceChange>; 2] = Default::default();
        let mut add = |side: usize, card: u16, v: i32| {
            experience[side].entry(card).or_default().add(v);
        };
        match winner {
            Some(0) => {
                add(0, 221, 4);
                add(0, 392, 24);
                add(1, 397, -32);
            }
            Some(1) => {
                for (c, v) in [(396, 4), (399, 4), (400, 4), (425, 12)] {
                    add(1, c, v);
                }
                add(1, 397, 40);
                add(0, 392, -16);
            }
            _ => add(0, 392, -16),
        }
        let mut reward_flags = RewardCardTable::from_record_flag_bytes(std::iter::repeat_n(
            0x20u8,
            usize::from(CARD_COUNT),
        ));
        reward_flags.reel_eligible[409] = false;
        DuelEndSetup {
            outcome: Outcome {
                winner,
                reason: EndReason::LifePoints,
            },
            human: [true, false],
            first: 0,
            map: 2,
            sound: true,
            experience,
            leaders: [392, 397],
            graveyards: [
                vec![370, 133, 763, 757, 691, 369, 804, 4, 15, 374, 383, 244, 385],
                vec![817, 406, 757, 809],
            ],
            leader_types: [8, 9],
            special_draw_announced: [false, false],
            reward_flags,
            // The scripts poke the HUD panel to 0x80 at the first frame that
            // starts in 0x56, so 0x56 waited exactly one frame there.
            timings: EndTimings {
                hud_slide_frames: 1,
                ..EndTimings::default()
            },
        }
    }

    fn blocks() -> [CardBlock; 2] {
        let mut b0 = CardBlock::empty(0x6D92_1E28);
        b0.records[221] = CardRecord {
            rank_word: OWNED_BIT,
            experience: 2,
            ..Default::default()
        };
        b0.records[392] = CardRecord {
            rank_word: OWNED_BIT | 1,
            experience: 500,
            ..Default::default()
        };
        let mut b1 = CardBlock::empty(0x4000_001C);
        b1.records[397] = CardRecord {
            rank_word: OWNED_BIT | 2,
            experience: 1000,
            ..Default::default()
        };
        [b0, b1]
    }

    /// Drive like run1.py: a press every fourth frame in 0x5C/0x61.
    fn drive(end: &mut DuelEnd, blocks: &mut [CardBlock; 2]) -> Vec<(u64, u8, Vec<EndEvent>)> {
        let mut log = Vec::new();
        let mut last = 0xFF;
        for f in 0..900 {
            let st = end.view().state;
            // run1.py's frame f is this frame + 1 (its frame 0 ends the rules).
            let pad = if matches!(st, 0x5C | 0x61) && (f + 1) % 4 == 0 {
                0x40
            } else {
                0
            };
            let events = end.step(pad, blocks);
            if st != last || !events.is_empty() {
                log.push((f, st, events));
                last = st;
            }
            if end.view().finished.is_some() {
                break;
            }
        }
        log
    }

    fn state_frames(log: &[(u64, u8, Vec<EndEvent>)]) -> Vec<(u64, u8)> {
        let mut out: Vec<(u64, u8)> = Vec::new();
        for (f, st, _) in log {
            if out.last().map(|l| l.1) != Some(*st) {
                out.push((*f, *st));
            }
        }
        out
    }

    #[test]
    fn win_matches_run1() {
        let mut end = DuelEnd::new(setup(Some(0)), T);
        let mut blocks = blocks();
        let log = drive(&mut end, &mut blocks);
        // run1: 0x54 f1, 0x56 f2, 0x59 f3, 0x5C f66, press f68, 0x5D f69,
        // mode 0xF at f132 (script frames are one higher; 0x5E and 0x62
        // run inside 0x5D's last frame and never start a frame).
        assert_eq!(
            state_frames(&log),
            vec![(0, 0x54), (1, 0x56), (2, 0x59), (65, 0x5C), (68, 0x5D)]
        );
        let all: Vec<&EndEvent> = log.iter().flat_map(|l| l.2.iter()).collect();
        assert_eq!(
            all[..3],
            [
                &EndEvent::Music {
                    track: MUSIC_WIN,
                    arg: 0
                },
                &EndEvent::Music {
                    track: MUSIC_WIN,
                    arg: 1
                },
                &EndEvent::CameraToLeader { side: 0 },
            ]
        );
        assert!(all.contains(&&EndEvent::Sfx { id: 4 }));
        assert!(all.contains(&&EndEvent::SaveDuelEnd { won: true }));
        assert_eq!(end.frame(), 132);
        let finished = end.view().finished.unwrap();
        assert_eq!(
            finished,
            EndOutcome::SlotMachine(ReelData {
                strip: vec![817, 406, 757, 809, 397, 820, 820, 820],
                random_slip: true,
                enabled_lines: [true, false, false, false, false],
            })
        );
        assert_eq!(blocks[0].experience(221), Some(6));
        assert_eq!(blocks[0].experience(392), Some(524));
        assert_eq!(blocks[1].experience(397), Some(968));
        assert!(matches!(
            all.iter().find(|e| matches!(e, EndEvent::RankList { .. })),
            Some(EndEvent::RankList { side: 0, changes }) if changes.is_empty()
        ));
        // Nothing more happens after the mode call.
        assert!(end.step(0xF0, &mut blocks).is_empty());
    }

    #[test]
    fn loss_and_draw_match_run1b() {
        for winner in [Some(1), None] {
            let mut end = DuelEnd::new(setup(winner), T);
            let mut blocks = blocks();
            let log = drive(&mut end, &mut blocks);
            assert_eq!(
                state_frames(&log),
                vec![(0, 0x54), (1, 0x56), (2, 0x5A), (65, 0x5C), (68, 0x5D)]
            );
            assert_eq!(end.frame(), 132);
            let all: Vec<&EndEvent> = log.iter().flat_map(|l| l.2.iter()).collect();
            assert_eq!(
                all[0],
                &EndEvent::Music {
                    track: MUSIC_LOSE,
                    arg: 0
                }
            );
            assert!(!all
                .iter()
                .any(|e| matches!(e, EndEvent::CameraToLeader { .. })));
            assert!(all.contains(&&EndEvent::SaveDuelEnd { won: false }));
            assert_eq!(
                end.view().finished,
                Some(EndOutcome::ReturnToMap { human_won: false })
            );
            assert_eq!(blocks[0].experience(392), Some(484));
            // The loss darkened the scene during the slide-in.
            assert_eq!(end.view().darken, DARKEN_MAX);
        }
        let mut end = DuelEnd::new(setup(Some(1)), T);
        let mut blocks = blocks();
        drive(&mut end, &mut blocks);
        assert_eq!(blocks[1].experience(397), Some(1040));
        assert_eq!(blocks[1].experience(425), Some(12));
        // The draw shows YOU LOSE vs the computer.
        let mut end = DuelEnd::new(setup(None), T);
        let mut draw_blocks = self::tests::blocks();
        end.step(0, &mut draw_blocks);
        assert_eq!(end.view().banner, banner::YOU_LOSE);
    }

    #[test]
    fn rank_list_matches_run4() {
        for (winner, expect_entry, expect_final) in
            [(Some(0), 0x00DD, 502u16), (Some(1), 0x4188, 989)]
        {
            let mut end = DuelEnd::new(setup(winner), T);
            let mut blocks = blocks();
            blocks[0].records[221].experience = 498;
            blocks[0].records[392].rank_word = OWNED_BIT | 2;
            blocks[0].records[392].experience = 1005;
            let log = drive(&mut end, &mut blocks);
            // run4: 0x60 printed at f133 (entered in f132 with c9c = 16),
            // 0x61 at f139 (c9c 64 at f138), press f140, 0x62 at f141
            // (script frames are one higher).
            let frames = state_frames(&log);
            assert_eq!(
                &frames[frames.len() - 3..],
                &[(132, 0x60), (138, 0x61), (140, 0x62)]
            );
            assert_eq!(end.frame(), 141);
            let all: Vec<&EndEvent> = log.iter().flat_map(|l| l.2.iter()).collect();
            let list = all
                .iter()
                .find_map(|e| match e {
                    EndEvent::RankList { changes, .. } => Some(changes.clone()),
                    _ => None,
                })
                .unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].entry_code(0), expect_entry);
            let card = if winner == Some(0) { 221 } else { 392 };
            assert_eq!(blocks[0].experience(card), Some(expect_final));
            // Three confirm sounds: the banner press, the entry, the last press.
            assert_eq!(
                all.iter()
                    .filter(|e| matches!(e, EndEvent::Sfx { id: 4 }))
                    .count(),
                3
            );
        }
    }

    #[test]
    fn first_promotion_frame_slides_by_16_and_later_entries_do_not_slide() {
        let mut end = DuelEnd::new(setup(Some(0)), T);
        let mut blocks = blocks();
        blocks[0].records[221].experience = 498;
        blocks[0].records[4] = CardRecord {
            rank_word: OWNED_BIT | 3,
            experience: 100,
            ..Default::default()
        };
        // Reach the end of 0x5D.
        while end.view().state != 0x5C {
            end.step(0, &mut blocks);
        }
        end.step(0x10, &mut blocks);
        while end.view().state == 0x5D {
            end.step(0, &mut blocks);
        }
        let v = end.view();
        assert_eq!((v.state, v.slide, v.banner), (0x60, 16, banner::PROMOTION));
        let e = v.rank_entry.unwrap();
        assert_eq!((e.card, e.demotion, e.new_rank), (4, true, 1));
        for expect in [24, 32, 40, 48, 56] {
            end.step(0, &mut blocks);
            assert_eq!(end.view().slide, expect);
        }
        // c9c reaches 64 and the press is accepted in the same frame.
        end.step(0x20, &mut blocks);
        let v = end.view();
        assert_eq!((v.state, v.slide), (0x5F, 64));
        end.step(0, &mut blocks);
        let v = end.view();
        assert_eq!((v.state, v.slide), (0x61, 64));
        assert_eq!(v.rank_entry.unwrap().card, 221);
        // A held button is not a new press: the caller passes 0.
        end.step(0, &mut blocks);
        assert_eq!(end.view().state, 0x61);
        end.step(0x80, &mut blocks);
        assert_eq!((end.view().state, end.view().banner), (0x62, banner::OFF));
        end.step(0, &mut blocks);
        assert!(matches!(
            end.view().finished,
            Some(EndOutcome::SlotMachine(_))
        ));
    }

    #[test]
    fn two_pad_sides_pop_at_0x5d_without_save_or_ranks() {
        let mut s = setup(Some(1));
        s.human = [true, true];
        let mut end = DuelEnd::new(s, T);
        let mut bl = blocks();
        bl[0].records[392].experience = 1005;
        bl[0].records[392].rank_word = OWNED_BIT | 2;
        let log = drive(&mut end, &mut bl);
        assert_eq!(
            log[0].2[0],
            EndEvent::Music {
                track: MUSIC_WIN,
                arg: 0
            }
        );
        end.step(0, &mut bl);
        assert_eq!(log.iter().find(|l| l.1 == 0x56).map(|_| ()), Some(()));
        let all: Vec<&EndEvent> = log.iter().flat_map(|l| l.2.iter()).collect();
        assert!(all.contains(&&EndEvent::CameraToLeader { side: 1 }));
        assert!(!all
            .iter()
            .any(|e| matches!(e, EndEvent::SaveDuelEnd { .. })));
        assert!(!all.iter().any(|e| matches!(e, EndEvent::RankList { .. })));
        assert_eq!(end.view().finished, Some(EndOutcome::Return));
        // Experience applied, rank not recomputed.
        assert_eq!(bl[0].experience(392), Some(989));
        assert_eq!(bl[0].stored_rank(392), Some(2));
        let mut s = setup(None);
        s.human = [true, true];
        let mut end = DuelEnd::new(s, T);
        let log = drive(&mut end, &mut blocks());
        assert!(log[0].2.is_empty());
        end.step(0, &mut blocks());
        assert_eq!(state_frames(&log)[2], (2, 0x5B));
    }

    #[test]
    fn waits_delay_the_sequence() {
        let mut s = setup(Some(0));
        s.timings = EndTimings {
            sound_idle_frames: 3,
            hud_slide_frames: 5,
            camera_wait_frames: 70,
        };
        let mut end = DuelEnd::new(s, T);
        let mut blocks = blocks();
        let log = drive(&mut end, &mut blocks);
        // 0x55 waits frames 0..2 and passes in frame 3, 0x56 waits frames
        // 3..7, 0x58 runs in frame 8 (camera started); 0x59 slides to 64 by
        // frame 72 and waits for the camera until frame 78.
        assert_eq!(
            state_frames(&log)[..5],
            [(0, 0x54), (1, 0x55), (4, 0x56), (9, 0x59), (79, 0x5C)]
        );
    }

    #[test]
    fn reels_need_a_pad_winner_and_a_list() {
        // The computer won: no reels even with a list.
        let mut end = DuelEnd::new(setup(Some(1)), T);
        drive(&mut end, &mut blocks());
        assert_eq!(
            end.view().finished,
            Some(EndOutcome::ReturnToMap { human_won: false })
        );
        // A pad winner whose opponent leaves no eligible card pops too.
        let mut s = setup(Some(0));
        s.graveyards[1] = vec![409, 409];
        s.leaders[1] = 409;
        let mut end = DuelEnd::new(s, T);
        drive(&mut end, &mut blocks());
        assert_eq!(
            end.view().finished,
            Some(EndOutcome::ReturnToMap { human_won: true })
        );
        // The winner's rank read at 0x62 is the one stored at 0x5E, and a
        // special draw turns the slip off.
        let mut s = setup(Some(0));
        s.special_draw_announced = [true, false];
        let mut end = DuelEnd::new(s, T);
        let mut bl = blocks();
        bl[0].records[392].experience = 3990; // +24 → 4014 → rank 4
        drive(&mut end, &mut bl);
        match end.view().finished {
            Some(EndOutcome::SlotMachine(data)) => {
                assert!(!data.random_slip);
                assert_eq!(data.enabled_lines, [true, true, false, false, false]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn setup_codes() {
        let mut s = setup(Some(0));
        assert_eq!(s.winner_code(), 0);
        assert!(s.human_won());
        s.outcome.winner = Some(1);
        assert!(!s.human_won());
        s.outcome.winner = None;
        assert_eq!(s.winner_code(), 2);
        assert!(!s.human_won());
        assert_eq!(s.reason_code(), 0);
        s.outcome.reason = EndReason::LeaderTrapped;
        assert_eq!(s.reason_code(), 3);
        s.outcome.reason = EndReason::TurnLimit;
        assert_eq!(s.reason_code(), 1);
    }

    /// The captured blocks of state 767 (`ROSES_TEST_END_FIXTURE`, see
    /// `card_block.rs`): the whole sequence on the real blocks.
    #[test]
    #[ignore = "requires the private savestate dump; set ROSES_TEST_END_FIXTURE"]
    fn private_capture_sequence_matches_run1_and_run4() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("ROSES_TEST_END_FIXTURE").expect("ROSES_TEST_END_FIXTURE"),
        );
        let load = || {
            [
                CardBlock::from_bytes(&std::fs::read(dir.join("blk0.bin")).unwrap()).unwrap(),
                CardBlock::from_bytes(&std::fs::read(dir.join("blk1.bin")).unwrap()).unwrap(),
            ]
        };
        let thresholds = T;
        // run1 (win): only experience changes, no rank entries.
        let mut blocks = load();
        let before = blocks.clone();
        let mut end = DuelEnd::new(setup(Some(0)), thresholds);
        drive(&mut end, &mut blocks);
        assert_eq!(blocks[0].experience(221), Some(6));
        assert_eq!(blocks[0].experience(392), Some(524));
        assert_eq!(blocks[1].experience(397), Some(968));
        let changed = |a: &CardBlock, b: &CardBlock| {
            (0..854)
                .filter(|&c| a.records[c] != b.records[c])
                .collect::<Vec<_>>()
        };
        assert_eq!(changed(&before[0], &blocks[0]), vec![221, 392]);
        assert_eq!(changed(&before[1], &blocks[1]), vec![397]);
        assert!(
            matches!(end.view().finished, Some(EndOutcome::SlotMachine(ref r)) if r.strip == [817, 406, 757, 809, 397, 820, 820, 820] && r.random_slip)
        );
        // run4 (win / loss) with the poked records.
        for (winner, entry) in [(Some(0), 0x00DDu16), (Some(1), 0x4188)] {
            let mut blocks = load();
            blocks[0].records[221].rank_word = 0x4000;
            blocks[0].records[221].experience = 498;
            blocks[0].records[392].rank_word = 0x4002;
            blocks[0].records[392].experience = 1005;
            let mut end = DuelEnd::new(setup(winner), thresholds);
            let log = drive(&mut end, &mut blocks);
            let list = log
                .iter()
                .flat_map(|l| l.2.iter())
                .find_map(|e| match e {
                    EndEvent::RankList { changes, .. } => Some(changes.clone()),
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                list.iter().map(|c| c.entry_code(0)).collect::<Vec<_>>(),
                vec![entry]
            );
            assert_eq!(end.frame(), 141);
        }
    }
}
