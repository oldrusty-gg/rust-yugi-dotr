//! N-player duel tests (a port addition, `duel/multi.rs`).

use super::super::tests::{cards, monster, NoTables};
use super::computer::{play_out, project, MultiComputer};
use super::*;
use crate::duel::{
    kind, terrain, Catalogue, CardInfo, DuelCommand, DuelSetup, DuelUnit, PAD_CONTROLLER,
};
use crate::UnitId;

fn p(column: u8, row: u8) -> GridPos {
    GridPos { column, row }
}

fn deck(seed: u64) -> Vec<u16> {
    let pool = [2u16, 3, 4, 5, 6, 7, 9, 10, 20, 21];
    let mut state = seed;
    (0..40)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            pool[((state >> 33) % pool.len() as u64) as usize]
        })
        .collect()
}

fn setup(n: usize, rules: MultiRules, seed: u64) -> MultiSetup {
    MultiSetup {
        rules,
        terrain: vec![terrain::NORMAL; 49],
        leaders: [1u16, 0, 1, 0][..n].to_vec(),
        decks: (0..n as u64).map(|s| deck(seed * 10 + s)).collect(),
        first: (seed % n as u64) as u8,
        leader_abilities: vec![vec![]; n],
        controllers: vec![PAD_CONTROLLER; n],
    }
}

fn ffa() -> MultiRules {
    MultiRules::default()
}

fn teams(shared_lp: bool) -> MultiRules {
    MultiRules {
        mode: MultiMode::Teams,
        shared_lp,
        ..MultiRules::default()
    }
}

fn catalogue() -> Catalogue {
    Catalogue::new(cards(), [])
}

