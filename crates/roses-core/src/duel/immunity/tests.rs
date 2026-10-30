//! Synthetic command regressions and private original-instruction oracle.

use crate::duel::*;

fn catalogue() -> Vec<CardInfo> {
    let mut cards: Vec<CardInfo> = (0..761)
        .map(|id| CardInfo {
            id,
            kind: kind::WARRIOR,
            level: 3,
            attribute: 0,
            attack: 900,
            defense: 700,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        })
        .collect();
    cards[1].kind = kind::DRAGON;
    cards[1].attack = 1200;
    cards[2].kind = kind::SPELLCASTER;
    cards[2].effects[2] = true;
    cards[2].entries[2] = Some([0xB40, 0]);
    for (id, class, word, parameter) in [
        (5, class::FAR_TRAP, 0x2CBD, 0),
        (6, class::FAR_TRAP, 0x30BD, 0),
        (7, class::NEAR_TRAP, 0, 0x20000),
        (8, class::MAGIC, 0x1FB3, 0),
        (9, class::MAGIC, 0x17E1, 255),
        (10, class::MAGIC, 0x20DF, 0),
        (11, class::MAGIC, 0x12C0, 350),
        (12, class::MAGIC, 0x17C0, 255),
        (13, class::MAGIC, 0x1FA1, 0),
        (760, class::POWER_UP, 0, 0),
        (699, class::MAGIC, 0x4DF, 0),
    ] {
        cards[id].kind = class << 5;
        cards[id].entries[0] = Some([word, parameter]);
    }
    cards[760].power_up = Some(PowerUp {
        category: 0,
        value: 350,
    });
    cards[1].equip_bits[1] = 1;
    cards[15].effects[3] = true;
    cards[15].entries[3] = Some([0x23F4, 1]);
    cards
}

fn board(cards: &impl CardData) -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0, 0],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        cards,
    )
    .unwrap()
    .0
}

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn put(d: &mut Duel, c: &impl CardData, card: u16, side: u8, at: GridPos) -> UnitId {
    d.create(side, card, at, c, &mut vec![]).unwrap()
}

fn source(d: &mut Duel, c: &impl CardData, side: u8) -> UnitId {
    let id = put(d, c, 2, side, p(6, 6));
    let u = d.units.iter_mut().find(|u| u.id == id).unwrap();
    u.face_up = true;
    u.committed_face_up = true;
    u.defence = true;
    u.committed_defence = true;
    id
}

fn command(d: &Duel, c: &impl CardData, id: UnitId, path: Vec<GridPos>, reveal: bool) -> Duel {
    d.apply(
        0,
        &DuelCommand::Move {
            unit: id,
            path,
            face_up: reveal,
            defence: None,
        },
        c,
    )
    .unwrap()
    .0
}

#[test]
fn dragon_protection_requires_committed_defence_but_ignores_mode_and_current_face() {
    let mut c = catalogue();
    let mut d = board(&c);
    let t = put(&mut d, &c, 1, 0, p(0, 0));
    let s = source(&mut d, &c, 1);
    for mode in 0..4 {
        c[2].entries[2] = Some([0xB00 | mode << 6, 0x9876]);
        assert!(d.dragon_immunity(d.unit(t).unwrap(), &c));
    }
    let u = d.units.iter_mut().find(|u| u.id == s).unwrap();
    u.face_up = false;
    u.acted = true;
    u.spellbind = 255;
    u.stolen = true;
    assert!(d.dragon_immunity(d.unit(t).unwrap(), &c));
    for filter in [1, 31, 63] {
        c[2].entries[2] = Some([0xB00 | filter, 0]);
        assert!(!d.dragon_immunity(d.unit(t).unwrap(), &c));
    }
    c[2].entries[2] = Some([0xB40, 0]);
    d.units
        .iter_mut()
        .find(|u| u.id == s)
        .unwrap()
        .committed_defence = false;
    assert!(!d.dragon_immunity(d.unit(t).unwrap(), &c));
}

#[test]
fn protected_dragon_moves_through_block_attack_and_near_trap_without_spending_them() {
    for trap_card in [5, 7] {
        let c = catalogue();
        let mut d = board(&c);
        source(&mut d, &c, 1);
        let t = put(&mut d, &c, 1, 0, p(2, 3));
        let trap = put(&mut d, &c, trap_card, 1, p(3, 2));
        let d = command(&d, &c, t, vec![p(3, 3)], false);
        assert_eq!(d.unit(t).unwrap().position, p(3, 3));
        assert_eq!(d.unit(t).unwrap().spellbind, 0);
        assert!(d.unit(trap).is_some());
        assert!(!d.unit(trap).unwrap().committed_face_up);
    }
}

