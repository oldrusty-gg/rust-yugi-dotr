//! Flip types 0x36/0x37 (Blue-Eyed Silver Zombie 112, Mech Mole Zombie 116,
//! Tao the Chanter 90), Sword of Dragon's Soul 791 (category 1) and Riryoku
//! 789 (category 0, value 0): FLIP-112-POWERUPS-789-791.md. Synthetic cards
//! shaped like the originals; the effect words are the traced ones, the
//! stats are invented.

use super::*;

fn monster(id: u16, kind: u8, attribute: u8, attack: u16, defense: u16) -> CardInfo {
    CardInfo {
        id,
        kind,
        level: 4,
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

fn flipper(id: u16, word0: u32, word1: u32) -> CardInfo {
    let mut c = monster(id, kind::ZOMBIE, 1, 300, 300);
    c.entries[3] = Some([word0, word1]);
    c.effects[3] = true;
    c
}

fn power_up(id: u16, category: u8, value: u16) -> CardInfo {
    CardInfo {
        power_up: Some(PowerUp { category, value }),
        ..monster(id, 64, 0, 0, 0)
    }
}

const LEADER_A: u16 = 1;
const LEADER_B: u16 = 0;
const WARRIOR: u16 = 2; // 1400/1200, attribute 3
const DRAGON: u16 = 3; // 2500/2000, attribute 0 (light)
const LIGHT: u16 = 4; // Fairy 800/800, attribute 0
const WEAK: u16 = 5; // Beast 500/400
const MODIFIER_2C: u16 = 6; // entry-2 0x3B filter 0x2C, +100 (Kryuel-like)
const SILVER: u16 = 112; // e3 0x365D / 2: mode 1, filter 0x1D, kind := Zombie
const MOLE: u16 = 116; // e3 0x369A / 2: mode 2, filter 0x1A (battle opponent)
const TAO: u16 = 90; // e3 0x37D4 / 1: mode 3, filter 0x14, attribute := 1
const SWORD_DRAGON: u16 = 791; // category 1, value 2
const JAVELIN: u16 = 792; // category 1, value 3
const RIRYOKU: u16 = 789; // category 0, value 0
const SWORD: u16 = 752; // category 0, value 500

fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..820)
        .map(|i| monster(i, kind::MACHINE, 3, 100, 100))
        .collect();
    v[usize::from(LEADER_A)] = monster(LEADER_A, kind::SPELLCASTER, 1, 2500, 2100);
    v[usize::from(LEADER_B)] = monster(LEADER_B, kind::DRAGON, 0, 3000, 2500);
    v[usize::from(WARRIOR)] = monster(WARRIOR, kind::WARRIOR, 3, 1400, 1200);
    v[usize::from(DRAGON)] = monster(DRAGON, kind::DRAGON, 0, 2500, 2000);
    v[usize::from(LIGHT)] = monster(LIGHT, kind::FAIRY, 0, 800, 800);
    v[usize::from(WEAK)] = monster(WEAK, kind::BEAST, 3, 500, 400);
    v[usize::from(MODIFIER_2C)] = CardInfo {
        entries: [None, None, Some([0x3B2C, 100]), None, None],
        effects: [false, false, true, false, false],
        ..monster(MODIFIER_2C, kind::FIEND, 1, 800, 600)
    };
    v[usize::from(SILVER)] = flipper(SILVER, 0x365D, 2);
    v[usize::from(MOLE)] = flipper(MOLE, 0x369A, 2);
    v[usize::from(TAO)] = CardInfo {
        kind: kind::SPELLCASTER,
        attribute: 3,
        ..flipper(TAO, 0x37D4, 1)
    };
    v[usize::from(SWORD_DRAGON)] = power_up(SWORD_DRAGON, 1, 2);
    v[usize::from(JAVELIN)] = power_up(JAVELIN, 1, 3);
    v[usize::from(RIRYOKU)] = power_up(RIRYOKU, 0, 0);
    v[usize::from(SWORD)] = power_up(SWORD, 0, 500);
    for pu in [SWORD_DRAGON, JAVELIN, RIRYOKU, SWORD] {
        let bit = usize::from(pu - 0x2F0);
        for id in [WARRIOR, WEAK] {
            v[usize::from(id)].equip_bits[bit >> 3] |= 1 << (bit & 7);
        }
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

fn setup(decks: [Vec<u16>; 2]) -> DuelSetup {
    DuelSetup {
        terrain: vec![terrain::NORMAL; 49],
        leaders: [LEADER_A, LEADER_B],
        decks,
        first: 0,
        leader_abilities: [vec![], vec![]],
        controllers: [PAD_CONTROLLER; 2],
    }
}

fn board(units: Vec<DuelUnit>) -> Duel {
    let (mut d, _) = Duel::new(setup([vec![], vec![]]), &cards()).unwrap();
    d.units.extend(units);
    d.next_unit = 100;
    d
}

fn act(d: &Duel, unit: u16, path: Vec<GridPos>, face_up: bool) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(
        0,
        &DuelCommand::Move {
            unit: UnitId(unit),
            path,
            face_up,
            defence: None,
        },
        &cards(),
    )
}

fn kind_of(d: &Duel, id: u16) -> u8 {
    d.unit_card(d.unit(UnitId(id)).unwrap(), &cards()).kind
}

fn attribute_of(d: &Duel, id: u16) -> u8 {
    d.unit_card(d.unit(UnitId(id)).unwrap(), &cards()).attribute
}

fn activations(events: &[DuelEvent]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, DuelEvent::Activated { .. }))
        .count()
}

