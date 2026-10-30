use super::*;

pub(super) fn monster(id: u16, kind: u8, level: u8, attribute: u8, attack: u16, defense: u16) -> CardInfo {
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

/// Synthetic catalogue (invented numbers, not retail data).
pub(super) fn cards() -> Vec<CardInfo> {
    let mut v: Vec<CardInfo> = (0..20)
        .map(|i| monster(i, kind::MACHINE, 1, 0, 100, 100))
        .collect();
    v[0] = monster(0, kind::DRAGON, 8, 0, 3000, 2500); // leader B
    v[1] = monster(1, kind::SPELLCASTER, 7, 1, 2500, 2100); // leader A
    v[2] = monster(2, kind::WARRIOR, 4, 3, 1400, 1200); // earth warrior
    v[3] = monster(3, kind::DRAGON, 3, 5, 1200, 700); // wind dragon
    v[4] = monster(4, kind::WARRIOR, 4, 3, 1400, 1200);
    v[5] = monster(5, kind::ROCK, 3, 3, 300, 1200);
    v[6] = monster(6, kind::SPELLCASTER, 4, 4, 1200, 2000);
    v[7] = monster(7, kind::DRAGON, 4, 1, 1500, 1200);
    v[8] = monster(8, kind::MACHINE, 9, 0, 100, 100);
    v[9] = monster(9, kind::MACHINE, 4, 0, 1800, 1000);
    v[10] = monster(10, kind::WARRIOR, 5, 0, 2500, 2000);
    let card = |id: u16, kind: u8, kind_index: u8, word0: u32, word1: u32| CardInfo {
        kind,
        kind_index,
        entries: [Some([word0, word1]), None, None, None, None],
        ..monster(id, 0, 0, 0, 0, 0)
    };
    // Entry words as in the PAL table (Block Attack, Negate Attack, Bear
    // Trap, Infinite Dismissal, Tears of the Mermaid).
    v[12] = card(12, 128, 0, 0x2CBD, 0);
    v[14] = card(14, 128, 1, 0x30BD, 0);
    v[15] = card(15, 96, 8, 1, 0);
    v[16] = card(16, 96, 4, 0, 0x30000);
    v[17] = card(17, 96, 3, 0, 0x10258);
    v[18] = card(18, 128, 2, 0xFEBD, 0); // synthetic untraced full-range type
                                         // Magic: Mountain-like field (radius 2, terrain 2), Remedy-like +1000 LP,
                                         // Trumpeter-like +600 to Machines on both sides (mode 3, filter 14).
    v[19] = card(19, 32, 0, 0x2FD, 0x20002);
    v.push(card(20, 32, 1, 0xD7D, 0x3E8));
    v.push(card(21, 32, 2, 0x12C0 | u32::from(kind::MACHINE), 600));
    // Power-ups: fits Warriors (bit 0), and a transforming one.
    let mut power_up = card(22, 64, 0, 0, 0);
    power_up.power_up = Some(PowerUp {
        category: 0,
        value: 500,
    });
    v.push(power_up);
    let mut cocoon = card(23, 64, 1, 0, 0);
    cocoon.power_up = Some(PowerUp {
        category: 4,
        value: 7,
    });
    v.push(cocoon);
    v.push(card(24, 160, 0, 0x325C, 0x1A6)); // ritual
    v[13] = CardInfo {
        kind: 64, // power-up
        ..monster(13, 0, 0, 0, 0, 0)
    };
    v
}

fn setup(decks: [Vec<u16>; 2]) -> DuelSetup {
    DuelSetup {
        terrain: vec![terrain::NORMAL; 49],
        leaders: [1, 0],
        decks,
        first: 0,
        leader_abilities: [vec![], vec![]],
        controllers: [PAD_CONTROLLER; 2],
    }
}

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn run(duel: &Duel, side: u8, cmd: DuelCommand) -> (Duel, Vec<DuelEvent>) {
    duel.apply(side, &cmd, &cards()).unwrap()
}

fn summon(duel: &Duel, side: u8, cell: GridPos, hand_index: usize) -> Duel {
    let (d, _) = run(duel, side, DuelCommand::OpenSummon { cell });
    run(
        &d,
        side,
        DuelCommand::ChooseSummon {
            hand: vec![hand_index],
        },
    )
    .0
}

fn step(duel: &Duel, side: u8, unit: UnitId, path: Vec<GridPos>) -> (Duel, Vec<DuelEvent>) {
    run(
        duel,
        side,
        DuelCommand::Move {
            unit,
            path,
            face_up: false,
            defence: None,
        },
    )
}

#[test]
fn opening_turn_counts_rounds_and_stars_like_the_tutorial() {
    let (d, events) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap();
    assert_eq!(
        (d.round, d.active, d.sides[0].stars, d.sides[0].lp),
        (1, 0, 4, 4000)
    );
    assert!(matches!(
        events[0],
        DuelEvent::TurnStarted {
            side: 0,
            round: 1,
            stars: 4
        }
    ));
    // Summon Celtic-like level 4 to (3,5): hand refilled to five, cost 4.
    let d = summon(&d, 0, p(3, 5), 0);
    assert_eq!(
        (
            d.sides[0].stars,
            d.sides[0].hand.len(),
            d.sides[0].deck.len()
        ),
        (0, 4, 35)
    );
    assert!(!d.sides[0].can_summon);
    let unit = d.unit_at(p(3, 5)).unwrap().clone();
    assert!(!unit.face_up && !unit.acted && unit.slot == 1);
    // Freshly summoned monsters may move this turn, once.
    let (d, _) = step(&d, 0, unit.id, vec![p(3, 4)]);
    assert!(d.unit(unit.id).unwrap().acted);
    assert!(d
        .apply(
            0,
            &DuelCommand::Move {
                unit: unit.id,
                path: vec![p(3, 3)],
                face_up: false,
                defence: None
            },
            &cards()
        )
        .is_err());
    // Second player's first turn: still round 1, no income (observed 4 -> 1 after a level 3).
    let (d, _) = run(&d, 0, DuelCommand::EndTurn);
    assert_eq!((d.active, d.round, d.sides[1].stars), (1, 1, 4));
    let d = summon(&d, 1, p(3, 1), 0);
    assert_eq!(d.sides[1].stars, 1);
    // Back to the first player: round 2, +3 stars (observed 0 -> 3).
    let (d, _) = run(&d, 1, DuelCommand::EndTurn);
    assert_eq!((d.active, d.round, d.sides[0].stars), (0, 2, 3));
    assert!(!d.unit(unit.id).unwrap().acted);
}

#[test]
fn summon_areas_are_the_leaders_eight_neighbours() {
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap();
    for cell in [p(2, 5), p(3, 5), p(4, 5), p(2, 6), p(4, 6)] {
        assert!(
            d.apply(0, &DuelCommand::OpenSummon { cell }, &cards())
                .is_ok(),
            "{cell:?}"
        );
    }
    for cell in [p(3, 6), p(3, 4), p(1, 5), p(5, 6)] {
        assert!(
            d.apply(0, &DuelCommand::OpenSummon { cell }, &cards())
                .is_err(),
            "{cell:?}"
        );
    }
    let mut walls = setup([vec![2; 40], vec![3; 40]]);
    walls.terrain[5 * 7 + 3] = terrain::LABYRINTH;
    let (d, _) = Duel::new(walls, &cards()).unwrap();
    assert!(d
        .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &cards())
        .is_err());
}

#[test]
fn attack_position_battles_follow_the_traced_table() {
    // Attacker 1400 (earth) vs defender 1200 (wind), attack position.
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap();
    let d = summon(&d, 0, p(3, 5), 0);
    let (d, _) = run(&d, 0, DuelCommand::EndTurn);
    let d = summon(&d, 1, p(3, 1), 0);
    let dragon = d.unit_at(p(3, 1)).unwrap().id;
    let (d, _) = step(&d, 1, dragon, vec![p(3, 2)]);
    let (d, _) = run(&d, 1, DuelCommand::EndTurn);
    let celtic = d.unit_at(p(3, 5)).unwrap().id;
    let (d, _) = step(&d, 0, celtic, vec![p(3, 4)]);
    let (d, _) = run(&d, 0, DuelCommand::EndTurn);
    let (d, _) = run(&d, 1, DuelCommand::EndTurn);
    let (d, _) = step(&d, 0, celtic, vec![p(3, 3)]);
    let (d, _) = run(&d, 0, DuelCommand::EndTurn);
    let (d, _) = run(&d, 1, DuelCommand::EndTurn);
    let (d, events) = step(&d, 0, celtic, vec![p(3, 2)]);
    assert!(events
        .iter()
        .any(|e| matches!(e, DuelEvent::Revealed { .. })));
    assert!(d.unit(dragon).is_none(), "defender destroyed");
    assert_eq!(d.sides[1].lp, 3800, "ATK difference to the defender's side");
    assert_eq!(
        d.unit(celtic).unwrap().position,
        p(3, 2),
        "attacker advances"
    );
}

