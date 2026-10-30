//! Private checks: the Bevy-independent duel rules against saved original
//! states captured through PINE (`work/research/duel/claude-pine`).
#[cfg(test)]
mod tests {
    use roses_compare::pine::{MemoryWatch, ReadWidth};
    use roses_compare::state::capture_state;
    use roses_core::duel::ai::{self, pad, Action, Position};
    use roses_core::duel::{
        build_campaign_decks, Catalogue, Duel, DuelCommand, DuelUnit, SideState,
    };
    use roses_core::{GridPos, UnitId};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    const SIDE: u32 = 0x2D_ABA0;
    const SIDE_STRIDE: u32 = 0x350;
    const UNIT: u32 = 0x2D_B990;

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")))
    }

    fn catalogue() -> Catalogue {
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        crate::duel_catalogue::build(&elf).unwrap()
    }

    struct Ram(BTreeMap<u32, u64>);
    impl Ram {
        fn b(&self, a: u32) -> u8 {
            self.get(a) as u8
        }
        fn h(&self, a: u32) -> u16 {
            self.get(a) as u16
        }
        fn get(&self, a: u32) -> u64 {
            *self
                .0
                .get(&a)
                .unwrap_or_else(|| panic!("no watch for {a:#x}"))
        }
    }

    fn read(state: &str) -> Ram {
        let mut watches = Vec::new();
        let mut add = |address: u32, width: ReadWidth| {
            watches.push(MemoryWatch {
                name: format!("{address:x}"),
                address,
                width,
            });
        };
        for a in 0x2D_B8F0..0x2D_B8F8 {
            add(a, ReadWidth::Bits8);
        }
        for side in 0..2 {
            let s = SIDE + side * SIDE_STRIDE;
            for i in 0..40 {
                add(s + 0x1E0 + 2 * i, ReadWidth::Bits16);
            }
            for i in 0..5 {
                add(s + 0x324 + 2 * i, ReadWidth::Bits16);
            }
            add(s + 0x230, ReadWidth::Bits8);
            add(s + 0x322, ReadWidth::Bits8);
            // Graveyard: up to 120 ids between +0x232 and the count at +0x322.
            for i in 0..120 {
                add(s + 0x232 + 2 * i, ReadWidth::Bits16);
            }
            add(s + 0x33A, ReadWidth::Bits16);
            for o in [0x33D, 0x33E, 0x341, 0x343, 0x344, 0x346] {
                add(s + o, ReadWidth::Bits8);
            }
        }
        for g in 0..2 {
            for slot in 0..11 {
                let u = UNIT + g * 0x3F4 + slot * 0x5C;
                add(u + 0x3C, ReadWidth::Bits16);
                add(u + 0x38, ReadWidth::Bits16);
                add(u + 0x3A, ReadWidth::Bits16);
                for o in [0x40, 0x41, 0x44, 0x45, 0x47, 0x48, 0x49, 0x4B, 0x57, 0x58] {
                    add(u + o, ReadWidth::Bits8);
                }
            }
        }
        for cell in 0..49 {
            add(0x2D_C570 + cell * 28 + 0x16, ReadWidth::Bits8);
        }
        let capture = capture_state(env("ROSES_TEST_SESSION").join(state), &watches).unwrap();
        Ram(capture
            .observations
            .into_iter()
            .map(|o| (o.address, o.value))
            .collect())
    }

    fn duel_from(ram: &Ram, cards: &Catalogue) -> Duel {
        let terrain = (0..49).map(|c| ram.b(0x2D_C570 + c * 28 + 0x16)).collect();
        let mut units = Vec::new();
        for g in 0..2u32 {
            for slot in 0..11u32 {
                let u = UNIT + g * 0x3F4 + slot * 0x5C;
                let card = ram.h(u + 0x3C);
                if card == 999 {
                    continue;
                }
                let f = ram.b(u + 0x57);
                units.push(DuelUnit {
                    id: UnitId((g * 11 + slot) as u16),
                    card,
                    side: ram.b(u + 0x44),
                    slot: ram.b(u + 0x45),
                    position: GridPos {
                        column: ram.b(u + 0x40),
                        row: ram.b(u + 0x41),
                    },
                    face_up: f & 4 != 0,
                    defence: f & 8 != 0,
                    committed_face_up: f & 0x10 != 0,
                    committed_defence: f & 0x20 != 0,
                    acted: f & 1 != 0,
                    spellbind: ram.b(u + 0x49),
                    summon_order: ram.b(u + 0x47),
                    attack_bonus: ram.h(u + 0x38) as i16,
                    defense_bonus: ram.h(u + 0x3A) as i16,
                    spellbind_fresh: ram.b(u + 0x58) & 8 != 0,
                    evo_counter: ram.b(u + 0x4B),
                    stolen: ram.b(u + 0x57) & 2 != 0,
                    discount: ram.b(u + 0x48),
                    stolen_by: None,
                });
            }
        }
        let side = |side: u32| {
            let s = SIDE + side * SIDE_STRIDE;
            SideState {
                lp: ram.h(s + 0x33A),
                stars: ram.b(s + 0x341),
                can_summon: ram.b(s + 0x346) & 1 != 0,
                hand: (0..5)
                    .map(|i| ram.h(s + 0x324 + 2 * i))
                    .filter(|&c| c != 999)
                    .collect(),
                deck: (0..u32::from(ram.b(s + 0x230)))
                    .map(|i| ram.h(s + 0x1E0 + 2 * i))
                    .collect(),
                leader_abilities: vec![],
                summons: ram.b(s + 0x343),
                // +0x344: the Destiny Draw byte (SPECIAL-DRAW.md section 1).
                destiny: ram.b(s + 0x344),
                graveyard: (0..u32::from(ram.b(s + 0x322)))
                    .map(|i| ram.h(s + 0x232 + 2 * i))
                    .collect(),
            }
        };
        Duel::from_observed(
            terrain,
            units,
            [side(0), side(1)],
            ram.b(0x2D_B8F3),
            ram.b(0x2D_B8F2),
            ram.b(0x2D_B8F4),
            cards,
        )
        .unwrap()
    }

    fn summary(d: &Duel) -> (Vec<(u16, u8, GridPos)>, [u16; 2]) {
        let mut u: Vec<_> = d
            .units
            .iter()
            .map(|u| (u.card, u.side, u.position))
            .collect();
        u.sort_by_key(|x| (x.1, x.0, x.2.row, x.2.column));
        (u, [d.sides[0].lp, d.sides[1].lp])
    }

    /// Units with slot, order and committed position, plus stars, hands,
    /// round and active side.
    /// Units (slot, card, cell, committed position, spellbind, bonuses) plus
    /// stars, summon counts, hands, graveyards, round and active side.
    fn full(d: &Duel) -> serde_json::Value {
        let mut units: Vec<_> = d
            .units
            .iter()
            .map(|u| {
                serde_json::json!([
                    u.side,
                    u.slot,
                    u.card,
                    [u.position.column, u.position.row],
                    u.committed_face_up,
                    u.committed_defence,
                    u.spellbind,
                    u.attack_bonus,
                    u.defense_bonus
                ])
            })
            .collect();
        units.sort_by_key(|x| (x[0].as_u64(), x[1].as_u64()));
        serde_json::json!({
            "units": units,
            "sides": d.sides.iter().map(|s| serde_json::json!({
                "stars": s.stars,
                "summons": s.summons,
                "hand": s.hand,
                "graveyard": s.graveyard,
            })).collect::<Vec<_>>(),
            "round": d.round,
            "active": d.active,
            "terrain": d.terrain,
        })
    }

    fn at(d: &Duel, column: u8, row: u8) -> UnitId {
        d.unit_at(GridPos { column, row }).unwrap().id
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_weevil_turns_one_to_six_match_the_original() {
        let cards = catalogue();
        let stay = |d: &Duel, c, r, defence| DuelCommand::Move {
            unit: at(d, c, r),
            path: vec![],
            face_up: false,
            defence,
        };
        // Turn 2 (recorded in ai-weevil-t2.jsonl): set 817 at (2,1) and commit
        // it to defence, leader (2,0) -> (1,0), 406 committed to defence.
        let pre = duel_from(&read("685-p2-summon.p2s"), &cards);
        let (d, _) = pre.apply(0, &DuelCommand::EndTurn, &cards).unwrap();
        let open = |column, row| DuelCommand::OpenSummon {
            cell: GridPos { column, row },
        };
        let (d, _) = d.apply(1, &open(2, 1), &cards).unwrap();
        let (d, _) = d
            .apply(1, &DuelCommand::ChooseSummon { hand: vec![1] }, &cards)
            .unwrap();
        let (d, _) = d.apply(1, &stay(&d, 2, 1, Some(true)), &cards).unwrap();
        let leader = DuelCommand::Move {
            unit: at(&d, 2, 0),
            path: vec![GridPos { column: 1, row: 0 }],
            face_up: false,
            defence: None,
        };
        let (d, _) = d.apply(1, &leader, &cards).unwrap();
        let (d, _) = d.apply(1, &stay(&d, 3, 2, Some(true)), &cards).unwrap();
        let (d, _) = d.apply(1, &DuelCommand::EndTurn, &cards).unwrap();
        assert_eq!(
            full(&d),
            full(&duel_from(&read("686-weevil-after-t2.p2s"), &cards))
        );
        // Turn 3 (ai-weevil-t3.jsonl): set 757 at (2,1), leader and 406 stay,
        // 757 committed to defence.
        let pre = duel_from(&read("692-p3-summon.p2s"), &cards);
        let (d, _) = pre.apply(0, &DuelCommand::EndTurn, &cards).unwrap();
        let (d, _) = d.apply(1, &open(2, 1), &cards).unwrap();
        let (d, _) = d
            .apply(1, &DuelCommand::ChooseSummon { hand: vec![2] }, &cards)
            .unwrap();
        let (d, _) = d.apply(1, &stay(&d, 1, 0, None), &cards).unwrap();
        let (d, _) = d.apply(1, &stay(&d, 3, 2, None), &cards).unwrap();
        let (d, _) = d.apply(1, &stay(&d, 2, 1, Some(true)), &cards).unwrap();
        let (d, _) = d.apply(1, &DuelCommand::EndTurn, &cards).unwrap();
        assert_eq!(
            full(&d),
            full(&duel_from(&read("693-weevil-after-t3.p2s"), &cards))
        );
        // Turns 4-6: summons, moves and the two battles against face-down
        // Orion (406 revealed, 1200 vs 1800 loses; Hunter Spider 2100 wins).
        let go = |d: &Duel, (c, r): (u8, u8), path: &[(u8, u8)], face_up| DuelCommand::Move {
            unit: at(d, c, r),
            path: path
                .iter()
                .map(|&(column, row)| GridPos { column, row })
                .collect(),
            face_up,
            defence: None,
        };
        type Plan<'a> = (
            &'a str,
            (u8, u8),
            usize,
            &'a [((u8, u8), &'a [(u8, u8)], bool)],
            &'a str,
        );
        let turns: [Plan; 4] = [
            (
                "679-weevil1-loaded.p2s",
                (3, 1),
                2,
                &[((3, 0), &[(2, 0)], false), ((3, 1), &[(3, 2)], false)],
                "680-weevil1-after-t1.p2s",
            ),
            (
                "695-p4-envoy.p2s",
                (1, 1),
                0,
                &[
                    ((1, 0), &[], false),
                    ((3, 2), &[(3, 3), (3, 4)], true),
                    ((1, 1), &[(1, 2)], false),
                ],
                "697-t4-wait.p2s",
            ),
            (
                "698-p5-moves.p2s",
                (2, 0),
                0,
                &[
                    ((1, 0), &[], false),
                    ((2, 0), &[(3, 0)], false),
                    ((1, 2), &[(2, 2)], false),
                    ((2, 1), &[], false),
                ],
                "699-weevil-after-t5.p2s",
            ),
            (
                "700-p6-moves.p2s",
                (1, 1),
                1,
                &[
                    ((1, 0), &[], false),
                    ((3, 0), &[(3, 1)], false),
                    ((2, 2), &[(2, 3)], false),
                    ((1, 1), &[(1, 2)], false),
                    ((2, 1), &[], false),
                ],
                "701-weevil-after-t6.p2s",
            ),
        ];
        for (before, cell, hand_index, moves, after) in turns {
            let pre = duel_from(&read(before), &cards);
            let (mut d, _) = pre.apply(0, &DuelCommand::EndTurn, &cards).unwrap();
            d = d.apply(1, &open(cell.0, cell.1), &cards).unwrap().0;
            d = d
                .apply(
                    1,
                    &DuelCommand::ChooseSummon {
                        hand: vec![hand_index],
                    },
                    &cards,
                )
                .unwrap()
                .0;
            for &(from, path, face_up) in moves {
                d = d.apply(1, &go(&d, from, path, face_up), &cards).unwrap().0;
            }
            d = d.apply(1, &DuelCommand::EndTurn, &cards).unwrap().0;
            let post = duel_from(&read(after), &cards);
            assert_eq!(full(&d), full(&post), "{before} -> {after}");
            assert_eq!(
                [d.sides[0].lp, d.sides[1].lp],
                [post.sides[0].lp, post.sides[1].lp],
                "{after}"
            );
        }
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_tutorial_battles_match_the_original() {
        let cards = catalogue();
        let celtic_to = |target: GridPos| DuelCommand::Move {
            unit: UnitId(1),
            path: vec![target],
            face_up: false,
            defence: None,
        };
        // (before, after): Celtic Guardian at (3,4) attacks the unit at (3,3).
        for (before, after) in [
            ("025-e6.p2s", "029-b1.p2s"),   // Baby Dragon, attack position
            ("040-c8.p2s", "044-c12.p2s"),  // repeat
            ("053-c21.p2s", "057-c25.p2s"), // equal ATK, both destroyed
            ("063-c31.p2s", "067-d1.p2s"),  // Hard Armor in defence
            ("071-c37.p2s", "075-d3.p2s"),  // Aqua Madoor DEF 2000
            ("078-c43.p2s", "082-c47.p2s"), // Rock Ogre Grotto #2 DEF = ATK
        ] {
            let pre = duel_from(&read(before), &cards);
            let post = duel_from(&read(after), &cards);
            let (native, _) = pre
                .apply(0, &celtic_to(GridPos { column: 3, row: 3 }), &cards)
                .unwrap();
            assert_eq!(summary(&native), summary(&post), "{before} -> {after}");
        }
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_first_campaign_duel_deck_shuffle_matches() {
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let decks = roses_assets::parse_pal_decks(&elf).unwrap().decks;
        // RNG state at 655-aa1 is read through the reent pointer 0x28ED18+0xA8;
        // 127 draws separate it from the duel-init deck loop 0x1AE7B0.
        let before = env("ROSES_TEST_SESSION").join("655-aa1.p2s");
        let ptr = read_word(&before, 0x28_ED18, ReadWidth::Bits32) as u32;
        let mut rng = read_word(&before, ptr + 0xA8, ReadWidth::Bits64);
        for _ in 0..127 {
            roses_core::duel::next_rand31(&mut rng);
        }
        let built = build_campaign_decks(&mut rng, [&decks[0x09].cards, &decks[0x1C].cards]);
        let start = read("678-aa24.p2s");
        for side in 0..2u32 {
            let s = SIDE + side * SIDE_STRIDE;
            let observed: Vec<u16> = (0..40).map(|i| start.h(s + 0x1E0 + 2 * i)).collect();
            assert_eq!(built[side as usize], observed, "side {side}");
        }
    }

    fn read_word(path: &std::path::Path, address: u32, width: ReadWidth) -> u64 {
        let capture = capture_state(
            path,
            &[MemoryWatch {
                name: "w".into(),
                address,
                width,
            }],
        )
        .unwrap();
        capture.observations[0].value
    }

    /// Key plans of a record_ai.py log: consecutive queue contents that
    /// extend each other are one plan being written.
    fn logged_plans(name: &str) -> (GridPos, Vec<Vec<u16>>) {
        let text = std::fs::read_to_string(env("ROSES_TEST_SESSION").join(name)).unwrap();
        let mut start = None;
        let mut plans = Vec::new();
        let mut current: Vec<u16> = Vec::new();
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let Some(queue) = v.get("queue").and_then(|q| q.as_array()) else {
                continue;
            };
            start.get_or_insert(GridPos {
                column: v["cursor"][0].as_u64().unwrap() as u8,
                row: v["cursor"][1].as_u64().unwrap() as u8,
            });
            let q: Vec<u16> = queue
                .iter()
                .map(|k| k.as_u64().unwrap() as u16)
                .take_while(|&k| k != 0xFFFF)
                .collect();
            if (q.is_empty() || !q.starts_with(&current)) && !current.is_empty() {
                plans.push(std::mem::take(&mut current));
            }
            if !q.is_empty() {
                current = q;
            }
        }
        if !current.is_empty() {
            plans.push(current);
        }
        (start.unwrap(), plans)
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_weevil_key_presses_match_the_recorded_queues() {
        let cards = catalogue();
        let summon = |c, r, i| {
            (
                Some((GridPos { column: c, row: r }, i)),
                (0, 0),
                (0, 0),
                false,
                Position::Keep,
            )
        };
        let act =
            |from: (u8, u8), to: (u8, u8), reveal, position| (None, from, to, reveal, position);
        type Step = (Option<(GridPos, usize)>, (u8, u8), (u8, u8), bool, Position);
        let turns: [(&str, &str, Vec<Step>); 6] = [
            (
                "679-weevil1-loaded.p2s",
                "ai-weevil-t1b.jsonl",
                vec![
                    summon(3, 1, 2),
                    act((3, 0), (2, 0), false, Position::Keep),
                    act((3, 1), (3, 2), false, Position::Keep),
                ],
            ),
            (
                "685-p2-summon.p2s",
                "ai-weevil-t2.jsonl",
                vec![
                    summon(2, 1, 1),
                    act((2, 1), (2, 1), false, Position::Defence),
                    act((2, 0), (1, 0), false, Position::Keep),
                    act((3, 2), (3, 2), false, Position::Defence),
                ],
            ),
            (
                "692-p3-summon.p2s",
                "ai-weevil-t3.jsonl",
                vec![
                    summon(2, 1, 2),
                    act((1, 0), (1, 0), false, Position::Keep),
                    act((3, 2), (3, 2), false, Position::Keep),
                    act((2, 1), (2, 1), false, Position::Defence),
                ],
            ),
            (
                "695-p4-envoy.p2s",
                "ai-weevil-t4.jsonl",
                vec![
                    summon(1, 1, 0),
                    act((1, 0), (1, 0), false, Position::Keep),
                    act((3, 2), (3, 4), true, Position::Keep),
                    // Logged after the recording stopped: (1,1) -> (1,2).
                    act((1, 1), (1, 2), false, Position::Keep),
                ],
            ),
            (
                "698-p5-moves.p2s",
                "ai-weevil-t5.jsonl",
                vec![
                    summon(2, 0, 0),
                    act((1, 0), (1, 0), false, Position::Keep),
                    act((2, 0), (3, 0), false, Position::Keep),
                    act((1, 2), (2, 2), false, Position::Keep),
                    act((2, 1), (2, 1), false, Position::Keep),
                ],
            ),
            (
                "700-p6-moves.p2s",
                "ai-weevil-t6.jsonl",
                vec![
                    summon(1, 1, 1),
                    act((1, 0), (1, 0), false, Position::Keep),
                    act((3, 0), (3, 1), false, Position::Keep),
                    act((2, 2), (2, 3), false, Position::Keep),
                    act((1, 1), (1, 2), false, Position::Keep),
                    act((2, 1), (2, 1), false, Position::Keep),
                ],
            ),
        ];
        let mut compared = 0;
        for (state, log, steps) in turns {
            let (mut cursor, plans) = logged_plans(log);
            let unrecorded = usize::from(log == "ai-weevil-t4.jsonl");
            assert_eq!(plans.len() + unrecorded, steps.len(), "{log}");
            let mut d = duel_from(&read(state), &cards)
                .apply(0, &DuelCommand::EndTurn, &cards)
                .unwrap()
                .0;
            for (i, (summon, from, to, reveal, position)) in steps.into_iter().enumerate() {
                let action = match summon {
                    Some((cell, index)) => Action::Summon {
                        cell,
                        hand: vec![index],
                    },
                    None => Action::Act {
                        unit: at(&d, from.0, from.1),
                        target: GridPos {
                            column: to.0,
                            row: to.1,
                        },
                        reveal,
                        position,
                    },
                };
                let keys = ai::keys(&d, 1, cursor, &action, &cards);
                if let Some(plan) = plans.get(i) {
                    assert_eq!(&keys, plan, "{log} plan {i}");
                    compared += 1;
                }
                cursor = ai::cursor_after(&action);
                d = match &action {
                    Action::Summon { cell, hand } => {
                        let d = d
                            .apply(1, &DuelCommand::OpenSummon { cell: *cell }, &cards)
                            .unwrap()
                            .0;
                        d.apply(1, &DuelCommand::ChooseSummon { hand: hand.clone() }, &cards)
                            .unwrap()
                            .0
                    }
                    Action::Act {
                        unit,
                        target,
                        reveal,
                        position,
                    } => {
                        let path = ai::path(&d, *unit, *target, &cards);
                        let defence = match (path.is_empty(), position) {
                            (true, Position::Defence) => Some(true),
                            (true, Position::Attack) => Some(false),
                            _ => None,
                        };
                        let cmd = DuelCommand::Move {
                            unit: *unit,
                            path,
                            face_up: *reveal,
                            defence,
                        };
                        d.apply(1, &cmd, &cards).unwrap().0
                    }
                    Action::Activate { .. } => {
                        panic!("{log}: Weevil never plans an activation (0xC/0xD)")
                    }
                };
            }
        }
        // Every logged plan except the unrecorded fourth of turn 4.
        assert_eq!(compared, 3 + 4 + 4 + 3 + 5 + 6);
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_block_attack_matches_the_original() {
        // 686: player's turn 3 starts; the move Orion (3,5) -> (3,4) triggers
        // Weevil's 817 at (2,1); 690 is the state after the trap resolved.
        let cards = catalogue();
        let pre = duel_from(&read("686-weevil-after-t2.p2s"), &cards);
        let orion = at(&pre, 3, 5);
        let (d, _) = pre
            .apply(
                0,
                &DuelCommand::Move {
                    unit: orion,
                    path: vec![GridPos { column: 3, row: 4 }],
                    face_up: false,
                    defence: None,
                },
                &cards,
            )
            .unwrap();
        let post = duel_from(&read("690-p3-trap.p2s"), &cards);
        assert_eq!(full(&d), full(&post));
        assert!(d.unit(orion).unwrap().acted);
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_card_experiments_match_the_original() {
        // claude-effects/EXPERIMENTS.md: (pre state, unit cell, path, reveal,
        // post state). Pre states already contain the documented RAM setups.
        let cards = catalogue();
        type Case<'a> = (&'a str, (u8, u8), &'a [(u8, u8)], bool, &'a str);
        let cases: [Case; 7] = [
            (
                "704-x1-cursor.p2s",
                (2, 2),
                &[(2, 1)],
                false,
                "707-x1-after.p2s",
            ),
            (
                "712-x3-summoned.p2s",
                (3, 5),
                &[(4, 5)],
                false,
                "714-x3-after.p2s",
            ),
            (
                "717-x4-set.p2s",
                (4, 5),
                &[(3, 5)],
                false,
                "719-x4-after.p2s",
            ),
            ("722-x5-set.p2s", (2, 5), &[], true, "725-x5-settle.p2s"),
            (
                "726-x6-sel.p2s",
                (1, 2),
                &[(2, 2)],
                false,
                "728-x6-after.p2s",
            ),
            // Manual flips: Hoshiningen and Key Mace (EXPERIMENTS.md E6/E7).
            ("741-x8-summon.p2s", (2, 5), &[], true, "742-x8-flip.p2s"),
            ("747-x9-summon.p2s", (4, 5), &[], true, "748-x9-flip.p2s"),
        ];
        for (before, (c0, r0), path, face_up, after) in cases {
            let pre = duel_from(&read(before), &cards);
            let cmd = DuelCommand::Move {
                unit: at(&pre, c0, r0),
                path: path
                    .iter()
                    .map(|&(column, row)| GridPos { column, row })
                    .collect(),
                face_up,
                defence: None,
            };
            let (d, _) = pre.apply(0, &cmd, &cards).unwrap();
            let post = duel_from(&read(after), &cards);
            assert_eq!(full(&d), full(&post), "{before} -> {after}");
        }
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_fusions_match_the_original() {
        // claude-effects/EXPERIMENTS.md F1-F3 (round 14, RAM setups noted there).
        let cards = catalogue();
        let cell = |column, row| GridPos { column, row };
        // F1: hand 4 + 15 marked in that order, summoned at (2,6) -> 7, cost 6.
        let pre = duel_from(&read("751-f1-cell.p2s"), &cards);
        let (d, _) = pre
            .apply(0, &DuelCommand::OpenSummon { cell: cell(2, 6) }, &cards)
            .unwrap();
        let (d, _) = d
            .apply(0, &DuelCommand::ChooseSummon { hand: vec![2, 3] }, &cards)
            .unwrap();
        assert_eq!(
            full(&d),
            full(&duel_from(&read("756-f1-fused.p2s"), &cards))
        );
        // F2: Key Mace (slot 2) onto Hoshiningen (slot 1) -> 385 in slot 2 on
        // the target cell, acted. F3: 244 (slot 3) onto 385 (slot 2) with slot
        // 1 free -> 221 in slot 1, left on the mover's cell, not acted.
        for (before, from, after, acted) in [
            ("757-f2-setup.p2s", cell(3, 5), "761-f2-after.p2s", true),
            ("762-f3-setup.p2s", cell(2, 6), "765-f3-after.p2s", false),
        ] {
            let pre = duel_from(&read(before), &cards);
            let mover = at(&pre, from.column, from.row);
            let cmd = DuelCommand::Move {
                unit: mover,
                path: vec![cell(2, 5)],
                face_up: false,
                defence: None,
            };
            let (d, _) = pre.apply(0, &cmd, &cards).unwrap();
            assert_eq!(full(&d), full(&duel_from(&read(after), &cards)), "{before}");
            assert_eq!(d.unit(mover).unwrap().acted, acted, "{before}");
        }
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_weevil_key_queues_drive_the_pad_input() {
        // The same recorded turns as the replay below, but every key goes
        // through `ui::DuelUi` (the pad-driven states 0x18/0x1D/0x3E/0x43/
        // 0x46), which alone decides the commands.
        use roses_core::duel::ui::{DuelUi, Response};
        let cards = catalogue();
        let turns = [
            ("t1b", "679-weevil1-loaded", "680-weevil1-after-t1"),
            ("t2", "685-p2-summon", "686-weevil-after-t2"),
            ("t3", "692-p3-summon", "693-weevil-after-t3"),
            ("t5", "698-p5-moves", "699-weevil-after-t5"),
            ("t6", "700-p6-moves", "701-weevil-after-t6"),
            ("t7", "709-x2-bow-set", "710-weevil-after-t7"),
            ("t8", "714-x3-after", "715-weevil-after-t8"),
            ("t9", "719-x4-after", "720-weevil-after-t9"),
            ("t10", "733-x7-cancel2", "734-weevil-after-t10"),
            ("t11", "737-x7-set", "738-weevil-after-t11"),
            ("t13", "748-x9-flip", "749-weevil-after-t13"),
            // Javelin Beetle Pact revealed; Hercules Beetle written into
            // Weevil's unit g1 s5 first (EXPERIMENTS.md R1).
            ("t16", "768-r1-setup", "769-weevil-after-t16"),
        ];
        let mut keys_fed = 0;
        for (log, before, after) in turns {
            let (cursor, plans) = logged_plans(&format!("ai-weevil-{log}.jsonl"));
            let mut d = duel_from(&read(&format!("{before}.p2s")), &cards)
                .apply(0, &DuelCommand::EndTurn, &cards)
                .unwrap()
                .0;
            let mut input = DuelUi::new(&d);
            input.cursors[1] = cursor;
            for key in plans.iter().flatten().copied().chain([pad::START]) {
                keys_fed += 1;
                match input.press(&d, &cards, key) {
                    Ok(Response::Commands(cmds)) => {
                        for cmd in cmds {
                            d = match d.apply(1, &cmd, &cards) {
                                Ok((next, _)) => next,
                                Err(e) => panic!("{log} {cmd:?}: {e}"),
                            };
                        }
                    }
                    Ok(Response::Refused | Response::Unsupported(_)) => {
                        panic!("{log}: key {key:#x} refused in {:?}", input.mode)
                    }
                    Ok(_) => (),
                    Err(e) => panic!("{log}: key {key:#x}: {e}"),
                }
            }
            assert_eq!(d.active, 0, "{log}: the turn did not end");
            let post = duel_from(&read(&format!("{after}.p2s")), &cards);
            assert_eq!(full(&d), full(&post), "{log}: {before} -> {after}");
        }
        assert!(keys_fed >= 300, "{keys_fed}");
    }

    /// Side +0x33D/+0x33E: the side's board cursor.
    fn cursor_of(ram: &Ram, side: u32) -> GridPos {
        let s = SIDE + side * SIDE_STRIDE;
        GridPos {
            column: ram.b(s + 0x33D),
            row: ram.b(s + 0x33E),
        }
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_player_keys_drive_the_pad_input() {
        // The player's own recorded presses (session json "actions") through
        // `ui::DuelUi`, from the state before them to the state after.
        use pad::*;
        use roses_core::duel::ui::{DuelUi, Response};
        let cards = catalogue();
        type Case<'a> = (&'a str, &'a [(usize, u16)], &'a [u16], &'a str);
        let cases: [Case; 5] = [
            // E6/E7: select, reveal, confirm in place.
            ("741-x8-summon", &[], &[CROSS, R2, CROSS], "742-x8-flip"),
            ("747-x9-summon", &[], &[CROSS, R2, CROSS], "748-x9-flip"),
            // F1 (EXPERIMENTS.md; hand slots 2 and 3 were written first).
            (
                "749-weevil-after-t13",
                &[(2, 4), (3, 15)],
                &[
                    LEFT, DOWN, SQUARE, LEFT, CROSS, RIGHT, RIGHT, UP, RIGHT, UP, CROSS, CROSS,
                ],
                "756-f1-fused",
            ),
            // F2 and F3: moving onto an own monster.
            (
                "757-f2-setup",
                &[],
                &[RIGHT, UP, CROSS, LEFT, CROSS],
                "761-f2-after",
            ),
            (
                "762-f3-setup",
                &[],
                &[DOWN, CROSS, UP, CROSS],
                "765-f3-after",
            ),
        ];
        for (before, hand, keys, after) in cases {
            let ram = read(&format!("{before}.p2s"));
            let mut d = duel_from(&ram, &cards);
            if !hand.is_empty() {
                // Rebuild the written slots (the RAM reader drops empty ones).
                let mut slots: Vec<u16> = (0..5).map(|i| ram.h(SIDE + 0x324 + 2 * i)).collect();
                for &(i, card) in hand {
                    slots[i] = card;
                }
                d.sides[0].hand = slots.into_iter().filter(|&c| c != 999).collect();
            }
            let mut input = DuelUi::new(&d);
            input.cursors = vec![cursor_of(&ram, 0), cursor_of(&ram, 1)];
            for &key in keys {
                match input.press(&d, &cards, key) {
                    Ok(Response::Commands(cmds)) => {
                        for cmd in cmds {
                            d = match d.apply(0, &cmd, &cards) {
                                Ok((next, _)) => next,
                                Err(e) => panic!("{before} {cmd:?}: {e}"),
                            };
                        }
                    }
                    Ok(Response::Refused | Response::Unsupported(_)) => {
                        panic!("{before}: key {key:#x} refused in {:?}", input.mode)
                    }
                    Ok(_) => (),
                    Err(e) => panic!("{before}: key {key:#x}: {e}"),
                }
            }
            let post = read(&format!("{after}.p2s"));
            assert_eq!(
                full(&d),
                full(&duel_from(&post, &cards)),
                "{before} -> {after}"
            );
            assert_eq!(input.cursors[0], cursor_of(&post, 0), "{before} cursor");
        }
    }

    /// Inverse of `ai::keys` for one recorded plan: walk the cursor, then a
    /// summon (Square, cell, hand marks) or an action on the unit under the
    /// cursor (reveal/position keys, then the pressed path, which is used as
    /// the move path). Returns the action and the path.
    fn decode_plan(
        d: &Duel,
        side: u8,
        mut cursor: GridPos,
        keys: &[u16],
    ) -> (Action, Vec<GridPos>) {
        use roses_core::duel::ai::pad;
        let step = |at: GridPos, key: u16| -> Option<GridPos> {
            let (dc, dr): (i8, i8) = match key {
                pad::LEFT => (-1, 0),
                pad::RIGHT => (1, 0),
                pad::UP => (0, -1),
                pad::DOWN => (0, 1),
                _ => return None,
            };
            let (dc, dr) = if side == 1 { (-dc, -dr) } else { (dc, dr) };
            Some(GridPos {
                column: (at.column as i8 + dc) as u8,
                row: (at.row as i8 + dr) as u8,
            })
        };
        let mut i = 0;
        while let Some(next) = step(cursor, keys[i]) {
            cursor = next;
            i += 1;
        }
        if keys[i] == pad::SQUARE {
            i += 1;
            while let Some(next) = step(cursor, keys[i]) {
                cursor = next;
                i += 1;
            }
            assert_eq!(keys[i], pad::CROSS);
            i += 1;
            let (mut at, mut hand) = (0usize, Vec::new());
            while keys[i] != pad::CROSS {
                match keys[i] {
                    pad::RIGHT => at += 1,
                    pad::LEFT => at -= 1,
                    pad::UP => hand.push(at),
                    k => panic!("unexpected hand key {k:#x}"),
                }
                i += 1;
            }
            assert_eq!(&keys[i..], [pad::CROSS, pad::CROSS]);
            return (Action::Summon { cell: cursor, hand }, Vec::new());
        }
        assert_eq!(keys[i], pad::CROSS);
        i += 1;
        let unit = d.unit_at(cursor).expect("unit under the cursor").id;
        let (mut reveal, mut position) = (false, Position::Keep);
        loop {
            match keys[i] {
                pad::R2 => reveal = true,
                pad::R1 => position = Position::Defence,
                pad::L1 => position = Position::Attack,
                _ => break,
            }
            i += 1;
        }
        let mut path = Vec::new();
        while let Some(next) = step(cursor, keys[i]) {
            cursor = next;
            path.push(cursor);
            i += 1;
        }
        assert_eq!(&keys[i..], [pad::CROSS]);
        (
            Action::Act {
                unit,
                target: cursor,
                reveal,
                position,
            },
            path,
        )
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_weevil_recorded_keys_replay_through_the_engine() {
        // Every recorded key plan of Weevil's turns 1-11 is decoded, checked
        // against ai::keys and applied; the result must equal the saved state
        // after his turn. Turn 4 is skipped (its last plan was not logged).
        let cards = catalogue();
        let turns = [
            ("t1b", "679-weevil1-loaded", "680-weevil1-after-t1"),
            ("t2", "685-p2-summon", "686-weevil-after-t2"),
            ("t3", "692-p3-summon", "693-weevil-after-t3"),
            ("t5", "698-p5-moves", "699-weevil-after-t5"),
            ("t6", "700-p6-moves", "701-weevil-after-t6"),
            ("t7", "709-x2-bow-set", "710-weevil-after-t7"),
            ("t8", "714-x3-after", "715-weevil-after-t8"),
            ("t9", "719-x4-after", "720-weevil-after-t9"),
            ("t10", "733-x7-cancel2", "734-weevil-after-t10"),
            ("t11", "737-x7-set", "738-weevil-after-t11"),
            // Tears of the Mermaid fires on the spider's attack target.
            ("t13", "748-x9-flip", "749-weevil-after-t13"),
            // Javelin Beetle Pact revealed; Hercules Beetle written into
            // Weevil's unit g1 s5 first (EXPERIMENTS.md R1).
            ("t16", "768-r1-setup", "769-weevil-after-t16"),
        ];
        let mut plans_checked = 0;
        for (log, before, after) in turns {
            let (mut cursor, plans) = logged_plans(&format!("ai-weevil-{log}.jsonl"));
            let mut d = duel_from(&read(&format!("{before}.p2s")), &cards)
                .apply(0, &DuelCommand::EndTurn, &cards)
                .unwrap()
                .0;
            for keys in &plans {
                let (action, path) = decode_plan(&d, 1, cursor, keys);
                assert_eq!(
                    &ai::keys(&d, 1, cursor, &action, &cards),
                    keys,
                    "{log} {action:?}"
                );
                plans_checked += 1;
                cursor = ai::cursor_after(&action);
                let commands = match &action {
                    Action::Summon { cell, hand } => {
                        vec![
                            DuelCommand::OpenSummon { cell: *cell },
                            DuelCommand::ChooseSummon { hand: hand.clone() },
                        ]
                    }
                    Action::Act {
                        unit,
                        reveal,
                        position,
                        ..
                    } => vec![DuelCommand::Move {
                        unit: *unit,
                        defence: match (path.is_empty(), position) {
                            (true, Position::Defence) => Some(true),
                            (true, Position::Attack) => Some(false),
                            _ => None,
                        },
                        path: path.clone(),
                        face_up: *reveal,
                    }],
                    Action::Activate { .. } => {
                        panic!("{log}: Weevil never plans an activation (0xC/0xD)")
                    }
                };
                for cmd in commands {
                    d = match d.apply(1, &cmd, &cards) {
                        Ok((next, _)) => next,
                        Err(e) => panic!("{log} {cmd:?}: {e}"),
                    };
                }
            }
            d = d.apply(1, &DuelCommand::EndTurn, &cards).unwrap().0;
            let post = duel_from(&read(&format!("{after}.p2s")), &cards);
            assert_eq!(full(&d), full(&post), "{log}: {before} -> {after}");
        }
        assert!(plans_checked >= 50, "{plans_checked}");
    }
}
