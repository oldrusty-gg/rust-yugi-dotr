//! The Destiny Draw for every side (`duel/destiny.rs`,
//! claude-effects/SPECIAL-DRAW.md §2–4, next/ABILITY-3-CPU.md). Synthetic
//! cards; the rows are made-up ids (the real ones come from the executable).

use super::script::destiny::{pick, DestinyRows, DESTINY_USED};
use super::*;

const LEADER: u16 = 0; // Warrior
const INSECT_LEADER: u16 = 1;
const FIEND: u16 = 2; // 1000 ATK Fiend
const BIG: u16 = 3; // 3500 ATK Warrior
const PLAIN: u16 = 4; // 500 ATK Warrior
const WALKER: u16 = 5; // movement entry type 8
const TRAP: u16 = 6; // far trap
const R1: u16 = 10;
const R2: u16 = 11;
const R3: u16 = 12;

fn monster(id: u16, kind: u8, attack: u16) -> CardInfo {
    CardInfo {
        id,
        kind,
        level: 4,
        attribute: 3,
        attack,
        defense: attack,
        effects: [false; 5],
        toon: false,
        kind_index: 0,
        entries: [None; 5],
        equip_bits: [0; 7],
        power_up: None,
    }
}

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..=20).map(|i| monster(i, kind::WARRIOR, 100)).collect();
    v[usize::from(LEADER)] = monster(LEADER, kind::WARRIOR, 3000);
    v[usize::from(INSECT_LEADER)] = monster(INSECT_LEADER, kind::INSECT, 3000);
    v[usize::from(FIEND)] = monster(FIEND, kind::FIEND, 1000);
    v[usize::from(BIG)] = monster(BIG, kind::WARRIOR, 3500);
    v[usize::from(PLAIN)] = monster(PLAIN, kind::WARRIOR, 500);
    v[usize::from(WALKER)].entries[1] = Some([0x800, 0]);
    v[usize::from(TRAP)].kind = class::FAR_TRAP << 5;
    v
}

fn rows() -> DestinyRows {
    let mut r = [[999; 3]; 8];
    for row in r.iter_mut().skip(1) {
        *row = [R1, R2, R3];
    }
    r
}

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn duel(controllers: [u8; 2], leaders: [u16; 2], abilities: [Vec<u8>; 2]) -> Duel {
    let (mut d, _) = Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders,
            decks: [vec![PLAIN; 10], vec![PLAIN; 10]],
            first: 0,
            leader_abilities: abilities,
            controllers,
        },
        &cards(),
    )
    .unwrap();
    d.seed_rng(0x1234_5678_9ABC);
    d
}

fn put(d: &mut Duel, card: u16, side: u8, at: GridPos) -> UnitId {
    let id = d.create(side, card, at, &cards(), &mut vec![]).unwrap();
    let u = d.units.iter_mut().find(|u| u.id == id).unwrap();
    (u.face_up, u.committed_face_up) = (true, true);
    id
}

fn end_turn(d: &Duel) -> Duel {
    d.apply(d.active, &DuelCommand::EndTurn, &cards()).unwrap().0
}

#[test]
fn seto_arms_with_one_rand_and_the_rank_bonus() {
    let mut d = duel([PAD_CONTROLLER, 1], [LEADER; 2], [vec![], vec![]]);
    d.set_leader_ranks([0, 10]);
    d.sides[1].lp = 999;
    let mut rng = d.rng;
    let r = next_rand31(&mut rng);
    let d1 = end_turn(&d);
    assert_eq!(d1.sides[1].destiny, (6 + (r & 1) as u8) | 0x20);
    assert_eq!(d1.rng, rng, "exactly one rand31");
}

#[test]
fn other_cpu_modules_and_full_lp_never_arm_or_draw() {
    for (ctrl, lp) in [(2, 500), (3, 1), (9, 999), (1, 1000)] {
        let mut d = duel([PAD_CONTROLLER, ctrl], [LEADER; 2], [vec![], vec![0, 1, 3]]);
        d.sides[1].lp = lp;
        let d1 = end_turn(&d);
        assert_eq!((d1.sides[1].destiny, d1.rng), (0, d.rng), "controller {ctrl} LP {lp}");
    }
}

#[test]
fn the_armed_refill_ends_with_the_special_card() {
    let mut d = duel([PAD_CONTROLLER, 1], [LEADER; 2], [vec![], vec![]]);
    d.set_destiny_rows(rows());
    d.sides[1].lp = 500;
    let d1 = end_turn(&d);
    let byte = d1.sides[1].destiny;
    assert!(matches!(byte, 6 | 7));
    let mut rng = d1.rng;
    let expect = pick(&rows(), byte, kind::WARRIOR, &mut rng).unwrap();
    let (d2, events) = d1
        .apply(1, &DuelCommand::OpenSummon { cell: p(3, 1) }, &cards())
        .unwrap();
    // Event 0x72 first, then the refill: four deck cards and the special one.
    let at = events.iter().position(|e| matches!(e, DuelEvent::DestinyDraw { side: 1, card } if *card == expect));
    let drew = events.iter().position(|e| matches!(e, DuelEvent::Drew { side: 1, .. }));
    assert!(at.is_some() && at < drew);
    assert_eq!(d2.sides[1].hand, vec![PLAIN, PLAIN, PLAIN, PLAIN, expect]);
    assert_eq!(d2.sides[1].deck.len(), 6);
    assert_eq!(d2.sides[1].destiny, byte | DESTINY_USED);
    assert_eq!(d2.rng, rng);
    // Used: never re-armed, the next refill is plain.
    let mut d3 = d2.clone();
    d3.pending_summon = None;
    d3.sides[1].hand.clear();
    d3.sides[1].lp = 100;
    let d4 = end_turn(&end_turn(&d3));
    assert_eq!(d4.sides[1].destiny, byte | DESTINY_USED);
}

