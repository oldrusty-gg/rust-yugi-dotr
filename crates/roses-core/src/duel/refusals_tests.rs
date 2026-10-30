//! Effects ported by the refusal sweep (claude-effects/REFUSALS-SWEEP.md):
//! synthetic cards with the originals' effect words, checked against the
//! emulator cases named in each test.

use super::*;

const LEADER: u16 = 0;
const INSECT_LEADER: u16 = 1;
const WEAK: u16 = 3; // 1000/1000
const MID: u16 = 4; // 1500/1200
const SET_CARD: u16 = 5; // a power-up (non-monster)
const MACHINE: u16 = 6; // 1000/1000 Machine
const WINGED: u16 = 7; // 1000/1000 Winged Beast
const DRAGON: u16 = 8; // 1500/1000 Dragon, takes the Cocoon
const INSECT_IMITATION_TARGET: u16 = 17; // Manga Ryu-Ran
const THOUSAND_DRAGON: u16 = 24;
const TIME_WIZARD: u16 = 36;
const HURRICAIL: u16 = 98;
const TEMPLE: u16 = 113;
const FIRE_REAPER: u16 = 115;
const FLAME_GHOST: u16 = 118;
const ONE_WHO_HUNTS: u16 = 217;
const HANE_HANE: u16 = 257;
const MONSTER_EYE: u16 = 344;
const WALL_ILLUSION: u16 = 361;
const BERFOMET: u16 = 364;
const ARSENAL_BUG: u16 = 429;
const SERPENT: u16 = 456;
const TURTLE: u16 = 591;
const ANCIENT_JAR: u16 = 627;
const SOULEATER: u16 = 673;
const BAGWORM: u16 = 677;
const TIMEATER: u16 = 678;
const RIGRAS: u16 = 681;
const DARK_HOLE: u16 = 699;
const EYE_OF_TRUTH: u16 = 718;
const STAIN_STORM: u16 = 722;
const ETAQUA: u16 = 740;
const RIRYOKU: u16 = 789;
const COCOON: u16 = 798;
const INSECT_IMITATION: u16 = 800;
const GOBLIN_FAN: u16 = 814;
const REVERSE_TRAP: u16 = 816;
const LAST: u16 = 860;

fn fits(mut c: CardInfo, power_up: u16) -> CardInfo {
    let bit = usize::from(power_up - 0x2F0);
    c.equip_bits[bit >> 3] |= 1 << (bit & 7);
    c
}

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

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..=LAST)
        .map(|i| monster(i, kind::WARRIOR, 100, 100))
        .collect();
    v[usize::from(LEADER)] = monster(LEADER, kind::WARRIOR, 3000, 2500);
    v[usize::from(INSECT_LEADER)] = monster(INSECT_LEADER, kind::INSECT, 2000, 2000);
    v[usize::from(WEAK)] = monster(WEAK, kind::WARRIOR, 1000, 1000);
    v[usize::from(MID)] = monster(MID, kind::WARRIOR, 1500, 1200);
    v[usize::from(SET_CARD)] = CardInfo {
        kind: class::POWER_UP << 5,
        power_up: Some(PowerUp {
            category: 0,
            value: 500,
        }),
        ..monster(SET_CARD, 0, 0, 0)
    };
    v[usize::from(MACHINE)] = monster(MACHINE, kind::MACHINE, 1000, 1000);
    v[usize::from(WINGED)] = monster(WINGED, kind::WINGED_BEAST, 1000, 1000);
    v[usize::from(DRAGON)] = fits(monster(DRAGON, kind::DRAGON, 1500, 1000), COCOON);
    v[usize::from(INSECT_IMITATION_TARGET)] = fits(
        monster(INSECT_IMITATION_TARGET, kind::DRAGON, 1200, 900),
        INSECT_IMITATION,
    );
    v[usize::from(THOUSAND_DRAGON)] = monster(THOUSAND_DRAGON, kind::DRAGON, 2400, 2000);
    v[usize::from(TIME_WIZARD)] = with_entry(
        monster(TIME_WIZARD, kind::SPELLCASTER, 500, 400),
        3,
        0x33F7,
        0x095F_0018,
    );
    v[usize::from(TEMPLE)] = with_entry(monster(TEMPLE, kind::ZOMBIE, 1200, 1300), 2, 0x0EE9, 0);
    v[usize::from(ONE_WHO_HUNTS)] =
        with_entry(monster(ONE_WHO_HUNTS, kind::FIEND, 1100, 1000), 0, 0x3A9A, 0);
    v[usize::from(HANE_HANE)] = with_entry(monster(HANE_HANE, kind::BEAST, 450, 500), 3, 0x245B, 0);
    v[usize::from(WALL_ILLUSION)] =
        with_entry(monster(WALL_ILLUSION, kind::FIEND, 1000, 1850), 3, 0x249A, 0);
    v[usize::from(MONSTER_EYE)] =
        with_entry(monster(MONSTER_EYE, kind::FIEND, 250, 350), 3, 0x18AD, 1);
    v[usize::from(BERFOMET)] =
        with_entry(monster(BERFOMET, kind::FIEND, 1400, 1800), 3, 0x1A7D, 0xC);
    v[usize::from(ARSENAL_BUG)] =
        with_entry(monster(ARSENAL_BUG, kind::INSECT, 2000, 2000), 3, 0x3578, 0x9_05DC);
    v[usize::from(SERPENT)] = with_entry(monster(SERPENT, kind::REPTILE, 300, 250), 4, 0x1ABD, 0);
    v[usize::from(TURTLE)] =
        with_entry(monster(TURTLE, kind::AQUA, 700, 900), 3, 0x259A, 0x1F_0000);
    v[usize::from(ANCIENT_JAR)] =
        with_entry(monster(ANCIENT_JAR, kind::ROCK, 0, 0), 2, 0x47DF, 0);
    v[usize::from(SOULEATER)] =
        with_entry(monster(SOULEATER, kind::FIEND, 1200, 0), 2, 0x53DD, 0xC8);
    v[usize::from(BAGWORM)] =
        with_entry(monster(BAGWORM, kind::MACHINE, 1400, 1000), 3, 0x1F8E, 0);
    v[usize::from(TIMEATER)] =
        with_entry(monster(TIMEATER, kind::FIEND, 1900, 1400), 2, 0x55FD, 0);
    v[usize::from(RIGRAS)] = with_entry(monster(RIGRAS, kind::INSECT, 1500, 1300), 3, 0x57FD, 0);
    let magic = |id: u16, w0: u32, w1: u32| CardInfo {
        kind: class::MAGIC << 5,
        ..with_entry(monster(id, 0, 0, 0), 0, w0, w1)
    };
    v[usize::from(DARK_HOLE)] = magic(DARK_HOLE, 0x04DF, 0);
    v[usize::from(EYE_OF_TRUTH)] = magic(EYE_OF_TRUTH, 0x1DBD, 0);
    v[usize::from(STAIN_STORM)] = magic(STAIN_STORM, 0x04CE, 0);
    v[usize::from(ETAQUA)] = magic(ETAQUA, 0x12C6, 0x258);
    let power = |id: u16, category: u8, value: u16| CardInfo {
        kind: class::POWER_UP << 5,
        power_up: Some(PowerUp { category, value }),
        ..monster(id, 0, 0, 0)
    };
    v[usize::from(RIRYOKU)] = power(RIRYOKU, 0, 0);
    v[usize::from(COCOON)] = power(COCOON, 4, 0);
    v[usize::from(INSECT_IMITATION)] = power(INSECT_IMITATION, 4, 0);
    let trap = |id: u16, w0: u32| CardInfo {
        kind: class::FAR_TRAP << 5,
        ..with_entry(monster(id, 0, 0, 0), 0, w0, 0)
    };
    v[usize::from(GOBLIN_FAN)] = trap(GOBLIN_FAN, 0x29FD);
    v[usize::from(REVERSE_TRAP)] = trap(REVERSE_TRAP, 0x2BFD);
    v[usize::from(MID)] = fits(v[usize::from(MID)].clone(), RIRYOKU);
    v[usize::from(WEAK)] = fits(v[usize::from(WEAK)].clone(), RIRYOKU);
    v[usize::from(HURRICAIL)] =
        with_entry(monster(HURRICAIL, kind::WINGED_BEAST, 900, 200), 3, 0x04DE, 0x1_0000);
    v[usize::from(FIRE_REAPER)] =
        with_entry(monster(FIRE_REAPER, kind::ZOMBIE, 700, 500), 3, 0x04DF, 0x17_0000);
    v[usize::from(FLAME_GHOST)] =
        with_entry(monster(FLAME_GHOST, kind::ZOMBIE, 1000, 800), 3, 0x04DF, 0x18_0000);
    v
}

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn board_with(leaders: [u16; 2], abilities: [Vec<u8>; 2]) -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders,
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

