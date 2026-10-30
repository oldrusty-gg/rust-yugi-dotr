//! Lessons 4 and 5 (controllers 35/36) and the new input views they drive:
//! the tables, the duel init, the hand view, the graveyard view, the
//! settings panel, the L2 toggle and the details pages; a full synthetic
//! run of lesson 5; and (private) both lessons against the original runs
//! from the map's tutorial menu (`modes/tutorial45/runs/l4.jsonl`,
//! `l5.jsonl`: the board at every step change).

use super::*;
use crate::duel::ai::weevil::FusionData;
use crate::duel::session::DuelSession;
use crate::duel::ui::{Mode, Response, SettingsPanel};
use crate::duel::{CardInfo, Catalogue};
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

/// Synthetic catalogue (invented numbers): every id a level-1 monster, the
/// leaders and the Celtic Guardian (level 4) as lesson 5 summons it.
fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..800).map(|i| monster(i, 1, 1, 0, 100, 100)).collect();
    v[0] = monster(0, 0, 8, 0, 3000, 2500);
    v[60] = monster(60, 1, 7, 1, 2500, 2100);
    v[156] = monster(156, 3, 4, 3, 1400, 1200);
    v
}

/// Deck records 0x19/0x1A with the first five cards of side 0 as the
/// original's lesson-5 hand shows them [emu]; the rest filler.
fn records(lesson: Lesson) -> [DeckInput; 2] {
    let (a, b) = lesson.deck_pair();
    let mut first: Vec<u16> = vec![321, 285, 156, 249, 505];
    first.extend((0..35).map(|i| 5 + i % 3));
    let second: Vec<u16> = (0..40).map(|i| 21 + i % 3).collect();
    [
        DeckInput {
            index: a,
            leader_card: 60,
            leader_rank_raw: 1,
            cards: first,
        },
        DeckInput {
            index: b,
            leader_card: 0,
            leader_rank_raw: 1,
            cards: second,
        },
    ]
}

fn session(lesson: Lesson, cards: &impl CardData) -> DuelSession {
    let r = records(lesson);
    let setup = lesson_setup(lesson, [&r[0], &r[1]], [vec![], vec![]]).unwrap();
    let (duel, _) = Duel::new(setup, cards).unwrap();
    DuelSession::tutorial(duel, Tutorial::single(lesson, true))
}

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

/// Cross on every fifth poll that reads the pad while a page is shown.
#[derive(Default)]
struct Operator {
    checks: u32,
}

impl Operator {
    fn pad(&mut self, s: &DuelSession) -> u16 {
        if s.tutorial.as_ref().unwrap().message_accepts_input() {
            self.checks += 1;
            if self.checks.is_multiple_of(5) {
                return pad::CROSS;
            }
        }
        0
    }
}

#[test]
fn menu_rows_pick_the_lessons_and_their_labels() {
    // 0x23B3B0: rows 0..3 → controls 32..35, anything else → 36 [F].
    let picked: Vec<u8> = (0..7)
        .map(|row| Lesson::from_menu_row(row).controller())
        .collect();
    assert_eq!(picked, vec![32, 33, 34, 35, 36, 36, 36]);
    assert_eq!(Lesson::Technique.deck_pair(), (0x17, 0x18));
    assert_eq!(Lesson::Functions.deck_pair(), (0x19, 0x1A));
    assert_eq!(Lesson::Technique.step_count(), 322);
    assert_eq!(Lesson::Functions.step_count(), 69);
    let labels: Vec<u16> = Lesson::ALL.iter().map(|l| l.menu_label()).collect();
    assert_eq!(labels, vec![4087, 4088, 4089, 4090, 4091]);
    assert_eq!(Lesson::Technique.next(), None);
    assert_eq!(Lesson::from_controller(36), Some(Lesson::Functions));
}

#[test]
fn single_lessons_end_by_popping_with_counter_zero() {
    let t = Tutorial::single(Lesson::Technique, true);
    assert_eq!(t.series_counter, 0);
    assert_eq!(Tutorial::series(Lesson::Functions).series_counter, 0);
    let mut t = Tutorial::single(Lesson::Basics, false);
    assert_eq!(t.end(-1), LessonEnd::SeriesEnd { quit: false });
}

#[test]
fn forced_hands_follow_the_table() {
    assert_eq!(lesson4::forced_hand(0x25), Some([321, 7, 156, 249, 294]));
    assert_eq!(lesson4::forced_hand(0x48), Some([15, 5, 156, 249, 294]));
    assert_eq!(lesson4::forced_hand(0x5D), lesson4::forced_hand(0xAC));
    assert_eq!(lesson4::forced_hand(0x87), Some([321, 7, 156, 766, 766]));
    assert_eq!(lesson4::forced_hand(0x98), Some([15, 5, 294, 766, 766]));
    assert_eq!(lesson4::forced_hand(0x26), None);
}

