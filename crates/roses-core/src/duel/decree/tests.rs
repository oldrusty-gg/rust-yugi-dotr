use crate::duel::*;

fn cards() -> Vec<CardInfo> {
    let mut c: Vec<CardInfo> = (0..16)
        .map(|id| CardInfo {
            id,
            kind: kind::WARRIOR,
            level: 3,
            attribute: 0,
            attack: 1200,
            defense: 700,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        })
        .collect();
    for (id, ty) in [
        (2, 0x2E),
        (3, 0x2C),
        (4, 0x4A),
        (5, 0x23),
        (6, 0x30),
        (7, 4),
        (8, 0x31),
    ] {
        c[id].kind = class::FAR_TRAP << 5;
        c[id].entries[0] = Some([ty << 8 | 0xBD, 0]);
    }
    c[9].kind = class::NEAR_TRAP << 5;
    c[9].kind_index = 11; // Its ordinary handler is intentionally unported.
    c[9].entries[0] = Some([0, 0]);
    c[10].entries[2] = Some([0xF40, 0]);
    c[10].effects[2] = true;
    c[11].kind = class::NEAR_TRAP << 5;
    c[11].entries[0] = Some([0, 2 << 16]);
    c
}

fn board(c: &impl CardData) -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [0, 0],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        c,
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

#[test]
fn decree_keeps_confirm_traps_and_the_requested_move() {
    let c = cards();
    for trap_card in [3, 4, 5] {
        for side in [0, 1] {
            let mut d = board(&c);
            let mover = put(&mut d, &c, 1, 0, p(2, 3));
            let trap = put(&mut d, &c, trap_card, 1, p(6, 0));
            let decree = put(&mut d, &c, 2, side, p(6, 6));
            let before = d.unit(trap).unwrap().clone();
            let (after, events) = d
                .apply(
                    0,
                    &DuelCommand::Move {
                        unit: mover,
                        path: if trap_card == 3 {
                            vec![p(3, 3)]
                        } else {
                            vec![]
                        },
                        face_up: false,
                        defence: if trap_card == 3 { None } else { Some(true) },
                    },
                    &c,
                )
                .unwrap();
            assert_eq!(after.unit(trap), Some(&before));
            assert!(after.unit(decree).unwrap().acted);
            assert_eq!(after.unit(mover).unwrap().defence, trap_card != 3);
            assert_eq!(
                after.unit(mover).unwrap().position,
                if trap_card == 3 { p(3, 3) } else { p(2, 3) }
            );
            assert!(!events
                .iter()
                .any(|e| matches!(e, DuelEvent::Cancelled { .. })));
        }
    }
}

#[test]
fn decree_cancels_the_entire_battle_trap_priority_once() {
    let c = cards();
    let mut d = board(&c);
    let att = put(&mut d, &c, 1, 0, p(2, 3));
    let def = put(&mut d, &c, 1, 1, p(3, 3));
    let traps: Vec<_> = [6, 7, 8]
        .iter()
        .enumerate()
        .map(|(i, &card)| put(&mut d, &c, card, 1, p(i as u8, 0)))
        .collect();
    let decree = put(&mut d, &c, 2, 0, p(6, 6));
    let before: Vec<_> = traps
        .iter()
        .map(|&id| d.unit(id).unwrap().clone())
        .collect();
    let mut events = vec![];
    d.battle(att, def, p(3, 3), &c, &mut events).unwrap();
    for trap in before {
        assert_eq!(d.unit(trap.id), Some(&trap));
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e,DuelEvent::Activated{unit,..} if *unit==decree))
            .count(),
        1
    );
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Cancelled { .. })));
}

#[test]
fn decree_precedes_near_trap_applicability_and_uses_controllers() {
    let c = cards();
    for stolen_mover in [false, true] {
        for stolen_trap in [false, true] {
            let mut d = board(&c);
            let mover = put(&mut d, &c, 1, 0, p(2, 3));
            let trap = put(&mut d, &c, 9, 1, p(3, 2));
            let decree = put(&mut d, &c, 2, 0, p(6, 6));
            d.units.iter_mut().find(|u| u.id == mover).unwrap().stolen = stolen_mover;
            d.units.iter_mut().find(|u| u.id == trap).unwrap().stolen = stolen_trap;
            let before = d.unit(trap).unwrap().clone();
            let mut events = vec![];
            assert!(!d.near_trap(mover, p(3, 3), &c, &mut events).unwrap());
            assert_eq!(d.unit(trap), Some(&before));
            assert_eq!(d.unit(decree).unwrap().acted, stolen_mover == stolen_trap);
        }
    }
}

