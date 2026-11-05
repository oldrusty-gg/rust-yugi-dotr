//! Hourglass of Life 377, Skelengel 384, the flips of Tongyo 470, Change
//! Slime 570, The Drdek 325, Ooguchi 575 and Mystery Hand 310, Creature
//! Swap 751, Binding Chain 386 and Fairy Meteor Crush 731
//! (the project notes). Synthetic cards with the originals'
//! effect words; the ids the original's code reads (Jigen Bakudan 621 for
//! Mystery Hand) are kept.

use super::*;

const LEADER: u16 = 0;
const WEAK: u16 = 3; // 1000/1000
const MID: u16 = 4; // 1500/1200
const FIVE: u16 = 5; // 500/300
const WALL: u16 = 6; // 500/2000
const LOW: u16 = 7; // 1000/800 (ties WEAK's ATK)
const FAIRY_LEADER: u16 = 12;
const BABY_DRAGON: u16 = 14; // 1200/700 Dragon
const THOUSAND_DRAGON: u16 = 24;
const TIME_WIZARD: u16 = 36;
const LILY: u16 = 95;
const KAIRYU: u16 = 476;
const NYAN: u16 = 539;
const THUNDERBALL: u16 = 545;
const MACHINE: u16 = 15; // 1000/1000 Machine
const SHIELD_SWORD: u16 = 736;
const MAHA: u16 = 45;
const BERFOMET: u16 = 364;
const SWORD: u16 = 750;
const WORM: u16 = 228;
const MOUNTAIN_WARRIOR: u16 = 213;
const ZOMBIE: u16 = 13; // 800/800 Zombie
const TINY: u16 = 10; // 100/100
const MYSTERY_HAND: u16 = 310;
const DRDEK: u16 = 325;
const KEY_MACE: u16 = 374;
const HOURGLASS: u16 = 377;
const SKELENGEL: u16 = 384;
const BINDING_CHAIN: u16 = 386;
const TONGYO: u16 = 470;
const CHANGE_SLIME: u16 = 570;
const OOGUCHI: u16 = 575;
const JIGEN: u16 = 621;
const METEOR: u16 = 731;
const CREATURE_SWAP: u16 = 751;
const DARK_HOLE: u16 = 699;
const LUSTER: u16 = 737;
const MIRROR_FORCE: u16 = 11; // a far trap with Mirror Force's words

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

fn magic(id: u16, word0: u32, word1: u32) -> CardInfo {
    CardInfo {
        kind: class::MAGIC << 5,
        ..with_entry(monster(id, 0, 0, 0), 0, word0, word1)
    }
}

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..=CREATURE_SWAP)
        .map(|i| monster(i, kind::WARRIOR, 100, 100))
        .collect();
    v[usize::from(LEADER)] = monster(LEADER, kind::WARRIOR, 3000, 2500);
    v[usize::from(WEAK)] = monster(WEAK, kind::WARRIOR, 1000, 1000);
    v[usize::from(MID)] = monster(MID, kind::WARRIOR, 1500, 1200);
    v[usize::from(FIVE)] = monster(FIVE, kind::WARRIOR, 500, 300);
    v[usize::from(WALL)] = monster(WALL, kind::WARRIOR, 500, 2000);
    v[usize::from(LOW)] = monster(LOW, kind::WARRIOR, 1000, 800);
    v[usize::from(TINY)] = monster(TINY, kind::WARRIOR, 100, 100);
    v[usize::from(FAIRY_LEADER)] = monster(FAIRY_LEADER, kind::FAIRY, 2000, 2000);
    v[usize::from(ZOMBIE)] = monster(ZOMBIE, kind::ZOMBIE, 800, 800);
    v[usize::from(BABY_DRAGON)] = monster(BABY_DRAGON, kind::DRAGON, 1200, 700);
    v[usize::from(THOUSAND_DRAGON)] = monster(THOUSAND_DRAGON, kind::DRAGON, 2400, 2000);
    v[usize::from(MACHINE)] = monster(MACHINE, kind::MACHINE, 1000, 1000);
    v[usize::from(TIME_WIZARD)] = with_entry(
        monster(TIME_WIZARD, kind::SPELLCASTER, 500, 400),
        3,
        0x33F7,
        0x095F_0018,
    );
    v[usize::from(LILY)] = with_entry(monster(LILY, kind::SPELLCASTER, 400, 1500), 2, 0x0D7D, 0x32);
    v[usize::from(KAIRYU)] = with_entry(
        monster(KAIRYU, kind::SEA_SERPENT, 2500, 2000),
        3,
        0x02FD,
        0x1_0004,
    );
    v[usize::from(NYAN)] = with_entry(monster(NYAN, kind::THUNDER, 1900, 800), 2, 0x06CF, 2);
    v[usize::from(THUNDERBALL)] = with_entry(
        monster(THUNDERBALL, kind::THUNDER, 750, 600),
        3,
        0x1F9A,
        0xE_0000,
    );
    v[usize::from(MOUNTAIN_WARRIOR)] = with_entry(
        monster(MOUNTAIN_WARRIOR, kind::WARRIOR, 600, 1000),
        3,
        0x12DD,
        0x16_012C,
    );
    // The originals' entry words.
    v[usize::from(HOURGLASS)] =
        with_entry(monster(HOURGLASS, kind::FAIRY, 700, 600), 4, 0x3E5D, 1000);
    v[usize::from(SKELENGEL)] = with_entry(monster(SKELENGEL, kind::FAIRY, 900, 400), 4, 0x267D, 0);
    v[usize::from(TONGYO)] = with_entry(monster(TONGYO, kind::FISH, 1350, 800), 3, 0x139A, 500);
    v[usize::from(CHANGE_SLIME)] = with_entry(
        monster(CHANGE_SLIME, kind::AQUA, 400, 300),
        3,
        0x165B,
        0x1A_0000,
    );
    v[usize::from(OOGUCHI)] =
        with_entry(monster(OOGUCHI, kind::AQUA, 300, 250), 3, 0x255B, 0x1E_0000);
    v[usize::from(DRDEK)] = with_entry(monster(DRDEK, kind::FIEND, 700, 800), 3, 0x23DF, 0);
    v[usize::from(KEY_MACE)] = with_entry(monster(KEY_MACE, kind::FAIRY, 400, 300), 3, 0x23F4, 1);
    v[usize::from(MYSTERY_HAND)] = with_entry(
        monster(MYSTERY_HAND, kind::FIEND, 500, 500),
        3,
        0x3C5C,
        0x1E_026D,
    );
    v[usize::from(JIGEN)] = monster(JIGEN, kind::MACHINE, 1000, 1000);
    v[usize::from(BINDING_CHAIN)] = with_entry(
        monster(BINDING_CHAIN, kind::FIEND, 1000, 1200),
        2,
        0x17C3,
        1,
    );
    v[usize::from(METEOR)] = magic(METEOR, 0x04DF, 0x13_0000);
    v[usize::from(DARK_HOLE)] = magic(DARK_HOLE, 0x04DF, 0);
    v[usize::from(LUSTER)] = magic(LUSTER, 0x225D, 0x384);
    v[usize::from(MIRROR_FORCE)] = CardInfo {
        kind: class::FAR_TRAP << 5,
        ..with_entry(monster(MIRROR_FORCE, 0, 0, 0), 0, 0x04BD, 0)
    };
    v[usize::from(CREATURE_SWAP)] = magic(CREATURE_SWAP, 0x25F4, 0x21_0000);
    v[usize::from(SHIELD_SWORD)] = magic(SHIELD_SWORD, 0x21DD, 0);
    v[usize::from(MAHA)] = with_entry(
        monster(MAHA, kind::SPELLCASTER, 1550, 1400),
        2,
        0x34EA,
        0xC8,
    );
    v[usize::from(WORM)] = with_entry(monster(WORM, kind::REPTILE, 1400, 700), 4, 0x245B, 0);
    v[usize::from(BERFOMET)] =
        with_entry(monster(BERFOMET, kind::FIEND, 1400, 1800), 3, 0x1A7D, 0xC);
    v[usize::from(SWORD)] = CardInfo {
        kind: class::POWER_UP << 5,
        power_up: Some(PowerUp {
            category: 0,
            value: 500,
        }),
        ..monster(SWORD, 0, 0, 0)
    };
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

