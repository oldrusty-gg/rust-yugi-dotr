//! Lesson 3 checks: the (poll, step, key) list of the interpreter run
//! (`emu-34.txt`), the board/LP/round/stars/flags after every setup and
//! battle, the spellbind countdown, the Destiny Draw, the recorded ranks,
//! the explicit refusals, and (private) the saved original states
//! (`states-34.txt`).
//!
//! The engine refuses eight of the lesson's keys today (see `lesson3.rs`):
//! the attacks with a face-up Dragon Capture Jar on the field (57, 65, 74,
//! 80, 86, 95, 102) and the ability-#13 flip (284). The runs here apply the
//! outcome the saved states show for each refused key and go on, so every
//! other step is exercised; when the engine grows those rules the patches
//! simply stop being needed (a refusal at any other step fails the test).

use crate::duel::ai::weevil::FusionData;
use crate::duel::script::destiny::{kind_weights, pick, DestinyRows, DESTINY_USED};
use crate::duel::script::{
    lesson_setup, pad, unit_flag, DestinyDraw, Lesson, LessonEnd, Tutorial, DECK_RESET,
};
use crate::duel::session::DuelSession;
use crate::duel::ui::Mode;
use crate::duel::{next_rand31, CardData, CardInfo, Duel, DuelError, DuelEvent, DuelSetup};
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

/// Synthetic catalogue (not retail data): the card ids lesson 3 names with
/// the ATK/DEF and types its messages state (3765: Celtic Guardian
/// 1400/1200, a Warrior; 3783: Gyakutenno Megami 1800/2000, a Fairy; 3795:
/// Flame Swordsman above Wall Shadow; 3803: Flame Swordsman 1800; 3824:
/// Parrot Dragon 2000, WIND; 3825: Celtic Guardian EARTH), the rest
/// invented. The Wall Shadow carries the entry-1 type 8 that lets it stand
/// in the Labyrinth. The Dragon Capture Jar is a magic card (class 1) with
/// an entry-0 effect of type 0x17, which the engine's activation refuses.
/// The Dark Magician is a Spellcaster (kind 1) for the Destiny weights.
fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..854).map(|i| monster(i, 1, 6, 0, 100, 100)).collect();
    v[60] = monster(60, 1, 7, 1, 2500, 2100); // Dark Magician, side 0 leader
    v[21] = monster(21, 0, 3, 5, 1200, 700); // Baby Dragon, side 1 leader
    v[156] = monster(156, 3, 4, 3, 1400, 1200);
    v[5] = monster(5, 0, 4, 1, 1500, 1200);
    v[160] = monster(160, 3, 5, 3, 1500, 1700);
    v[321] = monster(321, 7, 4, 1, 1700, 1400);
    v[522] = monster(522, 14, 5, 3, 1400, 1800);
    v[3] = monster(3, 0, 7, 0, 2300, 2000);
    v[368] = monster(368, 8, 6, 0, 1800, 2000);
    v[151] = monster(151, 3, 5, 2, 1800, 1600);
    let mut wall = monster(144, 7, 7, 1, 1600, 3000);
    wall.effects[1] = true;
    wall.entries[1] = Some([8 << 8, 0]);
    v[144] = wall;
    v[29] = monster(29, 0, 5, 5, 2000, 1300);
    let mut jar = monster(683, 32, 0, 0, 0, 0);
    jar.effects[0] = true;
    jar.entries[0] = Some([0x17 << 8 | 0xC0, 0xFF]);
    v[683] = jar;
    v
}

/// Synthetic Destiny rows (invented ids): row 2 is the one the lesson arms.
fn rows() -> DestinyRows {
    let mut r = [[999u16; 3]; 8];
    for (i, row) in r.iter_mut().enumerate().skip(1) {
        *row = [600 + 3 * i as u16, 601 + 3 * i as u16, 602 + 3 * i as u16];
    }
    r
}

/// Deck records 0x15/0x16 with the first five cards of each as the lesson
/// shows them ([O 197] and `DECK_RESET`); the rest filler.
fn records() -> [DeckInput; 2] {
    let mut a: Vec<u16> = DECK_RESET[0].to_vec();
    a.extend((0..35).map(|i| 5 + i % 3));
    let mut b: Vec<u16> = DECK_RESET[1].to_vec();
    b.extend((0..35).map(|i| 5 + i % 3));
    [
        DeckInput {
            index: 0x15,
            leader_card: 60,
            leader_rank_raw: 1,
            cards: a,
        },
        DeckInput {
            index: 0x16,
            leader_card: 21,
            leader_rank_raw: 1,
            cards: b,
        },
    ]
}

fn setup() -> DuelSetup {
    let r = records();
    lesson_setup(Lesson::Rules, [&r[0], &r[1]], [vec![], vec![]]).unwrap()
}

