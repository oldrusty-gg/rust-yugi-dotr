//! Ability 8/14 regressions use invented cards; the private oracle reads
//! independently executed PAL function results from a user-local file.

use super::*;

fn card(id: u16, kind: u8, level: u8) -> CardInfo {
    CardInfo {
        id,
        kind,
        level,
        attribute: 0,
        attack: 1800,
        defense: 1300,
        effects: [false; 5],
        toon: false,
        kind_index: 0,
        entries: [None; 5],
        equip_bits: [0; 7],
        power_up: None,
    }
}

fn pos(p: [u8; 2]) -> GridPos {
    GridPos {
        column: p[0],
        row: p[1],
    }
}

fn board(kinds: [u8; 2], levels: [u8; 2], abilities: [Vec<u8>; 2]) -> (Duel, Vec<CardInfo>) {
    let cards = vec![
        card(0, kinds[0], levels[0]),
        card(1, kinds[1], levels[1]),
        card(2, 7, 4),
        card(3, 0, 1),
    ];
    let (mut duel, _) = Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0, 1],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: abilities,
            controllers: [PAD_CONTROLLER; 2],
        },
        &cards,
    )
    .unwrap();
    duel.units[0].position = pos([3, 4]);
    duel.units[1].position = pos([3, 2]);
    let mut target = duel.units[0].clone();
    target.id = UnitId(2);
    target.card = 2;
    target.slot = 1;
    target.position = pos([3, 3]);
    duel.units.push(target);
    (duel, cards)
}

fn context(option: u32) -> queries::Context {
    queries::Context {
        option_28: option,
        option_29: 3,
        ..Default::default()
    }
}

#[test]
fn ability_14_reduces_both_stats_and_combines_with_10_at_the_queried_cell() {
    // [emu] 0x273420: a level-3 Fairy's +300 and level-4 opposing
    // Spellcaster's -400 combine even for a face-down, spellbound target.
    let (mut d, c) = board([7, 1], [3, 4], [vec![10], vec![14]]);
    d.units[2].face_up = false;
    d.units[2].committed_face_up = false;
    d.units[2].spellbind = 3;
    let u = &d.units[2];
    assert_eq!(d.leader_aura(u, pos([3, 3]), &c), -100);
    assert_eq!(d.effective(u, pos([3, 3]), true, &c), 1700);
    assert_eq!(d.effective(u, pos([3, 3]), false, &c), 1200);
    // Battle reads use the target cell, not the attacker's stored position.
    assert_eq!(d.leader_aura(u, pos([3, 4]), &c), 300);
    assert_eq!(d.leader_aura(u, pos([3, 0]), &c), 0);
    d.sides[1].leader_abilities.push(9);
    assert_eq!(d.leader_aura(&d.units[2], pos([3, 0]), &c), -400);
    // Stolen units use their current controller for both halves, in HUD and rules.
    d.units[2].stolen = true;
    assert_eq!(d.leader_aura(&d.units[2], pos([3, 3]), &c), 0);
    let ctx = context(3);
    let stats = queries::Query::new(&d, &c, &ctx)
        .stats(queries::Record::Field(&d.units[2]), pos([3, 3]))
        .unwrap();
    assert_eq!((stats.attack, stats.defence), (1800, 1300));
}

#[test]
fn ability_14_changes_the_resolved_battle_and_life_point_loss() {
    let (mut d, mut c) = board([7, 1], [3, 4], [vec![10], vec![14]]);
    d.units[2].position = pos([2, 3]);
    let mut defender = d.units[2].clone();
    defender.id = UnitId(3);
    defender.card = 3;
    defender.side = 1;
    defender.position = pos([3, 3]);
    c[3].kind = 2;
    c[3].attack = 1750;
    d.units.push(defender);
    let (after, events) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(2),
                path: vec![pos([3, 3])],
                face_up: false,
                defence: None,
            },
            &c,
        )
        .unwrap();
    // Original aura at the battle cell is +300 -400: 1700 loses to 1750.
    // Omitting #14 instead gives 2100 and destroys the defender.
    assert!(events.iter().any(|e| matches!(e, DuelEvent::Battle { .. })));
    assert!(after.unit(UnitId(2)).is_none());
    assert!(after.unit(UnitId(3)).is_some());
    assert_eq!([after.sides[0].lp, after.sides[1].lp], [3950, 4000]);
}

#[test]
fn ability_8_pays_reduced_stars_and_stores_the_discount_on_the_summoned_unit() {
    let (mut d, mut c) = board([4, 1], [6, 4], [vec![8], vec![]]);
    c[2].kind = 4;
    d.sides[0].deck = vec![2];
    let cell = pos([2, 4]);
    let (d, _) = d.apply(0, &DuelCommand::OpenSummon { cell }, &c).unwrap();
    let (d, _) = d
        .apply(0, &DuelCommand::ChooseSummon { hand: vec![0] }, &c)
        .unwrap();
    let unit = d.unit_at(cell).unwrap();
    assert_eq!(d.sides[0].stars, 2);
    assert_eq!(unit.discount, 2);
    assert_eq!(d.unit_cost(unit, &c), 2);
}