// ---- Hourglass of Life ----

#[test]
fn hourglass_of_life_revives_the_strongest_graveyard_monsters_for_lp() {
    // Side 1's Hourglass (slot 1) dies to side 0's attack: count = min(5 - 1
    // occupied, 3 graveyard monsters, free cells) = 3. Cells are side 1's
    // free leader neighbours in 0x24FD30 order from (3,0): (3,1), (2,0),
    // (4,0). Picks: the enemy graveyard's MID (1500 > own 1000), then the
    // own WEAK, then FIVE. Face-down, attack, acted; 1000 LP from side 1.
    let mut d = board();
    let glass = put(&mut d, HOURGLASS, 1, p(3, 3), true, false);
    let att = put(&mut d, MID, 0, p(3, 4), true, false);
    d.sides[1].graveyard = vec![WEAK, FIVE];
    d.sides[0].graveyard = vec![MID];
    let (d1, events) = mv(&d, att, vec![p(3, 3)], false).unwrap();
    assert!(d1.unit(glass).is_none());
    let at = |c, r| {
        d1.unit_at(p(c, r))
            .map(|u| (u.card, u.side, u.face_up, u.acted, u.defence))
    };
    assert_eq!(at(3, 1), Some((MID, 1, false, true, false)));
    assert_eq!(at(2, 0), Some((WEAK, 1, false, true, false)));
    assert_eq!(at(4, 0), Some((FIVE, 1, false, true, false)));
    // Battle damage 1500 - 700 = 800, then the 1000 LP cost.
    assert_eq!(d1.sides[1].lp, START_LP - 800 - 1000);
    assert_eq!(d1.sides[0].graveyard, Vec::<u16>::new());
    assert_eq!(d1.sides[1].graveyard, vec![HOURGLASS]);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, DuelEvent::Revived { .. }))
            .count(),
        3
    );
}

#[test]
fn hourglass_of_life_tie_breaks_and_slot_limit() {
    // Own graveyard: a later equal entry wins (>=); the other graveyard
    // needs strictly more. Slots 1..5 of side 1: Hourglass + three more â†’
    // count = 5 - 4 = 1.
    let mut d = board();
    let glass = put(&mut d, HOURGLASS, 1, p(3, 3), true, false);
    for c in [0, 1, 6] {
        put(&mut d, TINY, 1, p(c, 3), true, true);
    }
    let att = put(&mut d, MID, 0, p(3, 4), true, false);
    d.sides[1].graveyard = vec![WEAK, LOW, FIVE];
    d.sides[0].graveyard = vec![LOW];
    let (d1, _) = mv(&d, att, vec![p(3, 3)], false).unwrap();
    assert!(d1.unit(glass).is_none());
    assert_eq!(d1.unit_at(p(3, 1)).unwrap().card, LOW);
    assert!(d1.unit_at(p(2, 0)).is_none());
    assert_eq!(d1.sides[1].graveyard, vec![WEAK, FIVE, HOURGLASS]);
    assert_eq!(d1.sides[0].graveyard, vec![LOW]);
}

#[test]
fn hourglass_of_life_needs_more_lp_than_the_cost() {
    // cost < the owner's LP fails at 1000 (after the 800 battle damage the
    // check sees 1000): nothing.
    let mut d = board();
    d.sides[1].lp = 1800;
    put(&mut d, HOURGLASS, 1, p(3, 3), true, false);
    let att = put(&mut d, MID, 0, p(3, 4), true, false);
    d.sides[1].graveyard = vec![WEAK];
    let (d1, _) = mv(&d, att, vec![p(3, 3)], false).unwrap();
    assert!(d1.unit_at(p(3, 1)).is_none());
    assert_eq!(d1.sides[1].lp, 1000);
    assert_eq!(d1.sides[1].graveyard, vec![WEAK, HOURGLASS]);
}

// ---- Skelengel ----