fn session(cards: &impl CardData) -> DuelSession {
    let (duel, _) = Duel::new(setup(), cards).unwrap();
    DuelSession::tutorial(
        duel,
        Tutorial::series(Lesson::Rules).with_destiny_rows(rows()),
    )
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

/// The outcome of a key the engine refuses, as the saved states show it.
struct Patch {
    step: u16,
    key: u16,
    apply: fn(&mut DuelSession),
}

fn field(s: &mut DuelSession) {
    s.input.mode = Mode::Field;
}

fn acted(s: &mut DuelSession, side: u8, slot: u8) {
    s.duel
        .script_unit_flag(side, slot, unit_flag::ACTED, true)
        .unwrap();
}

/// The refused keys and their traced outcomes (the cursor already stands on
/// the target cell when the key is refused).
fn patches() -> Vec<Patch> {
    vec![
        // The Celtic Guardian destroys the enemy Jar at (3,3) and advances
        // (0x2493A0; the position is not observed, step 59 removes it).
        Patch {
            step: 57,
            key: pad::CROSS,
            apply: |s| {
                s.duel.script_remove(1, 6);
                s.duel
                    .script_teleport(0, 1, GridPos { column: 3, row: 3 })
                    .unwrap();
                acted(s, 0, 1);
                field(s);
            },
        },
        // Face-down, the same: the Jar dies, the monster stays face-down at
        // (3,3) [O 232].
        Patch {
            step: 65,
            key: pad::CROSS,
            apply: |s| {
                s.duel.script_remove(1, 6);
                s.duel
                    .script_teleport(0, 1, GridPos { column: 3, row: 3 })
                    .unwrap();
                acted(s, 0, 1);
                field(s);
            },
        },
        // A Jar attacking a monster is destroyed [O 237, 241, 244].
        Patch {
            step: 74,
            key: pad::CROSS,
            apply: |s| {
                s.duel.script_remove(0, 6);
                field(s);
            },
        },
        Patch {
            step: 80,
            key: pad::CROSS,
            apply: |s| {
                s.duel.script_remove(0, 7);
                field(s);
            },
        },
        Patch {
            step: 86,
            key: pad::CROSS,
            apply: |s| {
                s.duel.script_remove(0, 8);
                field(s);
            },
        },
        // A face-down Jar destroys the enemy Jar and moves in [O 252].
        Patch {
            step: 95,
            key: pad::CROSS,
            apply: |s| {
                s.duel.script_remove(1, 6);
                s.duel
                    .script_teleport(0, 6, GridPos { column: 2, row: 3 })
                    .unwrap();
                acted(s, 0, 6);
                field(s);
            },
        },
        // The face-up Jar destroys the enemy Jar, activates on completing
        // its move and leaves the field; the Kaiser Dragon is spellbound
        // for ever and marked acted (+0x57 0x15) [O 257–260].
        Patch {
            step: 102,
            key: pad::CROSS,
            apply: |s| {
                s.duel.script_remove(1, 7);
                s.duel.script_remove(0, 7);
                s.duel.script_spellbind(1, 1, 0xFF).unwrap();
                acted(s, 1, 1);
                field(s);
            },
        },
        // The Celtic Guardian moves to (3,1) and the Baby Dragon's ability
        // #13 turns it face-up [O 409].
        Patch {
            step: 284,
            key: pad::CROSS,
            apply: |s| {
                s.duel
                    .script_teleport(0, 1, GridPos { column: 3, row: 1 })
                    .unwrap();
                s.duel
                    .script_unit_flag(0, 1, unit_flag::COMMITTED_FACE_UP, true)
                    .unwrap();
                acted(s, 0, 1);
                field(s);
            },
        },
    ]
}

struct Run {
    session: DuelSession,
    transitions: Vec<Transition>,
    /// Duel, sub-state and side 0's cursor right after each step change, by
    /// the step entered.
    entered: Vec<(u16, Duel, u8, GridPos)>,
    /// The steps whose key the engine refused (patched), with the error.
    refused: Vec<(u16, DuelError)>,
    /// The Destiny Draw frames.
    destiny: Vec<(u32, DestinyDraw)>,
}

/// Runs until the lesson ends. A refusal at a patched step applies its
/// traced outcome and continues (the refused frame counts as the poll it
/// was in the original); any other refusal fails.
fn run_lesson(mut session: DuelSession, cards: &impl CardData, patches: &[Patch]) -> Run {
    let fusions = NoFusions;
    let mut op = Operator { checks: 0 };
    let mut transitions = Vec::new();
    let mut entered = Vec::new();
    let mut refused = Vec::new();
    let mut destiny = Vec::new();
    for _ in 0..8000 {
        let poll = session.tutorial.as_ref().unwrap().polls;
        let pad = op.pad(&session);
        let frame = match session.frame(pad, cards, &fusions) {
            Ok(f) => f,
            Err(e) => {
                let step = session.tutorial.as_ref().unwrap().step;
                let patch = patches
                    .iter()
                    .find(|p| p.step == step)
                    .unwrap_or_else(|| panic!("step {step} refused: {e}"));
                assert!(matches!(e, DuelError::Unsupported(_)), "step {step}: {e}");
                transitions.push((poll, step, step + 1, patch.key));
                let t = session.tutorial.as_mut().unwrap();
                t.polls += 1;
                t.step = step + 1;
                t.counter = 0;
                (patch.apply)(&mut session);
                entered.push((
                    step + 1,
                    session.duel.clone(),
                    session.input.state(),
                    session.input.cursors[0],
                ));
                refused.push((step, e));
                continue;
            }
        };
        let Some(s) = frame.script else { break };
        if let Some(d) = s.destiny {
            destiny.push((poll, d));
        }
        if s.key != 0 || s.step.0 != s.step.1 {
            transitions.push((poll, s.step.0, s.step.1, s.key));
        }
        if s.step.0 != s.step.1 {
            entered.push((
                s.step.1,
                session.duel.clone(),
                session.input.state(),
                session.input.cursors[0],
            ));
        }
        if s.ended.is_some() {
            break;
        }
    }
    let t = session.tutorial.as_ref().unwrap();
    assert!(
        t.ended.is_some(),
        "the run stopped at step {} (poll {}) without ending: outcome {:?}, sub-state {:#x}",
        t.step,
        t.polls,
        session.duel.outcome,
        session.input.state()
    );
    Run {
        session,
        transitions,
        entered,
        refused,
        destiny,
    }
}

impl Run {
    /// The duel right after the step was entered (after the frame that
    /// entered it, key applied).
    fn entered(&self, step: u16) -> &Duel {
        &self.entry(step).1
    }

    fn entry(&self, step: u16) -> &(u16, Duel, u8, GridPos) {
        self.entered
            .iter()
            .find(|(s, ..)| *s == step)
            .unwrap_or_else(|| panic!("step {step} never entered"))
    }

    fn state_at(&self, step: u16) -> u8 {
        self.entry(step).2
    }

    fn cursor_at(&self, step: u16) -> GridPos {
        self.entry(step).3
    }

    fn refused_steps(&self) -> Vec<u16> {
        self.refused.iter().map(|(s, _)| *s).collect()
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

fn at(duel: &Duel, column: u8, row: u8) -> &crate::duel::DuelUnit {
    duel.unit_at(GridPos { column, row })
        .unwrap_or_else(|| panic!("no unit at ({column},{row})"))
}

fn cell(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

/// The 3×3 block of the terrain lessons.
fn block(duel: &Duel) -> Vec<u8> {
    (2..=4)
        .flat_map(|c| (2..=4).map(move |r| (c, r)))
        .map(|(c, r)| duel.terrain_of(cell(c, r)).unwrap())
        .collect()
}

/// The interpreter run from saved state 189 (`emu-34.txt`, TUTORIAL.md
/// §6): poll, step before → after, key. It started inside step 2, so the
/// polls are compared relative to the 2 → 3 change.
const EMU_34: &[Transition] = &[
    (16, 2, 3, 0x0),
    (33, 3, 4, 0x0),
    (65, 4, 5, 0x0),
    (82, 5, 6, 0x0),
    (100, 6, 7, 0x0),
    (162, 7, 8, 0x0),
    (190, 8, 9, 0x0),
    (204, 9, 10, 0x80),
    (218, 10, 11, 0x1000),
    (232, 11, 12, 0x40),
    (246, 12, 13, 0x2000),
    (260, 13, 14, 0x2000),
    (274, 14, 15, 0x40),
    (288, 15, 16, 0x40),
    (316, 16, 17, 0x0),
    (333, 17, 18, 0x0),
    (365, 18, 19, 0x0),
    (382, 19, 20, 0x0),
    (410, 20, 21, 0x0),
    (472, 21, 22, 0x0),
    (490, 22, 23, 0x0),
    (504, 23, 24, 0x80),
    (518, 24, 25, 0x1000),
    (532, 25, 26, 0x40),
    (546, 26, 27, 0x2000),
    (560, 27, 28, 0x2000),
    (574, 28, 29, 0x40),
    (588, 29, 30, 0x40),
    (650, 30, 31, 0x0),
    (678, 31, 32, 0x0),
    (692, 32, 33, 0x800),
    (706, 33, 34, 0x800),
    (768, 34, 35, 0x0),
    (786, 35, 36, 0x0),
    (795, 36, 37, 0x0),
    (827, 37, 38, 0x0),
    (836, 38, 39, 0x0),
    (898, 39, 40, 0x0),
    (921, 40, 41, 0x0),
    (935, 41, 42, 0x800),
    (949, 42, 43, 0x800),
    (1011, 43, 44, 0x0),
    (1029, 44, 45, 0x0),
    (1046, 45, 46, 0x0),
    (1078, 46, 47, 0x0),
    (1095, 47, 48, 0x0),
    (1123, 48, 49, 0x0),
    (1140, 49, 50, 0x0),
    (1172, 50, 51, 0x0),
    (1189, 51, 52, 0x0),
    (1212, 52, 53, 0x0),
    (1226, 53, 54, 0x1000),
    (1240, 54, 55, 0x1000),
    (1254, 55, 56, 0x40),
    (1268, 56, 57, 0x1000),
    (1282, 57, 58, 0x40),
    (1300, 58, 59, 0x0),
    (1309, 59, 60, 0x0),
    (1341, 60, 61, 0x0),
    (1358, 61, 62, 0x0),
    (1381, 62, 63, 0x0),
    (1395, 63, 64, 0x40),
    (1409, 64, 65, 0x1000),
    (1423, 65, 66, 0x40),
    (1446, 66, 67, 0x0),
    (1455, 67, 68, 0x0),
    (1487, 68, 69, 0x0),
    (1504, 69, 70, 0x0),
    (1522, 70, 71, 0x0),
    (1536, 71, 72, 0x8000),
    (1550, 72, 73, 0x40),
    (1564, 73, 74, 0x1000),
    (1578, 74, 75, 0x40),
    (1601, 75, 76, 0x0),
    (1615, 76, 77, 0x2000),
    (1629, 77, 78, 0x4000),
    (1643, 78, 79, 0x40),
    (1657, 79, 80, 0x1000),
    (1671, 80, 81, 0x40),
    (1694, 81, 82, 0x0),
    (1708, 82, 83, 0x2000),
    (1722, 83, 84, 0x4000),
    (1736, 84, 85, 0x40),
    (1750, 85, 86, 0x1000),
    (1764, 86, 87, 0x40),
    (1792, 87, 88, 0x0),
    (1801, 88, 89, 0x8000),
    (1833, 89, 90, 0x0),
    (1850, 90, 91, 0x0),
    (1878, 91, 92, 0x0),
    (1892, 92, 93, 0x8000),
    (1906, 93, 94, 0x40),
    (1920, 94, 95, 0x1000),
    (1934, 95, 96, 0x40),
    (1967, 96, 97, 0x0),
    (1981, 97, 98, 0x4000),
    (1995, 98, 99, 0x2000),
    (2009, 99, 100, 0x2000),
    (2023, 100, 101, 0x40),
    (2037, 101, 102, 0x1000),
    (2051, 102, 103, 0x40),
    (2089, 103, 104, 0x0),
    (2106, 104, 105, 0x0),
    (2138, 105, 106, 0x0),
    (2155, 106, 107, 0x0),
    (2188, 107, 108, 0x0),
    (2197, 108, 109, 0x0),
    (2229, 109, 110, 0x0),
    (2238, 110, 111, 0x0),
    (2256, 111, 112, 0x0),
    (2318, 112, 113, 0x0),
    (2341, 113, 114, 0x0),
    (2355, 114, 115, 0x40),
    (2369, 115, 116, 0x1000),
    (2383, 116, 117, 0x40),
    (2411, 117, 118, 0x0),
    (2420, 118, 119, 0x0),
    (2452, 119, 120, 0x0),
    (2461, 120, 121, 0x0),
    (2489, 121, 122, 0x0),
    (2551, 122, 123, 0x0),
    (2569, 123, 124, 0x0),
    (2583, 124, 125, 0x40),
    (2601, 125, 126, 0x0),
    (2615, 126, 127, 0x1000),
    (2629, 127, 128, 0x1000),
    (2643, 128, 129, 0x40),
    (2661, 129, 130, 0x0),
    (2670, 130, 131, 0x0),
    (2702, 131, 132, 0x0),
    (2711, 132, 133, 0x0),
    (2729, 133, 134, 0x0),
    (2743, 134, 135, 0x40),
    (2766, 135, 136, 0x0),
    (2828, 136, 137, 0x2),
    (2890, 137, 138, 0x1),
    (2952, 138, 139, 0x2),
    (3014, 139, 140, 0x1),
    (3076, 140, 141, 0x2),
    (3077, 141, 142, 0x0),
    (3100, 142, 143, 0x0),
    (3117, 143, 144, 0x20),
    (3149, 144, 145, 0x0),
    (3150, 145, 155, 0x0),
    (3166, 155, 156, 0x0),
    (3184, 156, 157, 0x0),
    (3193, 157, 158, 0x0),
    (3225, 158, 159, 0x0),
    (3234, 159, 160, 0x0),
    (3296, 160, 161, 0x0),
    (3324, 161, 162, 0x0),
    (3333, 162, 163, 0x0),
    (3365, 163, 164, 0x0),
    (3374, 164, 165, 0x0),
    (3388, 165, 166, 0x40),
    (3402, 166, 167, 0x1000),
    (3416, 167, 168, 0x40),
    (3444, 168, 169, 0x0),
    (3461, 169, 170, 0x0),
    (3493, 170, 171, 0x0),
    (3510, 171, 172, 0x0),
    (3553, 172, 173, 0x0),
    (3562, 173, 174, 0x0),
    (3594, 174, 175, 0x0),
    (3611, 175, 176, 0x0),
    (3629, 176, 177, 0x4000),
    (3647, 177, 178, 0x0),
    (3661, 178, 179, 0x40),
    (3675, 179, 180, 0x1000),
    (3689, 180, 181, 0x40),
    (3712, 181, 182, 0x0),
    (3729, 182, 183, 0x0),
    (3761, 183, 184, 0x0),
    (3778, 184, 185, 0x0),
    (3811, 185, 186, 0x0),
    (3828, 186, 187, 0x0),
    (3860, 187, 188, 0x0),
    (3877, 188, 189, 0x0),
    (3895, 189, 190, 0x0),
    (3909, 190, 191, 0x8000),
    (3923, 191, 192, 0x40),
    (3937, 192, 193, 0x1000),
    (3951, 193, 194, 0x40),
    (3969, 194, 195, 0x0),
    (3983, 195, 196, 0x4000),
    (3997, 196, 197, 0x2000),
    (4011, 197, 198, 0x2000),
    (4025, 198, 199, 0x40),
    (4039, 199, 200, 0x1000),
    (4053, 200, 201, 0x40),
    (4091, 201, 202, 0x0),
    (4108, 202, 203, 0x0),
    (4140, 203, 204, 0x0),
    (4157, 204, 205, 0x0),
    (4190, 205, 206, 0x0),
    (4199, 206, 207, 0x0),
    (4231, 207, 208, 0x0),
    (4240, 208, 146, 0x0),
    (4302, 146, 147, 0x0),
    (4303, 147, 209, 0x0),
    (4331, 209, 210, 0x0),
    (4345, 210, 211, 0x800),
    (4359, 211, 212, 0x800),
    (4373, 212, 213, 0x800),
    (4387, 213, 214, 0x800),
    (4410, 214, 215, 0x0),
    (4419, 215, 216, 0x0),
    (4451, 216, 217, 0x0),
    (4460, 217, 218, 0x0),
    (4488, 218, 219, 0x0),
    (4550, 219, 220, 0x0),
    (4578, 220, 221, 0x0),
    (4595, 221, 222, 0x0),
    (4627, 222, 223, 0x0),
    (4636, 223, 224, 0x0),
    (4698, 224, 225, 0x0),
    (4716, 225, 226, 0x0),
    (4725, 226, 227, 0x0),
    (4757, 227, 228, 0x0),
    (4766, 228, 229, 0x0),
    (4784, 229, 230, 0x0),
    (4798, 230, 231, 0x40),
    (4812, 231, 232, 0x1000),
    (4874, 232, 233, 0x0),
    (4897, 233, 234, 0x0),
    (4911, 234, 235, 0x40),
    (4944, 235, 236, 0x0),
    (4961, 236, 237, 0x0),
    (4993, 237, 238, 0x0),
    (5010, 238, 239, 0x0),
    (5043, 239, 240, 0x0),
    (5052, 240, 241, 0x0),
    (5084, 241, 242, 0x0),
    (5093, 242, 243, 0x0),
    (5121, 243, 244, 0x0),
    (5183, 244, 245, 0x0),
    (5206, 245, 246, 0x0),
    (5220, 246, 247, 0x80),
    (5234, 247, 248, 0x1000),
    (5248, 248, 249, 0x40),
    (5266, 249, 250, 0x0),
    (5280, 250, 251, 0x2000),
    (5294, 251, 252, 0x2000),
    (5308, 252, 253, 0x2000),
    (5322, 253, 254, 0x2000),
    (5340, 254, 148, 0x0),
    (5341, 148, 149, 0x20),
    (5342, 149, 255, 0x20),
    (5359, 255, 256, 0x20),
    (5391, 256, 257, 0x20),
    (5408, 257, 258, 0x0),
    (5451, 258, 259, 0x0),
    (5468, 259, 260, 0x0),
    (5500, 260, 261, 0x0),
    (5517, 261, 262, 0x0),
    (5540, 262, 263, 0x0),
    (5602, 263, 264, 0x0),
    (5625, 264, 265, 0x0),
    (5687, 265, 266, 0x10),
    (5749, 266, 267, 0x4000),
    (5811, 267, 268, 0x4000),
    (5873, 268, 269, 0x4000),
    (5896, 269, 270, 0x0),
    (5958, 270, 271, 0x20),
    (5972, 271, 272, 0x800),
    (6034, 272, 273, 0x10),
    (6096, 273, 274, 0x4000),
    (6158, 274, 275, 0x4000),
    (6220, 275, 276, 0x4000),
    (6253, 276, 277, 0x20),
    (6315, 277, 278, 0x800),
    (6324, 278, 279, 0x0),
    (6356, 279, 280, 0x0),
    (6365, 280, 281, 0x0),
    (6388, 281, 282, 0x0),
    (6402, 282, 283, 0x40),
    (6416, 283, 284, 0x1000),
    (6430, 284, 285, 0x40),
    (6463, 285, 286, 0x0),
    (6480, 286, 287, 0x0),
    (6498, 287, 288, 0x0),
];

/// The run's transitions from step 2 on, re-based so the 2 → 3 change is
/// at poll 16 as in the interpreter log.
fn relative(transitions: &[Transition]) -> Vec<Transition> {
    let base = transitions
        .iter()
        .find(|t| t.1 == 2 && t.2 == 3)
        .expect("step 2 closes")
        .0;
    transitions
        .iter()
        .filter(|t| t.1 >= 2)
        .map(|&(p, a, b, k)| (p + 16 - base, a, b, k))
        .collect()
}

/// The steps the engine refuses with the synthetic catalogue today. The
/// face-up Jars of steps 57-102 are handled by the engine since face-up
/// magic cards activate when acted with (TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md
/// §4); their patches stay as the traced outcome, unused.
const REFUSED: [u16; 1] = [284];

#[test]
fn lesson_three_keys_and_poll_counts_match_the_interpreter_run() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, &patches());
    assert_eq!(run.transitions[0], (0, 0, 2, 0));
    assert_eq!(relative(&run.transitions), EMU_34.to_vec());
    assert_eq!(run.refused_steps(), REFUSED.to_vec());
    let t = run.session.tutorial.as_ref().unwrap();
    assert_eq!(t.ended, Some(LessonEnd::SeriesEnd { quit: false }));
    assert_eq!(t.step, 288);
    assert!(t.no_lp0_end);
    assert!(run.session.duel.outcome.is_none());
    // The last message closes on a black screen (step 286's fade).
    assert_eq!(t.fade, [0, 0]);
}

#[test]
fn lesson_three_limit_and_star_setups_reach_the_traced_positions() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, &patches());
    // Step 3 [O 191]: five face-up monsters in slots 1..5, five face-down
    // Jars in 6..10.
    let d = run.entered(4);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 3, 4),
            (0, 2, 5, 2, 4),
            (0, 3, 160, 4, 4),
            (0, 4, 321, 2, 5),
            (0, 5, 522, 4, 5),
            (0, 6, 683, 2, 6),
            (0, 7, 683, 4, 6),
            (0, 8, 683, 1, 6),
            (0, 9, 683, 5, 6),
            (0, 10, 683, 1, 5),
            (1, 0, 21, 3, 0)
        ]
    );
    for u in d.units.iter().filter(|u| u.slot != 0) {
        assert_eq!(
            u.face_up && u.committed_face_up,
            u.slot <= 5,
            "slot {}",
            u.slot
        );
        assert!(!u.acted && !u.defence);
    }
    // Steps 9–15 [O 196–199]: the summon cell, the refill, the sixth
    // monster reduced to dust: no unit, the card and the stars gone.
    assert_eq!(run.state_at(11), 0x3E);
    let d = run.entered(12);
    assert_eq!(d.sides[0].hand, vec![321, 285, 156, 249, 505]);
    assert_eq!(d.sides[0].deck.len(), 35);
    assert_eq!(run.state_at(12), 0x43);
    let d = run.entered(16);
    assert_eq!(unit_summary(d).len(), 12);
    assert_eq!(d.sides[0].hand, vec![321, 285, 249, 505]);
    assert_eq!(d.sides[0].stars, 0);
    assert!(!d.sides[0].can_summon);
    assert_eq!(run.state_at(16), 0x18);
    // Step 17 [O 202]: cleared, the five-card decks, 4 stars, summon on.
    let d = run.entered(18);
    assert_eq!(unit_summary(d), vec![(0, 0, 60, 3, 6), (1, 0, 21, 3, 0)]);
    assert_eq!(d.sides[0].deck, DECK_RESET[0].to_vec());
    assert_eq!(d.sides[1].deck, DECK_RESET[1].to_vec());
    assert!(d.sides[0].hand.is_empty() && d.sides[1].hand.is_empty());
    assert_eq!((d.sides[0].stars, d.sides[1].stars), (4, 4));
    assert!(d.sides[0].can_summon);
    assert_eq!(run.cursor_at(19), cell(3, 6));
    // Steps 23–29 [O 207–208]: the Celtic Guardian (level 4) at (3,5)
    // face-down for the 4 stars.
    let d = run.entered(30);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 5), (1, 0, 21, 3, 0)]
    );
    assert!(!at(d, 3, 5).face_up);
    assert_eq!(d.sides[0].stars, 0);
    assert!(d.sides[0].deck.is_empty());
    // Steps 32–33 [O 212–214]: two turn ends, round 2, +3 stars.
    assert_eq!((run.entered(33).active, run.entered(33).round), (1, 1));
    let d = run.entered(34);
    assert_eq!((d.active, d.round, d.sides[0].stars), (0, 2, 3));
    assert!(d.sides[0].can_summon);
    // Steps 37–42 [O 215–220]: 12 stars stay 12 after the income.
    assert_eq!(run.entered(38).sides[0].stars, 12);
    let d = run.entered(43);
    assert_eq!((d.round, d.sides[0].stars, d.sides[1].stars), (3, 12, 12));
    // Step 45 [O 221]: round 0, decks reset, 4 stars.
    let d = run.entered(46);
    assert_eq!((d.round, d.active, d.sides[0].stars), (0, 0, 4));
    assert_eq!(unit_summary(d).len(), 2);
    assert_eq!(d.sides[0].deck, DECK_RESET[0].to_vec());
}

