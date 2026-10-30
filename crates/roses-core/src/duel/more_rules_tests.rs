//! Mask of Darkness 305 / Magician of Faith 43 (flip type 0x14), Dokurorider
//! 128 (entry-1 type 2), Call of the Haunted 745 (magic type 0x36) and
//! Fissure 726 (magic type 4 / hi 0, filter 0x20)
//! (claude-effects/MORE-RULES-305-128-745-726.md). Synthetic cards with the
//! originals' effect words; Fissure keeps its id (the builder dispatches on
//! it) and graveyard non-monsters use ids >= 0x2AB (the list filter 0x251920
//! reads the id).

use super::*;

const LEADER: u16 = 0;
const WEAK: u16 = 3; // 1000/1000
const MID: u16 = 4; // 1500/1200
const LOW_DEF: u16 = 5; // 1000/800
const ZERO: u16 = 6; // 0/500
const LORD: u16 = 7; // e2 0xB40: Dragon immunity source
const DRAGON: u16 = 8; // 900/900 Dragon
const FIVE: u16 = 9; // 500/300
const TINY: u16 = 10; // 100/100
const MAGICIAN_OF_FAITH: u16 = 43;
const DOKURORIDER: u16 = 128;
const MASK: u16 = 305;
const FISSURE: u16 = 726;
const RECOVERY: u16 = 744; // a magic card in a graveyard
const HAUNTED: u16 = 745;
const SET_TRAP: u16 = 750; // a trap card in a graveyard

fn monster(id: u16, kind: u8, attack: u16, defense: u16) -> CardInfo {
    CardInfo {
        id,
        kind,
        level: 4,
        attribute: 3,
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

fn with_entry(mut c: CardInfo, k: usize, word0: u32, word1: u32) -> CardInfo {
    c.entries[k] = Some([word0, word1]);
    c.effects[k] = true;
    c
}

fn other(id: u16, kind: u8, word0: u32, word1: u32) -> CardInfo {
    CardInfo {
        kind,
        ..with_entry(monster(id, 0, 0, 0), 0, word0, word1)
    }
}

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..=SET_TRAP)
        .map(|i| monster(i, kind::WARRIOR, 100, 100))
        .collect();
    v[usize::from(LEADER)] = monster(LEADER, kind::WARRIOR, 3000, 2500);
    v[usize::from(WEAK)] = monster(WEAK, kind::WARRIOR, 1000, 1000);
    v[usize::from(MID)] = monster(MID, kind::WARRIOR, 1500, 1200);
    v[usize::from(LOW_DEF)] = monster(LOW_DEF, kind::WARRIOR, 1000, 800);
    v[usize::from(ZERO)] = monster(ZERO, kind::WARRIOR, 0, 500);
    v[usize::from(LORD)] = with_entry(monster(LORD, kind::FIEND, 1200, 1100), 2, 0xB40, 0);
    v[usize::from(DRAGON)] = monster(DRAGON, kind::DRAGON, 900, 900);
    v[usize::from(FIVE)] = monster(FIVE, kind::WARRIOR, 500, 300);
    v[usize::from(TINY)] = monster(TINY, kind::WARRIOR, 100, 100);
    // The originals' entry words.
    v[usize::from(MASK)] = with_entry(monster(MASK, kind::FIEND, 300, 400), 3, 0x145E, 0);
    v[usize::from(MAGICIAN_OF_FAITH)] = with_entry(
        monster(MAGICIAN_OF_FAITH, kind::SPELLCASTER, 300, 400),
        3,
        0x14DE,
        0,
    );
    v[usize::from(DOKURORIDER)] =
        with_entry(monster(DOKURORIDER, kind::ZOMBIE, 1900, 1850), 1, 0x23D, 1);
    v[usize::from(FISSURE)] = other(FISSURE, class::MAGIC << 5, 0x4E0, 0);
    v[usize::from(HAUNTED)] = other(HAUNTED, class::MAGIC << 5, 0x365D, 2);
    v[usize::from(RECOVERY)] = other(RECOVERY, class::MAGIC << 5, 0x245D, 0);
    v[usize::from(SET_TRAP)] = other(SET_TRAP, class::FAR_TRAP << 5, 0x2EBD, 0);
    v
}

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn board() -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [LEADER, LEADER],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        &cards(),
    )
    .unwrap()
    .0
}

