//! The Immortal of Thunder 536 (entry-2 aura 0x13, filter 0x31) and Mirror
//! Force (full-range trap type 4), Raigeki 700 with Anti-Raigeki (0x50)
//! (claude-effects/E2-536-TRAP-0x4.md; emu cases I, MF and RG). Synthetic
//! cards shaped like the originals; Raigeki keeps its real id because the
//! builder binds event 5 to it.

use super::*;

const LEADER: u16 = 0;
const WARRIOR: u16 = 2; // 1400/1200, no Electro-Whip bit
const WEAK: u16 = 3; // 1000/1000
const BIG: u16 = 4; // 2400/2000 Dragon, like Red-Eyes
const FEMALE: u16 = 5; // 1900/2000, Electro-Whip fits (like Nekogal #2)
const IMMORTAL: u16 = 6; // 1500/1300, e2 0x13F1 / 500
const LORD: u16 = 7; // e2 0xB40: Dragon immunity source (Lord of D.)
const MIRROR_FORCE: u16 = 10;
const DECREE: u16 = 11;
const SET_MAGIC: u16 = 12;
const NEGATE: u16 = 13;
const ANTI: u16 = 14;

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

fn other(id: u16, kind: u8, word0: u32, word1: u32) -> CardInfo {
    CardInfo {
        kind,
        entries: [Some([word0, word1]), None, None, None, None],
        effects: [true, false, false, false, false],
        ..monster(id, 0, 0, 0)
    }
}

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..=RAIGEKI)
        .map(|i| monster(i, kind::WARRIOR, 100, 100))
        .collect();
    v[usize::from(LEADER)] = monster(LEADER, kind::WARRIOR, 3000, 2500);
    v[usize::from(WARRIOR)] = monster(WARRIOR, kind::WARRIOR, 1400, 1200);
    v[usize::from(WEAK)] = monster(WEAK, kind::WARRIOR, 1000, 1000);
    v[usize::from(BIG)] = monster(BIG, kind::DRAGON, 2400, 2000);
    let mut female = monster(FEMALE, kind::WARRIOR, 1900, 2000);
    // Bit 15 of the compatibility row = power-up 0x2FF (Electro-Whip).
    female.equip_bits[1] = 0x80;
    v[usize::from(FEMALE)] = female;
    let mut immortal = monster(IMMORTAL, kind::WARRIOR, 1500, 1300);
    immortal.entries[2] = Some([0x13F1, 0x1F4]);
    immortal.effects[2] = true;
    v[usize::from(IMMORTAL)] = immortal;
    let mut lord = monster(LORD, kind::WARRIOR, 1200, 1100);
    lord.entries[2] = Some([0xB40, 0]);
    lord.effects[2] = true;
    v[usize::from(LORD)] = lord;
    let trap = class::FAR_TRAP << 5;
    v[usize::from(MIRROR_FORCE)] = other(MIRROR_FORCE, trap, 0x04BD, 0);
    v[usize::from(DECREE)] = other(DECREE, trap, 0x2EBD, 0);
    v[usize::from(NEGATE)] = other(NEGATE, trap, 0x30BD, 0);
    v[usize::from(ANTI)] = other(ANTI, trap, 0x50BD, 0);
    v[usize::from(SET_MAGIC)] = other(SET_MAGIC, class::MAGIC << 5, 0x2FD, 0x2_0002);
    // Type 4, mode 3, filter 0x1D (monsters), word1 >> 16 = 0x12.
    v[usize::from(RAIGEKI)] = other(RAIGEKI, class::MAGIC << 5, 0x4DD, 0x12_0000);
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

fn attack(d: &Duel, unit: UnitId, to: GridPos) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(
        0,
        &DuelCommand::Move {
            unit,
            path: vec![to],
            face_up: false,
            defence: None,
        },
        &cards(),
    )
}

fn stats(d: &Duel, id: UnitId) -> (u16, u16) {
    let u = d.unit(id).expect("unit on the field");
    (
        d.effective(u, u.position, true, &cards()),
        d.effective(u, u.position, false, &cards()),
    )
}

fn graves(d: &Duel) -> (Vec<u16>, Vec<u16>) {
    (d.sides[0].graveyard.clone(), d.sides[1].graveyard.clone())
}

