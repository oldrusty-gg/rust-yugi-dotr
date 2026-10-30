//! Lesson 1 checks: the (poll, step, key) list of the interpreter run
//! (`emu-32.txt`), the board/LP after every setup and battle, the details
//! view and surrender prompt, and (private) the saved original states.

use crate::duel::ai::weevil::FusionData;
use crate::duel::script::{pad, Lesson, LessonEnd, Tutorial};
use crate::duel::session::DuelSession;
use crate::duel::ui::{Mode, Response, SurrenderPrompt};
use crate::duel::{CardData, CardInfo, Duel, DuelError, DuelSetup, EndReason};
use crate::practice::DeckInput;
use crate::GridPos;

fn monster(id: u16, kind: u8, level: u8, attribute: u8, attack: u16, defense: u16) -> CardInfo {
    CardInfo {
        id,
        kind,
        level,
        attribute,
        attack,
        defense,
        effects: [false; 5],
        toon: false,
        kind_index: 0,
        entries: [None; 5],
        equip_bits: [0; 7],
        power_up: None,
    }
}

/// Synthetic catalogue (not retail data): the card ids lesson 1 names with
/// the ATK/DEF values its messages state (3613, 3616, 3619, 3628, 3632,
/// 3641: 1400/1200 Celtic Guardian, 1200 Baby Dragon, 1500 Koumori, 1400
/// Winged Dragon #1, DEF 2000 Aqua Madoor, DEF 1400 Rock Ogre Grotto #2);
/// the rest invented. Only the Celtic Guardian (level 4) is summonable
/// with 4 stars; the Baby Dragon is level 3. The Dragon Capture Jar is a
/// magic card (class 1) with its entry-0 effect of type 0x17 (mode 3,
/// filter 0 Dragon, 0xFF: a permanent spellbind, see
/// `activation_of_the_jar_spellbinds_the_kaiser_dragon`).
fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..700).map(|i| monster(i, 1, 6, 0, 100, 100)).collect();
    v[0] = monster(0, 0, 8, 0, 3000, 2500); // Blue-Eyes, side 1 leader
    v[60] = monster(60, 1, 7, 1, 2500, 2100); // Dark Magician, side 0 leader
    v[156] = monster(156, 3, 4, 3, 1400, 1200);
    v[21] = monster(21, 0, 3, 5, 1200, 700);
    v[5] = monster(5, 0, 4, 1, 1500, 1200);
    v[22] = monster(22, 0, 4, 5, 1400, 1200);
    v[169] = monster(169, 3, 3, 3, 300, 1200);
    v[100] = monster(100, 1, 4, 4, 1200, 2000);
    v[631] = monster(631, 18, 3, 3, 700, 1400);
    v[626] = monster(626, 18, 3, 3, 1300, 2000);
    v[3] = monster(3, 0, 7, 0, 2300, 2000);
    v[160] = monster(160, 3, 5, 3, 1500, 1700);
    v[130] = monster(130, 2, 6, 1, 2200, 1800);
    v[157] = monster(157, 3, 4, 3, 1500, 1200);
    let mut jar = monster(683, 32, 0, 0, 0, 0);
    jar.effects[0] = true;
    jar.entries[0] = Some([0x17 << 8 | 0xC0, 0xFF]);
    v[683] = jar;
    v
}

/// Deck records 0x11/0x12 with the first five cards of each as the lesson
/// shows them in the hands [O 000, 021]; the rest filler.
fn records() -> [DeckInput; 2] {
    let mut a: Vec<u16> = vec![321, 285, 156, 249, 505];
    a.extend((0..35).map(|i| 5 + i % 3));
    let mut b: Vec<u16> = vec![21, 423, 522, 12, 433];
    b.extend((0..35).map(|i| 5 + i % 3));
    [
        DeckInput {
            index: 0x11,
            leader_card: 60,
            leader_rank_raw: 1,
            cards: a,
        },
        DeckInput {
            index: 0x12,
            leader_card: 0,
            leader_rank_raw: 1,
            cards: b,
        },
    ]
}

fn setup() -> DuelSetup {
    let r = records();
    crate::duel::script::lesson_setup(Lesson::Basics, [&r[0], &r[1]], [vec![], vec![]]).unwrap()
}

fn session(cards: &impl CardData) -> DuelSession {
    let (duel, _) = Duel::new(setup(), cards).unwrap();
    DuelSession::tutorial(duel, Tutorial::series(Lesson::Basics))
}

/// The operator of the interpreter run: Cross on every fifth poll that
/// reads the pad while a page is fully shown.
struct Operator {
    checks: u32,
}

impl Operator {
    fn pad(&mut self, session: &DuelSession) -> u16 {
        let t = session.tutorial.as_ref().unwrap();
        if t.message_accepts_input() {
            self.checks += 1;
            if self.checks.is_multiple_of(5) {
                return pad::CROSS;
            }
        }
        0
    }
}

