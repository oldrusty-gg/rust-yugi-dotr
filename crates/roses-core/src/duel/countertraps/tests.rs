use super::*;

const PLAIN: u16 = 0;
const WHITE: u16 = 1;
const ANTI: u16 = 2;
const DECREE: u16 = 3;
const TREE: u16 = 4;

fn catalogue() -> Vec<CardInfo> {
    let plain = CardInfo {
        id: PLAIN,
        kind: 0,
        kind_index: 0,
        level: 2,
        attribute: 0,
        attack: 100,
        defense: 100,
        effects: [false; 5],
        toon: false,
        entries: [None; 5],
        equip_bits: [0; 7],
        power_up: None,
    };
    let mut v = vec![plain; 701];
    for (id, c) in v.iter_mut().enumerate() {
        c.id = id as u16;
    }
    for (id, ty) in [(WHITE, 0x2D), (ANTI, 0x50), (DECREE, 0x2E)] {
        v[usize::from(id)].kind = class::FAR_TRAP << 5;
        v[usize::from(id)].entries[0] = Some([ty << 8, 0]);
    }
    v[usize::from(TREE)].entries[2] = Some([0xF00, 0]);
    v[usize::from(TREE)].effects[2] = true;
    // The executable dispatches these magic behaviors by id, but these
    // records use invented stats and only the required instruction fields.
    v[699].kind = class::MAGIC << 5;
    v[699].entries[0] = Some([0x4DF, 0]);
    v[700].kind = class::MAGIC << 5;
    v[700].entries[0] = Some([0x4DD, 0x12_0000]);
    v
}

fn board() -> Duel {
    Duel::new(
        DuelSetup {
            terrain: vec![terrain::NORMAL; 49],
            leaders: [PLAIN; 2],
            decks: [vec![], vec![]],
            first: 0,
            leader_abilities: [vec![], vec![]],
            controllers: [PAD_CONTROLLER; 2],
        },
        &catalogue(),
    )
    .unwrap()
    .0
}

fn add(d: &mut Duel, card: u16, side: u8, slot: u8, flags: u8, order: u8) -> UnitId {
    let id = UnitId(100 + u16::from(side) * 11 + u16::from(slot));
    d.units.push(DuelUnit {
        id,
        card,
        side,
        slot,
        position: GridPos {
            column: slot % 7,
            row: 1 + side * 3 + slot / 7,
        },
        face_up: flags & 4 != 0,
        defence: flags & 8 != 0,
        committed_face_up: flags & 16 != 0,
        committed_defence: flags & 32 != 0,
        acted: flags & 1 != 0,
        stolen: flags & 2 != 0,
        summon_order: order,
        spellbind: 0,
        spellbind_fresh: false,
        attack_bonus: 0,
        defense_bonus: 0,
        evo_counter: 0,
        discount: 0,
        stolen_by: None,
    });
    d.next_unit = 200;
    id
}

fn reveal(d: &Duel, card: UnitId) -> Result<(Duel, Vec<DuelEvent>)> {
    d.apply(
        0,
        &DuelCommand::Move {
            unit: card,
            path: vec![],
            face_up: true,
            defence: None,
        },
        &catalogue(),
    )
}

#[test]
fn white_hole_command_protects_the_oldest_traps_group_and_preserves_it() {
    let mut d = board();
    let own = add(&mut d, PLAIN, 0, 1, 0, 4);
    let enemy = add(&mut d, PLAIN, 1, 1, 0, 4);
    let younger = add(&mut d, WHITE, 0, 6, 0, 7);
    let older = add(&mut d, WHITE, 1, 6, 0, 3);
    let magic = add(&mut d, 699, 0, 7, 0, 9);
    let (after, _) = reveal(&d, magic).unwrap();
    assert!(after.unit(own).is_none());
    assert!(after.unit(younger).is_none());
    assert!(after.unit(enemy).is_some());
    let trap = after.unit(older).unwrap();
    assert!(trap.face_up && trap.committed_face_up && trap.acted);
    assert_eq!(after.sides[0].graveyard, vec![699, PLAIN, WHITE]);
    assert!(after.sides[1].graveyard.is_empty());
}

