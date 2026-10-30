//! Private check: the engine's Destiny Draw classifier (`Duel::destiny_situation`,
//! 0x270900 for a human side) against the game's own function on the T9 / T10 /
//! T13 boards dumped by `claude-effects/tools/special-draw/t18_dump.py`.
#[cfg(test)]
mod tests {
    use roses_core::duel::{Duel, DuelUnit, SideState, UnitRecord};
    use roses_core::{GridPos, UnitId};
    use serde_json::Value;

    #[test]
    #[ignore = "Requires the user's executable (ROSES_TEST_ELF_PATH) and the T18 board dump (ROSES_DESTINY_BOARDS)"]
    fn private_destiny_classifier_matches_the_t18_boards() {
        let elf = std::fs::read(std::env::var_os("ROSES_TEST_ELF_PATH").unwrap()).unwrap();
        let cards = crate::duel_catalogue::build(&elf).unwrap();
        let boards: Vec<Value> = serde_json::from_slice(
            &std::fs::read(std::env::var_os("ROSES_DESTINY_BOARDS").unwrap()).unwrap(),
        )
        .unwrap();
        let n = |v: &Value| v.as_u64().unwrap();
        let (mut ok, mut bad) = (0, Vec::new());
        for b in &boards {
            let mut units = Vec::new();
            let mut kinds = Vec::new();
            for (i, u) in b["units"].as_array().unwrap().iter().enumerate() {
                let f = n(&u["f57"]) as u8;
                let id = UnitId(i as u16 + 2);
                kinds.push((id, n(&u["card"]) as u16, n(&u["kind"]) as u8));
                units.push(DuelUnit {
                    id,
                    card: n(&u["card"]) as u16,
                    side: n(&u["side"]) as u8,
                    slot: n(&u["slot"]) as u8,
                    position: GridPos { column: n(&u["col"]) as u8, row: n(&u["row"]) as u8 },
                    face_up: f & 4 != 0,
                    defence: f & 8 != 0,
                    committed_face_up: f & 0x10 != 0,
                    committed_defence: f & 0x20 != 0,
                    acted: f & 1 != 0,
                    spellbind: n(&u["b49"]) as u8,
                    summon_order: n(&u["order"]) as u8,
                    attack_bonus: n(&u["atkd"]) as u16 as i16,
                    defense_bonus: n(&u["defd"]) as u16 as i16,
                    spellbind_fresh: n(&u["f58"]) as u8 & 8 != 0,
                    evo_counter: n(&u["b4b"]) as u8,
                    stolen: f & 2 != 0,
                    discount: n(&u["b48"]) as u8,
                    stolen_by: None,
                });
            }
            let side = |s: usize| SideState {
                lp: n(&b["lp"][s]) as u16,
                stars: 0,
                can_summon: true,
                hand: if s == 0 {
                    b["hand"].as_array().unwrap().iter().map(|h| n(h) as u16).filter(|&h| h != 999).collect()
                } else {
                    vec![]
                },
                deck: vec![],
                leader_abilities: vec![3],
                summons: 0,
                graveyard: vec![],
                destiny: 0,
            };
            let terrain = b["terrain"].as_array().unwrap().iter().map(|t| n(t) as u8).collect();
            let mut duel = Duel::from_observed(terrain, units, [side(0), side(1)], 2, 0, 0, &cards).unwrap();
            // The scenario rewrites the leader's kind byte (+0x3E).
            for (id, card, kind) in kinds {
                use roses_core::duel::CardData;
                if cards.card(card).map(|c| c.kind) != Some(kind) {
                    duel.records.insert(id.0, UnitRecord { kind: Some(kind), ..Default::default() });
                }
            }
            let mine = match duel.destiny_situation(0, &cards) {
                Ok(Some(v)) => i64::from(v),
                Ok(None) if b["lp"][0].as_u64().unwrap() < 250 => 67,
                Ok(None) => -1,
                Err(e) => {
                    bad.push(format!("{}: refused: {e}", b["label"]));
                    continue;
                }
            };
            let game = b["game"].as_i64().unwrap();
            let game = if game == 6 || game == 7 { 67 } else { game };
            if mine == game {
                ok += 1;
            } else {
                bad.push(format!("{}: engine {mine} game {game}", b["label"]));
            }
        }
        println!("{ok} of {} boards agree", boards.len());
        for line in &bad {
            println!("  {line}");
        }
        assert!(bad.is_empty());
    }
}