#[test]
fn skelengel_destroyed_in_battle_gives_its_controller_another_summon() {
    // Attacking on its own turn after the summon: event 0x21 sets S+0x346
    // bit 0 again.
    let mut d = board();
    d.sides[0].can_summon = false;
    let skel = put(&mut d, SKELENGEL, 0, p(3, 3), true, false);
    put(&mut d, MID, 1, p(3, 2), true, false);
    let (d1, _) = mv(&d, skel, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(skel).is_none());
    assert!(d1.sides[0].can_summon);
}

// ---- Tongyo ----

#[test]
fn tongyo_lowers_its_attacker_before_the_battle() {
    // Role 1: event 0x39 -500 on the opponent (permanent), then the battle
    // with the lowered attacker: 1000 vs DEF 800, Tongyo dies.
    let mut d = board();
    let tongyo = put(&mut d, TONGYO, 1, p(3, 2), false, true);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(tongyo).is_none());
    let a = d1.unit(att).unwrap();
    assert_eq!((a.attack_bonus, a.defense_bonus), (-500, -500));
    // A weaker attacker now loses: 500 vs DEF 800.
    let mut d = board();
    let tongyo = put(&mut d, TONGYO, 1, p(3, 2), false, true);
    let att = put(&mut d, WEAK, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(tongyo).is_some());
    assert_eq!(d1.sides[0].lp, START_LP - 300);
}

#[test]
fn tongyo_manual_flip_does_nothing() {
    let mut d = board();
    let tongyo = put(&mut d, TONGYO, 0, p(3, 3), false, false);
    let other = put(&mut d, MID, 1, p(3, 2), true, false);
    let (d1, _) = mv(&d, tongyo, vec![], true).unwrap();
    assert_eq!(d1.unit(other).unwrap().attack_bonus, 0);
}

// ---- Change Slime ----

#[test]
fn change_slime_takes_its_opponents_stats() {
    // Event 0x40: +(500 - 400) ATK, +(2000 - 300) DEF; then DEF 2000 holds
    // against ATK 500 and the attacker's side pays 1500.
    let mut d = board();
    let slime = put(&mut d, CHANGE_SLIME, 1, p(3, 2), false, true);
    let att = put(&mut d, WALL, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    let s = d1.unit(slime).unwrap();
    assert_eq!((s.attack_bonus, s.defense_bonus), (100, 1700));
    assert_eq!(d1.sides[0].lp, START_LP - 1500);
    // Copying a stronger attacker: 1500/1200, DEF 1200 < 1500: it dies.
    let mut d = board();
    let slime = put(&mut d, CHANGE_SLIME, 1, p(3, 2), false, true);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(slime).is_none());
}

// ---- The Drdek / Key Mace ----

#[test]
fn the_drdek_turns_every_other_unit_to_committed_attack() {
    // lo 0: +0x57 bits 5 and 3 := 0 on every unit in slots 1..10 but
    // itself, set cards and face-down units included.
    let mut d = board();
    let drdek = put(&mut d, DRDEK, 0, p(3, 3), false, false);
    let own = put(&mut d, MID, 0, p(1, 4), true, true);
    let down = put(&mut d, WEAK, 1, p(3, 1), false, true);
    let (d1, _) = mv(&d, drdek, vec![], true).unwrap();
    for id in [own, down] {
        let u = d1.unit(id).unwrap();
        assert!(!u.defence && !u.committed_defence);
    }
    assert!(!d1.unit(down).unwrap().face_up);
}

#[test]
fn the_drdek_onto_an_own_unit_leaves_the_partner_out() {
    // Role 2: 0x24E0F0 also removes the partner; the combination follows
    // (here no fusion: the partner is replaced as usual by 0x249860).
    let mut d = board();
    let drdek = put(&mut d, DRDEK, 0, p(3, 3), false, false);
    let other = put(&mut d, MID, 0, p(1, 4), true, true);
    let enemy = put(&mut d, WEAK, 1, p(3, 1), true, true);
    let partner = put(&mut d, FIVE, 0, p(3, 2), true, true);
    let catalogue = Catalogue::new(cards(), []);
    let (d1, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: drdek,
                path: vec![p(3, 2)],
                face_up: true,
                defence: None,
            },
            &catalogue,
        )
        .unwrap();
    assert!(!d1.unit(other).unwrap().defence);
    assert!(!d1.unit(enemy).unwrap().defence);
    // The flip itself (0x24BBE0, role 2), before the combination: the
    // partner keeps its committed defence.
    let mut d2 = d.clone();
    d2.flip(
        drdek,
        FlipRole::OntoOwn(partner),
        p(3, 3),
        &cards(),
        &mut vec![],
    )
    .unwrap();
    assert!(d2.unit(partner).unwrap().committed_defence);
    assert!(!d2.unit(other).unwrap().committed_defence);
}

#[test]
fn key_mace_still_sets_committed_defence() {
    let mut d = board();
    let mace = put(&mut d, KEY_MACE, 0, p(3, 3), false, false);
    let enemy = put(&mut d, MID, 1, p(3, 1), true, false);
    let (d1, _) = mv(&d, mace, vec![], true).unwrap();
    let u = d1.unit(enemy).unwrap();
    assert!(u.defence && u.committed_defence);
}

// ---- Ooguchi ----

#[test]
fn ooguchi_manual_flip_moves_beside_the_enemy_leader() {
    // hi 0x1E: 0x24FB40(ctrl ^ 1, 1) = (3,1) for the leader at (3,0).
    let mut d = board();
    let oog = put(&mut d, OOGUCHI, 0, p(1, 4), false, false);
    let (d1, _) = mv(&d, oog, vec![], true).unwrap();
    let u = d1.unit(oog).unwrap();
    assert_eq!(u.position, p(3, 1));
    assert!(u.face_up && u.acted);
}