// ---- flip types 0x36 / 0x37 ----------------------------------------------

#[test]
fn silver_zombie_makes_every_other_own_monster_a_zombie() {
    // emu A: own monsters in slots 1..5 (face-down or up) become kind 2;
    // the leader, the own power-up, enemy monsters and itself are untouched.
    let d = board(vec![
        placed(10, SILVER, 0, 1, p(4, 5), false, false),
        placed(11, WARRIOR, 0, 2, p(2, 6), false, false),
        placed(12, DRAGON, 0, 3, p(1, 6), true, true),
        placed(13, SWORD, 0, 6, p(0, 6), false, false),
        placed(14, DRAGON, 1, 1, p(3, 1), false, true),
    ]);
    let (d1, events) = act(&d, 10, vec![], true).unwrap();
    assert_eq!(activations(&events), 1);
    assert_eq!(kind_of(&d1, 11), kind::ZOMBIE);
    assert_eq!(kind_of(&d1, 12), kind::ZOMBIE);
    assert_eq!(kind_of(&d1, 13), 64);
    assert_eq!(kind_of(&d1, 14), kind::DRAGON);
    assert_eq!(kind_of(&d1, 0), kind::SPELLCASTER); // leader A (unit 0)
    assert!(d1.unit_record(UnitId(10)).is_none());
    // Stats and the card id stay; only the record's kind byte changes.
    let u = d1.unit(UnitId(11)).unwrap();
    assert_eq!((u.card, u.attack_bonus), (WARRIOR, 0));
    assert!(d1.unit(UnitId(10)).unwrap().committed_face_up);
}

#[test]
fn a_rewritten_kind_is_what_terrain_and_the_dragon_filters_read() {
    // Dark terrain favours Zombies: a Warrior turned Zombie gains 500.
    let mut d = board(vec![
        placed(10, SILVER, 0, 1, p(4, 5), false, false),
        placed(11, WARRIOR, 0, 2, p(2, 5), false, true),
    ]);
    d.terrain = vec![terrain::DARK; 49];
    let before = {
        let u = d.unit(UnitId(11)).unwrap();
        d.effective(u, u.position, true, &cards())
    };
    let (d1, _) = act(&d, 10, vec![], true).unwrap();
    let u = d1.unit(UnitId(11)).unwrap();
    assert_eq!(d1.effective(u, u.position, true, &cards()), before + 500);
    // The record survives the turn and serialises.
    let json = serde_json::to_string(&d1).unwrap();
    let back: Duel = serde_json::from_str(&json).unwrap();
    assert_eq!(kind_of(&back, 11), kind::ZOMBIE);
    // A duel without rewritten records serialises as before.
    assert!(!serde_json::to_string(&d).unwrap().contains("records"));
}

#[test]
fn silver_zombie_without_other_own_monsters_does_nothing() {
    let d = board(vec![
        placed(10, SILVER, 0, 1, p(4, 5), false, false),
        placed(14, DRAGON, 1, 1, p(3, 1), false, true),
    ]);
    let (d1, events) = act(&d, 10, vec![], true).unwrap();
    assert_eq!(activations(&events), 0);
    assert!(d1.records.is_empty());
}