fn board() -> Duel {
    board_with([LEADER, LEADER], [vec![], vec![]])
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

fn set_terrain(d: &mut Duel, at: GridPos, t: u8) {
    d.terrain[at.index().unwrap()] = t;
}

// ---- Leader ability #7 ----

#[test]
fn leader_ability_7_lays_the_leader_kinds_terrain_on_the_entered_cell() {
    // emu L1/L2: the Fairy leader turned Sea and Crush into Normal; here an
    // Insect leader lays Forest (0x2730D0). Without #7 nothing changes.
    for (abilities, want) in [(vec![7], terrain::FOREST), (vec![], terrain::SEA)] {
        let mut d = board_with([INSECT_LEADER, LEADER], [abilities, vec![]]);
        let leader = d.leader(0).clone();
        let to = p(leader.position.column, leader.position.row - 1);
        set_terrain(&mut d, to, terrain::SEA);
        let (d1, _) = mv(&d, leader.id, vec![to], false).unwrap();
        assert_eq!(d1.terrain_at(to), want);
        assert_eq!(d1.terrain_at(leader.position), terrain::NORMAL);
    }
    assert_eq!(leader_terrain(kind::IMMORTAL), 9);
    assert_eq!(leader_terrain(kind::AQUA), terrain::SEA);
    assert_eq!(leader_terrain(kind::FIEND), terrain::DARK);
    assert_eq!(leader_terrain(kind::SPELLCASTER), terrain::DARK);
    assert_eq!(leader_terrain(kind::FAIRY), terrain::NORMAL);
}

// ---- Flip type 4 scopes ----

#[test]
fn flame_ghost_destroys_every_unit_on_sea_itself_included() {
    // emu FG1 / FG2: scope 0x18 = terrain 4 (Sea), filter 0x1F, mode 3.
    let mut d = board();
    let ghost = put(&mut d, FLAME_GHOST, 0, p(4, 5), false, false);
    let own = put(&mut d, WEAK, 0, p(2, 6), false, false);
    let enemy = put(&mut d, MID, 1, p(2, 4), true, false);
    let dry = put(&mut d, MID, 1, p(3, 2), true, false);
    set_terrain(&mut d, p(2, 6), terrain::SEA);
    set_terrain(&mut d, p(2, 4), terrain::SEA);
    let (d1, _) = mv(&d, ghost, vec![], true).unwrap();
    assert!(d1.unit(own).is_none() && d1.unit(enemy).is_none());
    assert!(d1.unit(ghost).is_some() && d1.unit(dry).is_some());
    assert_eq!(d1.sides[0].graveyard, vec![WEAK]);
    assert_eq!(d1.sides[1].graveyard, vec![MID]);
    // On Sea itself: it goes too.
    let mut d = board();
    let ghost = put(&mut d, FLAME_GHOST, 0, p(4, 5), false, false);
    set_terrain(&mut d, p(4, 5), terrain::SEA);
    let (d1, _) = mv(&d, ghost, vec![], true).unwrap();
    assert!(d1.unit(ghost).is_none());
    assert_eq!(d1.sides[0].graveyard, vec![FLAME_GHOST]);
}

#[test]
fn fire_reaper_destroys_on_meadow_only() {
    // emu FR1: scope 0x17 = terrain 3; a unit on Sea stays.
    let mut d = board();
    let reaper = put(&mut d, FIRE_REAPER, 0, p(4, 5), false, false);
    let meadow = put(&mut d, MID, 1, p(2, 4), true, false);
    let sea = put(&mut d, MID, 1, p(3, 2), true, false);
    set_terrain(&mut d, p(2, 4), terrain::MEADOW);
    set_terrain(&mut d, p(3, 2), terrain::SEA);
    let (d1, _) = mv(&d, reaper, vec![], true).unwrap();
    assert!(d1.unit(meadow).is_none() && d1.unit(sea).is_some());
}

#[test]
fn hurricail_destroys_non_monster_cards_within_distance_one() {
    // emu HC1: own set cards at (4,4) and (3,5), an enemy one at (5,5) go;
    // the diagonal (5,4) stays, so does an adjacent monster.
    let mut d = board();
    let bird = put(&mut d, HURRICAIL, 0, p(4, 5), false, false);
    let a = put(&mut d, SET_CARD, 0, p(4, 4), false, false);
    let b = put(&mut d, SET_CARD, 0, p(3, 5), false, false);
    let c = put(&mut d, SET_CARD, 1, p(5, 5), false, false);
    let diagonal = put(&mut d, SET_CARD, 0, p(5, 4), false, false);
    let monster = put(&mut d, WEAK, 1, p(4, 6), false, false);
    let (d1, _) = mv(&d, bird, vec![], true).unwrap();
    for gone in [a, b, c] {
        assert!(d1.unit(gone).is_none());
    }
    for kept in [bird, diagonal, monster] {
        assert!(d1.unit(kept).is_some());
    }
    assert_eq!(d1.sides[1].graveyard, vec![SET_CARD]);
}

fn act(d: &Duel, side: u8, cmd: DuelCommand) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(side, &cmd, &Catalogue::new(cards(), []))
}