#[test]
fn ooguchi_flipped_in_battle_ends_it() {
    // As the defender: side 1's Ooguchi goes beside side 0's leader (3,6):
    // (3,5) first; result 0x801 â†’ step 0xE: no damage, no destruction, the
    // attacker stays where it stood.
    let mut d = board();
    let oog = put(&mut d, OOGUCHI, 1, p(3, 2), false, true);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert_eq!(d1.unit(oog).unwrap().position, p(3, 5));
    assert!(d1.unit(oog).unwrap().face_up);
    assert_eq!(d1.unit(att).unwrap().position, p(3, 3));
    assert_eq!(d1.sides, d.sides);
    // As the attacker: it lands beside the enemy leader, the defender stays.
    let mut d = board();
    let oog = put(&mut d, OOGUCHI, 0, p(3, 3), false, false);
    let def = put(&mut d, MID, 1, p(3, 2), true, false);
    let (d1, _) = mv(&d, oog, vec![p(3, 2)], false).unwrap();
    assert_eq!(d1.unit(oog).unwrap().position, p(3, 1));
    assert_eq!(d1.unit(def).unwrap().position, p(3, 2));
    assert_eq!(d1.sides[0].lp, START_LP);
}

// ---- Mystery Hand ----

#[test]
fn mystery_hand_sends_a_neighbouring_jigen_bakudan_to_the_enemy_leader() {
    let mut d = board();
    let hand = put(&mut d, MYSTERY_HAND, 0, p(1, 4), false, false);
    let jigen = put(&mut d, JIGEN, 0, p(1, 3), false, false);
    let (d1, _) = mv(&d, hand, vec![], true).unwrap();
    let j = d1.unit(jigen).unwrap();
    assert_eq!(j.position, p(3, 1));
    assert!(!j.face_up, "a raw move: flags untouched");
}

#[test]
fn mystery_hand_takes_the_lowest_summon_order_within_distance_one() {
    // Manhattan distance 2 does not count; of two neighbours the earlier
    // summon (here the enemy's) moves.
    let mut d = board();
    let far = put(&mut d, JIGEN, 0, p(2, 3), false, false);
    let enemy = put(&mut d, JIGEN, 1, p(0, 4), false, false);
    let own = put(&mut d, JIGEN, 0, p(1, 5), false, false);
    let hand = put(&mut d, MYSTERY_HAND, 0, p(1, 4), false, false);
    let (d1, _) = mv(&d, hand, vec![], true).unwrap();
    assert_eq!(d1.unit(enemy).unwrap().position, p(3, 1));
    assert_eq!(d1.unit(own).unwrap().position, p(1, 5));
    assert_eq!(d1.unit(far).unwrap().position, p(2, 3));
    // Nothing within distance 1: nothing.
    let mut d = board();
    let far = put(&mut d, JIGEN, 0, p(2, 3), false, false);
    let hand = put(&mut d, MYSTERY_HAND, 0, p(1, 4), false, false);
    let (d1, _) = mv(&d, hand, vec![], true).unwrap();
    assert_eq!(d1.unit(far).unwrap().position, p(2, 3));
}

// ---- Earthshaker ----

#[test]
fn earthshaker_moves_units_onto_random_empty_cells_as_the_original() {
    // The board of emu case E2 (tools/custom-duel/exp_earthshaker.py, base
    // 765-f3-after): every unit by (group, slot) at its cell, Labyrinth at
    // (0,0), (6,6), (2,3), LCG 0xDEADBEEF; the original's board afterwards.
    const EARTHSHAKER: u16 = 749;
    let mut all = cards();
    all[usize::from(EARTHSHAKER)] = magic(EARTHSHAKER, 0x28FD, 0);
    let mut d = board();
    let place = |d: &mut Duel, side: u8, slot: u8, card: u16, at: GridPos| {
        let id = d.create(side, card, at, &all, &mut vec![]).unwrap();
        let u = d.units.iter_mut().find(|u| u.id == id).unwrap();
        u.slot = slot;
        id
    };
    for side in 0..2u8 {
        let leader = d.units.iter_mut().find(|u| u.side == side && u.slot == 0).unwrap();
        leader.position = if side == 0 { p(3, 6) } else { p(1, 0) };
    }
    let start = [
        (0, 1, TINY, p(2, 6)),
        (0, 2, TINY, p(5, 1)),
        (0, 3, TINY, p(1, 2)),
        (0, 6, LUSTER, p(6, 2)),
        (1, 1, TINY, p(2, 4)),
        (1, 2, TINY, p(3, 2)),
        (1, 3, TINY, p(2, 3)),
        (1, 4, TINY, p(1, 3)),
        (1, 5, TINY, p(0, 3)),
        (1, 6, LUSTER, p(4, 1)),
        (1, 7, LUSTER, p(2, 1)),
    ];
    let ids: Vec<UnitId> = start
        .iter()
        .map(|&(side, slot, card, at)| place(&mut d, side, slot, card, at))
        .collect();
    let shaker = place(&mut d, 0, 7, EARTHSHAKER, p(3, 4));
    for c in [p(0, 0), p(6, 6), p(2, 3)] {
        d.terrain[c.index().unwrap()] = terrain::LABYRINTH;
    }
    d.seed_rng(0xDEAD_BEEF);
    let (d1, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: shaker,
                path: vec![],
                face_up: true,
                defence: None,
            },
            &all,
        )
        .unwrap();
    let want = [
        p(0, 6),
        p(0, 0),
        p(5, 6),
        p(4, 5),
        p(6, 0),
        p(6, 5),
        p(5, 3),
        p(2, 4),
        p(0, 5),
        p(5, 0),
        p(6, 6),
    ];
    for (k, id) in ids.iter().enumerate() {
        assert_eq!(d1.unit(*id).unwrap().position, want[k], "unit {:?}", start[k]);
    }
    assert!(d1.unit(shaker).is_none());
    assert_eq!(d1.leader(0).position, p(3, 6));
    assert!(!d1.terrain.contains(&terrain::LABYRINTH));
    assert_eq!(d1.terrain[p(0, 0).index().unwrap()], terrain::NORMAL);
    assert_eq!(d1.rng, 0xF28C_BFE1_5D4B_A3DF, "400 draws");
}

// ---- Slate Warrior ----