#[test]
fn protected_dragon_battles_past_negate_attack() {
    let c = catalogue();
    let mut d = board(&c);
    source(&mut d, &c, 1);
    let a = put(&mut d, &c, 1, 0, p(2, 3));
    let b = put(&mut d, &c, 3, 1, p(3, 3));
    let trap = put(&mut d, &c, 6, 1, p(5, 0));
    let d = command(&d, &c, a, vec![p(3, 3)], false);
    assert!(d.unit(b).is_none());
    assert_eq!(d.unit(a).unwrap().position, p(3, 3));
    assert!(d.unit(trap).is_some());
    assert_eq!(d.sides[1].lp, START_LP - 300);
}

#[test]
fn unprotected_dragon_still_triggers_block_attack() {
    let c = catalogue();
    let mut d = board(&c);
    let t = put(&mut d, &c, 1, 0, p(2, 3));
    let trap = put(&mut d, &c, 5, 1, p(3, 2));
    let d = command(&d, &c, t, vec![p(3, 3)], false);
    assert_eq!(d.unit(t).unwrap().position, p(2, 3));
    assert!(d.unit(t).unwrap().acted);
    assert!(d.unit(trap).is_none());
}

#[test]
fn key_mace_position_flip_bypasses_dragon_protection() {
    let c = catalogue();
    let mut d = board(&c);
    source(&mut d, &c, 1);
    let dragon = put(&mut d, &c, 1, 1, p(4, 3));
    let flipper = put(&mut d, &c, 15, 0, p(1, 3));
    let d = command(&d, &c, flipper, vec![], true);
    let u = d.unit(dragon).unwrap();
    // 0x265AE4 builder and 0x25B980 mode5 have no immunity filter.
    assert!(u.defence && u.committed_defence);
}

#[test]
fn enemy_power_up_cannot_equip_a_protected_dragon() {
    let c = catalogue();
    for protected in [false, true] {
        let mut d = board(&c);
        if protected {
            source(&mut d, &c, 1);
        }
        let target = put(&mut d, &c, 1, 1, p(3, 3));
        let equip = put(&mut d, &c, 760, 0, p(2, 3));
        let d = command(&d, &c, equip, vec![p(3, 3)], false);
        assert!(d.unit(equip).is_none());
        // [emu] Actual battle caller 0x24BCBC passes the defender first.
        // A moving power-up gives the stationary monster no experience.
        assert!(!d.experience[1].contains_key(&1));
        assert_eq!(
            d.unit(target).unwrap().attack_bonus,
            if protected { 0 } else { 350 }
        );
    }
}

#[test]
fn magic_boost_and_binding_exclude_only_enemy_controlled_dragons() {
    for spell in [11, 12] {
        let c = catalogue();
        let mut d = board(&c);
        source(&mut d, &c, 1);
        let own = put(&mut d, &c, 1, 0, p(1, 3));
        let enemy = put(&mut d, &c, 1, 1, p(4, 3));
        let stolen = put(&mut d, &c, 1, 1, p(4, 4));
        d.units.iter_mut().find(|u| u.id == stolen).unwrap().stolen = true;
        let m = put(&mut d, &c, spell, 0, p(2, 5));
        let d = command(&d, &c, m, vec![], true);
        assert!(d.unit(m).is_none());
        assert_eq!(d.unit(enemy).unwrap().attack_bonus, 0);
        assert_eq!(d.unit(enemy).unwrap().spellbind, 0);
        for id in [own, stolen] {
            if spell == 11 {
                assert_eq!(d.unit(id).unwrap().attack_bonus, 350);
            } else {
                assert_eq!(d.unit(id).unwrap().spellbind, 255);
            }
        }
    }
}

#[test]
fn strongest_selection_does_not_retarget_after_immunity_removes_its_winner() {
    for spell in [9, 13] {
        let mut c = catalogue();
        c[2].attack = 200;
        let mut d = board(&c);
        source(&mut d, &c, 1);
        let dragon = put(&mut d, &c, 1, 1, p(4, 3));
        let other = put(&mut d, &c, 3, 1, p(4, 4));
        let m = put(&mut d, &c, spell, 0, p(2, 5));
        let d = command(&d, &c, m, vec![], true);
        assert!(d.unit(m).is_none());
        for id in [dragon, other] {
            assert_eq!(d.unit(id).unwrap().spellbind, 0);
            assert_eq!(d.unit(id).unwrap().controller(), 1);
        }
    }
}