/// (poll, step before, step after, key) of every poll that returned a key
/// or changed the step, as `emu_run.py` logs them.
type Transition = (u32, u16, u16, u16);

/// (side, slot, card, column, row) of every unit, sorted.
type Units = Vec<(u8, u8, u16, u8, u8)>;

struct NoFusions;

impl FusionData for NoFusions {
    fn fuse(&self, _a: u16, _b: u16) -> Option<u16> {
        None
    }
    fn equip(&self, _monster: u16, _card: u16) -> i32 {
        -1
    }
    fn category(&self, _card: u16) -> u8 {
        0
    }
    fn value(&self, _card: u16) -> u16 {
        0
    }
}

struct Run {
    session: DuelSession,
    transitions: Vec<Transition>,
    /// Duel after each step change, by the step left.
    after: Vec<(u16, Duel)>,
    /// Input state (0x2DB8F7) after each step change, by the step left.
    states: Vec<(u16, u8)>,
    /// The first refusal, with the step it happened in.
    refused: Option<(u16, DuelError)>,
    /// The session's poll count when the run began.
    start: u32,
}

/// Runs from the session's current poll count until the lesson ends or the
/// port refuses a step.
fn run_lesson(mut session: DuelSession, cards: &impl CardData) -> Run {
    let fusions = NoFusions;
    let mut op = Operator { checks: 0 };
    let mut transitions = Vec::new();
    let mut after = Vec::new();
    let mut states = Vec::new();
    let mut refused = None;
    let start = session.tutorial.as_ref().unwrap().polls;
    for _ in 0..6000 {
        let poll = session.tutorial.as_ref().unwrap().polls;
        let pad = op.pad(&session);
        let frame = match session.frame(pad, cards, &fusions) {
            Ok(f) => f,
            Err(e) => {
                let step = session.tutorial.as_ref().unwrap().step;
                refused = Some((step, e));
                break;
            }
        };
        let Some(s) = frame.script else { break };
        if s.key != 0 || s.step.0 != s.step.1 {
            transitions.push((poll, s.step.0, s.step.1, s.key));
        }
        if s.step.0 != s.step.1 {
            after.push((s.step.0, session.duel.clone()));
            states.push((s.step.0, session.input.state()));
        }
        if s.ended.is_some() {
            break;
        }
    }
    Run {
        session,
        transitions,
        after,
        states,
        refused,
        start,
    }
}

impl Run {
    fn after(&self, step: u16) -> &Duel {
        &self
            .after
            .iter()
            .find(|(s, _)| *s == step)
            .unwrap_or_else(|| panic!("step {step} never left"))
            .1
    }

    fn state_after(&self, step: u16) -> u8 {
        self.states.iter().find(|(s, _)| *s == step).unwrap().1
    }
}

fn unit_summary(duel: &Duel) -> Units {
    let mut v: Vec<_> = duel
        .units
        .iter()
        .map(|u| (u.side, u.slot, u.card, u.position.column, u.position.row))
        .collect();
    v.sort_unstable();
    v
}

fn lp(duel: &Duel) -> [u16; 2] {
    [duel.sides[0].lp, duel.sides[1].lp]
}

