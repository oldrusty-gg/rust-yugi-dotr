//! Experience bookkeeping (`duel/experience.rs`) with invented cards.
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

const POWER_UP: u16 = 0x2F0;

/// Synthetic catalogue (invented numbers, entry words shaped like the PAL
/// ones): all earth Warriors unless noted, so no attribute or terrain bonus.
fn catalogue() -> Catalogue {
    let mut v: Vec<CardInfo> = (0..=POWER_UP)
        .map(|i| monster(i, kind::MACHINE, 1, 3, 100, 100))
        .collect();
    let warrior = |id: u16, level: u8, attack: u16, defense: u16| {
        let mut c = monster(id, kind::WARRIOR, level, 3, attack, defense);
        c.equip_bits[0] = 1; // the power-up fits every Warrior
        c
    };
    v[0] = monster(0, kind::DRAGON, 8, 0, 3000, 2500); // leader of side 1
    v[1] = monster(1, kind::SPELLCASTER, 7, 1, 2500, 2100); // leader of side 0
    v[2] = warrior(2, 4, 1400, 1200);
    v[3] = warrior(3, 6, 1400, 1200);
    v[4] = warrior(4, 5, 1000, 900);
    v[5] = warrior(5, 14, 2000, 2000); // level above 12: cost 12
    v[6] = warrior(6, 3, 500, 2500);
    v[7] = warrior(7, 7, 2200, 2000); // fusion result of 2 + 4
    v[8] = warrior(8, 2, 1200, 1200);
    v[9] = monster(9, kind::MACHINE, 4, 3, 1800, 1000);
    // Man-Eater Bug (flip type 4).
    v[10] = CardInfo {
        effects: [false, false, false, true, false],
        entries: [None, None, None, Some([0x049A, 0]), None],
        ..monster(10, kind::INSECT, 2, 3, 450, 600)
    };
    let card = |id: u16, kind: u8, kind_index: u8, word0: u32, word1: u32| CardInfo {
        kind,
        kind_index,
        entries: [Some([word0, word1]), None, None, None, None],
        ..monster(id, 0, 0, 0, 0, 0)
    };
    v[12] = card(12, 128, 1, 0x30BD, 0); // Negate Attack
    v[13] = card(13, 128, 0, 0x2CBD, 0); // Block Attack
    v[15] = card(15, 96, 8, 1, 0); // Bear Trap (ATK <= 2000)
                                   // +1000 LP magic.
    v[19] = card(19, 32, 1, 0xD7D, 0x3E8);
    // A larva turning into 33 at counter 1 (entry 2 type 0xC).
    v[20] = CardInfo {
        effects: [false, false, true, false, false],
        entries: [None, None, Some([0xC5B, 0x1_0021]), None, None],
        ..monster(20, kind::INSECT, 2, 3, 0, 2000)
    };
    v[21] = card(21, 160, 0, 0x325C, 22); // ritual creating 22
    v[22] = monster(22, kind::INSECT, 7, 3, 2450, 2550);
    for id in 25..=27u16 {
        v[usize::from(id)] = monster(id, kind::INSECT, 3, 3, 500, 400);
    }
    v[33] = monster(33, kind::INSECT, 4, 3, 1000, 1000);
    v[usize::from(POWER_UP)] = CardInfo {
        power_up: Some(PowerUp {
            category: 0,
            value: 500,
        }),
        ..card(POWER_UP, 64, 0, 0, 0)
    };
    let ritual = [
        0x0019_001C,
        u32::from(kind::INSECT),
        u32::from(kind::INSECT),
    ];
    Catalogue::new(v, [[2, 4, 7]]).with_rituals([(22, ritual)])
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

/// Leaders 1 (side 0, at (3,6)) and 0 (side 1, at (3,0)), side 0 to act.
fn board(units: Vec<DuelUnit>) -> Duel {
    let setup = DuelSetup {
        terrain: vec![terrain::NORMAL; 49],
        leaders: [1, 0],
        decks: [vec![], vec![]],
        first: 0,
        leader_abilities: [vec![], vec![]],
        controllers: [PAD_CONTROLLER; 2],
    };
    let (mut d, _) = Duel::new(setup, &catalogue()).unwrap();
    d.units.extend(units);
    d.next_unit = 100;
    d
}

fn go(d: &Duel, unit: u16, path: Vec<GridPos>, face_up: bool) -> Duel {
    d.apply(
        d.active,
        &DuelCommand::Move {
            unit: UnitId(unit),
            path,
            face_up,
            defence: None,
        },
        &catalogue(),
    )
    .unwrap()
    .0
}

/// The recorded deltas (every recorded change here starts far from the
/// clamps, so `delta` is the whole story).
fn deltas(d: &Duel) -> [Vec<(u16, i32)>; 2] {
    let e = d.experience.clone();
    [e[0].clone(), e[1].clone()].map(|m| m.into_iter().map(|(c, e)| (c, e.delta)).collect())
}

fn only(side0: &[(u16, i32)], side1: &[(u16, i32)]) -> [Vec<(u16, i32)>; 2] {
    [side0.to_vec(), side1.to_vec()]
}

#[test]
fn clamped_additions_compose_exactly() {
    // 0x2152A0 clamps after every call: 0 - 4 + 20 is 20, not 16.
    let mut e = ExperienceChange::default();
    e.add(-4);
    e.add(20);
    assert_eq!((e.apply(0), e.apply(2), e.apply(100)), (20, 20, 116));
    let mut top = ExperienceChange::default();
    top.add(20);
    top.add(-8);
    assert_eq!(
        (top.apply(65535), top.apply(65530), top.apply(0)),
        (65527, 65527, 12)
    );
    // Against a direct simulation for many sequences and start values.
    let mut seed = 0x1234_5678_u64;
    for _ in 0..200 {
        let mut change = ExperienceChange::default();
        let mut values = Vec::new();
        for _ in 0..(next_rand31(&mut seed) % 12) {
            let v = (next_rand31(&mut seed) % 81) as i32 - 40;
            let v = if next_rand31(&mut seed).is_multiple_of(8) {
                v * 1000
            } else {
                v
            };
            change.add(v);
            values.push(v);
        }
        for start in [0u16, 1, 3, 4, 7, 39, 500, 32768, 65500, 65531, 65535] {
            let direct = values
                .iter()
                .fold(i32::from(start), |acc, v| (acc + v).clamp(0, 65535));
            assert_eq!(
                i32::from(change.apply(start)),
                direct,
                "{values:?} from {start}"
            );
        }
    }
}

#[test]
fn only_the_cards_0x2152a0_accepts_are_recorded() {
    let mut d = board(vec![]);
    d.record_experience(0, REJECTED_DECK_CARD, 5);
    d.record_experience(0, CARD_COUNT, 5);
    d.record_experience(2, 2, 5);
    d.record_experience(1, CARD_COUNT - 1, 5);
    assert_eq!(deltas(&d), only(&[], &[(CARD_COUNT - 1, 5)]));
}

/// Side 0's unit 10 (card `att`) at (3,4) attacks side 1's unit 11 (card
/// `def`) at (3,3).
fn battle(att: u16, def: u16, defence: bool) -> Duel {
    let d = board(vec![
        placed(10, att, 0, 1, p(3, 4), false, true),
        placed(11, def, 1, 1, p(3, 3), defence, true),
    ]);
    go(&d, 10, vec![p(3, 3)], false)
}

#[test]
fn battle_results_follow_0x248130() {
    // ATK 1400 beats 1000 (level 5) and deals damage: 4 x 5; the destroyed
    // unit's card gets nothing (its step-11 call reads a removed unit).
    let d = battle(2, 4, false);
    assert!(d.unit(UnitId(11)).is_none());
    assert_eq!(deltas(&d), only(&[(2, 20)], &[]));
    // The defender wins: 4 x cost(attacker).
    let d = battle(4, 2, false);
    assert!(d.unit(UnitId(10)).is_none());
    assert_eq!(deltas(&d), only(&[], &[(2, 20)]));
    // Equal ATK: each gets 2 x (cost(other) + 2).
    let d = battle(2, 3, false);
    assert_eq!(deltas(&d), only(&[(2, 16)], &[(3, 12)]));
    // DEF broken: 2 x (cost(defender) + 2), whatever the difference.
    let d = battle(2, 4, true);
    assert_eq!(deltas(&d), only(&[(2, 14)], &[]));
    // DEF held (greater or equal): nothing, even with damage to the attacker.
    let d = battle(2, 6, true);
    assert_eq!(d.sides[0].lp, 4000 - 1100);
    assert_eq!(deltas(&d), only(&[], &[]));
    assert_eq!(deltas(&battle(8, 2, true)), only(&[], &[]));
    // The cost is clamped to 12.
    assert_eq!(deltas(&battle(7, 5, false)), only(&[(7, 48)], &[]));
}

#[test]
fn leader_hits_give_20_or_24_and_cost_the_leader_8() {
    let d = board(vec![placed(10, 2, 0, 1, p(3, 1), false, true)]);
    let hit = go(&d, 10, vec![p(3, 0)], false);
    assert_eq!(hit.sides[1].lp, 2600);
    assert_eq!(deltas(&hit), only(&[(2, 20)], &[(0, -8)]));
    let mut low = d.clone();
    low.sides[1].lp = 1000;
    let hit = go(&low, 10, vec![p(3, 0)], false);
    assert!(hit.outcome.is_some());
    assert_eq!(deltas(&hit), only(&[(2, 24)], &[(0, -8)]));
}

#[test]
fn attacking_a_set_card_gives_6_and_the_card_loses_4() {
    let attack = |att: u16, target: u16| {
        let d = board(vec![
            placed(10, att, 0, 1, p(3, 4), false, true),
            placed(11, target, 1, 6, p(3, 3), false, false),
        ]);
        go(&d, 10, vec![p(3, 3)], false)
    };
    let d = attack(2, 19);
    assert_eq!(d.unit(UnitId(10)).unwrap().position, p(3, 3));
    assert_eq!(deltas(&d), only(&[(2, 6)], &[(19, -4)]));
    // A Bear Trap target fires first (0x24CC30): trap and attacker are both
    // removed by 0x250C90 and there is no attack.
    let d = attack(2, 15);
    assert!(d.unit(UnitId(10)).is_none());
    assert_eq!(deltas(&d), only(&[(2, -4)], &[(15, -4)]));
    // Too strong for the trap: an ordinary attack on a set card.
    assert_eq!(deltas(&attack(7, 15)), only(&[(7, 6)], &[(15, -4)]));
}

#[test]
fn traps_flip_effects_crush_and_magic_remove_through_0x250c90() {
    // Negate Attack: the trap is used up; no battle, no battle experience.
    let d = board(vec![
        placed(10, 2, 0, 1, p(3, 4), false, true),
        placed(11, 4, 1, 1, p(3, 3), false, true),
        placed(12, 12, 1, 6, p(0, 1), false, false),
    ]);
    assert_eq!(
        deltas(&go(&d, 10, vec![p(3, 3)], false)),
        only(&[], &[(12, -4)])
    );
    // Block Attack on the confirm press.
    let d = board(vec![
        placed(10, 2, 0, 1, p(3, 4), false, true),
        placed(12, 13, 1, 6, p(0, 1), false, false),
    ]);
    let d = go(&d, 10, vec![p(3, 3)], false);
    assert_eq!(d.unit(UnitId(10)).unwrap().position, p(3, 4));
    assert_eq!(deltas(&d), only(&[], &[(13, -4)]));
    // Man-Eater Bug revealed as defender destroys the attacker.
    let d = board(vec![
        placed(10, 2, 0, 1, p(3, 4), false, true),
        placed(11, 10, 1, 1, p(3, 3), false, false),
    ]);
    let d = go(&d, 10, vec![p(3, 3)], false);
    assert!(d.unit(UnitId(10)).is_none());
    assert_eq!(deltas(&d), only(&[(2, -4)], &[]));
    // Crush terrain (event 0x70).
    let mut d = board(vec![placed(10, 9, 0, 1, p(3, 4), false, true)]);
    d.terrain[p(3, 3).index().unwrap()] = terrain::CRUSH;
    let d = go(&d, 10, vec![p(3, 3)], false);
    assert!(d.unit(UnitId(10)).is_none());
    assert_eq!(deltas(&d), only(&[(9, -4)], &[]));
    // A magic card is removed before its effect.
    let d = board(vec![placed(10, 19, 0, 6, p(3, 4), false, false)]);
    let d = go(&d, 10, vec![], true);
    assert_eq!(d.sides[0].lp, 5000);
    assert_eq!(deltas(&d), only(&[(19, -4)], &[]));
}

#[test]
fn moves_onto_own_units_charge_the_removed_card() {
    let own = |mover_card: u16, target_card: u16, target_slot: u8| {
        board(vec![
            placed(10, mover_card, 0, 1, p(3, 4), false, false),
            placed(11, target_card, 0, target_slot, p(3, 3), false, false),
        ])
    };
    // Fusion (event 0x64): the target -4; the mover's card goes to the
    // graveyard without experience.
    let d = go(&own(2, 4, 2), 10, vec![p(3, 3)], false);
    assert_eq!(d.unit(UnitId(10)).unwrap().card, 7);
    assert_eq!(deltas(&d), only(&[(4, -4)], &[]));
    // No fusion: the target is destroyed.
    assert_eq!(
        deltas(&go(&own(2, 8, 2), 10, vec![p(3, 3)], false)),
        only(&[(8, -4)], &[])
    );
    // A fitting power-up is used up (event 0x24).
    let d = go(&own(2, POWER_UP, 6), 10, vec![p(3, 3)], false);
    assert_eq!(d.unit(UnitId(10)).unwrap().attack_bonus, 500);
    assert_eq!(deltas(&d), only(&[(POWER_UP, -4)], &[]));
    // The leader moving onto an own monster (event 0x5F).
    let d = board(vec![placed(11, 8, 0, 1, p(3, 5), false, false)]);
    let d = go(&d, 0, vec![p(3, 5)], false);
    assert_eq!(deltas(&d), only(&[(8, -4)], &[]));
}

#[test]
fn transformations_and_rituals_charge_the_removed_cards() {
    // A larva at counter 2 in face-up defence transforms at its side's next
    // turn start (0x26ABF0).
    let mut larva = placed(10, 20, 0, 1, p(3, 4), true, true);
    larva.evo_counter = 2;
    let mut d = board(vec![larva]);
    d.active = 1;
    let (d, _) = d.apply(1, &DuelCommand::EndTurn, &catalogue()).unwrap();
    assert_eq!(d.unit_at(p(3, 4)).unwrap().card, 33);
    assert_eq!(deltas(&d), only(&[(20, -4)], &[]));
    // A revealed ritual: the ritual card, then its three sacrifices.
    let d = board(vec![
        placed(10, 25, 0, 1, p(1, 5), false, false),
        placed(11, 26, 0, 2, p(2, 5), false, false),
        placed(12, 27, 0, 3, p(4, 5), false, false),
        placed(20, 21, 0, 6, p(3, 4), false, false),
    ]);
    let d = go(&d, 20, vec![], true);
    assert_eq!(d.unit_at(p(3, 4)).unwrap().card, 22);
    assert_eq!(
        deltas(&d),
        only(&[(21, -4), (25, -4), (26, -4), (27, -4)], &[])
    );
}

#[test]
fn summon_chain_cards_cost_4_on_the_active_side() {
    let c = catalogue();
    let chain = |d: &Duel, cell: GridPos, hand: Vec<usize>| {
        let side = d.active;
        let (d, _) = d
            .apply(side, &DuelCommand::OpenSummon { cell }, &c)
            .unwrap();
        d.apply(side, &DuelCommand::ChooseSummon { hand }, &c)
            .unwrap()
            .0
    };
    let setup = DuelSetup {
        terrain: vec![terrain::NORMAL; 49],
        leaders: [1, 0],
        decks: [vec![2, 4, 8, POWER_UP], vec![2, 4]],
        first: 0,
        leader_abilities: [vec![], vec![]],
        controllers: [PAD_CONTROLLER; 2],
    };
    let (mut d, _) = Duel::new(setup, &c).unwrap();
    d.sides[0].stars = 12;
    // 2 + 4 fuse (0x271960): both materials -4.
    let fused = chain(&d, p(3, 5), vec![0, 1]);
    assert_eq!(fused.unit_at(p(3, 5)).unwrap().card, 7);
    assert_eq!(deltas(&fused), only(&[(2, -4), (4, -4)], &[]));
    // No fusion (0x2717A0): the older card is dropped.
    assert_eq!(
        deltas(&chain(&d, p(3, 5), vec![0, 2])),
        only(&[(2, -4)], &[])
    );
    // Equip (0x271B90): the used power-up.
    assert_eq!(
        deltas(&chain(&d, p(3, 5), vec![0, 3])),
        only(&[(POWER_UP, -4)], &[])
    );
    // A lifted field unit that fuses counts like a hand card.
    let mut lifted = d.clone();
    lifted.units.push(placed(10, 4, 0, 1, p(3, 5), false, true));
    assert_eq!(
        deltas(&chain(&lifted, p(3, 5), vec![0])),
        only(&[(2, -4), (4, -4)], &[])
    );
    // No free monster slot: the summoned card is dropped.
    let mut full = d.clone();
    for slot in 1..=5u8 {
        full.units.push(placed(
            10 + u16::from(slot),
            8,
            0,
            slot,
            p(slot, 3),
            false,
            false,
        ));
    }
    assert_eq!(
        deltas(&chain(&full, p(3, 5), vec![0])),
        only(&[(2, -4)], &[])
    );
    // The active side's block is charged.
    let (mut other, _) = d.apply(0, &DuelCommand::EndTurn, &c).unwrap();
    other.sides[1].stars = 12;
    assert_eq!(
        deltas(&chain(&other, p(3, 1), vec![0, 1])),
        only(&[], &[(2, -4), (4, -4)])
    );
}

#[test]
fn ending_a_turn_gives_the_monster_slots_4_face_up_or_2() {
    let d = board(vec![
        placed(10, 2, 0, 1, p(0, 4), false, true),
        placed(11, 4, 0, 2, p(1, 4), true, false),
        placed(12, 2, 0, 3, p(2, 4), true, true),
        placed(13, 19, 0, 6, p(5, 5), false, false),
        placed(14, 3, 1, 1, p(5, 1), false, true),
    ]);
    let (d, _) = d.apply(0, &DuelCommand::EndTurn, &catalogue()).unwrap();
    // Two units with the same card both count; leaders and cards do not.
    assert_eq!(deltas(&d), only(&[(2, 8), (4, 2)], &[]));
}

fn award(d: &Duel, winner: Option<u8>, reason: EndReason) -> Vec<(u8, u16, i32)> {
    d.end_award(Outcome { winner, reason })
        .into_iter()
        .map(|a| (a.side, a.card, a.value))
        .collect()
}

#[test]
fn the_duel_end_award_follows_0x2745f0() {
    let mut d = board(vec![
        placed(10, 2, 0, 1, p(0, 4), false, true),
        placed(11, 4, 0, 3, p(1, 4), false, false),
        placed(12, 3, 1, 2, p(5, 1), false, false),
        placed(13, 19, 0, 6, p(5, 5), false, false),
    ]);
    d.sides[0].graveyard = vec![2; 13];
    d.sides[1].graveyard = vec![3; 4];
    // The first campaign duel's shape: winner's graveyard 13 -> +24, the
    // loser's 4 -> -32.
    assert_eq!(
        award(&d, Some(0), EndReason::LifePoints),
        [(0, 2, 6), (0, 4, 4), (0, 1, 24), (1, 0, -32)]
    );
    assert_eq!(
        award(&d, Some(0), EndReason::Surrender),
        award(&d, Some(0), EndReason::LifePoints)
    );
    // Reason rows: turn limit, Exodia, leader trapped.
    assert_eq!(
        award(&d, Some(1), EndReason::TurnLimit),
        [(1, 3, 4), (1, 0, 32), (0, 1, -12)]
    );
    assert_eq!(
        award(&d, Some(1), EndReason::Exodia),
        [(1, 3, 4), (1, 0, 44), (0, 1, -20)]
    );
    assert_eq!(
        award(&d, Some(1), EndReason::LeaderTrapped),
        [(1, 3, 4), (1, 0, 48), (0, 1, -24)]
    );
    // Brackets: < 5, < 10, < 15, else.
    for (count, value) in [
        (0, 40),
        (4, 40),
        (5, 32),
        (9, 32),
        (10, 24),
        (15, 16),
        (120, 16),
    ] {
        d.sides[0].graveyard = vec![2; count];
        assert_eq!(award(&d, Some(0), EndReason::LifePoints)[2], (0, 1, value));
    }
    d.sides[0].graveyard = vec![2; 13];
    // A draw: no winner award, side 0 charged as the loser.
    assert_eq!(award(&d, None, EndReason::LifePoints), [(0, 1, -16)]);
    assert_eq!(award(&d, None, EndReason::TurnLimit), [(0, 1, -12)]);
}

#[test]
fn final_experience_adds_the_award_to_what_play_recorded() {
    let mut d = board(vec![placed(10, 2, 0, 1, p(3, 1), false, true)]);
    d.sides[1].lp = 1000;
    let d = go(&d, 10, vec![p(3, 0)], false);
    assert_eq!(
        d.outcome,
        Some(Outcome {
            winner: Some(0),
            reason: EndReason::LifePoints
        })
    );
    let total = d.final_experience();
    let get = |side: usize, card: u16| total[side][&card];
    // Leader hit +24, then the award: committed face-up slot unit +6.
    assert_eq!(get(0, 2).delta, 30);
    assert_eq!(get(0, 1).delta, 40);
    // Leader hit -8, then the loser's award -32: from 5, 5 - 8 clamps to 0.
    assert_eq!(get(1, 0).delta, -40);
    assert_eq!(get(1, 0).apply(5), 0);
    assert_eq!(get(1, 0).apply(1000), 960);
    // Unfinished duels only carry what play recorded.
    assert_eq!(board(vec![]).final_experience().to_vec(), board(vec![]).experience);
}

#[test]
fn ranks_are_recomputed_from_the_threshold_table() {
    let t: [u16; RANK_COUNT] = [0, 10, 20, 40, 80, 100, 200, 300, 400, 500, 600, 700, 65535];
    assert_eq!(recompute_rank(2, 0, 0, &t), Some(0));
    assert_eq!(recompute_rank(2, 9, 0, &t), Some(0));
    assert_eq!(recompute_rank(2, 10, 0, &t), Some(1));
    assert_eq!(recompute_rank(2, 79, 0, &t), Some(3));
    assert_eq!(recompute_rank(2, 65534, 0, &t), Some(11));
    assert_eq!(recompute_rank(2, 65535, 0, &t), Some(12));
    // A stored rank other than 0 never falls back to 0; demotions otherwise.
    assert_eq!(recompute_rank(2, 0, 3, &t), Some(1));
    assert_eq!(recompute_rank(2, 0, 0x4003, &t), Some(1));
    assert_eq!(recompute_rank(2, 0, 0x4000, &t), Some(0));
    assert_eq!(recompute_rank(2, 25, 5, &t), Some(2));
    // No rank for non-monster ids or card 0x29F (read as 999).
    assert_eq!(recompute_rank(RANKED_CARDS - 1, 50, 0, &t), Some(3));
    assert_eq!(recompute_rank(RANKED_CARDS, 50, 0, &t), None);
    assert_eq!(recompute_rank(REJECTED_DECK_CARD, 50, 0, &t), None);
}

#[test]
fn experience_survives_serialisation_and_old_documents_default_it() {
    let d = battle(2, 4, false);
    let json = serde_json::to_value(&d).unwrap();
    assert_eq!(serde_json::from_value::<Duel>(json.clone()).unwrap(), d);
    let mut old = json;
    old.as_object_mut().unwrap().remove("experience");
    let back: Duel = serde_json::from_value(old).unwrap();
    assert_eq!(
        back.experience,
        <[BTreeMap<u16, ExperienceChange>; 2]>::default()
    );
}
