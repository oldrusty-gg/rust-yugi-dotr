use super::*;
use crate::duel::session::DuelSession;
use crate::duel::{CardInfo, DuelSetup, EndReason};
use crate::practice::DeckInput;

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

/// Synthetic catalogue (invented numbers, not retail data): the card ids
/// lesson 2 names, with stats that keep its arithmetic (a 1400-ATK warrior
/// against a 1000-LP leader, then against a 750-ATK monster whose
/// destruction deals 500 to its opponent).
fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..700).map(|i| monster(i, 1, 1, 0, 100, 100)).collect();
    v[0] = monster(0, 0, 8, 0, 3000, 2500); // side 1 leader
    v[58] = monster(58, 1, 3, 1, 1000, 1000); // Exodia head (EXODIA_HEAD)
    for limb in crate::duel::EXODIA_LIMBS {
        v[usize::from(limb)] = monster(limb, 1, 1, 1, 200, 300);
    }
    v[156] = monster(156, 3, 4, 3, 1400, 1200);
    let mut needle = monster(346, 7, 2, 1, 750, 700);
    needle.effects[4] = true;
    // Entry 4 type 0xD, mode 2, word1 hi 1 / lo 500: damage on destruction.
    needle.entries[4] = Some([0xD << 8 | 2 << 6, 1 << 16 | 500]);
    v[346] = needle;
    v[3] = monster(3, 0, 7, 0, 2300, 2000);
    v[285] = monster(285, 6, 6, 5, 2000, 1900);
    v[522] = monster(522, 14, 5, 3, 1400, 1800);
    v[631] = monster(631, 18, 3, 3, 700, 1400);
    v[21] = monster(21, 0, 3, 5, 1200, 700);
    // A magic card (class 1) with an entry-0 word the engine never reads
    // while it stays face-down.
    let mut jar = monster(683, 32, 0, 0, 0, 0);
    jar.entries[0] = Some([0, 0]);
    v[683] = jar;
    v
}

fn records() -> [DeckInput; 2] {
    let mut a: Vec<u16> = vec![56, 57, 54, 55, 156];
    a.extend((0..35).map(|i| 5 + i % 3));
    let mut b: Vec<u16> = vec![346, 3, 285, 522, 631, 21, 683];
    b.extend((0..33).map(|i| 5 + i % 3));
    [
        DeckInput {
            index: 0x13,
            leader_card: 58,
            leader_rank_raw: 1,
            cards: a,
        },
        DeckInput {
            index: 0x14,
            leader_card: 0,
            leader_rank_raw: 1,
            cards: b,
        },
    ]
}

fn setup() -> DuelSetup {
    let r = records();
    lesson_setup(Lesson::Victory, [&r[0], &r[1]], [vec![], vec![]]).unwrap()
}