#[test]
fn ability_8_membership_threshold_and_suppression_affect_costs() {
    let (mut d, mut c) = board([4, 1], [6, 4], [vec![8], vec![]]);
    c[2].kind = 4;
    assert_eq!(d.summon_cost(0, &c[2], &c), 2);
    d.units[0].discount = 1; // Effective leader level 5, below the race-4 threshold.
    assert_eq!(d.summon_cost(0, &c[2], &c), 3);
    let ctx = context(3);
    assert_eq!(queries::Query::new(&d, &c, &ctx).hand_card_cost(0, 2), 3);
    d.units[0].discount = 0;
    let mut source = d.units[2].clone();
    source.id = UnitId(3);
    source.card = 3;
    source.side = 1;
    source.slot = 2;
    source.defence = true;
    source.committed_defence = true;
    source.face_up = false; // Current face-up bit is not required; committed one is.
    source.committed_face_up = true;
    c[3].entries[2] = Some([0x5800, 0]);
    d.units.push(source);
    assert_eq!(d.summon_cost(0, &c[2], &c), 4);
    d.units[3].stolen = true;
    assert_eq!(d.summon_cost(0, &c[2], &c), 2);
    c[2].kind = 5;
    assert_eq!(d.summon_cost(0, &c[2], &c), 4);
    c[2].kind = 4 | 0x20;
    assert_eq!(d.summon_cost(0, &c[2], &c), 4);
}

#[derive(Deserialize)]
struct DiscountCase {
    kind: u8,
    level: u8,
    stored: u8,
    target_kind: u8,
    discount: u8,
    leader_discount: u8,
}

#[derive(Deserialize)]
struct Suppressor {
    owner: u8,
    stolen: bool,
    flags: u8,
}

#[derive(Deserialize)]
struct AuraCase {
    name: String,
    kinds: [u8; 2],
    levels: [u8; 2],
    abilities: [Vec<u8>; 2],
    discounts: [u8; 2],
    pos: [[u8; 2]; 2],
    cell: [u8; 2],
    side: u8,
    slot: u8,
    kind: u8,
    stolen: bool,
    suppress: Option<Suppressor>,
    option: u32,
    expected: i32,
}

#[derive(Deserialize)]
struct Oracle {
    weakens: Vec<(i32, u8, i32)>,
    discounts: Vec<DiscountCase>,
    auras: Vec<AuraCase>,
    ranks: Vec<(u8, u16, i32, i32)>,
}

#[test]
#[ignore = "requires private original-instruction results in ROSES_ABILITY_ORACLE"]
fn private_abilities_match_original_instruction_oracle() {
    let path = std::env::var("ROSES_ABILITY_ORACLE").expect("ROSES_ABILITY_ORACLE");
    let oracle: Oracle = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(oracle.weakens.len(), 32 * 32);
    for (leader, target, expected) in oracle.weakens {
        assert_eq!(
            i32::from(queries::weakens(leader, target)),
            expected,
            "leader {leader}, target {target}"
        );
    }
    for case in oracle.discounts {
        let (mut d, mut c) = board([case.kind, 1], [case.level, 4], [vec![8], vec![]]);
        d.units[0].discount = case.stored;
        c[2].kind = case.target_kind;
        assert_eq!(d.leader_discount(0, &c), case.leader_discount);
        assert_eq!(
            d.summon_discount(0, 2, &c),
            case.discount,
            "race {}, level {}, stored {}, target {}",
            case.kind,
            case.level,
            case.stored,
            case.target_kind
        );
        let ctx = context(3);
        assert_eq!(
            queries::Query::new(&d, &c, &ctx).hand_card_cost(0, 2),
            4 - i32::from(case.discount)
        );
    }
    for case in oracle.auras {
        let (mut d, mut c) = board(case.kinds, case.levels, case.abilities);
        for side in 0..2 {
            d.units[side].position = pos(case.pos[side]);
            d.units[side].discount = case.discounts[side];
        }
        let index = if case.slot == 0 {
            usize::from(case.side)
        } else {
            2
        };
        d.units[index].side = case.side;
        d.units[index].slot = case.slot;
        d.units[index].stolen = case.stolen;
        // The original changes only the target record's kind, not its leader
        // card's kind. The generated leader case uses an unchanged kind.
        c[usize::from(d.units[index].card)].kind = case.kind;
        if let Some(s) = case.suppress {
            let mut source = d.units[2].clone();
            source.id = UnitId(3);
            source.card = 3;
            source.side = s.owner;
            source.slot = 2;
            source.stolen = s.stolen;
            source.defence = s.flags & 8 != 0;
            source.committed_face_up = s.flags & 0x10 != 0;
            source.committed_defence = s.flags & 0x20 != 0;
            c[3].entries[2] = Some([0x5800, 0]);
            d.units.push(source);
        }
        let cell = pos(case.cell);
        if case.option == 3 {
            assert_eq!(
                d.leader_aura(&d.units[index], cell, &c),
                case.expected,
                "rules: {}",
                case.name
            );
        }
        let ctx = context(case.option);
        let stats = queries::Query::new(&d, &c, &ctx)
            .stats(queries::Record::Field(&d.units[index]), cell)
            .unwrap();
        assert_eq!(stats.attack - 1800, case.expected, "HUD: {}", case.name);
        assert_eq!(
            stats.defence - 1300,
            case.expected,
            "HUD DEF: {}",
            case.name
        );
    }
    for (ability, word, rank, expected) in oracle.ranks {
        let mut row = [0xFFFF; 20];
        row[if ability == 8 { 9 } else { 15 }] = word;
        assert_eq!(
            i32::from(queries::record_ability(&row, ability, rank)),
            expected
        );
    }
}
