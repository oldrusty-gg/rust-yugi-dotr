//! Widespread Ruin 812 (limited-range trap case 11, event 0x2E), Warrior
//! Elimination 720 (magic type 4 / hi 0, default id branch, event 0x15) and
//! Dark Elf 79 (battle pre-pass type 0xD), Solomon's Lawbook 747 (magic
//! type 0x26)
//! (the project notes; emu cases WR, E). Synthetic
//! cards shaped like the originals; the Eliminations keep their real ids
//! because the builder dispatches on the id.

use super::*;

const LEADER: u16 = 0;
const WARRIOR: u16 = 2; // 1400/1200
const WEAK: u16 = 3; // 1000/1000
const BIG: u16 = 4; // 2400/2000 Dragon, like Red-Eyes
const FIEND: u16 = 5; // 1900/2000, not a Warrior
const LORD: u16 = 7; // e2 0xB40: Dragon immunity source (Lord of D.)
const DARK_ELF: u16 = 8; // 2000/800, e0 0xD7D / 0x10032
const GAINER: u16 = 9; // 1000/1000, e0 0xD7D / 0x00032 (hi 0)
const DECREE: u16 = 11;
const SET_MAGIC: u16 = 12;
const RUIN: u16 = 13;
const LAWBOOK: u16 = 14; // e0 0x267D / 0
const WARRIOR_ELIMINATION: u16 = 720;
const DRAGON_ELIMINATION: u16 = 723; // the real 723 is Eradicating Aerosol

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
    let mut v: Vec<CardInfo> = (0..=DRAGON_ELIMINATION)
        .map(|i| monster(i, kind::WARRIOR, 100, 100))
        .collect();
    v[usize::from(LEADER)] = monster(LEADER, kind::WARRIOR, 3000, 2500);
    v[usize::from(WARRIOR)] = monster(WARRIOR, kind::WARRIOR, 1400, 1200);
    v[usize::from(WEAK)] = monster(WEAK, kind::WARRIOR, 1000, 1000);
    v[usize::from(BIG)] = monster(BIG, kind::DRAGON, 2400, 2000);
    v[usize::from(FIEND)] = monster(FIEND, kind::FIEND, 1900, 2000);
    let mut lord = monster(LORD, kind::FIEND, 1200, 1100);
    lord.entries[2] = Some([0xB40, 0]);
    lord.effects[2] = true;
    v[usize::from(LORD)] = lord;
    let mut elf = monster(DARK_ELF, kind::FIEND, 2000, 800);
    elf.entries[0] = Some([0xD7D, 0x1_0032]);
    elf.effects[0] = true;
    v[usize::from(DARK_ELF)] = elf;
    let mut gainer = monster(GAINER, kind::FIEND, 1000, 1000);
    gainer.entries[0] = Some([0xD7D, 0x32]);
    gainer.effects[0] = true;
    v[usize::from(GAINER)] = gainer;
    v[usize::from(DECREE)] = other(DECREE, class::FAR_TRAP << 5, 0x2EBD, 0);
    v[usize::from(SET_MAGIC)] = other(SET_MAGIC, class::MAGIC << 5, 0x2FD, 0x2_0002);
    v[usize::from(LAWBOOK)] = other(LAWBOOK, class::MAGIC << 5, 0x267D, 0);
    v[usize::from(RUIN)] = CardInfo {
        kind_index: 11,
        ..other(RUIN, class::NEAR_TRAP << 5, 1, 0)
    };
    // Type 4, mode 3, filter 3 (Warrior), hi 0 — the real 720's words.
    v[usize::from(WARRIOR_ELIMINATION)] = other(WARRIOR_ELIMINATION, class::MAGIC << 5, 0x4C3, 0);
    // Type 4, mode 3, filter 0 (Dragon), hi 0.
    v[usize::from(DRAGON_ELIMINATION)] = other(DRAGON_ELIMINATION, class::MAGIC << 5, 0x4C0, 0);
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