#[test]
fn tao_rewrites_the_attribute_of_light_monsters_of_both_sides() {
    // emu B: mode 3 filter 0x14 (attribute 0) → +0x3F := 1; leaders and
    // non-monsters are never candidates.
    let d = board(vec![
        placed(10, TAO, 0, 1, p(4, 5), false, false),
        placed(11, LIGHT, 0, 2, p(2, 6), false, false),
        placed(12, WARRIOR, 0, 3, p(1, 6), false, true),
        placed(13, SWORD, 0, 6, p(0, 6), false, false),
        placed(14, DRAGON, 1, 1, p(3, 1), false, true),
    ]);
    let (d1, events) = act(&d, 10, vec![], true).unwrap();
    assert_eq!(activations(&events), 1);
    assert_eq!(attribute_of(&d1, 11), 1);
    assert_eq!(attribute_of(&d1, 14), 1);
    assert_eq!(attribute_of(&d1, 12), 3);
    assert_eq!(attribute_of(&d1, 13), 0);
    assert_eq!(attribute_of(&d1, 1), 0); // leader B (unit 1, attribute 0) untouched
    assert_eq!(kind_of(&d1, 11), kind::FAIRY);
}

#[test]
fn mech_mole_retypes_only_its_battle_opponent() {
    // emu C: a manual flip (role 0) has no opponent: nothing.
    let d = board(vec![
        placed(10, MOLE, 0, 1, p(4, 5), false, false),
        placed(11, WARRIOR, 0, 2, p(2, 6), false, true),
    ]);
    let (d1, events) = act(&d, 10, vec![], true).unwrap();
    assert_eq!(activations(&events), 0);
    assert!(d1.records.is_empty());
    // In battle (role 1) the opponent becomes a Zombie and the battle goes
    // on: the 1400 attacker still beats the 300 defence.
    let d = board(vec![
        placed(11, WARRIOR, 0, 1, p(3, 4), false, true),
        placed(20, MOLE, 1, 1, p(3, 3), true, false),
    ]);
    let (d2, events) = act(&d, 11, vec![p(3, 3)], false).unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            unit: UnitId(20),
            target: Some(UnitId(11)),
            ..
        }
    )));
    assert_eq!(kind_of(&d2, 11), kind::ZOMBIE);
    assert!(d2.unit(UnitId(20)).is_none());
}

// ---- summon-list equips ---------------------------------------------------

fn summon_list(d: &Duel, hand: Vec<usize>) -> (Duel, Vec<DuelEvent>) {
    let c = cards();
    let (d, _) = d
        .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &c)
        .unwrap();
    d.apply(0, &DuelCommand::ChooseSummon { hand }, &c).unwrap()
}

fn with_hand(hand: Vec<u16>) -> Duel {
    let (mut d, _) = Duel::new(setup([hand, vec![]]), &cards()).unwrap();
    d.sides[0].stars = 12;
    d
}

#[test]
fn sword_of_dragons_soul_writes_entry_0_in_either_order() {
    // emu A/B: the summoned record carries e0 = {0x480, 0}; no stat change.
    for hand in [vec![0, 1], vec![1, 0]] {
        let d = with_hand(vec![WEAK, SWORD_DRAGON, 2, 2, 2]);
        let (d1, events) = summon_list(&d, hand);
        let u = d1.unit_at(p(3, 5)).unwrap().clone();
        assert_eq!((u.card, u.attack_bonus, u.defense_bonus), (WEAK, 0, 0));
        let c = cards();
        let rec = d1.unit_card(&u, &c);
        assert_eq!(rec.entries[0], Some([0x480, 0]));
        assert!(rec.effects[0]);
        assert_eq!(d1.sides[0].graveyard, [SWORD_DRAGON]);
        assert!(events.iter().any(|e| matches!(
            e,
            DuelEvent::Equipped {
                power_up: SWORD_DRAGON,
                value: 0,
                ..
            }
        )));
    }
}

#[test]
fn an_engraved_monster_destroys_dragons_it_battles() {
    // e0 type 4 mode 2 filter 0 (0x263B48): against a Dragon the pre-pass
    // decides the battle whatever the stats; against others it is a plain
    // battle.
    let d = with_hand(vec![WEAK, SWORD_DRAGON, 2, 2, 2]);
    let (mut d1, _) = summon_list(&d, vec![0, 1]);
    let id = d1.unit_at(p(3, 5)).unwrap().id;
    d1.units.push(placed(20, DRAGON, 1, 1, p(3, 4), false, true));
    let (d2, events) = act(&d1, id.0, vec![p(3, 4)], false).unwrap();
    assert!(d2.unit(UnitId(20)).is_none());
    assert!(d2.unit(id).is_some());
    assert!(events.iter().any(|e| matches!(
        e,
        DuelEvent::Activated {
            target: Some(UnitId(20)),
            ..
        }
    )));
    assert_eq!(d2.sides[1].lp, START_LP);
    // A Warrior opponent: an ordinary battle (500 < 1400, the weak one dies).
    d1.units.retain(|u| u.id != UnitId(20));
    d1.units.push(placed(21, WARRIOR, 1, 1, p(3, 4), false, true));
    let (d3, _) = act(&d1, id.0, vec![p(3, 4)], false).unwrap();
    assert!(d3.unit(id).is_none());
}

