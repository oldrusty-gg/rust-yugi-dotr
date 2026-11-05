//! Tests for the effect families traced in the project notes
//! (ABILITY-2, ABILITY-3-CPU, ABILITY-9-10, ABILITY-12, F01–F06) on synthetic
//! cards shaped like the originals (invented ids and numbers, no retail data).

use super::*;

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

fn with_entry(mut card: CardInfo, entry: usize, words: [u32; 2]) -> CardInfo {
    card.entries[entry] = Some(words);
    card.effects[entry] = true;
    card
}

fn other(id: u16, kind: u8, word0: u32, word1: u32) -> CardInfo {
    CardInfo {
        kind,
        entries: [Some([word0, word1]), None, None, None, None],
        ..monster(id, 0, 0, 0, 0, 0)
    }
}

// Synthetic ids.
const LEADER_SPELL: u16 = 1; // Spellcaster level 5 (like 61 Illusionist Faceless Mage)
const LEADER_DRAGON: u16 = 0; // Dragon level 8
const LEADER_DINO: u16 = 30; // Dinosaur level 4 (like 434 Two-Headed King Rex)
const LEADER_LOW: u16 = 33; // Dragon level 3
const WARRIOR: u16 = 2;
const DINO: u16 = 3;
const REPTILE: u16 = 4;
const SPELL: u16 = 6;
const DRAGON: u16 = 7;
const STRONG: u16 = 9; // 1800/1000, like Red-Eyes for these tests
const MOISTURE: u16 = 31; // entry-2 type 0x58
const WORM: u16 = 32; // Dinosaur with entry-1 type 8 (wall flag)
const FLIP_SELF: u16 = 40; // flip boost +300 to card 40
const FLIP_OTHER: u16 = 41; // flip boost +500 to card 42
const PLAIN: u16 = 42;
const FLIP_DEF: u16 = 44; // 300/200, flip boost +300 to card 44
const WEAK_ATT: u16 = 45; // 400/300
const SPARKS: u16 = 50;
const DESSERTS: u16 = 51;
const FAN: u16 = 52;
const DECREE: u16 = 53;
const MODIFIER_2C: u16 = 54; // entry-2 0x3B filter 0x2C, +100
const KURIBOH: u16 = 60; // 300/200, entry-0 type 5
const SHIELD_STRONG: u16 = 61; // 1800/1000, entry-0 type 5
const HAND: u16 = 70; // Fiend's Hand-like: e4 0x049A
const SOUL: u16 = 80; // Hinotama Soul-like: e4 0x04DE
const SECTARIAN: u16 = 90; // 700/600, e4 0x139A / 300
const SECTARIAN_WEAK: u16 = 91; // 100/100, same effect
const KOROGASHI: u16 = 92; // e4 0x13DD / 0x12012C
const JAMMER: u16 = 100;
const RITUAL: u16 = 101;
const TREE: u16 = 102; // entry-2 type 0xF
const MAGIC: u16 = 103; // a plain set magic card (field change type 2)
const JUGGLER: u16 = 509; // the real id: the 0x2E code is bound to 506/509
const COH: u16 = 110; // Change of Heart-like: 0x1FB3 (mode 2, filter 0x33)
const BRAIN: u16 = 111; // Brain Control-like: 0x1FA1 (mode 2, filter 0x21)
const MIMIC: u16 = 112; // Mimicat-like: 0x14DF (mode 3, filter 0x1F any card)
const REBORN: u16 = 113; // Monster Reborn-like: 0x14DD (mode 3, filter 0x1D monsters)
const CELTIC: u16 = 114; // entry-2 type 0x39 (Celtic Guardian)
const WEAKENER: u16 = 115; // entry-2 type 0x4D, 100 (Great Moth-like turn start)
                           // ROUTE-3-5 (the project notes).
const GHOUL: u16 = 120; // Shadow Ghoul-like: e1 0x095B / WALL
const WALL: u16 = 121; // Wall Shadow-like: e1 0x085B
const GUMO: u16 = 122; // Jirai Gumo-like: e0 0x019A, e1 0x425B / 100
const REAPER: u16 = 123; // Reaper of the Cards-like: e1 0x075B
const BIGEYE: u16 = 124; // Big Eye-like: e4 0x179A / 1
const GOLGOIL: u16 = 125; // Golgoil-like: e4 0x145B / 0
const MKING: u16 = 126; // Machine King-like: e3 0x35EE / 0xE0064
const BARREL: u16 = 127; // Barrel Dragon-like: e3 0x04F0 / 0x10000
const PATROL: u16 = 128; // Patrol Robo-like: e3 0x189E / 1
const TIMESEAL: u16 = 129; // Time Seal-like: magic 0x17E1 / 0xFF
const SHIFT: u16 = 130; // Shift-like: magic 0x2561 / 0
const MAGLAB: u16 = 131; // Magical Labyrinth-like: magic 0x48FD / 0
const GORGON: u16 = 133; // Gorgon's Eye-like: far trap 0x4A9F / 0xFF
const BUG: u16 = 135; // Man-Eater Bug-like: e3 0x049A / 0
const SCORPION: u16 = 136; // Nightmare Scorpion-like: e3 0x179A / 1
const PIT: u16 = 134; // limited-range trap, case 10 (ATK <= 4000 destroyed)
const POTION: u16 = 0x31B; // Paralyzing Potion-like: category 3 (an id inside the equip bitmap range)

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..820)
        .map(|i| monster(i, kind::MACHINE, 1, 0, 100, 100))
        .collect();
    v[usize::from(LEADER_DRAGON)] = monster(LEADER_DRAGON, kind::DRAGON, 8, 0, 3000, 2500);
    v[usize::from(LEADER_SPELL)] = monster(LEADER_SPELL, kind::SPELLCASTER, 5, 1, 2500, 2100);
    v[usize::from(WARRIOR)] = monster(WARRIOR, kind::WARRIOR, 4, 3, 1400, 1200);
    v[usize::from(DINO)] = monster(DINO, kind::DINOSAUR, 4, 3, 1200, 1000);
    v[usize::from(REPTILE)] = monster(REPTILE, kind::REPTILE, 3, 3, 1000, 1000);
    v[usize::from(SPELL)] = monster(SPELL, kind::SPELLCASTER, 4, 3, 1400, 1200);
    v[usize::from(DRAGON)] = monster(DRAGON, kind::DRAGON, 4, 3, 1500, 1200);
    v[usize::from(STRONG)] = monster(STRONG, kind::MACHINE, 4, 3, 1800, 1000);
    v[usize::from(LEADER_DINO)] = monster(LEADER_DINO, kind::DINOSAUR, 4, 3, 2000, 1500);
    v[usize::from(MOISTURE)] = with_entry(
        monster(MOISTURE, kind::AQUA, 3, 4, 900, 1200),
        2,
        [0x58BD, 0],
    );
    v[usize::from(WORM)] = with_entry(
        monster(WORM, kind::DINOSAUR, 3, 3, 1000, 900),
        1,
        [0x085B, 0],
    );
    v[usize::from(LEADER_LOW)] = monster(LEADER_LOW, kind::DRAGON, 3, 0, 1000, 1000);
    v[usize::from(FLIP_SELF)] = with_entry(
        monster(FLIP_SELF, kind::FAIRY, 2, 0, 800, 600),
        3,
        [0x12DC, u32::from(FLIP_SELF) << 16 | 300],
    );
    v[usize::from(FLIP_OTHER)] = with_entry(
        monster(FLIP_OTHER, kind::FAIRY, 2, 0, 800, 600),
        3,
        [0x12DC, u32::from(PLAIN) << 16 | 500],
    );
    v[usize::from(PLAIN)] = monster(PLAIN, kind::BEAST, 2, 3, 300, 200);
    v[usize::from(FLIP_DEF)] = with_entry(
        monster(FLIP_DEF, kind::BEAST, 2, 3, 300, 200),
        3,
        [0x12DC, u32::from(FLIP_DEF) << 16 | 300],
    );
    v[usize::from(WEAK_ATT)] = monster(WEAK_ATT, kind::BEAST, 2, 3, 400, 300);
    // Magic type 0xD mode 2 hi 1 (Sparks-like 500) and Just Desserts (filter
    // 0x32, 300 per unit).
    v[usize::from(SPARKS)] = other(SPARKS, 32, 0xD80, 0x10000 | 500);
    v[usize::from(DESSERTS)] = other(DESSERTS, 32, 0xDB2, 0x10000 | 300);
    v[usize::from(FAN)] = other(FAN, 128, 0x29BD, 0);
    v[usize::from(DECREE)] = other(DECREE, 128, 0x2EBD, 0);
    v[usize::from(MODIFIER_2C)] = with_entry(
        monster(MODIFIER_2C, kind::FIEND, 3, 1, 800, 600),
        2,
        [0x3B2C, 100],
    );
    v[usize::from(KURIBOH)] = with_entry(
        monster(KURIBOH, kind::FIEND, 1, 1, 300, 200),
        0,
        [0x055B, 0],
    );
    v[usize::from(SHIELD_STRONG)] = with_entry(
        monster(SHIELD_STRONG, kind::FIEND, 4, 1, 1800, 1000),
        0,
        [0x055B, 0],
    );
    v[usize::from(HAND)] = with_entry(monster(HAND, kind::FIEND, 2, 1, 600, 600), 4, [0x049A, 0]);
    v[usize::from(SOUL)] = with_entry(
        monster(SOUL, kind::PYRO, 2, 3, 500, 500),
        4,
        [0x04DE, 0x10000],
    );
    v[usize::from(SECTARIAN)] = with_entry(
        monster(SECTARIAN, kind::SPELLCASTER, 3, 1, 700, 600),
        4,
        [0x139A, 0x12C],
    );
    v[usize::from(SECTARIAN_WEAK)] = with_entry(
        monster(SECTARIAN_WEAK, kind::SPELLCASTER, 1, 1, 100, 100),
        4,
        [0x139A, 0x12C],
    );
    v[usize::from(KOROGASHI)] = with_entry(
        monster(KOROGASHI, kind::INSECT, 2, 3, 400, 300),
        4,
        [0x13DD, 0x12012C],
    );
    v[usize::from(JAMMER)] = other(JAMMER, 128, 0x0EBD, 0);
    v[usize::from(RITUAL)] = other(RITUAL, 160, 0x325C, 0x1A6);
    v[usize::from(TREE)] = with_entry(monster(TREE, kind::PLANT, 3, 3, 500, 1500), 2, [0x0FBD, 0]);
    v[usize::from(MAGIC)] = other(MAGIC, 32, 0x2FD, 0x20002);
    v[usize::from(COH)] = other(COH, 32, 0x1FB3, 0);
    v[usize::from(BRAIN)] = other(BRAIN, 32, 0x1FA1, 0);
    v[usize::from(MIMIC)] = other(MIMIC, 32, 0x14DF, 0);
    v[usize::from(REBORN)] = other(REBORN, 32, 0x14DD, 0);
    v[usize::from(CELTIC)] = with_entry(
        monster(CELTIC, kind::WARRIOR, 4, 3, 1400, 1200),
        2,
        [0x3900, 0],
    );
    v[usize::from(WEAKENER)] = with_entry(
        monster(WEAKENER, kind::INSECT, 8, 3, 2600, 2500),
        2,
        [0x4D00, 100],
    );
    v[usize::from(JUGGLER)] = with_entry(
        monster(JUGGLER, kind::MACHINE, 3, 2, 900, 700),
        4,
        [0x04DF, 0x10000],
    );
    // ROUTE-3-5 (Labyrinth-ruler, Keith, Darkness-ruler decks).
    v[usize::from(GHOUL)] = with_entry(
        monster(GHOUL, kind::ZOMBIE, 5, 1, 1600, 1300),
        1,
        [0x095B, u32::from(WALL)],
    );
    v[usize::from(WALL)] = with_entry(
        monster(WALL, kind::WARRIOR, 7, 1, 1600, 3000),
        1,
        [0x085B, 0],
    );
    v[usize::from(GUMO)] = with_entry(
        with_entry(monster(GUMO, kind::INSECT, 4, 3, 2200, 100), 0, [0x019A, 0]),
        1,
        [0x425B, 100],
    );
    v[usize::from(REAPER)] = with_entry(
        monster(REAPER, kind::FIEND, 5, 1, 1380, 1930),
        1,
        [0x075B, 0],
    );
    v[usize::from(BIGEYE)] = with_entry(
        monster(BIGEYE, kind::FIEND, 4, 1, 1200, 1000),
        4,
        [0x179A, 1],
    );
    v[usize::from(GOLGOIL)] = with_entry(
        monster(GOLGOIL, kind::MACHINE, 4, 1, 900, 1600),
        4,
        [0x145B, 0],
    );
    v[usize::from(MKING)] = with_entry(
        monster(MKING, kind::MACHINE, 6, 1, 2200, 2000),
        3,
        [0x35EE, 0xE_0064],
    );
    v[usize::from(BARREL)] = with_entry(
        monster(BARREL, kind::MACHINE, 7, 1, 2600, 2200),
        3,
        [0x04F0, 0x1_0000],
    );
    v[usize::from(PATROL)] = with_entry(
        monster(PATROL, kind::MACHINE, 3, 1, 1100, 900),
        3,
        [0x189E, 1],
    );
    v[usize::from(BUG)] = with_entry(monster(BUG, kind::INSECT, 2, 1, 450, 600), 3, [0x049A, 0]);
    v[usize::from(SCORPION)] = with_entry(monster(SCORPION, kind::INSECT, 3, 1, 900, 1100), 3, [0x179A, 1]);
    v[usize::from(TIMESEAL)] = other(TIMESEAL, 32, 0x17E1, 0xFF);
    v[usize::from(SHIFT)] = other(SHIFT, 32, 0x2561, 0);
    v[usize::from(MAGLAB)] = other(MAGLAB, 32, 0x48FD, 0);
    v[usize::from(GORGON)] = other(GORGON, 128, 0x4A9F, 0xFF);
    v[usize::from(PIT)] = CardInfo {
        kind_index: 10,
        ..other(PIT, 96, 0, 0)
    };
    v[usize::from(POTION)] = CardInfo {
        power_up: Some(PowerUp {
            category: 3,
            value: 666,
        }),
        ..other(POTION, 64, 0, 0)
    };
    let bit = usize::from(POTION - 0x2F0);
    for id in [WARRIOR, STRONG] {
        v[usize::from(id)].equip_bits[bit >> 3] |= 1 << (bit & 7);
    }
    v
}

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn placed(
    id: u16,
    card: u16,
    side: u8,
    slot: u8,
    at: GridPos,
    defence: bool,
    face_up: bool,
) -> DuelUnit {
    DuelUnit {
        id: UnitId(id),
        card,
        side,
        slot,
        position: at,
        face_up,
        defence,
        committed_face_up: face_up,
        committed_defence: defence,
        acted: side != 0,
        spellbind: 0,
        summon_order: 0,
        attack_bonus: 0,
        defense_bonus: 0,
        spellbind_fresh: false,
        evo_counter: 0,
        stolen: false,
        discount: 0,
        stolen_by: None,
    }
}