fn unit(id: u16, card: u16, side: u8, slot: u8, at: GridPos, defence: bool) -> DuelUnit {
    DuelUnit {
        id: UnitId(id),
        card,
        side,
        slot,
        position: at,
        face_up: true,
        defence,
        committed_face_up: true,
        committed_defence: defence,
        acted: false,
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

/// Invariants after every command of an N-player duel.
fn check(d: &Duel) {
    let n = d.side_count();
    for u in &d.units {
        assert!(u.side < n && u.controller() < n, "unit {u:?}");
        if d.multi().unwrap().rules.cards_leave {
            assert!(!d.is_out(u.side), "a knocked-out side's unit stayed: {u:?}");
        }
        if u.stolen {
            assert!(d.alive(u.controller()), "a unit is controlled by a knocked-out side");
        }
    }
    for s in 0..n {
        if d.alive(s) {
            assert!(d.leader_cell(s).is_some(), "side {s} plays without a leader");
        }
    }
    if d.outcome.is_none() {
        assert!(d.alive(d.active), "a knocked-out side is acting");
    }
}

/// Play a duel with computers on every side, checking invariants after
/// every applied command.
fn play(d: Duel, max: usize) -> Duel {
    let c = catalogue();
    let mut computers: Vec<MultiComputer> = (0..d.side_count()).map(|s| MultiComputer::new(&d, s)).collect();
    let mut duel = d;
    for _ in 0..max {
        if duel.outcome.is_some() {
            break;
        }
        let before = duel.clone();
        duel = play_out(duel, &mut computers, &c, &NoTables, 1);
        check(&duel);
        if let Some(m) = duel.multi() {
            if m.rules.mode == MultiMode::Teams && m.rules.shared_lp {
                for t in [0u8, 1] {
                    let lps: Vec<u16> = (0..4u8)
                        .filter(|&s| m.teams[usize::from(s)] == t && duel.alive(s))
                        .map(|s| duel.sides[usize::from(s)].lp)
                        .collect();
                    assert!(lps.windows(2).all(|w| w[0] == w[1]), "shared LP differ: {lps:?}");
                }
            }
        }
        if duel == before {
            break;
        }
    }
    duel
}

#[test]
fn two_player_duels_answer_as_the_original() {
    let c = catalogue();
    let s = DuelSetup {
        terrain: vec![terrain::NORMAL; 49],
        leaders: [1, 0],
        decks: [deck(1), deck(2)],
        first: 0,
        leader_abilities: [vec![], vec![]],
        controllers: [PAD_CONTROLLER; 2],
    };
    let (d, _) = Duel::new(s, &c).unwrap();
    assert!(d.multi().is_none());
    for s in [0u8, 1] {
        assert_eq!(d.opp(s), s ^ 1);
        assert_eq!(d.foes(s), vec![s ^ 1]);
        assert_eq!(d.enemy_leaders(s), vec![s ^ 1]);
        assert_eq!(d.first_then_all(s), vec![s, s ^ 1]);
        assert_eq!(d.foes_then_all(s), vec![s ^ 1, s]);
        assert_eq!(d.next_side(s), s ^ 1);
        assert!(d.foe(s, s ^ 1) && !d.foe(s, s) && !d.team_mates(s, s ^ 1));
    }
    assert_eq!(d.living_sides(), vec![0, 1]);
    // No new key in a 2-player state.
    let json = serde_json::to_string(&d).unwrap();
    assert!(!json.contains("multi") && !json.contains("stolen_by"));
    // The N-player command is refused.
    assert!(d.apply(0, &DuelCommand::Target { side: 1 }, &c).is_err());
}

#[test]
fn turns_go_round_the_table_and_rounds_count_from_the_first_player() {
    let c = catalogue();
    let mut s = setup(4, ffa(), 1);
    s.first = 2;
    let (mut d, _) = Duel::new_multi(s, &c).unwrap();
    assert_eq!((d.active, d.round), (2, 1));
    let mut seen = vec![(d.active, d.round)];
    for _ in 0..8 {
        d = d.apply(d.active, &DuelCommand::EndTurn, &c).unwrap().0;
        seen.push((d.active, d.round));
    }
    assert_eq!(
        seen,
        [(2, 1), (1, 1), (3, 1), (0, 1), (2, 2), (1, 2), (3, 2), (0, 2), (2, 3)]
    );
    // Leaders at the middle of each edge.
    for (side, cell) in MULTI_LEADER_CELLS.iter().enumerate() {
        assert_eq!(d.leader_cell(side as u8), Some(*cell));
    }
}

#[test]
fn a_knocked_out_player_is_skipped_and_its_cards_leave() {
    let c = catalogue();
    let (mut d, _) = Duel::new_multi(setup(4, ffa(), 1), &c).unwrap();
    assert_eq!(d.active, 1);
    d.units.push(unit(40, 4, 2, 1, p(5, 3), false));
    d.units.push(unit(41, 4, 2, 6, p(5, 2), false));
    d.sides[2].lp = 0;
    let (d, events) = d.apply(1, &DuelCommand::EndTurn, &c).unwrap();
    assert!(events.iter().any(|e| matches!(e, DuelEvent::KnockedOut { side: 2, reason: EndReason::LifePoints })));
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::Destroyed { .. })));
    assert!(d.is_out(2) && d.units.iter().all(|u| u.side != 2));
    assert_eq!(d.sides[2].graveyard, vec![4, 4]);
    assert_eq!(d.multi().unwrap().out[2].unwrap().place, 4);
    // Turn order 1 -> 3 (2 is skipped: 1 is followed by 3 anyway), 3 -> 0 -> 1.
    assert_eq!(d.active, 3);
    let d = d.apply(3, &DuelCommand::EndTurn, &c).unwrap().0;
    assert_eq!(d.active, 0);
    let d = d.apply(0, &DuelCommand::EndTurn, &c).unwrap().0;
    assert_eq!(d.active, 1, "side 2 is skipped");
    check(&d);
}

#[test]
fn obstacles_stay_when_the_room_says_so() {
    let c = catalogue();
    let rules = MultiRules {
        cards_leave: false,
        ..ffa()
    };
    let (mut d, _) = Duel::new_multi(setup(4, rules, 1), &c).unwrap();
    d.units.push(unit(40, 4, 2, 1, p(5, 3), false));
    d.sides[2].lp = 0;
    let (d, _) = d.apply(1, &DuelCommand::EndTurn, &c).unwrap();
    assert!(d.is_out(2));
    assert!(d.unit(UnitId(40)).is_some() && d.leader_cell(2).is_some());
    assert_ne!(d.active, 2);
}

#[test]
fn the_last_player_standing_wins_and_surrender_knocks_out() {
    let c = catalogue();
    let (d, _) = Duel::new_multi(setup(3, ffa(), 3), &c).unwrap();
    assert_eq!(d.multi().unwrap().order, vec![0, 2, 1]);
    let active = d.active;
    let (d, _) = d.apply(active, &DuelCommand::Surrender, &c).unwrap();
    assert!(d.is_out(active) && d.outcome.is_none());
    assert_ne!(d.active, active);
    let other = d.active;
    let (d, _) = d.forfeit_with(other, &c).unwrap();
    let winner = (0..3).find(|&s| d.alive(s)).unwrap();
    assert_eq!(d.outcome.unwrap().winner, Some(winner));
    assert_eq!(d.outcome.unwrap().reason, EndReason::Surrender);
}