#[test]
fn decree_forces_both_face_bits_and_keeps_first_group_on_equal_age() {
    let c = cards();
    for first in [0, 1] {
        let mut d = board(&c);
        let ids = [
            put(&mut d, &c, 2, 0, p(6, 6)),
            put(&mut d, &c, 2, 1, p(6, 0)),
        ];
        for u in d.units.iter_mut().filter(|u| ids.contains(&u.id)) {
            u.summon_order = 7;
            u.face_up = false;
            u.committed_face_up = true;
        }
        let mut events = vec![];
        assert!(d.royal_decree_reacts(first, &c, &mut events));
        for g in [0, 1] {
            let u = d.unit(ids[g]).unwrap();
            assert_eq!(u.acted, g == usize::from(first));
            assert_eq!(u.face_up, g == usize::from(first));
        }
    }
}

#[test]
fn active_tree_suppresses_near_traps_even_without_decree() {
    let c = cards();
    let mut d = board(&c);
    let mover = put(&mut d, &c, 1, 0, p(2, 3));
    let trap = put(&mut d, &c, 9, 1, p(3, 2));
    let tree = put(&mut d, &c, 10, 0, p(6, 6));
    let u = d.units.iter_mut().find(|u| u.id == tree).unwrap();
    u.committed_face_up = true;
    u.defence = true;
    u.committed_defence = true;
    let original = d.unit(trap).unwrap().clone();
    let mut events = vec![];
    assert!(!d.near_trap(mover, p(3, 3), &c, &mut events).unwrap());
    assert_eq!(d.unit(trap), Some(&original));
    assert!(events.is_empty());
}

#[test]
fn near_trap_order_uses_descending_age_and_original_spatial_ties() {
    let c = cards();
    for equal in [false, true] {
        let mut d = board(&c);
        let mover = put(&mut d, &c, 1, 0, p(1, 3));
        // Insert in an order distinct from the original column-major scan.
        let right = put(&mut d, &c, 11, 1, p(3, 3));
        let above = put(&mut d, &c, 11, 1, p(2, 2));
        let below = put(&mut d, &c, 11, 1, p(2, 4));
        for u in d
            .units
            .iter_mut()
            .filter(|u| [right, above, below].contains(&u.id))
        {
            u.summon_order = if equal {
                7
            } else if u.id == right {
                20
            } else {
                0
            };
        }
        let mut events = vec![];
        assert!(d.near_trap(mover, p(2, 3), &c, &mut events).unwrap());
        let expected = if equal { below } else { right };
        assert!(d.unit(expected).is_none());
        assert!(events
            .iter()
            .any(|e| matches!(e, DuelEvent::Activated {unit,..} if *unit==expected)));
    }
}

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    hook: String,
    side: u8,
    defence: bool,
    cell: GridPos,
    units: Vec<DuelUnit>,
    selected: Option<u16>,
    flags: Vec<u8>,
}
#[derive(serde::Deserialize)]
struct Evidence {
    cards: Vec<CardInfo>,
    cases: Vec<Case>,
}
struct PrivateCards(Vec<CardInfo>);
impl CardData for PrivateCards {
    fn card(&self, id: u16) -> Option<&CardInfo> {
        self.0.iter().find(|c| c.id == id)
    }
}
fn flags(u: &DuelUnit) -> u8 {
    u8::from(u.acted)
        | u8::from(u.stolen) << 1
        | u8::from(u.face_up) << 2
        | u8::from(u.defence) << 3
        | u8::from(u.committed_face_up) << 4
        | u8::from(u.committed_defence) << 5
}

