//! Private checks: the engine's experience bookkeeping against runs of the
//! original's own code in the EE interpreter from saved states
//! (the project notes §2; the in-duel cases
//! were run from `757-f2-setup` with every experience value set to 1000, so
//! the numbers below are plain deltas).
#[cfg(test)]
mod tests {
    use roses_compare::pine::{MemoryWatch, ReadWidth};
    use roses_compare::state::capture_state;
    use roses_core::duel::{
        recompute_rank, Catalogue, Duel, DuelCommand, DuelUnit, EndReason, Outcome, SideState,
    };
    use roses_core::{GridPos, UnitId};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    const SIDE: u32 = 0x2D_ABA0;
    const SIDE_STRIDE: u32 = 0x350;
    const UNIT: u32 = 0x2D_B990;
    /// Card blocks: side byte[0x312B50] at 0x2F2C10, the other 0xFFA0 on;
    /// record stride 0x4C, rank word +0x48, experience +0x4A.
    const BLOCK: u32 = 0x2F_2C10;
    const OWN_BLOCK_SIDE: u32 = 0x31_2B50;

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")))
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

    fn capture(state: &str, reads: &[(u32, ReadWidth)]) -> Ram {
        let watches: Vec<MemoryWatch> = reads
            .iter()
            .map(|&(address, width)| MemoryWatch {
                name: format!("{address:x}"),
                address,
                width,
            })
            .collect();
        let capture = capture_state(env("ROSES_TEST_SESSION").join(state), &watches).unwrap();
        Ram(capture
            .observations
            .into_iter()
            .map(|o| (o.address, o.value))
            .collect())
    }