fn board(units: Vec<DuelUnit>) -> Duel {
    let (mut d, _) = Duel::new(setup([vec![], vec![]]), &cards()).unwrap();
    d.units.extend(units);
    d.next_unit = 100;
    d
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

#[test]
fn defence_and_tie_outcomes_match_the_tutorial() {
    let attacker = |id| placed(id, 2, 0, 1, p(3, 4), false, true);
    // Hard-Armor-like 300/1200 in defence: destroyed, no damage.
    let d = board(vec![attacker(10), placed(11, 5, 1, 1, p(3, 3), true, true)]);
    let (d2, _) = step(&d, 0, UnitId(10), vec![p(3, 3)]);
    assert!(d2.unit(UnitId(11)).is_none());
    assert_eq!((d2.sides[0].lp, d2.sides[1].lp), (4000, 4000));
    assert_eq!(d2.unit(UnitId(10)).unwrap().position, p(3, 3));
    // Aqua-Madoor-like DEF 2000: attacker's side takes 600, nobody dies, no advance.
    let d = board(vec![attacker(10), placed(11, 6, 1, 1, p(3, 3), true, true)]);
    let (d2, _) = step(&d, 0, UnitId(10), vec![p(3, 3)]);
    assert_eq!(d2.sides[0].lp, 3400);
    assert!(d2.unit(UnitId(11)).is_some());
    assert_eq!(d2.unit(UnitId(10)).unwrap().position, p(3, 4));
    // Equal ATK 1400 vs 1400: both destroyed, no damage.
    let d = board(vec![
        attacker(10),
        placed(11, 4, 1, 1, p(3, 3), false, true),
    ]);
    let (d2, _) = step(&d, 0, UnitId(10), vec![p(3, 3)]);
    assert!(d2.unit(UnitId(10)).is_none() && d2.unit(UnitId(11)).is_none());
    assert_eq!(d2.sides[1].lp, 4000);
    // Stronger attack-position defender 1500: attacker destroyed, 100 damage.
    let d = board(vec![
        attacker(10),
        placed(11, 7, 1, 1, p(3, 3), false, true),
    ]);
    let (d2, _) = step(&d, 0, UnitId(10), vec![p(3, 3)]);
    assert!(d2.unit(UnitId(10)).is_none());
    assert_eq!(d2.sides[0].lp, 3900);
}

#[test]
fn direct_attack_deals_full_attack_and_does_not_advance() {
    let d = board(vec![placed(10, 2, 0, 1, p(3, 1), false, true)]);
    let (d2, _) = step(&d, 0, UnitId(10), vec![p(3, 0)]);
    assert_eq!(d2.sides[1].lp, 2600);
    assert_eq!(d2.unit(UnitId(10)).unwrap().position, p(3, 1));
    // Reduce to zero: the duel ends on life points.
    let mut low = d.clone();
    low.sides[1].lp = 1000;
    let (d3, events) = step(&low, 0, UnitId(10), vec![p(3, 0)]);
    assert_eq!(
        d3.outcome,
        Some(Outcome {
            winner: Some(0),
            reason: EndReason::LifePoints
        })
    );
    assert!(matches!(events.last(), Some(DuelEvent::Ended { .. })));
    assert!(d3.apply(1, &DuelCommand::EndTurn, &cards()).is_err());
}

#[test]
fn attribute_advantage_spellbinds_the_survivor_for_one_turn() {
    // Earth attacker 1400 vs wind defender in defence 2000 DEF: nobody dies,
    // the defender is spellbound (earth beats wind).
    let mut cards = cards();
    cards[3].defense = 2000;
    let (mut d, _) = Duel::new(setup([vec![], vec![]]), &cards).unwrap();
    d.units.push(placed(10, 2, 0, 1, p(3, 4), false, true));
    d.units.push(placed(11, 3, 1, 1, p(3, 3), true, true));
    let (d, events) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(10),
                path: vec![p(3, 3)],
                face_up: false,
                defence: None,
            },
            &cards,
        )
        .unwrap();
    assert!(events.contains(&DuelEvent::Spellbound { unit: UnitId(11) }));
    let (d, _) = d.apply(0, &DuelCommand::EndTurn, &cards).unwrap();
    assert!(
        d.unit(UnitId(11)).unwrap().acted,
        "spellbound on its owner's turn"
    );
    let (d, _) = d.apply(1, &DuelCommand::EndTurn, &cards).unwrap();
    assert_eq!(d.unit(UnitId(11)).unwrap().spellbind, 0);
    assert_eq!(attribute_relation(3, 5), 1);
    assert_eq!(attribute_relation(5, 3), -1);
    assert_eq!(attribute_relation(0, 4), 1);
    assert_eq!(attribute_relation(1, 0), 1);
    assert_eq!(attribute_relation(0, 3), 0);
}

#[test]
fn terrain_changes_stats_and_movement() {
    let mut s = setup([vec![], vec![]]);
    s.terrain = vec![terrain::MEADOW; 49];
    let (mut d, _) = Duel::new(s, &cards()).unwrap();
    d.units.push(placed(10, 2, 0, 1, p(3, 4), false, true));
    let paths = d.reachable(UnitId(10), &cards()).unwrap();
    assert!(
        paths.iter().any(|p| p.len() == 2),
        "favoured face-up warrior moves two"
    );
    d.units[2].face_up = false;
    d.units[2].committed_face_up = false;
    assert!(d
        .reachable(UnitId(10), &cards())
        .unwrap()
        .iter()
        .all(|p| p.len() == 1));
    let unit = d.units[2].clone();
    assert_eq!(d.effective(&unit, p(3, 4), true, &cards()), 1900);
    // Spellcaster on meadow is unfavoured.
    let spell = placed(11, 6, 0, 2, p(2, 4), false, true);
    assert_eq!(d.effective(&spell, p(2, 4), true, &cards()), 700);
    assert_eq!(terrain_affinity(terrain::NORMAL, &cards()[2]), 0);
}

#[test]
fn crush_destroys_strong_monsters_and_labyrinth_blocks() {
    let mut s = setup([vec![], vec![]]);
    s.terrain[3 * 7 + 3] = terrain::CRUSH;
    s.terrain[3 * 7 + 2] = terrain::LABYRINTH;
    let (mut d, _) = Duel::new(s, &cards()).unwrap();
    d.units.push(placed(10, 9, 0, 1, p(3, 4), false, true)); // 1800 ATK
    d.units.push(placed(11, 2, 0, 2, p(2, 4), false, true)); // 1400 ATK
    let (d2, events) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(10),
                path: vec![p(3, 3)],
                face_up: false,
                defence: None,
            },
            &cards(),
        )
        .unwrap();
    assert!(events.contains(&DuelEvent::Crushed { unit: UnitId(10) }));
    assert!(d2.unit(UnitId(10)).is_none());
    assert!(d
        .apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(11),
                path: vec![p(2, 3)],
                face_up: false,
                defence: None
            },
            &cards()
        )
        .is_err());
}

/// 0x269D80 (turn-verified.md §7 D): Exodia's head as the leader with the
/// four limbs face up, unbound and ready around it ends the duel at that
/// side's turn start (reason 2); a spellbound limb does not count.
#[test]
fn exodia_complete_at_turn_start_ends_the_duel() {
    let mut v = cards();
    while v.len() <= usize::from(EXODIA_HEAD) {
        let id = v.len() as u16;
        v.push(monster(id, kind::MACHINE, 1, 0, 100, 100));
    }
    for limb in EXODIA_LIMBS {
        v[usize::from(limb)] = monster(limb, kind::SPELLCASTER, 1, 1, 200, 300);
    }
    v[usize::from(EXODIA_HEAD)] = monster(EXODIA_HEAD, kind::SPELLCASTER, 3, 1, 1000, 1000);
    let (mut d, _) = Duel::new(setup([vec![], vec![]]), &v).unwrap();
    d.units.iter_mut().find(|u| u.side == 1 && u.slot == 0).unwrap().card = EXODIA_HEAD;
    for (i, (limb, cell)) in EXODIA_LIMBS.into_iter().zip([p(2, 0), p(4, 0), p(2, 1), p(4, 1)]).enumerate() {
        d.units.push(placed(30 + i as u16, limb, 1, i as u8 + 1, cell, false, true));
    }
    let mut bound = d.clone();
    let (d2, events) = d.apply(0, &DuelCommand::EndTurn, &v).unwrap();
    assert_eq!(d2.outcome, Some(Outcome { winner: Some(1), reason: EndReason::Exodia }));
    assert!(events.iter().any(|e| matches!(e, DuelEvent::Ended { .. })));
    bound.units.iter_mut().find(|u| u.card == EXODIA_LIMBS[2]).unwrap().spellbind = 2;
    let (d3, _) = bound.apply(0, &DuelCommand::EndTurn, &v).unwrap();
    assert_eq!(d3.outcome, None);
    assert_eq!(d3.active, 1);
}

#[test]
fn leader_trapped_and_turn_limit_end_the_duel() {
    let (mut d, _) = Duel::new(setup([vec![], vec![]]), &cards()).unwrap();
    for (i, cell) in [p(2, 5), p(3, 5), p(4, 5), p(2, 6), p(4, 6)]
        .into_iter()
        .enumerate()
    {
        d.units
            .push(placed(20 + i as u16, 2, 1, i as u8 + 1, cell, false, true));
    }
    let (d2, _) = d.apply(0, &DuelCommand::EndTurn, &cards()).unwrap();
    assert_eq!(
        d2.outcome,
        Some(Outcome {
            winner: Some(1),
            reason: EndReason::LeaderTrapped
        })
    );
    let (mut d, _) = Duel::new(setup([vec![], vec![]]), &cards()).unwrap();
    d.round = ROUND_LIMIT;
    d.active = 1;
    d.sides[0].lp = 100;
    let (d2, _) = d.apply(1, &DuelCommand::EndTurn, &cards()).unwrap();
    assert_eq!(
        d2.outcome,
        Some(Outcome {
            winner: Some(1),
            reason: EndReason::TurnLimit
        })
    );
}

#[test]
fn unverified_rules_are_refused_not_guessed() {
    let mut c = cards();
    c[2].effects[3] = true;
    c[2].entries[3] = Some([0x9900, 0]); // an untraced flip type
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &c).unwrap();
    let (d, _) = d
        .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &c)
        .unwrap();
    let (d, _) = d
        .apply(0, &DuelCommand::ChooseSummon { hand: vec![0] }, &c)
        .unwrap();
    let id = d.unit_at(p(3, 5)).unwrap().id;
    assert!(matches!(
        d.apply(
            0,
            &DuelCommand::Move {
                unit: id,
                path: vec![],
                face_up: true,
                defence: None
            },
            &c
        ),
        Err(DuelError::Unsupported(_))
    ));
    // Ability #14 (the opposing leader's race-weakening aura) is accepted
    // since 1 Oct; #7 since 7 Oct (refusals_tests.rs); numbers past #16
    // are refused.
    let mut s = setup([vec![], vec![]]);
    s.leader_abilities[0] = vec![17];
    assert!(matches!(
        Duel::new(s, &cards()),
        Err(DuelError::Unsupported(_))
    ));
    // Ability #8 (the kind-matching summon discount) is accepted.
    let mut s = setup([vec![], vec![]]);
    s.leader_abilities[1] = vec![8];
    assert!(Duel::new(s, &cards()).is_ok());
}

#[test]
fn set_cards_cost_nothing_use_other_slots_and_block_unverified_play() {
    // Mirrors Weevil's turns 2 and 3: a trap and a power-up set face-down at
    // no cost in slot 6 upwards, then committed to defence in place.
    let (d, _) = Duel::new(
        setup([vec![2; 40], [vec![12, 13], vec![3; 38]].concat()]),
        &cards(),
    )
    .unwrap();
    let (d, _) = run(&d, 0, DuelCommand::EndTurn);
    let stars = d.sides[1].stars;
    let d = summon(&d, 1, p(3, 1), 0);
    let trap = d.unit_at(p(3, 1)).unwrap().clone();
    assert_eq!((trap.card, trap.slot, trap.face_up), (12, 6, false));
    assert_eq!(d.sides[1].stars, stars);
    let (d, _) = run(
        &d,
        1,
        DuelCommand::Move {
            unit: trap.id,
            path: vec![],
            face_up: false,
            defence: Some(true),
        },
    );
    let trap = d.unit(trap.id).unwrap();
    assert!(trap.acted && trap.committed_defence && !trap.face_up);
    // A set card moves one step (MONSTER-EFFECTS.md §3.2); revealing a trap
    // is not traced.
    let mut fresh = d.clone();
    fresh.units.iter_mut().for_each(|u| u.acted = false);
    let (moved, _) = fresh
        .apply(
            1,
            &DuelCommand::Move {
                unit: trap.id,
                path: vec![p(3, 2)],
                face_up: false,
                defence: None,
            },
            &cards(),
        )
        .unwrap();
    assert_eq!(moved.unit(trap.id).unwrap().position, p(3, 2));
    assert!(matches!(
        fresh.apply(
            1,
            &DuelCommand::Move {
                unit: trap.id,
                path: vec![],
                face_up: true,
                defence: None,
            },
            &cards()
        ),
        Err(DuelError::Unsupported(_))
    ));
    assert_eq!(d.reachable(trap.id, &cards()).unwrap().len(), 3);
    let (d, _) = run(&d, 1, DuelCommand::EndTurn);
    // Block Attack fires on the opponent's move from anywhere (savestates
    // 688-690): the move is cancelled and used, the mover stays face-down.
    let d = summon(&d, 0, p(3, 5), 0);
    let mine = d.unit_at(p(3, 5)).unwrap().id;
    let (d, events) = step(&d, 0, mine, vec![p(3, 4)]);
    let unit = d.unit(mine).unwrap();
    assert_eq!(
        (unit.position, unit.acted, unit.face_up),
        (p(3, 5), true, false)
    );
    assert!(d.unit(trap.id).is_none());
    assert_eq!(d.sides[1].graveyard, [12]);
    assert!(events.contains(&DuelEvent::Cancelled { unit: mine }));
    let (d, _) = run(&d, 0, DuelCommand::EndTurn);
    let d = summon(&d, 1, p(2, 0), 0);
    let power_up = d.unit_at(p(2, 0)).unwrap();
    assert_eq!((power_up.card, power_up.slot), (13, 6));
    assert_eq!(power_up.summon_order, 1);
    #[cfg(feature = "online")]
    assert_eq!(
        d.view(0)
            .units
            .iter()
            .find(|u| u.position == p(2, 0))
            .unwrap()
            .card,
        None
    );
}