#[test]
fn a_later_category_1_equip_overwrites_entry_0() {
    // 0x249F3C writes over the record: Javelin's Fiend filter replaces the
    // Dragon one; a fusion/replacement would start from the card again.
    let d = with_hand(vec![WEAK, SWORD_DRAGON, JAVELIN, 2, 2]);
    let (d1, _) = summon_list(&d, vec![0, 1, 2]);
    let u = d1.unit_at(p(3, 5)).unwrap().clone();
    assert_eq!(d1.unit_card(&u, &cards()).entries[0], Some([0x487, 0]));
}

#[test]
fn riryoku_drains_half_the_opponents_lp_into_the_monster() {
    // emu C/D: 4000 → +2000 and LP 2000; 3001 → +1500 and LP 1501.
    for (lp, gain) in [(4000u16, 2000i16), (3001, 1500)] {
        let mut d = with_hand(vec![WEAK, RIRYOKU, 2, 2, 2]);
        d.sides[1].lp = lp;
        let (d1, events) = summon_list(&d, vec![0, 1]);
        let u = d1.unit_at(p(3, 5)).unwrap();
        assert_eq!((u.attack_bonus, u.defense_bonus), (gain, gain));
        assert_eq!(d1.sides[1].lp, lp - gain as u16);
        assert_eq!(d1.sides[0].lp, START_LP);
        assert_eq!(d1.sides[0].graveyard, [RIRYOKU]);
        assert!(events.iter().any(|e| matches!(
            e,
            DuelEvent::Damage { side: 1, amount } if *amount == gain as u16
        )));
    }
}

#[test]
fn a_second_riryoku_halves_the_already_drained_lp() {
    // emu E: 4000 → 2000 → 1000, the monster +3000.
    let d = with_hand(vec![WEAK, RIRYOKU, RIRYOKU, 2, 2]);
    let (d1, _) = summon_list(&d, vec![0, 1, 2]);
    assert_eq!(d1.unit_at(p(3, 5)).unwrap().attack_bonus, 3000);
    assert_eq!(d1.sides[1].lp, 1000);
}

#[test]
fn riryoku_goes_through_the_lp_loss_modifiers() {
    // 0x24A750 → 0x268A10(0x2C, 2000): a face-up committed-defence +100
    // loss modifier makes it 2100 for both the gain and the loss.
    let mut d = with_hand(vec![WEAK, RIRYOKU, 2, 2, 2]);
    d.units
        .push(placed(30, MODIFIER_2C, 1, 1, p(0, 0), true, true));
    let (d1, _) = summon_list(&d, vec![0, 1]);
    assert_eq!(d1.unit_at(p(3, 5)).unwrap().attack_bonus, 2100);
    assert_eq!(d1.sides[1].lp, 1900);
}

#[test]
fn riryoku_on_a_lifted_field_unit_keeps_its_record() {
    // emu F: field unit + 791 + 789 → e0 written, +LP/2, same unit.
    let mut d = board(vec![placed(10, WEAK, 0, 1, p(3, 5), false, true)]);
    d.sides[0].hand = vec![SWORD_DRAGON, RIRYOKU];
    d.sides[0].stars = 12;
    d.sides[1].lp = 2000;
    let (d1, _) = summon_list(&d, vec![0, 1]);
    let u = d1.unit(UnitId(10)).unwrap().clone();
    assert_eq!(u.attack_bonus, 1000);
    assert_eq!(d1.sides[1].lp, 1000);
    assert_eq!(d1.unit_card(&u, &cards()).entries[0], Some([0x480, 0]));
}

#[test]
fn a_category_1_power_up_engraves_a_field_monster_too() {
    // Event 0x24 (0x24A410 → 0x218): the set power-up moved onto an own
    // monster is spent and writes e0; no stat change.
    let d = board(vec![
        placed(10, SWORD_DRAGON, 0, 6, p(3, 5), false, false),
        placed(11, WEAK, 0, 1, p(3, 4), false, true),
    ]);
    let (d1, _) = act(&d, 10, vec![p(3, 4)], false).unwrap();
    let u = d1.unit(UnitId(11)).unwrap().clone();
    assert_eq!(d1.unit_card(&u, &cards()).entries[0], Some([0x480, 0]));
    assert_eq!(u.attack_bonus, 0);
    assert!(d1.unit(UnitId(10)).is_none());
}