#[test]
fn lesson_inits_set_stars_and_graveyards() {
    let cards = Catalogue::new(cards(), []);
    let s = session(Lesson::Technique, &cards);
    assert_eq!(s.duel.sides[0].stars, 8);
    let s = session(Lesson::Functions, &cards);
    assert_eq!(s.duel.sides[0].graveyard, vec![0, 3, 6, 700]);
    assert_eq!(s.duel.sides[1].graveyard, vec![0, 3, 6, 699]);
}

/// Lesson 5 end to end on synthetic cards: the summon, the hand view, the
/// graveyard view with both sides and the details, the settings panel
/// (rows 0/1, cancelled), L2 twice, the sticks, then −1 → the map.
#[test]
fn lesson_five_runs_to_its_end() {
    let cards = Catalogue::new(cards(), []);
    let mut s = session(Lesson::Functions, &cards);
    let mut op = Operator::default();
    let mut states: Vec<(u16, u8)> = Vec::new();
    let mut sticks = Vec::new();
    let mut hidden = Vec::new();
    let mut end = None;
    for _ in 0..20_000 {
        let pad_word = op.pad(&s);
        let f = s.frame(pad_word, &cards, &NoFusions).unwrap();
        let t = s.tutorial.as_ref().unwrap();
        let state = (t.step, s.input.state());
        if states.last() != Some(&state) {
            states.push(state);
        }
        if t.analog != [0x80; 4] {
            sticks.push((t.step, t.analog));
        }
        hidden.push((t.step, s.input.hud_hidden));
        if let Some(e) = f.script.and_then(|x| x.ended) {
            end = Some(e);
            break;
        }
    }
    assert_eq!(end, Some(LessonEnd::SeriesEnd { quit: false }));
    let has = |step: u16, state: u8| states.contains(&(step, state));
    // Summon (0x43) and the Celtic Guardian at (3,5); the hand view after
    // Down + Square; closed by step 13's Circle.
    assert!(has(8, 0x43));
    assert!(has(13, 0x53));
    assert!(has(14, 0x18));
    // R1: the graveyard view (0x2D), its details (0x21), closed.
    assert!(has(19, 0x2D) && has(27, 0x21) && has(28, 0x2D) && has(29, 0x18));
    // L1: the settings panel through its two rows; cancelled.
    assert!(has(34, 0x29) && has(43, 0x18));
    assert_eq!(s.input.options, crate::duel::ui::DEFAULT_OPTIONS);
    // L2 hides and shows the info windows (FLAG906).
    assert!(hidden.contains(&(48, true)) && hidden.contains(&(50, false)));
    // The sticks: left X 0 for 8 polls, then 0xFF for 7 (13th..19th).
    let left_x: Vec<_> = sticks.iter().filter(|(_, a)| a[2] != 0x80).collect();
    assert_eq!(left_x.iter().filter(|(_, a)| a[2] == 0).count(), 8);
    assert_eq!(left_x.iter().filter(|(_, a)| a[2] == 0xFF).count(), 7);
    assert!(s.duel.units.iter().all(|u| u.slot == 0));
}