fn session(cards: &impl CardData) -> DuelSession {
    let (duel, _) = Duel::new(setup(), cards).unwrap();
    DuelSession::tutorial(duel, Tutorial::series(Lesson::Victory))
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

struct Run {
    session: DuelSession,
    transitions: Vec<Transition>,
    /// Duel after each step change, by the step left.
    after: Vec<(u16, Duel)>,
    exodia_events: Vec<u32>,
}

/// The lessons never fuse (no planner runs), so the session's fusion data
/// is unused.
struct NoFusions;

impl crate::duel::ai::weevil::FusionData for NoFusions {
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

fn run_lesson(mut session: DuelSession, cards: &impl CardData, quit_at: Option<u32>) -> Run {
    let fusions = NoFusions;
    let mut op = Operator { checks: 0 };
    let mut transitions = Vec::new();
    let mut after = Vec::new();
    let mut exodia_events = Vec::new();
    for poll in 0..5000 {
        let mut pad = op.pad(&session);
        if quit_at == Some(poll) {
            pad |= pad::START;
        }
        let frame = session.frame(pad, cards, &fusions).unwrap();
        let Some(s) = frame.script else { break };
        if s.exodia_event {
            exodia_events.push(poll);
        }
        if s.key != 0 || s.step.0 != s.step.1 {
            transitions.push((poll, s.step.0, s.step.1, s.key));
        }
        if s.step.0 != s.step.1 {
            after.push((s.step.0, session.duel.clone()));
        }
        if s.ended.is_some() {
            break;
        }
    }
    Run {
        session,
        transitions,
        after,
        exodia_events,
    }
}

/// The interpreter run from saved state 133 (`emu-33.txt`, TUTORIAL.md §6):
/// poll, step before → after, key. It started inside step 2, so the polls
/// are compared relative to the 2 → 3 change.
const EMU_33: &[Transition] = &[
    (21, 2, 3, 0),
    (38, 3, 4, 0),
    (70, 4, 5, 0),
    (87, 5, 6, 0),
    (105, 6, 7, 0),
    (167, 7, 8, 0),
    (185, 8, 9, 0),
    (199, 9, 10, 0x40),
    (213, 10, 11, 0x1000),
    (227, 11, 12, 0x40),
    (250, 12, 13, 0),
    (267, 13, 14, 0),
    (299, 14, 15, 0),
    (316, 15, 16, 0),
    (344, 16, 17, 0),
    (358, 17, 18, 0x1000),
    (372, 18, 19, 0x10),
    (386, 19, 20, 0x4000),
    (414, 20, 21, 0),
    (428, 21, 22, 0x20),
    (442, 22, 23, 0x4000),
    (456, 23, 24, 0x40),
    (470, 24, 25, 0x1000),
    (484, 25, 26, 0x40),
    (512, 26, 27, 0),
    (529, 27, 28, 0),
    (561, 28, 29, 0),
    (578, 29, 30, 0),
    (596, 30, 31, 0),
    (610, 31, 32, 0x80),
    (628, 32, 33, 0),
    (637, 33, 34, 0x20),
    (669, 34, 35, 0),
    (678, 35, 36, 0),
    (706, 36, 37, 0),
    (723, 37, 38, 0),
    (755, 38, 39, 0x80),
    (772, 39, 40, 0),
    (790, 40, 41, 0),
    (799, 41, 42, 0),
    (825, 42, 43, 0),
    (834, 43, 44, 0x20),
    (866, 44, 45, 0x80),
    (883, 45, 46, 0),
    (901, 46, 47, 0),
    (910, 47, 48, 0),
    (936, 48, 49, 0),
    (953, 49, 50, 0x20),
    (985, 50, 51, 0),
    (1019, 51, 52, 0),
    (1081, 52, 53, 0),
    (1114, 53, 54, 0),
    (1131, 54, 55, 0),
    (1163, 55, 56, 0),
    (1217, 56, 57, 0),
    (1226, 57, 58, 0),
    (1296, 58, 59, 0),
    (1314, 59, 60, 0),
    (1323, 60, 61, 0),
    (1359, 61, 62, 0x800),
    (1360, 62, 63, 0x800),
    (1378, 63, 64, 0),
];

fn relative(transitions: &[Transition]) -> Vec<Transition> {
    let base = transitions
        .iter()
        .find(|t| t.1 == 2 && t.2 == 3)
        .expect("step 2 closes")
        .0;
    transitions
        .iter()
        .filter(|t| t.1 >= 2)
        .map(|&(p, a, b, k)| (p + 21 - base, a, b, k))
        .collect()
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

#[test]
fn text_remap_applies_only_with_the_variant_flag() {
    assert_eq!(remap_text(3589, true), 4092);
    assert_eq!(remap_text(3589, false), 3589);
    assert_eq!(remap_text(3670, true), 3670);
    assert_eq!(remap_text(3992, true), 4110);
}

#[test]
fn series_counters_and_ends_follow_the_practice_counter() {
    assert_eq!(Tutorial::series(Lesson::Basics).series_counter, 1);
    assert_eq!(Tutorial::series(Lesson::Victory).series_counter, 2);
    assert_eq!(Tutorial::series(Lesson::Rules).series_counter, 0);
    let mut t = Tutorial::series(Lesson::Basics);
    assert_eq!(
        t.end(-1),
        LessonEnd::NextLesson {
            lesson: Lesson::Victory,
            series_counter: 2
        }
    );
    let mut t = Tutorial::series(Lesson::Victory);
    assert_eq!(
        t.end(-1),
        LessonEnd::NextLesson {
            lesson: Lesson::Rules,
            series_counter: 0
        }
    );
    let mut t = Tutorial::series(Lesson::Rules);
    assert_eq!(t.end(-1), LessonEnd::SeriesEnd { quit: false });
    let mut t = Tutorial::series(Lesson::Basics);
    assert_eq!(t.end(-2), LessonEnd::SeriesEnd { quit: true });
    assert_eq!(Lesson::Victory.deck_pair(), (0x13, 0x14));
    assert_eq!(Lesson::from_controller(34), Some(Lesson::Rules));
    assert!(scripted(33) && !scripted(100) && !scripted(2));
}

#[test]
fn lesson_setup_checks_the_records() {
    let r = records();
    assert!(lesson_setup(Lesson::Basics, [&r[0], &r[1]], [vec![], vec![]]).is_err());
    let s = setup();
    assert_eq!(s.controllers, [33, 33]);
    assert_eq!(s.leaders, [58, 0]);
    assert!(s.terrain.iter().all(|&t| t == 7));
    assert_eq!(s.decks[0].len(), 40);
}

#[test]
fn lesson_two_keys_and_poll_counts_match_the_interpreter_run() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, None);
    assert_eq!(relative(&run.transitions), EMU_33.to_vec());
    // Step 0 jumps to 2 in the first poll, without a key.
    assert_eq!(run.transitions[0], (0, 0, 2, 0));
    let t = run.session.tutorial.as_ref().unwrap();
    assert_eq!(
        t.ended,
        Some(LessonEnd::NextLesson {
            lesson: Lesson::Rules,
            series_counter: 0
        })
    );
    assert!(t.no_lp0_end);
    assert!(run.session.duel.outcome.is_none());
    // Step 63 returns −1 on the poll its message closes (0x17642C) [F, emu:
    // from state 184 the practice counter goes 2 → 0 on that poll], so the
    // lesson ends in the frame of the 63 → 64 change, not one poll later.
    let last = run.transitions.last().unwrap();
    assert_eq!((last.1, last.2, last.3), (63, 64, 0));
    assert_eq!(t.polls, last.0 + 1);
}

#[test]
fn camera_requests_are_reported_once_per_step() {
    let cards = cards();
    let (mut duel, _) = Duel::new(setup(), &cards).unwrap();
    let mut ui = DuelUi::new(&duel);
    let mut tut = Tutorial::series(Lesson::Victory);
    tut.step = 5;
    let mut ctx = Ctx {
        tut: &mut tut,
        duel: &mut duel,
        ui: &mut ui,
        cards: &cards,
        human: 0,
        frame: ScriptFrame::default(),
        ret: None,
    };
    let close_up = CameraOp::CloseUp { card: 156 };
    // The arm repeats the call on every poll of its fade: once per step.
    ctx.camera(close_up);
    ctx.camera(close_up);
    assert_eq!(ctx.frame.camera, vec![close_up]);
    // A later step asking for the same view again is reported again
    // (lesson 3 steps 51 and 61; steps 157 and 158 both save the camera).
    ctx.goto(6);
    ctx.camera(close_up);
    ctx.camera(CameraOp::Save);
    ctx.camera(CameraOp::Save);
    assert_eq!(ctx.frame.camera, vec![close_up, close_up, CameraOp::Save]);
    assert_eq!(
        (tut.camera, tut.camera_step),
        (Some(CameraOp::Save), Some(6))
    );
}

#[test]
fn lesson_two_edits_and_battles_reach_the_traced_positions() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, None);
    let after = |step: u16| -> &Duel { &run.after.iter().find(|(s, _)| *s == step).unwrap().1 };
    // Step 3: LP 4000/1000, Celtic Guardian face-up at (3,1).
    let d = after(3);
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [4000, 1000]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 58, 3, 6), (0, 1, 156, 3, 1), (1, 0, 0, 3, 0)]
    );
    let celtic = d.unit_at(GridPos { column: 3, row: 1 }).unwrap();
    assert!(celtic.face_up && celtic.committed_face_up && !celtic.defence && !celtic.acted);
    // Step 11: the direct attack empties LP1 without ending the duel.
    let d = after(11);
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [4000, 0]);
    assert!(d.outcome.is_none());
    assert!(d.unit_at(GridPos { column: 3, row: 1 }).unwrap().acted);
    // Step 13: 500/500, Celtic (3,4) against the Needle Ball (3,3).
    let d = after(13);
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [500, 500]);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 58, 3, 6),
            (0, 1, 156, 3, 4),
            (1, 0, 0, 3, 0),
            (1, 1, 346, 3, 3)
        ]
    );
    // Steps 18-21: the details view of the Needle Ball, closed by Circle.
    assert_eq!(after(18).active, 0);
    // Step 25: the Needle Ball dies, its effect empties LP0 too; a draw
    // the duel does not end on.
    let d = after(25);
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [0, 0]);
    assert!(d.outcome.is_none());
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 58, 3, 6), (0, 1, 156, 3, 3), (1, 0, 0, 3, 0)]
    );
    // Step 27: leader at (3,4), LP restored.
    let d = after(27);
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [4000, 4000]);
    assert_eq!(unit_summary(d), vec![(0, 0, 58, 3, 4), (1, 0, 0, 3, 0)]);
    // Step 33: the eight neighbours enemy-held (monsters 1..5, jars 6..8).
    let d = after(33);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 58, 3, 4),
            (1, 0, 0, 3, 0),
            (1, 1, 3, 3, 3),
            (1, 2, 285, 2, 3),
            (1, 3, 522, 4, 3),
            (1, 4, 631, 2, 4),
            (1, 5, 21, 4, 4),
            (1, 6, 683, 2, 5),
            (1, 7, 683, 3, 5),
            (1, 8, 683, 4, 5)
        ]
    );
    // Step 43: Labyrinth ring around (3,5).
    let d = after(43);
    assert_eq!(d.terrain_of(GridPos { column: 2, row: 5 }), Some(8));
    assert_eq!(d.terrain_of(GridPos { column: 3, row: 4 }), Some(7));
    assert_eq!(d.leader(0).position, GridPos { column: 3, row: 5 });
    // Step 49: cleared, terrain back, round 99.
    let d = after(49);
    assert_eq!(unit_summary(d), vec![(0, 0, 58, 3, 6), (1, 0, 0, 3, 0)]);
    assert!(d.terrain.iter().all(|&t| t == 7));
    assert_eq!(d.round, 99);
    // Step 54: limbs at the corners, round 0.
    let d = after(54);
    assert_eq!(d.round, 0);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 58, 3, 6),
            (0, 1, 57, 6, 0),
            (0, 2, 56, 5, 0),
            (0, 3, 55, 0, 0),
            (0, 4, 54, 1, 0),
            (1, 0, 0, 3, 0)
        ]
    );
    // Step 61: side 0's turn ends; 62: side 1's; Exodia is complete at
    // side 0's turn start but the scripted side only gets the event.
    assert_eq!(after(61).active, 1);
    let d = after(62);
    assert_eq!(d.active, 0);
    assert_eq!(d.round, 1);
    assert!(d.outcome.is_none());
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 58, 3, 6),
            (0, 1, 57, 2, 6),
            (0, 2, 56, 4, 6),
            (0, 3, 55, 2, 5),
            (0, 4, 54, 4, 5),
            (1, 0, 0, 3, 0)
        ]
    );
    let start_poll = run.transitions.iter().find(|t| t.1 == 62).unwrap().0;
    assert_eq!(run.exodia_events, vec![start_poll]);
}