#[test]
fn lesson_three_spell_battles_follow_the_saved_states() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, &patches());
    // Step 49 [O 224]: the Celtic Guardian face-up at (3,4), an enemy Jar
    // face-up at (3,3) in slot 6.
    let d = run.entered(50);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 3, 4),
            (1, 0, 21, 3, 0),
            (1, 6, 683, 3, 3)
        ]
    );
    assert!(at(d, 3, 4).committed_face_up && at(d, 3, 3).committed_face_up);
    // Steps 53–57: Up, Up, select, Up, attack (refused today, patched).
    assert_eq!(run.cursor_at(55), cell(3, 4));
    assert_eq!(run.state_at(56), 0x1D);
    assert_eq!(run.cursor_at(57), cell(3, 3));
    let d = run.entered(58);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 3), (1, 0, 21, 3, 0)]
    );
    assert_eq!(lp(d), [4000, 4000]);
    // Step 59 [O 229]: a fresh face-down Celtic Guardian, a fresh Jar.
    let d = run.entered(60);
    assert!(!at(d, 3, 4).face_up && at(d, 3, 3).committed_face_up);
    assert_eq!(run.cursor_at(61), cell(3, 4));
    // Step 65 [O 232]: the Jar dies, the monster stays face-down at (3,3).
    let d = run.entered(66);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 3, 3), (1, 0, 21, 3, 0)]
    );
    assert!(!at(d, 3, 3).face_up && at(d, 3, 3).acted);
    // Step 67 [O 234]: three own Jars face-up, three Kaiser Dragons.
    let d = run.entered(68);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 6, 683, 2, 4),
            (0, 7, 683, 3, 4),
            (0, 8, 683, 4, 4),
            (1, 0, 21, 3, 0),
            (1, 1, 3, 2, 3),
            (1, 2, 3, 3, 3),
            (1, 3, 3, 4, 3)
        ]
    );
    let k2 = at(d, 3, 3);
    assert!(k2.committed_face_up && k2.defence && k2.committed_defence);
    assert!(!at(d, 4, 3).face_up);
    assert_eq!(run.cursor_at(69), cell(3, 4));
    // Steps 71–86 [O 237, 241, 244]: each attacking Jar is destroyed, the
    // face-down Kaiser Dragon stays face-down; the cursor rests on the
    // target after each attack.
    assert_eq!(run.cursor_at(73), cell(2, 4));
    assert_eq!(run.cursor_at(75), cell(2, 3));
    assert!(run.entered(75).unit_at(cell(2, 4)).is_none());
    assert_eq!(run.cursor_at(78), cell(3, 4));
    assert!(run.entered(81).unit_at(cell(3, 4)).is_none());
    assert_eq!(run.cursor_at(84), cell(4, 4));
    let d = run.entered(87);
    assert!(d.unit_at(cell(4, 4)).is_none() && !at(d, 4, 3).face_up);
    assert_eq!(lp(d), [4000, 4000]);
    // Step 88 [O 247]: spell against spell; Left is pressed with the fade.
    let d = run.entered(89);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 6, 683, 2, 4),
            (0, 7, 683, 4, 4),
            (1, 0, 21, 3, 0),
            (1, 1, 3, 3, 3),
            (1, 6, 683, 2, 3),
            (1, 7, 683, 4, 3)
        ]
    );
    assert!(!at(d, 2, 4).face_up && !at(d, 2, 4).committed_face_up);
    assert!(at(d, 4, 4).committed_face_up);
    assert!(at(d, 3, 3).committed_face_up && !at(d, 3, 3).acted);
    let j7 = at(d, 4, 3);
    assert!(!j7.face_up && j7.defence && j7.committed_defence);
    assert_eq!(run.cursor_at(89), cell(3, 3));
    assert_eq!(run.cursor_at(90), cell(3, 4));
    // Step 95 [O 252]: the face-down Jar penetrates to (2,3).
    let d = run.entered(96);
    assert!(d.unit_at(cell(2, 4)).is_none());
    let j6 = at(d, 2, 3);
    assert!((j6.side, j6.slot, j6.acted, j6.face_up) == (0, 6, true, false));
    // Step 102 [O 257–259]: the face-up Jar activates on its move; the
    // Kaiser Dragon is bound for ever, both Jars leave.
    let d = run.entered(103);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 6, 683, 2, 3),
            (1, 0, 21, 3, 0),
            (1, 1, 3, 3, 3)
        ]
    );
    let kaiser = at(d, 3, 3);
    assert!(kaiser.spellbind == 0xFF && kaiser.spellbind_fresh);
    assert_eq!(run.cursor_at(103), cell(4, 3));
    assert_eq!(run.state_at(103), 0x18);
}