#[test]
fn input_views_follow_the_traced_handlers() {
    let cards = Catalogue::new(cards(), []);
    let mut s = session(Lesson::Functions, &cards);
    let duel = s.duel.clone();
    let ui = &mut s.input;
    // R1: view-only lists of both graveyards, side 0 shown; Right/Left
    // switch; Cross does nothing; Triangle shows the entry's details.
    assert_eq!(ui.press(&duel, &cards, pad::R1).unwrap(), Response::Moved);
    assert_eq!(ui.state(), 0x2D);
    assert_eq!(ui.press(&duel, &cards, pad::CROSS).unwrap(), Response::Ignored);
    assert_eq!(ui.press(&duel, &cards, pad::RIGHT).unwrap(), Response::Moved);
    assert!(matches!(&ui.mode, Mode::Graveyard(p) if p.shown == 1 && p.lists[1].len() == 4));
    assert_eq!(ui.press(&duel, &cards, pad::TRIANGLE).unwrap(), Response::Moved);
    assert_eq!(ui.state(), 0x21);
    assert_eq!(ui.details.unwrap().card, 0);
    // Pages wrap mod 4.
    for _ in 0..4 {
        ui.press(&duel, &cards, pad::DOWN).unwrap();
    }
    assert_eq!(ui.details.unwrap().page, 0);
    ui.press(&duel, &cards, pad::UP).unwrap();
    assert_eq!(ui.details.unwrap().page, 3);
    ui.press(&duel, &cards, pad::CIRCLE).unwrap();
    assert_eq!(ui.state(), 0x2D);
    ui.press(&duel, &cards, pad::CIRCLE).unwrap();
    assert_eq!(ui.state(), 0x18);
    // L1: rows 0/1 edit option bits 1/2; Cross applies, Circle discards.
    ui.press(&duel, &cards, pad::L1).unwrap();
    assert_eq!(ui.settings, Some(SettingsPanel { row: 0, bits: 0b111 }));
    ui.press(&duel, &cards, pad::RIGHT).unwrap();
    ui.press(&duel, &cards, pad::DOWN).unwrap();
    ui.press(&duel, &cards, pad::RIGHT).unwrap();
    assert_eq!(ui.settings, Some(SettingsPanel { row: 1, bits: 0b001 }));
    ui.press(&duel, &cards, pad::CIRCLE).unwrap();
    assert_eq!(ui.options, 0b111);
    ui.press(&duel, &cards, pad::L1).unwrap();
    ui.press(&duel, &cards, pad::RIGHT).unwrap();
    ui.press(&duel, &cards, pad::CROSS).unwrap();
    assert_eq!((ui.options, ui.state()), (0b101, 0x18));
    // L2 toggles the display flag.
    ui.press(&duel, &cards, pad::L2).unwrap();
    assert!(ui.hud_hidden);
    // L3 + R3 are the camera's.
    assert_eq!(
        ui.press(&duel, &cards, pad::L3 | pad::R3).unwrap(),
        Response::Ignored
    );
}

