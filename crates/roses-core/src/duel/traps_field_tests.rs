//! Shadow of Eyes (trap 0x23), Mirror Wall (0x31), Fake Trap (0x4B) with
//! Harpie's Feather Duster, and magic cards left face-up on the field
//! (claude-effects/TRAPS-0x23-0x31-FAKE-TRAP-FIELD.md; emu cases S, W, F, M).
//! Synthetic cards shaped like the originals; only the Duster keeps its real
//! id because the builder binds event 7 to it.

use super::*;

const LEADER: u16 = 0;
const WARRIOR: u16 = 2; // 1400/1200
const WEAK: u16 = 3; // 1000/1000
const BIG: u16 = 4; // 2400/2000, like Red-Eyes
const SHADOW: u16 = 10;
const GORGON: u16 = 11;
const WALL: u16 = 12;
const FAKE: u16 = 13;
const DECREE: u16 = 14;
const FIELD: u16 = 15; // field change: radius 2, terrain 2 (Mountain-like)

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
    CardInfo {
        kind,
        entries: [Some([word0, word1]), None, None, None, None],
        effects: [true, false, false, false, false],
        ..monster(id, 0, 0)
    }
}

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..=FEATHER_DUSTER).map(|i| monster(i, 100, 100)).collect();
    v[usize::from(LEADER)] = monster(LEADER, 3000, 2500);
    v[usize::from(WARRIOR)] = monster(WARRIOR, 1400, 1200);
    v[usize::from(WEAK)] = monster(WEAK, 1000, 1000);
    v[usize::from(BIG)] = monster(BIG, 2400, 2000);
    let trap = class::FAR_TRAP << 5;
    v[usize::from(SHADOW)] = other(SHADOW, trap, 0x23BD, 0);
    v[usize::from(GORGON)] = other(GORGON, trap, 0x4A9F, 0xFF);
    v[usize::from(WALL)] = other(WALL, trap, 0x31BD, 0);
    v[usize::from(FAKE)] = other(FAKE, trap, 0x4BBD, 0);
    v[usize::from(DECREE)] = other(DECREE, trap, 0x2EBD, 0);
    v[usize::from(FIELD)] = other(FIELD, class::MAGIC << 5, 0x2FD, 0x2_0002);
    // Type 4, mode 3, filter 0x1E (non-monster cards), word1 >> 16 = 0x12.
    v[usize::from(FEATHER_DUSTER)] = other(FEATHER_DUSTER, class::MAGIC << 5, 0x4DE, 0x12_0000);
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

/// Put `card` for `side` at `at` with the given face and position
/// (committed as well).
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

fn act(
    d: &Duel,
    unit: UnitId,
    path: Vec<GridPos>,
    face_up: bool,
    defence: Option<bool>,
) -> (Duel, Vec<DuelEvent>) {
    d.apply(
        0,
        &DuelCommand::Move {
            unit,
            path,
            face_up,
            defence,
        },
        &cards(),
    )
    .unwrap()
}

fn u(d: &Duel, id: UnitId) -> &DuelUnit {
    d.unit(id).expect("unit on the field")
}

fn graves(d: &Duel) -> (Vec<u16>, Vec<u16>) {
    (d.sides[0].graveyard.clone(), d.sides[1].graveyard.clone())
}