#[test]
fn white_hole_equal_age_protects_both_groups_but_only_second_is_marked_acted() {
    let mut d = board();
    let first = add(&mut d, WHITE, 0, 6, 0, 4);
    let second = add(&mut d, WHITE, 1, 6, 0, 4);
    let magic = add(&mut d, 699, 0, 7, 0, 9);
    let (after, _) = reveal(&d, magic).unwrap();
    assert!(after.unit(first).unwrap().committed_face_up);
    assert!(!after.unit(first).unwrap().acted);
    assert!(after.unit(second).unwrap().acted);
    assert_eq!(after.sides[0].graveyard, vec![699]);
    assert!(after.sides[1].graveyard.is_empty());
}

#[test]
fn white_hole_stolen_controller_protects_other_group_and_can_destroy_itself() {
    let mut d = board();
    let own = add(&mut d, PLAIN, 0, 1, 0, 1);
    let enemy = add(&mut d, PLAIN, 1, 1, 0, 1);
    let trap = add(&mut d, WHITE, 1, 6, 2, 2);
    let magic = add(&mut d, 699, 0, 6, 0, 9);
    let (after, _) = reveal(&d, magic).unwrap();
    assert!(after.unit(own).is_some());
    assert!(after.unit(enemy).is_none());
    assert!(after.unit(trap).is_none());
    assert_eq!(after.sides[1].graveyard, vec![PLAIN, WHITE]);
}

#[test]
fn white_hole_royal_decree_reveals_before_both_are_wiped() {
    let mut d = board();
    let trap = add(&mut d, WHITE, 1, 6, 0, 2);
    let decree = add(&mut d, DECREE, 0, 6, 0, 3);
    let magic = add(&mut d, 699, 0, 7, 0, 9);
    let (after, events) = reveal(&d, magic).unwrap();
    assert!(after.unit(trap).is_none());
    assert!(after.unit(decree).is_none());
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::Activated { unit, .. } if *unit == decree)));
    assert!(!events
        .iter()
        .any(|e| matches!(e, DuelEvent::Activated { unit, .. } if *unit == trap)));
}

#[test]
fn anti_raigeki_passive_field_presence_allows_unrelated_commands() {
    for side in 0..2 {
        let mut d = board();
        let own = add(&mut d, PLAIN, 0, 1, 0, 1);
        let enemy = add(&mut d, PLAIN, 1, 1, 0, 1);
        let anti = add(&mut d, ANTI, side, 6, 0, 3);
        let (after, _) = d.apply(0, &DuelCommand::EndTurn, &catalogue()).unwrap();
        assert!(after.unit(own).is_some() && after.unit(enemy).is_some());
        let trap = after.unit(anti).unwrap();
        assert!(!trap.face_up && !trap.committed_face_up);
        assert!(after.sides[0].graveyard.is_empty());
        assert!(after.sides[1].graveyard.is_empty());
        assert_eq!([after.sides[0].lp, after.sides[1].lp], [START_LP; 2]);
    }
}

#[test]
fn raigeki_paths_follow_event_5() {
    // Raigeki is no longer refused (E2-536-TRAP-0x4.md §3, emu RG3-RG6):
    // a set Anti-Raigeki reacts and stays; a Royal Decree (either group,
    // the activator's first) lets Raigeki through. The card itself always
    // ends in the graveyard. A face-up Ancient Tree is still refused as a
    // continuous effect... no longer: since 5 Oct (WHITE-REFUSALS §20) the
    // tree's 0x268710(0xF) suppresses Anti-Raigeki's search and Raigeki
    // goes through, Anti-Raigeki unused.
    for extra in [None, Some(ANTI), Some(DECREE), Some(TREE)] {
        let mut d = board();
        let mut anti = None;
        let mut other = None;
        if let Some(card) = extra {
            anti = Some(add(&mut d, ANTI, 1, 6, 0, 3));
            if card != ANTI {
                let slot = if card == TREE { 1 } else { 7 };
                other = Some(add(&mut d, card, 1, slot, 0x3c, 4));
            }
        }
        let magic = add(&mut d, 700, 0, 6, 0, 9);
        let (after, _) = reveal(&d, magic).unwrap_or_else(|e| panic!("{extra:?}: {e}"));
        assert!(after.unit(magic).is_none());
        assert_eq!(after.sides[0].graveyard, vec![700]);
        if let Some(anti) = anti {
            let a = after.unit(anti).expect("Anti-Raigeki stays");
            assert_eq!(a.acted, extra == Some(ANTI));
        }
        if extra == Some(DECREE) {
            assert!(after.unit(other.unwrap()).unwrap().acted);
        }
    }
}

