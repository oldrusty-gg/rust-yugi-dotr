//! Private check: native practice start versus saved original states.
#[cfg(test)]
mod tests {
    use roses_compare::pine::{MemoryWatch, ReadWidth};
    use roses_compare::state::capture_state;
    use roses_core::practice::{practice_start, DeckInput};
    use roses_core::GridPos;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    // PAL structure addresses from private FINDINGS.md (instruction-derived).
    const UNIT_BASE: u32 = 0x2D_B990;
    const UNIT_GROUP: u32 = 0x3F4;
    const UNIT_STRIDE: u32 = 0x5C;
    const LIFE: [u32; 2] = [0x2D_AEDA, 0x2D_B22A];
    const CELLS: u32 = 0x2D_C570;
    const CELL_STRIDE: u32 = 28;

    fn watch(name: String, address: u32, width: ReadWidth) -> MemoryWatch {
        MemoryWatch {
            name,
            address,
            width,
        }
    }

    fn read(state: &str) -> BTreeMap<String, u64> {
        let root = PathBuf::from(std::env::var_os("ROSES_TEST_RECORDINGS").unwrap());
        let mut watches = Vec::new();
        for group in 0..3u32 {
            for slot in 0..6u32 {
                let unit = UNIT_BASE + group * UNIT_GROUP + slot * UNIT_STRIDE;
                watches.push(watch(
                    format!("u{group}.{slot}.card"),
                    unit + 0x3C,
                    ReadWidth::Bits16,
                ));
                watches.push(watch(
                    format!("u{group}.{slot}.col"),
                    unit + 0x40,
                    ReadWidth::Bits8,
                ));
                watches.push(watch(
                    format!("u{group}.{slot}.row"),
                    unit + 0x41,
                    ReadWidth::Bits8,
                ));
            }
        }
        for (side, &address) in LIFE.iter().enumerate() {
            watches.push(watch(format!("lp{side}"), address, ReadWidth::Bits16));
        }
        for cell in 0..49u32 {
            watches.push(watch(
                format!("terrain{cell}"),
                CELLS + cell * CELL_STRIDE + 0x16,
                ReadWidth::Bits8,
            ));
        }
        capture_state(root.join(state), &watches)
            .unwrap()
            .observations
            .into_iter()
            .map(|o| (o.name, o.value))
            .collect()
    }

    #[test]
    #[ignore = "Requires the user's executable and private saved states"]
    fn private_practice_start_matches_saved_original_states() {
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let decks: Vec<DeckInput> = roses_assets::parse_pal_decks(&elf)
            .unwrap()
            .decks
            .into_iter()
            .map(|d| DeckInput {
                index: d.index,
                leader_card: d.leader_card,
                leader_rank_raw: d.leader_rank_raw,
                cards: d.cards,
            })
            .collect();
        let start = practice_start(0, &decks).unwrap();
        // First practice screen: leaders, LP and terrain.
        let first = read("start-practice-run1/endpoint.p2s");
        for side in 0..2usize {
            let leader = start
                .board
                .document()
                .units
                .iter()
                .find(|u| usize::from(u.side) == side)
                .unwrap();
            assert_eq!(first[&format!("u{side}.0.card")], u64::from(leader.card_id));
            assert_eq!(
                first[&format!("u{side}.0.col")],
                u64::from(leader.position.column)
            );
            assert_eq!(
                first[&format!("u{side}.0.row")],
                u64::from(leader.position.row)
            );
            assert_eq!(
                first[&format!("lp{side}")],
                u64::from(start.board.document().life_points[side])
            );
        }
        for (cell, &terrain) in start.board.document().terrain.iter().enumerate() {
            assert_eq!(first[&format!("terrain{cell}")], u64::from(terrain));
        }
        assert!(start.board.unit_at(GridPos { column: 3, row: 3 }).is_none());
        // Tutorial hand screen: group-2 slots 1..=5 hold the first five cards.
        let hand = read("advance-tutorial-run1/endpoint.p2s");
        for (slot, &card) in start.sides[0].hand.iter().enumerate() {
            assert_eq!(
                hand[&format!("u2.{}.card", slot + 1)],
                u64::from(card),
                "hand slot {slot}"
            );
        }
    }
}