#[test]
fn start_on_the_human_pad_quits_the_series() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, Some(300));
    let t = run.session.tutorial.as_ref().unwrap();
    assert_eq!(t.ended, Some(LessonEnd::SeriesEnd { quit: true }));
    assert_eq!(t.polls, 301);
    assert!(Tutorial::continue_with(t.ended.unwrap(), true).is_none());
    let next = Tutorial::continue_with(
        LessonEnd::NextLesson {
            lesson: Lesson::Rules,
            series_counter: 0,
        },
        true,
    )
    .unwrap();
    assert_eq!(
        (next.lesson, next.series_counter, next.step),
        (Lesson::Rules, 0, 0)
    );
}

#[test]
fn messages_slide_page_and_close_with_the_traced_counts() {
    let cards = cards();
    let mut s = session(&cards);
    let fusions = NoFusions;
    // Poll 0: step 0 → 2. Poll 1: the window opens (offset 0x1E0 − 0x50).
    s.frame(0, &cards, &fusions).unwrap();
    s.frame(0, &cards, &fusions).unwrap();
    let m = s.tutorial.as_ref().unwrap().message.unwrap();
    assert_eq!(
        (m.first, m.last, m.page, m.offset()),
        (3670, 3672, 3670, 0x190)
    );
    assert!(!s.tutorial.as_ref().unwrap().message_accepts_input());
    for _ in 0..5 {
        s.frame(0, &cards, &fusions).unwrap();
    }
    let t = s.tutorial.as_ref().unwrap();
    assert_eq!(t.message.unwrap().phase, MessagePhase::Shown);
    assert!(t.message_accepts_input());
    // A face button pages; the last page starts the slide-out; six polls
    // later the window is gone and the step advanced.
    let f = s.frame(pad::TRIANGLE, &cards, &fusions).unwrap();
    assert_eq!(f.script.unwrap().sounds, vec![sfx::PAGE]);
    assert_eq!(s.tutorial.as_ref().unwrap().message.unwrap().page, 3671);
    s.frame(pad::SQUARE, &cards, &fusions).unwrap();
    let f = s.frame(pad::CIRCLE, &cards, &fusions).unwrap();
    assert_eq!(f.script.unwrap().sounds, vec![sfx::LAST_PAGE]);
    for k in 1..=6 {
        s.frame(pad::CROSS, &cards, &fusions).unwrap();
        let m = s.tutorial.as_ref().unwrap().message.unwrap();
        assert_eq!(m.phase, MessagePhase::SlideOut { polls: k });
        assert_eq!(m.offset(), 0x50 * u16::from(k));
    }
    let f = s.frame(0, &cards, &fusions).unwrap();
    assert_eq!(f.script.unwrap().step, (2, 3));
    let t = s.tutorial.as_ref().unwrap();
    assert!(t.message.is_none() && !t.hud_flag);
}