#[cfg(feature = "online")]
#[test]
fn views_hide_the_opponents_hidden_information() {
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap();
    let d = summon(&d, 0, p(3, 5), 0);
    let mine = d.view(0);
    let theirs = d.view(1);
    let unit = |v: &DuelView| {
        v.units
            .iter()
            .find(|u| u.position == p(3, 5))
            .unwrap()
            .clone()
    };
    assert_eq!(unit(&mine).card, Some(2));
    assert_eq!(unit(&theirs).card, None);
    assert_eq!(theirs.hand, Vec::<u16>::new());
    assert_eq!(theirs.hand_counts[0], 4);
    let json = serde_json::to_string(&theirs).unwrap();
    assert!(!json.contains("\"deck\""));
}

/// A face-down unit taken by Change of Heart (+0x57 bit 1) is shown to its
/// new controller (the original's 0x1DA6A0 / 0x268CA0 read the controller)
/// and stays known to its owner; it goes back to hidden for the taker when
/// control returns.
#[cfg(feature = "online")]
#[test]
fn a_stolen_face_down_unit_is_seen_by_its_controller() {
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap();
    let mut d = summon(&d, 0, p(3, 5), 0);
    let at = |v: &DuelView| v.units.iter().find(|u| u.position == p(3, 5)).unwrap().clone();
    assert_eq!(at(&d.view(1)).card, None);
    let id = d.unit_at(p(3, 5)).unwrap().id;
    d.units.iter_mut().find(|u| u.id == id).unwrap().stolen = true;
    assert_eq!(d.unit(id).unwrap().controller(), 1);
    assert_eq!(at(&d.view(1)).card, Some(2), "the taker sees the card");
    assert_eq!(at(&d.view(0)).card, Some(2), "the owner still knows it");
    assert!(!at(&d.view(1)).face_up);
    d.units.iter_mut().find(|u| u.id == id).unwrap().stolen = false;
    assert_eq!(at(&d.view(1)).card, None);
}

#[cfg(feature = "online")]
#[test]
fn forfeit_ends_the_duel_from_either_side_and_only_once() {
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap();
    // Side 1 is not acting: Surrender is refused, the online forfeit is not.
    assert_eq!(
        d.apply(1, &DuelCommand::Surrender, &cards()).unwrap_err(),
        DuelError::NotYourTurn
    );
    let (open, _) = run(&d, 0, DuelCommand::OpenSummon { cell: p(3, 5) });
    let (done, events) = open.forfeit(1).unwrap();
    let outcome = Outcome {
        winner: Some(0),
        reason: EndReason::Surrender,
    };
    assert_eq!(done.outcome, Some(outcome));
    assert_eq!(events, vec![DuelEvent::Ended { outcome }]);
    assert_eq!(done.pending_summon, None);
    assert_eq!(done.forfeit(0).unwrap_err(), DuelError::Finished);
    assert!(d.forfeit(2).is_err());
}

#[test]
fn campaign_deck_build_interleaves_sides_and_is_deterministic() {
    let a: Vec<u16> = (100..140).collect();
    let b: Vec<u16> = (200..240).collect();
    let mut rng = 1;
    let decks = build_campaign_decks(&mut rng, [&a, &b]);
    let mut rng2 = 1;
    assert_eq!(decks, build_campaign_decks(&mut rng2, [&a, &b]));
    let mut sorted = decks[0].clone();
    sorted.sort();
    assert_eq!(sorted, a);
    assert_ne!(decks[0], a);
    let mut with_rejected = a.clone();
    with_rejected[0] = REJECTED_DECK_CARD;
    let mut rng3 = 1;
    assert_eq!(
        build_campaign_decks(&mut rng3, [&with_rejected, &b])[0].len(),
        39
    );
}

#[test]
fn computer_key_presses_follow_the_traced_emitter() {
    use ai::pad::*;
    use ai::{Action, Position};
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap();
    // Side 1's d-pad is mirrored; hand scrolling is not. Two hand indices
    // form a fusion list, each marked with Up.
    let summon = Action::Summon {
        cell: p(3, 1),
        hand: vec![2, 0],
    };
    assert_eq!(
        ai::keys(&d, 1, p(5, 2), &summon, &cards()),
        [
            RIGHT, RIGHT, DOWN, DOWN, SQUARE, UP, CROSS, RIGHT, RIGHT, UP, LEFT, LEFT, UP, CROSS,
            CROSS
        ]
    );
    assert_eq!(ai::cursor_after(&summon), p(3, 1));
    // Side 0: select, reveal (R2), then columns before rows...
    let mut d = board(vec![
        placed(10, 2, 0, 1, p(3, 5), false, false),
        placed(11, 4, 0, 2, p(4, 4), true, false),
    ]);
    d.units.iter_mut().for_each(|u| u.acted = false);
    let attack = Action::Act {
        unit: UnitId(10),
        target: p(2, 4),
        reveal: true,
        position: Position::Keep,
    };
    assert_eq!(
        ai::keys(&d, 0, p(3, 5), &attack, &cards()),
        [CROSS, R2, LEFT, UP, CROSS]
    );
    assert_eq!(
        ai::path(&d, UnitId(10), p(2, 4), &cards()),
        [p(2, 5), p(2, 4)]
    );
    // ...unless the column-first corner is taken (0x2784A0).
    d.units.push(placed(12, 4, 0, 3, p(2, 5), false, false));
    assert_eq!(
        ai::keys(&d, 0, p(3, 5), &attack, &cards()),
        [CROSS, R2, UP, LEFT, CROSS]
    );
    assert_eq!(
        ai::path(&d, UnitId(10), p(2, 4), &cards()),
        [p(3, 4), p(2, 4)]
    );
    // Staying in place: R1 enters defence, L1 leaves committed defence;
    // leaders never get R2.
    let stay = |unit, at, position| Action::Act {
        unit: UnitId(unit),
        target: at,
        reveal: true,
        position,
    };
    assert_eq!(
        ai::keys(
            &d,
            0,
            p(0, 0),
            &stay(0, p(3, 6), Position::Defence),
            &cards()
        ),
        [RIGHT, RIGHT, RIGHT, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, CROSS, CROSS]
    );
    assert_eq!(
        ai::keys(
            &d,
            0,
            p(4, 4),
            &stay(11, p(4, 4), Position::Attack),
            &cards()
        ),
        [CROSS, R2, L1, CROSS]
    );
    assert_eq!(
        ai::keys(
            &d,
            0,
            p(3, 5),
            &stay(10, p(3, 5), Position::Defence),
            &cards()
        ),
        [CROSS, R2, R1, CROSS]
    );
}

#[test]
fn negate_attack_stops_the_attack_where_the_attacker_stands() {
    let mut d = board(vec![
        placed(10, 9, 0, 1, p(3, 3), false, true),
        placed(11, 2, 1, 1, p(3, 2), false, true),
        placed(12, 14, 1, 6, p(0, 1), false, false),
    ]);
    d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let (d, events) = step(&d, 0, UnitId(10), vec![p(3, 2)]);
    assert_eq!(d.unit(UnitId(10)).unwrap().position, p(3, 3));
    assert!(d.unit(UnitId(10)).unwrap().acted);
    assert!(d.unit(UnitId(11)).is_some());
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [4000, 4000]);
    assert_eq!(d.sides[1].graveyard, [14]);
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Battle { .. })));
}

#[test]
fn limited_range_traps_follow_their_byte1_case() {
    let setup = |mover_card, trap_card| {
        let mut d = board(vec![
            placed(10, mover_card, 0, 1, p(3, 3), false, false),
            placed(11, trap_card, 1, 6, p(2, 2), false, false),
        ]);
        d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
        d
    };
    // Bear Trap (case 8): a monster with ATK <= 2000 entering a cell next to
    // it is destroyed; a stronger one walks on.
    let (d, _) = step(&setup(2, 15), 0, UnitId(10), vec![p(3, 2)]);
    assert!(d.unit(UnitId(10)).is_none());
    assert_eq!(
        (d.sides[0].graveyard.clone(), d.sides[1].graveyard.clone()),
        (vec![2], vec![15])
    );
    let (d, _) = step(&setup(10, 15), 0, UnitId(10), vec![p(3, 2)]);
    assert_eq!(d.unit(UnitId(10)).unwrap().position, p(3, 2));
    assert!(d.unit(UnitId(11)).is_some());
    // Infinite Dismissal (case 4, 0x30000): stopped, revealed, bound for three
    // of its own turns (not counted down at the end of the current one).
    let (mut d, _) = step(&setup(2, 16), 0, UnitId(10), vec![p(3, 2)]);
    let unit = d.unit(UnitId(10)).unwrap();
    assert_eq!(
        (unit.position, unit.committed_face_up, unit.spellbind),
        (p(3, 2), true, 3)
    );
    for expected in [3, 2, 1, 0] {
        d = run(&d, 0, DuelCommand::EndTurn).0;
        assert_eq!(d.unit(UnitId(10)).unwrap().spellbind, expected);
        d = run(&d, 1, DuelCommand::EndTurn).0;
        assert_eq!(d.unit(UnitId(10)).unwrap().acted, expected > 0);
    }
    // Tears of the Mermaid (case 3, 0x10258): one turn and -600 ATK/DEF.
    let (d, _) = step(&setup(2, 17), 0, UnitId(10), vec![p(3, 2)]);
    let unit = d.unit(UnitId(10)).unwrap();
    assert_eq!(
        (unit.attack_bonus, unit.defense_bonus, unit.spellbind),
        (-600, -600, 1)
    );
    assert_eq!(d.effective(unit, unit.position, true, &cards()), 800);
    // Untraced full-range trap types are refused rather than ignored.
    let d = setup(2, 18);
    assert!(matches!(
        d.apply(0, &DuelCommand::EndTurn, &cards()),
        Err(DuelError::Unsupported(_))
    ));
}