fn setup(leaders: [u16; 2], abilities: [Vec<u8>; 2]) -> DuelSetup {
    DuelSetup {
        terrain: vec![terrain::NORMAL; 49],
        leaders,
        decks: [vec![], vec![]],
        first: 0,
        leader_abilities: abilities,
        controllers: [PAD_CONTROLLER; 2],
    }
}

/// A duel with the given leaders and abilities, side 0 to act, with `units`
/// placed (side 1's units marked acted).
fn board_with(leaders: [u16; 2], abilities: [Vec<u8>; 2], units: Vec<DuelUnit>) -> Duel {
    let (mut d, _) = Duel::new(setup(leaders, abilities), &cards()).unwrap();
    d.units.extend(units);
    d.next_unit = 100;
    d
}

fn board(units: Vec<DuelUnit>) -> Duel {
    board_with([LEADER_SPELL, LEADER_DRAGON], [vec![], vec![]], units)
}

fn apply(d: &Duel, side: u8, cmd: DuelCommand) -> (Duel, Vec<DuelEvent>) {
    d.apply(side, &cmd, &cards()).unwrap()
}

fn attack(d: &Duel, unit: u16, to: GridPos) -> (Duel, Vec<DuelEvent>) {
    apply(
        d,
        0,
        DuelCommand::Move {
            unit: UnitId(unit),
            path: vec![to],
            face_up: false,
            defence: None,
        },
    )
}

fn reveal(d: &Duel, unit: u16) -> (Duel, Vec<DuelEvent>) {
    apply(
        d,
        0,
        DuelCommand::Move {
            unit: UnitId(unit),
            path: vec![],
            face_up: true,
            defence: None,
        },
    )
}

fn try_reveal(d: &Duel, unit: u16) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(
        0,
        &DuelCommand::Move {
            unit: UnitId(unit),
            path: vec![],
            face_up: true,
            defence: None,
        },
        &cards(),
    )
}

fn lp(d: &Duel) -> [u16; 2] {
    [d.sides[0].lp, d.sides[1].lp]
}

fn graves(d: &Duel) -> (Vec<u16>, Vec<u16>) {
    (d.sides[0].graveyard.clone(), d.sides[1].graveyard.clone())
}

fn bonus(d: &Duel, id: u16) -> (i16, i16) {
    let u = d.unit(UnitId(id)).unwrap();
    (u.attack_bonus, u.defense_bonus)
}

fn count_events(events: &[DuelEvent], f: impl Fn(&DuelEvent) -> bool) -> usize {
    events.iter().filter(|e| f(e)).count()
}

fn experience(d: &Duel, side: usize, card: u16) -> i32 {
    d.experience[side].get(&card).map_or(0, |e| e.delta)
}

// ---- ABILITY-2 ---------------------------------------------------------------

#[test]
fn ability_2_is_accepted_at_setup_and_changes_nothing() {
    // #2 has no consumer in the executable (ABILITY-2.md §2); other rank
    // abilities stay refused.
    let d = board_with(
        [LEADER_SPELL, LEADER_DRAGON],
        [vec![2], vec![0, 2, 3]],
        vec![],
    );
    assert_eq!(d.sides[1].leader_abilities, [0, 2, 3]);
    let (d, events) = apply(&d, 0, DuelCommand::EndTurn);
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::TurnStarted { side: 1, .. })));
    assert_eq!(lp(&d), [4000, 4000]);
    // #8 and #14 are implemented since 1 Oct (summon discount, race aura),
    // #11 since 4 Oct (white_rules / immunity.rs), #7 since 7 Oct
    // (refusals_tests.rs); numbers past #16 do not exist.
    for refused in [17] {
        assert!(
            matches!(
                Duel::new(
                    setup([LEADER_SPELL, LEADER_DRAGON], [vec![refused], vec![]]),
                    &cards()
                ),
                Err(DuelError::Unsupported(_))
            ),
            "ability #{refused}"
        );
    }
}

// ---- ABILITY-3-CPU -----------------------------------------------------------

#[test]
fn destiny_draw_arming_follows_the_controller_class() {
    // ABILITY-3-CPU.md §2: a computer module other than Seto (1) / Yugi
    // (0x13) never arms, whatever its leader lists; scripts and Seto/Yugi
    // always qualify; a pad side needs #3.
    let low = |controllers: [u8; 2], abilities: Vec<u8>| {
        let mut s = setup([LEADER_SPELL, LEADER_DINO], [vec![], abilities]);
        s.controllers = controllers;
        let (mut d, _) = Duel::new(s, &cards()).unwrap();
        d.sides[1].lp = 500;
        d
    };
    for cpu in [2, 3, 4, 9, 0, 0x16] {
        let d = low([PAD_CONTROLLER, cpu], vec![0, 1, 3, 12]);
        let (d, events) = apply(&d, 0, DuelCommand::EndTurn);
        assert!(
            matches!(events.last(), Some(DuelEvent::TurnStarted { side: 1, .. })),
            "controller {cpu}"
        );
        assert_eq!(
            (d.active, d.sides[1].lp, d.sides[1].hand.len()),
            (1, 500, 0)
        );
        assert!(d.sides[1].can_summon);
    }
    // Seto, Yugi and scripts roll 6 + (r & 1) (one rand31); a pad side with
    // #3 on an empty board (no hand) is situation 4 [SPECIAL-DRAW.md §2].
    for (arming, expect) in [
        (([PAD_CONTROLLER, 1], vec![]), [6, 7]),
        (([PAD_CONTROLLER, 0x13], vec![]), [6, 7]),
        (([PAD_CONTROLLER, 0x25], vec![]), [6, 7]),
        (([PAD_CONTROLLER, PAD_CONTROLLER], vec![3]), [4, 4]),
    ] {
        let d = low(arming.0, arming.1.clone());
        let (d, _) = apply(&d, 0, DuelCommand::EndTurn);
        assert!(expect.contains(&d.sides[1].destiny), "{arming:?}: {}", d.sides[1].destiny);
    }
    // A pad side without #3 below 1000 LP just goes on.
    let d = low([PAD_CONTROLLER, PAD_CONTROLLER], vec![0, 1]);
    assert!(d.apply(0, &DuelCommand::EndTurn, &cards()).is_ok());
    // At 1000 LP nothing arms even for Seto.
    let mut d = low([PAD_CONTROLLER, 1], vec![]);
    d.sides[1].lp = 1000;
    let (d, _) = apply(&d, 0, DuelCommand::EndTurn);
    assert_eq!(d.sides[1].destiny, 0);
}

// ---- ABILITY-9-10 ------------------------------------------------------------

#[test]
fn ability_10_adds_capped_level_bonus_to_same_type_units_in_the_window() {
    let c = cards();
    let units = || {
        vec![
            placed(10, SPELL, 0, 1, p(2, 5), false, true), // adjacent
            placed(11, SPELL, 0, 2, p(1, 4), true, false), // distance 2, face-down defence
            placed(12, WARRIOR, 0, 3, p(4, 5), false, true), // other kind
            placed(13, SPELL, 1, 1, p(3, 4), false, true), // enemy Spellcaster near our leader
            placed(14, SPELL, 0, 4, p(0, 0), false, true), // far away
        ]
    };
    let eff = |d: &Duel, id: u16, atk: bool| {
        let u = d.unit(UnitId(id)).unwrap();
        d.effective(u, u.position, atk, &c)
    };
    // #10 alone: 3x3 window, +min(100 * 5, 500) = 500 on ATK and DEF.
    let d = board_with([LEADER_SPELL, LEADER_DRAGON], [vec![10], vec![]], units());
    assert_eq!((eff(&d, 10, true), eff(&d, 10, false)), (1900, 1700));
    assert_eq!(eff(&d, 11, true), 1400); // distance 2 without #9
    assert_eq!(eff(&d, 12, true), 1400);
    assert_eq!(eff(&d, 13, true), 1400); // the enemy's own leader is a Dragon
    assert_eq!(eff(&d, 14, true), 1400);
    // The leader itself reads the bonus at its own cell.
    assert_eq!(eff(&d, 0, true), 3000);
    // #9 widens the window to 5x5: the face-down defender at distance 2 too.
    let d = board_with(
        [LEADER_SPELL, LEADER_DRAGON],
        [vec![9, 10], vec![]],
        units(),
    );
    assert_eq!((eff(&d, 11, true), eff(&d, 11, false)), (1900, 1700));
    assert_eq!(eff(&d, 14, true), 1400);
    // #9 alone changes nothing.
    let d = board_with([LEADER_SPELL, LEADER_DRAGON], [vec![9], vec![]], units());
    assert_eq!(eff(&d, 10, true), 1400);
    // Level 3 leader: +300; level 8: capped at 500; a non-monster card under
    // a Dragon leader is kind 0 = Dragon and gets it too (code-literal).
    let d = board_with(
        [LEADER_LOW, LEADER_DRAGON],
        [vec![10], vec![10]],
        vec![
            placed(10, DRAGON, 0, 1, p(2, 5), false, true),
            placed(11, DRAGON, 1, 1, p(4, 1), false, true),
            placed(12, MAGIC, 1, 6, p(2, 1), false, false),
        ],
    );
    assert_eq!(eff(&d, 10, true), 1800);
    assert_eq!(eff(&d, 11, true), 2000);
    assert_eq!(eff(&d, 12, true), 500);
    // The bonus is read at the evaluated cell: an attacker leaving the window
    // fights without it (battle at the defender's cell).
    let d = board_with(
        [LEADER_SPELL, LEADER_DRAGON],
        [vec![10], vec![]],
        vec![
            placed(10, SPELL, 0, 1, p(3, 5), false, true),
            placed(11, WARRIOR, 1, 1, p(2, 5), false, true),
            placed(12, WARRIOR, 1, 2, p(1, 5), false, true),
        ],
    );
    let (d1, events) = attack(&d, 10, p(2, 5));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Battle {
            attack: 1900,
            defend: 1400,
            ..
        }
    )));
    assert_eq!(lp(&d1), [4000, 3500]);
    // From (2,5) the Spellcaster attacks (1,5): outside the 3x3, 1400 vs 1400.
    let mut d2 = d1.clone();
    d2.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let (d3, events) = attack(&d2, 10, p(1, 5));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Battle {
            attack: 1400,
            defend: 1400,
            ..
        }
    )));
    assert!(d3.unit(UnitId(10)).is_none() && d3.unit(UnitId(12)).is_none());
}

#[test]
fn a_controlled_moisture_creature_suppresses_the_leader_abilities() {
    // 0x268890(0x58): only a face-up committed-defence unit the opponent
    // controls; attack position or face-down does not count.
    let c = cards();
    let d = board_with(
        [LEADER_SPELL, LEADER_DRAGON],
        [vec![9, 10, 12], vec![]],
        vec![
            placed(10, SPELL, 0, 1, p(2, 5), false, true),
            placed(11, MOISTURE, 1, 1, p(3, 2), true, true),
        ],
    );
    assert!(!d.leader_has(0, 10, &c) && !d.leader_has(0, 9, &c) && !d.leader_has(0, 12, &c));
    let u = d.unit(UnitId(10)).unwrap();
    assert_eq!(d.effective(u, u.position, true, &c), 1400);
    let mut d2 = d.clone();
    d2.units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .defence = false;
    assert!(d2.leader_has(0, 10, &c));
    let mut d3 = d.clone();
    d3.units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .committed_face_up = false;
    assert!(d3.leader_has(0, 10, &c));
    // An own Moisture Creature does not suppress the own leader.
    let mut d4 = d.clone();
    d4.units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .side = 0;
    assert!(d4.leader_has(0, 10, &c));
}

// ---- ABILITY-12 --------------------------------------------------------------

