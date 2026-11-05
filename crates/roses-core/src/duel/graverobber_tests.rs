//! Graverobber 688 (magic type 0x14, mode 3, filter 0x1E; builder arm
//! 0x254A18 → 0x254B4C) (the project notes).
//! Synthetic cards with the originals' effect words; graveyard
//! non-monsters use ids >= 0x2AB (the list filter 0x251920 reads the id).

use super::*;

const LEADER: u16 = 0;
const WARRIOR: u16 = 3;
const COPYCAT: u16 = 686; // 0x149E: mode 2 (the opponent's graveyard), filter 0x1E
const GRAVEROBBER: u16 = 688; // 0x14DE: mode 3, filter 0x1E
const RECOVERY: u16 = 744; // a magic card in a graveyard
const SET_TRAP: u16 = 750; // a trap card in a graveyard

fn monster(id: u16, attack: u16, defense: u16) -> CardInfo {
    CardInfo {
        id,
        kind: kind::WARRIOR,
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

fn other(id: u16, kind: u8, word0: u32, word1: u32) -> CardInfo {
    let mut c = monster(id, 0, 0);
    c.kind = kind;
    c.entries[0] = Some([word0, word1]);
    c.effects[0] = true;
    c
}

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..=SET_TRAP).map(|i| monster(i, 100, 100)).collect();
    v[usize::from(LEADER)] = monster(LEADER, 3000, 2500);
    v[usize::from(WARRIOR)] = monster(WARRIOR, 1000, 1000);
    v[usize::from(COPYCAT)] = other(COPYCAT, class::MAGIC << 5, 0x149E, 0);
    v[usize::from(GRAVEROBBER)] = other(GRAVEROBBER, class::MAGIC << 5, 0x14DE, 0);
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

fn put(d: &mut Duel, card: u16, side: u8, at: GridPos) -> UnitId {
    d.create(side, card, at, &cards(), &mut vec![]).unwrap()
}

/// Reveal the set magic card where it stands (state 0x38 → 0x253D60).
fn reveal(d: &Duel, unit: UnitId) -> (Duel, Vec<DuelEvent>) {
    d.apply(
        0,
        &DuelCommand::Move {
            unit,
            path: vec![],
            face_up: true,
            defence: None,
        },
        &cards(),
    )
    .unwrap()
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

#[test]
fn graverobber_lists_the_non_monsters_of_both_graveyards() {
    // emu G1: {1, ctrl | 0x20000, card cell, 999}, state 0x2D over both
    // lists; the pick is created face-down on the Graverobber's own cell.
    let mut d = board();
    let card = put(&mut d, GRAVEROBBER, 0, p(3, 4));
    d.sides[0].graveyard = vec![WARRIOR, RECOVERY];
    d.sides[1].graveyard = vec![SET_TRAP, WARRIOR];
    let (d1, events) = reveal(&d, card);
    assert_eq!(
        d1.pending,
        Some(Pending::GraveyardCopy {
            by: 0,
            at: p(3, 4),
            graveyards: vec![0, 1],
            monsters: false,
            non_monsters: true,
        })
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::SelectionOpened { card } if *card == GRAVEROBBER)));
    assert_eq!(d1.sides[0].graveyard, vec![WARRIOR, RECOVERY, GRAVEROBBER]);
    // Monsters cannot be taken.
    assert!(pick(&d1, 0, 0).is_err());
    assert!(pick(&d1, 1, 1).is_err());
    // Own magic card: slot 6..10, face-down, acted (0x2589CC).
    let (d2, events) = pick(&d1, 0, 1).unwrap();
    assert_eq!(d2.pending, None);
    let new = d2.unit_at(p(3, 4)).expect("the copy");
    assert_eq!((new.card, new.side, new.face_up), (RECOVERY, 0, false));
    assert!(new.acted && (6..=10).contains(&new.slot));
    assert_eq!(d2.sides[0].graveyard, vec![WARRIOR, GRAVEROBBER]);
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Revived {
            card: RECOVERY,
            from: 0,
            ..
        }
    )));
}

#[test]
fn graverobber_takes_a_trap_from_the_enemy_graveyard() {
    // emu G4: list1 [811], RIGHT then Cross → {1, 0x20000, 0x403, 0x1032B}:
    // the enemy's trap becomes the activator's face-down card.
    let mut d = board();
    let card = put(&mut d, GRAVEROBBER, 0, p(3, 4));
    d.sides[1].graveyard = vec![WARRIOR, SET_TRAP];
    let (d1, _) = reveal(&d, card);
    assert!(d1.pending.is_some());
    let (d2, _) = pick(&d1, 1, 1).unwrap();
    let new = d2.unit_at(p(3, 4)).expect("the copy");
    assert_eq!((new.card, new.side, new.controller()), (SET_TRAP, 0, 0));
    assert!(!new.face_up && new.acted);
    assert_eq!(d2.sides[1].graveyard, vec![WARRIOR]);
    assert_eq!(d2.sides[0].graveyard, vec![GRAVEROBBER]);
}

#[test]
fn graverobber_can_copy_itself_once_another_non_monster_exists() {
    // emu G3: the count runs before the discard (0x254B4C < 0x254CE4), so
    // the enemy's trap opens the pick, and list0 then holds the
    // Graverobber itself (Cross on list0 → {1, 0x20000, 0x403, 0x2B0}).
    let mut d = board();
    let card = put(&mut d, GRAVEROBBER, 0, p(3, 4));
    d.sides[1].graveyard = vec![SET_TRAP];
    let (d1, _) = reveal(&d, card);
    let (d2, _) = pick(&d1, 0, 0).unwrap();
    let new = d2.unit_at(p(3, 4)).expect("the copy");
    assert_eq!((new.card, new.side), (GRAVEROBBER, 0));
    assert!(d2.sides[0].graveyard.is_empty());
    assert_eq!(d2.sides[1].graveyard, vec![SET_TRAP]);
}

#[test]
fn graverobber_without_a_non_monster_is_spent_for_nothing() {
    // emu G2: graveyards of monsters only → no selection, the card goes to
    // the graveyard (0x254C4C).
    let mut d = board();
    let card = put(&mut d, GRAVEROBBER, 0, p(3, 4));
    d.sides[0].graveyard = vec![WARRIOR];
    d.sides[1].graveyard = vec![WARRIOR];
    let (d1, events) = reveal(&d, card);
    assert_eq!(d1.pending, None);
    assert!(d1.unit_at(p(3, 4)).is_none());
    assert_eq!(d1.sides[0].graveyard, vec![WARRIOR, GRAVEROBBER]);
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::SelectionOpened { .. })));
}

#[test]
fn mode_two_offers_only_the_opponents_graveyard() {
    // 0x254A98: mode 2 → s1 = ctrl ^ 1; the own graveyard's non-monsters
    // (and the card itself) are neither counted nor listed.
    let mut d = board();
    let card = put(&mut d, COPYCAT, 0, p(3, 4));
    d.sides[0].graveyard = vec![RECOVERY];
    let (d1, _) = reveal(&d, card);
    assert_eq!(d1.pending, None);
    let mut d = board();
    let card = put(&mut d, COPYCAT, 0, p(3, 4));
    d.sides[1].graveyard = vec![SET_TRAP];
    let (d1, _) = reveal(&d, card);
    assert!(matches!(
        &d1.pending,
        Some(Pending::GraveyardCopy { graveyards, non_monsters: true, .. }) if graveyards == &vec![1]
    ));
    assert!(pick(&d1, 0, 0).is_err());
}