#[test]
fn a_spellbound_attacker_loses_its_next_turn() {
    // 0x251CD0 marks a bind made during the unit's own turn (+0x58 bit3), so
    // that turn's end does not count it down (0x273E20).
    let mut d = board(vec![
        placed(10, 3, 0, 1, p(3, 3), false, true),
        placed(11, 4, 1, 1, p(3, 2), true, true),
    ]);
    d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    // Wind dragon (1200) attacks an earth warrior in defence (DEF 1200):
    // nothing dies, and earth beats wind, so the attacker is bound.
    let (d, _) = step(&d, 0, UnitId(10), vec![p(3, 2)]);
    assert_eq!(d.unit(UnitId(10)).unwrap().spellbind, 1);
    let d = run(&d, 0, DuelCommand::EndTurn).0;
    assert_eq!(d.unit(UnitId(10)).unwrap().spellbind, 1);
    let d = run(&d, 1, DuelCommand::EndTurn).0;
    assert!(d.unit(UnitId(10)).unwrap().acted);
    let d = run(&d, 0, DuelCommand::EndTurn).0;
    assert_eq!(d.unit(UnitId(10)).unwrap().spellbind, 0);
}

/// The compatibility bit of a power-up is `id - 0x2F0`, so the synthetic
/// power-ups 22/23 are copied to ids 0x2F0/0x2F1; Warriors 2 and 4 fit both.
fn with_equips() -> Vec<CardInfo> {
    let mut v = cards();
    while v.len() <= 0x2F1 {
        let n = v.len() as u16;
        v.push(monster(n, kind::MACHINE, 1, 0, 100, 100));
    }
    for (from, to) in [(22usize, 0x2F0usize), (23, 0x2F1)] {
        v[to] = CardInfo {
            id: to as u16,
            ..v[from].clone()
        };
    }
    for id in [2usize, 4] {
        v[id].equip_bits[0] = 0b11;
    }
    v
}

#[test]
fn power_ups_equip_by_moving_and_attacked_cards_are_destroyed() {
    let c = with_equips();
    let run = |d: &Duel, side, cmd| d.apply(side, &cmd, &c);
    let mv = |unit, path: Vec<GridPos>| DuelCommand::Move {
        unit: UnitId(unit),
        path,
        face_up: false,
        defence: None,
    };
    let mut d = board(vec![
        placed(10, 2, 0, 1, p(3, 4), false, false), // Warrior, fits
        placed(11, 9, 0, 2, p(4, 4), false, false), // Machine, does not
        placed(12, 0x2F0, 0, 6, p(3, 3), false, false),
        placed(13, 0x2F0, 0, 7, p(4, 3), false, false),
        placed(14, 0x2F0, 1, 6, p(5, 3), false, false),
        placed(15, 0x2F1, 0, 8, p(2, 4), false, false),
    ]);
    d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    // Fits (E2): moves in, +500/+500, stays face-down, card to the graveyard.
    let (d1, _) = run(&d, 0, mv(10, vec![p(3, 3)])).unwrap();
    let u = d1.unit(UnitId(10)).unwrap();
    assert_eq!(
        (
            u.position,
            u.attack_bonus,
            u.defense_bonus,
            u.face_up,
            u.acted
        ),
        (p(3, 3), 500, 500, false, true)
    );
    assert_eq!(d1.sides[0].graveyard, [0x2F0]);
    // Does not fit (E3): moves in, nothing gained, card lost.
    let (d2, _) = run(&d1, 0, mv(11, vec![p(4, 3)])).unwrap();
    let u = d2.unit(UnitId(11)).unwrap();
    assert_eq!((u.position, u.attack_bonus), (p(4, 3), 0));
    // An enemy set card attacked by a monster it does not fit (E1): the card
    // is destroyed and the attacker advances without a reveal.
    let mut d3 = d2.clone();
    d3.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let (d3, events) = run(&d3, 0, mv(11, vec![p(5, 3)])).unwrap();
    assert_eq!(d3.unit(UnitId(11)).unwrap().position, p(5, 3));
    assert!(!d3.unit(UnitId(11)).unwrap().face_up);
    assert_eq!(d3.sides[1].graveyard, [0x2F0]);
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Battle { .. })));
    // A transforming power-up remains refused.
    assert!(matches!(
        run(&d, 0, mv(10, vec![p(2, 4)])),
        Err(DuelError::Unsupported(_))
    ));
    let mut d4 = d1.clone();
    d4.units.retain(|u| u.id != UnitId(14));
    d4.units
        .push(placed(16, 0x2F0, 1, 6, p(3, 2), false, false));
    d4.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let (d5, _) = run(&d4, 0, mv(10, vec![p(3, 2)])).unwrap();
    let attacker = d5.unit(UnitId(10)).unwrap();
    assert_eq!(
        (
            attacker.position,
            attacker.attack_bonus,
            attacker.defense_bonus
        ),
        (p(3, 2), 1000, 1000)
    );
    assert!(!attacker.face_up);
    assert_eq!(d5.sides[1].graveyard, [0x2F0]);
    assert_eq!(d5.experience[0][&2].delta, 6);
}

#[test]
fn magic_cards_take_effect_when_revealed() {
    let c = cards();
    let reveal = |unit, path: Vec<GridPos>| DuelCommand::Move {
        unit: UnitId(unit),
        path,
        face_up: true,
        defence: None,
    };
    let mut d = board(vec![
        placed(10, 19, 0, 6, p(2, 5), false, false),
        placed(11, 20, 0, 7, p(4, 5), false, false),
        placed(12, 21, 0, 8, p(2, 6), false, false),
        placed(13, 9, 0, 1, p(5, 5), false, false),
        placed(14, 8, 1, 1, p(3, 1), false, true),
        placed(15, 24, 0, 9, p(4, 6), false, false),
    ]);
    d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    d.terrain[p(0, 0).index().unwrap()] = terrain::LABYRINTH;
    d.terrain[p(2, 3).index().unwrap()] = terrain::LABYRINTH;
    // Field (E4): radius 2 around the card's cell after one step, Labyrinth
    // kept; the card goes to the graveyard first.
    let (d1, _) = d.apply(0, &reveal(10, vec![p(2, 4)]), &c).unwrap();
    for (i, &t) in d1.terrain.iter().enumerate() {
        let (col, row) = ((i % 7) as u8, (i / 7) as u8);
        let near = col.abs_diff(2) + row.abs_diff(4) <= 2;
        let wall = i == p(2, 3).index().unwrap() || i == p(0, 0).index().unwrap();
        assert_eq!(
            t,
            if wall {
                8
            } else if near {
                2
            } else {
                7
            },
            "cell ({col},{row})"
        );
    }
    assert!(d1.unit(UnitId(10)).is_none());
    assert_eq!(d1.sides[0].graveyard, [19]);
    // LP gain, clamped at 9999.
    let (d2, _) = d1.apply(0, &reveal(11, vec![]), &c).unwrap();
    assert_eq!(d2.sides[0].lp, 5000);
    let mut rich = d1.clone();
    rich.sides[0].lp = 9500;
    assert_eq!(
        rich.apply(0, &reveal(11, vec![]), &c).unwrap().0.sides[0].lp,
        MAX_LP
    );
    // Stat boost: every non-leader Machine on both sides.
    let (d3, _) = d2.apply(0, &reveal(12, vec![]), &c).unwrap();
    assert_eq!(d3.unit(UnitId(13)).unwrap().attack_bonus, 600);
    assert_eq!(d3.unit(UnitId(14)).unwrap().defense_bonus, 600);
    // Revealing a ritual is not traced yet.
    assert!(matches!(
        d.apply(0, &reveal(15, vec![]), &c),
        Err(DuelError::Unsupported(_))
    ));
}

#[test]
fn moving_set_cards_follow_the_traced_combinations() {
    let c = with_equips();
    let go = |d: &Duel, side, unit, to| {
        let mut d = d.clone();
        d.units.iter_mut().for_each(|u| u.acted = u.side != side);
        d.apply(
            side,
            &DuelCommand::Move {
                unit: UnitId(unit),
                path: vec![to],
                face_up: false,
                defence: None,
            },
            &c,
        )
        .unwrap()
        .0
    };
    let d = board(vec![
        placed(10, 2, 0, 1, p(3, 4), false, false), // Warrior (fits 0x2F0)
        placed(11, 9, 0, 2, p(1, 4), false, false), // Machine (does not)
        placed(12, 0x2F0, 0, 6, p(3, 3), false, false), // own power-up
        placed(13, 0x2F0, 0, 7, p(1, 3), false, false), // own power-up
        placed(14, 12, 0, 8, p(5, 4), false, false), // own trap card
        placed(15, 19, 0, 9, p(5, 5), false, false), // own magic card
        placed(16, 2, 1, 1, p(5, 2), false, false), // enemy Warrior
        placed(17, 19, 1, 6, p(0, 2), false, false), // enemy magic card
    ]);
    // Power-up onto an own monster it fits: the monster gains and stays.
    let d1 = go(&d, 0, 12, p(3, 4));
    let u = d1.unit(UnitId(10)).unwrap();
    assert_eq!((u.position, u.attack_bonus), (p(3, 4), 500));
    assert!(d1.unit(UnitId(12)).is_none());
    // Power-up onto an own monster it does not fit: the monster is destroyed
    // and the power-up takes its cell.
    let d2 = go(&d, 0, 13, p(1, 4));
    assert!(d2.unit(UnitId(11)).is_none());
    assert_eq!(d2.unit(UnitId(13)).unwrap().position, p(1, 4));
    assert_eq!(d2.sides[0].graveyard, [9]);
    // Card onto an own card: the stationary card goes, the mover advances.
    let d3 = go(&d, 0, 14, p(5, 5));
    assert!(d3.unit(UnitId(15)).is_none());
    assert_eq!(d3.unit(UnitId(14)).unwrap().position, p(5, 5));
    // A card moving onto an enemy monster is destroyed; a power-up that fits
    // the enemy monster boosts it instead (both code-only).
    let mut d4 = d.clone();
    d4.units.push(placed(18, 12, 0, 10, p(5, 3), false, false));
    let d4 = go(&d4, 0, 18, p(5, 2));
    assert!(d4.unit(UnitId(18)).is_none());
    assert_eq!(d4.unit(UnitId(16)).unwrap().attack_bonus, 0);
    let mut d5 = d.clone();
    d5.units
        .push(placed(18, 0x2F0, 0, 10, p(5, 3), false, false));
    let d5 = go(&d5, 0, 18, p(5, 2));
    assert_eq!(d5.unit(UnitId(16)).unwrap().attack_bonus, 500);
    // A card moving onto an enemy card destroys it and advances.
    let mut d6 = d.clone();
    d6.units.push(placed(18, 12, 0, 10, p(0, 3), false, false));
    let d6 = go(&d6, 0, 18, p(0, 2));
    assert!(d6.unit(UnitId(17)).is_none());
    assert_eq!(d6.unit(UnitId(18)).unwrap().position, p(0, 2));
    // A leader moving onto an own card destroys it and advances.
    let mut d7 = d.clone();
    d7.units.push(placed(18, 12, 0, 10, p(2, 6), false, false));
    let d7 = go(&d7, 0, 0, p(2, 6));
    assert_eq!(d7.unit(UnitId(0)).unwrap().position, p(2, 6));
    assert!(d7.unit(UnitId(18)).is_none());
}