fn flip_onto(d: &Duel, unit: UnitId, to: GridPos) -> Result<(Duel, Vec<DuelEvent>)> {
    act(
        d,
        0,
        DuelCommand::Move {
            unit,
            path: vec![to],
            face_up: true,
            defence: None,
        },
    )
}

// ---- Monster Eye ----

#[test]
fn monster_eye_reveals_the_newest_enemy_card_only() {
    // emu ME1: filter 0x2D = any card, then the highest summon order.
    let mut d = board();
    let eye = put(&mut d, MONSTER_EYE, 0, p(4, 5), false, false);
    let old = put(&mut d, WEAK, 1, p(2, 1), false, true);
    let new = put(&mut d, SET_CARD, 1, p(4, 1), false, false);
    let (d1, _) = mv(&d, eye, vec![], true).unwrap();
    assert!(d1.unit(new).unwrap().face_up);
    assert!(!d1.unit(old).unwrap().face_up);
}

// ---- Flip type 0x24 ----

#[test]
fn hane_hane_goes_back_to_its_deck_and_wall_of_illusion_sends_the_attacker() {
    // emu HH1: manual flip, back into the deck; WI1: the attacker goes.
    let mut d = board();
    d.sides[0].deck = vec![WEAK];
    let hane = put(&mut d, HANE_HANE, 0, p(4, 5), false, false);
    let (d1, _) = mv(&d, hane, vec![], true).unwrap();
    assert!(d1.unit(hane).is_none());
    assert_eq!(d1.deck_count(0), 2);
    assert!(d1.sides[0].graveyard.is_empty());

    let mut d = board();
    let wall = put(&mut d, WALL_ILLUSION, 1, p(3, 2), false, true);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(att).is_none());
    assert_eq!(d1.deck_count(0), 1);
    let w = d1.unit(wall).unwrap();
    assert!(w.face_up && w.position == p(3, 2));
    assert_eq!(d1.sides[0].lp, START_LP);
    assert!(d1.sides[0].graveyard.is_empty());
}

#[test]
fn hane_hane_moved_onto_an_own_unit_takes_the_partner_to_the_graveyard() {
    // emu R13: Hane-Hane back in the deck, then record 0x5F removes the
    // partner.
    let mut d = board();
    let hane = put(&mut d, HANE_HANE, 0, p(4, 5), false, false);
    let partner = put(&mut d, WEAK, 0, p(4, 4), true, false);
    let (d1, _) = flip_onto(&d, hane, p(4, 4)).unwrap();
    assert!(d1.unit(hane).is_none() && d1.unit(partner).is_none());
    assert_eq!(d1.sides[0].graveyard, vec![WEAK]);
    assert_eq!(d1.deck_count(0), 1);
}

// ---- Sinister Serpent / One Who Hunts Souls ----

#[test]
fn sinister_serpent_destroyed_zeroes_the_enemys_stars() {
    // emu SS1.
    let mut d = board();
    d.sides[0].stars = 5;
    put(&mut d, SERPENT, 1, p(3, 2), true, false);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert_eq!(d1.sides[0].stars, 0);
    assert_eq!(d1.sides[1].graveyard, vec![SERPENT]);
}

#[test]
fn one_who_hunts_souls_keeps_the_battle_dead_out_of_the_graveyard() {
    // emu OH1/OH2: rec+0x10 bit 2 -> step flag 0x400, no 0x1E00F0.
    let mut d = board();
    let hunts = put(&mut d, ONE_WHO_HUNTS, 1, p(3, 2), true, false);
    let att = put(&mut d, MID, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(hunts).is_none());
    assert!(d1.sides[1].graveyard.is_empty());
    assert_eq!(d1.sides[1].lp, START_LP - 400);
}