fn step(d: &Duel, unit: UnitId, to: GridPos, face_up: bool) -> (Duel, Vec<DuelEvent>) {
    d.apply(
        0,
        &DuelCommand::Move {
            unit,
            path: vec![to],
            face_up,
            defence: None,
        },
        &cards(),
    )
    .unwrap()
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

fn graves(d: &Duel) -> (Vec<u16>, Vec<u16>) {
    (d.sides[0].graveyard.clone(), d.sides[1].graveyard.clone())
}

#[test]
fn widespread_ruin_destroys_everything_around_itself_and_the_mover() {
    // emu WR1: Red-Eyes (4,4) LEFT to (3,4), enemy Ruin set at (3,3); own
    // set Mooyan Curry (4,3), enemy monsters (2,3), (2,4), (3,2) in any face
    // or position. All go, the trap first, then column by column (col
    // outer, row inner); (1,3) is outside the 3x3 square and stays.
    let mut d = board();
    let ruin = put(&mut d, RUIN, 1, p(3, 3), false, false);
    let mover = put(&mut d, BIG, 0, p(4, 4), true, false);
    let curry = put(&mut d, SET_MAGIC, 0, p(4, 3), false, true);
    let flea = put(&mut d, WARRIOR, 1, p(2, 3), false, true);
    let spider = put(&mut d, WEAK, 1, p(2, 4), true, false);
    let spider2 = put(&mut d, FIEND, 1, p(3, 2), true, true);
    let outside = put(&mut d, WEAK, 1, p(1, 3), false, true);
    let (d1, events) = step(&d, mover, p(3, 4), false);
    for gone in [ruin, mover, curry, flea, spider, spider2] {
        assert!(d1.unit(gone).is_none(), "{gone:?} should be gone");
    }
    assert!(d1.unit(outside).is_some());
    assert_eq!(
        graves(&d1),
        (vec![BIG, SET_MAGIC], vec![RUIN, WARRIOR, WEAK, FIEND])
    );
    assert_eq!((d1.sides[0].lp, d1.sides[1].lp), (START_LP, START_LP));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated { unit, target: Some(t), .. } if *unit == ruin && *t == mover
    )));
}

#[test]
fn widespread_ruin_needs_a_mover_next_to_it() {
    // emu WR0: entering a cell at distance 2 from the trap: nothing.
    let mut d = board();
    let ruin = put(&mut d, RUIN, 1, p(3, 3), false, false);
    let mover = put(&mut d, BIG, 0, p(4, 5), true, false);
    let (d1, _) = step(&d, mover, p(4, 4), false);
    assert_eq!(d1.unit(mover).unwrap().position, p(4, 4));
    let r = d1.unit(ruin).unwrap();
    assert!(!r.face_up);
}

#[test]
fn widespread_ruin_takes_any_mover_class() {
    // emu WR2: a face-down set magic card moving next to the trap sets it
    // off too (case 11 has no class test).
    let mut d = board();
    let ruin = put(&mut d, RUIN, 1, p(3, 3), false, false);
    let mover = put(&mut d, SET_MAGIC, 0, p(4, 4), false, false);
    let flea = put(&mut d, WARRIOR, 1, p(2, 3), false, true);
    let (d1, _) = step(&d, mover, p(3, 4), false);
    for gone in [ruin, mover, flea] {
        assert!(d1.unit(gone).is_none());
    }
    assert_eq!(graves(&d1), (vec![SET_MAGIC], vec![RUIN, WARRIOR]));
}