#[test]
fn shadow_of_eyes_puts_a_unit_confirming_in_defence_into_attack_position() {
    // emu S1: face-up attack, R1 in place → attack position, acted; the trap
    // is revealed, marked acted and stays. No cancel: the action goes on.
    let mut d = board();
    let w = put(&mut d, WARRIOR, 0, p(4, 5), true, false);
    let t = put(&mut d, SHADOW, 1, p(5, 0), false, false);
    let (d1, events) = act(&d, w, vec![], false, Some(true));
    let m = u(&d1, w);
    assert_eq!(
        (
            m.face_up,
            m.defence,
            m.committed_defence,
            m.acted,
            m.spellbind
        ),
        (true, false, false, true, 0)
    );
    let trap = u(&d1, t);
    assert!(trap.face_up && trap.committed_face_up && trap.acted);
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Cancelled { .. })));
    assert!(events.iter().any(
        |e| matches!(e, DuelEvent::Activated { unit, target: Some(x), .. } if *unit == t && *x == w)
    ));

    // emu S3/S4: a unit in committed defence confirming in place, face-up or
    // face-down, ends in attack position with its face unchanged.
    for face_up in [true, false] {
        let mut d = board();
        let w = put(&mut d, WARRIOR, 0, p(4, 5), face_up, true);
        put(&mut d, SHADOW, 1, p(5, 0), false, false);
        let (d1, _) = act(&d, w, vec![], false, None);
        let m = u(&d1, w);
        assert_eq!(
            (
                m.face_up,
                m.committed_face_up,
                m.defence,
                m.committed_defence,
                m.acted
            ),
            (face_up, face_up, false, false, true)
        );
    }

    // emu S6: an already revealed Shadow of Eyes fires again.
    let mut again = d1.clone();
    let w2 = put(&mut again, WARRIOR, 0, p(1, 5), true, false);
    let (d2, _) = act(&again, w2, vec![], false, Some(true));
    assert!(!u(&d2, w2).defence && u(&d2, w2).acted);
    assert!(u(&d2, t).face_up);

    // emu S2: a move (attack position) does not reach it.
    let (d1, _) = act(&d, w, vec![p(4, 4)], false, None);
    assert!(!u(&d1, t).face_up);
}

#[test]
fn gorgons_eye_is_searched_before_shadow_of_eyes() {
    // emu S5: 0x26C934 (type 0x4A) precedes 0x26CA14 (type 0x23).
    let mut d = board();
    let w = put(&mut d, WARRIOR, 0, p(4, 5), true, false);
    let s = put(&mut d, SHADOW, 1, p(5, 0), false, false);
    let g = put(&mut d, GORGON, 1, p(6, 0), false, false);
    let (d1, _) = act(&d, w, vec![], false, Some(true));
    assert_eq!(u(&d1, w).spellbind, 255);
    assert!(u(&d1, g).face_up && !u(&d1, s).face_up);
}

#[test]
fn mirror_wall_halves_the_attackers_atk_and_the_battle_goes_on() {
    // emu W1: 2400 → ATK change -1200; the battle continues at step 3 with
    // the halved attacker (1200 beats 1000 by 200); the wall stays.
    let mut d = board();
    let a = put(&mut d, BIG, 0, p(2, 3), true, false);
    let v = put(&mut d, WEAK, 1, p(3, 3), true, false);
    let wall = put(&mut d, WALL, 1, p(6, 0), false, false);
    let (d1, events) = act(&d, a, vec![p(3, 3)], false, None);
    assert_eq!(u(&d1, a).attack_bonus, -1200);
    assert_eq!(u(&d1, a).defense_bonus, 0);
    assert!(d1.unit(v).is_none());
    assert_eq!(d1.sides[1].lp, START_LP - 200);
    let w = u(&d1, wall);
    assert!(w.face_up && w.committed_face_up && w.acted);
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Modified { unit, attack: -1200, defense: 0 } if *unit == a
    )));

    // emu W2: halved below the defender: the attacker loses.
    let mut d = board();
    let a = put(&mut d, BIG, 0, p(2, 3), true, false);
    put(&mut d, WARRIOR, 1, p(3, 3), true, false);
    put(&mut d, WALL, 1, p(6, 0), false, false);
    let (d1, _) = act(&d, a, vec![p(3, 3)], false, None);
    assert!(d1.unit(a).is_none());
    assert_eq!(d1.sides[0].lp, START_LP - 200);

    // emu W3/W4: the existing change counts; odd values round down.
    for (bonus, expected) in [(600i16, -900i16), (-1, -1200)] {
        let mut d = board();
        let a = put(&mut d, BIG, 0, p(2, 3), true, false);
        d.units.iter_mut().find(|x| x.id == a).unwrap().attack_bonus = bonus;
        put(&mut d, WEAK, 1, p(3, 3), true, false);
        put(&mut d, WALL, 1, p(6, 0), true, false);
        let (d1, _) = act(&d, a, vec![p(3, 3)], false, None);
        assert_eq!(u(&d1, a).attack_bonus, expected);
    }
}