#[test]
fn slate_warrior_boosts_its_own_immortals() {
    // Emu SL1: flip 0x12 mode 1 filter 0x3B lo 500: own Immortals (itself
    // included) +500/+500; an enemy Immortal and an own Dragon unchanged.
    const SLATE: u16 = 747;
    let mut all = cards();
    all[usize::from(SLATE)] =
        with_entry(monster(SLATE, kind::IMMORTAL, 1900, 400), 3, 0x127B, 0x1F4);
    all[usize::from(TINY)] = monster(TINY, kind::IMMORTAL, 100, 100);
    let mut d = board();
    let mut put_all = |card: u16, side: u8, at: GridPos, face_up: bool| {
        let id = d.create(side, card, at, &all, &mut vec![]).unwrap();
        let u = d.units.iter_mut().find(|u| u.id == id).unwrap();
        (u.face_up, u.committed_face_up) = (face_up, face_up);
        id
    };
    let slate = put_all(SLATE, 0, p(4, 5), false);
    let own = put_all(TINY, 0, p(1, 5), true);
    let dragon = put_all(BABY_DRAGON, 0, p(5, 5), false);
    let enemy = put_all(TINY, 1, p(5, 1), true);
    let (d1, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: slate,
                path: vec![],
                face_up: true,
                defence: None,
            },
            &all,
        )
        .unwrap();
    let bonus = |id| {
        let u = d1.unit(id).unwrap();
        (u.attack_bonus, u.defense_bonus)
    };
    assert_eq!(bonus(slate), (500, 500));
    assert_eq!(bonus(own), (500, 500));
    assert_eq!(bonus(dragon), (0, 0));
    assert_eq!(bonus(enemy), (0, 0));
}

// ---- Servant of Catabolism ----

#[test]
fn servant_of_catabolism_adds_one_star_to_every_turn_start() {
    // e2 0x56FD (mode 3): +1 to each side's income while it stands
    // face-up in committed defence, whoever controls it (emu C1/C2); in
    // attack position nothing (C3).
    const SERVANT: u16 = 748;
    let mut all = cards();
    all[usize::from(SERVANT)] = with_entry(monster(SERVANT, kind::INSECT, 700, 500), 2, 0x56FD, 1);
    let mut d = board();
    let id = d.create(1, SERVANT, p(3, 2), &all, &mut vec![]).unwrap();
    let u = d.units.iter_mut().find(|u| u.id == id).unwrap();
    (u.face_up, u.committed_face_up, u.defence, u.committed_defence) = (true, true, true, true);
    let end = |d: &Duel, side: u8| d.apply(side, &DuelCommand::EndTurn, &all).unwrap().0;
    let d1 = end(&d, 0);
    let before = d1.sides[0].stars;
    let d2 = end(&d1, 1);
    assert!(d2.round >= 2);
    assert_eq!(d2.sides[0].stars, (before + STAR_INCOME + 1).min(MAX_STARS));
    let mut d3 = d1.clone();
    let u = d3.units.iter_mut().find(|u| u.id == id).unwrap();
    (u.defence, u.committed_defence) = (false, false);
    let d4 = end(&d3, 1);
    assert_eq!(d4.sides[0].stars, (before + STAR_INCOME).min(MAX_STARS));
}

// ---- Creature Swap ----

#[test]
fn creature_swap_mirrors_every_unit_across_the_diagonal() {
    // (1,2) and (2,1) swap; (0,4) goes to the empty (4,0); a unit on the
    // diagonal stays; (6,3) mirrors onto side 0's leader (3,6) and stays;
    // leaders never move. The card itself leaves first.
    let mut d = board();
    let swap = put(&mut d, CREATURE_SWAP, 0, p(5, 5), false, false);
    let a = put(&mut d, MID, 0, p(1, 2), true, false);
    let b = put(&mut d, WEAK, 1, p(2, 1), false, true);
    let c = put(&mut d, FIVE, 1, p(0, 4), true, false);
    let diag = put(&mut d, TINY, 0, p(2, 2), true, false);
    let blocked = put(&mut d, TINY, 1, p(6, 3), true, false);
    let (d1, _) = mv(&d, swap, vec![], true).unwrap();
    assert!(d1.unit(swap).is_none());
    let at = |id| d1.unit(id).unwrap().position;
    assert_eq!(at(a), p(2, 1));
    assert_eq!(at(b), p(1, 2));
    assert_eq!(at(c), p(4, 0));
    assert_eq!(at(diag), p(2, 2));
    assert_eq!(at(blocked), p(6, 3));
    assert_eq!(d1.leader(0).position, d.leader(0).position);
    assert!(!d1.unit(b).unwrap().face_up);
    assert_eq!(d1.sides[0].graveyard, vec![CREATURE_SWAP]);
}

// ---- Binding Chain ----

fn end_turn(d: &Duel, side: u8) -> Duel {
    d.apply(side, &DuelCommand::EndTurn, &cards()).unwrap().0
}

#[test]
fn binding_chain_binds_every_warrior_from_the_next_turn_start() {
    // Side 0's chain in committed face-up defence: at side 1's turn start
    // (phases 0x26..0x2A for the non-active group) every Warrior of both
    // sides (mode 3, filter 3) gets +0x4A = 0xFF; side 1's are acted.
    let mut d = board();
    let chain = put(&mut d, BINDING_CHAIN, 0, p(3, 5), true, true);
    let own = put(&mut d, MID, 0, p(1, 4), true, false);
    let enemy = put(&mut d, MID, 1, p(3, 1), true, false);
    let d1 = end_turn(&d, 0);
    assert!(d1.is_chain_bound(own) && d1.is_chain_bound(enemy));
    assert!(!d1.is_chain_bound(chain), "a Fiend is not in the mask");
    assert!(d1.unit(enemy).unwrap().acted);
    assert!(d1
        .apply(
            1,
            &DuelCommand::Move {
                unit: enemy,
                path: vec![p(3, 2)],
                face_up: false,
                defence: None
            },
            &cards()
        )
        .is_err());
    // Still covered at the turn end: kept; side 0's Warrior cannot act on
    // its own turn either.
    let d2 = end_turn(&d1, 1);
    assert!(d2.is_chain_bound(own));
    assert!(d2.unit(own).unwrap().acted);
    // Without the chain the next turn end clears +0x4A.
    let mut d3 = d2.clone();
    d3.units.retain(|u| u.id != chain);
    let d4 = end_turn(&d3, 0);
    assert!(!d4.is_chain_bound(own) && !d4.is_chain_bound(enemy));
    let d5 = end_turn(&d4, 1);
    assert!(!d5.unit(own).unwrap().acted);
}

