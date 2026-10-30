//! Traced outcomes for the lesson steps whose key the engine still refuses
//! (the Dragon Capture Jar's activation, magic type 0x17; the continuous
//! effects guard against face-up magic cards; leader ability #13). A
//! frontend that reaches such a refusal in a lesson may apply the outcome
//! the saved original states show for that step, explicitly (it must say so
//! in its log or title), so the scripted lesson can go on. The tables are
//! the ones the lesson tests use; when the engine grows the rule the
//! refusal does not happen and the stand-in is never consulted. Tags as in
//! `TUTORIAL.md`: [O] saved state.

use super::{unit_flag, Lesson};
use crate::duel::ui::{DuelUi, Mode};
use crate::duel::Duel;
use crate::GridPos;

/// The traced outcome of one refused key.
pub struct StandIn {
    pub step: u16,
    /// The key the script pressed.
    pub key: u16,
    /// What the engine lacks and what the original shows.
    pub note: &'static str,
    apply: fn(&mut Duel, &mut DuelUi),
}

impl StandIn {
    /// Apply the traced outcome; the caller advances the script's step and
    /// counter as for a key that was taken.
    pub fn apply(&self, duel: &mut Duel, ui: &mut DuelUi) {
        (self.apply)(duel, ui);
    }
}

fn field(ui: &mut DuelUi) {
    ui.mode = Mode::Field;
    ui.details = None;
}

fn unit_flag_acted(duel: &mut Duel, side: u8, slot: u8) {
    let _ = duel.script_unit_flag(side, slot, unit_flag::ACTED, true);
}

fn remove_card(duel: &mut Duel, side: u8, card: u16) {
    if let Some(slot) = duel
        .units
        .iter()
        .find(|u| u.side == side && u.card == card)
        .map(|u| u.slot)
    {
        duel.script_remove(side, slot);
    }
}

fn teleport(duel: &mut Duel, side: u8, slot: u8, column: u8, row: u8) {
    // The target cell is free in every traced outcome; a failure here would
    // mean the board differs from the original and is left as it is.
    let _ = duel.script_teleport(side, slot, GridPos { column, row });
}

const LESSON_1: [StandIn; 1] = [
    // The Jar's face-up hold (Cross in 0x1D with R2 toggled) activates it:
    // the Jar leaves the field and the Kaiser Dragon is spellbound
    // ("Spellbound!") [O 123]; the cursor stays at (2,6), sub-state 0x18.
    StandIn {
        step: 144,
        key: 0x40,
        note: "Dragon Capture Jar activation (magic type 0x17) is not modelled: the Jar leaves the field and the Kaiser Dragon is spellbound [O 123]",
        apply: |duel, ui| {
            remove_card(duel, 0, 683);
            if let Some(slot) = duel
                .units
                .iter()
                .find(|u| u.side == 1 && u.card == 3)
                .map(|u| u.slot)
            {
                let _ = duel.script_spellbind(1, slot, 0xFF);
            }
            field(ui);
        },
    },
];

const LESSON_3: [StandIn; 8] = [
    // The Celtic Guardian destroys the enemy Jar at (3,3) and advances
    // (0x2493A0; step 59 removes it anyway).
    StandIn {
        step: 57,
        key: 0x40,
        note: "attacks with a face-up magic card on the field are refused by the engine's continuous-effects guard: the Jar is destroyed, the attacker advances [O 224-232]",
        apply: |duel, ui| {
            duel.script_remove(1, 6);
            teleport(duel, 0, 1, 3, 3);
            unit_flag_acted(duel, 0, 1);
            field(ui);
        },
    },
    // Face-down, the same: the Jar dies, the monster stays face-down [O 232].
    StandIn {
        step: 65,
        key: 0x40,
        note: "continuous-effects guard: the enemy Jar is destroyed by the face-down attacker, which advances [O 232]",
        apply: |duel, ui| {
            duel.script_remove(1, 6);
            teleport(duel, 0, 1, 3, 3);
            unit_flag_acted(duel, 0, 1);
            field(ui);
        },
    },
    // A Jar attacking a monster is destroyed [O 237, 241, 244].
    StandIn {
        step: 74,
        key: 0x40,
        note: "continuous-effects guard: the attacking Jar (slot 6) is destroyed [O 237]",
        apply: |duel, ui| {
            duel.script_remove(0, 6);
            field(ui);
        },
    },
    StandIn {
        step: 80,
        key: 0x40,
        note: "continuous-effects guard: the attacking Jar (slot 7) is destroyed [O 241]",
        apply: |duel, ui| {
            duel.script_remove(0, 7);
            field(ui);
        },
    },
    StandIn {
        step: 86,
        key: 0x40,
        note: "continuous-effects guard: the attacking Jar (slot 8) is destroyed [O 244]",
        apply: |duel, ui| {
            duel.script_remove(0, 8);
            field(ui);
        },
    },
    // A face-down Jar destroys the enemy Jar and moves in [O 252].
    StandIn {
        step: 95,
        key: 0x40,
        note: "continuous-effects guard: the face-down Jar destroys the enemy Jar and moves to (2,3) [O 252]",
        apply: |duel, ui| {
            duel.script_remove(1, 6);
            teleport(duel, 0, 6, 2, 3);
            unit_flag_acted(duel, 0, 6);
            field(ui);
        },
    },
    // The face-up Jar destroys the enemy Jar, activates on completing its
    // move and leaves the field; the Kaiser Dragon is spellbound for ever
    // and marked acted [O 257-260].
    StandIn {
        step: 102,
        key: 0x40,
        note: "Jar activation (magic type 0x17): both Jars leave the field, the Kaiser Dragon is spellbound for ever [O 257-260]",
        apply: |duel, ui| {
            duel.script_remove(1, 7);
            duel.script_remove(0, 7);
            let _ = duel.script_spellbind(1, 1, 0xFF);
            unit_flag_acted(duel, 1, 1);
            field(ui);
        },
    },
    // The Celtic Guardian moves to (3,1) and the Baby Dragon's rank-12
    // ability #13 "Open Opponent's Card" turns it face-up [O 409].
    StandIn {
        step: 284,
        key: 0x40,
        note: "leader ability #13 (Open Opponent's Card) is not modelled: the Celtic Guardian moves to (3,1) and is turned face-up [O 408-409]",
        apply: |duel, ui| {
            teleport(duel, 0, 1, 3, 1);
            let _ = duel.script_unit_flag(0, 1, unit_flag::COMMITTED_FACE_UP, true);
            unit_flag_acted(duel, 0, 1);
            field(ui);
        },
    },
];

/// The traced outcome for a refused key at `step` of `lesson`, if the
/// refusal is one of the known engine gaps.
pub fn stand_in(lesson: Lesson, step: u16) -> Option<&'static StandIn> {
    let table: &'static [StandIn] = match lesson {
        Lesson::Basics => &LESSON_1,
        Lesson::Victory => &[],
        Lesson::Rules => &LESSON_3,
        Lesson::Technique | Lesson::Functions => &[],
    };
    table.iter().find(|s| s.step == step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stand_ins_cover_the_known_refusals_only() {
        assert!(stand_in(Lesson::Basics, 144).is_some());
        assert!(stand_in(Lesson::Basics, 145).is_none());
        assert!(stand_in(Lesson::Victory, 0).is_none());
        let steps: Vec<u16> = LESSON_3.iter().map(|s| s.step).collect();
        assert_eq!(steps, vec![57, 65, 74, 80, 86, 95, 102, 284]);
        assert!(LESSON_3.iter().all(|s| s.key == 0x40));
    }
}