// ---- Magic ----

#[test]
fn stain_storm_destroys_every_machine_and_etaqua_boosts_winged_beasts() {
    // emu SS1 (722) / WE1 (740).
    let mut d = board();
    let storm = put(&mut d, STAIN_STORM, 0, p(3, 4), false, false);
    let own = put(&mut d, MACHINE, 0, p(1, 5), false, false);
    let enemy = put(&mut d, MACHINE, 1, p(5, 1), true, false);
    let other = put(&mut d, WEAK, 0, p(0, 5), false, false);
    let (d1, _) = mv(&d, storm, vec![], true).unwrap();
    assert!(d1.unit(own).is_none() && d1.unit(enemy).is_none());
    assert!(d1.unit(other).is_some());
    assert_eq!(d1.sides[0].graveyard, vec![STAIN_STORM, MACHINE]);

    let mut d = board();
    let wind = put(&mut d, ETAQUA, 0, p(3, 4), false, false);
    let bird = put(&mut d, WINGED, 1, p(5, 1), true, false);
    let (d1, _) = mv(&d, wind, vec![], true).unwrap();
    let b = d1.unit(bird).unwrap();
    assert_eq!((b.attack_bonus, b.defense_bonus), (600, 600));
}

#[test]
fn the_eye_of_truth_changes_nothing_but_its_own_card() {
    // emu ET1/ET2.
    let mut d = board();
    d.sides[1].hand = vec![WEAK, MID];
    let eye = put(&mut d, EYE_OF_TRUTH, 0, p(3, 4), false, false);
    let (d1, _) = mv(&d, eye, vec![], true).unwrap();
    assert_eq!(d1.sides[1].hand, vec![WEAK, MID]);
    assert_eq!(d1.sides[0].graveyard, vec![EYE_OF_TRUTH]);
}

#[test]
fn temple_of_skulls_in_defence_negates_magic_from_either_side() {
    // emu TS1/TS2: the card goes to the graveyard, the Temple acts.
    let mut d = board();
    let temple = put(&mut d, TEMPLE, 1, p(5, 1), true, true);
    let hole = put(&mut d, DARK_HOLE, 0, p(3, 4), false, false);
    let victim = put(&mut d, WEAK, 1, p(1, 1), true, false);
    let (d1, _) = mv(&d, hole, vec![], true).unwrap();
    assert!(d1.unit(victim).is_some());
    assert!(d1.unit(temple).unwrap().acted);
    assert_eq!(d1.sides[0].graveyard, vec![DARK_HOLE]);
}

// ---- Role 2 (moving onto an own unit) ----

#[test]
fn flame_ghost_onto_an_own_unit_on_sea_destroys_it_and_takes_its_cell() {
    // emu R1: the partner in the mask -> 0x20B, the mover advances; R2:
    // itself in the mask -> the move ends.
    let mut d = board();
    let ghost = put(&mut d, FLAME_GHOST, 0, p(4, 5), false, false);
    let partner = put(&mut d, WEAK, 0, p(4, 4), true, false);
    set_terrain(&mut d, p(4, 4), terrain::SEA);
    let (d1, _) = flip_onto(&d, ghost, p(4, 4)).unwrap();
    assert!(d1.unit(partner).is_none());
    let g = d1.unit(ghost).unwrap();
    assert_eq!((g.position, g.face_up, g.acted), (p(4, 4), true, true));

    let mut d = board();
    let ghost = put(&mut d, FLAME_GHOST, 0, p(4, 5), false, false);
    let partner = put(&mut d, WEAK, 0, p(4, 4), true, false);
    set_terrain(&mut d, p(4, 5), terrain::SEA);
    let (d1, _) = flip_onto(&d, ghost, p(4, 4)).unwrap();
    assert!(d1.unit(ghost).is_none());
    assert_eq!(d1.unit(partner).unwrap().position, p(4, 4));
}

#[test]
fn berfomet_and_turtle_raccoon_onto_an_own_unit_act_then_combine() {
    // emu R5 (stars 12; the failed fusion puts the partner in the
    // graveyard and the mover on its cell) and R11 (the teleport is
    // overwritten by the combination).
    for (card, stars) in [(BERFOMET, 12), (TURTLE, 3)] {
        let mut d = board();
        d.sides[0].stars = 3;
        let mover = put(&mut d, card, 0, p(4, 5), false, false);
        let partner = put(&mut d, WEAK, 0, p(4, 4), true, false);
        let (d1, _) = flip_onto(&d, mover, p(4, 4)).unwrap();
        assert_eq!(d1.sides[0].stars, stars, "card {card}");
        assert!(d1.unit(partner).is_none(), "card {card}");
        assert_eq!(d1.unit(mover).unwrap().position, p(4, 4), "card {card}");
    }
}

// ---- Arsenal Bug ----

#[test]
fn arsenal_bug_gains_with_an_insect_leader_only() {
    // emu AB0/AB1.
    for (leader, gain) in [(LEADER, 0), (INSECT_LEADER, 1500)] {
        let mut d = board_with([leader, LEADER], [vec![], vec![]]);
        let bug = put(&mut d, ARSENAL_BUG, 0, p(4, 5), false, false);
        let (d1, _) = mv(&d, bug, vec![], true).unwrap();
        let u = d1.unit(bug).unwrap();
        assert_eq!((u.attack_bonus, u.defense_bonus), (gain, gain));
    }
}

// ---- Countdowns: Ancient Jar, Timeater ----