#[test]
fn immortal_of_thunder_weakens_whip_monsters_on_both_sides() {
    // emu I0/I7: face-up committed defence (0x3C) in g1: Nekogal #2 (g0) and
    // Harpie Lady / Dark Elf -500/-500, every other monster unchanged.
    let mut d = board();
    let imm = put(&mut d, IMMORTAL, 1, p(5, 0), true, true);
    let mine = put(&mut d, FEMALE, 0, p(1, 5), true, false);
    let theirs = put(&mut d, FEMALE, 1, p(1, 1), true, false);
    let plain = put(&mut d, WARRIOR, 0, p(2, 5), true, false);
    assert_eq!(stats(&d, mine), (1400, 1500));
    assert_eq!(stats(&d, theirs), (1400, 1500));
    assert_eq!(stats(&d, plain), (1400, 1200));
    assert_eq!(stats(&d, imm), (1500, 1300), "itself: no Whip bit");
    // The duel is no longer refused.
    assert!(d.apply(0, &DuelCommand::EndTurn, &cards()).is_ok());
}

#[test]
fn immortal_of_thunder_needs_face_up_committed_defence() {
    // emu I1-I4: flags 0x14, 0x28, 0x2C, 0x1C: no change anywhere.
    for (face_up, defence, committed_face_up, committed_defence) in [
        (true, false, true, false),
        (false, true, false, true),
        (true, true, false, true),
        (true, true, true, false),
    ] {
        let mut d = board();
        let imm = put(&mut d, IMMORTAL, 1, p(5, 0), face_up, defence);
        let u = d.units.iter_mut().find(|u| u.id == imm).unwrap();
        (u.committed_face_up, u.committed_defence) = (committed_face_up, committed_defence);
        let f = put(&mut d, FEMALE, 0, p(1, 5), true, false);
        assert_eq!(stats(&d, f), (1900, 2000));
    }
}

#[test]
fn immortals_stack_and_the_floor_is_zero() {
    // emu I8: two Immortals and an ATK change of -1700: 1900 - 1700 = 200,
    // then -1000 → 0 (0x2521F0); DEF 2000 - 1000.
    let mut d = board();
    put(&mut d, IMMORTAL, 1, p(5, 0), true, true);
    put(&mut d, IMMORTAL, 1, p(6, 0), true, true);
    let f = put(&mut d, FEMALE, 0, p(1, 5), true, false);
    d.units.iter_mut().find(|u| u.id == f).unwrap().attack_bonus = -1700;
    assert_eq!(stats(&d, f), (0, 1000));
}

#[test]
fn mirror_force_destroys_the_attackers_side_not_in_defence() {
    // emu MF1/MF2/MF3: the attacker and every other unit its side controls in
    // slots 1..10 with +0x57 bit 3 clear (face-down attack monsters and a
    // set card left in attack position too) go; defence-position units,
    // the leader and the defender stay; the trap goes to the graveyard; no
    // damage, no reveal of the defender, the battle is over.
    let mut d = board();
    let a = put(&mut d, BIG, 0, p(2, 3), true, false);
    let def = put(&mut d, WEAK, 1, p(3, 3), false, true);
    let up = put(&mut d, WARRIOR, 0, p(3, 5), true, false);
    let down_attack = put(&mut d, WARRIOR, 0, p(4, 4), false, false);
    let down_defence = put(&mut d, WARRIOR, 0, p(5, 4), false, true);
    let shown_defence = put(&mut d, WARRIOR, 0, p(2, 5), true, true);
    let set_attack = put(&mut d, SET_MAGIC, 0, p(5, 5), false, false);
    let set_defence = put(&mut d, SET_MAGIC, 0, p(6, 5), false, true);
    let trap = put(&mut d, MIRROR_FORCE, 1, p(6, 0), false, false);
    let (d1, events) = attack(&d, a, p(3, 3)).unwrap();
    for gone in [a, up, down_attack, set_attack, trap] {
        assert!(d1.unit(gone).is_none());
    }
    for kept in [def, down_defence, shown_defence, set_defence] {
        assert!(d1.unit(kept).is_some());
    }
    let dd = d1.unit(def).unwrap();
    assert!(!dd.face_up, "the defender is not revealed");
    assert_eq!((d1.sides[0].lp, d1.sides[1].lp), (START_LP, START_LP));
    // 0x250C90 order: the trap first, then group 0 by slot.
    assert_eq!(
        graves(&d1),
        (vec![BIG, WARRIOR, WARRIOR, SET_MAGIC], vec![MIRROR_FORCE])
    );
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated { unit, target: Some(t), .. } if *unit == trap && *t == a
    )));
}