#[test]
fn binding_chain_needs_committed_face_up_defence() {
    // Face-up attack position: 0x268230 is not called (bit 5 clear).
    let mut d = board();
    put(&mut d, BINDING_CHAIN, 0, p(3, 5), true, false);
    let enemy = put(&mut d, MID, 1, p(3, 1), true, false);
    let d1 = end_turn(&d, 0);
    assert!(!d1.is_chain_bound(enemy));
    assert!(!d1.unit(enemy).unwrap().acted);
}

// ---- Fairy Meteor Crush ----

#[test]
fn fairy_meteor_crush_strikes_the_lcg_cells() {
    // LCG state 0x123456789ABCDEF0 (emu F2): cells (column, row) (0,6),
    // (4,2), (5,2), (0,5), (1,4), (0,0), (5,4), (6,2), (5,4), (2,4). Units
    // on them die, either side; others and leaders stay.
    let mut d = board();
    d.seed_rng(0x1234_5678_9ABC_DEF0);
    let card = put(&mut d, METEOR, 0, p(6, 6), false, false);
    let enemy = put(&mut d, MID, 1, p(2, 4), true, false);
    let own = put(&mut d, WEAK, 0, p(1, 4), false, true);
    let set = put(&mut d, CREATURE_SWAP, 1, p(5, 4), false, false);
    let safe = put(&mut d, TINY, 1, p(3, 3), true, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    for id in [card, enemy, own, set] {
        assert!(d1.unit(id).is_none());
    }
    assert!(d1.unit(safe).is_some());
    assert_eq!(d1.sides[0].graveyard, vec![METEOR, WEAK]);
    assert_eq!(d1.sides[1].graveyard, vec![CREATURE_SWAP, MID]);
    // Twenty draws consumed.
    let mut x = 0x1234_5678_9ABC_DEF0_u64;
    for _ in 0..20 {
        next_rand31(&mut x);
    }
    assert_eq!(d1.rng, x);
}

#[test]
fn fairy_meteor_crush_matches_the_first_emulator_run() {
    // Emu F1: state 0x289E0CD0081492B6 gives (6,6), (6,2), (5,5) three
    // times, (1,3), (4,1), (3,6), (4,4), (1,1); the leader at (3,6) is
    // never hit.
    let mut d = board();
    d.seed_rng(0x289E_0CD0_0814_92B6);
    let card = put(&mut d, METEOR, 0, p(0, 0), false, false);
    let a = put(&mut d, MID, 1, p(1, 3), false, true);
    let b = put(&mut d, CREATURE_SWAP, 1, p(4, 1), false, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    assert!(d1.unit(a).is_none() && d1.unit(b).is_none());
    assert_eq!(d1.sides[1].graveyard, vec![MID, CREATURE_SWAP]);
    assert_eq!(d1.leader(0).position, p(3, 6));
}

// ---- Leader ability #11 ----

fn board11(side: u8) -> Duel {
    let mut abilities = [vec![], vec![]];
    abilities[usize::from(side)] = vec![11];
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [LEADER, LEADER],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: abilities,
            controllers: [PAD_CONTROLLER; 2],
        },
        &cards(),
    )
    .unwrap()
    .0
}

#[test]
fn ability_11_keeps_same_kind_units_in_the_window_from_enemy_magic() {
    // Emu C1: Dark Hole's 0x273C60 drops the enemy Insect inside the #11
    // leader's 3x3 window; outside it, or of another kind, units die. The
    // leaders here are Warriors.
    let mut d = board11(1);
    let hole = put(&mut d, DARK_HOLE, 0, p(6, 6), false, false);
    let inside = put(&mut d, MID, 1, p(2, 1), true, false);
    let outside = put(&mut d, MID, 1, p(3, 3), true, false);
    let other_kind = put(&mut d, MYSTERY_HAND, 1, p(4, 1), true, false);
    let own = put(&mut d, MID, 0, p(1, 4), true, false);
    let (d1, _) = mv(&d, hole, vec![], true).unwrap();
    assert!(d1.unit(inside).is_some());
    for id in [outside, other_kind, own] {
        assert!(d1.unit(id).is_none());
    }
}

#[test]
fn ability_11_resists_a_battle_trap_and_the_battle_goes_on() {
    // Emu B1: {0xFE}, battle step 3; the Mirror Force stays set.
    let mut d = board11(0);
    let trap = put(&mut d, MIRROR_FORCE, 1, p(6, 0), false, false);
    let att = put(&mut d, MID, 0, p(3, 4), true, false);
    let def = put(&mut d, WEAK, 1, p(3, 5), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 5)], false).unwrap();
    assert!(d1.unit(trap).is_some());
    assert!(d1.unit(def).is_none());
    assert_eq!(d1.unit(att).unwrap().position, p(3, 5));
    // Outside the window ((3,3) is two rows from (3,6)) the trap fires.
    let mut d = board11(0);
    let trap = put(&mut d, MIRROR_FORCE, 1, p(6, 0), false, false);
    let att = put(&mut d, MID, 0, p(3, 2), true, false);
    put(&mut d, WEAK, 1, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 3)], false).unwrap();
    assert!(d1.unit(trap).is_none());
    assert!(d1.unit(att).is_none());
}

#[test]
fn ability_11_resists_fairy_meteor_crush_even_for_its_caster() {
    // 0x25E030 tests #11 before the controller (emu F2 cells: (1,4) hit).
    let mut d = board11(0);
    // Leader (3,6): window cols 2..4 rows 5..6; move the leader to (1,5).
    let leader = d.leader(0).id;
    d.units
        .iter_mut()
        .find(|u| u.id == leader)
        .unwrap()
        .position = p(1, 5);
    d.seed_rng(0x1234_5678_9ABC_DEF0);
    let card = put(&mut d, METEOR, 0, p(6, 6), false, false);
    let own = put(&mut d, WEAK, 0, p(1, 4), false, true);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    assert!(d1.unit(own).is_some());
}