#[test]
fn targets_choose_the_opponent_of_single_target_effects() {
    let c = catalogue();
    let (d, _) = Duel::new_multi(setup(4, ffa(), 1), &c).unwrap();
    let s = d.active;
    // Default: the next enemy in turn order.
    assert_eq!(d.opp(s), d.after(s)[0]);
    let target = d.after(s)[1];
    let (d, events) = d.apply(s, &DuelCommand::Target { side: target }, &c).unwrap();
    assert_eq!(events, vec![DuelEvent::TargetChosen { side: s, target }]);
    assert_eq!(d.opp(s), target);
    assert!(d.apply(s, &DuelCommand::Target { side: s }, &c).is_err());
    // In 2v2 a team-mate cannot be the target.
    let (t, _) = Duel::new_multi(setup(4, teams(false), 1), &c).unwrap();
    let a = t.active;
    let mate = a ^ 1;
    assert!(t.team_mates(a, mate));
    assert!(t.apply(a, &DuelCommand::Target { side: mate }, &c).is_err());
    assert!(!t.foes(a).contains(&mate));
}

#[test]
fn mode_two_auras_spare_the_team_mate_and_hit_every_enemy() {
    // A weakening aura (entry-2 type 0x13, mode 2 = "the other side", filter
    // 0x1D any monster, -300) on side 0 (team S + N).
    let mut list = cards();
    let mut weak = monster(30, kind::FIEND, 4, 1, 1000, 1000);
    weak.entries[2] = Some([0x13 << 8 | 2 << 6 | 0x1D, 300]);
    list.push(monster(25, 0, 0, 0, 0, 0));
    list.push(monster(26, 0, 0, 0, 0, 0));
    list.push(monster(27, 0, 0, 0, 0, 0));
    list.push(monster(28, 0, 0, 0, 0, 0));
    list.push(monster(29, 0, 0, 0, 0, 0));
    list.push(weak);
    let c = Catalogue::new(list, []);
    let mut s = setup(4, teams(false), 1);
    s.decks = vec![vec![2; 40]; 4];
    let (mut d, _) = Duel::new_multi(s, &c).unwrap();
    d.units.push(unit(40, 30, 0, 1, p(3, 5), true));
    for (id, side, at) in [(41, 1, p(3, 1)), (42, 2, p(5, 3)), (43, 3, p(1, 3))] {
        d.units.push(unit(id, 2, side, 1, at, false));
    }
    let atk = |d: &Duel, id: u16| {
        let u = d.unit(UnitId(id)).unwrap();
        d.effective(u, u.position, true, &c)
    };
    assert_eq!(atk(&d, 41), 1400, "the team-mate is spared");
    assert_eq!(atk(&d, 42), 1100);
    assert_eq!(atk(&d, 43), 1100);
    // In a free-for-all every other player is an enemy.
    let mut f = d.clone();
    f.multi.as_mut().unwrap().rules.mode = MultiMode::FreeForAll;
    f.multi.as_mut().unwrap().teams = vec![0, 1, 2, 3];
    assert_eq!(atk(&f, 41), 1100);
}

#[test]
fn ability_14_subtracts_for_each_enemy_window() {
    // Leaders N (3,0) and E (6,3) with #9 (5x5 windows) and #14: a cell in
    // both windows loses both amounts. Leader card 1 (Spellcaster, level 7)
    // weakens Fiends (race table 0x2735C0: 1 -> 7).
    let mut list = cards();
    for id in 25..30 {
        list.push(monster(id, 0, 0, 0, 0, 0));
    }
    list.push(monster(30, kind::FIEND, 4, 1, 1000, 1000));
    let c = Catalogue::new(list, []);
    let mut s = setup(4, ffa(), 1);
    s.leaders = vec![0, 1, 1, 0];
    s.leader_abilities = vec![vec![], vec![9, 14], vec![9, 14], vec![]];
    let (mut d, _) = Duel::new_multi(s, &c).unwrap();
    d.units.push(unit(40, 30, 0, 1, p(4, 2), false));
    d.units.push(unit(41, 30, 0, 2, p(2, 1), false));
    let atk = |d: &Duel, id: u16| {
        let u = d.unit(UnitId(id)).unwrap();
        d.effective(u, u.position, true, &c)
    };
    // (4,2): both windows, 2 x min(100 x 7, 500); (2,1): N's only.
    assert_eq!(atk(&d, 40), 0);
    assert_eq!(atk(&d, 41), 500);
}