#[test]
fn ability_12_gives_two_steps_to_same_type_units_in_the_window() {
    let c = cards();
    let steps = |d: &Duel, id: u16| d.allowance(d.unit(UnitId(id)).unwrap(), &c).unwrap();
    let make = |abilities: Vec<u8>, units: Vec<DuelUnit>| {
        board_with([LEADER_DINO, LEADER_DRAGON], [abilities, vec![]], units)
    };
    let d = make(
        vec![12],
        vec![
            placed(10, DINO, 0, 1, p(3, 5), false, true), // adjacent, Normal
            placed(11, DINO, 0, 2, p(2, 5), true, true),  // diagonal, defence
            placed(12, DINO, 0, 3, p(3, 4), false, true), // distance 2
            placed(13, DINO, 0, 4, p(4, 5), false, false), // face-down
            placed(14, REPTILE, 0, 5, p(4, 6), false, true), // other kind
            placed(15, WORM, 1, 1, p(2, 6), false, true), // enemy Dinosaur next to our leader
        ],
    );
    assert_eq!(
        [
            steps(&d, 10),
            steps(&d, 11),
            steps(&d, 12),
            steps(&d, 13),
            steps(&d, 14)
        ],
        [2, 2, 1, 1, 1]
    );
    // The enemy's Dinosaur uses its own (Dragon) leader: terrain only.
    assert_eq!(steps(&d, 15), 1);
    assert_eq!(steps(&d, 0), 1); // the leader itself: only #4
                                 // With #9 the distance-2 unit is inside the window.
    let d9 = make(
        vec![9, 12],
        vec![placed(12, DINO, 0, 3, p(3, 4), false, true)],
    );
    assert_eq!(steps(&d9, 12), 2);
    // Overrides the terrain result (a Dinosaur on Sea would have 1 step).
    let mut sea = make(vec![12], vec![placed(10, DINO, 0, 1, p(3, 5), false, true)]);
    sea.terrain[p(3, 5).index().unwrap()] = terrain::SEA;
    assert_eq!(steps(&sea, 10), 2);
    // Entry-1 type 8 keeps its wall flag and gets 2 steps.
    let worm = make(vec![12], vec![placed(10, WORM, 0, 1, p(3, 5), false, true)]);
    assert_eq!(steps(&worm, 10), 2);
    assert!(worm.has_wall_flag(worm.unit(UnitId(10)).unwrap(), &c));
    // Face-up non-monster under a Dragon-kind #12 leader: kind 0 = Dragon.
    let dragon = board_with(
        [LEADER_DRAGON, LEADER_SPELL],
        [vec![12], vec![]],
        vec![placed(10, MAGIC, 0, 6, p(3, 5), false, true)],
    );
    assert_eq!(steps(&dragon, 10), 2);
    // An opposing face-up committed-defence Moisture Creature switches #12
    // (and #4) off.
    let moist = make(
        vec![4, 12],
        vec![
            placed(10, DINO, 0, 1, p(3, 5), false, true),
            placed(11, MOISTURE, 1, 1, p(3, 2), true, true),
        ],
    );
    assert_eq!((steps(&moist, 10), steps(&moist, 0)), (1, 1));
    // Through the command: a two-cell walk is legal next to the leader and
    // refused from two cells away.
    let walk = |d: &Duel, id: u16, path: Vec<GridPos>| {
        d.apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(id),
                path,
                face_up: false,
                defence: None,
            },
            &c,
        )
    };
    let d = make(vec![12], vec![placed(10, DINO, 0, 1, p(3, 5), false, true)]);
    let (d1, _) = walk(&d, 10, vec![p(2, 5), p(1, 5)]).unwrap();
    assert_eq!(d1.unit(UnitId(10)).unwrap().position, p(1, 5));
    let d = make(vec![12], vec![placed(10, DINO, 0, 1, p(3, 4), false, true)]);
    assert!(matches!(
        walk(&d, 10, vec![p(2, 4), p(1, 4)]),
        Err(DuelError::Illegal(_))
    ));
}

// ---- F01 ---------------------------------------------------------------------

#[test]
fn flip_boost_by_card_id_hits_every_copy_but_leaders() {
    // F01.md §3: both sides, any state, itself included, leaders never.
    let d = board_with(
        [LEADER_SPELL, FLIP_SELF],
        [vec![], vec![]],
        vec![
            placed(10, FLIP_SELF, 0, 1, p(3, 5), false, false),
            placed(11, FLIP_SELF, 0, 2, p(2, 5), true, false), // own face-down
            placed(12, FLIP_SELF, 1, 1, p(3, 1), false, true), // enemy face-up
            placed(13, FLIP_SELF, 1, 2, p(0, 0), true, false), // enemy face-down defence
            placed(14, PLAIN, 0, 3, p(4, 5), false, true),
        ],
    );
    let (d1, events) = reveal(&d, 10);
    assert_eq!(
        count_events(&events, |e| matches!(e, DuelEvent::Activated { .. })),
        1
    );
    for id in [10, 11, 12, 13] {
        assert_eq!(bonus(&d1, id), (300, 300), "unit {id}");
    }
    assert_eq!(bonus(&d1, 14), (0, 0));
    assert_eq!(bonus(&d1, 1), (0, 0)); // the leader with the matching id
                                       // Targets are not revealed.
    assert!(!d1.unit(UnitId(11)).unwrap().face_up && !d1.unit(UnitId(13)).unwrap().face_up);
    assert!(d1.unit(UnitId(10)).unwrap().committed_face_up);
    // No target: no event, the flip is still consumed.
    let d = board(vec![placed(10, FLIP_OTHER, 0, 1, p(3, 5), false, false)]);
    let (d2, events) = reveal(&d, 10);
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Activated { .. })));
    assert!(d2.unit(UnitId(10)).unwrap().committed_face_up);
    // The 9999 cap: the delta becomes 9999 - base.
    let mut plain = placed(11, PLAIN, 1, 1, p(0, 0), false, true);
    plain.attack_bonus = 9600;
    plain.defense_bonus = 9750;
    let d = board(vec![
        placed(10, FLIP_OTHER, 0, 1, p(3, 5), false, false),
        plain,
    ]);
    let (d3, _) = reveal(&d, 10);
    assert_eq!(bonus(&d3, 11), (9699, 9799));
    // Event order follows the mask (side 0 slots ascending, then side 1),
    // not the units' creation order.
    let d = board(vec![
        placed(12, FLIP_SELF, 1, 2, p(0, 0), true, false),
        placed(11, FLIP_SELF, 1, 1, p(3, 1), false, true),
        placed(13, FLIP_SELF, 0, 3, p(2, 5), true, false),
        placed(10, FLIP_SELF, 0, 1, p(3, 5), false, false),
    ]);
    let (_, events) = reveal(&d, 10);
    let order: Vec<UnitId> = events
        .iter()
        .filter_map(|e| match e {
            DuelEvent::Modified { unit, .. } => Some(*unit),
            _ => None,
        })
        .collect();
    assert_eq!(order, [UnitId(10), UnitId(13), UnitId(11), UnitId(12)]);
}

#[test]
fn flip_boost_lands_before_the_battle_stats() {
    // A face-down defence 300/200 card boosting itself at step 3 holds a
    // 400 attacker (DEF 500); the attacker's side pays 100.
    let d = board(vec![
        placed(10, WEAK_ATT, 0, 1, p(3, 4), false, true),
        placed(11, FLIP_DEF, 1, 1, p(3, 3), true, false),
    ]);
    let (d1, events) = attack(&d, 10, p(3, 3));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Battle {
            attack: 400,
            defend: 500,
            defence: true,
            ..
        }
    )));
    assert_eq!(lp(&d1), [3900, 4000]);
    assert!(d1.unit(UnitId(11)).is_some());
    // A face-down attacker's flip at step 2 counts too: 300 -> 600 beats a
    // 400/300 defender in attack position.
    let d = board(vec![
        placed(10, FLIP_DEF, 0, 1, p(3, 4), false, false),
        placed(11, WEAK_ATT, 1, 1, p(3, 3), false, true),
    ]);
    let (d2, _) = attack(&d, 10, p(3, 3));
    assert!(d2.unit(UnitId(11)).is_none());
    assert_eq!(lp(&d2), [4000, 3800]);
    assert_eq!(d2.unit(UnitId(10)).unwrap().position, p(3, 3));
}

#[test]
fn a_monster_flipped_on_its_move_onto_a_card_flips_then_takes_the_cell() {
    // MONSTER-EFFECTS.md §3.1 step 2: flipped face-up during this move, its
    // e3 runs (the card as battle opponent); the set card is removed without
    // a reveal and the attacker advances. Not flipped: no reveal, no effect.
    let d = board(vec![
        placed(10, FLIP_DEF, 0, 1, p(3, 4), false, false),
        placed(11, MAGIC, 1, 6, p(3, 3), false, false),
    ]);
    let (d1, events) = apply(
        &d,
        0,
        DuelCommand::Move {
            unit: UnitId(10),
            path: vec![p(3, 3)],
            face_up: true,
            defence: None,
        },
    );
    assert_eq!(count_events(&events, |e| matches!(e, DuelEvent::Activated { unit: UnitId(10), .. })), 1);
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Revealed { unit: UnitId(11), .. })));
    assert_eq!(bonus(&d1, 10), (300, 300));
    assert!(d1.unit(UnitId(11)).is_none());
    let u = d1.unit(UnitId(10)).unwrap();
    assert!(u.committed_face_up && u.position == p(3, 3));
    let (d2, events) = attack(&d, 10, p(3, 3));
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Activated { .. } | DuelEvent::Revealed { .. })));
    assert_eq!(bonus(&d2, 10), (0, 0));
    let u = d2.unit(UnitId(10)).unwrap();
    assert!(!u.face_up && u.position == p(3, 3) && d2.unit(UnitId(11)).is_none());
    // Filter 0x1A takes the card: the Bug destroys it by its own event (the
    // attacker gets no +6) and advances; the Scorpion binds it, then the
    // attack removes it (+6) [emu tools/flip-card FC1/FC3].
    let flip_onto = |card: u16| {
        let d = board(vec![
            placed(10, card, 0, 1, p(3, 4), false, false),
            placed(11, MAGIC, 1, 6, p(3, 3), false, false),
        ]);
        let before = experience(&d, 0, card);
        let (d1, events) = apply(
            &d,
            0,
            DuelCommand::Move { unit: UnitId(10), path: vec![p(3, 3)], face_up: true, defence: None },
        );
        assert!(events.iter().any(|e| matches!(e, DuelEvent::Activated { unit: UnitId(10), target: Some(UnitId(11)), .. })));
        assert!(d1.unit(UnitId(11)).is_none() && graves(&d1).1 == vec![MAGIC]);
        let u = d1.unit(UnitId(10)).unwrap();
        assert!(u.committed_face_up && u.position == p(3, 3));
        experience(&d1, 0, card) - before
    };
    assert_eq!(flip_onto(BUG), 0);
    assert_eq!(flip_onto(SCORPION), 6);
}

// ---- F02 ---------------------------------------------------------------------

#[test]
fn direct_damage_magic_discards_first_then_hurts_the_opponent() {
    let d = board(vec![placed(10, SPARKS, 0, 6, p(2, 5), false, false)]);
    let (d1, events) = reveal(&d, 10);
    assert_eq!(lp(&d1), [4000, 3500]);
    assert_eq!(graves(&d1), (vec![SPARKS], vec![]));
    let order: Vec<usize> = ["activated", "discarded", "damage"]
        .iter()
        .map(|kind| {
            events
                .iter()
                .position(|e| {
                    matches!(
                        (kind, e),
                        (&"activated", DuelEvent::Activated { .. })
                            | (&"discarded", DuelEvent::Discarded { .. })
                            | (
                                &"damage",
                                DuelEvent::Damage {
                                    side: 1,
                                    amount: 500,
                                },
                            )
                    )
                })
                .unwrap()
        })
        .collect();
    assert!(order[0] < order[1] && order[1] < order[2]);
    assert_eq!(experience(&d1, 0, SPARKS), -4);
    // Down to 0 ends the duel.
    let mut low = d.clone();
    low.sides[1].lp = 300;
    let (d2, events) = reveal(&low, 10);
    assert_eq!(d2.sides[1].lp, 0);
    assert!(matches!(
        events.last(),
        Some(DuelEvent::Ended {
            outcome: Outcome {
                winner: Some(0),
                reason: EndReason::LifePoints
            }
        })
    ));
    // Just Desserts: 300 per unit in the opponent's slots 1..=10, any class
    // or face; nothing at all with no units.
    let d = board(vec![
        placed(10, DESSERTS, 0, 6, p(2, 5), false, false),
        placed(11, WARRIOR, 1, 1, p(3, 1), false, true),
        placed(12, WARRIOR, 1, 2, p(2, 1), true, false),
        placed(13, MAGIC, 1, 6, p(4, 1), false, false),
    ]);
    let (d3, _) = reveal(&d, 10);
    assert_eq!(lp(&d3), [4000, 3100]);
    let d = board(vec![placed(10, DESSERTS, 0, 6, p(2, 5), false, false)]);
    let (d4, events) = reveal(&d, 10);
    assert_eq!(lp(&d4), [4000, 4000]);
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Damage { .. })));
    assert_eq!(graves(&d4), (vec![DESSERTS], vec![]));
}