#[test]
fn lesson_three_terrain_lessons_follow_the_saved_states() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, &patches());
    // Step 104 [O 265]: a Meadow block, the board cleared.
    let d = run.entered(105);
    assert_eq!(unit_summary(d).len(), 2);
    assert_eq!(block(d), vec![3; 9]);
    assert_eq!(d.terrain_of(cell(3, 5)), Some(7));
    // Steps 108–116 [O 269–273]: the Celtic Guardian steps onto the Meadow.
    assert!(at(run.entered(109), 3, 5).committed_face_up);
    assert_eq!(run.cursor_at(109), cell(3, 5));
    let d = run.entered(117);
    assert!(at(d, 3, 4).acted && at(d, 3, 4).card == 156);
    // Step 118 [O 275]: its action restored; steps 124–128 [O 279–281]:
    // two steps from favoured terrain.
    assert!(!at(run.entered(119), 3, 4).acted);
    assert_eq!(run.state_at(125), 0x1D);
    let d = run.entered(129);
    assert_eq!(at(d, 3, 2).card, 156);
    assert_eq!(run.state_at(129), 0x18);
    // Steps 130–141 [O 282–289]: face-down at (3,3), one step until R2
    // turns it up in the move state; the fifth toggle leaves it face-up.
    let d = run.entered(131);
    assert!(!at(d, 3, 3).face_up);
    assert_eq!(run.cursor_at(132), cell(3, 3));
    for step in [135, 137, 138, 139, 140, 141, 142] {
        assert_eq!(run.state_at(step), 0x1D, "step {step}");
    }
    // Step 143 [O 293]: the selected unit is replaced by the Gyakutenno
    // Megami on a Dark block; Circle leaves the move state, cursor kept.
    let d = run.entered(144);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 368, 3, 5), (1, 0, 21, 3, 0)]
    );
    assert!(at(d, 3, 5).committed_face_up);
    assert_eq!(block(d), vec![5; 9]);
    assert_eq!(run.state_at(144), 0x18);
    assert_eq!(run.cursor_at(144), cell(3, 3));
    // 145 → 155 → 156 [emu].
    assert_eq!(run.cursor_at(159), cell(3, 5));
    let d = run.entered(168);
    assert_eq!(at(d, 3, 4).card, 368);
    assert!(at(d, 3, 4).acted);
    // Steps 169–180 [O 303–315]: the Labyrinth; the Wall Shadow inside it
    // is destroyed, LP1 3800, the Flame Swordsman cannot enter.
    let d = run.entered(170);
    assert_eq!(block(d), vec![8; 9]);
    let d = run.entered(174);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 151, 3, 5),
            (1, 0, 21, 3, 0),
            (1, 1, 144, 3, 4)
        ]
    );
    assert_eq!(run.cursor_at(174), cell(3, 4));
    assert_eq!(run.cursor_at(177), cell(3, 5));
    let d = run.entered(181);
    assert_eq!(lp(d), [4000, 3800]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 151, 3, 5), (1, 0, 21, 3, 0)]
    );
    assert!(at(d, 3, 5).acted);
    // Steps 182–200 [O 317–328]: Crush; the Celtic Guardian (1400) enters
    // it, the Flame Swordsman (1800) is destroyed on entry.
    assert_eq!(block(run.entered(183)), vec![9; 9]);
    let d = run.entered(187);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 2, 5),
            (0, 2, 151, 4, 5),
            (1, 0, 21, 3, 0)
        ]
    );
    assert_eq!(run.cursor_at(188), cell(3, 5));
    let d = run.entered(194);
    assert_eq!(at(d, 2, 4).card, 156);
    let d = run.entered(201);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (0, 1, 156, 2, 4), (1, 0, 21, 3, 0)]
    );
    assert_eq!(lp(d), [4000, 3800]);
    // Step 202 [O 333]: back to normal terrain at (3,3), ready to act.
    let d = run.entered(203);
    assert_eq!(block(d), vec![7; 9]);
    assert!(at(d, 3, 3).card == 156 && !at(d, 3, 3).acted);
}