#[test]
fn the_projection_keeps_two_sides_and_walls_the_rest() {
    let c = catalogue();
    let (mut d, _) = Duel::new_multi(setup(4, ffa(), 1), &c).unwrap();
    d.units.push(unit(40, 4, 3, 1, p(1, 4), false));
    let (p2, ps) = project(&d, 2, 1, &[]);
    assert!(p2.multi().is_none());
    assert_eq!(p2.sides.len(), 2);
    // E (6,3) is not south of N (3,0): side 2 plays as group 0.
    assert_eq!(ps, 0);
    assert_eq!(p2.leader_cell(0), Some(p(6, 3)));
    assert_eq!(p2.leader_cell(1), Some(p(3, 0)));
    assert!(p2.units.iter().all(|u| u.side < 2));
    for cell in [p(3, 6), p(0, 3), p(1, 4)] {
        assert_eq!(p2.terrain_of(cell), Some(terrain::LABYRINTH));
        assert!(p2.unit_at(cell).is_none());
    }
    assert!(p2.active < 2);
}

#[test]
fn four_computers_play_free_for_all_to_the_end() {
    for seed in 1..=6u64 {
        let rules = MultiRules {
            cards_leave: seed % 2 == 1,
            round_limit: 30,
            ..ffa()
        };
        let (d, _) = Duel::new_multi(setup(4, rules, seed), &catalogue()).unwrap();
        let d = play(d, 20_000);
        let outcome = d.outcome.unwrap_or_else(|| panic!("seed {seed}: unfinished at round {}", d.round));
        if let Some(w) = outcome.winner {
            assert!(d.alive(w), "seed {seed}: the winner is out");
        }
        eprintln!("ffa seed {seed}: {outcome:?} round {} out {:?}", d.round, d.multi().unwrap().out);
    }
}

#[test]
fn three_computers_play_free_for_all_to_the_end() {
    let (d, _) = Duel::new_multi(setup(3, ffa(), 7), &catalogue()).unwrap();
    let d = play(d, 20_000);
    assert!(d.outcome.is_some());
}

#[test]
fn four_computers_play_2v2_to_the_end() {
    for (seed, shared) in [(1u64, false), (2, true), (3, false), (4, true)] {
        let rules = MultiRules {
            round_limit: 30,
            ..teams(shared)
        };
        let (d, _) = Duel::new_multi(setup(4, rules, seed), &catalogue()).unwrap();
        let d = play(d, 20_000);
        let outcome = d.outcome.unwrap_or_else(|| panic!("seed {seed}: unfinished"));
        if let Some(w) = outcome.winner {
            let m = d.multi().unwrap();
            let team = m.teams[usize::from(w)];
            // Every survivor is on the winning team (or the round limit).
            if outcome.reason != EndReason::TurnLimit {
                assert!((0..4u8).filter(|&s| d.alive(s)).all(|s| m.teams[usize::from(s)] == team));
            }
        }
        eprintln!("2v2 seed {seed} shared {shared}: {outcome:?} round {}", d.round);
    }
}

#[test]
fn the_round_limit_ends_by_lp() {
    let c = catalogue();
    let rules = MultiRules {
        round_limit: 2,
        ..ffa()
    };
    let (mut d, _) = Duel::new_multi(setup(4, rules, 4), &c).unwrap();
    for _ in 0..7 {
        d = d.apply(d.active, &DuelCommand::EndTurn, &c).unwrap().0;
    }
    assert!(d.outcome.is_none());
    let mut low = d.clone();
    low.sides[3].lp = 4500;
    let (low, _) = low.apply(low.active, &DuelCommand::EndTurn, &c).unwrap();
    assert_eq!(low.outcome.map(|o| (o.winner, o.reason)), Some((Some(3), EndReason::TurnLimit)));
    // Equal LP: a draw.
    let (tie, _) = d.apply(d.active, &DuelCommand::EndTurn, &c).unwrap();
    assert_eq!(tie.outcome.map(|o| o.winner), Some(None));
}