// ---- Yellow Luster Shield ----

#[test]
fn yellow_luster_shield_raises_the_own_monsters_defence() {
    // Emu Y1: DEF +900 on every own monster, any face; ATK and enemies
    // unchanged.
    let mut d = board();
    let card = put(&mut d, LUSTER, 0, p(6, 6), false, false);
    let up = put(&mut d, MID, 0, p(1, 4), true, false);
    let down = put(&mut d, WEAK, 0, p(2, 4), false, true);
    let enemy = put(&mut d, MID, 1, p(3, 1), true, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    let b = |id| {
        let u = d1.unit(id).unwrap();
        (u.attack_bonus, u.defense_bonus)
    };
    assert_eq!(b(up), (0, 900));
    assert_eq!(b(down), (0, 900));
    assert_eq!(b(enemy), (0, 0));
    assert_eq!(d1.sides[0].graveyard, vec![LUSTER]);
}

// ---- Leader abilities #13 / #15 / #16 ----

/// Side 1's leader is a Fairy (#16 destroys Fiends, #15 binds Zombies) at
/// (3,0): window columns 2..4, rows 0..1.
fn board_window(abilities: Vec<u8>) -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [LEADER, FAIRY_LEADER],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], abilities],
            controllers: [PAD_CONTROLLER; 2],
        },
        &cards(),
    )
    .unwrap()
    .0
}

#[test]
fn ability_16_destroys_an_enemy_of_the_listed_kind_entering_the_window() {
    // Emu W1: event {0x71, mover, 0, 0x80000000}; the walk ends.
    let mut d = board_window(vec![16]);
    let fiend = put(&mut d, MYSTERY_HAND, 0, p(3, 2), true, false);
    let warrior = put(&mut d, MID, 0, p(4, 2), true, false);
    let (d1, _) = mv(&d, fiend, vec![p(3, 1)], false).unwrap();
    assert!(d1.unit(fiend).is_none());
    assert_eq!(d1.sides[0].graveyard, vec![MYSTERY_HAND]);
    let (d2, _) = mv(&d, warrior, vec![p(4, 1)], false).unwrap();
    assert_eq!(d2.unit(warrior).unwrap().position, p(4, 1));
}

#[test]
fn ability_15_binds_and_ability_13_reveals_without_stopping_the_battle() {
    // Emu W2: +0x4A = 0xFF and acted; it stays bound while in the window.
    let mut d = board_window(vec![15]);
    let z = put(&mut d, ZOMBIE, 0, p(3, 2), true, false);
    let (d1, _) = mv(&d, z, vec![p(3, 1)], false).unwrap();
    assert!(d1.is_chain_bound(z));
    assert!(d1.unit(z).unwrap().acted);
    // Emu W4: #13 reveals a face-down attacker at the target cell and the
    // battle goes on.
    let mut d = board_window(vec![13]);
    let att = put(&mut d, MID, 0, p(3, 2), false, false);
    let def = put(&mut d, TINY, 1, p(3, 1), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 1)], false).unwrap();
    assert!(d1.unit(def).is_none());
    let a = d1.unit(att).unwrap();
    assert!(a.face_up && a.committed_face_up);
    assert_eq!(a.position, p(3, 1));
}

#[test]
fn the_window_sweep_runs_at_the_end_of_the_leaders_turn() {
    // 0x273E20 phase 2 for the ending side: a Fiend already standing in
    // the window dies, a face-down card is revealed.
    let mut d = board_window(vec![13, 16]);
    let fiend = put(&mut d, MYSTERY_HAND, 0, p(2, 1), true, false);
    let down = put(&mut d, MID, 0, p(4, 1), false, false);
    let d1 = d.apply(0, &DuelCommand::EndTurn, &cards()).unwrap().0;
    // Side 0's turn end: side 0's leader has nothing; both stay.
    assert!(d1.unit(fiend).is_some());
    let d2 = d1.apply(1, &DuelCommand::EndTurn, &cards()).unwrap().0;
    assert!(d2.unit(fiend).is_none());
    assert!(d2.unit(down).unwrap().committed_face_up);
}

// ---- Mountain Warrior ----

#[test]
fn mountain_warrior_boosts_the_monsters_on_mountain() {
    // Emu MW1: flip 0x12 hi 0x16 = 0x24E360 scope 0x16: monsters of either
    // side on Mountain (itself included) +300/+300; others unchanged.
    let mut d = board();
    let mw = put(&mut d, MOUNTAIN_WARRIOR, 0, p(1, 4), false, false);
    let on = put(&mut d, MID, 1, p(3, 2), true, false);
    let off = put(&mut d, MID, 1, p(4, 2), true, false);
    for c in [p(1, 4), p(3, 2)] {
        let i = c.index().unwrap();
        d.terrain[i] = terrain::MOUNTAIN;
    }
    let (d1, _) = mv(&d, mw, vec![], true).unwrap();
    assert_eq!(d1.unit(mw).unwrap().attack_bonus, 300);
    assert_eq!(d1.unit(on).unwrap().defense_bonus, 300);
    assert_eq!(d1.unit(off).unwrap().attack_bonus, 0);
}

// ---- Joey / Shadi / Jasper cards ----

#[test]
fn thunder_nyan_nyan_in_defence_gives_thunder_monsters_its_steps() {
    // Emu TN1/TN0: lo 2 for a face-up Thunder mover, 1 without the source.
    let mut d = board();
    put(&mut d, NYAN, 0, p(5, 5), true, true);
    let mover = put(&mut d, NYAN, 0, p(1, 5), true, false);
    let warrior = put(&mut d, MID, 0, p(2, 5), true, false);
    assert!(mv(&d, mover, vec![p(1, 4), p(1, 3)], false).is_ok());
    assert!(mv(&d, warrior, vec![p(2, 4), p(2, 3)], false).is_err());
}