#[test]
fn ancient_jar_freezes_and_timeater_doubles_the_spellbind_countdown() {
    // emu J0/J1, T1/T2 (0x273E20); RT1: Timeater advances the round at
    // every turn start.
    for (extra, after) in [(None, 1), (Some(ANCIENT_JAR), 2), (Some(TIMEATER), 0)] {
        let mut d = board();
        let bound = put(&mut d, WEAK, 0, p(1, 5), true, false);
        d.units.iter_mut().find(|u| u.id == bound).unwrap().spellbind = 2;
        if let Some(c) = extra {
            put(&mut d, c, 1, p(5, 1), true, true);
        }
        let round = d.round;
        let (d1, _) = act(&d, 0, DuelCommand::EndTurn).unwrap();
        assert_eq!(d1.unit(bound).unwrap().spellbind, after, "{extra:?}");
        let rounds = u32::from(extra == Some(TIMEATER));
        assert_eq!(u32::from(d1.round), u32::from(round) + rounds, "{extra:?}");
    }
}

// ---- Power-up transforms ----

#[test]
fn cocoon_of_evolution_transforms_its_attacker_and_an_enemy_it_moves_onto() {
    // emu CE1: the attacker becomes Larva of Moth 427 on the Cocoon's cell,
    // acted; CE2: the enemy monster becomes Larva in place, not acted.
    let mut d = board();
    let cocoon = put(&mut d, COCOON, 1, p(3, 2), false, false);
    let att = put(&mut d, DRAGON, 0, p(3, 3), true, false);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(att).is_none() && d1.unit(cocoon).is_none());
    let larva = d1.unit_at(p(3, 2)).unwrap();
    assert_eq!((larva.card, larva.side, larva.face_up, larva.acted), (427, 0, false, true));
    assert_eq!(d1.sides[0].graveyard, vec![DRAGON]);
    assert_eq!(d1.sides[1].graveyard, vec![COCOON]);

    let mut d = board();
    let cocoon = put(&mut d, COCOON, 0, p(3, 3), false, false);
    let enemy = put(&mut d, DRAGON, 1, p(3, 2), true, false);
    let (d1, _) = mv(&d, cocoon, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(enemy).is_none());
    let larva = d1.unit_at(p(3, 2)).unwrap();
    assert_eq!((larva.card, larva.side, larva.acted), (427, 1, false));
}

#[test]
fn insect_imitation_draws_its_result() {
    // emu II1: Manga Ryu-Ran -> 13 + rand31 % 5.
    let mut d = board();
    d.seed_rng(12345);
    put(&mut d, INSECT_IMITATION, 1, p(3, 2), false, false);
    let att = put(&mut d, INSECT_IMITATION_TARGET, 0, p(3, 3), true, false);
    let mut x = 12345;
    let want = 13 + (next_rand31(&mut x) % 5) as u16;
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert_eq!(d1.unit_at(p(3, 2)).unwrap().card, want);
    assert_eq!(d1.rng, x);
}

// ---- Riryoku ----

#[test]
fn riryoku_moved_onto_an_enemy_feeds_it_from_its_own_side() {
    // emu RY1: the enemy monster gains half its side's LP, its side pays.
    let mut d = board();
    d.sides[1].lp = 3400;
    let r = put(&mut d, RIRYOKU, 0, p(3, 3), false, false);
    let enemy = put(&mut d, WEAK, 1, p(3, 2), true, false);
    let (d1, _) = mv(&d, r, vec![p(3, 2)], false).unwrap();
    assert_eq!(d1.sides[1].lp, 1700);
    assert_eq!(d1.unit(enemy).unwrap().attack_bonus, 1700);
    assert_eq!(d1.sides[0].graveyard, vec![RIRYOKU]);
}

#[test]
fn riryoku_attacked_with_a_goblin_fan_or_a_reverse_trap() {
    // emu RT1-RT4: the Fan of the Riryoku's own side does nothing; the
    // attacker's Fan makes the owner pay and the attacker gains nothing;
    // a Reverse Trap of either side turns the gain into a loss.
    let run = |trap: Option<(u16, u8)>| {
        let mut d = board();
        d.sides[0].lp = 2000;
        put(&mut d, RIRYOKU, 1, p(3, 2), false, false);
        let att = put(&mut d, MID, 0, p(3, 3), true, false);
        let t = trap.map(|(c, side)| put(&mut d, c, side, p(6, 3 + 2 * side), false, false));
        let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
        let a = d1.unit(att).unwrap().clone();
        (
            d1.sides[0].lp,
            d1.sides[1].lp,
            a.attack_bonus,
            t.map(|t| d1.unit(t).unwrap().acted),
        )
    };
    assert_eq!(run(None), (1000, START_LP, 1000, None));
    assert_eq!(run(Some((GOBLIN_FAN, 1))), (1000, START_LP, 1000, Some(false)));
    assert_eq!(run(Some((GOBLIN_FAN, 0))), (2000, START_LP - 1000, 0, Some(true)));
    assert_eq!(run(Some((REVERSE_TRAP, 1))), (1000, START_LP, -1000, Some(true)));
    assert_eq!(run(Some((REVERSE_TRAP, 0))), (1000, START_LP, -1000, Some(true)));
}

// ---- Deck 0x30 cards ----

#[test]
fn rigras_leever_sends_both_hands_to_the_graveyards() {
    // emu RL1.
    let mut d = board();
    d.sides[0].hand = vec![WEAK, MID];
    d.sides[1].hand = vec![SET_CARD];
    let rig = put(&mut d, RIGRAS, 0, p(4, 5), false, false);
    let (d1, _) = mv(&d, rig, vec![], true).unwrap();
    assert!(d1.sides[0].hand.is_empty() && d1.sides[1].hand.is_empty());
    assert_eq!(d1.sides[0].graveyard, vec![WEAK, MID]);
    assert_eq!(d1.sides[1].graveyard, vec![SET_CARD]);
}