#[test]
fn shared_team_lp_moves_together() {
    let c = catalogue();
    let rules = MultiRules {
        start_lp: 3000,
        ..teams(true)
    };
    let (mut d, _) = Duel::new_multi(setup(4, rules, 2), &c).unwrap();
    assert!(d.sides.iter().all(|s| s.lp == 3000));
    // A command that costs one member 1000 LP costs the team 1000.
    d.sides[1].lp = 2000;
    let a = d.active;
    let (d, _) = d.apply(a, &DuelCommand::EndTurn, &c).unwrap();
    // `before` was taken inside apply: the hand-set change does not count.
    assert_eq!(d.sides[0].lp, 3000);
    let mut e = d.clone();
    // Simulate an effect inside a command.
    e.sides[0].lp = 3000;
    e.sides[1].lp = 3000;
    let before = vec![3000, 3000, 3000, 3000];
    e.sides[0].lp = 2500;
    e.multi_after(&before, &c, &mut Vec::new()).unwrap();
    assert_eq!((e.sides[0].lp, e.sides[1].lp), (2500, 2500));
}

#[test]
fn team_mates_cannot_be_attacked_or_entered() {
    let c = catalogue();
    let (mut d, _) = Duel::new_multi(setup(4, teams(false), 1), &c).unwrap();
    let a = d.active;
    let mate = a ^ 1;
    let enemy = if a < 2 { 2 } else { 0 };
    d.units.push(unit(40, 4, a, 1, p(3, 3), false));
    d.units.push(unit(41, 4, mate, 1, p(3, 2), false));
    d.units.push(unit(42, 4, enemy, 1, p(2, 3), false));
    let paths = d.reachable(UnitId(40), &c).unwrap();
    assert!(paths.iter().all(|path| path.last() != Some(&p(3, 2))), "the team-mate's cell");
    assert!(paths.iter().any(|path| path.last() == Some(&p(2, 3))), "the enemy's cell");
    let r = d.apply(
        a,
        &DuelCommand::Move {
            unit: UnitId(40),
            path: vec![p(3, 2)],
            face_up: false,
            defence: None,
        },
        &c,
    );
    assert!(r.is_err());
}

#[test]
fn a_team_mate_sees_the_others_cards() {
    let c = catalogue();
    let (mut d, _) = Duel::new_multi(setup(4, teams(false), 1), &c).unwrap();
    let mut hidden = unit(40, 4, 1, 1, p(3, 1), false);
    hidden.face_up = false;
    hidden.committed_face_up = false;
    d.units.push(hidden);
    let card = |viewer: u8| d.view(viewer).units.iter().find(|u| u.id == UnitId(40)).unwrap().card;
    assert_eq!(card(1), Some(4));
    assert_eq!(card(0), Some(4), "team-mate");
    assert_eq!(card(2), None);
    assert_eq!(card(3), None);
    let _ = CardInfo::is_monster;
}

#[test]
#[ignore = "survey: who wins seeded computer matches"]
fn survey_winners() {
    for mode in [MultiMode::FreeForAll, MultiMode::Teams] {
        let mut wins = [0u32; 5];
        for seed in 1..=24u64 {
            let rules = MultiRules { mode, round_limit: 30, ..MultiRules::default() };
            let (d, _) = Duel::new_multi(setup(4, rules, seed), &catalogue()).unwrap();
            let d = play(d, 20_000);
            match d.outcome.and_then(|o| o.winner) {
                Some(w) => wins[usize::from(w)] += 1,
                None => wins[4] += 1,
            }
        }
        eprintln!("{mode:?}: wins by side {wins:?}");
    }
}

/// A catalogue with Exodia's head (0x3A) and limbs (0x36..=0x39).
fn exodia_catalogue() -> Catalogue {
    let mut v = cards();
    while v.len() <= usize::from(crate::duel::EXODIA_HEAD) {
        let id = v.len() as u16;
        v.push(monster(id, kind::MACHINE, 1, 0, 100, 100));
    }
    v[usize::from(crate::duel::EXODIA_HEAD)] = monster(crate::duel::EXODIA_HEAD, kind::SPELLCASTER, 3, 1, 1000, 1000);
    for limb in crate::duel::EXODIA_LIMBS {
        v[usize::from(limb)] = monster(limb, kind::SPELLCASTER, 1, 1, 200, 300);
    }
    Catalogue::new(v, [])
}