#[test]
fn mirror_force_comes_before_negate_attack_and_after_royal_decree() {
    // emu MF6: with a Negate Attack also set, Mirror Force fires.
    let mut d = board();
    let a = put(&mut d, BIG, 0, p(2, 3), true, false);
    put(&mut d, WEAK, 1, p(3, 3), true, false);
    let negate = put(&mut d, NEGATE, 1, p(4, 0), false, false);
    put(&mut d, MIRROR_FORCE, 1, p(6, 0), false, false);
    let (d1, _) = attack(&d, a, p(3, 3)).unwrap();
    assert!(d1.unit(a).is_none());
    assert!(!d1.unit(negate).unwrap().face_up, "Negate Attack untouched");

    // emu MF4: an own Royal Decree reacts instead; the battle is normal
    // (2400 beats 1000: defender destroyed, 1400 damage, the attacker
    // advances); Mirror Force stays set.
    let mut d = board();
    let a = put(&mut d, BIG, 0, p(2, 3), true, false);
    let v = put(&mut d, WEAK, 1, p(3, 3), true, false);
    let mf = put(&mut d, MIRROR_FORCE, 1, p(6, 0), false, false);
    let decree = put(&mut d, DECREE, 0, p(6, 6), false, false);
    let (d1, _) = attack(&d, a, p(3, 3)).unwrap();
    assert!(d1.unit(v).is_none());
    assert_eq!(d1.unit(a).unwrap().position, p(3, 3));
    assert_eq!(d1.sides[1].lp, START_LP - 1400);
    let m = d1.unit(mf).unwrap();
    assert!(!m.face_up && !m.acted);
    assert!(d1.unit(decree).unwrap().face_up);
}

#[test]
fn mirror_force_spares_an_immune_dragon_and_the_battle_goes_on() {
    // emu MF5 (own Lord of D.) / MF7 (the enemy's): the trap still fires,
    // the other attack-position units go, the Dragon attacker is dropped
    // from the mask (0x273C60) and the battle continues at step 3.
    for lord_side in [0u8, 1] {
        let mut d = board();
        let a = put(&mut d, BIG, 0, p(2, 3), true, false);
        let v = put(&mut d, WEAK, 1, p(3, 3), true, false);
        let up = put(&mut d, WARRIOR, 0, p(3, 5), true, false);
        let lord_at = if lord_side == 0 { p(5, 5) } else { p(5, 1) };
        let lord = put(&mut d, LORD, lord_side, lord_at, true, true);
        let trap = put(&mut d, MIRROR_FORCE, 1, p(6, 0), false, false);
        let (d1, _) = attack(&d, a, p(3, 3)).unwrap();
        assert!(d1.unit(trap).is_none() && d1.unit(up).is_none());
        assert!(d1.unit(lord).is_some(), "in defence");
        assert!(d1.unit(v).is_none());
        assert_eq!(d1.unit(a).unwrap().position, p(3, 3));
        assert_eq!(d1.sides[1].lp, START_LP - 1400);
    }
}

#[test]
fn mirror_force_with_a_surviving_non_immune_attacker_is_refused() {
    // Result 0x1010F clears the attacker pointer even when filter 0x35
    // spares it (an attacker in defence position): not traced.
    let mut d = board();
    let a = put(&mut d, WARRIOR, 0, p(2, 3), true, false);
    put(&mut d, WEAK, 1, p(3, 3), true, false);
    put(&mut d, MIRROR_FORCE, 1, p(6, 0), false, false);
    d.units.iter_mut().find(|u| u.id == a).unwrap().defence = true;
    // A moving unit leaves defence before battle, so the engine never gets
    // here through a Move; check the guard directly.
    let mut events = vec![];
    let def = d
        .units
        .iter()
        .find(|u| u.side == 1 && u.slot == 1)
        .unwrap()
        .id;
    let r = d.clone().battle(a, def, p(3, 3), &cards(), &mut events);
    assert!(matches!(r, Err(DuelError::Unsupported(_))));
}