    /// The duel as `duel_check::duel_from` reads it.
    fn duel_from(state: &str, cards: &Catalogue) -> Duel {
        let mut reads = Vec::new();
        for a in 0x2D_B8F0..0x2D_B8F8 {
            reads.push((a, ReadWidth::Bits8));
        }
        for side in 0..2 {
            let s = SIDE + side * SIDE_STRIDE;
            for i in 0..40 {
                reads.push((s + 0x1E0 + 2 * i, ReadWidth::Bits16));
            }
            for i in 0..5 {
                reads.push((s + 0x324 + 2 * i, ReadWidth::Bits16));
            }
            for i in 0..120 {
                reads.push((s + 0x232 + 2 * i, ReadWidth::Bits16));
            }
            reads.push((s + 0x33A, ReadWidth::Bits16));
            for o in [0x230, 0x322, 0x341, 0x343, 0x344, 0x346] {
                reads.push((s + o, ReadWidth::Bits8));
            }
        }
        for g in 0..2 {
            for slot in 0..11 {
                let u = UNIT + g * 0x3F4 + slot * 0x5C;
                for o in [0x38, 0x3A, 0x3C] {
                    reads.push((u + o, ReadWidth::Bits16));
                }
                for o in [0x40, 0x41, 0x44, 0x45, 0x47, 0x49, 0x4B, 0x57, 0x58] {
                    reads.push((u + o, ReadWidth::Bits8));
                }
            }
        }
        for cell in 0..49 {
            reads.push((0x2D_C570 + cell * 28 + 0x16, ReadWidth::Bits8));
        }
        let ram = capture(state, &reads);
        let terrain = (0..49).map(|c| ram.b(0x2D_C570 + c * 28 + 0x16)).collect();
        let mut units = Vec::new();
        for g in 0..2u32 {
            for slot in 0..11u32 {
                let u = UNIT + g * 0x3F4 + slot * 0x5C;
                let card = ram.h(u + 0x3C);
                if card == 999 {
                    continue;
                }
                let mut unit = placed(
                    (g * 11 + slot) as u16,
                    card,
                    ram.b(u + 0x44),
                    ram.b(u + 0x45),
                    (ram.b(u + 0x40), ram.b(u + 0x41)),
                    ram.b(u + 0x57),
                );
                unit.spellbind = ram.b(u + 0x49);
                unit.summon_order = ram.b(u + 0x47);
                unit.attack_bonus = ram.h(u + 0x38) as i16;
                unit.defense_bonus = ram.h(u + 0x3A) as i16;
                unit.spellbind_fresh = ram.b(u + 0x58) & 8 != 0;
                unit.evo_counter = ram.b(u + 0x4B);
                units.push(unit);
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

    /// A unit with the +0x57 flags (bit 0 acted, 2 face-up, 3 defence, 4/5
    /// committed face-up/defence).
    fn placed(id: u16, card: u16, side: u8, slot: u8, (column, row): (u8, u8), f: u8) -> DuelUnit {
        DuelUnit {
            id: UnitId(id),
            card,
            side,
            slot,
            position: GridPos { column, row },
            face_up: f & 4 != 0,
            defence: f & 8 != 0,
            committed_face_up: f & 0x10 != 0,
            committed_defence: f & 0x20 != 0,
            acted: f & 1 != 0,
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

    /// Stored (rank word, experience) of each (side, card).
    fn stored(state: &str, wanted: &[(u8, u16)]) -> BTreeMap<(u8, u16), (u16, u16)> {
        let own = capture(state, &[(OWN_BLOCK_SIDE, ReadWidth::Bits8)]).b(OWN_BLOCK_SIDE);
        let record = |side: u8, card: u16| {
            BLOCK + if side == own { 0 } else { 0xFFA0 } + u32::from(card) * 0x4C
        };
        let mut reads = Vec::new();
        for &(side, card) in wanted {
            reads.push((record(side, card) + 0x48, ReadWidth::Bits16));
            reads.push((record(side, card) + 0x4A, ReadWidth::Bits16));
        }
        let ram = capture(state, &reads);
        wanted
            .iter()
            .map(|&(side, card)| {
                let r = record(side, card);
                ((side, card), (ram.h(r + 0x48), ram.h(r + 0x4A)))
            })
            .collect()
    }

    fn catalogue_and_thresholds() -> (Catalogue, [u16; 13]) {
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        (
            crate::duel_catalogue::build(&elf).unwrap(),
            roses_assets::parse_pal_rank_thresholds(&elf).unwrap(),
        )
    }

    /// The recorded (side, card, delta) triples.
    fn deltas(d: &Duel) -> Vec<(u8, u16, i32)> {
        (0..2u8)
            .flat_map(|side| {
                d.experience[usize::from(side)]
                    .iter()
                    .map(move |(&card, e)| (side, card, e.delta))
            })
            .collect()
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_duel_end_award_matches_the_interpreter() {
        const STATE: &str = "767-weevil-after-t15.p2s";
        let (cards, thresholds) = catalogue_and_thresholds();
        let duel = duel_from(STATE, &cards);
        assert_eq!(
            [duel.sides[0].graveyard.len(), duel.sides[1].graveyard.len()],
            [13, 4]
        );
        let field: BTreeSet<(u8, u16)> = duel.units.iter().map(|u| (u.side, u.card)).collect();
        let before = stored(STATE, &field.into_iter().collect::<Vec<_>>());
        // Interpreter runs of states 0x54..0x5E (Weevil's LP, the player's
        // LP or both set to 0): (side, card, stored, after).
        type Case = (Option<u8>, Vec<(u8, u16, u16, u16)>);
        let cases: [Case; 3] = [
            (
                Some(0),
                vec![(0, 221, 2, 6), (0, 392, 500, 524), (1, 397, 1000, 968)],
            ),
            (
                Some(1),
                vec![
                    (0, 392, 500, 484),
                    (1, 396, 16, 20),
                    (1, 397, 1000, 1040),
                    (1, 399, 14, 18),
                    (1, 400, 18, 22),
                    (1, 425, 112, 124),
                ],
            ),
            (None, vec![(0, 392, 500, 484)]),
        ];
        for (winner, expected) in cases {
            let mut d = duel.clone();
            d.outcome = Some(Outcome {
                winner,
                reason: EndReason::LifePoints,
            });
            let total = d.final_experience();
            let mut changed = Vec::new();
            for side in 0..2u8 {
                for (&card, change) in &total[usize::from(side)] {
                    let (rank, old) = before[&(side, card)];
                    let new = change.apply(old);
                    if new != old {
                        changed.push((side, card, old, new));
                    }
                    // State 0x5E recomputes the player's ranks only; none
                    // moves in these runs (392 stays rank 1).
                    let stored_rank = rank & 0x3FFF;
                    if side == 0 {
                        assert_eq!(
                            recompute_rank(card, new, stored_rank, &thresholds),
                            Some(stored_rank),
                            "{winner:?} {card}"
                        );
                    }
                }
            }
            assert_eq!(changed, expected, "{winner:?}");
        }
        // Rank lists (interpreter, experience poked first): Nekogal #2 at
        // 498, rank 0, wins +4 -> promoted to 1; the leader at 1005, rank 2,
        // loses 16 -> demoted to 1.
        assert_eq!(recompute_rank(221, 498 + 4, 0, &thresholds), Some(1));
        assert_eq!(recompute_rank(392, 1005 - 16, 2, &thresholds), Some(1));
        assert_eq!(recompute_rank(392, 499, 1, &thresholds), Some(1));
    }

    #[test]
    #[ignore = "Requires the user's executable and private PINE session states"]
    fn private_in_duel_experience_matches_the_interpreter() {
        const STATE: &str = "757-f2-setup.p2s";
        let (cards, _) = catalogue_and_thresholds();
        let base = duel_from(STATE, &cards);
        assert_eq!(base.active, 0);
        let red_eyes = base
            .units
            .iter()
            .find(|u| u.card == 7 && u.side == 0)
            .unwrap()
            .id;
        let run = |d: &Duel, unit: UnitId, to: (u8, u8)| {
            d.apply(
                0,
                &DuelCommand::Move {
                    unit,
                    path: vec![GridPos {
                        column: to.0,
                        row: to.1,
                    }],
                    face_up: false,
                    defence: None,
                },
                &cards,
            )
            .unwrap()
            .0
        };
        // Weevil's slot-5 unit at (1,6) replaced by a test defender; the
        // face-down Red-Eyes at (2,6) attacks it.
        let against = |card: u16, flags: u8| {
            let mut d = base.clone();
            d.units.retain(|u| !(u.side == 1 && u.slot == 5));
            d.units.push(placed(200, card, 1, 5, (1, 6), flags));
            deltas(&run(&d, red_eyes, (1, 6)))
        };
        assert_eq!(against(425, 0x14), [(0, 7, 20)]);
        assert_eq!(against(0, 0x14), [(1, 0, 28)]);
        assert_eq!(against(7, 0x14), [(0, 7, 18), (1, 7, 18)]);
        assert_eq!(against(425, 0x3C), [(0, 7, 14)]);
        assert!(against(0, 0x3C).is_empty());
        assert_eq!(against(757, 0x29), [(0, 7, 6), (1, 757, -4)]);
        assert_eq!(against(809, 0x29), [(0, 7, 6), (1, 809, -4)]);
        // A face-down Red-Eyes at (1,1) hits Weevil's leader at (1,0).
        let mut d = base.clone();
        d.units.retain(|u| !(u.side == 1 && u.slot == 5));
        d.units.push(placed(201, 7, 0, 4, (1, 1), 0));
        assert_eq!(
            deltas(&run(&d, UnitId(201), (1, 0))),
            [(0, 7, 20), (1, 397, -8)]
        );
        d.sides[1].lp = 100;
        assert_eq!(
            deltas(&run(&d, UnitId(201), (1, 0))),
            [(0, 7, 24), (1, 397, -8)]
        );
        // Ending the player's turn: Red-Eyes face-down +2, the committed
        // face-up Key Mace and Hoshiningen +4.
        let (ended, _) = base.apply(0, &DuelCommand::EndTurn, &cards).unwrap();
        assert_eq!(deltas(&ended), [(0, 7, 2), (0, 374, 4), (0, 383, 4)]);
    }
}
