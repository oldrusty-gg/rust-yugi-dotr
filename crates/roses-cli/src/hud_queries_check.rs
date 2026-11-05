//! Private check: the duel-rule getters of `roses_core::duel::queries`
//! against the values the game's own getters returned on savestates
//! (the project notes,
//! produced by `tools/export_getters.py` in the EE interpreter).
#[cfg(test)]
mod tests {
    use roses_compare::pine::{MemoryWatch, ReadWidth};
    use roses_compare::state::capture_state;
    use roses_core::duel::queries::{self, Context, Query, Record, Stats};
    use roses_core::duel::{CardData, Catalogue, Duel, DuelUnit, SideState};
    use roses_core::{GridPos, UnitId};
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    const SIDE: u32 = 0x2D_ABA0;
    const SIDE_STRIDE: u32 = 0x350;
    const UNIT: u32 = 0x2D_B990;
    const CELLS: u32 = 0x2D_C570;
    /// 0x203380 pad words: 0x2E8E40 + 92·port + 0x200.
    const PAD: u32 = 0x2E_9040;

    fn env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")))
    }

    /// Byte-wise reads of one savestate's EE RAM through `capture_state`.
    struct Ram {
        path: PathBuf,
        bytes: BTreeMap<u32, u8>,
    }

    impl Ram {
        fn load(&mut self, ranges: &[(u32, u32)]) {
            let want: BTreeSet<u32> = ranges
                .iter()
                .flat_map(|&(a, n)| a..a + n)
                .filter(|a| !self.bytes.contains_key(a))
                .collect();
            let want: Vec<u32> = want.into_iter().collect();
            for chunk in want.chunks(4096) {
                let watches: Vec<MemoryWatch> = chunk
                    .iter()
                    .map(|&address| MemoryWatch {
                        name: format!("{address:x}"),
                        address,
                        width: ReadWidth::Bits8,
                    })
                    .collect();
                for o in capture_state(&self.path, &watches).unwrap().observations {
                    self.bytes.insert(o.address, o.value as u8);
                }
            }
        }
        fn b(&self, a: u32) -> u8 {
            self.bytes[&a]
        }
        fn h(&self, a: u32) -> u16 {
            u16::from_le_bytes([self.b(a), self.b(a + 1)])
        }
        fn w(&self, a: u32) -> u32 {
            u32::from_le_bytes([self.b(a), self.b(a + 1), self.b(a + 2), self.b(a + 3)])
        }
    }

    fn record(group: u32, slot: u32) -> u32 {
        UNIT + group * 0x3F4 + slot * 0x5C
    }

    fn side_base(side: u32) -> u32 {
        SIDE + side * SIDE_STRIDE
    }

    /// 0x2154C0(side, card): u16 & 0x3FFF of the side's saved card table
    /// (block 0 when side == [0x312B50]); card 671 counts as 999, ids >= 683
    /// and sides outside 0..2 have none (−1). Same as `hud_check`.
    fn rank_address(ram: &Ram, side: u8, card: u16) -> Option<u32> {
        let card = if card == 671 { 999 } else { card };
        (side < 2 && card < 683).then(|| {
            let block = u32::from(side != ram.b(0x31_2B50));
            0x2F_2C10 + block * 0xFFA0 + u32::from(card) * 0x4C + 0x48
        })
    }

    fn rank(ram: &Ram, side: u8, card: u16) -> i32 {
        rank_address(ram, side, card).map_or(-1, |a| i32::from(ram.h(a) & 0x3FFF))
    }

    /// The engine duel of a savestate, as `duel_check::duel_from` builds it,
    /// plus the leader abilities 0x1E1E80 finds (each leader record's
    /// ability row at the leader's rank). Savestate 600-y11 (a leader
    /// screen) holds a single record, side 0's leader; the engine needs two
    /// leaders, so a stand-in without abilities takes side 1's leader cell.
    /// No getter of the lone record reads it (no ability 14, nothing in
    /// range of it). Returns the duel and whether a stand-in was added.
    fn duel_from(ram: &Ram, cards: &Catalogue) -> (Duel, bool) {
        let terrain = (0..49).map(|c| ram.b(CELLS + c * 28 + 0x16)).collect();
        let mut units = Vec::new();
        for g in 0..2u32 {
            for slot in 0..11u32 {
                let u = record(g, slot);
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
        let mut stand_in = false;
        for side in 0..2u8 {
            if !units.iter().any(|u| u.side == side && u.slot == 0) {
                stand_in = true;
                let mut leader = units.first().cloned().expect("one record");
                leader.id = UnitId(u16::from(side) * 11);
                leader.side = side;
                leader.slot = 0;
                leader.position = roses_core::duel::LEADER_CELLS[usize::from(side)];
                units.push(leader);
            }
        }
        let side = |side: u32| {
            let s = side_base(side);
            let leader = record(side, 0);
            let row: [u16; 20] = std::array::from_fn(|i| ram.h(leader + 8 + 2 * i as u32));
            SideState {
                lp: ram.h(s + 0x33A),
                stars: ram.b(s + 0x341),
                can_summon: ram.b(s + 0x346) & 1 != 0,
                // Hand words carry the marking order in bits 12..15 (the
                // engine keeps the marks in `ui::HandMenu`).
                hand: (0..5)
                    .map(|i| ram.h(s + 0x324 + 2 * i) & 0xFFF)
                    .filter(|&c| c != 999)
                    .collect(),
                deck: (0..u32::from(ram.b(s + 0x230)))
                    .map(|i| ram.h(s + 0x1E0 + 2 * i))
                    .collect(),
                leader_abilities: if ram.h(leader + 0x3C) == 999 {
                    Vec::new()
                } else {
                    queries::leader_abilities(
                        &row,
                        rank(ram, ram.b(leader + 0x44), ram.h(leader + 0x3C)),
                    )
                },
                summons: ram.b(s + 0x343),
                // +0x344: the Destiny Draw byte (SPECIAL-DRAW.md section 1).
                destiny: ram.b(s + 0x344),
                graveyard: (0..u32::from(ram.b(s + 0x322)))
                    .map(|i| ram.h(s + 0x232 + 2 * i))
                    .collect(),
            }
        };
        let duel = Duel::from_observed(
            terrain,
            units,
            [side(0), side(1)],
            ram.b(0x2D_B8F3),
            ram.b(0x2D_B8F2),
            ram.b(0x2D_B8F4),
            cards,
        )
        .unwrap();
        (duel, stand_in)
    }

    fn context(ram: &Ram) -> Context {
        let mut control_changed = Vec::new();
        for g in 0..2u32 {
            for slot in 0..11u32 {
                let u = record(g, slot);
                if ram.h(u + 0x3C) != 999 && ram.b(u + 0x57) & 2 != 0 {
                    control_changed.push(UnitId((g * 11 + slot) as u16));
                }
            }
        }
        Context {
            option_28: ram.w(0x2D_AB00),
            option_29: ram.w(0x2D_AB04),
            controllers: [ram.b(side_base(0) + 0x33C), ram.b(side_base(1) + 0x33C)],
            control_changed,
            r2_held: [ram.h(PAD) & 2 != 0, ram.h(PAD + 92) & 2 != 0],
        }
    }

    fn stats_json(s: Option<Stats>) -> Value {
        s.map_or(Value::Null, |s| {
            serde_json::json!([s.attack, s.defence, s.terrain_effect, s.allowance])
        })
    }

    #[derive(Default)]
    struct Tally {
        checked: BTreeMap<&'static str, usize>,
        failures: Vec<String>,
    }

    impl Tally {
        fn check(&mut self, what: &'static str, state: &str, at: &str, got: Value, want: &Value) {
            *self.checked.entry(what).or_default() += 1;
            if &got != want {
                self.failures
                    .push(format!("{state} {at} {what}: game {want}, port {got}"));
            }
        }
    }

    /// Recorded savestates: `golden-getters/` (`tools/export_getters.py`).
    #[test]
    #[ignore = "Requires the user's executable, private savestates and HUD getter results"]
    fn private_hud_queries_match_the_game_getters() {
        let states = compare_folder("golden-getters");
        assert!(states >= 34, "{states} savestates checked");
    }

    /// Savestates with patched records, leader rows, options, pad, cursor
    /// and A/B: `golden-getters-patched/` (`tools/export_getters_patched.py`
    /// ran the game's getters on the same RAM).
    #[test]
    #[ignore = "Requires the user's executable, private savestates and HUD getter results"]
    fn private_hud_queries_match_the_game_on_patched_states() {
        let states = compare_folder("golden-getters-patched");
        assert!(states >= 19, "{states} patched states checked");
    }

    /// Compares every file of the folder (a sibling of
    /// `ROSES_TEST_HUD_GOLDEN`); returns the number of states.
    fn compare_folder(folder: &str) -> usize {
        let elf = std::fs::read(env("ROSES_TEST_ELF_PATH")).unwrap();
        let cards = crate::duel_catalogue::build(&elf).unwrap();
        let getters_dir = env("ROSES_TEST_HUD_GOLDEN").parent().unwrap().join(folder);
        let session = env("ROSES_TEST_SESSION");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&getters_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        let mut tally = Tally::default();
        let mut states = 0;
        for file in &files {
            let golden: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
            let name = golden["savestate"].as_str().unwrap().to_string();
            let base = golden["base"].as_str().unwrap_or(&name).to_string();
            let path: PathBuf = [
                session.join(format!("{base}.p2s")),
                session
                    .parent()
                    .unwrap()
                    .join("checkpoints")
                    .join(format!("{base}.p2s")),
            ]
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| panic!("savestate {base}"));
            check_state(&path, &name, &golden, &cards, &mut tally);
            states += 1;
        }
        let summary: Vec<String> = tally
            .checked
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect();
        eprintln!(
            "HUD getters ({folder}): {states} states, {} values compared ({}), {} mismatches",
            tally.checked.values().sum::<usize>(),
            summary.join(", "),
            tally.failures.len()
        );
        assert!(tally.failures.is_empty(), "{}", tally.failures.join("\n"));
        assert_eq!(states, files.len());
        states
    }

    fn check_state(path: &Path, name: &str, golden: &Value, cards: &Catalogue, tally: &mut Tally) {
        // Patched states: the writes take the place of the savestate bytes.
        let mut bytes = BTreeMap::new();
        for write in golden["writes"].as_array().into_iter().flatten() {
            let address = write[0].as_u64().unwrap() as u32;
            for (i, v) in write[1].as_array().unwrap().iter().enumerate() {
                bytes.insert(address + i as u32, v.as_u64().unwrap() as u8);
            }
        }
        let patched = golden.get("writes").is_some();
        let mut ram = Ram {
            path: path.to_path_buf(),
            bytes,
        };
        ram.load(&[
            (SIDE, 2 * SIDE_STRIDE),
            (0x2D_B268, 8),
            (0x2D_B8D0, 0x40),
            (0x2D_AB00, 12),
            (0x31_2B50, 1),
            (PAD, 2 * 92),
            (UNIT, 3 * 0x3F4),
            (CELLS, 49 * 28),
        ]);
        let mut ranks = Vec::new();
        for g in 0..3 {
            for slot in 0..11 {
                let r = record(g, slot);
                if let Some(a) = rank_address(&ram, ram.b(r + 0x44), ram.h(r + 0x3C)) {
                    ranks.push((a, 2));
                }
            }
        }
        ram.load(&ranks);
        let (duel, stand_in) = duel_from(&ram, cards);
        if stand_in {
            eprintln!("{name}: one leader record; a stand-in leader was added");
        }
        let ctx = context(&ram);
        let q = Query::new(&duel, cards, &ctx);
        let rank_of = |side: u8, card: u16| rank(&ram, side, card);
        // The leader +0x48 the queries take as 0.
        for side in 0..2 {
            let at = format!("leader {side}");
            let got = Value::from(ram.b(record(side, 0) + 0x48));
            tally.check("leader_discount_zero", name, &at, got, &Value::from(0));
        }
        let d3c = ram.b(0x2D_B8DC);
        let s = side_base(u32::from(d3c));
        let cursor = GridPos {
            column: ram.b(s + 0x33D),
            row: ram.b(s + 0x33E),
        };
        let mut own = BTreeMap::new();
        for cell in 0..49u32 {
            let p = ram.w(CELLS + cell * 28);
            if p != 0 {
                own.insert(
                    p,
                    GridPos {
                        column: (cell % 7) as u8,
                        row: (cell / 7) as u8,
                    },
                );
            }
        }
        let record_at = |p: u32| -> Record {
            let (group, slot) = ((p - UNIT) / 0x3F4, (p - UNIT) % 0x3F4 / 0x5C);
            if group < 2 {
                Record::Field(
                    duel.unit(UnitId((group * 11 + slot) as u16))
                        .unwrap_or_else(|| panic!("{name}: no engine unit for {p:#X}")),
                )
            } else {
                Record::Hand {
                    side: ram.b(p + 0x44),
                    slot: ram.b(p + 0x45),
                    card: ram.h(p + 0x3C),
                }
            }
        };
        for (key, want) in golden["units"].as_object().unwrap() {
            let p = u32::from_str_radix(key.trim_start_matches("0x"), 16).unwrap();
            let r = record_at(p);
            let slot = ram.b(p + 0x45);
            tally.check(
                "hidden",
                name,
                key,
                Value::from(i32::from(q.hidden(r, d3c))),
                &want["hidden"],
            );
            tally.check(
                "effect",
                name,
                key,
                Value::from(i32::from(q.has_effect(r))),
                &want["effect"],
            );
            tally.check(
                "abilities",
                name,
                key,
                Value::from(q.abilities(r, rank_of)),
                &want["abilities"],
            );
            // Recorded as 0x268CA0(u) with a1 = 0, for slot > 0.
            tally.check(
                "legend",
                name,
                key,
                Value::from(i32::from(slot > 0 && q.legend_query(r, 0))),
                &want["legend"],
            );
            if !want["legend_d3c"].is_null() {
                tally.check(
                    "legend_d3c",
                    name,
                    key,
                    Value::from(i32::from(q.legend_query(r, d3c))),
                    &want["legend_d3c"],
                );
            }
            tally.check(
                "at_cursor",
                name,
                key,
                stats_json(q.stats(r, cursor)),
                &want["at_cursor"],
            );
            let at_own = own
                .get(&p)
                .map_or(Value::Null, |&cell| stats_json(q.stats(r, cell)));
            tally.check("at_own_cell", name, key, at_own, &want["at_own_cell"]);
        }
        // Attribute advantage of A = [0x2DB268] against B = [0x2DB26C].
        let (a, b) = (ram.w(0x2D_B268), ram.w(0x2D_B26C));
        let valid = |p: u32| p != 0 && ram.b(p + 0x45) < 11;
        let advantage = if valid(a) && valid(b) {
            let (sa, sb) = (
                (ram.b(a + 0x44), ram.b(a + 0x45)),
                (ram.b(b + 0x44), ram.b(b + 0x45)),
            );
            (q.controller(sa.0, sa.1) != q.controller(sb.0, sb.1)).then(|| q.advantage(sa, sb))
        } else {
            None
        };
        tally.check(
            "advantage",
            name,
            "A/B",
            advantage.map_or(Value::Null, Value::from),
            &golden["advantage"],
        );
        // Hand boxes: 0x270280 over the active side's marked hand words; the
        // left box 0x1E1B70(2, d4f + 1, 0) against the record's own +0x48.
        let active = ram.b(0x2D_B8F4);
        let hs = side_base(u32::from(active & 1));
        let words: Vec<u16> = (0..5).map(|i| ram.h(hs + 0x324 + 2 * i)).collect();
        let count = usize::from(ram.b(hs + 0x32E)).min(5);
        let mut marks = Vec::new();
        let mut index = 0;
        for (i, &word) in words.iter().enumerate() {
            if word & 0xFFF == 999 {
                continue;
            }
            if i < count && word & 0xF000 != 0 {
                marks.push(index);
            }
            index += 1;
        }
        let right = (patched || ram.b(0x2D_B8EE) != 0).then(|| q.marked_cost(&marks));
        tally.check(
            "hand_box_right",
            name,
            "",
            right.map_or(Value::Null, Value::from),
            &golden["hand_box_right"],
        );
        // 0x2703B0(side, card) of every hand card, as the hand record's +0x48.
        for d in golden["discounts"].as_array().into_iter().flatten() {
            let (side, card) = (d[0].as_u64().unwrap() as u8, d[1].as_u64().unwrap() as u16);
            let level = i32::from(cards.card(card).unwrap().level);
            let want = (level - d[2].as_i64().unwrap() as i32).clamp(0, 12);
            tally.check(
                "hand_card_cost",
                name,
                &format!("{side}/{card}"),
                Value::from(q.hand_card_cost(side, card)),
                &Value::from(want),
            );
        }
        // The left box against the hand record's stored +0x48 (only where
        // the record is as the game built it: the patches change leaders
        // after the hand records were made).
        let slot = u32::from(ram.b(0x2D_B8EF)) + 1;
        if slot < 11 && !patched {
            let r = record(2, slot);
            let card = ram.h(r + 0x3C);
            if ram.b(r + 0x45) < 11 && ram.w(r) != 0 {
                let level = i32::from(cards.card(card).unwrap().level);
                let want = (level - i32::from(ram.b(r + 0x48))).clamp(0, 12);
                let got = q.hand_card_cost(ram.b(r + 0x44), card);
                tally.check(
                    "hand_box_left",
                    name,
                    "",
                    Value::from(got),
                    &Value::from(want),
                );
            }
        }
    }
}