fn raigeki_board() -> (Duel, UnitId, [UnitId; 5]) {
    // Raigeki at (3,4); enemy monsters at (2,4) (row) and (3,2) (column),
    // an own monster at (3,5) (column), an enemy monster outside the cross
    // and an enemy set card in the column.
    let mut d = board();
    let r = put(&mut d, RAIGEKI, 0, p(3, 4), false, false);
    let row = put(&mut d, WARRIOR, 1, p(2, 4), true, false);
    let column = put(&mut d, WEAK, 1, p(3, 2), false, true);
    let own = put(&mut d, FEMALE, 0, p(3, 5), true, false);
    let outside = put(&mut d, WEAK, 1, p(5, 2), true, false);
    let set = put(&mut d, SET_MAGIC, 1, p(3, 1), false, false);
    (d, r, [row, column, own, outside, set])
}

fn activate(d: &Duel, unit: UnitId) -> (Duel, Vec<DuelEvent>) {
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

#[test]
fn raigeki_destroys_every_monster_in_its_row_and_column() {
    // emu RG0/RG2/RG7: both sides' monsters in the cross, any face or
    // position; set cards and units elsewhere stay; group 0 first.
    let (d, r, [row, column, own, outside, set]) = raigeki_board();
    let (d1, _) = activate(&d, r);
    for gone in [r, row, column, own] {
        assert!(d1.unit(gone).is_none());
    }
    assert!(d1.unit(outside).is_some() && d1.unit(set).is_some());
    assert_eq!(graves(&d1), (vec![RAIGEKI, FEMALE], vec![WARRIOR, WEAK]));
}

#[test]
fn anti_raigeki_of_either_side_cancels_raigeki_and_stays() {
    // emu RG3 (the opponent's) / RG4 (the activator's own): the trap is
    // revealed and marked acted, nothing is destroyed, Raigeki is spent.
    for side in [1u8, 0] {
        let (mut d, r, targets) = raigeki_board();
        let at = if side == 1 { p(6, 0) } else { p(6, 6) };
        let anti = put(&mut d, ANTI, side, at, false, false);
        let (d1, _) = activate(&d, r);
        for t in targets {
            assert!(d1.unit(t).is_some());
        }
        let a = d1.unit(anti).expect("Anti-Raigeki stays");
        assert!(a.face_up && a.committed_face_up && a.acted);
        assert!(d1.unit(r).is_none());
        assert_eq!(graves(&d1), (vec![RAIGEKI], vec![]));
    }
    // emu RG6: with nothing in the cross the event still runs and the
    // Anti-Raigeki still reacts.
    let mut d = board();
    let r = put(&mut d, RAIGEKI, 0, p(3, 4), false, false);
    let anti = put(&mut d, ANTI, 1, p(6, 0), false, false);
    let (d1, _) = activate(&d, r);
    assert!(d1.unit(anti).unwrap().acted);
}

#[test]
fn royal_decree_lets_raigeki_through_anti_raigeki() {
    // emu RG5: an own Decree is revealed, marked acted and stays; the
    // Anti-Raigeki stays set and Raigeki destroys its targets.
    let (mut d, r, [row, column, own, ..]) = raigeki_board();
    let anti = put(&mut d, ANTI, 1, p(6, 0), false, false);
    let decree = put(&mut d, DECREE, 0, p(6, 6), false, false);
    let (d1, _) = activate(&d, r);
    for gone in [row, column, own] {
        assert!(d1.unit(gone).is_none());
    }
    let a = d1.unit(anti).unwrap();
    assert!(!a.face_up && !a.acted);
    let dc = d1.unit(decree).unwrap();
    assert!(dc.face_up && dc.acted);
}

#[test]
fn raigeki_spares_a_dragon_under_lord_of_d_on_the_other_side() {
    // emu RG8: the enemy's Lord of D. (outside the cross) keeps the enemy
    // Dragon in the column; an own Dragon is not protected from its own
    // controller's Raigeki (0x273B40 drops only non-controlled units).
    let mut d = board();
    let r = put(&mut d, RAIGEKI, 0, p(3, 4), false, false);
    let dragon = put(&mut d, BIG, 1, p(3, 1), true, false);
    let own_dragon = put(&mut d, BIG, 0, p(3, 5), true, false);
    put(&mut d, LORD, 1, p(5, 0), true, true);
    let (d1, _) = activate(&d, r);
    assert!(d1.unit(dragon).is_some());
    assert!(d1.unit(own_dragon).is_none());
}