fn duster_board() -> (Duel, UnitId, [UnitId; 4]) {
    let mut d = board();
    let duster = put(&mut d, FEATHER_DUSTER, 0, p(3, 4), false, false);
    let column = put(&mut d, FIELD, 1, p(3, 1), false, false);
    let row = put(&mut d, FIELD, 1, p(0, 4), false, false);
    let outside = put(&mut d, FIELD, 1, p(5, 2), false, false);
    let monster = put(&mut d, WEAK, 1, p(5, 4), true, false);
    (d, duster, [column, row, outside, monster])
}

fn activate(d: &Duel, unit: UnitId) -> (Duel, Vec<DuelEvent>) {
    act(d, unit, vec![], true, None)
}

#[test]
fn feather_duster_destroys_the_cards_in_its_row_and_column() {
    // emu F9 / F0: non-monster, non-leader units sharing the card's column
    // or row, either side; monsters and cards elsewhere stay.
    let (d, duster, [column, row, outside, monster]) = duster_board();
    let (d1, _) = activate(&d, duster);
    assert!(d1.unit(column).is_none() && d1.unit(row).is_none());
    assert!(d1.unit(outside).is_some() && d1.unit(monster).is_some());
    assert_eq!(graves(&d1), (vec![FEATHER_DUSTER], vec![FIELD, FIELD]));
    // An own card in the cross is destroyed too (mode 3).
    let mut d = d.clone();
    let own = put(&mut d, FIELD, 0, p(3, 5), false, false);
    let (d1, _) = activate(&d, duster);
    assert!(d1.unit(own).is_none());
}

#[test]
fn fake_trap_is_destroyed_instead_of_the_dusters_targets() {
    // emu F1: the enemy's Fake Trap, outside the cross, is revealed and
    // destroyed; nothing else happens.
    let (mut d, duster, [column, row, ..]) = duster_board();
    let fake = put(&mut d, FAKE, 1, p(6, 0), false, false);
    // A set Fake Trap no longer stops the duel.
    assert!(d.apply(0, &DuelCommand::EndTurn, &cards()).is_ok());
    let (d1, events) = activate(&d, duster);
    assert!(d1.unit(fake).is_none());
    assert!(d1.unit(column).is_some() && d1.unit(row).is_some());
    assert_eq!(graves(&d1), (vec![FEATHER_DUSTER], vec![FAKE]));
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::Activated { unit, .. } if *unit == fake)));

    // emu F8: it saves the activator's own cards as well.
    let mut d = board();
    let duster = put(&mut d, FEATHER_DUSTER, 0, p(3, 4), false, false);
    let own = put(&mut d, FIELD, 0, p(0, 4), false, false);
    put(&mut d, FAKE, 1, p(6, 0), false, false);
    let (d1, _) = activate(&d, duster);
    assert!(d1.unit(own).is_some());
    assert_eq!(graves(&d1), (vec![FEATHER_DUSTER], vec![FAKE]));

    // emu F3: an own Fake Trap is not searched.
    let (mut d, duster, [column, ..]) = duster_board();
    let own_fake = put(&mut d, FAKE, 0, p(6, 6), false, false);
    let (d1, _) = activate(&d, duster);
    assert!(d1.unit(column).is_none() && !u(&d1, own_fake).face_up);

    // emu F6: nothing in the cross: no event, the Fake Trap stays set.
    let mut d = board();
    let duster = put(&mut d, FEATHER_DUSTER, 0, p(3, 4), false, false);
    let fake = put(&mut d, FAKE, 1, p(6, 0), false, false);
    let (d1, _) = activate(&d, duster);
    assert!(!u(&d1, fake).face_up);
    assert_eq!(graves(&d1), (vec![FEATHER_DUSTER], vec![]));
}

