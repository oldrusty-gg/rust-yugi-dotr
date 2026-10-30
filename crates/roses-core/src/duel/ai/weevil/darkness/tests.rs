//! Private oracle comparison of Darkness-ruler's module: set
//! `ROSES_WEEVIL_ORACLE` to the port directory holding `cases-opp7/*.json`
//! (written by `tools/gen_opponent.py 7 ...`) and `data/`;
//! `ROSES_WEEVIL_CASE` filters case names. Side 1's leader abilities (card
//! 300 at rank 6) are taken from the ability row of the EE image next to
//! the tools, falling back to the documented list [2, 3, 10]; the deck
//! costs 0x278E30 compares come from `data/deck_costs.json` (byte 3 of the
//! card records), which the dump tables do not carry.

use super::super::tests::{load_json, memory_of, Case, DumpTables, Oracle};
use super::super::{decide_raw, sync, FusionData};
use super::Darkness;
use crate::duel::{DuelError, SideState};
use std::collections::BTreeMap;

/// Ability table 0x2C26B8 (one row of 20 u16 per card) in the EE image
/// `tools/008-activate.ee` (EE address = file offset).
const ABILITY_TABLE_EE: usize = 0x2C_26B8;

#[derive(serde::Deserialize)]
struct OpponentInfo {
    leader: u16,
    rank: i32,
}

#[derive(serde::Deserialize)]
struct OpponentCase {
    #[serde(flatten)]
    case: Case,
    opponent: OpponentInfo,
}

/// The dump tables plus the card records' byte 3.
struct DarknessTables<'a> {
    inner: &'a DumpTables,
    deck_costs: Vec<u8>,
}

impl FusionData for DarknessTables<'_> {
    fn fuse(&self, a: u16, b: u16) -> Option<u16> {
        self.inner.fuse(a, b)
    }
    fn equip(&self, monster: u16, card: u16) -> i32 {
        self.inner.equip(monster, card)
    }
    fn category(&self, card: u16) -> u8 {
        self.inner.category(card)
    }
    fn value(&self, card: u16) -> u16 {
        self.inner.value(card)
    }
    fn deck_cost(&self, card: u16) -> Option<u8> {
        self.deck_costs.get(usize::from(card)).copied()
    }
}

fn abilities(o: &Oracle, info: &OpponentInfo) -> Vec<u8> {
    if let Ok(image) = std::fs::read(o.root.join("tools/008-activate.ee")) {
        let at = ABILITY_TABLE_EE + usize::from(info.leader) * 40;
        if let Some(bytes) = image.get(at..at + 40) {
            let mut row = [0u16; 20];
            for (i, h) in row.iter_mut().enumerate() {
                *h = u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
            }
            return crate::duel::queries::leader_abilities(&row, info.rank);
        }
    }
    vec![2, 3, 10]
}

#[test]
fn private_darkness_oracle_cases_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let tables = DarknessTables {
        inner: &o.tables,
        deck_costs: load_json(&o.root.join("data/deck_costs.json")),
    };
    let (mut ok, mut bad) = (0, Vec::new());
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    for path in o.files("cases-opp7") {
        let pc: OpponentCase = load_json(&path);
        let case = pc.case;
        let mut duel = o.duel(&case.duel);
        let side = usize::from(case.side);
        let s: &mut SideState = &mut duel.sides[side];
        s.leader_abilities = abilities(&o, &pc.opponent);
        let mut memory = memory_of(&case.memory);
        sync(&mut memory, &duel, case.side);
        let synced = memory_of(&case.expected.synced);
        if memory != synced {
            bad.push(format!("{}: sync {:?} vs {:?}", case.name, memory, synced));
            continue;
        }
        match decide_raw::<Darkness, _, _>(&duel, &mut memory, &o.cards, &tables, case.tier) {
            Err(DuelError::Unsupported(why)) => {
                refused.entry(why).or_default().push(case.name.clone());
            }
            Err(e) => bad.push(format!("{}: error {e}", case.name)),
            Ok((ret, dec)) => {
                let e = &case.expected.decision;
                let want_mem = memory_of(&case.expected.memory_after);
                let same_d = dec.key == e.key
                    && dec.kind == e.kind
                    && dec.target == e.target
                    && dec.hand == e.hand
                    && dec.reveal == e.reveal
                    && dec.position == e.position;
                if ret != case.expected.ret || !same_d || memory != want_mem {
                    bad.push(format!(
                        "{}: ret {ret} vs {}; D {:?} vs key {:#x} type {:#x} target {:#x} hand {:?} reveal {} pos {}; memory {}",
                        case.name,
                        case.expected.ret,
                        dec,
                        e.key,
                        e.kind,
                        e.target,
                        e.hand,
                        e.reveal,
                        e.position,
                        if memory == want_mem {
                            "same".to_string()
                        } else {
                            format!("{:?} vs {:?}", memory.records, want_mem.records)
                        }
                    ));
                } else {
                    ok += 1;
                    *kinds.entry(dec.kind).or_default() += 1;
                }
            }
        }
    }
    let refused_n: usize = refused.values().map(Vec::len).sum();
    eprintln!(
        "darkness oracle: {ok} match, {} differ, {refused_n} refused; matched action types {kinds:x?}",
        bad.len()
    );
    for (why, names) in &refused {
        eprintln!("REFUSED {} x {why}: {:?}", names.len(), names);
    }
    for b in bad.iter().take(40) {
        eprintln!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} Darkness-ruler oracle cases differ", bad.len());
}