#[test]
fn goblin_fan_reflects_direct_damage_unless_a_royal_decree_reacts() {
    // The Fan in the victim's group reflects onto the activator; it is
    // revealed, marked acted and stays.
    let d = board(vec![
        placed(10, SPARKS, 0, 6, p(2, 5), false, false),
        placed(11, FAN, 1, 6, p(2, 1), false, false),
    ]);
    let (d1, events) = reveal(&d, 10);
    assert_eq!(lp(&d1), [3500, 4000]);
    let fan = d1.unit(UnitId(11)).unwrap();
    assert!(fan.face_up && fan.committed_face_up && fan.acted);
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Revealed {
            unit: UnitId(11),
            ..
        }
    )));
    assert_eq!(graves(&d1), (vec![SPARKS], vec![]));
    // A Royal Decree in either group reacts instead: the Fan is untouched.
    for decree_side in [0, 1] {
        let d = board(vec![
            placed(10, SPARKS, 0, 6, p(2, 5), false, false),
            placed(11, FAN, 1, 6, p(2, 1), false, false),
            placed(
                12,
                DECREE,
                decree_side,
                7,
                p(4, 1 + 4 * decree_side),
                false,
                false,
            ),
        ]);
        let (d2, _) = reveal(&d, 10);
        assert_eq!(lp(&d2), [4000, 3500]);
        let decree = d2.unit(UnitId(12)).unwrap();
        assert!(decree.face_up && decree.acted);
        assert!(!d2.unit(UnitId(11)).unwrap().face_up);
    }
    // The activator's own Fan and a lone Decree are never consulted.
    let d = board(vec![
        placed(10, SPARKS, 0, 6, p(2, 5), false, false),
        placed(11, FAN, 0, 7, p(4, 5), false, false),
        placed(12, DECREE, 1, 6, p(2, 1), false, false),
    ]);
    let (d3, _) = reveal(&d, 10);
    assert_eq!(lp(&d3), [4000, 3500]);
    assert!(!d3.unit(UnitId(11)).unwrap().face_up && !d3.unit(UnitId(12)).unwrap().face_up);
}

#[test]
fn lp_loss_modifiers_apply_in_side_and_slot_order() {
    // 0x268A10(0x2C): +100 per face-up committed-defence modifier; attack
    // position and face-down sources do not apply. Reached directly since
    // the continuous guard still refuses such a unit on the field.
    let c = cards();
    let d = board(vec![
        placed(10, MODIFIER_2C, 1, 1, p(3, 1), true, true),
        placed(11, MODIFIER_2C, 0, 1, p(3, 5), false, true),
        placed(12, MODIFIER_2C, 0, 2, p(2, 5), true, false),
    ]);
    assert_eq!(d.lp_modifier(0x2C, 500, &c), 600);
    assert_eq!(d.lp_modifier(0x2C, 0, &c), 0);
    assert_eq!(d.lp_modifier(0x2B, 500, &c), 500);
}

// ---- F03 ---------------------------------------------------------------------

#[test]
fn type_5_shields_the_losers_side_from_battle_damage() {
    // Attack-position Kuriboh destroyed: no LP loss, the attacker advances,
    // the winner gets 2 * (cost + 2) instead of 4 * cost.
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, KURIBOH, 1, 1, p(3, 3), false, true),
    ]);
    let (d1, events) = attack(&d, 10, p(3, 3));
    assert_eq!(lp(&d1), [4000, 4000]);
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Damage { .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            unit: UnitId(11),
            target: Some(UnitId(10)),
            ..
        }
    )));
    assert!(d1.unit(UnitId(11)).is_none());
    assert_eq!(d1.unit(UnitId(10)).unwrap().position, p(3, 3));
    assert_eq!(graves(&d1), (vec![], vec![KURIBOH]));
    assert_eq!(experience(&d1, 0, STRONG), 2 * (1 + 2));
    // Control: a plain 100/100 loser costs its side 1700 and yields 4 * cost.
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, 5, 1, 1, p(3, 3), false, true),
    ]);
    let (d2, _) = attack(&d, 10, p(3, 3));
    assert_eq!(lp(&d2), [4000, 2300]);
    assert_eq!(experience(&d2, 0, STRONG), 4);
    // Face-down defence Kuriboh: same shield.
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, KURIBOH, 1, 1, p(3, 3), true, false),
    ]);
    let (d3, _) = attack(&d, 10, p(3, 3));
    assert_eq!(lp(&d3), [4000, 4000]);
    assert!(d3.unit(UnitId(11)).is_none());
    // Kuriboh attacking and losing in attack position: no loss, destroyed.
    let d = board(vec![
        placed(10, KURIBOH, 0, 1, p(3, 4), false, true),
        placed(11, WARRIOR, 1, 1, p(3, 3), false, true),
    ]);
    let (d4, _) = attack(&d, 10, p(3, 3));
    assert_eq!(lp(&d4), [4000, 4000]);
    assert!(d4.unit(UnitId(10)).is_none() && d4.unit(UnitId(11)).is_some());
    // Repelled by DEF: no loss, Kuriboh survives.
    let d = board(vec![
        placed(10, KURIBOH, 0, 1, p(3, 4), false, true),
        placed(11, WARRIOR, 1, 1, p(3, 3), true, true),
    ]);
    let (d5, _) = attack(&d, 10, p(3, 3));
    assert_eq!(lp(&d5), [4000, 4000]);
    assert!(d5.unit(UnitId(10)).is_some());
    // The winner's shield never protects the loser's side.
    let d = board(vec![
        placed(10, SHIELD_STRONG, 0, 1, p(3, 4), false, true),
        placed(11, WARRIOR, 1, 1, p(3, 3), false, true),
    ]);
    let (d6, _) = attack(&d, 10, p(3, 3));
    assert_eq!(lp(&d6), [4000, 3600]);
    assert_eq!(experience(&d6, 0, SHIELD_STRONG), 4 * 4);
    // A direct attack on the leader is not shielded (the leader has slot 0).
    let d = board(vec![placed(10, SHIELD_STRONG, 0, 1, p(3, 1), false, true)]);
    let (d7, _) = attack(&d, 10, p(3, 0));
    assert_eq!(lp(&d7), [4000, 2200]);
}

// ---- F04a --------------------------------------------------------------------

#[test]
fn fiends_hand_takes_its_killer_with_it() {
    // Defender killed: the attacker is removed by the effect (owner's
    // graveyard, -4), no advance, no crush sweep needed here.
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, HAND, 1, 1, p(3, 3), true, false),
    ]);
    let (d1, events) = attack(&d, 10, p(3, 3));
    assert!(d1.unit(UnitId(10)).is_none() && d1.unit(UnitId(11)).is_none());
    assert_eq!(graves(&d1), (vec![STRONG], vec![HAND]));
    assert_eq!(experience(&d1, 0, STRONG), 2 * (2 + 2) - 4);
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            unit: UnitId(11),
            target: Some(UnitId(10)),
            ..
        }
    )));
    assert_eq!(lp(&d1), [4000, 4000]); // DEF broken: no damage
                                       // Attacker killed by a surviving defender: the defender is removed.
    let d = board(vec![
        placed(10, HAND, 0, 1, p(3, 4), false, true),
        placed(11, WARRIOR, 1, 1, p(3, 3), false, true),
    ]);
    let (d2, _) = attack(&d, 10, p(3, 3));
    assert!(d2.unit(UnitId(10)).is_none() && d2.unit(UnitId(11)).is_none());
    assert_eq!(graves(&d2), (vec![HAND], vec![WARRIOR]));
    assert_eq!(lp(&d2), [3200, 4000]);
    // Mutual destruction: neither copy fires.
    let d = board(vec![
        placed(10, HAND, 0, 1, p(3, 4), false, true),
        placed(11, HAND, 1, 1, p(3, 3), false, true),
    ]);
    let (d3, events) = attack(&d, 10, p(3, 3));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Activated { .. })));
    assert_eq!(graves(&d3), (vec![HAND], vec![HAND]));
    assert_eq!(experience(&d3, 0, HAND), 2 * (2 + 2));
    // A flip-effect kill is not a battle death: no effect.
    let d = board(vec![
        placed(10, HAND, 0, 1, p(3, 4), false, true),
        placed(11, KURIBOH, 1, 1, p(3, 3), false, true),
    ]);
    let (d4, _) = attack(&d, 10, p(3, 3));
    assert!(d4.unit(UnitId(10)).is_some());
}

// ---- F04b --------------------------------------------------------------------

#[test]
fn blast_juggler_destroys_its_3x3_square_and_ends_the_battle() {
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, JUGGLER, 1, 1, p(3, 3), true, false),
        placed(12, WARRIOR, 0, 2, p(2, 2), false, true), // diagonal, own side of the attacker
        placed(13, WARRIOR, 0, 3, p(4, 4), true, false), // diagonal, face-down
        placed(14, MAGIC, 1, 6, p(2, 3), false, false),  // set card next to it
        placed(15, WARRIOR, 1, 2, p(3, 1), false, true), // distance 2: survives
        placed(16, DINO, 1, 3, p(4, 3), false, true),
    ]);
    let (d1, events) = attack(&d, 10, p(3, 3));
    for id in [10, 11, 12, 13, 14, 16] {
        assert!(d1.unit(UnitId(id)).is_none(), "unit {id}");
    }
    assert!(d1.unit(UnitId(15)).is_some());
    assert_eq!(d1.units.iter().filter(|u| u.slot == 0).count(), 2);
    // Centre first, then column by column: side 0 (2,2), (3,4), (4,4).
    assert_eq!(
        graves(&d1),
        (vec![WARRIOR, STRONG, WARRIOR], vec![JUGGLER, MAGIC, DINO])
    );
    assert_eq!(experience(&d1, 1, JUGGLER), -4);
    assert_eq!(experience(&d1, 0, STRONG), 2 * (3 + 2) - 4);
    assert!(!events.iter().any(|e| matches!(
        e,
        DuelEvent::Revealed {
            unit: UnitId(13),
            ..
        }
    )));
    assert_eq!(lp(&d1), [4000, 4000]);
    // Off-board cells are skipped; the leader survives an adjacent blast.
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 2), false, true),
        placed(11, JUGGLER, 1, 1, p(3, 1), false, true),
    ]);
    let (d2, _) = attack(&d, 10, p(3, 1));
    assert!(d2.unit(UnitId(10)).is_none() && d2.unit(UnitId(11)).is_none());
    assert!(d2.unit(UnitId(1)).is_some());
    assert_eq!(lp(&d2), [4000, 3100]);
    // As a dying attacker it blows up around its own cell, defender included.
    let d = board(vec![
        placed(10, JUGGLER, 0, 1, p(3, 4), false, true),
        placed(11, WARRIOR, 1, 1, p(3, 3), false, true),
        placed(12, WARRIOR, 1, 2, p(3, 2), false, true),
    ]);
    let (d3, _) = attack(&d, 10, p(3, 3));
    assert!(d3.unit(UnitId(10)).is_none() && d3.unit(UnitId(11)).is_none());
    assert!(d3.unit(UnitId(12)).is_some());
    assert_eq!(lp(&d3), [3500, 4000]);
    // Killed by a flip effect: no explosion.
    let d = board(vec![
        placed(10, JUGGLER, 0, 1, p(3, 4), false, true),
        placed(11, KURIBOH, 1, 1, p(3, 3), false, true),
        placed(12, WARRIOR, 1, 2, p(2, 3), false, true),
    ]);
    let (d4, _) = attack(&d, 10, p(3, 3));
    assert!(d4.unit(UnitId(12)).is_some() && d4.unit(UnitId(10)).is_some());
}

#[test]
fn hinotama_soul_burns_the_cards_around_the_attack_target() {
    // Defender: non-monster cards at Manhattan distance 1 around its cell;
    // monsters, diagonals and distance 2 survive; the attacker advances.
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, SOUL, 1, 1, p(3, 3), true, false),
        placed(12, MAGIC, 1, 6, p(2, 3), false, false),
        placed(13, MAGIC, 0, 6, p(3, 2), false, false),
        placed(14, MAGIC, 1, 7, p(2, 2), false, false), // diagonal
        placed(15, MAGIC, 1, 8, p(5, 3), false, false), // distance 2
        placed(16, WARRIOR, 1, 2, p(4, 3), false, true),
    ]);
    let (d1, events) = attack(&d, 10, p(3, 3));
    assert!(d1.unit(UnitId(12)).is_none() && d1.unit(UnitId(13)).is_none());
    for id in [14, 15, 16] {
        assert!(d1.unit(UnitId(id)).is_some(), "unit {id}");
    }
    assert_eq!(d1.unit(UnitId(10)).unwrap().position, p(3, 3));
    // Victims (side 0 first) precede the Soul in the graveyards.
    assert_eq!(graves(&d1), (vec![MAGIC], vec![MAGIC, SOUL]));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            unit: UnitId(11),
            target: None,
            ..
        }
    )));
    // As the attacker the area is centred on the defender's cell.
    let d = board(vec![
        placed(10, SOUL, 0, 1, p(3, 4), false, true),
        placed(11, WARRIOR, 1, 1, p(3, 3), false, true),
        placed(12, MAGIC, 1, 6, p(3, 2), false, false), // two cells from the Soul
        placed(13, MAGIC, 1, 7, p(2, 4), false, false), // adjacent to the Soul only
    ]);
    let (d2, _) = attack(&d, 10, p(3, 3));
    assert!(d2.unit(UnitId(10)).is_none());
    assert!(d2.unit(UnitId(12)).is_none() && d2.unit(UnitId(13)).is_some());
    // No target: no event, ordinary death.
    let d = board(vec![
        placed(10, SOUL, 0, 1, p(3, 4), false, true),
        placed(11, WARRIOR, 1, 1, p(3, 3), false, true),
    ]);
    let (d3, events) = attack(&d, 10, p(3, 3));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Activated { .. })));
    assert_eq!(graves(&d3), (vec![SOUL], vec![]));
}

// ---- F05a / F05b -------------------------------------------------------------