/// `side`'s leader made Exodia's head with the four limbs face up around
/// it (the cells on the board side of the leader).
fn assemble_exodia(d: &mut Duel, side: u8) {
    let leader = d.leader_cell(side).unwrap();
    if let Some(l) = d.units.iter_mut().find(|u| u.side == side && u.slot == 0) {
        l.card = crate::duel::EXODIA_HEAD;
    }
    let mut cells: Vec<GridPos> = Vec::new();
    for dc in -1i32..=1 {
        for dr in -1i32..=1 {
            let (c, r) = (i32::from(leader.column) + dc, i32::from(leader.row) + dr);
            if (dc, dr) != (0, 0) && (0..7).contains(&c) && (0..7).contains(&r) {
                cells.push(p(c as u8, r as u8));
            }
        }
    }
    for (k, limb) in crate::duel::EXODIA_LIMBS.into_iter().enumerate() {
        d.units.push(unit(60 + u16::from(side) * 8 + k as u16, limb, side, 1 + k as u8, cells[k], false));
    }
}

/// N-player Exodia (a port addition, docs/ADDITIONS.md): Exodia complete
/// at a player's turn start wins the whole free-for-all at once, as the
/// 2-player rule (0x269D80, `EndReason::Exodia`); the others are not
/// knocked out first.
#[test]
fn exodia_at_turn_start_wins_a_free_for_all() {
    let c = exodia_catalogue();
    let (mut d, _) = Duel::new_multi(setup(4, ffa(), 1), &c).unwrap();
    assert_eq!(d.active, 1);
    assemble_exodia(&mut d, 0);
    // Turn order 1 -> 3 -> 0: Exodia is checked when side 0's turn starts.
    let (d, _) = d.apply(1, &DuelCommand::EndTurn, &c).unwrap();
    assert!(d.outcome.is_none());
    assert_eq!(d.active, 3);
    let (d, events) = d.apply(3, &DuelCommand::EndTurn, &c).unwrap();
    assert_eq!(d.outcome.map(|o| (o.winner, o.reason)), Some((Some(0), EndReason::Exodia)));
    assert!(events.iter().any(|e| matches!(e, DuelEvent::Ended { .. })));
    assert!(!events.iter().any(|e| matches!(e, DuelEvent::KnockedOut { .. })), "nobody is knocked out first");
    assert!((0..4).all(|s| d.alive(s)));
    // The game is over: no side may act.
    assert!(d.apply(0, &DuelCommand::EndTurn, &c).is_err());
}

/// 2v2: the player's team wins (its team-mate with it).
#[test]
fn exodia_at_turn_start_wins_2v2_for_the_team() {
    let c = exodia_catalogue();
    let (mut d, _) = Duel::new_multi(setup(4, teams(false), 1), &c).unwrap();
    assert_eq!(d.active, 1);
    assemble_exodia(&mut d, 2);
    let (d, _) = d.apply(1, &DuelCommand::EndTurn, &c).unwrap();
    assert_eq!(d.active, 3);
    let (d, _) = d.apply(3, &DuelCommand::EndTurn, &c).unwrap();
    assert_eq!(d.active, 0);
    let (d, _) = d.apply(0, &DuelCommand::EndTurn, &c).unwrap();
    let outcome = d.outcome.expect("Exodia at side 2's turn start");
    assert_eq!((outcome.winner, outcome.reason), (Some(2), EndReason::Exodia));
    assert!(d.team_mates(2, 3) && !d.team_mates(2, 0));
}

/// Exodia counts only at its owner's own turn start, and only with all
/// four limbs; a knocked-out player's turns (and checks) are skipped.
#[test]
fn exodia_needs_all_four_limbs_at_its_own_turn_start() {
    let c = exodia_catalogue();
    let (mut d, _) = Duel::new_multi(setup(4, ffa(), 1), &c).unwrap();
    assemble_exodia(&mut d, 0);
    // Three limbs: play goes on through side 0's turn.
    let limb = d.units.iter().position(|u| u.card == crate::duel::EXODIA_LIMBS[3]).unwrap();
    let removed = d.units.remove(limb);
    let mut d = d;
    // 1 -> 3 -> 0.
    for _ in 0..2 {
        d = d.apply(d.active, &DuelCommand::EndTurn, &c).unwrap().0;
    }
    assert!(d.outcome.is_none());
    assert_eq!(d.active, 0);
    // The fourth limb arrives on side 0's turn: the win waits for its next
    // turn start, not the other players'.
    d.units.push(removed);
    for _ in 0..3 {
        d = d.apply(d.active, &DuelCommand::EndTurn, &c).unwrap().0;
        if d.active != 0 {
            assert!(d.outcome.is_none(), "another player's turn start");
        }
    }
    let d = d.apply(d.active, &DuelCommand::EndTurn, &c).unwrap().0;
    assert_eq!(d.outcome.map(|o| (o.winner, o.reason)), Some((Some(0), EndReason::Exodia)));
}