#[test]
fn change_of_heart_keeps_its_group_list_but_protected_selection_changes_nothing() {
    let c = catalogue();
    let mut d = board(&c);
    source(&mut d, &c, 1);
    let dragon = put(&mut d, &c, 1, 1, p(4, 3));
    let m = put(&mut d, &c, 8, 0, p(2, 5));
    let d = command(&d, &c, m, vec![], true);
    let Some(Pending::ControlChange { ref units, .. }) = d.pending else {
        panic!("the unprotected source leaves a nonempty candidate mask");
    };
    assert!(units.contains(&dragon));
    let (d, _) = d
        .apply(
            0,
            &DuelCommand::Pick {
                pick: Pick::Unit { unit: dragon },
            },
            &c,
        )
        .unwrap();
    assert_eq!(d.unit(dragon).unwrap().controller(), 1);
    assert!(d.pending.is_none());
}

#[test]
fn dark_hole_freezes_protection_before_removing_its_source() {
    let c = catalogue();
    let mut d = board(&c);
    let s = source(&mut d, &c, 0);
    let own = put(&mut d, &c, 1, 0, p(1, 3));
    let enemy = put(&mut d, &c, 1, 1, p(4, 3));
    let m = put(&mut d, &c, 699, 0, p(2, 5));
    let d = command(&d, &c, m, vec![], true);
    assert!(d.unit(s).is_none());
    assert!(d.unit(own).is_none());
    assert!(d.unit(enemy).is_some());
    assert!(!d.dragon_immunity(d.unit(enemy).unwrap(), &c));
}

#[test]
fn neutralizing_field_leaves_enemy_protected_dragon_modifiers_and_bind_intact() {
    let c = catalogue();
    let mut d = board(&c);
    source(&mut d, &c, 0);
    let own = put(&mut d, &c, 1, 0, p(1, 3));
    let enemy = put(&mut d, &c, 1, 1, p(4, 3));
    for u in d.units.iter_mut().filter(|u| u.id == own || u.id == enemy) {
        u.attack_bonus = 250;
        u.defense_bonus = -100;
        u.spellbind = 3;
    }
    let m = put(&mut d, &c, 10, 0, p(2, 5));
    let d = command(&d, &c, m, vec![], true);
    let o = d.unit(own).unwrap();
    let e = d.unit(enemy).unwrap();
    assert_eq!((o.attack_bonus, o.defense_bonus, o.spellbind), (0, 0, 0));
    assert_eq!(
        (e.attack_bonus, e.defense_bonus, e.spellbind),
        (250, -100, 3)
    );
}

#[derive(serde::Deserialize)]
struct OracleCase {
    name: String,
    units: Vec<DuelUnit>,
    expected: Vec<u8>,
    masks: [u32; 2],
}

#[derive(serde::Deserialize)]
struct Evidence {
    cards: Vec<CardInfo>,
    cases: Vec<OracleCase>,
}

struct PrivateCards(Vec<CardInfo>);
impl CardData for PrivateCards {
    fn card(&self, id: u16) -> Option<&CardInfo> {
        self.0.iter().find(|c| c.id == id)
    }
}

#[test]
fn private_dragon_immunity_matches_original_instructions() {
    let Some(path) = std::env::var_os("ROSES_DRAGON_IMMUNITY_ORACLE") else {
        return;
    };
    let raw = std::fs::read_to_string(path).expect("configured Dragon oracle must exist");
    let evidence: Evidence = serde_json::from_str(&raw).expect("valid Dragon oracle evidence");
    assert!(
        !evidence.cases.is_empty(),
        "configured oracle must contain cases"
    );
    let cards = PrivateCards(evidence.cards);
    let mut d = Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [392, 0],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        &cards,
    )
    .unwrap()
    .0;
    let mut count = 0;
    for c in &evidence.cases {
        d.units = c.units.clone();
        assert_eq!(d.units.len(), c.expected.len(), "{}", c.name);
        for (u, &expected) in d.units.iter().zip(&c.expected) {
            assert_eq!(
                u8::from(d.dragon_immunity(u, &cards)),
                expected,
                "{} unit {:?}",
                c.name,
                u.id
            );
            count += 1;
        }
        for by in 0..2 {
            let actual = d
                .units
                .iter()
                .filter(|u| d.magic_target_admitted(u, by, &cards))
                .fold(0u32, |m, u| m | (1 << (21 - u.side * 11 - u.slot)));
            assert_eq!(
                actual,
                c.masks[usize::from(by)],
                "{} filter by {}",
                c.name,
                by
            );
        }
    }
    eprintln!(
        "Dragon immunity: {count} predicates + {} masks match original",
        evidence.cases.len() * 2
    );
}