/// The interpreter run from the practice start (`emu-32.txt`, step 2,
/// TUTORIAL.md §6): poll, step before → after, key. The polls are compared
/// relative to the 2 → 3 change.
const EMU_32: &[Transition] = &[
    (26, 2, 3, 0x0),
    (35, 3, 4, 0x0),
    (105, 4, 5, 0x0),
    (128, 5, 6, 0x0),
    (137, 6, 7, 0x0),
    (163, 7, 8, 0x0),
    (177, 8, 9, 0x80),
    (209, 9, 10, 0x0),
    (237, 10, 11, 0x0),
    (251, 11, 12, 0x1000),
    (265, 12, 13, 0x40),
    (293, 13, 14, 0x0),
    (355, 14, 15, 0x0),
    (388, 15, 16, 0x0),
    (450, 16, 17, 0x0),
    (483, 17, 18, 0x0),
    (497, 18, 19, 0x2000),
    (511, 19, 20, 0x2000),
    (525, 20, 21, 0x2000),
    (539, 21, 22, 0x8000),
    (557, 22, 23, 0x0),
    (571, 23, 24, 0x40),
    (594, 24, 25, 0x0),
    (608, 25, 26, 0x40),
    (636, 26, 27, 0x0),
    (706, 27, 28, 0x0),
    (776, 28, 29, 0x0),
    (794, 29, 30, 0x0),
    (803, 30, 31, 0x0),
    (829, 31, 32, 0x0),
    (843, 32, 33, 0x40),
    (871, 33, 34, 0x0),
    (885, 34, 35, 0x1000),
    (899, 35, 36, 0x40),
    (937, 36, 37, 0x800),
    (955, 37, 38, 0x0),
    (963, 38, 39, 0x40),
    (971, 39, 40, 0x1000),
    (979, 40, 41, 0x40),
    (987, 41, 42, 0x80),
    (995, 42, 43, 0x1000),
    (1003, 43, 44, 0x40),
    (1011, 44, 45, 0x40),
    (1019, 45, 46, 0x40),
    (1027, 46, 47, 0x40),
    (1035, 47, 48, 0x1000),
    (1043, 48, 49, 0x40),
    (1051, 49, 50, 0x800),
    (1084, 50, 51, 0x0),
    (1098, 51, 52, 0x40),
    (1112, 52, 53, 0x1000),
    (1130, 53, 54, 0x0),
    (1144, 54, 55, 0x40),
    (1172, 55, 56, 0x0),
    (1189, 56, 57, 0x0),
    (1221, 57, 58, 0x0),
    (1238, 58, 59, 0x0),
    (1276, 59, 60, 0x0),
    (1290, 60, 61, 0x1000),
    (1304, 61, 62, 0x4000),
    (1322, 62, 63, 0x40),
    (1336, 63, 64, 0x1000),
    (1350, 64, 65, 0x40),
    (1373, 65, 66, 0x0),
    (1390, 66, 67, 0x0),
    (1422, 67, 68, 0x0),
    (1439, 68, 69, 0x0),
    (1462, 69, 70, 0x40),
    (1476, 70, 71, 0x1000),
    (1490, 71, 72, 0x40),
    (1508, 72, 73, 0x0),
    (1525, 73, 74, 0x0),
    (1557, 74, 75, 0x0),
    (1574, 75, 76, 0x0),
    (1597, 76, 77, 0x40),
    (1611, 77, 78, 0x1000),
    (1625, 78, 79, 0x40),
    (1648, 79, 80, 0x0),
    (1665, 80, 81, 0x0),
    (1697, 81, 82, 0x0),
    (1714, 82, 83, 0x0),
    (1747, 83, 84, 0x40),
    (1761, 84, 85, 0x1000),
    (1775, 85, 86, 0x40),
    (1793, 86, 87, 0x0),
    (1810, 87, 88, 0x0),
    (1842, 88, 89, 0x0),
    (1859, 89, 90, 0x0),
    (1882, 90, 91, 0x40),
    (1896, 91, 92, 0x1000),
    (1910, 92, 93, 0x40),
    (1933, 93, 94, 0x0),
    (1950, 94, 95, 0x0),
    (1982, 95, 96, 0x0),
    (1999, 96, 97, 0x0),
    (2017, 97, 98, 0x40),
    (2031, 98, 99, 0x1000),
    (2045, 99, 100, 0x40),
    (2073, 100, 101, 0x0),
    (2090, 101, 102, 0x0),
    (2122, 102, 103, 0x0),
    (2139, 103, 104, 0x0),
    (2177, 104, 105, 0x40),
    (2191, 105, 106, 0x1000),
    (2205, 106, 107, 0x40),
    (2228, 107, 108, 0x0),
    (2245, 108, 109, 0x0),
    (2277, 109, 110, 0x0),
    (2294, 110, 111, 0x0),
    (2322, 111, 112, 0x40),
    (2340, 112, 113, 0x0),
    (2354, 113, 114, 0x8),
    (2368, 114, 115, 0x4),
    (2382, 115, 116, 0x8),
    (2396, 116, 117, 0x4),
    (2414, 117, 118, 0x0),
    (2428, 118, 119, 0x8),
    (2442, 119, 120, 0x1000),
    (2456, 120, 121, 0x4000),
    (2470, 121, 122, 0x8),
    (2484, 122, 123, 0x2000),
    (2498, 123, 124, 0x8000),
    (2531, 124, 125, 0x0),
    (2545, 125, 126, 0x2),
    (2559, 126, 127, 0x1),
    (2573, 127, 128, 0x2),
    (2587, 128, 129, 0x1),
    (2605, 129, 130, 0x0),
    (2619, 130, 131, 0x8),
    (2633, 131, 132, 0x2),
    (2647, 132, 133, 0x40),
    (2670, 133, 134, 0x0),
    (2687, 134, 135, 0x0),
    (2719, 135, 136, 0x0),
    (2736, 136, 137, 0x0),
    (2764, 137, 138, 0x0),
    (2769, 138, 138, 0x4000),
    (2774, 138, 138, 0x4000),
    (2779, 138, 138, 0x4000),
    (2784, 138, 138, 0x4000),
    (2785, 138, 139, 0x10),
    (2847, 139, 140, 0x4000),
    (2885, 140, 141, 0x0),
    (2947, 141, 142, 0x20),
    (2961, 142, 143, 0x40),
    (2975, 143, 144, 0x2),
    (2989, 144, 145, 0x40),
    (2994, 145, 145, 0x1000),
    (2999, 145, 145, 0x1000),
    (3004, 145, 145, 0x1000),
    (3009, 145, 145, 0x1000),
    (3010, 145, 146, 0x0),
    (3028, 146, 147, 0x0),
    (3045, 147, 148, 0x0),
    (3046, 148, 149, 0x0),
    (3078, 149, 150, 0x0),
    (3095, 150, 151, 0x0),
    (3123, 151, 152, 0x0),
    (3137, 152, 153, 0x100),
    (3151, 153, 154, 0x8000),
    (3165, 154, 155, 0x20),
    (3166, 155, 156, 0x0),
    (3189, 156, 157, 0x0),
    (3223, 157, 158, 0x0),
];