#[test]
fn electromagnetic_bagworm_takes_every_enemy_machine() {
    // emu BW1: both enemy Machines, any face; the own one stays.
    let mut d = board();
    let worm = put(&mut d, BAGWORM, 0, p(4, 5), false, false);
    let a = put(&mut d, MACHINE, 1, p(5, 1), true, false);
    let b = put(&mut d, MACHINE, 1, p(6, 1), false, false);
    let own = put(&mut d, MACHINE, 0, p(0, 5), false, false);
    let (d1, _) = mv(&d, worm, vec![], true).unwrap();
    assert_eq!(d1.unit(a).unwrap().controller(), 0);
    assert_eq!(d1.unit(b).unwrap().controller(), 0);
    assert!(!d1.unit(own).unwrap().stolen);
}

#[test]
fn souleater_eats_the_graveyard_monsters_at_its_turn_start() {
    // emu SE1: every monster card leaves both graveyards, +lo each.
    let mut d = board();
    let soul = put(&mut d, SOULEATER, 1, p(4, 1), true, true);
    d.sides[0].graveyard = vec![WEAK, SET_CARD, MID];
    d.sides[1].graveyard = vec![MID];
    let (d1, _) = act(&d, 0, DuelCommand::EndTurn).unwrap();
    assert_eq!(d1.sides[0].graveyard, vec![SET_CARD]);
    assert!(d1.sides[1].graveyard.is_empty());
    let u = d1.unit(soul).unwrap();
    assert_eq!((u.attack_bonus, u.defense_bonus), (600, 600));
}

// ---- The Wicked Worm Beast into an empty group-1 deck ----

#[test]
fn a_group_1_card_stranded_past_slot_0_waits_until_slot_0_is_filled() {
    // 0x1DFCB0 writes past slot 0 of an empty group-1 deck: the count
    // grows, draws return 999 and compact nothing; a later card written to
    // slot 0 is drawn and the compaction brings the stranded ones after it.
    let mut d = board();
    d.place_in_deck(1, 7, WEAK);
    assert_eq!((d.deck_count(1), d.deck_slot(1, 7), d.deck_slot(1, 0)), (1, WEAK, 999));
    assert!(d.sides[1].deck.is_empty());
    d.place_in_deck(1, 3, MID);
    d.place_in_deck(1, 0, SET_CARD);
    assert_eq!(d.sides[1].deck, vec![SET_CARD]);
    assert_eq!(d.draw_front(1), SET_CARD);
    assert_eq!(d.sides[1].deck, vec![MID, WEAK]);
    assert_eq!(d.deck_count(1), 2);
}

// ---- Time Wizard replacing a combatant ----

#[test]
fn time_wizard_turns_its_attacker_into_thousand_dragon_which_fights_on() {
    // emu TW2: the new unit in the attacker's slot wins and advances.
    let mut d = board();
    let tw = put(&mut d, TIME_WIZARD, 1, p(3, 2), false, true);
    let att = put(&mut d, DRAGON, 0, p(3, 3), true, false);
    let slot = d.unit(att).unwrap().slot;
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    assert!(d1.unit(att).is_none() && d1.unit(tw).is_none());
    let new = d1.unit_at(p(3, 2)).unwrap();
    assert_eq!((new.card, new.slot, new.face_up, new.acted), (THOUSAND_DRAGON, slot, true, true));
    assert_eq!(d1.sides[0].graveyard, vec![DRAGON]);
    assert_eq!(d1.sides[1].graveyard, vec![TIME_WIZARD]);
}

#[test]
fn time_wizard_emptying_the_attackers_slot_ends_the_battle() {
    // emu TW4: Thousand Dragon took a lower free slot; no battle, the
    // Time Wizard stays revealed.
    let mut d = board();
    let low = put(&mut d, WEAK, 0, p(0, 5), true, false);
    let tw = put(&mut d, TIME_WIZARD, 1, p(3, 2), false, true);
    let att = put(&mut d, DRAGON, 0, p(3, 3), true, false);
    d.units.retain(|u| u.id != low);
    let (d1, _) = mv(&d, att, vec![p(3, 2)], false).unwrap();
    let t = d1.unit(tw).unwrap();
    assert!(t.face_up && t.committed_face_up);
    let new = d1.unit_at(p(3, 3)).unwrap();
    assert_eq!((new.card, new.slot), (THOUSAND_DRAGON, 1));
    assert_eq!(d1.sides[1].lp, START_LP);
}

// ---- Second pass (REFUSALS-SWEEP.md §5) ----

fn cards2() -> Vec<CardInfo> {
    let mut v = cards();
    let magic = |id: u16, w0: u32| CardInfo {
        kind: class::MAGIC << 5,
        ..with_entry(monster(id, 0, 0, 0), 0, w0, 0)
    };
    v[730] = magic(730, 0x04CA); // Cold Wave: mode 3, Dinosaurs
    v[744] = magic(744, 0x245D); // Monster Recovery: mode 1, monsters
    v[825] = CardInfo {
        kind: class::FAR_TRAP << 5,
        ..with_entry(monster(825, 0, 0, 0), 0, 0x2EFD, 0)
    };
    v[379] = fits(monster(379, kind::FAIRY, 1200, 900), INSECT_IMITATION);
    v[676] = with_entry(monster(676, kind::ROCK, 1000, 1000), 2, 0x54F4, 0);
    v[600] = monster(600, kind::DINOSAUR, 1500, 1000);
    v
}

fn run2(d: &Duel, unit: UnitId, path: Vec<GridPos>, face_up: bool) -> (Duel, Vec<DuelEvent>) {
    d.apply(
        0,
        &DuelCommand::Move {
            unit,
            path,
            face_up,
            defence: None,
        },
        &Catalogue::new(cards2(), []),
    )
    .unwrap()
}