#[test]
fn lesson_three_spellbinds_count_down_and_the_attribute_bind_lands() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, &patches());
    // Step 206 [O 337]: bound for one turn, fresh.
    let c = at(run.entered(207), 3, 3);
    assert!(c.spellbind == 1 && c.spellbind_fresh);
    // 208 → 146 → 147 → 209 [emu]; the four turn ends: the fresh bind
    // survives the first, counts down at the third, the monster acts again
    // in round 2 [O 341–344].
    for step in [146, 147, 209] {
        run.entered(step);
    }
    let d = run.entered(211);
    assert_eq!(d.active, 1);
    let c = at(d, 3, 3);
    assert!(c.spellbind == 1 && !c.spellbind_fresh);
    let d = run.entered(212);
    assert_eq!((d.active, d.round), (0, 1));
    assert!(at(d, 3, 3).acted && at(d, 3, 3).spellbind == 1);
    assert_eq!(at(run.entered(213), 3, 3).spellbind, 0);
    let d = run.entered(214);
    assert_eq!((d.active, d.round), (0, 2));
    assert!(!at(d, 3, 3).acted);
    // Step 215 [O 346]: eternal; step 221 [O 352]: cleared, the Celtic
    // Guardian at (3,4), a Parrot Dragon face-up at (3,3).
    assert_eq!(at(run.entered(216), 3, 3).spellbind, 0xFF);
    let d = run.entered(222);
    assert_eq!(
        unit_summary(d),
        vec![
            (0, 0, 60, 3, 6),
            (0, 1, 156, 3, 4),
            (1, 0, 21, 3, 0),
            (1, 1, 29, 3, 3)
        ]
    );
    assert_eq!(at(d, 3, 4).spellbind, 0);
    assert!(at(d, 3, 3).committed_face_up);
    assert_eq!(run.cursor_at(222), cell(3, 4));
    // Steps 230–234 [O 355–361]: the attack costs 600 and the Celtic
    // Guardian; EARTH beats WIND, so the survivor is bound for a turn.
    assert_eq!(run.state_at(232), 0x1D);
    assert_eq!(run.cursor_at(232), cell(3, 3));
    let d = run.entered(235);
    assert_eq!(lp(d), [3400, 3800]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (1, 0, 21, 3, 0), (1, 1, 29, 3, 3)]
    );
    let parrot = at(d, 3, 3);
    assert!(parrot.spellbind == 1 && parrot.spellbind_fresh);
}