#[test]
fn a_short_deck_still_gets_the_special_card_on_top() {
    // target = min(hand + deck, 5) + 1: two deck cards and the special one.
    let mut d = duel([PAD_CONTROLLER, 1], [LEADER; 2], [vec![], vec![]]);
    d.set_destiny_rows(rows());
    d.sides[1].lp = 500;
    d.sides[1].deck.truncate(2);
    let d1 = end_turn(&d);
    let (d2, _) = d1
        .apply(1, &DuelCommand::OpenSummon { cell: p(3, 1) }, &cards())
        .unwrap();
    assert_eq!(d2.sides[1].hand.len(), 3);
    assert!(d2.sides[1].deck.is_empty());
    assert!([R1, R2, R3].contains(&d2.sides[1].hand[2]));
}

#[test]
fn without_rows_an_armed_summon_is_refused() {
    let mut d = duel([PAD_CONTROLLER, 1], [LEADER; 2], [vec![], vec![]]);
    d.sides[1].lp = 500;
    let d1 = end_turn(&d);
    assert!(matches!(
        d1.apply(1, &DuelCommand::OpenSummon { cell: p(3, 1) }, &cards()),
        Err(DuelError::Unsupported(_))
    ));
}

#[test]
fn a_human_needs_ability_3_and_a_situation() {
    // No #3: nothing; #3 on an empty board with an empty hand: situation 4
    // (no strongest: A = 0 > best = -1), no rand.
    let mut d = duel([PAD_CONTROLLER; 2], [LEADER; 2], [vec![], vec![]]);
    d.sides[1].lp = 500;
    d.sides[1].hand.clear();
    assert_eq!(end_turn(&d).sides[1].destiny, 0);
    let mut d = duel([PAD_CONTROLLER; 2], [LEADER; 2], [vec![], vec![3]]);
    d.sides[1].lp = 500;
    let d1 = end_turn(&d);
    assert_eq!((d1.sides[1].destiny, d1.rng), (4, d.rng));
}

fn human(leader: u16) -> Duel {
    let mut d = duel([PAD_CONTROLLER; 2], [LEADER, leader], [vec![], vec![3]]);
    d.sides[1].lp = 500;
    d
}

#[test]
fn the_classifier_follows_0x270900() {
    let c = cards();
    // 1: an opposing Fiend strongest within distance 3 of the leader (3,0),
    // the opponent controlling three Fiends.
    let mut d = human(LEADER);
    put(&mut d, FIEND, 0, p(3, 2));
    put(&mut d, FIEND, 0, p(0, 6));
    put(&mut d, FIEND, 0, p(1, 6));
    assert_eq!(d.destiny_situation(1, &c).unwrap(), Some(1));
    // 2: an Insect leader, three opposing monsters at ATK 3000..4000.
    let mut d = human(INSECT_LEADER);
    for col in [0, 1, 2] {
        put(&mut d, BIG, 0, p(col, 6));
    }
    assert_eq!(d.destiny_situation(1, &c).unwrap(), Some(2));
    // 3: a Labyrinth board (>= 31 walls), three opposing wall-walkers
    // (movement type 8..10), no own one; with one it falls through (to 4).
    let mut d = human(LEADER);
    for i in 0..31 {
        d.terrain[7 + i] = terrain::LABYRINTH;
    }
    for col in [0, 1] {
        put(&mut d, WALKER, 0, p(col, 6));
    }
    put(&mut d, PLAIN, 0, p(2, 6));
    assert_eq!(d.destiny_situation(1, &c).unwrap(), Some(4), "two walkers only");
    put(&mut d, WALKER, 0, p(4, 6));
    assert_eq!(d.destiny_situation(1, &c).unwrap(), Some(3));
    put(&mut d, WALKER, 1, p(6, 0));
    assert_eq!(d.destiny_situation(1, &c).unwrap(), Some(4));
    // 5: the strongest is ours and the opponent holds four traps.
    let mut d = human(LEADER);
    put(&mut d, BIG, 1, p(3, 1));
    for col in [0, 1, 2, 4] {
        put(&mut d, TRAP, 0, p(col, 5));
    }
    assert_eq!(d.destiny_situation(1, &c).unwrap(), Some(5));
    // 4 needs the hand to trail the opposing strongest by less than half the
    // opponent's LP; otherwise LP < 250 falls back to 6 / 7 with one rand.
    let mut d = human(LEADER);
    put(&mut d, PLAIN, 0, p(3, 5));
    d.sides[1].hand = vec![BIG];
    assert_eq!(d.destiny_situation(1, &c).unwrap(), None);
    d.sides[1].hand = vec![FIEND, 999];
    assert_eq!(d.destiny_situation(1, &c).unwrap(), None);
    d.sides[1].hand = vec![];
    assert_eq!(d.destiny_situation(1, &c).unwrap(), Some(4));
    d.sides[1].hand = vec![BIG];
    let d1 = end_turn(&d);
    assert_eq!(d1.sides[1].destiny, 0);
    d.sides[1].lp = 249;
    let mut rng = d.rng;
    let r = next_rand31(&mut rng);
    let d1 = end_turn(&d);
    assert_eq!((d1.sides[1].destiny, d1.rng), (6 + (r & 1) as u8, rng));
}
