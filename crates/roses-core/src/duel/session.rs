//! One duel as a client runs it: the rules state, the pad-driven input
//! states ([`super::ui`]) and computer seats, advanced one 50 Hz frame at a
//! time. A computer seat plans with the ported controller
//! ([`super::ai::weevil`]), turns each plan into its key queue
//! ([`super::ai::keys`]) and presses the keys into the same input states a
//! human uses, as the original's driver 0x1117D0 does, including its
//! 600-key watchdog (Circle three times, then Start).
//!
//! Timing is only approximate: the original consumes a key whenever its
//! input states poll (between walking, battle and summon animations, none of
//! which are modelled here), so a computer seat presses one key every
//! [`COMPUTER_KEY_FRAMES`] frames, the interval measured between its cursor
//! keys in a recorded turn.
//!
//! A practice lesson ([`super::script`]) replaces both seats: the shared
//! script is polled every frame (the original polls whenever the input
//! layer is in a polled sub-state and nothing is busy; no animation is
//! modelled, so that is every frame), its key goes through the same input
//! states, and the human pad only pages the messages (face buttons) or
//! quits the series (Start). No watchdog, no per-turn reset [F].

use super::ai::{self, pad, weevil};
use super::script::{ScriptFrame, Tutorial};
use super::ui::{DuelUi, Response};
use super::{CardData, Duel, DuelCommand, DuelError, DuelEvent, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Frames between two computer key presses. The recorded Weevil turns show
/// his cursor keys consumed about every 0.32 s (16 frames at 50 Hz) with
/// PCSX2 at full speed (`ai-weevil-t16.jsonl`); the per-state input gating
/// that produces that pace (camera tracks, 0x1FB900 waits) is not modelled,
/// so every key uses this interval.
pub const COMPUTER_KEY_FRAMES: u8 = 16;
/// 0x1117D0 watchdog S+0x1A2: counted on every controller call of a turn
/// (NoKey and end-turn calls included); calls 600-602 return Circle, every
/// later call Start (WEEVIL-CONTROLLER.md section 1).
pub const WATCHDOG_CALLS: usize = 600;

/// A side played by the ported controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputerSeat {
    pub memory: weevil::Memory,
    /// Keys of the current plan not yet pressed.
    pub queue: VecDeque<u16>,
    /// Controller calls in the current turn (the watchdog counter +0x1A2).
    pub calls: usize,
    /// Frames until the next press.
    pub wait: u8,
    /// s16 0x29922E (0 for the first campaign duel).
    pub tier: i16,
    /// The side's control byte (+0x33C), which selects the opponent module
    /// ([`weevil::plan_for`]; Weevil 2, Pegasus 9).
    #[serde(default = "weevil_controller")]
    pub controller: u8,
}

fn weevil_controller() -> u8 {
    2
}

impl ComputerSeat {
    /// A seat played by Weevil's module (control byte 2).
    pub fn new(duel: &Duel, side: u8, tier: i16) -> Self {
        Self::with_controller(duel, side, tier, weevil_controller())
    }

    /// A seat played by the module of `controller` (see
    /// [`weevil::PORTED_CONTROLLERS`]; an unported byte makes every plan
    /// fail with [`DuelError::Unsupported`]).
    pub fn with_controller(duel: &Duel, side: u8, tier: i16, controller: u8) -> Self {
        Self {
            memory: weevil::Memory::new(duel, side),
            queue: VecDeque::new(),
            calls: 0,
            wait: 0,
            tier,
            controller,
        }
    }
}

/// What one frame did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// The key pressed this frame (human or computer), if any.
    pub key: Option<u16>,
    /// How the input states took it.
    pub response: Option<Response>,
    /// Engine events of the commands it produced.
    pub events: Vec<DuelEvent>,
    /// The practice script's poll (presentation one-shots, lesson end).
    pub script: Option<ScriptFrame>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuelSession {
    pub duel: Duel,
    pub input: DuelUi,
    /// `None` = a human pad.
    pub seats: [Option<ComputerSeat>; 2],
    /// A practice lesson driving both sides (the seats are then unused).
    #[serde(default)]
    pub tutorial: Option<Tutorial>,
}

impl DuelSession {
    pub fn new(duel: Duel, seats: [Option<ComputerSeat>; 2]) -> Self {
        let input = DuelUi::new(&duel);
        Self {
            duel,
            input,
            seats,
            tutorial: None,
        }
    }