fn fusing() -> Catalogue {
    // Invented pairs: 2 + 3 → 7, 7 + 9 → 6.
    Catalogue::new(with_equips(), [[3, 2, 7], [7, 9, 6]])
}

#[test]
fn fusion_lookup_is_order_free_and_needs_a_table() {
    let c = fusing();
    assert_eq!(c.fusion(2, 3), Some(Some(7)));
    assert_eq!(c.fusion(3, 2), Some(Some(7)));
    assert_eq!(c.fusion(2, 9), Some(None));
    assert_eq!(cards().fusion(2, 3), None);
    let (d, _) = Duel::new(setup([vec![2, 3, 9, 9, 9], vec![]]), &cards()).unwrap();
    let (d, _) = d
        .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &cards())
        .unwrap();
    assert!(matches!(
        d.apply(0, &DuelCommand::ChooseSummon { hand: vec![0, 1] }, &cards()),
        Err(DuelError::Unsupported(_))
    ));
}

#[test]
fn hand_lists_fuse_drop_or_equip_in_marking_order() {
    let c = fusing();
    let summon_list = |d: &Duel, cell, hand: Vec<usize>| {
        let (d, _) = d.apply(0, &DuelCommand::OpenSummon { cell }, &c).unwrap();
        d.apply(0, &DuelCommand::ChooseSummon { hand }, &c)
            .unwrap()
            .0
    };
    let mut d = Duel::new(setup([vec![2, 3, 9, 4, 0x2F0], vec![]]), &c)
        .unwrap()
        .0;
    d.sides[0].stars = 12;
    // 2 + 3 → 7, then 7 + 9 → 6; the cost is the highest marked level (4).
    let d1 = summon_list(&d, p(3, 5), vec![0, 1, 2]);
    let u = d1.unit_at(p(3, 5)).unwrap();
    assert_eq!((u.card, u.face_up, u.acted, u.slot), (6, false, false, 1));
    assert_eq!(d1.sides[0].graveyard, [2, 3, 7, 9]);
    assert_eq!(d1.sides[0].stars, 12 - 4);
    assert_eq!(d1.sides[0].hand, [4, 0x2F0]);
    // No fusion: the older card is dropped and the newer continues.
    let d2 = summon_list(&d, p(3, 5), vec![3, 2]);
    assert_eq!(d2.unit_at(p(3, 5)).unwrap().card, 9);
    assert_eq!(d2.sides[0].graveyard, [4]);
    // A fitting power-up equips the monster whichever comes first.
    for hand in [vec![3, 4], vec![4, 3]] {
        let d3 = summon_list(&d, p(3, 5), hand);
        let u = d3.unit_at(p(3, 5)).unwrap();
        assert_eq!((u.card, u.attack_bonus, u.defense_bonus), (4, 500, 500));
        assert_eq!(d3.sides[0].graveyard, [0x2F0]);
    }
}

#[test]
fn summoning_onto_an_own_unit_puts_it_first_in_the_chain() {
    let c = fusing();
    let mut d = board(vec![placed(10, 4, 0, 3, p(3, 5), false, true)]);
    d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    d.units
        .iter_mut()
        .find(|u| u.id == UnitId(10))
        .unwrap()
        .acted = true;
    d.sides[0].hand = vec![0x2F0, 3];
    d.sides[0].stars = 5;
    // Field Warrior + power-up: the field unit survives with its record
    // (acted, face-up), gaining the bonus; slot and summon order are new.
    let (d1, _) = d
        .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &c)
        .unwrap();
    let (d1, _) = d1
        .apply(0, &DuelCommand::ChooseSummon { hand: vec![0] }, &c)
        .unwrap();
    let u = d1.unit(UnitId(10)).unwrap();
    assert_eq!(
        (u.slot, u.acted, u.committed_face_up, u.attack_bonus),
        (1, true, true, 500)
    );
    // Field 4 + hand 3 do not fuse: the field card is dropped (to the
    // graveyard) and 3 is summoned fresh.
    let (d2, _) = d
        .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &c)
        .unwrap();
    let (d2, _) = d2
        .apply(0, &DuelCommand::ChooseSummon { hand: vec![1] }, &c)
        .unwrap();
    assert!(d2.unit(UnitId(10)).is_none());
    assert_eq!(d2.unit_at(p(3, 5)).unwrap().card, 3);
    assert_eq!(d2.sides[0].graveyard, [4]);
}

#[test]
fn summon_equips_ignore_untriggered_decree_and_own_goblin_fan() {
    // 0x272560 prefix: Decree alone never starts a reaction; Goblin Fan
    // is searched only in the opponent's group. Exercise both chain orders
    // through real commands, including the trap's unchanged flags/graveyard.
    for (trap_type, side) in [
        (trap_type::ROYAL_DECREE, 0),
        (trap_type::ROYAL_DECREE, 1),
        (trap_type::GOBLIN_FAN, 0),
    ] {
        for face_up in [false, true] {
            for hand in [vec![0, 1], vec![1, 0]] {
                let mut c = with_equips();
                c[12].entries[0] = Some([trap_type << 8 | 0xBD, 0]);
                let trap = placed(10, 12, side, 6, p(0, 3), face_up, false);
                let mut d = board(vec![trap.clone()]);
                d.sides[0].hand = vec![4, 0x2F0];
                d.sides[0].stars = 12;
                let (d, _) = d
                    .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &c)
                    .unwrap();
                let (d, events) = d.apply(0, &DuelCommand::ChooseSummon { hand }, &c).unwrap();
                let monster = d.unit_at(p(3, 5)).unwrap();
                assert_eq!(
                    (monster.card, monster.attack_bonus, monster.defense_bonus),
                    (4, 500, 500)
                );
                assert_eq!(d.unit(trap.id), Some(&trap));
                assert_eq!(d.sides[0].graveyard, [0x2F0]);
                assert!(d.sides[1].graveyard.is_empty());
                assert!(!events.iter().any(|event| matches!(event,
                    DuelEvent::Activated { unit, .. } if *unit == trap.id)));
            }
        }
    }
}

#[test]
fn summon_equips_still_refuse_actual_untraced_trap_reactions_atomically() {
    for (trap_type, side) in [(0x2B, 0), (0x2B, 1), (trap_type::GOBLIN_FAN, 1)] {
        let mut c = with_equips();
        c[12].entries[0] = Some([trap_type << 8 | 0xBD, 0]);
        let mut d = board(vec![placed(10, 12, side, 6, p(0, 3), false, false)]);
        d.sides[0].hand = vec![4, 0x2F0];
        d.sides[0].stars = 12;
        let (d, _) = d
            .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &c)
            .unwrap();
        let before = serde_json::to_value(&d).unwrap();
        assert!(matches!(
            d.apply(0, &DuelCommand::ChooseSummon { hand: vec![0, 1] }, &c),
            Err(DuelError::Unsupported(reason)) if reason == "equip negation traps in a summon list"
        ));
        assert_eq!(serde_json::to_value(&d).unwrap(), before);
    }
}

#[test]
fn a_full_monster_zone_sends_the_summon_to_the_graveyard() {
    let c = fusing();
    let mut units: Vec<DuelUnit> = (1..=5)
        .map(|slot| placed(10 + u16::from(slot), 9, 0, slot, p(slot, 3), false, false))
        .collect();
    units.push(placed(20, 9, 1, 1, p(0, 0), false, false));
    let mut d = board(units);
    d.sides[0].hand = vec![2];
    let (d, _) = d
        .apply(0, &DuelCommand::OpenSummon { cell: p(3, 5) }, &c)
        .unwrap();
    let (d, _) = d
        .apply(0, &DuelCommand::ChooseSummon { hand: vec![0] }, &c)
        .unwrap();
    assert!(d.unit_at(p(3, 5)).is_none());
    assert_eq!(d.sides[0].graveyard, [2]);
    assert_eq!(d.sides[0].stars, START_STARS - 4);
}

#[test]
fn moving_onto_an_own_monster_fuses_with_the_slot_quirk() {
    let c = fusing();
    let go = |d: &Duel, unit, to| {
        d.apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(unit),
                path: vec![to],
                face_up: false,
                defence: None,
            },
            &c,
        )
        .unwrap()
        .0
    };
    // Mover in slot 1: the result keeps slot 1, moves in, and is acted.
    let d = board(vec![
        placed(10, 2, 0, 1, p(3, 4), false, false),
        placed(11, 3, 0, 2, p(3, 3), false, false),
    ]);
    let mut d = d;
    d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let d1 = go(&d, 10, p(3, 3));
    let u = d1.unit(UnitId(10)).unwrap();
    assert_eq!((u.card, u.position, u.acted, u.slot), (7, p(3, 3), true, 1));
    assert!(d1.unit(UnitId(11)).is_none());
    assert_eq!(d1.sides[0].graveyard, [2, 3]);
    // Mover in slot 3 with slot 2 free: the result lands in slot 2, so the
    // original cannot find the attacker; it stays and is not marked acted.
    let mut q = board(vec![
        placed(10, 2, 0, 3, p(3, 4), false, false),
        placed(11, 3, 0, 1, p(3, 3), false, false),
    ]);
    q.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let q1 = go(&q, 10, p(3, 3));
    let u = q1.unit(UnitId(10)).unwrap();
    assert_eq!(
        (u.card, u.position, u.acted, u.slot),
        (7, p(3, 4), false, 2)
    );
    // No fusion: the target is destroyed and the mover moves in.
    let mut n = board(vec![
        placed(10, 2, 0, 1, p(3, 4), false, false),
        placed(11, 9, 0, 2, p(3, 3), false, false),
    ]);
    n.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let n1 = go(&n, 10, p(3, 3));
    assert_eq!(n1.unit(UnitId(10)).unwrap().position, p(3, 3));
    assert!(n1.unit(UnitId(11)).is_none());
    // A leader moving onto an own monster destroys it and moves in (emu).
    let mut l = board(vec![placed(10, 2, 0, 1, p(3, 5), false, false)]);
    l.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    let l1 = go(&l, 0, p(3, 5));
    assert_eq!(l1.unit(UnitId(0)).unwrap().position, p(3, 5));
    assert!(l1.unit(UnitId(10)).is_none());
}