#[test]
fn royal_decree_lets_the_duster_through_a_fake_trap() {
    // emu F2 / F2b: a Decree in either group is revealed, marked acted and
    // stays; the Fake Trap is untouched and the targets are destroyed.
    for side in [0, 1] {
        let (mut d, duster, [column, row, ..]) = duster_board();
        let fake = put(&mut d, FAKE, 1, p(6, 0), false, false);
        let at = if side == 0 { p(6, 6) } else { p(5, 0) };
        let decree = put(&mut d, DECREE, side, at, false, false);
        let (d1, _) = activate(&d, duster);
        let r = u(&d1, decree);
        assert!(r.face_up && r.acted);
        assert!(!u(&d1, fake).face_up);
        assert!(d1.unit(column).is_none() && d1.unit(row).is_none());
    }
}

#[test]
fn a_face_up_magic_card_activates_when_its_controller_acts_with_it() {
    // emu M1: revealed earlier (Patrol Robo), confirmed in place.
    let mut d = board();
    let m = put(&mut d, FIELD, 0, p(3, 4), true, false);
    let foe = put(&mut d, FIELD, 1, p(6, 0), true, false);
    // Face-up magic on either side no longer stops the duel.
    assert!(d.apply(0, &DuelCommand::EndTurn, &cards()).is_ok());
    let (d1, _) = act(&d, m, vec![], false, None);
    assert!(d1.unit(m).is_none());
    assert_eq!(d1.terrain_at(p(3, 2)), terrain::MOUNTAIN);
    assert_eq!(d1.terrain_at(p(3, 1)), terrain::NORMAL);
    assert!(d1.unit(foe).is_some());
    assert_eq!(graves(&d1), (vec![FIELD], vec![]));

    // emu M2/M4: one step, then it activates on the new cell.
    let (d1, _) = act(&d, m, vec![p(3, 3)], false, None);
    assert_eq!(d1.terrain_at(p(3, 1)), terrain::MOUNTAIN);
    assert_eq!(d1.terrain_at(p(3, 6)), terrain::NORMAL);
    assert!(d
        .apply(
            0,
            &DuelCommand::Move {
                unit: m,
                path: vec![p(3, 3), p(3, 2)],
                face_up: false,
                defence: None,
            },
            &cards(),
        )
        .is_err());
}

#[test]
fn a_face_up_magic_card_onto_an_enemy_card_destroys_it_and_activates_there() {
    // emu M6 (already face-up) and M7 (revealed at selection): event 0x5F,
    // the magic advances and activates on the new cell.
    for already_up in [true, false] {
        let mut d = board();
        let m = put(&mut d, FIELD, 0, p(3, 4), already_up, false);
        let set = put(&mut d, FIELD, 1, p(3, 3), false, false);
        let (d1, _) = act(&d, m, vec![p(3, 3)], !already_up, None);
        assert!(d1.unit(set).is_none() && d1.unit(m).is_none());
        assert_eq!(d1.terrain_at(p(3, 1)), terrain::MOUNTAIN);
        assert_eq!(graves(&d1), (vec![FIELD], vec![FIELD]));
    }
    // emu M8: onto an enemy monster the magic card is destroyed, unused.
    let mut d = board();
    let m = put(&mut d, FIELD, 0, p(3, 4), true, false);
    let foe = put(&mut d, WEAK, 1, p(3, 3), true, true);
    let (d1, _) = act(&d, m, vec![p(3, 3)], false, None);
    assert!(d1.unit(m).is_none() && d1.unit(foe).is_some());
    assert_eq!(d1.terrain_at(p(3, 1)), terrain::NORMAL);
    assert_eq!(graves(&d1), (vec![FIELD], vec![]));
}