#[test]
fn every_lesson_starts_at_step_two() {
    let cards = cards();
    let (duel, _) = Duel::new(setup(), &cards).unwrap();
    let fusions = NoFusions;
    // Lessons 1 and 3 have their own checks in `lesson1/tests.rs` and
    // `lesson3/tests.rs`; every lesson's first poll is the 0 → 2 jump.
    for lesson in Lesson::ALL {
        let mut s = DuelSession::tutorial(duel.clone(), Tutorial::series(lesson));
        let f = s.frame(0, &cards, &fusions).unwrap();
        assert_eq!(f.script.unwrap().step, (0, 2), "{lesson:?}");
        assert_eq!(
            s.tutorial.as_ref().unwrap().no_lp0_end,
            // Step 0 of 33, 34 and 35 sets NO_LP0_END; 32 and 36 do not.
            !matches!(lesson, Lesson::Basics | Lesson::Functions)
        );
    }
}

#[test]
fn script_edits_follow_the_traced_primitives() {
    let cards = cards();
    let (mut d, _) = Duel::new(setup(), &cards).unwrap();
    let mut ev = Vec::new();
    d.script_set_lp(1, 20000);
    assert_eq!(d.sides[1].lp, 9999);
    d.script_set_stars(0, 40);
    assert_eq!(d.sides[0].stars, 12);
    let cell = GridPos { column: 3, row: 5 };
    let id = d
        .script_place(0, 156, cell, &cards, &mut ev)
        .unwrap()
        .unwrap();
    assert!(d
        .script_place(0, 3, cell, &cards, &mut ev)
        .unwrap()
        .is_none());
    assert!(d.script_place(0, 9999, cell, &cards, &mut ev).is_err());
    let u = d.unit(id).unwrap();
    assert!((u.slot, u.face_up, u.defence, u.acted) == (1, false, false, false));
    d.script_unit_flag(0, 1, unit_flag::COMMITTED_DEFENCE, true)
        .unwrap();
    let u = d.unit(id).unwrap();
    assert!(u.defence && u.committed_defence);
    assert!(d.script_unit_flag(0, 1, unit_flag::BIT1, true).is_err());
    // A jar takes slot 6; a teleport onto an occupant deletes it.
    let jar = d
        .script_place(1, 683, GridPos { column: 3, row: 3 }, &cards, &mut ev)
        .unwrap()
        .unwrap();
    assert_eq!(d.unit(jar).unwrap().slot, 6);
    d.script_teleport(1, 6, cell).unwrap();
    assert!(d.unit(id).is_none());
    assert_eq!(d.unit(jar).unwrap().position, cell);
    assert!(d.script_teleport(1, 7, cell).is_err());
    d.script_spellbind(1, 6, 0xFF).unwrap();
    let u = d.unit(jar).unwrap();
    assert!(u.spellbind == 0xFF && u.spellbind_fresh);
    d.script_set_bind(1, 6, 0).unwrap();
    assert_eq!(d.unit(jar).unwrap().spellbind, 0);
    d.script_clear_units();
    assert_eq!(d.units.len(), 2);
    d.script_deck_reset();
    assert_eq!(d.sides[0].deck, DECK_RESET[0].to_vec());
    assert!(d.sides[1].hand.is_empty());
    d.script_set_round(99);
    d.script_set_summon_ok(0, false);
    assert!(d.round == 99 && !d.sides[0].can_summon);
    assert!(d
        .script_set_terrain(GridPos { column: 7, row: 0 }, 8)
        .is_err());
    d.script_set_terrain(GridPos { column: 0, row: 0 }, 8)
        .unwrap();
    assert_eq!(d.terrain[0], 8);
    // The LP-0 end is undone only for that reason.
    d.outcome = Some(crate::duel::Outcome {
        winner: None,
        reason: EndReason::LifePoints,
    });
    let mut events = vec![DuelEvent::Ended {
        outcome: d.outcome.unwrap(),
    }];
    d.script_suppress_lp0_end(&mut events);
    assert!(d.outcome.is_none() && events.is_empty());
    d.outcome = Some(crate::duel::Outcome {
        winner: Some(0),
        reason: EndReason::Surrender,
    });
    d.script_suppress_lp0_end(&mut events);
    assert!(d.outcome.is_some());
}