/// Effect monsters with the PAL entry words of the traced cards.
fn effect_cards() -> Vec<CardInfo> {
    let mut v = cards();
    let with = |id: u16, kind: u8, attr: u8, atk: u16, def: u16, e: usize, w: [u32; 2]| {
        let mut c = monster(id, kind, 2, attr, atk, def);
        c.entries[e] = Some(w);
        c.effects[e] = true;
        c
    };
    v[10] = with(10, kind::THUNDER, 5, 600, 600, 2, [0x13CE, 0x1F4]); // LaLa Li-oon
    v[11] = with(11, kind::INSECT, 5, 600, 700, 2, [0x12D9, 0x1F4]); // Bladefly
    v[12] = with(12, kind::INSECT, 3, 1800, 1500, 1, [0x085B, 0]); // Dungeon Worm
    v[13] = with(13, kind::DRAGON, 5, 2500, 2300, 0, [0x1B9B, 0x7012C]); // Seiyaryu
    v[14] = with(14, kind::DRAGON, 1, 2000, 1500, 0, [0x023D, 1]); // Curse of Dragon
    v[15] = with(15, kind::FAIRY, 0, 400, 300, 3, [0x23F4, 1]); // Key Mace
    v[16] = with(16, kind::INSECT, 3, 450, 600, 3, [0x049A, 0]); // Man-Eater Bug
    v[17] = with(17, kind::INSECT, 3, 900, 800, 3, [0x179A, 1]); // Nightmare Scorpion
    v[18] = with(18, kind::FAIRY, 0, 500, 700, 3, [0x1254, 0x12C]); // Hoshiningen
    v[19] = with(19, kind::FAIRY, 0, 1800, 1000, 3, [0x1265, 0x03E8_03E8]); // Megami
                                                                            // Ids 20-22 replace the synthetic magic and power-up cards of cards().
    v[20] = with(20, kind::BEAST, 3, 100, 900, 4, [0x0DBD, 0x101F4]); // Needle Ball
    v[21] = monster(21, kind::FIEND, 2, 1, 2600, 1000); // a Fiend
    v[22] = monster(22, kind::FAIRY, 2, 0, 700, 700); // a Light Fairy
    v
}

fn ready(units: Vec<DuelUnit>) -> Duel {
    let mut d = board(units);
    d.units.iter_mut().for_each(|u| u.acted = u.side != 0);
    d
}

fn attack(d: &Duel, unit: u16, to: GridPos, c: &[CardInfo]) -> (Duel, Vec<DuelEvent>) {
    d.apply(
        0,
        &DuelCommand::Move {
            unit: UnitId(unit),
            path: vec![to],
            face_up: false,
            defence: None,
        },
        &c.to_vec(),
    )
    .unwrap()
}

#[test]
fn continuous_auras_and_the_wall_flag() {
    let c = effect_cards();
    // A face-up committed-defence LaLa Li-oon weakens every Machine by 500
    // (both sides); a Bladefly boosts Wind monsters, itself included.
    let d = ready(vec![
        placed(30, 10, 0, 1, p(0, 5), true, true),
        placed(31, 9, 0, 2, p(1, 5), false, true), // Machine 1800
        placed(32, 9, 1, 1, p(1, 1), false, true),
        placed(33, 11, 1, 2, p(5, 1), true, true),
        placed(34, 3, 0, 3, p(2, 5), false, true), // Wind dragon 1200
    ]);
    let eff = |id| {
        let u = d.unit(UnitId(id)).unwrap();
        d.effective(u, u.position, true, &c)
    };
    assert_eq!((eff(31), eff(32)), (1300, 1300));
    assert_eq!((eff(33), eff(34)), (1100, 1700));
    assert_eq!(eff(30), 600 + 500); // LaLa is Wind: Bladefly boosts it
                                    // Dungeon Worm crosses an empty Labyrinth cell.
    let mut d = ready(vec![placed(35, 12, 0, 1, p(3, 4), false, false)]);
    d.terrain[p(3, 3).index().unwrap()] = terrain::LABYRINTH;
    let (d, _) = attack(&d, 35, p(3, 3), &c);
    assert_eq!(d.unit(UnitId(35)).unwrap().position, p(3, 3));
}

#[test]
fn battle_pre_pass_bonuses_and_terrain() {
    let c = effect_cards();
    // Seiyaryu gets +300 against a Fiend: 2500 + 300 vs 2600 wins, and the
    // delta stays on both stats (0x1E16D0 / 0x1E17F0 op 0; emu exp_prepass
    // P1).
    let d = ready(vec![
        placed(30, 13, 0, 1, p(3, 4), false, true),
        placed(31, 21, 1, 1, p(3, 3), false, true),
    ]);
    let (d1, _) = attack(&d, 30, p(3, 3), &c);
    assert!(d1.unit(UnitId(31)).is_none());
    assert_eq!(d1.sides[1].lp, 4000 - 200);
    let s = d1.unit(UnitId(30)).unwrap();
    assert_eq!((s.attack_bonus, s.defense_bonus), (300, 300));
    // As a defender in attack position (0x248DA0, emu P2): the Fiend's 2600
    // meets 2500 + 300 and dies; Seiyaryu keeps +300/+300.
    let d = ready(vec![
        placed(30, 21, 0, 1, p(3, 4), false, true),
        placed(31, 13, 1, 1, p(3, 3), false, true),
    ]);
    let (d1, _) = attack(&d, 30, p(3, 3), &c);
    assert!(d1.unit(UnitId(30)).is_none());
    assert_eq!(d1.sides[0].lp, 4000 - 200);
    let s = d1.unit(UnitId(31)).unwrap();
    assert_eq!((s.attack_bonus, s.defense_bonus), (300, 300));
    // Curse of Dragon turns the defender's cell into Wasteland (1) first.
    let d = ready(vec![
        placed(30, 14, 0, 1, p(3, 4), false, true),
        placed(31, 9, 1, 1, p(3, 3), false, true),
    ]);
    let (d2, _) = attack(&d, 30, p(3, 3), &c);
    assert_eq!(d2.terrain[p(3, 3).index().unwrap()], terrain::WASTELAND);
}

#[test]
fn flip_effects_fire_on_the_traced_hooks() {
    let c = effect_cards();
    // Key Mace revealed as defender: everything else goes to committed
    // defence, so the attacker's attack is cancelled without damage.
    let d = ready(vec![
        placed(30, 9, 0, 1, p(3, 4), false, true),
        placed(31, 15, 1, 1, p(3, 3), false, false),
    ]);
    let (d1, _) = attack(&d, 30, p(3, 3), &c);
    let u = d1.unit(UnitId(30)).unwrap();
    assert!(u.defence && u.committed_defence);
    assert_eq!([d1.sides[0].lp, d1.sides[1].lp], [4000, 4000]);
    // Man-Eater Bug as defender destroys the attacker; no damage or advance.
    let d = ready(vec![
        placed(30, 9, 0, 1, p(3, 4), false, true),
        placed(31, 16, 1, 1, p(3, 3), false, false),
    ]);
    let (d2, _) = attack(&d, 30, p(3, 3), &c);
    assert!(d2.unit(UnitId(30)).is_none());
    assert_eq!(d2.unit(UnitId(31)).unwrap().position, p(3, 3));
    // Man-Eater Bug as attacker destroys the defender and advances.
    let d = ready(vec![
        placed(30, 16, 0, 1, p(3, 4), false, false),
        placed(31, 9, 1, 1, p(3, 3), false, true),
    ]);
    let (d3, _) = attack(&d, 30, p(3, 3), &c);
    assert!(d3.unit(UnitId(31)).is_none());
    assert_eq!(d3.unit(UnitId(30)).unwrap().position, p(3, 3));
    // Nightmare Scorpion as defender binds the attacker: no battle.
    let d = ready(vec![
        placed(30, 9, 0, 1, p(3, 4), false, true),
        placed(31, 17, 1, 1, p(3, 3), false, false),
    ]);
    let (d4, _) = attack(&d, 30, p(3, 3), &c);
    assert_eq!(d4.unit(UnitId(30)).unwrap().spellbind, 1);
    assert!(d4.unit(UnitId(31)).is_some());
    // Manual flips (role 0): Hoshiningen boosts own Light monsters by 300,
    // itself included; Megami boosts own monsters with ATK <= 1000 by 1000.
    let d = ready(vec![
        placed(30, 18, 0, 1, p(3, 4), false, false),
        placed(31, 22, 0, 2, p(2, 5), false, true),
        placed(32, 19, 0, 3, p(4, 5), false, false),
        placed(33, 22, 1, 1, p(1, 1), false, true),
    ]);
    let flip = |d: &Duel, unit| {
        d.apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(unit),
                path: vec![],
                face_up: true,
                defence: None,
            },
            &c,
        )
        .unwrap()
        .0
    };
    let d5 = flip(&d, 30);
    let bonus = |d: &Duel, id| d.unit(UnitId(id)).unwrap().attack_bonus;
    assert_eq!(
        (bonus(&d5, 30), bonus(&d5, 31), bonus(&d5, 33)),
        (300, 300, 0)
    );
    let d6 = flip(&d, 32);
    assert_eq!(
        (bonus(&d6, 30), bonus(&d6, 31), bonus(&d6, 32)),
        (1000, 1000, 0)
    );
}

#[test]
fn needle_ball_costs_the_opponent_life_when_destroyed_in_battle() {
    let c = effect_cards();
    let d = ready(vec![
        placed(30, 9, 0, 1, p(3, 4), false, true),
        placed(31, 20, 1, 1, p(3, 3), false, true),
    ]);
    let (d, _) = attack(&d, 30, p(3, 3), &c);
    assert!(d.unit(UnitId(31)).is_none());
    // 1800 vs 100: side 1 takes 1700 battle damage; Needle Ball costs side 0
    // 500.
    assert_eq!([d.sides[0].lp, d.sides[1].lp], [3500, 2300]);
}

/// Feed pad keys through the input machine, applying its commands.
fn drive(
    d: &Duel,
    input: &mut ui::DuelUi,
    keys: &[u16],
    c: &Vec<CardInfo>,
) -> (Duel, Vec<ui::Response>) {
    let mut d = d.clone();
    let mut out = Vec::new();
    for &k in keys {
        let r = input.press(&d, c, k).unwrap();
        if let ui::Response::Commands(cmds) = &r {
            for cmd in cmds {
                d = d.apply(d.active, cmd, c).unwrap().0;
            }
        }
        out.push(r);
    }
    (d, out)
}

#[test]
fn computer_key_queues_drive_the_pad_input_like_the_original() {
    use ai::pad::*;
    use ai::{Action, Position};
    let c = cards();
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &c).unwrap();
    let d = run(&d, 0, DuelCommand::EndTurn).0;
    // Weevil's summon: cursor to his leader (mirrored d-pad), Square, the
    // cell, Cross, hand scrolling (not mirrored), Up, Cross, Cross.
    let summon = Action::Summon {
        cell: p(3, 1),
        hand: vec![2],
    };
    let keys = ai::keys(&d, 1, p(5, 2), &summon, &c);
    let mut input = ui::DuelUi::new(&d);
    input.cursors[1] = p(5, 2);
    let (after, responses) = drive(&d, &mut input, &keys, &c);
    let direct = run(&d, 1, DuelCommand::OpenSummon { cell: p(3, 1) }).0;
    let direct = run(&direct, 1, DuelCommand::ChooseSummon { hand: vec![2] }).0;
    assert_eq!(after, direct);
    assert_eq!(input.mode, ui::Mode::Field);
    assert_eq!(input.cursors[1], ai::cursor_after(&summon));
    let commands = responses
        .iter()
        .filter(|r| matches!(r, ui::Response::Commands(_)))
        .count();
    assert_eq!(commands, 2);
    // An attack with a reveal walks the path the emitter pressed.
    let mut d = board(vec![
        placed(10, 2, 0, 1, p(3, 5), false, false),
        placed(11, 4, 1, 1, p(3, 4), false, false),
    ]);
    d.units.iter_mut().for_each(|u| u.acted = false);
    let attack = Action::Act {
        unit: UnitId(10),
        target: p(3, 4),
        reveal: true,
        position: Position::Keep,
    };
    let keys = ai::keys(&d, 0, p(3, 5), &attack, &c);
    let mut input = ui::DuelUi::new(&d);
    input.cursors[0] = p(3, 5);
    let (after, _) = drive(&d, &mut input, &keys, &c);
    let direct = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(10),
                path: vec![p(3, 4)],
                face_up: true,
                defence: None,
            },
            &c,
        )
        .unwrap()
        .0;
    assert_eq!(after, direct);
    assert_eq!(keys, [CROSS, R2, UP, CROSS]);
}