/// The step the Dragon Capture Jar's activation happens in (a face-up hold
/// of a magic card; the engine refuses its effect type).
const JAR_STEP: u16 = 144;

/// The run's transitions from step 2 on in the interpreter log's frame:
/// re-based so the 2 → 3 change is at poll 26. With a `second` run resumed
/// after a refused step-144 frame, that frame's interpreter entry is
/// inserted and the resumed run's polls (which continue from the restored
/// counter, so their first poll stands for the poll after the refused one)
/// follow with the same shift plus one.
fn combined(first: &Run, second: Option<&Run>) -> Vec<Transition> {
    let base = i64::from(
        first
            .transitions
            .iter()
            .find(|t| t.1 == 2 && t.2 == 3)
            .expect("step 2 closes")
            .0,
    );
    let shift = |p: u32| u32::try_from(i64::from(p) + 26 - base).expect("poll after the base");
    let mut all: Vec<Transition> = first
        .transitions
        .iter()
        .filter(|t| t.1 >= 2)
        .map(|&(p, a, b, k)| (shift(p), a, b, k))
        .collect();
    if let Some(second) = second {
        let jar = *EMU_32.iter().find(|t| t.1 == JAR_STEP).unwrap();
        assert_eq!(shift(second.start), jar.0, "the refused poll");
        all.push(jar);
        all.extend(
            second
                .transitions
                .iter()
                .map(|&(p, a, b, k)| (shift(p) + 1, a, b, k)),
        );
    }
    all
}

/// A session standing at step 145 with the board the original has after
/// the Jar's activation [O 123]: the Jar has left the field, the input is
/// back in 0x18 with the cursor on its cell (2,6). Built from the refused
/// run's session (the frame that refused left it unchanged, so the Jar is
/// still selected in 0x1D there) so everything before is real.
fn resume_after_jar(mut session: DuelSession) -> DuelSession {
    let t = session.tutorial.as_mut().unwrap();
    assert_eq!(t.step, JAR_STEP);
    t.step = JAR_STEP + 1;
    t.counter = 0;
    let jar = session
        .duel
        .units
        .iter()
        .find(|u| u.card == 683)
        .expect("the Jar is on the field")
        .slot;
    session.duel.script_remove(0, jar);
    assert_eq!(session.input.state(), 0x1D);
    session.input.mode = Mode::Field;
    session.input.cursors[0] = GridPos { column: 2, row: 6 };
    session
}

/// The whole lesson in one run: the engine models the Jar's type-0x17
/// activation (step 144), so nothing is refused.
fn full_run(cards: &Vec<CardInfo>) -> Run {
    let run = run_lesson(session(cards), cards);
    assert!(run.refused.is_none(), "{:?}", run.refused);
    run
}

#[test]
fn lesson_one_keys_and_poll_counts_match_the_interpreter_run() {
    let cards = cards();
    let run = full_run(&cards);
    assert_eq!(run.transitions[0], (0, 0, 2, 0));
    assert_eq!(combined(&run, None), EMU_32.to_vec());
    let t = run.session.tutorial.as_ref().unwrap();
    assert_eq!(
        t.ended,
        Some(LessonEnd::NextLesson {
            lesson: Lesson::Victory,
            series_counter: 2
        })
    );
    // −1 on the poll the last message closes, screen black.
    assert_eq!(t.step, 158);
    assert_eq!(t.fade, [0, 0]);
    assert!(run.session.duel.outcome.is_none());
}