fn put(d: &mut Duel, card: u16, side: u8, at: GridPos, face_up: bool, defence: bool) -> UnitId {
    let id = d.create(side, card, at, &cards(), &mut vec![]).unwrap();
    let u = d.units.iter_mut().find(|u| u.id == id).unwrap();
    (
        u.face_up,
        u.committed_face_up,
        u.defence,
        u.committed_defence,
    ) = (face_up, face_up, defence, defence);
    id
}

fn mv(d: &Duel, unit: UnitId, path: Vec<GridPos>, face_up: bool) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(
        0,
        &DuelCommand::Move {
            unit,
            path,
            face_up,
            defence: None,
        },
        &cards(),
    )
}

fn pick(d: &Duel, side: u8, index: usize) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(
        0,
        &DuelCommand::Pick {
            pick: Pick::Graveyard { side, index },
        },
        &cards(),
    )
}

// ---- Mask of Darkness / Magician of Faith ----

#[test]
fn mask_of_darkness_manual_flip_creates_a_graveyard_non_monster_beside_the_leader() {
    // Role 0 on its controller's turn: 0x24FB40(0, 1) = the leader (3,6)'s
    // first neighbour (3,5); the pick lists ids >= 0x2AB of the own
    // graveyard only (s7 = ctrl, locked).
    let mut d = board();
    let mask = put(&mut d, MASK, 0, p(1, 4), false, false);
    d.sides[0].graveyard = vec![WEAK, SET_TRAP, RECOVERY];
    d.sides[1].graveyard = vec![SET_TRAP];
    let (d1, events) = mv(&d, mask, vec![], true).unwrap();
    assert_eq!(
        d1.pending,
        Some(Pending::GraveyardCopy {
            by: 0,
            at: p(3, 5),
            graveyards: vec![0],
            monsters: false,
            non_monsters: true,
        })
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::SelectionOpened { card } if *card == MASK)));
    // A monster cannot be taken; any other command waits for the answer.
    assert!(pick(&d1, 0, 0).is_err());
    assert!(d1.apply(0, &DuelCommand::EndTurn, &cards()).is_err());
    let (d2, events) = pick(&d1, 0, 1).unwrap();
    assert_eq!(d2.pending, None);
    let new = d2.unit_at(p(3, 5)).expect("the copy");
    assert_eq!((new.card, new.side, new.face_up), (SET_TRAP, 0, false));
    assert!((6..=10).contains(&new.slot));
    assert_eq!(d2.sides[0].graveyard, vec![WEAK, RECOVERY]);
    assert_eq!(d2.sides[1].graveyard, vec![SET_TRAP]);
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::Revived { card, .. } if *card == SET_TRAP)));
}

#[test]
fn mask_of_darkness_does_nothing_on_the_opponents_turn() {
    // The flipping unit's controller is not 0x2DB8F4: result 0.
    let mut d = board();
    let mask = put(&mut d, MASK, 1, p(3, 2), false, true);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    d.sides[1].graveyard = vec![SET_TRAP];
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert_eq!(d1.pending, None);
    assert!(d1.unit(mask).is_none(), "1500 beats DEF 400");
    assert_eq!(d1.sides[1].graveyard, vec![SET_TRAP, MASK]);
}

#[test]
fn mask_of_darkness_needs_a_non_monster_and_a_free_leader_neighbour() {
    // No non-monster in the graveyard: nothing.
    let mut d = board();
    let mask = put(&mut d, MASK, 0, p(1, 4), false, false);
    d.sides[0].graveyard = vec![WEAK];
    let (d1, _) = mv(&d, mask, vec![], true).unwrap();
    assert_eq!(d1.pending, None);
    // Every neighbour of the leader (3,6) taken (on the board): nothing.
    let mut d = board();
    let mask = put(&mut d, MASK, 0, p(0, 0), false, false);
    for (c, r) in [(2, 5), (3, 5), (4, 5), (2, 6), (4, 6)] {
        put(&mut d, TINY, 1, p(c, r), true, false);
    }
    d.sides[0].graveyard = vec![SET_TRAP];
    let (d1, _) = mv(&d, mask, vec![], true).unwrap();
    assert_eq!(d1.pending, None);
}