fn put2(d: &mut Duel, card: u16, side: u8, at: GridPos, face_up: bool, defence: bool) -> UnitId {
    let id = d.create(side, card, at, &cards2(), &mut vec![]).unwrap();
    let u = d.units.iter_mut().find(|u| u.id == id).unwrap();
    (
        u.face_up,
        u.committed_face_up,
        u.defence,
        u.committed_defence,
    ) = (face_up, face_up, defence, defence);
    id
}

#[test]
fn monster_recovery_and_cold_wave() {
    // emu MR1: own monsters back to the deck; CW1: every Dinosaur goes.
    let mut d = board();
    let rec = put2(&mut d, 744, 0, p(3, 4), false, false);
    let a = put2(&mut d, WEAK, 0, p(2, 5), false, false);
    let enemy = put2(&mut d, WEAK, 1, p(2, 1), true, false);
    let (d1, _) = run2(&d, rec, vec![], true);
    assert!(d1.unit(a).is_none() && d1.unit(enemy).is_some());
    assert_eq!(d1.deck_count(0), 1);
    assert_eq!(d1.sides[0].graveyard, vec![744]);

    let mut d = board();
    let wave = put2(&mut d, 730, 0, p(3, 4), false, false);
    let own = put2(&mut d, 600, 0, p(1, 5), true, false);
    let foe = put2(&mut d, 600, 1, p(5, 1), false, false);
    let other = put2(&mut d, WEAK, 1, p(4, 1), true, false);
    let (d1, _) = run2(&d, wave, vec![], true);
    assert!(d1.unit(own).is_none() && d1.unit(foe).is_none());
    assert!(d1.unit(other).is_some());
}

#[test]
fn riryoku_and_cocoon_on_own_monsters() {
    // emu OR1/OR2: the opponent pays half its LP to the own monster; OC1
    // (Cocoon onto the monster: Larva not acted), OC2 (the monster onto the
    // Cocoon: acted, on the Cocoon's cell).
    let mut d = board();
    let r = put2(&mut d, RIRYOKU, 0, p(4, 5), false, false);
    let m = put2(&mut d, WEAK, 0, p(4, 4), true, false);
    let (d1, _) = run2(&d, r, vec![p(4, 4)], false);
    assert_eq!(d1.sides[1].lp, START_LP / 2);
    assert_eq!(d1.unit(m).unwrap().attack_bonus, (START_LP / 2) as i16);

    let mut d = board();
    let m = put2(&mut d, WEAK, 0, p(4, 5), true, false);
    put2(&mut d, RIRYOKU, 0, p(4, 4), false, false);
    let (d1, _) = run2(&d, m, vec![p(4, 4)], false);
    let u = d1.unit(m).unwrap();
    assert_eq!((u.position, u.attack_bonus, u.acted), (p(4, 4), (START_LP / 2) as i16, true));

    let mut d = board();
    let c = put2(&mut d, COCOON, 0, p(4, 5), false, false);
    put2(&mut d, DRAGON, 0, p(4, 4), true, false);
    let (d1, _) = run2(&d, c, vec![p(4, 4)], false);
    let l = d1.unit_at(p(4, 4)).unwrap();
    assert_eq!((l.card, l.acted), (427, false));

    let mut d = board();
    let m = put2(&mut d, DRAGON, 0, p(4, 5), true, false);
    put2(&mut d, COCOON, 0, p(4, 4), false, false);
    let (d1, _) = run2(&d, m, vec![p(4, 4)], false);
    let l = d1.unit_at(p(4, 4)).unwrap();
    assert_eq!((l.card, l.acted), (427, true));
    assert!(d1.unit_at(p(4, 5)).is_none());
}

#[test]
fn a_group_0_royal_decree_overrides_the_riryoku_equip_trap() {
    // emu RD1: the attacker (g0, slot != 1) is seen by the group-0 Decree,
    // which reacts; the Riryoku resolves as without the Fan.
    let mut d = board();
    d.sides[0].lp = 2000;
    put2(&mut d, RIRYOKU, 1, p(3, 2), false, false);
    put2(&mut d, WEAK, 0, p(0, 6), false, true); // slot 1
    let att = put2(&mut d, MID, 0, p(3, 3), true, false);
    let fan = put2(&mut d, GOBLIN_FAN, 0, p(6, 3), false, false);
    let decree = put2(&mut d, 825, 0, p(6, 4), false, false);
    let (d1, _) = run2(&d, att, vec![p(3, 2)], false);
    assert_eq!(d1.sides[0].lp, 1000);
    assert_eq!(d1.unit(att).unwrap().attack_bonus, 1000);
    assert!(d1.unit(decree).unwrap().acted && !d1.unit(fan).unwrap().acted);
}

#[test]
fn wing_egg_elf_draws_one_of_six_elves() {
    // emu II2-II4 (0x24ADF0).
    let mut d = board();
    d.seed_rng(777);
    put2(&mut d, INSECT_IMITATION, 1, p(3, 2), false, false);
    let att = put2(&mut d, 379, 0, p(3, 3), true, false);
    let mut x = 777;
    let want = [0x23, 0x4F, 0x2C, 0x5C, 0x17B, 0x18A][(next_rand31(&mut x) % 6) as usize];
    let (d1, _) = run2(&d, att, vec![p(3, 2)], false);
    assert_eq!(d1.unit_at(p(3, 2)).unwrap().card, want);
}