#[test]
fn lesson_one_summon_move_and_enemy_turn_reach_the_saved_positions() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards);
    // Step 12: the hand is refilled from the unshuffled deck [O 000].
    let d = run.after(12);
    assert_eq!(d.sides[0].hand, vec![321, 285, 156, 249, 505]);
    assert_eq!(run.state_after(12), 0x43);
    // Step 25: the Celtic Guardian at (3,5) [O 000], cost 4.
    let d = run.after(25);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 5), (1, 0, 0, 3, 0)]
    );
    assert_eq!(d.sides[0].stars, 0);
    assert_eq!(run.state_after(25), 0x18);
    // Step 35: moved to (3,4) [O 011]; step 36 ends the turn [O 017].
    let d = run.after(35);
    assert_eq!(d.unit_at(GridPos { column: 3, row: 4 }).unwrap().card, 156);
    let d = run.after(36);
    assert_eq!((d.active, d.round), (1, 1));
    // The enemy's turn: leader to (3,1), Baby Dragon at (3,2) [O 020–021],
    // then at (3,3); step 49 ends it [O 022].
    assert_eq!(run.after(40).leader(1).position, GridPos { column: 3, row: 1 });
    assert_eq!(run.state_after(41), 0x3E);
    assert_eq!(run.after(44).sides[1].hand, vec![21, 423, 522, 12, 433]);
    assert_eq!(
        run.after(46).unit_at(GridPos { column: 3, row: 2 }).unwrap().card,
        21
    );
    let d = run.after(49);
    assert_eq!((d.active, d.round), (0, 2));
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 3, 4),
            (1, 0, 0, 3, 1),
            (1, 1, 21, 3, 3)
        ]
    );
    // Step 54: the Baby Dragon destroyed, LP1 3800, the attacker advanced
    // [O 029].
    let d = run.after(54);
    assert_eq!(lp(d), [4000, 3800]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 3), (1, 0, 0, 3, 1)]
    );
}

#[test]
fn lesson_one_battle_demos_match_the_original_outcomes() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards);
    let demo = |enemy: u16| {
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 3, 4),
            (1, 0, 0, 3, 1),
            (1, 1, enemy, 3, 3),
        ]
    };
    let advanced = vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 3), (1, 0, 0, 3, 1)];
    // Demo 1 [O 034 → 044]: the fresh units are face-up, not acted, in
    // attack position; the Baby Dragon is destroyed for 200.
    let d = run.after(56);
    assert_eq!((lp(d), unit_summary(d)), ([4000, 4000], demo(21)));
    for cell in [(3, 4), (3, 3)] {
        let u = d
            .unit_at(GridPos {
                column: cell.0,
                row: cell.1,
            })
            .unwrap();
        assert!(u.face_up && u.committed_face_up && !u.defence && !u.acted);
    }
    assert_eq!(run.state_after(62), 0x1D);
    let d = run.after(64);
    assert_eq!((lp(d), unit_summary(d)), ([4000, 3800], advanced.clone()));
    // Demo 2 [O 046]: no LP reset; the Celtic Guardian loses by 100.
    let d = run.after(66);
    assert_eq!((lp(d), unit_summary(d)), ([4000, 3800], demo(5)));
    let d = run.after(71);
    assert_eq!(lp(d), [3900, 3800]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (1, 0, 0, 3, 1), (1, 1, 5, 3, 3)]
    );
    // Demo 3 [O 051 → 057]: equal ATK, both destroyed, no damage.
    let d = run.after(73);
    assert_eq!((lp(d), unit_summary(d)), ([4000, 4000], demo(22)));
    let d = run.after(78);
    assert_eq!(lp(d), [4000, 4000]);
    assert_eq!(unit_summary(d), vec![(0, 0, 60, 3, 6), (1, 0, 0, 3, 1)]);
    // Demo 4 [O 059 → 067]: Hard Armor in committed defence, destroyed
    // without damage.
    let d = run.after(80);
    assert_eq!((lp(d), unit_summary(d)), ([4000, 4000], demo(169)));
    let armor = d.unit_at(GridPos { column: 3, row: 3 }).unwrap();
    assert!(armor.defence && armor.committed_defence && armor.face_up);
    let d = run.after(85);
    assert_eq!((lp(d), unit_summary(d)), ([4000, 4000], advanced.clone()));
    // Demo 5 [O 069 → 075]: Aqua Madoor's DEF 2000 costs the attacker 600
    // and no advance.
    let d = run.after(92);
    assert_eq!((lp(d), unit_summary(d)), ([3400, 4000], demo(100)));
    // Demo 6 [O 077 → 082]: DEF = ATK, nothing changes.
    let d = run.after(99);
    assert_eq!((lp(d), unit_summary(d)), ([4000, 4000], demo(631)));
    // Back to the duel [O 085 → 094]: the direct attack on the leader at
    // (3,1) takes 1400.
    let d = run.after(101);
    assert_eq!(lp(d), [4000, 3800]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 2), (1, 0, 0, 3, 1)]
    );
    let d = run.after(106);
    assert_eq!(lp(d), [4000, 2400]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 2), (1, 0, 0, 3, 1)]
    );
    assert!(d.outcome.is_none());
}