    /// A practice lesson: `duel` from [`super::script::lesson_setup`],
    /// `tutorial` from [`Tutorial::series`] or [`Tutorial::new`].
    /// A lesson that has not been polled yet gets the per-lesson duel init
    /// ([`super::script::Lesson::init_duel`]).
    pub fn tutorial(mut duel: Duel, tutorial: Tutorial) -> Self {
        if tutorial.step == 0 && tutorial.polls == 0 {
            tutorial.lesson.init_duel(&mut duel);
        }
        let input = DuelUi::new(&duel);
        Self {
            duel,
            input,
            seats: [None, None],
            tutorial: Some(tutorial),
        }
    }

    /// Whether the side to act is played by a computer seat or a script.
    pub fn computer_to_act(&self) -> bool {
        self.tutorial.is_some() || self.seats[usize::from(self.duel.active)].is_some()
    }

    /// Advance one frame. `pad` holds the human pad's keys pressed this
    /// frame (edges or auto-repeats); it is ignored while a computer seat
    /// acts. An `Err` leaves the session unchanged; `Unsupported` means the
    /// duel reached something the port refuses.
    pub fn frame(
        &mut self,
        pad: u16,
        cards: &impl CardData,
        fusions: &impl weevil::FusionData,
    ) -> Result<Frame> {
        if self.duel.outcome.is_some() {
            return Ok(Frame::default());
        }
        if self.tutorial.is_some() {
            return self.tutorial_frame(pad, cards);
        }
        let side = self.duel.active;
        // Taken before anything changes, so every error path restores it.
        let before = (self.input.clone(), self.seats.clone());
        let key = match self.seats[usize::from(side)].as_mut() {
            None if pad == 0 => return Ok(Frame::default()),
            None => pad,
            Some(seat) => {
                if seat.wait > 0 {
                    seat.wait -= 1;
                    return Ok(Frame::default());
                }
                // One controller call (0x1117D0): the watchdog counts it first.
                seat.calls += 1;
                let key = if seat.calls > WATCHDOG_CALLS + 2 {
                    pad::START
                } else if seat.calls >= WATCHDOG_CALLS {
                    pad::CIRCLE
                } else {
                    if seat.queue.is_empty() {
                        weevil::sync(&mut seat.memory, &self.duel, side);
                        let plan = weevil::plan_for_rng(
                            seat.controller,
                            &self.duel,
                            side,
                            &mut seat.memory,
                            cards,
                            fusions,
                            seat.tier,
                        )
                        .map(|(plan, rng)| {
                            // rand() draws of the module (0x19AE88).
                            self.duel.rng = rng;
                            plan
                        });
                        match plan {
                            Err(e) => {
                                (self.input, self.seats) = before;
                                return Err(e);
                            }
                            Ok(weevil::Plan::EndTurn) => seat.queue.push_back(pad::START),
                            // No key this call; the next call plans again.
                            Ok(weevil::Plan::NoKey) => return Ok(Frame::default()),
                            // The module set the unit's acted flag itself
                            // (0x1E30F0): no key, and the unit is skipped
                            // by the next plan.
                            Ok(weevil::Plan::MarkActed(id)) => {
                                if let Some(u) = self.duel.units.iter_mut().find(|u| u.id == id) {
                                    u.acted = true;
                                }
                                return Ok(Frame::default());
                            }
                            Ok(weevil::Plan::Act(action)) => seat.queue.extend(ai::keys(
                                &self.duel,
                                side,
                                self.input.cursors[usize::from(side)],
                                &action,
                                cards,
                            )),
                        }
                    }
                    seat.queue.pop_front().expect("a planned key")
                };
                // The next call comes COMPUTER_KEY_FRAMES frames after this one.
                seat.wait = COMPUTER_KEY_FRAMES - 1;
                key
            }
        };
        let response = match self.input.press(&self.duel, cards, key) {
            Ok(r) => r,
            Err(e) => {
                (self.input, self.seats) = before;
                return Err(e);
            }
        };
        let mut events = Vec::new();
        if let Response::Commands(commands) = &response {
            let mut next = self.duel.clone();
            for cmd in commands {
                match next.apply(side, cmd, cards) {
                    Ok((d, e)) => {
                        next = d;
                        events.extend(e);
                    }
                    Err(e) => {
                        (self.input, self.seats) = before;
                        return Err(e);
                    }
                }
            }
            self.duel = next;
            if self.duel.active != side {
                // A new turn (0x1DAAA0 -> 0x277770): the watchdog counts
                // per turn and the queue starts empty.
                for seat in self.seats.iter_mut().flatten() {
                    seat.calls = 0;
                    seat.queue.clear();
                }
            }
        }
        Ok(Frame {
            key: Some(key),
            response: Some(response),
            events,
            script: None,
        })
    }