#[test]
fn sectarian_of_secrets_weakens_its_killer_permanently() {
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, SECTARIAN, 1, 1, p(3, 3), true, false),
    ]);
    let (d1, events) = attack(&d, 10, p(3, 3));
    assert_eq!(bonus(&d1, 10), (-300, -300));
    assert_eq!(d1.unit(UnitId(10)).unwrap().position, p(3, 3));
    assert_eq!(graves(&d1), (vec![], vec![SECTARIAN]));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Modified {
            unit: UnitId(10),
            attack: -300,
            defense: -300
        }
    )));
    // Stacks with earlier deltas, as a destroyed attacker too.
    let mut killer = placed(11, WARRIOR, 1, 1, p(3, 3), false, true);
    killer.attack_bonus = -600;
    let d = board(vec![
        placed(10, SECTARIAN, 0, 1, p(3, 4), false, true),
        killer,
    ]);
    let (d2, _) = attack(&d, 10, p(3, 3));
    assert_eq!(bonus(&d2, 11), (-900, -300));
    assert!(d2.unit(UnitId(10)).is_none());
    // Floor: a killer whose current stat is below 300 reads exactly 0.
    let mut weak_killer = placed(10, WARRIOR, 0, 1, p(3, 4), false, true);
    weak_killer.attack_bonus = -1200; // 200 ATK beats DEF 100
    let d = board(vec![
        weak_killer,
        placed(11, SECTARIAN_WEAK, 1, 1, p(3, 3), true, false),
    ]);
    let (d3, _) = attack(&d, 10, p(3, 3));
    assert_eq!(bonus(&d3, 10), (-1400, -300));
    // Mutual destruction: nothing.
    let d = board(vec![
        placed(10, SECTARIAN, 0, 1, p(3, 4), false, true),
        placed(11, SECTARIAN, 1, 1, p(3, 3), false, true),
    ]);
    let (_, events) = attack(&d, 10, p(3, 3));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Modified { .. })));
}

#[test]
fn korogashi_weakens_every_monster_in_its_row_and_column() {
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(3, 4), false, true),
        placed(11, KOROGASHI, 1, 1, p(3, 3), true, false),
        placed(12, WARRIOR, 1, 2, p(0, 3), false, false), // row, face-down
        placed(13, WARRIOR, 0, 2, p(3, 1), true, true),   // column, own side of the killer
        placed(14, MAGIC, 1, 6, p(6, 3), false, false),   // a card in the row
        placed(15, WARRIOR, 1, 3, p(4, 4), false, true),  // off the cross
    ]);
    let (d1, events) = attack(&d, 10, p(3, 3));
    assert_eq!(bonus(&d1, 10), (-300, -300));
    assert_eq!(bonus(&d1, 12), (-300, -300));
    assert_eq!(bonus(&d1, 13), (-300, -300));
    assert_eq!(bonus(&d1, 15), (0, 0));
    assert_eq!(bonus(&d1, 1), (0, 0)); // the leader in column 3
    assert!(d1.unit(UnitId(14)).is_some());
    assert!(!d1.unit(UnitId(12)).unwrap().face_up);
    assert_eq!(
        count_events(&events, |e| matches!(e, DuelEvent::Modified { .. })),
        3
    );
    assert_eq!(d1.unit(UnitId(10)).unwrap().position, p(3, 3));
    // Mutual destruction: the attacker is excluded from the defender's
    // cross, but the still-present defender is in the attacker's.
    let d = board(vec![
        placed(10, KOROGASHI, 0, 1, p(3, 4), false, true),
        placed(11, KOROGASHI, 1, 1, p(3, 3), false, true),
        placed(12, WARRIOR, 1, 2, p(0, 4), false, true), // the attacker's row only
        placed(13, WARRIOR, 0, 2, p(0, 3), false, true), // the defender's row only
    ]);
    let (d2, events) = attack(&d, 10, p(3, 3));
    assert_eq!(bonus(&d2, 12), (-300, -300));
    assert_eq!(bonus(&d2, 13), (-300, -300));
    assert_eq!(
        count_events(&events, |e| matches!(e, DuelEvent::Activated { .. })),
        2
    );
    assert!(d2.unit(UnitId(10)).is_none() && d2.unit(UnitId(11)).is_none());
}

// ---- F06 ---------------------------------------------------------------------

#[test]
fn magic_jammer_negates_magic_and_rituals_of_the_other_group() {
    // The Jammer (any face, distance) fires first: Jammer then card to their
    // graveyards, -4 each, no effect.
    let d = board(vec![
        placed(10, SPARKS, 0, 6, p(2, 5), false, false),
        placed(11, JAMMER, 1, 6, p(6, 0), false, false),
    ]);
    let (d1, events) = reveal(&d, 10);
    assert_eq!(lp(&d1), [4000, 4000]);
    assert_eq!(graves(&d1), (vec![SPARKS], vec![JAMMER]));
    assert_eq!(
        (experience(&d1, 0, SPARKS), experience(&d1, 1, JAMMER)),
        (-4, -4)
    );
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            unit: UnitId(11),
            target: Some(UnitId(10)),
            ..
        }
    )));
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Damage { .. })));
    assert_eq!(d1.active, 0);
    // An own-group Jammer never fires.
    let d = board(vec![
        placed(10, SPARKS, 0, 6, p(2, 5), false, false),
        placed(11, JAMMER, 0, 7, p(4, 5), false, false),
    ]);
    let (d2, _) = reveal(&d, 10);
    assert_eq!(lp(&d2), [4000, 3500]);
    assert!(d2.unit(UnitId(11)).is_some());
    // Two Jammers: the oldest (lowest summon order) fires, face-up or not.
    let mut old = placed(12, JAMMER, 1, 8, p(5, 0), false, true);
    old.summon_order = 1;
    let mut young = placed(11, JAMMER, 1, 6, p(6, 0), false, false);
    young.summon_order = 5;
    let d = board(vec![
        placed(10, SPARKS, 0, 6, p(2, 5), false, false),
        young,
        old,
    ]);
    let (d3, _) = reveal(&d, 10);
    assert!(d3.unit(UnitId(12)).is_none() && d3.unit(UnitId(11)).is_some());
    // A Royal Decree in either group reacts instead: revealed, acted, the
    // Jammer untouched, the effect applies.
    for decree_side in [0, 1] {
        let d = board(vec![
            placed(10, SPARKS, 0, 6, p(2, 5), false, false),
            placed(11, JAMMER, 1, 6, p(6, 0), false, false),
            placed(
                12,
                DECREE,
                decree_side,
                7,
                p(0, 1 + 4 * decree_side),
                false,
                false,
            ),
        ]);
        let (d4, _) = reveal(&d, 10);
        assert_eq!(lp(&d4), [4000, 3500]);
        let decree = d4.unit(UnitId(12)).unwrap();
        assert!(decree.face_up && decree.committed_face_up && decree.acted);
        assert!(d4.unit(UnitId(11)).is_some_and(|u| !u.face_up));
    }
    // A ritual is negated before its requirement table is consulted.
    let d = board(vec![
        placed(10, RITUAL, 0, 6, p(2, 5), false, false),
        placed(11, JAMMER, 1, 6, p(6, 0), false, false),
    ]);
    let (d5, _) = reveal(&d, 10);
    assert_eq!(graves(&d5), (vec![RITUAL], vec![JAMMER]));
    // Without a Jammer the same ritual is still refused (no table).
    let d = board(vec![placed(10, RITUAL, 0, 6, p(2, 5), false, false)]);
    assert!(matches!(try_reveal(&d, 10), Err(DuelError::Unsupported(_))));
    // The CPU side's card against the player's Jammer: the same hook.
    let d = board(vec![
        placed(10, SPARKS, 1, 6, p(2, 1), false, false),
        placed(11, JAMMER, 0, 6, p(6, 6), false, false),
    ]);
    let (d6, _) = apply(&d, 0, DuelCommand::EndTurn);
    let (d7, _) = d6
        .apply(
            1,
            &DuelCommand::Move {
                unit: UnitId(10),
                path: vec![],
                face_up: true,
                defence: None,
            },
            &cards(),
        )
        .unwrap();
    assert_eq!(lp(&d7), [4000, 4000]);
    assert_eq!(graves(&d7), (vec![JAMMER], vec![SPARKS]));
}

#[test]
fn an_ancient_tree_blocks_every_full_range_trap_search() {
    // 0x268710(0xF) inside 0x26C5C0: reached directly, since the continuous
    // guard still refuses a face-up defence Tree on the field.
    let c = cards();
    let d = board(vec![
        placed(10, JAMMER, 1, 6, p(6, 0), false, false),
        placed(11, TREE, 0, 1, p(3, 5), true, true),
    ]);
    assert!(d.far_trap(1, trap_type::MAGIC_JAMMER, &c).is_none());
    let mut d2 = d.clone();
    d2.units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .face_up = false;
    d2.units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .committed_face_up = false;
    assert_eq!(
        d2.far_trap(1, trap_type::MAGIC_JAMMER, &c),
        Some(UnitId(10))
    );
}

// ---- Pegasus's deck: control changes and graveyard copies -------------------

fn pick(d: &Duel, side: u8, pick: Pick) -> (Duel, Vec<DuelEvent>) {
    apply(d, side, DuelCommand::Pick { pick })
}

fn unit(d: &Duel, id: u16) -> &DuelUnit {
    d.unit(UnitId(id)).unwrap()
}

#[test]
fn change_of_heart_lists_the_enemy_group_and_steals_the_pick_until_the_next_turn() {
    // Enemy group: a face-up Warrior in slot 1, a face-down Dinosaur in
    // slot 3, a set magic card in slot 6; an own Reptile stands too.
    let d = board(vec![
        placed(10, COH, 0, 6, p(2, 5), false, false),
        placed(11, WARRIOR, 1, 1, p(3, 1), false, true),
        placed(12, DINO, 1, 3, p(2, 1), false, false),
        placed(13, MAGIC, 1, 6, p(4, 1), false, false),
        placed(14, REPTILE, 0, 1, p(3, 5), false, true),
    ]);
    let (d1, events) = reveal(&d, 10);
    assert_eq!(
        d1.pending,
        Some(Pending::ControlChange {
            by: 0,
            units: vec![UnitId(11), UnitId(12), UnitId(13)],
        })
    );
    assert_eq!(graves(&d1), (vec![COH], vec![]));
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::SelectionOpened { card: COH })));
    // Nothing else is accepted while the selection is open.
    assert!(matches!(
        d1.apply(0, &DuelCommand::EndTurn, &cards()),
        Err(DuelError::InvalidCommand(_))
    ));
    assert!(matches!(
        d1.apply(
            0,
            &DuelCommand::Pick {
                pick: Pick::Unit { unit: UnitId(14) }
            },
            &cards()
        ),
        Err(DuelError::InvalidCommand(_))
    ));
    // The pick: the Dinosaur (acted, as every enemy unit) now answers to
    // side 0 and may act this turn.
    let (d2, events) = pick(&d1, 0, Pick::Unit { unit: UnitId(12) });
    assert_eq!(d2.pending, None);
    let dino = unit(&d2, 12);
    assert!(dino.stolen && !dino.acted && dino.side == 1 && dino.controller() == 0);
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Controlled {
            unit: UnitId(12),
            side: 0
        }
    )));
    // It can be moved by side 0 like an own unit, and the enemy's turn
    // start returns it (control cleared before the acted reset).
    let (d3, _) = apply(
        &d2,
        0,
        DuelCommand::Move {
            unit: UnitId(12),
            path: vec![p(2, 2)],
            face_up: false,
            defence: None,
        },
    );
    assert_eq!(unit(&d3, 12).position, p(2, 2));
    let (d4, _) = apply(&d3, 0, DuelCommand::EndTurn);
    assert_eq!(d4.active, 1);
    let dino = unit(&d4, 12);
    assert!(!dino.stolen && !dino.acted && dino.controller() == 1);
    assert!(unit(&d4, 14).acted);
    // A spellbound pick keeps its acted flag [emu coh_bound].
    let mut bound = d.clone();
    bound
        .units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .spellbind = 2;
    let (b1, _) = reveal(&bound, 10);
    let (b2, _) = pick(&b1, 0, Pick::Unit { unit: UnitId(11) });
    let warrior = unit(&b2, 11);
    assert!(warrior.stolen && warrior.acted);
    // Circle: the card is spent for nothing.
    let (c1, events) = pick(&d1, 0, Pick::Cancel);
    assert_eq!(c1.pending, None);
    assert!(c1.units.iter().all(|u| !u.stolen));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Controlled { .. })));
    // A graveyard answer does not fit a unit selection.
    assert!(matches!(
        d1.apply(
            0,
            &DuelCommand::Pick {
                pick: Pick::Graveyard { side: 0, index: 0 }
            },
            &cards()
        ),
        Err(DuelError::InvalidCommand(_))
    ));
    // No enemy-controlled unit at all: nothing opens, the card is spent.
    let empty = board(vec![placed(10, COH, 0, 6, p(2, 5), false, false)]);
    let (e1, events) = reveal(&empty, 10);
    assert_eq!(e1.pending, None);
    assert_eq!(graves(&e1), (vec![COH], vec![]));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::SelectionOpened { .. })));
}

#[test]
fn a_control_change_ends_before_the_turn_start_effects_run() {
    // Side 0 steals side 1's Dinosaur; at side 1's turn start the
    // control-changed bits clear first (state 0x17, MOTH-EFFECTS.md §6.3),
    // so its face-up defence Weakener (entry-2 type 0x4D) spares the
    // Dinosaur and lowers side 0's Reptile only.
    let d = board(vec![
        placed(10, COH, 0, 6, p(2, 5), false, false),
        placed(11, WEAKENER, 1, 1, p(3, 1), true, true),
        placed(12, DINO, 1, 2, p(2, 1), false, false),
        placed(14, REPTILE, 0, 1, p(3, 5), false, true),
    ]);
    let (d1, _) = reveal(&d, 10);
    let (d2, _) = pick(&d1, 0, Pick::Unit { unit: UnitId(12) });
    assert!(unit(&d2, 12).stolen);
    let (d3, events) = apply(&d2, 0, DuelCommand::EndTurn);
    assert_eq!(d3.active, 1);
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            unit: UnitId(11),
            ..
        }
    )));
    let dino = unit(&d3, 12);
    assert!(!dino.stolen && !dino.acted);
    assert_eq!((dino.attack_bonus, dino.defense_bonus), (0, 0));
    assert_eq!(
        (unit(&d3, 14).attack_bonus, unit(&d3, 14).defense_bonus),
        (-100, -100)
    );
}