#[test]
fn carat_idol_pulls_every_unit_one_cell_round_by_round() {
    // emu CI1 (the 765-f3 board around an Idol at (4,5), side 0's turn
    // start): column step first, the row step when the column cell is
    // held, nothing when the round already chose that cell.
    let mut d = board();
    let idol = put2(&mut d, 676, 1, p(4, 5), true, true);
    let neko = put2(&mut d, WEAK, 1, p(2, 6), false, false);
    let hs1 = put2(&mut d, WEAK, 0, p(2, 4), true, false);
    let hs2 = put2(&mut d, WEAK, 0, p(3, 2), true, true);
    let flea = put2(&mut d, WEAK, 0, p(2, 3), false, true);
    let pact = put2(&mut d, SET_CARD, 0, p(4, 1), false, true);
    let set_order = |d: &mut Duel, id: UnitId, o: u8| {
        d.units.iter_mut().find(|u| u.id == id).unwrap().summon_order = o;
    };
    for (id, o) in [(neko, 11), (hs1, 4), (hs2, 3), (flea, 5), (pact, 9)] {
        set_order(&mut d, id, o);
    }
    let d1 = d
        .apply(0, &DuelCommand::EndTurn, &Catalogue::new(cards2(), []))
        .unwrap()
        .0;
    let at = |id| d1.unit(id).unwrap().position;
    assert_eq!(at(hs1), p(3, 4));
    assert_eq!(at(neko), p(2, 5));
    assert_eq!(at(hs2), p(4, 2));
    assert_eq!(at(flea), p(3, 3));
    assert_eq!(at(pact), p(4, 1));
    assert_eq!(at(idol), p(4, 5));
}

#[test]
fn jowls_of_dark_demise_possesses_its_attacker_until_it_dies() {
    // emu JD1 (taken, the Jowls gone), JD4 (still taken after a turn
    // start), JD3 (destroyed in battle: a Jowls face-up beside its new
    // controller's leader).
    let mut v = cards2();
    v[672] = with_entry(monster(672, kind::FIEND, 1600, 1800), 3, 0x51FD, 0);
    let cat = Catalogue::new(v, []);
    let mut d = board();
    let jowls = d.create(1, 672, p(3, 2), &cat, &mut vec![]).unwrap();
    {
        let u = d.units.iter_mut().find(|u| u.id == jowls).unwrap();
        (u.defence, u.committed_defence) = (true, true);
    }
    let att = d.create(0, MID, p(3, 3), &cat, &mut vec![]).unwrap();
    {
        let u = d.units.iter_mut().find(|u| u.id == att).unwrap();
        (u.face_up, u.committed_face_up) = (true, true);
    }
    let mv = |d: &Duel, side: u8, unit: UnitId, to: GridPos| {
        d.apply(
            side,
            &DuelCommand::Move {
                unit,
                path: vec![to],
                face_up: false,
                defence: None,
            },
            &cat,
        )
        .unwrap()
        .0
    };
    let d1 = mv(&d, 0, att, p(3, 2));
    assert!(d1.unit(jowls).is_none());
    assert!(d1.sides[1].graveyard.is_empty());
    let a = d1.unit(att).unwrap();
    assert_eq!((a.controller(), a.position), (1, p(3, 3)));
    let d2 = d1.apply(0, &DuelCommand::EndTurn, &cat).unwrap().0;
    assert_eq!(d2.unit(att).unwrap().controller(), 1);
    // Side 1 ends its turn; side 0 kills the possessed monster.
    let mut d3 = d2.apply(1, &DuelCommand::EndTurn, &cat).unwrap().0;
    assert_eq!(d3.unit(att).unwrap().controller(), 1);
    let big = d3.create(0, LEADER, p(4, 3), &cat, &mut vec![]).unwrap();
    {
        let u = d3.units.iter_mut().find(|u| u.id == big).unwrap();
        (u.face_up, u.committed_face_up) = (true, true);
    }
    let d4 = mv(&d3, 0, big, p(3, 3));
    assert!(d4.unit(att).is_none());
    assert_eq!(d4.sides[0].graveyard, vec![MID]);
    let back = d4.units.iter().find(|u| u.card == 672).unwrap();
    assert_eq!((back.side, back.face_up), (1, true));
}

// ---- Third pass (REFUSALS-SWEEP.md §7) ----

#[test]
fn mask_of_darkness_onto_an_own_unit_picks_from_the_graveyard_then_combines() {
    // emu R16b (Mask of Darkness 305 flipped UP onto an own Red-Eyes, the
    // renderer's animation stood in): the pick (state 0x2D) creates the
    // magic card on the leader's neighbour (3,5), acted and face-down; the
    // failed combination sends the partner to the graveyard and the Mask
    // ends on its cell, face-up and acted.
    let mut v = cards();
    v[305] = with_entry(monster(305, kind::FIEND, 900, 400), 3, 0x145E, 0);
    v[763] = CardInfo {
        kind: class::POWER_UP << 5,
        ..monster(763, 0, 0, 0)
    };
    let cat = Catalogue::new(v, []);
    let mut d = board();
    d.sides[0].graveyard = vec![WEAK, 763];
    let mask = d.create(0, 305, p(4, 5), &cat, &mut vec![]).unwrap();
    let partner = d.create(0, WEAK, p(4, 4), &cat, &mut vec![]).unwrap();
    d.units.iter_mut().find(|u| u.id == partner).unwrap().face_up = true;
    let (d1, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: mask,
                path: vec![p(4, 4)],
                face_up: true,
                defence: None,
            },
            &cat,
        )
        .unwrap();
    assert!(d1.unit(partner).is_none());
    let m = d1.unit(mask).unwrap();
    assert_eq!((m.position, m.face_up, m.acted), (p(4, 4), true, true));
    assert_eq!(d1.sides[0].graveyard, vec![WEAK, 763, WEAK]);
    let Some(Pending::GraveyardCopy { at, .. }) = d1.pending.clone() else {
        panic!("the graveyard pick is open: {:?}", d1.pending);
    };
    let (d2, _) = d1
        .apply(
            0,
            &DuelCommand::Pick {
                pick: Pick::Graveyard { side: 0, index: 1 },
            },
            &cat,
        )
        .unwrap();
    let copy = d2.unit_at(at).unwrap();
    assert_eq!((copy.card, copy.face_up, copy.acted), (763, false, true));
    assert_eq!(d2.sides[0].graveyard, vec![WEAK, WEAK]);
}