#[test]
fn private_decree_hooks_match_original_builder_and_flag_setters() {
    let Some(path) = std::env::var_os("ROSES_DECREE_ORACLE") else {
        return;
    };
    let raw = std::fs::read_to_string(path).expect("configured Decree oracle must exist");
    let evidence: Evidence = serde_json::from_str(&raw).expect("valid Decree oracle");
    let cards = PrivateCards(evidence.cards);
    let mut count = 0;
    for case in evidence.cases.iter().filter(|c| c.selected.is_some()) {
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
        d.units = case.units.clone();
        let index = d
            .units
            .iter()
            .position(|u| u.side == case.side && u.slot == 1)
            .unwrap();
        let mover = d.units[index].id;
        let mut events = vec![];
        match case.hook.as_str() {
            "confirm" => assert!(
                !d.confirm_trap(index, case.defence, &cards, &mut events)
                    .unwrap(),
                "{}",
                case.name
            ),
            "near" => assert!(
                !d.near_trap(mover, case.cell, &cards, &mut events).unwrap(),
                "{}",
                case.name
            ),
            "battle" => {
                let def = d.unit_at(case.cell).expect("defender").id;
                d.battle(mover, def, case.cell, &cards, &mut events)
                    .unwrap();
            }
            _ => panic!("unknown original hook"),
        }
        let packed = case.selected.unwrap();
        let chosen = case
            .units
            .iter()
            .find(|u| u.side == ((packed >> 8) as u8) && u.slot == ((packed & 255) as u8))
            .unwrap();
        let reactions: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                DuelEvent::Activated { unit, card, .. } if *card == chosen.card => Some(*unit),
                _ => None,
            })
            .collect();
        assert_eq!(reactions, vec![chosen.id], "{}", case.name);
        for (u, &expected) in case.units.iter().zip(&case.flags) {
            // Full battle resolution follows the compared original builder;
            // compare only the trap reaction and its untouched trap peers.
            if cards.card(u.card).unwrap().class() == class::FAR_TRAP {
                assert_eq!(
                    flags(d.unit(u.id).expect("trap preserved")),
                    expected,
                    "{} {:?}",
                    case.name,
                    u.id
                );
            }
        }
        count += 1;
    }
    assert!(count > 0, "configured oracle needs Decree reactions");
    eprintln!("Decree: {count} original builder/reaction comparisons matched");
}

#[derive(serde::Deserialize)]
struct OrderCase {
    units: Vec<DuelUnit>,
    ordered: Vec<u16>,
}

#[test]
fn private_near_trap_sort_matches_original_instructions() {
    let Some(path) = std::env::var_os("ROSES_DECREE_ORACLE") else {
        return;
    };
    let path = std::path::Path::new(&path).with_file_name("near-order.json");
    let raw = std::fs::read_to_string(path).expect("configured near order evidence");
    let cases: Vec<OrderCase> = serde_json::from_str(&raw).expect("valid near order evidence");
    assert!(!cases.is_empty());
    let count = cases.len();
    for mut case in cases {
        Duel::order_near_traps(&mut case.units);
        let actual: Vec<_> = case
            .units
            .iter()
            .map(|u| u16::from(u.side) << 8 | u16::from(u.slot))
            .collect();
        assert_eq!(actual, case.ordered);
    }
    eprintln!("Near traps: {count} original sort comparisons matched");
}

#[derive(serde::Deserialize)]
struct TreeCase {
    units: Vec<DuelUnit>,
    suppressed: bool,
}
#[derive(serde::Deserialize)]
struct TreeEvidence {
    cards: Vec<CardInfo>,
    cases: Vec<TreeCase>,
}

#[test]
fn private_tree_near_trap_suppression_matches_original_instructions() {
    let Some(path) = std::env::var_os("ROSES_DECREE_ORACLE") else {
        return;
    };
    let path = std::path::Path::new(&path).with_file_name("tree.json");
    let raw = std::fs::read_to_string(path).expect("configured Tree evidence");
    let evidence: TreeEvidence = serde_json::from_str(&raw).expect("valid Tree evidence");
    let cards = PrivateCards(evidence.cards);
    assert!(!evidence.cases.is_empty());
    for case in &evidence.cases {
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
        d.units = case.units.clone();
        assert_eq!(d.traps_suppressed(&cards), case.suppressed);
        let mover = d
            .units
            .iter()
            .find(|u| u.side == 0 && u.slot == 1)
            .unwrap()
            .id;
        assert_eq!(
            d.near_trap(mover, p(3, 3), &cards, &mut vec![]).unwrap(),
            !case.suppressed
        );
    }
    eprintln!(
        "Ancient Tree: {} original predicate/builder comparisons matched",
        evidence.cases.len()
    );
}