#[test]
fn lesson_three_destiny_draw_replaces_the_last_deck_draw() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, &patches());
    // Step 236 [O 365]: a Kaiser Dragon at (2,5), LP0 400; step 240
    // [O 370]: the byte armed with situation 2, summon on.
    let d = run.entered(237);
    assert_eq!(lp(d), [400, 3800]);
    assert_eq!(
        unit_summary(d),
        vec![(0, 0, 60, 3, 6), (1, 0, 21, 3, 0), (1, 1, 3, 2, 5)]
    );
    assert_eq!(run.entered(238).sides[0].destiny, 0);
    let d = run.entered(241);
    assert_eq!(d.sides[0].destiny, 2);
    assert!(d.sides[0].can_summon);
    assert!(d.sides[0].hand.is_empty() && d.sides[0].deck.len() == 5);
    // Steps 246–248 [O 375–377]: the refill draws four deck cards and the
    // special card; the fifth deck card stays; the byte is used.
    assert_eq!(run.state_at(247), 0x3E);
    let d = run.entered(249);
    assert_eq!(run.state_at(249), 0x43);
    assert_eq!(&d.sides[0].hand[..4], &[321, 285, 156, 249]);
    assert!(rows()[2].contains(&d.sides[0].hand[4]));
    assert_eq!(d.sides[0].deck, vec![505]);
    assert_eq!(d.sides[0].destiny, 2 | DESTINY_USED);
    let poll = run.transitions.iter().find(|t| t.1 == 248).unwrap().0;
    assert_eq!(
        run.destiny,
        vec![(
            poll,
            DestinyDraw {
                side: 0,
                card: d.sides[0].hand[4]
            }
        )]
    );
    // Steps 250–253: the hand cursor on the far right; 254 → 148 → 149 →
    // 255: two Circles close the menu and the summon cell [O 378].
    assert_eq!(run.state_at(254), 0x43);
    assert_eq!(run.state_at(148), 0x43);
    assert_eq!(run.state_at(149), 0x3E);
    assert_eq!(run.state_at(255), 0x18);
    assert_eq!(run.cursor_at(255), cell(3, 6));
    let d = run.entered(256);
    assert_eq!(unit_summary(d), vec![(0, 0, 60, 3, 6), (1, 0, 21, 3, 0)]);
    assert_eq!(d.sides[0].hand.len(), 5);
    assert!(d.sides[0].can_summon);
    // Step 277: side 0's turn starts at 400 LP with the byte used, so the
    // engine's arming refusal does not fire [O 404].
    let d = run.entered(272);
    assert_eq!((d.active, d.round), (1, 2));
    let d = run.entered(278);
    assert_eq!((d.active, d.round, lp(d)[0]), (0, 3, 400));
}

#[test]
fn lesson_three_ranks_are_recorded_and_the_flip_is_refused_explicitly() {
    let cards = cards();
    let run = run_lesson(session(&cards), &cards, &patches());
    let t = run.session.tutorial.as_ref().unwrap();
    assert_eq!(t.leader_ranks, [Some(12), Some(12)]);
    assert_eq!(run.entered(259).sides[0].leader_abilities, Vec::<u8>::new());
    // Steps 265–277 [O 389–404]: the leader details on both sides.
    assert_eq!(run.cursor_at(261), cell(3, 6));
    assert_eq!(run.state_at(266), 0x21);
    assert_eq!(run.state_at(269), 0x21);
    assert_eq!(run.state_at(271), 0x18);
    assert_eq!(run.state_at(273), 0x21);
    assert_eq!(run.state_at(276), 0x21);
    assert_eq!(run.state_at(277), 0x18);
    // Step 278 [O 405]: a face-down Celtic Guardian at (3,2); steps
    // 282–283 select it and point at (3,1); 284 is refused (the patched
    // outcome is the flip [O 409]).
    let d = run.entered(279);
    assert!(!at(d, 3, 2).face_up && at(d, 3, 2).card == 156);
    assert_eq!(run.cursor_at(280), cell(3, 2));
    assert_eq!(run.state_at(284), 0x1D);
    assert_eq!(run.cursor_at(284), cell(3, 1));
    let (_, err) = run.refused.iter().find(|(s, _)| *s == 284).unwrap();
    assert!(
        matches!(err, DuelError::Unsupported(m) if m.contains("#13")),
        "{err}"
    );
    let d = run.entered(285);
    let c = at(d, 3, 1);
    assert!(c.face_up && c.committed_face_up && c.acted);
    // The first refusal of an unpatched run is the ability-#13 flip of step
    // 284 (the face-up Jars of 57-102 are run by the engine), and it leaves
    // the session as it was.
    let mut s = session(&cards);
    let fusions = NoFusions;
    let mut op = Operator { checks: 0 };
    let mut refusal = None;
    for _ in 0..8000 {
        let pad = op.pad(&s);
        match s.frame(pad, &cards, &fusions) {
            Ok(_) => (),
            Err(e) => {
                refusal = Some((s.tutorial.as_ref().unwrap().step, e));
                break;
            }
        }
    }
    let (step, err) = refusal.unwrap();
    assert_eq!(step, 284);
    assert!(
        matches!(&err, DuelError::Unsupported(m) if m.contains("#13")),
        "{err}"
    );
    assert_eq!(s.tutorial.as_ref().unwrap().step, 284);
    assert_eq!(s.input.state(), 0x1D);
}