#[test]
fn lesson_one_position_toggles_hold_the_giant_soldier_face_up_in_defence() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards);
    // Step 108 [O 096]: face-down in slot 2.
    let d = run.after(108);
    let soldier = d.unit_at(GridPos { column: 3, row: 5 }).unwrap();
    assert_eq!((soldier.card, soldier.slot, soldier.face_up), (626, 2, false));
    // Steps 111–131 stay in the move state (0x1D) [O 100–108]: the
    // Up/Down and Right/Left walks return to the origin.
    for step in [111, 116, 120, 123, 128, 131] {
        assert_eq!(run.state_after(step), 0x1D, "step {step}");
    }
    // Step 132 [O 109]: held face-up in defence, no move.
    let d = run.after(132);
    let soldier = d.unit_at(GridPos { column: 3, row: 5 }).unwrap();
    assert!(soldier.face_up && soldier.defence && soldier.committed_defence);
    assert_eq!(run.state_after(132), 0x18);
    assert_eq!(unit_summary(d).len(), 4);
}

#[test]
fn activation_of_the_jar_spellbinds_the_kaiser_dragon() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards);
    // Step 134 [O 111]: Kaiser Dragon face-up at (2,2), the Jar in slot 6.
    let d = run.after(134);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 3, 2),
            (0, 2, 626, 3, 5),
            (0, 6, 683, 2, 6),
            (1, 0, 0, 3, 1),
            (1, 1, 3, 2, 2)
        ]
    );
    // Step 138: the cursor walks (2,2) → (2,6) with four Downs, then the
    // details view of the Jar opens [O 116]; step 141 closes it.
    assert_eq!(run.state_after(138), 0x21);
    assert_eq!(run.state_after(141), 0x18);
    assert_eq!(run.state_after(143), 0x1D);
    // Step 144: the face-up hold of the Jar activates its type-0x17 magic
    // effect ("Spellbound!", the Jar leaves the field [O 123]): the Kaiser
    // Dragon is bound for good and cannot act, the input is back in 0x18.
    assert!(run.refused.is_none(), "{:?}", run.refused);
    let d = run.after(JAR_STEP);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 3, 2),
            (0, 2, 626, 3, 5),
            (1, 0, 0, 3, 1),
            (1, 1, 3, 2, 2)
        ]
    );
    let kaiser = d.unit_at(GridPos { column: 2, row: 2 }).unwrap();
    assert_eq!((kaiser.spellbind, kaiser.acted), (0xFF, true));
    assert_eq!(d.sides[0].graveyard.last(), Some(&683));
    assert_eq!(run.state_after(JAR_STEP), 0x18);
}

#[test]
fn lesson_one_ending_previews_the_surrender_and_starts_lesson_two() {
    let cards = cards();
    let run = full_run(&cards);
    // Step 145: four Ups take the cursor from row 6 to row 2.
    assert_eq!(
        run.session.input.cursors[0],
        GridPos { column: 3, row: 6 },
        "the cursor of step 148"
    );
    // Step 147 [O 124]: the cornered board.
    let d = run.after(147);
    assert_eq!(lp(d), [800, 2400]);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 157, 4, 6),
            (1, 0, 0, 3, 1),
            (1, 1, 0, 2, 6),
            (1, 2, 160, 3, 5),
            (1, 3, 130, 4, 5)
        ]
    );
    assert!(d.units.iter().filter(|u| u.slot != 0).all(|u| u.face_up));
    // Steps 152–154: Select opens the prompt (0x25 [O 128]), Left picks
    // "yes", Circle cancels: the duel goes on [O 129–131].
    assert_eq!(run.state_after(152), 0x25);
    assert_eq!(run.state_after(153), 0x25);
    assert_eq!(run.state_after(154), 0x18);
    let d = run.after(154);
    assert!(d.outcome.is_none());
    assert_eq!(lp(d), [800, 2400]);
    assert_eq!(unit_summary(d), unit_summary(run.after(147)));
    assert!(run.session.input.surrender.is_none());
}