    /// One frame of a practice lesson. `pad` holds the human's fresh
    /// presses (edges only: the script reads pad 0 for Start and the face
    /// buttons and nothing else). An `Err` leaves the session unchanged.
    fn tutorial_frame(&mut self, pad: u16, cards: &impl CardData) -> Result<Frame> {
        let Some(tutorial) = self.tutorial.as_mut() else {
            return Ok(Frame::default());
        };
        if tutorial.ended.is_some() {
            return Ok(Frame::default());
        }
        let side = self.duel.active;
        let before = (self.duel.clone(), self.input.clone(), tutorial.clone());
        let restore = |this: &mut Self, before: (Duel, DuelUi, Tutorial)| {
            this.duel = before.0;
            this.input = before.1;
            this.tutorial = Some(before.2);
        };
        let mut script = match tutorial.poll(&mut self.duel, &mut self.input, cards, pad) {
            Ok(f) => f,
            Err(e) => {
                restore(self, before);
                return Err(e);
            }
        };
        let mut events = std::mem::take(&mut script.events);
        if script.key == 0 {
            return Ok(Frame {
                key: None,
                response: None,
                events,
                script: Some(script),
            });
        }
        let key = script.key;
        let response = match self.input.press(&self.duel, cards, key) {
            Ok(Response::Unsupported(what)) => {
                restore(self, before);
                return Err(DuelError::Unsupported(format!(
                    "lesson key {key:#x} at step {}: {what}",
                    script.step.0
                )));
            }
            Ok(r) => r,
            Err(e) => {
                restore(self, before);
                return Err(e);
            }
        };
        if let Response::Commands(commands) = &response {
            // The Destiny Draw (state 0x3E, 0x1D648C): decided on the accepted
            // summon cell, drawn as the last card of the refill (0x1D8F90).
            // (Only when the duel has no rows: otherwise the engine's refill
            // draws it, `duel/destiny.rs`.)
            let destiny = self.duel.destiny_rows.is_none()
                && commands
                    .iter()
                    .any(|c| matches!(c, DuelCommand::OpenSummon { .. }))
                && self
                    .tutorial
                    .as_ref()
                    .expect("checked above")
                    .destiny_pending(&self.duel);
            let counts = {
                let s = &self.duel.sides[usize::from(side)];
                (s.hand.len(), self.duel.deck_count(side))
            };
            let mut next = self.duel.clone();
            for cmd in commands {
                match next.apply(side, cmd, cards) {
                    Ok((d, e)) => {
                        next = d;
                        events.extend(e);
                    }
                    Err(e) => {
                        restore(self, before);
                        return Err(e);
                    }
                }
            }
            if destiny {
                let draw = self.tutorial.as_mut().expect("checked above").destiny_draw(
                    &mut next,
                    cards,
                    counts.0,
                    counts.1,
                    &mut events,
                );
                match draw {
                    Ok(d) => script.destiny = d,
                    Err(e) => {
                        restore(self, before);
                        return Err(e);
                    }
                }
            }
            let tutorial = self.tutorial.as_ref().expect("checked above");
            tutorial.suppress_lp0_end(&mut next, &mut events);
            if next.active != side {
                // No per-turn script reset (0x277770 is skipped for
                // scripted sides); Exodia at a scripted side's turn start
                // is an event only.
                script.exodia_event = next.script_exodia_event(cards);
            }
            self.duel = next;
        }
        Ok(Frame {
            key: Some(key),
            response: Some(response),
            events,
            script: Some(script),
        })
    }

    /// Run frames until the duel ends, `frames` pass, or the port refuses
    /// something (the error is returned).
    pub fn run(
        &mut self,
        frames: usize,
        cards: &impl CardData,
        fusions: &impl weevil::FusionData,
    ) -> std::result::Result<Vec<DuelEvent>, DuelError> {
        let mut events = Vec::new();
        for _ in 0..frames {
            if self.duel.outcome.is_some()
                || self.tutorial.as_ref().is_some_and(|t| t.ended.is_some())
            {
                break;
            }
            events.extend(self.frame(0, cards, fusions)?.events);
        }
        Ok(events)
    }
}