#[test]
fn mask_of_darkness_skips_a_labyrinth_neighbour() {
    // 0x24FB40 flag 1: terrain 8 never counts; (3,5) is Labyrinth, the
    // next neighbour in 0x24FD30 order for side 0 is (3,7) — off the board —
    // then (4,6).
    let mut d = board();
    let i = p(3, 5).index().unwrap();
    d.terrain[i] = terrain::LABYRINTH;
    let mask = put(&mut d, MASK, 0, p(1, 4), false, false);
    d.sides[0].graveyard = vec![SET_TRAP];
    let (d1, _) = mv(&d, mask, vec![], true).unwrap();
    assert!(matches!(
        d1.pending,
        Some(Pending::GraveyardCopy { at, .. }) if at == p(4, 6)
    ));
}

#[test]
fn mask_of_darkness_attacking_face_down_opens_the_pick_after_the_battle() {
    // Role 1 on the own turn: the battle goes on (c64 hi 2, then the event
    // and the rest of the battle); the engine opens the pick after it.
    let mut d = board();
    let mask = put(&mut d, MASK, 0, p(3, 3), false, false);
    let def = put(&mut d, TINY, 1, p(3, 2), true, false);
    d.sides[0].graveyard = vec![RECOVERY];
    let (d1, _) = mv(&d, mask, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(def).is_none());
    assert_eq!(d1.sides[1].lp, START_LP - 200);
    assert!(matches!(
        d1.pending,
        Some(Pending::GraveyardCopy {
            by: 0,
            non_monsters: true,
            ..
        })
    ));
    let (d2, _) = pick(&d1, 0, 0).unwrap();
    assert_eq!(d2.unit_at(p(3, 5)).unwrap().card, RECOVERY);
    assert!(d2.unit(mask).is_some());
}

#[test]
fn magician_of_faith_offers_both_graveyards() {
    // Mode 3: s7 = 2, both lists, the shown one unlocked; the copy is the
    // activator's.
    let mut d = board();
    let mof = put(&mut d, MAGICIAN_OF_FAITH, 0, p(1, 4), false, false);
    d.sides[1].graveyard = vec![WEAK, SET_TRAP];
    let (d1, _) = mv(&d, mof, vec![], true).unwrap();
    assert!(matches!(
        &d1.pending,
        Some(Pending::GraveyardCopy { by: 0, graveyards, non_monsters: true, .. }) if graveyards == &vec![0, 1]
    ));
    let (d2, _) = pick(&d1, 1, 1).unwrap();
    let new = d2.unit_at(p(3, 5)).unwrap();
    assert_eq!((new.card, new.side), (SET_TRAP, 0));
    assert_eq!(d2.sides[1].graveyard, vec![WEAK]);
}

// ---- Dokurorider ----

#[test]
fn dokurorider_turns_each_entered_cell_into_wasteland() {
    // 0x251498 hi 0, lo 1: radius 0 → the entered cell only; face-down too;
    // the walk itself is normal (0x247E10 ignores type 2).
    let mut d = board();
    let rider = put(&mut d, DOKURORIDER, 0, p(1, 4), false, false);
    let (d1, _) = mv(&d, rider, vec![p(1, 3)], false).unwrap();
    assert_eq!(d1.unit(rider).unwrap().position, p(1, 3));
    assert_eq!(d1.terrain_at(p(1, 3)), terrain::WASTELAND);
    assert_eq!(d1.terrain_at(p(1, 4)), terrain::NORMAL);
    assert_eq!(d1.terrain_at(p(1, 2)), terrain::NORMAL);
    assert_eq!(
        d1.terrain
            .iter()
            .filter(|&&t| t == terrain::WASTELAND)
            .count(),
        1
    );
}

#[test]
fn dokurorider_advancing_after_a_battle_changes_the_cell() {
    let mut d = board();
    let rider = put(&mut d, DOKURORIDER, 0, p(1, 4), true, false);
    let def = put(&mut d, TINY, 1, p(1, 3), true, false);
    let (d1, _) = mv(&d, rider, vec![p(1, 3)], false).unwrap();
    assert!(d1.unit(def).is_none());
    assert_eq!(d1.unit(rider).unwrap().position, p(1, 3));
    assert_eq!(d1.terrain_at(p(1, 3)), terrain::WASTELAND);
}

// ---- Call of the Haunted ----