#[test]
fn a_celtic_guardian_in_defence_destroys_a_control_change_without_effect() {
    let d = board(vec![
        placed(10, COH, 0, 6, p(2, 5), false, false),
        placed(11, WARRIOR, 1, 1, p(3, 1), false, true),
        placed(12, CELTIC, 0, 1, p(4, 5), true, true),
    ]);
    let (d1, events) = reveal(&d, 10);
    assert_eq!(d1.pending, None);
    assert_eq!(graves(&d1), (vec![COH], vec![]));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::SelectionOpened { .. })));
    // Brain Control alike.
    let mut b = d.clone();
    b.units
        .iter_mut()
        .find(|u| u.id == UnitId(10))
        .unwrap()
        .card = BRAIN;
    let (b1, events) = reveal(&b, 10);
    assert!(b1.units.iter().all(|u| !u.stolen));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Controlled { .. })));
}

#[test]
fn brain_control_takes_the_strongest_enemy_monster_at_once() {
    let d = board(vec![
        placed(10, BRAIN, 0, 6, p(2, 5), false, false),
        placed(11, WARRIOR, 1, 1, p(3, 1), false, true),
        placed(12, STRONG, 1, 2, p(2, 1), false, false),
        placed(13, DRAGON, 1, 3, p(4, 1), true, false),
        placed(14, MAGIC, 1, 6, p(5, 1), false, false),
    ]);
    let (d1, events) = reveal(&d, 10);
    assert_eq!(d1.pending, None);
    assert_eq!(graves(&d1), (vec![BRAIN], vec![]));
    let strong = unit(&d1, 12);
    assert!(strong.stolen && !strong.acted && strong.controller() == 0);
    assert_eq!(
        count_events(&events, |e| matches!(e, DuelEvent::Controlled { .. })),
        1
    );
    assert_eq!(d1.units.iter().filter(|u| u.stolen).count(), 1);
    // Equal ATK and DEF: the lower summon order wins.
    let mut tie = board(vec![
        placed(10, BRAIN, 0, 6, p(2, 5), false, false),
        placed(11, STRONG, 1, 1, p(3, 1), false, true),
        placed(12, STRONG, 1, 2, p(2, 1), false, false),
    ]);
    tie.units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .summon_order = 7;
    tie.units
        .iter_mut()
        .find(|u| u.id == UnitId(12))
        .unwrap()
        .summon_order = 3;
    let (t1, _) = reveal(&tie, 10);
    assert!(unit(&t1, 12).stolen && !unit(&t1, 11).stolen);
    // An exact tie (same order): 0x24F28C replaces the best only when the
    // best's group equals the mode (2 here, never), so the first in list
    // order (group 1 slot 1) stays (REFUSALS-SWEEP.md §strongest).
    tie.units
        .iter_mut()
        .find(|u| u.id == UnitId(12))
        .unwrap()
        .summon_order = 7;
    let (t2, _) = reveal(&tie, 10);
    assert!(unit(&t2, 11).stolen && !unit(&t2, 12).stolen);
    // No enemy monster: nothing happens, the card is spent.
    let none = board(vec![
        placed(10, BRAIN, 0, 6, p(2, 5), false, false),
        placed(14, MAGIC, 1, 6, p(5, 1), false, false),
    ]);
    let (n1, events) = reveal(&none, 10);
    assert!(n1.units.iter().all(|u| !u.stolen));
    assert_eq!(graves(&n1), (vec![BRAIN], vec![]));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Controlled { .. })));
}

#[test]
fn mimicat_creates_the_picked_graveyard_card_on_its_own_cell() {
    let mut d = board(vec![placed(10, MIMIC, 0, 6, p(2, 5), false, false)]);
    d.sides[0].graveyard = vec![WARRIOR, MAGIC, WARRIOR];
    d.sides[1].graveyard = vec![DINO];
    let (d1, _) = reveal(&d, 10);
    assert_eq!(
        d1.pending,
        Some(Pending::GraveyardCopy {
            by: 0,
            at: p(2, 5),
            graveyards: vec![0, 1],
            monsters: false,
            non_monsters: false,
        })
    );
    // Mimicat itself is already in the graveyard and could be picked.
    assert_eq!(
        graves(&d1),
        (vec![WARRIOR, MAGIC, WARRIOR, MIMIC], vec![DINO])
    );
    // The second Warrior: a fresh face-down unit in the first free monster
    // slot, not acted; the FIRST Warrior entry leaves the graveyard.
    let order = d1.sides[0].summons;
    let (d2, events) = pick(&d1, 0, Pick::Graveyard { side: 0, index: 2 });
    assert_eq!(d2.pending, None);
    let copy = d2.unit_at(p(2, 5)).unwrap();
    assert_eq!((copy.card, copy.side, copy.slot), (WARRIOR, 0, 1));
    assert!(!copy.face_up && !copy.defence && !copy.acted && !copy.stolen);
    assert_eq!(copy.summon_order, order);
    assert_eq!(d2.sides[0].summons, order + 1);
    assert_eq!(graves(&d2), (vec![MAGIC, WARRIOR, MIMIC], vec![DINO]));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Revived {
            card: WARRIOR,
            from: 0,
            ..
        }
    )));
    // The enemy's graveyard: the card becomes side 0's unit.
    let (d3, _) = pick(&d1, 0, Pick::Graveyard { side: 1, index: 0 });
    let copy = d3.unit_at(p(2, 5)).unwrap();
    assert_eq!((copy.card, copy.side, copy.controller()), (DINO, 0, 0));
    assert_eq!(graves(&d3), (vec![WARRIOR, MAGIC, WARRIOR, MIMIC], vec![]));
    // A non-monster card lands in the first free slot 6..10 with the
    // acted bit set [emu mimicat_d: Insect Armor f57 = 0x01].
    let (d4, _) = pick(&d1, 0, Pick::Graveyard { side: 0, index: 1 });
    let copy = d4.unit_at(p(2, 5)).unwrap();
    assert_eq!((copy.card, copy.slot), (MAGIC, 6));
    assert!(copy.acted);
    // Circle: nothing, the card stays spent.
    let (d5, _) = pick(&d1, 0, Pick::Cancel);
    assert!(d5.unit_at(p(2, 5)).is_none());
    assert_eq!(graves(&d5), graves(&d1));
    // Five own monsters already: the pick does nothing (999 at 0x25873C).
    let mut full = d.clone();
    for (i, col) in (0..5u8).zip([0, 1, 3, 4, 5]) {
        full.units.push(placed(
            20 + u16::from(i),
            WARRIOR,
            0,
            i + 1,
            p(col, 4),
            false,
            true,
        ));
    }
    let (f1, _) = reveal(&full, 10);
    let (f2, events) = pick(&f1, 0, Pick::Graveyard { side: 0, index: 0 });
    assert_eq!(f2.pending, None);
    assert!(f2.unit_at(p(2, 5)).is_none());
    assert_eq!(graves(&f2), graves(&f1));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Revived { .. })));
    // Out-of-range answers are refused.
    assert!(matches!(
        d1.apply(
            0,
            &DuelCommand::Pick {
                pick: Pick::Graveyard { side: 1, index: 3 }
            },
            &cards()
        ),
        Err(DuelError::InvalidCommand(_))
    ));
    // Monster Reborn: monsters only; without any, nothing opens.
    let mut r = d.clone();
    r.units
        .iter_mut()
        .find(|u| u.id == UnitId(10))
        .unwrap()
        .card = REBORN;
    let (r1, _) = reveal(&r, 10);
    assert!(matches!(
        r1.pending,
        Some(Pending::GraveyardCopy { monsters: true, .. })
    ));
    assert!(matches!(
        r1.apply(
            0,
            &DuelCommand::Pick {
                pick: Pick::Graveyard { side: 0, index: 1 }
            },
            &cards()
        ),
        Err(DuelError::InvalidCommand(_))
    ));
    r.sides[0].graveyard = vec![MAGIC];
    r.sides[1].graveyard = vec![];
    let (r2, _) = reveal(&r, 10);
    assert_eq!(r2.pending, None);
    assert_eq!(graves(&r2), (vec![MAGIC, REBORN], vec![]));
    // Mimicat over two empty graveyards: the builder counts before its
    // own discard (0x254C74 < 0x254CE4), so nothing opens and the card is
    // spent; with one other card the list holds both and Mimicat can copy
    // itself [emu mimicat_d13].
    let mut e = d.clone();
    e.sides[0].graveyard = vec![];
    e.sides[1].graveyard = vec![];
    let (e1, events) = reveal(&e, 10);
    assert_eq!(e1.pending, None);
    assert_eq!(graves(&e1), (vec![MIMIC], vec![]));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::SelectionOpened { .. })));
    e.sides[1].graveyard = vec![DINO];
    let (e2, _) = reveal(&e, 10);
    assert!(e2.pending.is_some());
    let (e3, _) = pick(&e2, 0, Pick::Graveyard { side: 0, index: 0 });
    let copy = e3.unit_at(p(2, 5)).unwrap();
    assert_eq!((copy.card, copy.slot), (MIMIC, 6));
    assert!(copy.acted);
    assert_eq!(graves(&e3), (vec![], vec![DINO]));
}

#[test]
fn the_unit_pick_menu_walks_the_list_with_raw_keys_and_wraps() {
    use ai::pad::*;
    let c = cards();
    let d = board(vec![
        placed(10, COH, 0, 6, p(2, 5), false, false),
        placed(11, WARRIOR, 1, 1, p(3, 1), false, true),
        placed(12, DINO, 1, 3, p(2, 1), false, false),
        placed(13, MAGIC, 1, 6, p(4, 1), false, false),
    ]);
    let mut input = ui::DuelUi::new(&d);
    input.cursors[0] = p(2, 5);
    assert_eq!(input.press(&d, &c, CROSS).unwrap(), ui::Response::Moved);
    assert_eq!(input.press(&d, &c, R2).unwrap(), ui::Response::Moved);
    let ui::Response::Commands(cmds) = input.press(&d, &c, CROSS).unwrap() else {
        panic!("the activation command");
    };
    let (d1, _) = apply(&d, 0, cmds[0].clone());
    assert!(d1.pending.is_some());
    // The first press after the activation finds the menu open on the
    // first entry; Down walks forward, Up back with wrap-around; the
    // cursor follows (no mirroring even for side 1).
    assert_eq!(input.press(&d1, &c, DOWN).unwrap(), ui::Response::Moved);
    assert_eq!(input.state(), 0x30);
    assert_eq!(input.cursors[0], p(2, 1));
    assert_eq!(input.press(&d1, &c, UP).unwrap(), ui::Response::Moved);
    assert_eq!(input.press(&d1, &c, UP).unwrap(), ui::Response::Moved);
    assert_eq!(input.cursors[0], p(4, 1));
    assert_eq!(input.press(&d1, &c, LEFT).unwrap(), ui::Response::Moved);
    assert_eq!(input.cursors[0], p(3, 1));
    assert_eq!(
        input.press(&d1, &c, CROSS).unwrap(),
        ui::Response::Commands(vec![DuelCommand::Pick {
            pick: Pick::Unit { unit: UnitId(11) }
        }])
    );
    assert_eq!(input.state(), 0x18);
    // Circle cancels; a menu without a pending selection is a mismatch.
    let mut input2 = ui::DuelUi::new(&d1);
    assert_eq!(
        input2.press(&d1, &c, CIRCLE).unwrap(),
        ui::Response::Commands(vec![DuelCommand::Pick { pick: Pick::Cancel }])
    );
    let mut stale = ui::DuelUi::new(&d1);
    let _ = stale.press(&d1, &c, DOWN).unwrap();
    assert!(stale.press(&d, &c, DOWN).is_err());
    // Side 1 acting: Down is still "forward" (0x1D5AF8 reads the raw word).
    let mut d_side1 = board(vec![
        placed(10, COH, 1, 6, p(4, 1), false, false),
        placed(11, WARRIOR, 0, 1, p(3, 5), false, true),
        placed(12, DINO, 0, 3, p(2, 5), false, false),
    ]);
    d_side1.active = 1;
    d_side1.units.iter_mut().for_each(|u| u.acted = u.side == 0);
    let (s1, _) = apply(
        &d_side1,
        1,
        DuelCommand::Move {
            unit: UnitId(10),
            path: vec![],
            face_up: true,
            defence: None,
        },
    );
    let mut input = ui::DuelUi::new(&s1);
    assert_eq!(input.press(&s1, &c, DOWN).unwrap(), ui::Response::Moved);
    assert_eq!(input.cursors[1], p(2, 5));
}