/// Private checks against the original: `ROSES_TEST_CATALOGUE` = the
/// imported `catalogue.json`, `ROSES_TEST_ELF_PATH` = the PAL executable
/// (deck records 0x17..0x1A from its static deck table),
/// `ROSES_TEST_TUTORIAL45` = `modes/tutorial45/runs` (`l4.jsonl`,
/// `l5.jsonl`: the original's board at every step change of a run from
/// the map's tutorial menu, [emu]).
mod private {
    use super::*;
    use std::path::PathBuf;

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
    }

    fn catalogue() -> Catalogue {
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

    /// The comparable part of a board: units (side, slot, card, column,
    /// row), LP, stars, side 0's hand cards, round, active side.
    #[derive(Debug, PartialEq, Eq, Clone)]
    struct Board {
        units: Vec<(u8, u8, u16, u8, u8)>,
        lp: [u16; 2],
        stars: [u8; 2],
        hand: Vec<u16>,
        round: u8,
        active: u8,
        /// d57 (compared on settled message steps only).
        state: u8,
    }

    fn port_board(d: &Duel) -> Board {
        let mut units: Vec<_> = d
            .units
            .iter()
            .map(|u| (u.side, u.slot, u.card, u.position.column, u.position.row))
            .collect();
        units.sort_unstable();
        Board {
            units,
            lp: [d.sides[0].lp, d.sides[1].lp],
            stars: [d.sides[0].stars, d.sides[1].stars],
            hand: d.sides[0].hand.clone(),
            round: d.round,
            active: d.active,
            state: 0,
        }
    }

    fn original(line: &str) -> (u16, Board) {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let n = |x: &serde_json::Value| x.as_u64().unwrap();
        let mut units: Vec<_> = v["units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| {
                let u = u.as_array().unwrap();
                (
                    n(&u[0]) as u8,
                    n(&u[1]) as u8,
                    n(&u[2]) as u16,
                    n(&u[3]) as u8,
                    n(&u[4]) as u8,
                )
            })
            .collect();
        units.sort_unstable();
        let side = |i: usize| &v["sides"][i];
        let board = Board {
            units,
            lp: [n(&side(0)["lp"]) as u16, n(&side(1)["lp"]) as u16],
            stars: [n(&side(0)["stars"]) as u8, n(&side(1)["stars"]) as u8],
            hand: side(0)["hand"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| (n(c) & 0xFFF) as u16)
                .filter(|&c| c != 999)
                .collect(),
            round: n(&v["d"]["d53"]) as u8,
            active: n(&v["d"]["d54"]) as u8,
            state: n(&v["d"]["d57"]) as u8,
        };
        (n(&v["step"]) as u16, board)
    }

    /// Runs `lesson` on the real data and compares, for every step N that
    /// edits nothing, the port's board on the last frame of step N with
    /// the original's when step N + 1 began (the poll that left N runs in a
    /// polled sub-state, so the effects of N's predecessors have settled).
    /// Steps in `edits` (setups) and `confirms` (the summon confirmation,
    /// whose star cost and consumed field unit the original applies on the
    /// key itself, before the step change is seen) are skipped; the step
    /// after them compares the settled result. On message steps the port
    /// leaves without a key, the sub-state is compared too (the duel is
    /// settled while a page is open). Returns the mismatches, the refusal
    /// if any, the step reached and the number of steps compared.
    fn compare(
        lesson: Lesson,
        log: &str,
        edits: &[u16],
        confirms: &[u16],
    ) -> (Vec<String>, Option<String>, u16, usize) {
        let cards = catalogue();
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let (a, b) = lesson.deck_pair();
        let r = [record(&elf, a), record(&elf, b)];
        let setup = lesson_setup(lesson, [&r[0], &r[1]], [vec![], vec![]]).unwrap();
        let (duel, _) = Duel::new(setup, &cards).unwrap();
        let mut s = DuelSession::tutorial(duel, Tutorial::single(lesson, true));
        let text = std::fs::read_to_string(env("ROSES_TEST_TUTORIAL45").join(log)).unwrap();
        let observed: std::collections::BTreeMap<u16, Board> =
            text.lines().map(original).collect();
        let mut last: std::collections::BTreeMap<u16, Board> = Default::default();
        let mut settled: std::collections::BTreeSet<u16> = Default::default();
        let mut op = Operator::default();
        let mut refusal = None;
        for _ in 0..200_000 {
            let t = s.tutorial.as_ref().unwrap();
            let (step, in_message) = (t.step, t.message.is_some());
            if !last.contains_key(&step) && std::env::var_os("ROSES_PRINT_POLLS").is_some() {
                eprintln!("poll {} step {step}", t.polls);
            }
            let mut board = port_board(&s.duel);
            board.state = s.input.state();
            last.insert(step, board);
            let pad_word = op.pad(&s);
            match s.frame(pad_word, &cards, &NoFusions) {
                Ok(f) => {
                    let script = f.script.unwrap_or_default();
                    if in_message && script.step.1 != step && script.key == 0 {
                        settled.insert(step);
                    }
                    if script.ended.is_some() {
                        break;
                    }
                }
                Err(e) => {
                    refusal = Some(format!("step {step}: {e}"));
                    break;
                }
            }
        }
        let reached = s.tutorial.as_ref().unwrap().step;
        let mut out = Vec::new();
        let mut compared = 0;
        for (&step, port) in &last {
            if edits.contains(&step) || confirms.contains(&step) {
                continue;
            }
            if let Some(orig) = observed.get(&(step + 1)) {
                compared += 1;
                let mut orig = orig.clone();
                let mut port = port.clone();
                if !settled.contains(&step) {
                    (orig.state, port.state) = (0, 0);
                }
                if orig != port {
                    out.push(format!("step {step}:\n  port {port:?}\n  orig {orig:?}"));
                }
            }
        }
        (out, refusal, reached, compared)
    }

    #[test]
    #[ignore = "requires the private catalogue, executable and original runs"]
    fn private_lesson_four_matches_the_original_run() {
        let edits = [
            0, 37, 59, 72, 93, 113, 135, 152, 172, 180, 200, 208, 242, 256, 273, 284, 300, 307,
        ];
        let confirms = [35, 55, 70, 91, 111, 133, 150, 170, 198];
        let (diffs, refusal, reached, compared) =
            compare(Lesson::Technique, "l4.jsonl", &edits, &confirms);
        for d in &diffs {
            eprintln!("{d}");
        }
        eprintln!("reached step {reached}, refusal {refusal:?}, {compared} steps compared");
        assert!(compared > 250);
        assert!(diffs.is_empty(), "{} mismatches", diffs.len());
        assert_eq!(refusal, None);
        assert_eq!(reached, 322);
    }

    #[test]
    #[ignore = "requires the private catalogue, executable and original runs"]
    fn private_lesson_five_matches_the_original_run() {
        let (diffs, refusal, reached, compared) =
            compare(Lesson::Functions, "l5.jsonl", &[0, 14], &[9]);
        for d in &diffs {
            eprintln!("{d}");
        }
        eprintln!("reached step {reached}, refusal {refusal:?}, {compared} steps compared");
        assert!(compared > 50);
        assert!(diffs.is_empty(), "{} mismatches", diffs.len());
        assert_eq!(refusal, None);
        assert_eq!(reached, 69);
    }
}