#[test]
fn surrender_prompt_follows_the_traced_buttons() {
    let cards = cards();
    let (duel, _) = Duel::new(setup(), &cards).unwrap();
    let mut ui = crate::duel::ui::DuelUi::new(&duel);
    assert_eq!(ui.press(&duel, &cards, pad::SELECT).unwrap(), Response::Moved);
    assert_eq!(ui.surrender, Some(SurrenderPrompt { yes: false }));
    assert_eq!(ui.state(), 0x25);
    // "No" again: nothing; Left: "yes"; Right: "no"; a d-pad press does
    // not move the cursor.
    assert_eq!(ui.press(&duel, &cards, pad::RIGHT).unwrap(), Response::Ignored);
    assert_eq!(ui.press(&duel, &cards, pad::LEFT).unwrap(), Response::Moved);
    assert_eq!(ui.surrender, Some(SurrenderPrompt { yes: true }));
    assert_eq!(ui.press(&duel, &cards, pad::UP).unwrap(), Response::Ignored);
    assert_eq!(ui.cursors[0], GridPos { column: 3, row: 6 });
    // Circle cancels whatever is highlighted.
    assert_eq!(ui.press(&duel, &cards, pad::CIRCLE).unwrap(), Response::Moved);
    assert!(ui.surrender.is_none() && ui.state() == 0x18);
    // Confirming "no" returns to the field; confirming "yes" surrenders.
    ui.press(&duel, &cards, pad::SELECT).unwrap();
    assert_eq!(ui.press(&duel, &cards, pad::CROSS).unwrap(), Response::Moved);
    assert!(ui.surrender.is_none());
    ui.press(&duel, &cards, pad::SELECT).unwrap();
    ui.press(&duel, &cards, pad::LEFT).unwrap();
    let r = ui.press(&duel, &cards, pad::SQUARE).unwrap();
    assert_eq!(
        r,
        Response::Commands(vec![crate::duel::DuelCommand::Surrender])
    );
    let (ended, _) = duel
        .apply(0, &crate::duel::DuelCommand::Surrender, &cards)
        .unwrap();
    let o = ended.outcome.unwrap();
    assert_eq!((o.winner, o.reason), (Some(1), EndReason::Surrender));
    // Side 1 sees the d-pad mirrored: its "yes" is Right on the pad.
    let mut d1 = duel.clone();
    d1.active = 1;
    let mut ui = crate::duel::ui::DuelUi::new(&d1);
    ui.press(&d1, &cards, pad::SELECT).unwrap();
    ui.press(&d1, &cards, pad::RIGHT).unwrap();
    assert_eq!(ui.surrender, Some(SurrenderPrompt { yes: true }));
}