#[test]
fn call_of_the_haunted_turns_the_own_monsters_into_zombies() {
    // Mode 1, filter 0x1D: every monster the activator controls in slots
    // 1..10, any face; +0x3E := 2. Leaders, enemies and non-monsters stay.
    let mut d = board();
    let card = put(&mut d, HAUNTED, 0, p(1, 4), false, false);
    let a = put(&mut d, WEAK, 0, p(2, 4), true, false);
    let b = put(&mut d, DRAGON, 0, p(5, 4), false, true);
    let e = put(&mut d, MID, 1, p(3, 1), true, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    let kind_of = |id| d1.unit_card(d1.unit(id).unwrap(), &cards()).kind;
    assert_eq!(kind_of(a), kind::ZOMBIE);
    assert_eq!(kind_of(b), kind::ZOMBIE);
    assert_eq!(kind_of(e), kind::WARRIOR);
    assert_eq!(d1.unit_card(d1.leader(0), &cards()).kind, kind::WARRIOR);
    assert!(d1.unit(card).is_none());
    assert_eq!(d1.sides[0].graveyard, vec![HAUNTED]);
}

#[test]
fn call_of_the_haunted_without_monsters_is_spent() {
    let mut d = board();
    let card = put(&mut d, HAUNTED, 0, p(1, 4), false, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    assert!(d1.records.is_empty());
    assert_eq!(d1.sides[0].graveyard, vec![HAUNTED]);
}

// ---- Fissure ----

#[test]
fn fissure_destroys_the_weakest_monster_of_either_side() {
    // Mode 3: the lowest ATK at its cell over both groups, any face.
    let mut d = board();
    let card = put(&mut d, FISSURE, 0, p(1, 4), false, false);
    let own = put(&mut d, MID, 0, p(2, 4), true, false);
    let enemy = put(&mut d, WEAK, 1, p(3, 1), false, true);
    let enemy2 = put(&mut d, MID, 1, p(4, 1), true, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    assert!(d1.unit(enemy).is_none());
    assert!(d1.unit(own).is_some() && d1.unit(enemy2).is_some());
    assert_eq!(d1.sides[1].graveyard, vec![WEAK]);
    assert_eq!(d1.sides[0].graveyard, vec![FISSURE]);
}

#[test]
fn fissure_ties_keep_the_activators_group_first() {
    // 0x24E230 lists the activator's group first; an equal ATK (best word
    // 1000 << 16 | DEF never equals 1000) keeps the earlier unit even with a
    // lower DEF later.
    let mut d = board();
    let card = put(&mut d, FISSURE, 1, p(1, 2), false, false);
    let own = put(&mut d, WEAK, 1, p(2, 2), true, false);
    let enemy = put(&mut d, LOW_DEF, 0, p(3, 4), true, false);
    d.active = 1;
    let (d1, _) = d
        .apply(
            1,
            &DuelCommand::Move {
                unit: card,
                path: vec![],
                face_up: true,
                defence: None,
            },
            &cards(),
        )
        .unwrap();
    assert!(d1.unit(own).is_none());
    assert!(d1.unit(enemy).is_some());
}

#[test]
fn fissure_follows_the_whole_word_compare_after_an_atk_0_best() {
    // 0x24F3C0 compares the best word (0 << 16 | 500) with the candidate's
    // ATK 500: equal, so the DEF test runs and DEF 300 < 500 replaces the
    // 0-ATK monster by the 500-ATK one.
    let mut d = board();
    let card = put(&mut d, FISSURE, 0, p(1, 4), false, false);
    let zero = put(&mut d, ZERO, 0, p(2, 4), true, false);
    let five = put(&mut d, FIVE, 1, p(3, 1), true, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    assert!(d1.unit(zero).is_some());
    assert!(d1.unit(five).is_none());
}

#[test]
fn fissure_on_a_protected_dragon_does_nothing() {
    // The pick comes first, then 0x273C60: an enemy Dragon under Lord of
    // D. leaves the mask empty; the runner-up is not taken.
    let mut d = board();
    let card = put(&mut d, FISSURE, 0, p(1, 4), false, false);
    let dragon = put(&mut d, DRAGON, 1, p(3, 1), true, false);
    let lord = put(&mut d, LORD, 1, p(4, 1), true, true);
    let own = put(&mut d, WEAK, 0, p(2, 4), true, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    assert!(d1.unit(dragon).is_some());
    assert!(d1.unit(lord).is_some());
    assert!(d1.unit(own).is_some());
    assert_eq!(d1.sides[0].graveyard, vec![FISSURE]);
}