#[test]
fn move_selection_follows_the_cell_marks() {
    use ai::pad::*;
    let c = cards();
    // A face-up Warrior on Meadow has two steps.
    let mut d = board(vec![placed(10, 10, 0, 1, p(3, 4), false, true)]);
    d.units.iter_mut().for_each(|u| u.acted = false);
    d.terrain[p(3, 4).index().unwrap()] = terrain::MEADOW;
    let mut input = ui::DuelUi::new(&d);
    input.cursors[0] = p(3, 4);
    let (_, r) = drive(&d, &mut input, &[CROSS], &c);
    assert_eq!(r, [ui::Response::Moved]);
    let ui::Mode::Move(sel) = &input.mode else {
        panic!("not selecting")
    };
    assert_eq!(sel.allowance, 2);
    let mark = |sel: &ui::Selection, at: GridPos| sel.marks[at.index().unwrap()];
    assert_eq!(mark(sel, p(3, 4)), 0);
    assert_eq!(mark(sel, p(3, 3)), 1);
    assert_eq!(mark(sel, p(2, 3)), 2);
    assert_eq!(mark(sel, p(3, 2)), 2);
    assert_eq!(mark(sel, p(3, 5)), 1);
    // The own leader's cell is never a target.
    assert_eq!(mark(sel, p(3, 6)), ui::UNMARKED);
    // Two steps, a third is refused, stepping back pops the path.
    let (_, r) = drive(&d, &mut input, &[UP, UP, UP, DOWN], &c);
    assert_eq!(
        r,
        [
            ui::Response::Moved,
            ui::Response::Moved,
            ui::Response::Ignored,
            ui::Response::Moved
        ]
    );
    assert_eq!(input.cursors[0], p(3, 3));
    let ui::Mode::Move(sel) = &input.mode else {
        panic!("not selecting")
    };
    assert_eq!(sel.path, [p(3, 4)]);
    // Circle returns the cursor to the unit.
    drive(&d, &mut input, &[CIRCLE], &c);
    assert_eq!((input.cursors[0], &input.mode), (p(3, 4), &ui::Mode::Field));
    // Cross, two steps and Cross move the unit; the path excludes the origin.
    let (after, r) = drive(&d, &mut input, &[CROSS, UP, LEFT, CROSS], &c);
    assert_eq!(
        r[3],
        ui::Response::Commands(vec![DuelCommand::Move {
            unit: UnitId(10),
            path: vec![p(3, 3), p(2, 3)],
            face_up: false,
            defence: None,
        }])
    );
    assert_eq!(after.unit(UnitId(10)).unwrap().position, p(2, 3));
}

#[test]
fn pad_input_refuses_what_the_original_refuses() {
    use ai::pad::*;
    let c = cards();
    let mut d = board(vec![
        placed(10, 2, 0, 1, p(3, 5), false, false),
        placed(11, 4, 1, 1, p(3, 3), false, false),
        placed(12, 4, 0, 2, p(4, 5), false, false),
    ]);
    d.units
        .iter_mut()
        .for_each(|u| u.acted = u.id == UnitId(12));
    let mut input = ui::DuelUi::new(&d);
    let at = |input: &mut ui::DuelUi, cell| {
        input.cursors[0] = cell;
    };
    at(&mut input, p(0, 0));
    assert_eq!(
        drive(&d, &mut input, &[CROSS], &c).1,
        [ui::Response::Refused]
    );
    at(&mut input, p(3, 3));
    assert_eq!(
        drive(&d, &mut input, &[CROSS], &c).1,
        [ui::Response::Refused]
    );
    at(&mut input, p(4, 5));
    assert_eq!(
        drive(&d, &mut input, &[CROSS], &c).1,
        [ui::Response::Refused]
    );
    // Presses past the edge are dropped.
    at(&mut input, p(0, 0));
    assert_eq!(
        drive(&d, &mut input, &[LEFT], &c).1,
        [ui::Response::Ignored]
    );
    // R1 before stepping sets defence; a step takes attack position again.
    at(&mut input, p(3, 5));
    let (_, r) = drive(&d, &mut input, &[CROSS, R1], &c);
    assert_eq!(r[1], ui::Response::Moved);
    let (_, r) = drive(&d, &mut input, &[UP, DOWN, CROSS], &c);
    assert_eq!(
        r[2],
        ui::Response::Commands(vec![DuelCommand::Move {
            unit: UnitId(10),
            path: vec![],
            face_up: false,
            defence: None,
        }])
    );
    // Start ends the turn; Select opens the surrender prompt (0x25,
    // `script/lesson1/tests.rs` drives it).
    assert_eq!(
        drive(&d, &mut input, &[START], &c).1,
        [ui::Response::Commands(vec![DuelCommand::EndTurn])]
    );
    assert_eq!(
        drive(&d, &mut input, &[SELECT], &c).1,
        [ui::Response::Moved]
    );
    assert_eq!(input.state(), 0x25);
}

/// `cards()` plus a moth line with the PAL entry shapes (invented ids):
/// 30 Larva (0 steps; flip starts a 5-turn countdown to 31; destroyed in
/// battle it leaves 33 beside its leader), 31 Pupa (1 turn to 32), 32 a
/// Great Moth weakening enemies by 100, 33 a plain monster.
fn moth_cards() -> Vec<CardInfo> {
    let mut v = cards();
    while v.len() < 34 {
        let id = v.len() as u16;
        v.push(monster(id, kind::INSECT, 3, 0, 500, 400));
    }
    let moth = |id: u16, attack: u16, defense: u16, entries: [Option<[u32; 2]>; 5]| CardInfo {
        entries,
        effects: entries.map(|e| e.is_some()),
        ..monster(id, kind::INSECT, 2, 0, attack, defense)
    };
    v[30] = moth(
        30,
        0,
        2000,
        [
            None,
            Some([0x4F5B, 0]),
            Some([0xC5B, 0x5_001F]),
            Some([0xC5B, 0x5_001F]),
            Some([0x385C, 0x1F_0021]),
        ],
    );
    v[31] = moth(
        31,
        0,
        2000,
        [
            None,
            Some([0x4F5B, 0]),
            Some([0xC5B, 0x1_0020]),
            Some([0xC5B, 0x1_0020]),
            None,
        ],
    );
    v[32] = moth(
        32,
        3500,
        3000,
        [None, None, Some([0x4D9D, 0x64]), None, None],
    );
    v
}

#[test]
fn moth_flip_countdown_and_evolution_follow_the_traced_turn_start() {
    let c = moth_cards();
    let apply = |d: &Duel, side: u8, cmd: DuelCommand| d.apply(side, &cmd, &c).unwrap().0;
    let mut d = board(vec![placed(10, 30, 0, 1, p(3, 4), false, false)]);
    d.units.iter_mut().for_each(|u| u.acted = false);
    // Zero steps (entry 1 type 0x4F), but it may flip in place.
    assert!(d
        .apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(10),
                path: vec![p(3, 3)],
                face_up: false,
                defence: None,
            },
            &c
        )
        .is_err());
    let d = apply(
        &d,
        0,
        DuelCommand::Move {
            unit: UnitId(10),
            path: vec![],
            face_up: true,
            defence: Some(true),
        },
    );
    let larva = d.unit(UnitId(10)).unwrap();
    assert_eq!((larva.evo_counter, larva.committed_defence), (6, true));
    // 6 -> 5 -> 4 -> 3 -> 2 -> 1 on its own turn starts; at 1 it evolves.
    let mut d = d;
    for turn in 1..=5 {
        d = apply(&d, 0, DuelCommand::EndTurn);
        d = apply(&d, 1, DuelCommand::EndTurn);
        let here = d.unit_at(p(3, 4)).unwrap();
        if turn < 5 {
            assert_eq!((here.card, here.evo_counter), (30, 6 - turn), "turn {turn}");
        } else {
            assert_eq!(here.card, 31);
            assert!(!here.face_up && !here.defence && !here.acted);
            assert_eq!(here.evo_counter, 0);
        }
    }
    assert_eq!(d.sides[0].graveyard, [30]);
}

#[test]
fn a_larva_destroyed_in_battle_leaves_a_monster_beside_its_leader() {
    let c = moth_cards();
    // Weevil's side attacks the player's Larva (attack position, ATK 0).
    let mut d = board(vec![
        placed(10, 30, 0, 1, p(3, 5), false, true),
        placed(11, 9, 1, 1, p(3, 4), false, true),
    ]);
    d.active = 1;
    d.units.iter_mut().for_each(|u| u.acted = u.side == 0);
    let (d, _) = d
        .apply(
            1,
            &DuelCommand::Move {
                unit: UnitId(11),
                path: vec![p(3, 5)],
                face_up: false,
                defence: None,
            },
            &c,
        )
        .unwrap();
    // The dying Larva still held (3,5) and slot 1 when the event ran, so the
    // next neighbour of the leader at (3,6) in 0x24FD30 order is (4,6) and
    // the new unit takes slot 2 (MOTH-EFFECTS.md section 3).
    let new = d.unit_at(p(4, 6)).unwrap();
    assert_eq!(
        (new.card, new.side, new.slot, new.face_up),
        (33, 0, 2, false)
    );
    assert_eq!(d.sides[0].graveyard, [30]);
    assert_eq!(d.unit_at(p(3, 5)).unwrap().id, UnitId(11));
}

#[test]
fn a_great_moth_in_defence_weakens_enemy_monsters_each_own_turn() {
    let c = moth_cards();
    let mut d = board(vec![
        placed(10, 32, 0, 1, p(3, 4), true, true),
        placed(11, 9, 1, 1, p(3, 1), false, false),
        placed(12, 30, 1, 2, p(2, 1), false, false),
    ]);
    d.active = 1;
    let d = d.apply(1, &DuelCommand::EndTurn, &c).unwrap().0;
    let bonus = |id| {
        let u = d.unit(UnitId(id)).unwrap();
        (u.attack_bonus, u.defense_bonus)
    };
    assert_eq!(bonus(11), (-100, -100));
    // Not below zero: an ATK 0 monster keeps ATK 0.
    assert_eq!(bonus(12), (0, -100));
    assert_eq!(bonus(10), (0, 0));
}