#[test]
fn the_graveyard_pick_menu_scrolls_a_five_row_window_and_switches_sides() {
    use ai::pad::*;
    let c = cards();
    let mut d = board(vec![placed(10, MIMIC, 0, 6, p(2, 5), false, false)]);
    d.sides[0].graveyard = vec![WARRIOR, DINO, REPTILE, SPELL, DRAGON, STRONG];
    d.sides[1].graveyard = vec![MAGIC, WARRIOR];
    let (d1, _) = reveal(&d, 10);
    let mut input = ui::DuelUi::new(&d1);
    // Up at the top does nothing; Down walks rows 0..4, then scrolls.
    assert_eq!(input.press(&d1, &c, UP).unwrap(), ui::Response::Ignored);
    assert_eq!(input.state(), 0x2D);
    for _ in 0..4 {
        assert_eq!(input.press(&d1, &c, DOWN).unwrap(), ui::Response::Moved);
    }
    let ui::Mode::Graveyard(g) = &input.mode else {
        panic!("graveyard menu");
    };
    assert_eq!((g.shown, g.locked, g.row[0], g.scroll[0]), (0, false, 4, 0));
    assert_eq!(input.press(&d1, &c, DOWN).unwrap(), ui::Response::Moved);
    assert_eq!(input.press(&d1, &c, DOWN).unwrap(), ui::Response::Moved);
    let ui::Mode::Graveyard(g) = &input.mode else {
        panic!("graveyard menu");
    };
    // Seven entries (Mimicat joined its graveyard): the window slid twice.
    assert_eq!((g.row[0], g.scroll[0]), (4, 2));
    assert_eq!(g.selected(), Some(6));
    assert_eq!(input.press(&d1, &c, DOWN).unwrap(), ui::Response::Ignored);
    // Right shows side 1's list; the row carries over, clamped.
    assert_eq!(input.press(&d1, &c, RIGHT).unwrap(), ui::Response::Moved);
    let ui::Mode::Graveyard(g) = &input.mode else {
        panic!("graveyard menu");
    };
    assert_eq!((g.shown, g.row[1], g.scroll[1]), (1, 1, 0));
    assert_eq!(input.press(&d1, &c, RIGHT).unwrap(), ui::Response::Ignored);
    assert_eq!(
        input.press(&d1, &c, CROSS).unwrap(),
        ui::Response::Commands(vec![DuelCommand::Pick {
            pick: Pick::Graveyard { side: 1, index: 1 }
        }])
    );
    // L1/R1 page by five; Left returns to side 0's list.
    let mut input = ui::DuelUi::new(&d1);
    assert_eq!(input.press(&d1, &c, R1).unwrap(), ui::Response::Moved);
    assert_eq!(input.press(&d1, &c, R1).unwrap(), ui::Response::Ignored);
    let ui::Mode::Graveyard(g) = &input.mode else {
        panic!("graveyard menu");
    };
    assert_eq!((g.row[0], g.scroll[0]), (0, 2));
    assert_eq!(input.press(&d1, &c, L1).unwrap(), ui::Response::Moved);
    assert_eq!(input.press(&d1, &c, RIGHT).unwrap(), ui::Response::Moved);
    assert_eq!(input.press(&d1, &c, LEFT).unwrap(), ui::Response::Moved);
    assert_eq!(
        input.press(&d1, &c, CIRCLE).unwrap(),
        ui::Response::Commands(vec![DuelCommand::Pick { pick: Pick::Cancel }])
    );
    // Monster Reborn with the enemy's graveyard empty of monsters: Cross
    // on that empty list is refused (error sound), not a pick.
    let mut r = d.clone();
    r.units
        .iter_mut()
        .find(|u| u.id == UnitId(10))
        .unwrap()
        .card = REBORN;
    r.sides[1].graveyard = vec![MAGIC];
    let (r1, _) = reveal(&r, 10);
    let mut input = ui::DuelUi::new(&r1);
    assert_eq!(input.press(&r1, &c, RIGHT).unwrap(), ui::Response::Moved);
    assert_eq!(input.press(&r1, &c, CROSS).unwrap(), ui::Response::Refused);
}

#[test]
fn computer_activations_press_the_menu_keys_of_0x277bd0() {
    use ai::pad::*;
    use ai::Action;
    let c = cards();
    let d = board(vec![placed(10, MIMIC, 0, 6, p(2, 5), false, false)]);
    let mimicat = |graveyard, down| Action::Activate {
        unit: UnitId(10),
        at: p(2, 5),
        graveyard: Some(graveyard),
        down,
    };
    // Side 0 wanting side 1's graveyard: Right; its own: no switch.
    assert_eq!(
        ai::keys(&d, 0, p(2, 6), &mimicat(1, 2), &c),
        [UP, CROSS, R2, CROSS, RIGHT, DOWN, DOWN, CROSS]
    );
    assert_eq!(
        ai::keys(&d, 0, p(2, 5), &mimicat(0, 0), &c),
        [CROSS, R2, CROSS, CROSS]
    );
    // Side 1 wanting side 0's graveyard: Left (0x278340).
    assert_eq!(
        ai::keys(&d, 1, p(2, 5), &mimicat(0, 1), &c),
        [CROSS, R2, CROSS, LEFT, DOWN, CROSS]
    );
    let coh = Action::Activate {
        unit: UnitId(10),
        at: p(2, 5),
        graveyard: None,
        down: 3,
    };
    assert_eq!(
        ai::keys(&d, 1, p(2, 5), &coh, &c),
        [CROSS, R2, CROSS, DOWN, DOWN, DOWN, CROSS]
    );
    assert_eq!(ai::cursor_after(&coh), p(2, 5));
}

// ---- ROUTE-3-5 ---------------------------------------------------------------

fn move_cmd(unit: u16, path: Vec<GridPos>, defence: Option<bool>) -> DuelCommand {
    DuelCommand::Move {
        unit: UnitId(unit),
        path,
        face_up: false,
        defence,
    }
}

#[test]
fn shadow_ghoul_turns_into_wall_shadow_on_a_labyrinth_cell() {
    // 0x247E10: entry-1 type 9 carries the wall flag; 0x25159C: entering a
    // Labyrinth cell raises event 0x3C (emu B1/B2/B7).
    let mut d = board(vec![
        placed(10, GHOUL, 0, 3, p(4, 5), false, false),
        placed(11, WARRIOR, 0, 1, p(0, 6), false, true),
    ]);
    d.terrain[p(4, 4).index().unwrap()] = terrain::LABYRINTH;
    let (d1, events) = apply(&d, 0, move_cmd(10, vec![p(4, 4)], None));
    assert!(d1.unit(UnitId(10)).is_none());
    let wall = d1.units.iter().find(|u| u.card == WALL).unwrap();
    assert_eq!(
        (
            wall.position,
            wall.side,
            wall.slot,
            wall.face_up,
            wall.defence,
            wall.acted
        ),
        (p(4, 4), 0, 2, false, false, false)
    );
    assert_eq!(graves(&d1), (vec![GHOUL], vec![]));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            unit: UnitId(10),
            ..
        }
    )));
    // A plain cell is an ordinary move (emu B4).
    let (d2, _) = apply(&d, 0, move_cmd(10, vec![p(3, 5)], None));
    assert_eq!(
        (unit(&d2, 10).position, unit(&d2, 10).acted),
        (p(3, 5), true)
    );
    assert!(d2.units.iter().all(|u| u.card != WALL));
    // A plain monster cannot enter the wall cell.
    let mut plain = d.clone();
    plain
        .units
        .iter_mut()
        .find(|u| u.id == UnitId(11))
        .unwrap()
        .position = p(4, 3);
    assert!(matches!(
        plain.apply(0, &move_cmd(11, vec![p(4, 4)], None), &cards()),
        Err(DuelError::Illegal(_))
    ));
}

#[test]
fn jirai_gumo_pays_100_per_entered_cell_with_the_2c_modifier() {
    // 0x2513F4 mode 1 (emu D1-D5, D4): every cell, the advance included.
    let mut d = board(vec![
        placed(10, GUMO, 0, 1, p(4, 5), false, true),
        placed(20, MODIFIER_2C, 1, 1, p(0, 1), true, true),
    ]);
    d.terrain[p(4, 5).index().unwrap()] = terrain::FOREST; // two steps for an Insect
    let (d1, events) = apply(&d, 0, move_cmd(10, vec![p(4, 4), p(4, 3)], None));
    assert_eq!(lp(&d1), [3600, 4000]);
    assert_eq!(
        count_events(&events, |e| matches!(
            e,
            DuelEvent::Damage {
                side: 0,
                amount: 200
            }
        )),
        2
    );
    assert_eq!(unit(&d1, 10).position, p(4, 3));
    // Face-down too (emu D3), without the modifier.
    let d = board(vec![placed(10, GUMO, 0, 1, p(4, 5), false, false)]);
    let (d1, _) = apply(&d, 0, move_cmd(10, vec![p(4, 4)], None));
    assert_eq!(lp(&d1), [3900, 4000]);
    // Attacking a set card (0x2497D0 → step 0x20B → 0xB/0xC/0xD [F]): the
    // card goes, the Gumo advances and its entry fires on the target cell.
    let d = board(vec![
        placed(10, GUMO, 0, 1, p(4, 5), false, true),
        placed(20, MAGLAB, 1, 6, p(4, 4), false, false),
    ]);
    let (d1, _) = attack(&d, 10, p(4, 4));
    assert!(d1.unit(UnitId(20)).is_none());
    assert_eq!(unit(&d1, 10).position, p(4, 4));
    assert_eq!(lp(&d1), [3900, 4000]);
    assert_eq!(graves(&d1), (vec![], vec![MAGLAB]));
}

#[test]
fn jirai_gumo_nullifies_the_other_units_permanent_changes() {
    // 0x263E24 / 0x2487DC (emu C1b/C2b/C3/C4): the other unit's deltas are set
    // to 0 for good before its stats are read, both roles.
    let mut d = board(vec![
        placed(10, GUMO, 0, 1, p(4, 5), false, true),
        placed(20, WARRIOR, 1, 1, p(4, 4), false, true),
    ]);
    d.units
        .iter_mut()
        .find(|u| u.id == UnitId(20))
        .unwrap()
        .attack_bonus = 1500;
    let (d1, _) = attack(&d, 10, p(4, 4));
    // 2200 against 1400 (not 2900): the Warrior dies, 800 damage, then the
    // advance costs the Gumo's side 100.
    assert!(d1.unit(UnitId(20)).is_none());
    assert_eq!(lp(&d1), [3900, 3200]);
    assert_eq!(unit(&d1, 10).position, p(4, 4));
    // A boosted attacker against a face-down Gumo keeps nothing.
    let mut d = board(vec![
        placed(10, STRONG, 0, 1, p(4, 5), false, true),
        placed(20, GUMO, 1, 1, p(4, 4), true, false),
    ]);
    let s = d.units.iter_mut().find(|u| u.id == UnitId(10)).unwrap();
    s.attack_bonus = -1000;
    s.defense_bonus = -1000;
    let (d1, events) = attack(&d, 10, p(4, 4));
    assert!(d1.unit(UnitId(20)).is_none()); // 1800 > DEF 100
    assert_eq!(bonus(&d1, 10), (0, 0));
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Modified {
            unit: UnitId(10),
            attack: 0,
            defense: 0
        }
    )));
    // Against a leader the type-1 branch needs two non-leaders: nothing.
    let mut d = board(vec![placed(10, GUMO, 0, 1, p(3, 1), false, true)]);
    d.units
        .iter_mut()
        .find(|u| u.id == UnitId(10))
        .unwrap()
        .attack_bonus = 300;
    let (d1, _) = attack(&d, 10, p(3, 0));
    assert_eq!(bonus(&d1, 10), (300, 0));
    assert_eq!(lp(&d1), [4000, 1500]);
}

#[test]
fn reaper_of_the_cards_ignores_limited_range_traps() {
    // 0x24CE34 (emu K0-K3): entry-1 type 7 skips 0x24CC30's search.
    let traps = |mover: u16| {
        board(vec![
            placed(10, mover, 0, 1, p(4, 5), false, true),
            placed(20, PIT, 1, 6, p(4, 3), false, false),
        ])
    };
    let (d1, _) = apply(&traps(WARRIOR), 0, move_cmd(10, vec![p(4, 4)], None));
    assert!(d1.unit(UnitId(10)).is_none());
    assert_eq!(graves(&d1), (vec![WARRIOR], vec![PIT]));
    let (d1, _) = apply(&traps(REAPER), 0, move_cmd(10, vec![p(4, 4)], None));
    assert_eq!(
        (unit(&d1, 10).position, unit(&d1, 10).acted),
        (p(4, 4), true)
    );
    assert!(d1.unit(UnitId(20)).is_some());
}

#[test]
fn big_eye_binds_its_killer_and_a_dying_defender_stops_the_advance() {
    // 0x2533C8 / event 0 / result 0x401 (emu L1-L3).
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(4, 5), false, true),
        placed(20, BIGEYE, 1, 1, p(4, 4), true, false),
    ]);
    let (d1, events) = attack(&d, 10, p(4, 4));
    assert_eq!(graves(&d1), (vec![], vec![BIGEYE]));
    let s = unit(&d1, 10);
    assert_eq!((s.position, s.spellbind, s.acted), (p(4, 5), 1, true));
    assert!(!events.iter().any(|e| matches!(
        e,
        DuelEvent::Moved {
            unit: UnitId(10),
            ..
        }
    )));
    // A dying attacker binds the defender (step 0xA ignores bit 10).
    let d = board(vec![
        placed(10, BIGEYE, 0, 1, p(4, 5), false, true),
        placed(20, STRONG, 1, 1, p(4, 4), false, true),
    ]);
    let (d1, _) = attack(&d, 10, p(4, 4));
    assert_eq!(graves(&d1), (vec![BIGEYE], vec![]));
    assert_eq!((unit(&d1, 20).spellbind, unit(&d1, 20).acted), (1, true));
    assert_eq!(lp(&d1), [3400, 4000]);
    // Both dying: nothing.
    let d = board(vec![
        placed(10, BIGEYE, 0, 1, p(4, 5), false, true),
        placed(20, BIGEYE, 1, 1, p(4, 4), false, true),
    ]);
    let (d1, _) = attack(&d, 10, p(4, 4));
    assert_eq!(graves(&d1), (vec![BIGEYE], vec![BIGEYE]));
}