#[test]
fn widespread_ruin_skips_an_immune_mover_and_spares_immune_neighbours() {
    // emu WR4: an immune Dragon mover (own Lord of D.): no event at all, the
    // move goes on, the trap stays set.
    let mut d = board();
    let ruin = put(&mut d, RUIN, 1, p(3, 3), false, false);
    let mover = put(&mut d, BIG, 0, p(4, 4), true, false);
    put(&mut d, LORD, 0, p(6, 6), true, true);
    let flea = put(&mut d, WARRIOR, 1, p(2, 3), false, true);
    let (d1, _) = step(&d, mover, p(3, 4), false);
    assert_eq!(d1.unit(mover).unwrap().position, p(3, 4));
    assert!(d1.unit(ruin).is_some_and(|r| !r.face_up));
    assert!(d1.unit(flea).is_some());

    // emu WR5: the mover is not a Dragon; an own Red-Eyes next to the trap
    // is protected from the ENEMY trap (0x269020), the rest goes.
    let mut d = board();
    let ruin = put(&mut d, RUIN, 1, p(3, 3), false, false);
    let mover = put(&mut d, SET_MAGIC, 0, p(4, 4), false, false);
    let dragon = put(&mut d, BIG, 0, p(4, 3), true, false);
    put(&mut d, LORD, 0, p(6, 6), true, true);
    let flea = put(&mut d, WARRIOR, 1, p(2, 3), false, true);
    let (d1, _) = step(&d, mover, p(3, 4), false);
    for gone in [ruin, mover, flea] {
        assert!(d1.unit(gone).is_none());
    }
    assert!(d1.unit(dragon).is_some());

    // The trap controller's own Dragon has no such protection.
    let mut d = board();
    put(&mut d, RUIN, 1, p(3, 3), false, false);
    let mover = put(&mut d, SET_MAGIC, 0, p(4, 4), false, false);
    let theirs = put(&mut d, BIG, 1, p(2, 2), true, false);
    put(&mut d, LORD, 1, p(6, 0), true, true);
    let (d1, _) = step(&d, mover, p(3, 4), false);
    assert!(d1.unit(theirs).is_none());
}

#[test]
fn widespread_ruin_spares_leaders() {
    // emu WR6: Ruin at (3,5) next to the own leader (3,6); Red-Eyes (4,4)
    // DOWN to (4,5); the own Nekogal (2,6) and the mover go, the leader
    // stays, no LP change.
    let mut d = board();
    let ruin = put(&mut d, RUIN, 1, p(3, 5), false, false);
    let mover = put(&mut d, BIG, 0, p(4, 4), true, false);
    let neko = put(&mut d, FIEND, 0, p(2, 6), false, false);
    let (d1, _) = step(&d, mover, p(4, 5), false);
    for gone in [ruin, mover, neko] {
        assert!(d1.unit(gone).is_none());
    }
    assert!(d1.units.iter().any(|u| u.side == 0 && u.slot == 0));
    assert_eq!(graves(&d1), (vec![FIEND, BIG], vec![RUIN]));
    assert_eq!(d1.sides[0].lp, START_LP);
}

#[test]
fn widespread_ruin_yields_to_royal_decree() {
    // 0x24D014: the Decree search precedes every case.
    let mut d = board();
    let ruin = put(&mut d, RUIN, 1, p(3, 3), false, false);
    let mover = put(&mut d, BIG, 0, p(4, 4), true, false);
    let decree = put(&mut d, DECREE, 0, p(6, 6), false, false);
    let (d1, _) = step(&d, mover, p(3, 4), false);
    assert_eq!(d1.unit(mover).unwrap().position, p(3, 4));
    assert!(d1.unit(ruin).is_some());
    assert!(d1.unit(decree).unwrap().face_up);
}

#[test]
fn warrior_elimination_destroys_every_warrior_on_both_sides() {
    // emu E1/E3: own face-up Warrior, enemy set Warrior in defence and a
    // face-up enemy Warrior go (group 0 then 1, by slot); other kinds and
    // leaders stay; the card goes first.
    let mut d = board();
    let card = put(&mut d, WARRIOR_ELIMINATION, 0, p(3, 4), false, false);
    let own = put(&mut d, WARRIOR, 0, p(5, 5), true, false);
    let set = put(&mut d, WEAK, 1, p(5, 1), false, true);
    let up = put(&mut d, WARRIOR, 1, p(6, 2), true, false);
    let fiend = put(&mut d, FIEND, 1, p(1, 1), true, false);
    let dragon = put(&mut d, BIG, 0, p(1, 5), true, false);
    let (d1, _) = activate(&d, card);
    for gone in [card, own, set, up] {
        assert!(d1.unit(gone).is_none());
    }
    for kept in [fiend, dragon] {
        assert!(d1.unit(kept).is_some());
    }
    assert!(d1.units.iter().filter(|u| u.slot == 0).count() == 2);
    assert_eq!(
        graves(&d1),
        (vec![WARRIOR_ELIMINATION, WARRIOR], vec![WEAK, WARRIOR])
    );
}