#[test]
fn destiny_pick_follows_the_traced_weights() {
    assert_eq!(kind_weights(1), Some((2, 1)));
    assert_eq!(kind_weights(8), Some((12, 6)));
    assert_eq!(kind_weights(20), Some((-5, -2)));
    assert_eq!(kind_weights(21), None);
    let rows = rows();
    // A used byte or an unknown kind: no pick, no rand consumed.
    let mut rng = 7;
    assert_eq!(pick(&rows, 2 | DESTINY_USED, 1, &mut rng), None);
    assert_eq!(pick(&rows, 2, 21, &mut rng), None);
    assert_eq!(rng, 7);
    // Spellcaster, rank bonus 0: buckets r < 12, r < 33, else; one rand31
    // per pick. Find a seed for each bucket and check the card.
    let bucket = |seed: u64| {
        let mut probe = seed;
        (100.0_f32 * (next_rand31(&mut probe) as f32 / 2_147_483_648.0_f32)) as i32
    };
    let mut seen = [false; 3];
    for seed in 0..10_000u64 {
        let r = bucket(seed);
        let want = if r < 12 {
            0
        } else if r < 33 {
            1
        } else {
            2
        };
        let mut rng = seed;
        assert_eq!(pick(&rows, 2, 1, &mut rng), Some(rows[2][want]));
        let mut probe = seed;
        next_rand31(&mut probe);
        assert_eq!(rng, probe);
        seen[want] = true;
    }
    assert_eq!(seen, [true; 3]);
    // Rank bonus 1 pushes w0 to 92 for a Spellcaster.
    let mut rng = 0;
    let r = bucket(0);
    let want = if r < 92 {
        0
    } else if r < 153 {
        1
    } else {
        2
    };
    assert_eq!(pick(&rows, 0x12, 1, &mut rng), Some(rows[2][want]));
}

#[test]
fn destiny_draw_edits_the_refilled_hand_like_the_original_loop() {
    let cards = cards();
    let rows = rows();
    let (mut d, _) = Duel::new(setup(), &cards).unwrap();
    // hand + deck ≥ 5: the engine's fifth deck draw goes back, the special
    // card takes its place; the Drew event lists the hand's cards.
    d.sides[0].deck = vec![10, 11, 12, 13, 14, 15];
    d.sides[0].hand = vec![10, 11, 12, 13, 14];
    d.sides[0].deck.drain(..5);
    d.script_set_destiny(0, 2);
    let mut events = vec![DuelEvent::Drew {
        side: 0,
        cards: vec![10, 11, 12, 13, 14],
    }];
    let mut rng = 0;
    let draw = d
        .script_destiny_draw(0, (0, 6), &rows, &cards, &mut rng, &mut events)
        .unwrap()
        .unwrap();
    assert!(rows[2].contains(&draw.card));
    assert_eq!(d.sides[0].hand, vec![10, 11, 12, 13, draw.card]);
    assert_eq!(d.sides[0].deck, vec![14, 15]);
    assert_eq!(d.sides[0].destiny, 2 | DESTINY_USED);
    assert_eq!(
        events,
        vec![DuelEvent::Drew {
            side: 0,
            cards: vec![10, 11, 12, 13, draw.card]
        }]
    );
    assert!(!d.script_destiny_pending(0));
    // hand + deck < 5: every deck card is drawn and the special card is
    // added (target + 1).
    let (mut d, _) = Duel::new(setup(), &cards).unwrap();
    d.sides[0].deck = vec![];
    d.sides[0].hand = vec![10, 11, 12];
    d.script_set_destiny(0, 3);
    let mut events = Vec::new();
    let draw = d
        .script_destiny_draw(0, (0, 3), &rows, &cards, &mut rng, &mut events)
        .unwrap()
        .unwrap();
    assert!(rows[3].contains(&draw.card));
    assert_eq!(d.sides[0].hand, vec![10, 11, 12, draw.card]);
    assert!(d.sides[0].deck.is_empty());
    // A failed pick with hand + deck < 5 is refused, with ≥ 5 a no-op.
    let mut immortal = cards.clone();
    immortal[60].kind = 21;
    let (mut d, _) = Duel::new(setup(), &immortal).unwrap();
    d.script_set_destiny(0, 2);
    let err = d
        .script_destiny_draw(0, (0, 3), &rows, &immortal, &mut rng, &mut events)
        .unwrap_err();
    assert!(matches!(err, DuelError::Unsupported(_)), "{err}");
    assert_eq!(
        d.script_destiny_draw(0, (0, 40), &rows, &immortal, &mut rng, &mut events)
            .unwrap(),
        None
    );
    assert_eq!(d.sides[0].destiny, 2);
    // Pending needs a deck, an armed byte and room in the hand.
    let (mut d, _) = Duel::new(setup(), &cards).unwrap();
    assert!(!d.script_destiny_pending(0));
    d.script_set_destiny(0, 2);
    assert!(d.script_destiny_pending(0));
    d.sides[0].hand = vec![1, 2, 3, 4, 5];
    assert!(!d.script_destiny_pending(0));
    d.sides[0].hand.clear();
    d.sides[0].deck.clear();
    assert!(!d.script_destiny_pending(0));
}

#[test]
fn destiny_draw_without_the_rows_is_refused_at_the_summon() {
    let cards = cards();
    let (duel, _) = Duel::new(setup(), &cards).unwrap();
    let mut s = DuelSession::tutorial(duel, Tutorial::series(Lesson::Rules));
    let fusions = NoFusions;
    let mut op = Operator { checks: 0 };
    let patches = patches();
    let mut refused = None;
    for _ in 0..8000 {
        let pad = op.pad(&s);
        if let Err(e) = s.frame(pad, &cards, &fusions) {
            let step = s.tutorial.as_ref().unwrap().step;
            if let Some(p) = patches.iter().find(|p| p.step == step) {
                let t = s.tutorial.as_mut().unwrap();
                t.step = step + 1;
                t.counter = 0;
                (p.apply)(&mut s);
                continue;
            }
            refused = Some((step, e));
            break;
        }
    }
    let (step, err) = refused.unwrap();
    assert_eq!(step, 248);
    assert!(
        matches!(&err, DuelError::Unsupported(m) if m.contains("0x2B7AF0")),
        "{err}"
    );
    assert_eq!(s.input.state(), 0x3E);
    assert_eq!(s.duel.sides[0].destiny, 2);
}