/// Private checks against the original: `ROSES_TEST_CATALOGUE` = the
/// imported `catalogue.json`, `ROSES_TEST_ELF_PATH` = the PAL executable
/// (deck records 0x11/0x12 from its static deck table), `ROSES_TEST_SESSION`
/// = `claude-pine/session` (the `.json` summaries of states 000–131),
/// `ROSES_TEST_TUTORIAL` = the `claude-tutorial` folder (`emu-32.txt`).
mod private {
    use super::*;
    use crate::duel::script::lesson_setup;
    use std::path::PathBuf;

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
    }

    fn catalogue() -> crate::duel::Catalogue {
        let text = std::fs::read_to_string(env("ROSES_TEST_CATALOGUE")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// The static deck table (EE 0x2D10E0, file 0x1D7A60; 82-byte records).
    fn record(elf: &[u8], index: u8) -> DeckInput {
        let start = 0x1D7A60 + usize::from(index) * 82;
        let word = |i: usize| u16::from_le_bytes([elf[start + 2 * i], elf[start + 2 * i + 1]]);
        DeckInput {
            index,
            leader_card: word(0) & 0xFFF,
            leader_rank_raw: (word(0) >> 12) as u8,
            cards: (1..=40).map(word).collect(),
        }
    }

    /// The transitions of an `emu_run.py` log: `poll NNNN step AAA->BBB ret
    /// 0xK shown ...` (the fields are right-aligned: `step  99->100` has no
    /// blank after the arrow).
    fn emu_log(name: &str) -> Vec<Transition> {
        let log = std::fs::read_to_string(env("ROSES_TEST_TUTORIAL").join(name)).unwrap();
        log.lines()
            .filter_map(|l| {
                let l = l.strip_prefix("poll ")?;
                let (poll, rest) = l.split_once(" step ")?;
                let (steps, rest) = rest.split_once(" ret ")?;
                let (a, b) = steps.split_once("->")?;
                let key = rest.split_whitespace().next()?;
                Some((
                    poll.trim().parse().ok()?,
                    a.trim().parse().ok()?,
                    b.trim().parse().ok()?,
                    u16::from_str_radix(key.trim_start_matches("0x"), 16).ok()?,
                ))
            })
            .collect()
    }

    /// Units, LP, round D53, active D54 and sub-state D57 of a saved state.
    fn observed(state: &str) -> (Units, [u16; 2], u8, u8, u8) {
        let text = std::fs::read_to_string(env("ROSES_TEST_SESSION").join(state)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut units: Vec<_> = v["units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| {
                let f = |i: usize| u[i].as_u64().unwrap();
                (f(0) as u8, f(1) as u8, f(2) as u16, f(3) as u8, f(4) as u8)
            })
            .filter(|u| u.0 < 2)
            .collect();
        units.sort_unstable();
        let lp = [
            v["lp"][0].as_u64().unwrap() as u16,
            v["lp"][1].as_u64().unwrap() as u16,
        ];
        let b = v["b8f2_f7"].as_array().unwrap();
        let byte = |i: usize| b[i].as_u64().unwrap() as u8;
        (units, lp, byte(1), byte(2), byte(5))
    }

    #[test]
    #[ignore = "requires the private catalogue, executable and saved states"]
    fn private_lesson_one_matches_the_original_states_and_run() {
        let cards = catalogue();
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let r = [record(&elf, 0x11), record(&elf, 0x12)];
        assert_eq!((r[0].leader_card, r[1].leader_card), (60, 0));
        assert_eq!(&r[0].cards[..5], &[321, 285, 156, 249, 505]);
        let setup = lesson_setup(Lesson::Basics, [&r[0], &r[1]], [vec![], vec![]]).unwrap();
        let (duel, _) = Duel::new(setup, &cards).unwrap();
        let session = DuelSession::tutorial(duel, Tutorial::series(Lesson::Basics));
        let first = run_lesson(session, &cards);
        // Keys and poll counts of the interpreter run from the practice
        // start; the run from saved state 000 (`emu-32b.txt`, inside step
        // 26) has the same counts 615 polls earlier.
        let emu = emu_log("emu-32.txt");
        assert_eq!(emu, EMU_32.to_vec());
        let from_26: Vec<Transition> = EMU_32
            .iter()
            .filter(|t| t.1 >= 26)
            .map(|&(p, a, b, k)| (p - 615, a, b, k))
            .collect();
        assert_eq!(emu_log("emu-32b.txt"), from_26);
        // Either the engine models the Jar's activation by now (the whole
        // lesson runs) or step 144 is refused and the rest runs from 145.
        let (runs, transitions): (Vec<Run>, Vec<Transition>) = match &first.refused {
            None => {
                let t = combined(&first, None);
                (vec![first], t)
            }
            Some((step, err)) => {
                assert_eq!(*step, JAR_STEP);
                assert!(matches!(err, DuelError::Unsupported(_)), "{err}");
                let second = run_lesson(resume_after_jar(first.session.clone()), &cards);
                assert!(second.refused.is_none(), "{:?}", second.refused);
                let t = combined(&first, Some(&second));
                (vec![first, second], t)
            }
        };
        assert_eq!(transitions, emu);
        let after = |step: u16| -> &Duel {
            &runs
                .iter()
                .find_map(|r| r.after.iter().find(|(s, _)| *s == step))
                .unwrap_or_else(|| panic!("step {step}"))
                .1
        };
        let state_after = |step: u16| -> u8 {
            runs.iter()
                .find_map(|r| r.states.iter().find(|(s, _)| *s == step))
                .unwrap()
                .1
        };
        // Board, LP, round, active side and sub-state of the saved states
        // after the step that produced them (the states inside animations
        // 0x39/0x3C and the turn changes 0x15–0x17 are compared for the
        // board only).
        let pairs = [
            ("000-start.json", 25, true),
            ("007-t7.json", 32, true),
            ("011-t11.json", 35, true),
            ("017-endturn.json", 36, true),
            ("020-e1.json", 41, true),
            ("021-e2.json", 46, false),
            ("022-e3.json", 49, false),
            ("026-p1.json", 51, true),
            ("029-b1.json", 54, true),
            ("034-c2.json", 56, true),
            ("044-c12.json", 64, true),
            ("046-c14.json", 66, true),
            ("051-c19.json", 73, true),
            ("057-c25.json", 78, true),
            ("059-c27.json", 80, true),
            ("064-c32.json", 83, true),
            ("067-d1.json", 85, true),
            ("069-c35.json", 87, true),
            ("072-c38.json", 90, true),
            ("075-d3.json", 92, true),
            ("077-c42.json", 94, true),
            ("079-c44.json", 97, true),
            ("082-c47.json", 99, true),
            ("085-c50.json", 101, true),
            ("091-c56.json", 104, true),
            ("094-c59.json", 106, true),
            ("096-c61.json", 108, true),
            ("100-c65.json", 111, true),
            ("103-r1.json", 118, true),
            ("108-c72.json", 131, true),
            ("109-c73.json", 132, true),
            ("111-c75.json", 134, true),
            ("116-c80.json", 138, true),
            ("122-c86.json", 143, true),
            ("123-l2.json", 145, true),
            ("124-ent.json", 147, true),
            ("128-m4.json", 152, true),
            ("129-yes.json", 154, true),
            ("131-s2.json", 156, true),
        ];
        for (state, step, with_state) in pairs {
            let (units, lp_seen, round, active, d57) = observed(state);
            let d = after(step);
            assert_eq!(unit_summary(d), units, "{state} after step {step}");
            assert_eq!(lp(d), lp_seen, "{state}");
            assert!(d.outcome.is_none(), "{state}");
            if with_state {
                assert_eq!((d.round, d.active), (round, active), "{state}");
                assert_eq!(state_after(step), d57, "{state} sub-state");
            }
        }
        assert_eq!(
            runs.last().unwrap().session.tutorial.as_ref().unwrap().ended,
            Some(LessonEnd::NextLesson {
                lesson: Lesson::Victory,
                series_counter: 2
            })
        );
    }
}