#[test]
fn countertrap_order_ff_never_wins() {
    let mut d = board();
    add(&mut d, ANTI, 1, 6, 0, 255);
    assert_eq!(d.far_trap_in(&[1, 0], 0x50, &catalogue()), None);
    let eligible = add(&mut d, ANTI, 0, 6, 0, 254);
    assert_eq!(d.far_trap_in(&[1, 0], 0x50, &catalogue()), Some(eligible));
}

#[test]
fn private_white_hole_original_handler_oracle() {
    #[derive(Deserialize)]
    struct Input {
        side: u8,
        slot: u8,
        card: u16,
        flags: u8,
        order: u8,
        kind: u8,
        entries: [Option<[u32; 2]>; 5],
    }
    #[derive(Deserialize)]
    struct Output {
        side: u8,
        slot: u8,
        flags: u8,
    }
    #[derive(Deserialize)]
    struct Case {
        label: String,
        controller: u8,
        units: Vec<Input>,
        mask: u32,
        expected_mask: u32,
        after: Vec<Output>,
    }
    let Ok(dir) = std::env::var("ROSES_SETO_COUNTERTRAP_ORACLE") else {
        eprintln!("private White Hole oracle requires ROSES_SETO_COUNTERTRAP_ORACLE");
        return;
    };
    let path = std::path::Path::new(&dir).join("oracle.json");
    let cases: Vec<Case> = serde_json::from_slice(
        &std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .expect("private oracle must be valid");
    assert_eq!(cases.len(), 32, "incomplete private oracle");
    for case in &cases {
        let mut cards = catalogue();
        cards.resize(854, cards[0].clone());
        let mut d = board();
        for input in &case.units {
            let card = &mut cards[usize::from(input.card)];
            card.id = input.card;
            card.kind = input.kind;
            card.entries = input.entries;
            add(
                &mut d,
                input.card,
                input.side,
                input.slot,
                input.flags,
                input.order,
            );
        }
        let bit = |u: &DuelUnit| 1u32 << (21 - (u.side * 11 + u.slot));
        let mut targets: Vec<_> = d
            .units
            .iter()
            .filter(|u| case.mask & bit(u) != 0)
            .map(|u| u.id)
            .collect();
        d.dark_hole_countertraps(case.controller, &mut targets, &cards, &mut vec![]);
        let mask = targets
            .iter()
            .fold(0, |mask, id| mask | bit(d.unit(*id).unwrap()));
        assert_eq!(mask, case.expected_mask, "{} mask", case.label);
        for expected in &case.after {
            let u = d
                .units
                .iter()
                .find(|u| u.side == expected.side && u.slot == expected.slot)
                .unwrap();
            let flags = u8::from(u.acted)
                | u8::from(u.stolen) << 1
                | u8::from(u.face_up) << 2
                | u8::from(u.defence) << 3
                | u8::from(u.committed_face_up) << 4
                | u8::from(u.committed_defence) << 5;
            assert_eq!(
                flags, expected.flags,
                "{} side{} slot{}",
                case.label, u.side, u.slot
            );
        }
    }
    eprintln!(
        "White Hole original handler: {} matched / 0 differed",
        cases.len()
    );
}