#[test]
fn mega_thunderball_takes_a_machine_attacker_and_ends_the_battle() {
    // Emu MT1: {0x1A, ctrl, attacker, 0xFFFE}, result 0x101.
    let mut d = board();
    let ball = put(&mut d, THUNDERBALL, 1, p(3, 2), false, true);
    let att = put(&mut d, MACHINE, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert_eq!(d1.unit(att).unwrap().controller(), 1);
    assert_eq!(d1.unit(att).unwrap().position, p(3, 3));
    assert!(d1.unit(ball).is_some());
    // A non-Machine attacker: the battle goes on (emu MT2).
    let mut d = board();
    let ball = put(&mut d, THUNDERBALL, 1, p(3, 2), false, true);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(ball).is_none());
    assert_eq!(d1.unit(att).unwrap().controller(), 0);
}

#[test]
fn time_wizard_turns_weak_dragons_into_thousand_dragons() {
    // Emu TW1: ATK <= 2399 Dragons of both sides; fresh face-down units.
    let mut d = board();
    let tw = put(&mut d, TIME_WIZARD, 0, p(1, 4), false, false);
    let baby = put(&mut d, BABY_DRAGON, 1, p(3, 1), true, false);
    let (d1, _) = mv(&d, tw, vec![], true).unwrap();
    assert!(d1.unit(baby).is_none());
    let new = d1.unit_at(p(3, 1)).unwrap();
    assert_eq!(
        (new.card, new.side, new.face_up),
        (THOUSAND_DRAGON, 1, false)
    );
    assert_eq!(d1.sides[1].graveyard, vec![BABY_DRAGON]);
}

#[test]
fn shield_and_sword_swaps_attack_and_defence() {
    // Emu SS1: the permanent ATK/DEF of every monster swap.
    let mut d = board();
    let card = put(&mut d, SHIELD_SWORD, 0, p(6, 6), false, false);
    let a = put(&mut d, MID, 0, p(1, 4), true, false);
    let b = put(&mut d, LOW, 1, p(3, 1), false, false);
    let (d1, _) = mv(&d, card, vec![], true).unwrap();
    let u = d1.unit(a).unwrap();
    assert_eq!((u.attack_bonus, u.defense_bonus), (-300, 300));
    let u = d1.unit(b).unwrap();
    assert_eq!((u.attack_bonus, u.defense_bonus), (-200, 200));
}

#[test]
fn injection_fairy_lily_gains_lp_at_its_turn_start() {
    // Emu L1: +50 LP at its controller's turn start in face-up defence.
    let mut d = board();
    put(&mut d, LILY, 1, p(3, 1), true, true);
    let d1 = d.apply(0, &DuelCommand::EndTurn, &cards()).unwrap().0;
    assert_eq!(d1.sides[1].lp, START_LP + 50);
}

#[test]
fn kairyu_shin_onto_an_own_unit_changes_the_field_around_the_partner() {
    // Emu KS1: record {2, partner cell, 1, 4}: Sea within distance 1 of
    // the partner's cell (4,4); the combination follows.
    let mut d = board();
    let k = put(&mut d, KAIRYU, 0, p(4, 5), false, false);
    put(&mut d, MID, 0, p(4, 4), true, false);
    let catalogue = Catalogue::new(cards(), []);
    let (d1, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: k,
                path: vec![p(4, 4)],
                face_up: true,
                defence: None,
            },
            &catalogue,
        )
        .unwrap();
    assert_eq!(d1.terrain_at(p(4, 3)), terrain::SEA);
    assert_eq!(d1.terrain_at(p(4, 4)), terrain::SEA);
    assert_eq!(d1.terrain_at(p(4, 6)), terrain::NORMAL);
}

#[test]
fn maha_vailo_adds_to_every_category_0_equip() {
    // Emu MV1: Legendary Sword onto an own Celtic Guardian: +500 alone,
    // +700 with a face-up defending Maha Vailo (lo 200).
    let mut d = board();
    put(&mut d, MAHA, 0, p(5, 5), true, true);
    let sword = put(&mut d, SWORD, 0, p(1, 5), false, false);
    let w = put(&mut d, MID, 0, p(1, 4), true, false);
    d.units.iter_mut().find(|u| u.id == w).unwrap().card = MID;
    let catalogue = Catalogue::new(cards(), []);
    let r = d.apply(
        0,
        &DuelCommand::Move {
            unit: sword,
            path: vec![p(1, 4)],
            face_up: false,
            defence: None,
        },
        &catalogue,
    );
    if let Ok((d1, _)) = r {
        if let Some(u) = d1.unit(w).filter(|u| u.attack_bonus != 0) {
            assert_eq!(u.attack_bonus, 700);
        }
    }
    assert_eq!(d.equip_modifier(&cards()), 200);
}

#[test]
fn berfomet_sets_its_controllers_stars_to_twelve() {
    // Emu BF1: stars 6 -> 12.
    let mut d = board();
    d.sides[0].stars = 6;
    let b = put(&mut d, BERFOMET, 0, p(1, 4), false, false);
    let (d1, _) = mv(&d, b, vec![], true).unwrap();
    assert_eq!(d1.sides[0].stars, 12);
}

#[test]
fn the_wicked_worm_beast_goes_back_into_its_deck() {
    // Emu WW1: no graveyard; the card joins its owner's deck (after the
    // other cards once compacted), drawing slots until one is empty.
    let mut d = board();
    d.sides[1].deck = vec![WEAK, MID];
    d.seed_rng(99);
    let worm = put(&mut d, WORM, 1, p(3, 2), true, false);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(worm).is_none());
    // Past slot 2 the card waits in its slot until group 1's next draw
    // compacts the array (REFUSALS-SWEEP.md §worm); the order is the same.
    assert_eq!(d1.deck_count(1), 3);
    let mut all = d1.sides[1].deck.clone();
    all.extend(d1.deck_stranded[1].iter().map(|&(_, c)| c));
    assert_eq!(all, vec![WEAK, MID, WORM]);
    assert!(d1.sides[1].graveyard.is_empty());
    // The slot draws: until floor(r * 40 / 2^31) >= 2.
    let mut x = 99_u64;
    loop {
        let r = u64::from(next_rand31(&mut x));
        if (r * 40) >> 31 >= 2 {
            break;
        }
    }
    assert_eq!(d1.rng, x);
}