/// Private checks against the original: `ROSES_TEST_CATALOGUE` = the
/// imported `catalogue.json` (`roses import-duel`), `ROSES_TEST_ELF_PATH` =
/// the PAL executable (deck records 0x13/0x14 are read from its static
/// deck table), `ROSES_TEST_SESSION` = the project notes (the `.json`
/// summaries of states 133–186), `ROSES_TEST_TUTORIAL` = the
/// the project notes research folder (`emu-33.txt`).
mod private {
    use super::*;
    use std::path::PathBuf;

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
    }

    fn catalogue() -> crate::duel::Catalogue {
        let text = std::fs::read_to_string(env("ROSES_TEST_CATALOGUE")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// The static deck table (EE 0x2D10E0, file 0x1D7A60; 82-byte records:
    /// leader word rank<<12 | card, then 40 cards; `roses-assets::decks`).
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

    fn observed(state: &str) -> (Units, [u16; 2], u8, u8) {
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
        (
            units,
            lp,
            b[1].as_u64().unwrap() as u8,
            b[2].as_u64().unwrap() as u8,
        )
    }

    #[test]
    #[ignore = "requires the private catalogue, executable and saved states"]
    fn private_lesson_two_matches_the_original_states_and_run() {
        let cards = catalogue();
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let r = [record(&elf, 0x13), record(&elf, 0x14)];
        assert_eq!((r[0].leader_card, r[1].leader_card), (58, 0));
        let setup = lesson_setup(Lesson::Victory, [&r[0], &r[1]], [vec![], vec![]]).unwrap();
        let (duel, _) = Duel::new(setup, &cards).unwrap();
        let session = DuelSession::tutorial(duel, Tutorial::series(Lesson::Victory));
        let run = run_lesson(session, &cards, None);
        // Keys and poll counts of the interpreter run.
        let log = std::fs::read_to_string(env("ROSES_TEST_TUTORIAL").join("emu-33.txt")).unwrap();
        let emu: Vec<Transition> = log
            .lines()
            .filter_map(|l| {
                let l = l.strip_prefix("poll ")?;
                let mut it = l.split_whitespace();
                let poll = it.next()?.parse().ok()?;
                assert_eq!(it.next(), Some("step"));
                let a = it.next()?.trim_end_matches("->").parse().ok()?;
                let b = it.next()?.parse().ok()?;
                assert_eq!(it.next(), Some("ret"));
                let key = u16::from_str_radix(it.next()?.trim_start_matches("0x"), 16).ok()?;
                Some((poll, a, b, key))
            })
            .collect();
        assert_eq!(emu, EMU_33.to_vec());
        assert_eq!(relative(&run.transitions), emu);
        // Board, LP, round and active side of the saved states after the
        // step that produced them.
        let after = |step: u16| -> &Duel { &run.after.iter().find(|(s, _)| *s == step).unwrap().1 };
        let pairs = [
            ("133-n1.json", 0),
            ("137-n5.json", 3),
            ("142-n10.json", 11),
            ("145-n13.json", 13),
            ("154-n22.json", 25),
            ("157-n25.json", 27),
            ("159-n27.json", 33),
            ("163-n31.json", 37),
            ("164-n32.json", 41),
            ("165-n33.json", 43),
            ("166-n34.json", 47),
            ("167-n35.json", 49),
            ("172-n40.json", 54),
            ("179-n47.json", 60),
            ("183-n51.json", 62),
        ];
        for (state, step) in pairs {
            let (units, lp, round, active) = observed(state);
            let d = after(step);
            assert_eq!(unit_summary(d), units, "{state} after step {step}");
            assert_eq!([d.sides[0].lp, d.sides[1].lp], lp, "{state}");
            assert_eq!((d.round, d.active), (round, active), "{state}");
            assert!(d.outcome.is_none(), "{state}");
        }
        assert_eq!(
            run.session.tutorial.unwrap().ended,
            Some(LessonEnd::NextLesson {
                lesson: Lesson::Rules,
                series_counter: 0
            })
        );
    }
}