#[test]
fn golgoil_moves_beside_its_leader_instead_of_dying() {
    // 0x253560 / event 0x1F / result 0x101 (emu J1-J3b).
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(4, 5), false, true),
        placed(20, GOLGOIL, 1, 1, p(4, 4), true, false),
    ]);
    let (d1, _) = attack(&d, 10, p(4, 4));
    let g = unit(&d1, 20);
    // Side 1's leader is on (3,0): its first neighbour in the 0x24FD30
    // order (turned) is (3,1).
    assert_eq!((g.position, g.face_up, g.defence), (p(3, 1), true, true));
    assert_eq!(graves(&d1), (vec![], vec![]));
    assert_eq!(unit(&d1, 10).position, p(4, 4));
    // Golgoil dying alone as the attacker: relocated too, no advance.
    let d = board(vec![
        placed(10, GOLGOIL, 0, 1, p(4, 5), false, true),
        placed(20, STRONG, 1, 1, p(4, 4), false, true),
    ]);
    let (d1, _) = attack(&d, 10, p(4, 4));
    assert_eq!(
        (unit(&d1, 10).position, unit(&d1, 10).acted),
        (p(3, 5), true)
    );
    assert_eq!(lp(&d1), [3100, 4000]);
    // No free non-Labyrinth neighbour: an ordinary death.
    let mut d = board(vec![
        placed(10, STRONG, 0, 1, p(4, 5), false, true),
        placed(20, GOLGOIL, 1, 1, p(4, 4), true, false),
    ]);
    for cell in [p(3, 1), p(2, 0), p(4, 0), p(2, 1), p(4, 1)] {
        d.terrain[cell.index().unwrap()] = terrain::LABYRINTH;
    }
    let (d1, _) = attack(&d, 10, p(4, 4));
    assert_eq!(graves(&d1), (vec![], vec![GOLGOIL]));
    // 0x25356C: without Call of the Grave, Royal Decree is never queried.
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(4, 5), false, true),
        placed(20, GOLGOIL, 1, 1, p(4, 4), true, false),
        placed(21, DECREE, 1, 6, p(0, 1), false, false),
    ]);
    let (d1, _) = attack(&d, 10, p(4, 4));
    assert_eq!(unit(&d1, 20).position, p(3, 1));
    assert_eq!(d1.unit(UnitId(21)), d.unit(UnitId(21)));
    assert_eq!(graves(&d1), (vec![], vec![]));
}

#[test]
fn machine_king_gains_100_per_machine_on_the_board() {
    // 0x26610C (emu G1-G3): itself included, both sides, any face.
    let d = board(vec![
        placed(10, MKING, 0, 1, p(4, 5), false, false),
        placed(11, WARRIOR, 0, 2, p(0, 6), false, true),
        placed(20, STRONG, 1, 1, p(0, 1), true, false),
    ]);
    let (d1, _) = reveal(&d, 10);
    assert_eq!(bonus(&d1, 10), (200, 200));
    let d = board(vec![placed(10, MKING, 0, 1, p(4, 5), false, false)]);
    let (d1, _) = reveal(&d, 10);
    assert_eq!(bonus(&d1, 10), (100, 100));
}

#[test]
fn barrel_dragon_destroys_one_unit_drawn_from_both_sides() {
    // 0x265CA4 / 0x2504F0 (emu H1-H3): rand31 % count over g0 s1.. then g1.
    let mut d = board(vec![
        placed(10, BARREL, 0, 1, p(4, 5), false, false),
        placed(11, WARRIOR, 0, 2, p(0, 6), false, true),
        placed(20, STRONG, 1, 1, p(0, 1), true, false),
    ]);
    for seed in [1u64, 2, 3, 4, 5, 6] {
        d.seed_rng(seed);
        let mut s = seed;
        let expected = if next_rand31(&mut s).is_multiple_of(2) {
            11
        } else {
            20
        };
        let (d1, events) = reveal(&d, 10);
        assert!(d1.unit(UnitId(expected)).is_none(), "seed {seed}");
        assert_eq!(d1.units.len(), 4, "seed {seed}");
        assert_eq!(d1.rng, s);
        assert!(events.iter().any(|e| matches!(
            e,
            DuelEvent::Activated {
                unit: UnitId(10),
                target: Some(UnitId(t)),
                ..
            } if *t == expected
        )));
    }
    // Nobody else: nothing, no draw.
    let mut d = board(vec![placed(10, BARREL, 0, 1, p(4, 5), false, false)]);
    d.seed_rng(9);
    let (d1, _) = reveal(&d, 10);
    assert_eq!((d1.units.len(), d1.rng), (3, 9));
}

#[test]
fn patrol_robo_reveals_the_enemys_set_cards() {
    // 0x265B74 / event 0xD mode 4 (emu I1-I3): non-monster cards the other
    // side controls turn face-up; a revealed magic card waits face-up
    // (traps_field_tests.rs covers its later activation).
    let d = board(vec![
        placed(10, PATROL, 0, 1, p(4, 5), false, false),
        placed(12, FAN, 0, 6, p(0, 6), false, false),
        placed(20, STRONG, 1, 1, p(0, 1), true, false),
        placed(21, GORGON, 1, 6, p(1, 1), false, false),
        placed(22, PIT, 1, 7, p(2, 1), false, false),
    ]);
    let (d1, _) = reveal(&d, 10);
    assert!(unit(&d1, 21).face_up && unit(&d1, 21).committed_face_up);
    assert!(unit(&d1, 22).face_up);
    assert!(!unit(&d1, 20).face_up && !unit(&d1, 12).face_up);
    // A revealed magic card stays on its cell, face-up, and the duel goes on.
    let mut magic = d.clone();
    magic
        .units
        .push(placed(23, MAGIC, 1, 8, p(3, 1), false, false));
    let (d2, _) = reveal(&magic, 10);
    assert!(unit(&d2, 23).face_up && unit(&d2, 23).committed_face_up);
    assert!(d2.apply(0, &DuelCommand::EndTurn, &cards()).is_ok());
}

#[test]
fn time_seal_binds_the_strongest_monster_on_the_board() {
    // 0x254974 with filter 0x21 (emu N1-N3).
    let d = board(vec![
        placed(10, TIMESEAL, 0, 6, p(2, 5), false, false),
        placed(11, WARRIOR, 0, 1, p(4, 5), false, true),
        placed(20, STRONG, 1, 1, p(0, 1), true, false),
    ]);
    let (d1, _) = reveal(&d, 10);
    assert_eq!((unit(&d1, 20).spellbind, unit(&d1, 20).acted), (255, true));
    assert_eq!(unit(&d1, 11).spellbind, 0);
    assert_eq!(graves(&d1), (vec![TIMESEAL], vec![]));
}

#[test]
fn shift_moves_the_activators_strongest_monster_onto_its_cell() {
    // 0x255E0C / event 0x1F (emu F1-F4).
    let d = board(vec![
        placed(10, SHIFT, 0, 6, p(2, 5), false, false),
        placed(11, WARRIOR, 0, 1, p(4, 5), false, true),
        placed(12, REPTILE, 0, 2, p(5, 5), false, false),
        placed(20, STRONG, 1, 1, p(0, 1), true, false),
    ]);
    let (d1, _) = reveal(&d, 10);
    assert_eq!(
        (unit(&d1, 11).position, unit(&d1, 11).acted),
        (p(2, 5), false)
    );
    assert_eq!(unit(&d1, 12).position, p(5, 5));
    assert_eq!(graves(&d1), (vec![SHIFT], vec![]));
    // No own monster: the card is spent for nothing.
    let d = board(vec![placed(10, SHIFT, 0, 6, p(2, 5), false, false)]);
    let (d1, _) = reveal(&d, 10);
    assert_eq!(graves(&d1), (vec![SHIFT], vec![]));
}

#[test]
fn magical_labyrinth_moves_a_random_wall_onto_its_cell() {
    // 0x254E08 / event 3 (emu E1-E4).
    let mut d = board(vec![placed(10, MAGLAB, 0, 6, p(2, 5), false, false)]);
    d.terrain[p(6, 6).index().unwrap()] = terrain::LABYRINTH;
    d.terrain[p(0, 0).index().unwrap()] = terrain::LABYRINTH;
    for seed in [1u64, 2, 3, 4] {
        d.seed_rng(seed);
        let mut s = seed;
        // Column-major: (0,0) first, then (6,6).
        let chosen = if next_rand31(&mut s).is_multiple_of(2) {
            p(0, 0)
        } else {
            p(6, 6)
        };
        let (d1, _) = reveal(&d, 10);
        assert_eq!(
            d1.terrain[chosen.index().unwrap()],
            terrain::NORMAL,
            "seed {seed}"
        );
        assert_eq!(d1.terrain[p(2, 5).index().unwrap()], terrain::LABYRINTH);
        assert_eq!(
            d1.terrain
                .iter()
                .filter(|&&t| t == terrain::LABYRINTH)
                .count(),
            2
        );
        assert_eq!(graves(&d1), (vec![MAGLAB], vec![]));
    }
    let mut d = board(vec![placed(10, MAGLAB, 0, 6, p(2, 5), false, false)]);
    d.seed_rng(7);
    let (d1, _) = reveal(&d, 10);
    assert_eq!((d1.rng, graves(&d1).0), (7, vec![MAGLAB]));
    assert!(d1.terrain.iter().all(|&t| t == terrain::NORMAL));
}

#[test]
fn paralyzing_potion_binds_the_monster_it_is_moved_onto() {
    // 0x249D80 class 3 (emu P1-P3): own or enemy monster.
    let d = board(vec![
        placed(10, POTION, 0, 6, p(4, 5), false, false),
        placed(11, WARRIOR, 0, 1, p(4, 4), false, true),
    ]);
    let (d1, _) = apply(&d, 0, move_cmd(10, vec![p(4, 4)], None));
    let w = unit(&d1, 11);
    assert_eq!((w.spellbind, w.acted, w.position), (255, true, p(4, 4)));
    assert_eq!(graves(&d1), (vec![POTION], vec![]));
    let d = board(vec![
        placed(10, POTION, 0, 6, p(4, 5), false, false),
        placed(20, STRONG, 1, 1, p(4, 4), false, true),
    ]);
    let (d1, _) = apply(&d, 0, move_cmd(10, vec![p(4, 4)], None));
    assert_eq!(unit(&d1, 20).spellbind, 255);
    assert_eq!(graves(&d1), (vec![POTION], vec![]));
}

#[test]
fn gorgons_eye_petrifies_a_monster_changing_to_defence() {
    // 0x26C7A0 / event 0x47 (emu Q0-Q2): the trap stays, revealed.
    let d = board(vec![
        placed(10, WARRIOR, 0, 1, p(4, 5), false, true),
        placed(20, GORGON, 1, 6, p(0, 1), false, false),
    ]);
    let (d1, events) = apply(&d, 0, move_cmd(10, vec![], Some(true)));
    let w = unit(&d1, 10);
    assert_eq!((w.defence, w.spellbind, w.acted), (false, 255, true));
    assert!(unit(&d1, 20).face_up);
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::Cancelled { unit: UnitId(10) })));
    // Moving (attack position) does not trigger it.
    let (d1, _) = apply(&d, 0, move_cmd(10, vec![p(4, 4)], None));
    assert_eq!((unit(&d1, 10).spellbind, unit(&d1, 20).face_up), (0, false));
}

#[test]
fn the_2c_modifier_raises_battle_and_leader_damage() {
    // 0x2768EC and 0x275E00 read 0x268A10(0x2C, damage).
    let d = board(vec![
        placed(10, STRONG, 0, 1, p(4, 5), false, true),
        placed(20, WARRIOR, 1, 1, p(4, 4), false, true),
        placed(21, MODIFIER_2C, 1, 2, p(0, 1), true, true),
    ]);
    let (d1, _) = attack(&d, 10, p(4, 4));
    assert_eq!(lp(&d1), [4000, 3500]);
    let d = board(vec![
        placed(10, WARRIOR, 0, 1, p(3, 1), false, true),
        placed(21, MODIFIER_2C, 1, 2, p(0, 1), true, true),
    ]);
    let (d1, _) = attack(&d, 10, p(3, 0));
    assert_eq!(lp(&d1), [4000, 2500]);
}

#[test]
fn water_element_conceals_its_controllers_cards() {
    // 0x25B980 mode 2/4 with rec+0xC = 0 (word1 lo 0): 0x252FF0(g, s, 0)
    // turns the targets face down (MORE-EFFECTS.md 1.2.1). Patrol Robo's
    // entry with mode 1 (own side) and word1 0.
    let mut c = cards();
    c[usize::from(PATROL)] = with_entry(monster(PATROL, kind::MACHINE, 3, 1, 1100, 900), 3, [0x185E, 0]);
    let d = board(vec![
        placed(10, PATROL, 0, 1, p(4, 5), false, false),
        placed(12, FAN, 0, 6, p(0, 6), false, true),
        placed(21, GORGON, 1, 6, p(1, 1), false, true),
    ]);
    let (d1, events) = d
        .apply(0, &DuelCommand::Move { unit: UnitId(10), path: vec![], face_up: true, defence: None }, &c)
        .unwrap();
    assert!(!unit(&d1, 12).face_up && !unit(&d1, 12).committed_face_up);
    assert!(unit(&d1, 21).face_up, "the other side is not touched");
    assert!(events.iter().any(|e| matches!(e, DuelEvent::Concealed { unit: UnitId(12), .. })));
}