/// `moth_cards()` plus a ritual (id 34) creating card 35 from card 5 and
/// two Insects, as a catalogue with that requirement row.
fn ritual_catalogue() -> Catalogue {
    let mut v = moth_cards();
    v.push(CardInfo {
        kind: 160,
        entries: [Some([0x325C, 35]), None, None, None, None],
        ..monster(34, 0, 0, 0, 0, 0)
    });
    v.push(monster(35, kind::INSECT, 7, 0, 2450, 2550));
    let words = [
        0x0005_001C,
        u32::from(kind::INSECT),
        u32::from(kind::INSECT),
    ];
    Catalogue::new(v, []).with_rituals([(35, words)])
}

#[test]
fn a_revealed_ritual_sacrifices_the_cheapest_matching_units() {
    let c = ritual_catalogue();
    let reveal = |d: &Duel| {
        d.apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(20),
                path: vec![],
                face_up: true,
                defence: None,
            },
            &c,
        )
        .unwrap()
        .0
    };
    let mut units = vec![
        placed(10, 5, 0, 1, p(1, 5), false, false),
        placed(11, 25, 0, 2, p(2, 5), false, false),
        placed(12, 26, 0, 3, p(4, 5), false, false),
        placed(20, 34, 0, 6, p(3, 4), false, false),
    ];
    // A third Insect with a higher ATK + DEF score is kept.
    let mut strong = placed(13, 27, 0, 4, p(5, 5), false, false);
    strong.attack_bonus = 500;
    units.push(strong);
    let mut d = board(units.clone());
    d.units.iter_mut().for_each(|u| u.acted = false);
    let after = reveal(&d);
    assert_eq!(after.sides[0].graveyard, [34, 5, 25, 26]);
    let result = after.unit_at(p(3, 4)).unwrap();
    assert_eq!(
        (result.card, result.slot, result.face_up, result.acted),
        (35, 1, false, false)
    );
    assert!(after.unit(UnitId(13)).is_some());
    // Without a second Insect only the ritual card is used up.
    units.retain(|u| u.id != UnitId(12) && u.id != UnitId(13));
    let mut d = board(units);
    d.units.iter_mut().for_each(|u| u.acted = false);
    let after = reveal(&d);
    assert_eq!(after.sides[0].graveyard, [34]);
    assert_eq!(after.units.len(), d.units.len() - 1);
    // Plain card data has no requirement table: refused, not guessed.
    let plain = c.cards.clone();
    assert!(matches!(
        d.apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(20),
                path: vec![],
                face_up: true,
                defence: None,
            },
            &plain,
        ),
        Err(DuelError::Unsupported(_))
    ));
}

/// Fusion/equip tables with no fusions and no fitting equips.
pub(super) struct NoTables;
impl ai::weevil::FusionData for NoTables {
    fn fuse(&self, _a: u16, _b: u16) -> Option<u16> {
        None
    }
    fn equip(&self, _monster: u16, _card: u16) -> i32 {
        -1
    }
    fn category(&self, _card: u16) -> u8 {
        0
    }
    fn value(&self, _card: u16) -> u16 {
        0
    }
}

#[test]
fn a_session_hands_the_turn_to_the_computer_seat_and_back() {
    use session::{ComputerSeat, DuelSession};
    // An empty fusion table: pairs do not fuse (the older card is dropped).
    let c = Catalogue::new(cards(), []);
    let (d, _) = Duel::new(setup([vec![2; 40], vec![3; 40]]), &c).unwrap();
    let seat = ComputerSeat::new(&d, 1, 0);
    let mut s = DuelSession::new(d, [None, Some(seat)]);
    // The human's Start ends the turn.
    let f = s.frame(ai::pad::START, &c, &NoTables).unwrap();
    assert!(f
        .events
        .iter()
        .any(|e| matches!(e, DuelEvent::TurnEnded { side: 0 })));
    assert!(s.computer_to_act());
    // The computer plays its turn through key presses and ends it.
    let mut frames = 0;
    while s.duel.active == 1 && frames < 5000 {
        s.frame(0, &c, &NoTables).unwrap();
        frames += 1;
    }
    assert_eq!(s.duel.active, 0);
    assert!(s.duel.units.iter().any(|u| u.side == 1 && u.slot != 0));
    // Human input is ignored while nobody asks for it.
    assert_eq!(
        s.frame(0, &c, &NoTables).unwrap(),
        session::Frame::default()
    );
}

#[test]
fn attacking_a_card_listed_before_the_attacker_moves_the_right_unit() {
    // The attacked set card sits earlier in the unit list than the attacker,
    // so discarding it shifts the attacker's index (a panic before the fix).
    let c = cards();
    let mut d = board(vec![
        placed(10, 19, 1, 6, p(3, 3), false, false),
        placed(11, 2, 0, 1, p(3, 4), false, false),
    ]);
    d.units.iter_mut().for_each(|u| u.acted = u.side == 1);
    let (d, _) = d
        .apply(
            0,
            &DuelCommand::Move {
                unit: UnitId(11),
                path: vec![p(3, 3)],
                face_up: false,
                defence: None,
            },
            &c,
        )
        .unwrap();
    assert!(d.unit(UnitId(10)).is_none());
    assert_eq!(d.unit(UnitId(11)).unwrap().position, p(3, 3));
    assert_eq!(d.sides[1].graveyard, [19]);
}

#[test]
fn a_held_diagonal_alternates_until_state_0x16_like_the_original() {
    use ai::pad::*;
    let c = cards();
    let d = board(vec![]);
    let mut input = ui::DuelUi::new(&d);
    input.cursors[0] = p(3, 4);
    // Horizontal first, and the flag survives a press without d-pad bits.
    drive(&d, &mut input, &[UP | LEFT, CIRCLE, UP | LEFT], &c);
    assert_eq!(input.cursors[0], p(2, 3));
    // A turn end passes 0x16: the flag is cleared again.
    input.cursors[0] = p(3, 4);
    drive(&d, &mut input, &[UP | LEFT], &c);
    let _ = input.press(&d, &c, START).unwrap();
    drive(&d, &mut input, &[UP | LEFT], &c);
    assert_eq!(input.cursors[0], p(1, 4));
}

/// Millennium Shield's entry-2 type 0x22: +value to the DEF of its
/// controller's monsters (itself included) while it is face-up committed
/// defence; ATK unchanged [F/emu E2-0x22.md].
#[test]
fn millennium_shield_raises_def_only() {
    let mut c = effect_cards();
    let mut shield = monster(23, kind::SPELLCASTER, 2, 0, 0, 3000);
    shield.entries[2] = Some([0x225D, 0x3E8]);
    shield.effects[2] = true;
    while c.len() <= 23 {
        let id = c.len() as u16;
        c.push(monster(id, kind::MACHINE, 1, 0, 100, 100));
    }
    c[23] = shield;
    c[22] = monster(22, kind::FAIRY, 2, 0, 1000, 1200);
    let units = |shield_defence: bool, shield_face_up: bool| {
        vec![
            placed(30, 23, 1, 1, p(0, 1), shield_defence, shield_face_up),
            placed(31, 22, 1, 2, p(1, 1), true, true),
            placed(32, 22, 0, 1, p(1, 5), true, true),
        ]
    };
    let d = ready(units(true, true));
    let eff = |d: &Duel, id, attack| {
        let u = d.unit(UnitId(id)).unwrap();
        d.effective(u, u.position, attack, &c)
    };
    assert_eq!((eff(&d, 31, false), eff(&d, 31, true)), (2200, 1000));
    assert_eq!((eff(&d, 30, false), eff(&d, 30, true)), (4000, 0));
    assert_eq!(eff(&d, 32, false), 1200, "the other side is not boosted");
    for (defence, face_up) in [(false, true), (true, false)] {
        let d = ready(units(defence, face_up));
        assert_eq!(eff(&d, 31, false), 1200);
    }
    // Two Shields stack.
    let mut two = units(true, true);
    two.push(placed(33, 23, 1, 3, p(2, 1), true, true));
    assert_eq!(eff(&ready(two), 31, false), 3200);
    // A duel with a face-up committed-defence Shield is not refused.
    let d = ready(units(true, true));
    assert!(d.apply(0, &DuelCommand::EndTurn, &c).is_ok());
}

#[test]
fn custom_rules_set_the_starting_lp_and_the_sp_recharge_rate() {
    // Custom Duel's LP Setting / SP Recharge Rate (side +0x33A / +0x342):
    // the first turn keeps its 4 stars, later turns add the side's rate.
    let mut duel = Duel::new(setup([vec![2; 40], vec![3; 40]]), &cards()).unwrap().0;
    duel.set_custom_rules([100, 9999], [1, 12]);
    assert_eq!((duel.sides[0].lp, duel.sides[1].lp), (100, 9999));
    assert_eq!(duel.star_income, [1, 12]);
    let back: Duel = serde_json::from_str(&serde_json::to_string(&duel).unwrap()).unwrap();
    assert_eq!(back.star_income, [1, 12]);
}

/// Seeded computer-vs-computer duels (both sides the ported controller)
/// played to the end: a FNV-1a hash of each final state. The same values
/// hold with and without the `online` feature (`cargo test -p roses-core`
/// and `--no-default-features`), so the port additions compiled in for
/// online play change nothing in a 1v1 offline duel (docs/ADDITIONS.md).
pub(super) fn offline_duel_hashes() -> Vec<(u64, u64)> {
    use session::{ComputerSeat, DuelSession};
    let fnv = |bytes: &[u8]| bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
    let c = Catalogue::new(cards(), []);
    let pool = [2u16, 3, 4, 5, 6, 7, 9, 10];
    (1..=6u64)
        .map(|seed| {
            let mut state = seed;
            let mut next = || {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            };
            let mut deck = || (0..40).map(|_| pool[(next() % pool.len() as u64) as usize]).collect::<Vec<u16>>();
            let decks = [deck(), deck()];
            let mut s = setup(decks);
            s.first = (seed & 1) as u8;
            let (d, _) = Duel::new(s, &c).unwrap();
            let seats = [Some(ComputerSeat::new(&d, 0, 0)), Some(ComputerSeat::new(&d, 1, 0))];
            let mut session = DuelSession::new(d, seats);
            let result = session.run(60_000, &c, &NoTables);
            // Every duel is played out to its end (no refusal).
            assert!(result.is_ok() && session.duel.outcome.is_some(), "seed {seed}: {result:?}, round {}", session.duel.round);
            let mut bytes = serde_json::to_vec(&session.duel).unwrap();
            bytes.extend(format!("{:?}", result.map(|e| e.len())).bytes());
            (seed, fnv(&bytes))
        })
        .collect()
}

#[test]
fn offline_computer_duels_are_the_same_with_and_without_the_online_feature() {
    let hashes = offline_duel_hashes();
    for (seed, hash) in &hashes {
        eprintln!("offline duel seed {seed}: {hash:#018x}");
    }
    let expected: [(u64, u64); 6] = OFFLINE_DUEL_HASHES;
    assert_eq!(hashes, expected.to_vec());
}

/// Recorded once (both builds give these).
const OFFLINE_DUEL_HASHES: [(u64, u64); 6] = [
    (1, 0xe722e0c8680e593a),
    (2, 0x88e1a4c6df29dc88),
    (3, 0x474d3d7500d3b767),
    (4, 0xf92b8158bf6ad5a6),
    (5, 0x3b4fab20b4d57e21),
    (6, 0x0a675bb6d46272be),
];