/// Private checks against the original: `ROSES_TEST_CATALOGUE` = the
/// imported `catalogue.json`, `ROSES_TEST_ELF_PATH` = the PAL executable
/// (deck records 0x15/0x16 from its static deck table, the Destiny rows
/// from 0x2B7AF0), `ROSES_TEST_TUTORIAL` = the `claude-tutorial` folder
/// (`emu-34.txt`, `states-34.txt`).
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

    /// The Destiny rows (EE 0x2B7AF0, file 0x1BE470), as
    /// `roses-assets::parse_pal_destiny_rows` reads them.
    fn destiny_rows(elf: &[u8]) -> DestinyRows {
        let mut rows = [[0u16; 3]; 8];
        for (r, row) in rows.iter_mut().enumerate() {
            for (i, id) in row.iter_mut().enumerate() {
                let at = 0x1BE470 + 6 * r + 2 * i;
                *id = u16::from_le_bytes([elf[at], elf[at + 1]]);
            }
        }
        rows
    }

    /// A unit of a saved state: side, slot, card, column, row, +0x57, +0x49,
    /// +0x58.
    type ObservedUnit = (u8, u8, u16, u8, u8, u8, u8, u8);

    /// One line of `states-34.txt`: the saved state's step, LP, D53/D54/D57,
    /// side 0's +0x344/+0x346, deck counts, the terrain block and the units.
    struct Observed {
        name: String,
        step: u16,
        lp: [u16; 2],
        round: u8,
        active: u8,
        d57: u8,
        destiny: u8,
        can_summon: bool,
        deck: [usize; 2],
        hand: Vec<u16>,
        terr: Vec<u8>,
        units: Vec<ObservedUnit>,
    }

    fn between<'a>(line: &'a str, start: &str, end: &str) -> &'a str {
        let from = line.find(start).unwrap() + start.len();
        let to = line[from..].find(end).unwrap() + from;
        &line[from..to]
    }

    fn observed(line: &str) -> Observed {
        let name = line.split_whitespace().next().unwrap().to_string();
        let step = between(line, " step ", " ctr ").parse().unwrap();
        let lp: Vec<u16> = between(line, " lp [", "]")
            .split(", ")
            .map(|v| v.parse().unwrap())
            .collect();
        let d: Vec<&str> = between(line, " D53/54/57 ", " res/344/346 ")
            .split_whitespace()
            .collect();
        let res: Vec<u8> = between(line, " res/344/346 [", "] deck [")
            .replace(['(', ')'], "")
            .split(", ")
            .map(|v| v.parse().unwrap())
            .collect();
        let deck: Vec<usize> = between(line, "] deck [", "] hand [")
            .split(", ")
            .map(|v| v.parse().unwrap())
            .collect();
        let hand: Vec<u16> = between(line, "] hand [", "] terr ")
            .split(", ")
            .filter(|v| !v.is_empty())
            .map(|v| v.parse().unwrap())
            .collect();
        let terr: Vec<u8> = between(line, " terr ", " units [")
            .bytes()
            .map(|b| b - b'0')
            .collect();
        let units_text = between(line, " units [", "]\n").trim_end_matches(']');
        let units = units_text
            .split("), (")
            .filter(|u| !u.is_empty())
            .map(|u| {
                let f: Vec<&str> = u
                    .trim_matches(|c| c == '(' || c == ')')
                    .split(", ")
                    .collect();
                let n = |i: usize| f[i].parse::<u16>().unwrap();
                let flags =
                    u8::from_str_radix(f[5].trim_matches('\'').trim_start_matches("0x"), 16)
                        .unwrap();
                (
                    n(0) as u8,
                    n(1) as u8,
                    n(2),
                    n(3) as u8,
                    n(4) as u8,
                    flags,
                    n(6) as u8,
                    n(7) as u8,
                )
            })
            .collect();
        Observed {
            name,
            step,
            lp: [lp[0], lp[1]],
            round: d[0].parse().unwrap(),
            active: d[1].parse().unwrap(),
            d57: u8::from_str_radix(d[2].trim_start_matches("0x"), 16).unwrap(),
            destiny: res[1],
            can_summon: res[2] & 1 == 1,
            deck: [deck[0], deck[1]],
            hand,
            terr,
            units,
        }
    }

    /// +0x57 as the engine keeps it: bit0 acted, bit2 face-up, bit3
    /// defence, bit4 committed face-up, bit5 committed defence.
    fn flags(u: &crate::duel::DuelUnit) -> u8 {
        u8::from(u.acted)
            | u8::from(u.face_up) << 2
            | u8::from(u.defence) << 3
            | u8::from(u.committed_face_up) << 4
            | u8::from(u.committed_defence) << 5
    }

    #[test]
    #[ignore = "requires the private catalogue, executable and saved states"]
    fn private_lesson_three_matches_the_original_states_and_run() {
        let cards = catalogue();
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let r = [record(&elf, 0x15), record(&elf, 0x16)];
        assert_eq!((r[0].leader_card, r[1].leader_card), (60, 21));
        assert_eq!(&r[0].cards[..5], &DECK_RESET[0]);
        assert_eq!(&r[1].cards[..5], &DECK_RESET[1]);
        let rows = destiny_rows(&elf);
        assert_eq!(rows[2], [700, 699, 429]);
        let setup = lesson_setup(Lesson::Rules, [&r[0], &r[1]], [vec![], vec![]]).unwrap();
        let (duel, _) = Duel::new(setup, &cards).unwrap();
        let session = DuelSession::tutorial(
            duel,
            Tutorial::series(Lesson::Rules).with_destiny_rows(rows),
        );
        let run = run_lesson(session, &cards, &patches());
        // Keys and poll counts of the interpreter run.
        let log = std::fs::read_to_string(env("ROSES_TEST_TUTORIAL").join("emu-34.txt")).unwrap();
        let emu: Vec<Transition> = log
            .lines()
            .filter_map(|l| {
                // "poll %4d step %3d->%3d ret 0x%x ..." (the steps touch
                // the arrow from 100 on).
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
            .collect();
        assert_eq!(emu, EMU_34.to_vec());
        let t = run.session.tutorial.as_ref().unwrap();
        assert_eq!(
            (t.step, t.ended),
            (288, Some(LessonEnd::SeriesEnd { quit: false })),
            "refused {:?}, last entered {:?}",
            run.refused,
            run.entered.last().map(|e| (e.0, e.2, e.3))
        );
        assert_eq!(relative(&run.transitions), emu);
        // Only the known refusals, or fewer as the engine grows.
        assert!(
            run.refused_steps().iter().all(|s| REFUSED.contains(s)),
            "refused {:?}",
            run.refused
        );
        // Every saved state in a polled sub-state (0x18, 0x1D, 0x21, 0x3E,
        // 0x43): units with flags, spellbind and its fresh bit, LP, round,
        // active side, sub-state, side 0's Destiny byte and summon flag,
        // deck counts and the terrain block, right after the step the state
        // stands in was entered. Leaders compare their acted bit only (the
        // engine marks them face-up and committed, the original does not).
        // The hand buffer of the summaries keeps stale entries, so the hand
        // is compared only after the dust (step 16) and after the Destiny
        // Draw (from step 249: four deck cards and one of row 2).
        let text =
            std::fs::read_to_string(env("ROSES_TEST_TUTORIAL").join("states-34.txt")).unwrap();
        let mut compared = 0;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let o = observed(&format!("{line}\n"));
            // States 189–414 are the lesson (186 is lesson 2's last, 415 the
            // story); only the polled sub-states are compared.
            let number: u16 = o.name.split('-').next().unwrap().parse().unwrap();
            if !(189..=414).contains(&number) || !matches!(o.d57, 0x18 | 0x1D | 0x21 | 0x3E | 0x43)
            {
                continue;
            }
            let (_, d, state, _) = run.entry(o.step);
            let mask = if o.d57 == 0x1D { 0x39 } else { 0x3D };
            let mut engine: Vec<_> = d
                .units
                .iter()
                .map(|u| {
                    let m = if u.slot == 0 { 0x01 } else { mask };
                    (
                        u.side,
                        u.slot,
                        u.card,
                        u.position.column,
                        u.position.row,
                        flags(u) & m,
                        u.spellbind,
                        u.spellbind_fresh,
                    )
                })
                .collect();
            engine.sort_unstable();
            let mut seen: Vec<_> = o
                .units
                .iter()
                .map(|&(side, slot, card, c, r, f, bind, b58)| {
                    let m = if slot == 0 { 0x01 } else { mask };
                    (side, slot, card, c, r, f & m, bind, b58 & 8 != 0)
                })
                .collect();
            seen.sort_unstable();
            assert_eq!(
                engine, seen,
                "{} units after entering step {}",
                o.name, o.step
            );
            assert_eq!(lp(d), o.lp, "{} LP", o.name);
            assert_eq!(
                (d.round, d.active),
                (o.round, o.active),
                "{} D53/D54",
                o.name
            );
            assert_eq!(*state, o.d57, "{} D57", o.name);
            assert_eq!(d.sides[0].destiny, o.destiny, "{} +0x344", o.name);
            assert_eq!(d.sides[0].can_summon, o.can_summon, "{} +0x346", o.name);
            assert_eq!(
                [d.sides[0].deck.len(), d.sides[1].deck.len()],
                o.deck,
                "{} decks",
                o.name
            );
            assert_eq!(block(d), o.terr, "{} terrain", o.name);
            if o.step == 16 {
                assert_eq!(d.sides[0].hand, o.hand, "{} hand", o.name);
            }
            if o.step >= 249 {
                assert_eq!(&d.sides[0].hand[..4], &o.hand[..4], "{} hand", o.name);
                assert!(
                    rows[2].contains(&d.sides[0].hand[4]),
                    "{} special card",
                    o.name
                );
            }
            assert!(d.outcome.is_none(), "{}", o.name);
            compared += 1;
        }
        assert!(compared > 150, "{compared} states compared");
        assert_eq!(
            run.session.tutorial.as_ref().unwrap().ended,
            Some(LessonEnd::SeriesEnd { quit: false })
        );
    }
}