#[test]
fn warrior_elimination_without_targets_is_spent() {
    // emu E2: record 0xFE, the card is in the graveyard, nothing else.
    let mut d = board();
    let card = put(&mut d, WARRIOR_ELIMINATION, 0, p(3, 4), false, false);
    let fiend = put(&mut d, FIEND, 1, p(1, 1), true, false);
    let (d1, _) = activate(&d, card);
    assert!(d1.unit(card).is_none());
    assert!(d1.unit(fiend).is_some());
    assert_eq!(graves(&d1), (vec![WARRIOR_ELIMINATION], vec![]));
}

#[test]
fn an_elimination_respects_the_dragon_immunity_of_enemy_units_only() {
    // 0x273C60(mask, ctrl): with a Lord of D. on the field the enemy Dragon
    // stays, the activator's own Dragon goes. [F]
    let mut d = board();
    let card = put(&mut d, DRAGON_ELIMINATION, 0, p(3, 4), false, false);
    let own = put(&mut d, BIG, 0, p(5, 5), true, false);
    let theirs = put(&mut d, BIG, 1, p(5, 1), true, false);
    put(&mut d, LORD, 1, p(6, 0), true, true);
    let (d1, _) = activate(&d, card);
    assert!(d1.unit(own).is_none());
    assert!(d1.unit(theirs).is_some());
}

#[test]
fn dark_elf_costs_its_own_group_fifty_in_every_battle() {
    // 0x263DA8: hi 1 → 0x1E0710(g, 1, 0x268A10(0x2C, 50)), attacker or
    // defender, against a leader too.
    // Attacking: 2000 vs 1000 face-up attack: the enemy loses 1000, the Elf's
    // side 50.
    let mut d = board();
    let elf = put(&mut d, DARK_ELF, 0, p(3, 4), true, false);
    let v = put(&mut d, WEAK, 1, p(3, 3), true, false);
    let (d1, _) = step(&d, elf, p(3, 3), true);
    assert!(d1.unit(v).is_none());
    assert_eq!(
        (d1.sides[0].lp, d1.sides[1].lp),
        (START_LP - 50, START_LP - 1000)
    );

    // Attacked: an own 2400 Dragon beats the enemy Elf in attack (400); the
    // Elf's group pays 50 on top.
    let mut d = board();
    let a = put(&mut d, BIG, 0, p(3, 4), true, false);
    let elf = put(&mut d, DARK_ELF, 1, p(3, 3), true, false);
    let (d1, _) = step(&d, a, p(3, 3), true);
    assert!(d1.unit(elf).is_none());
    assert_eq!((d1.sides[0].lp, d1.sides[1].lp), (START_LP, START_LP - 450));

    // A direct attack on the leader.
    let mut d = board();
    let elf = put(&mut d, DARK_ELF, 0, p(3, 1), true, false);
    let (d1, _) = step(&d, elf, p(3, 0), true);
    assert_eq!(
        (d1.sides[0].lp, d1.sides[1].lp),
        (START_LP - 50, START_LP - 2000)
    );
}

#[test]
fn a_type_0xd_battle_effect_with_hi_0_gains() {
    // hi 0 → 0x1E0710(g, 0, 0x268A10(0x2B, lo)): +50 for the own group.
    let mut d = board();
    d.sides[0].lp = 3000;
    let g = put(&mut d, GAINER, 0, p(3, 4), true, false);
    put(&mut d, WEAK, 1, p(3, 3), true, true);
    let (d1, _) = step(&d, g, p(3, 3), true);
    assert_eq!(d1.sides[0].lp, 3050);
}

#[test]
fn solomons_lawbook_gives_its_controller_another_summon() {
    // emu L0/L1: S+0x346 bit 0 of the activating side set, the other side's
    // untouched; the card goes to the graveyard.
    let mut d = board();
    d.sides[0].can_summon = false;
    d.sides[1].can_summon = false;
    let card = put(&mut d, LAWBOOK, 0, p(3, 4), false, false);
    let (d1, _) = activate(&d, card);
    assert!(d1.unit(card).is_none());
    assert!(d1.sides[0].can_summon);
    assert!(!d1.sides[1].can_summon);
    assert_eq!(graves(&d1), (vec![LAWBOOK], vec![]));
}
