//! Private oracle comparison of Necromancer's module: set
//! `ROSES_WEEVIL_ORACLE` to the port directory holding `cases-opp6/*.json`
//! (written by `tools/gen_opponent.py 6 ...`) and `data/`;
//! `ROSES_WEEVIL_CASE` filters case names. Side 1's leader abilities (card
//! 108 at rank 6) are taken from the ability row of the EE image next to
//! the tools, falling back to the documented list [1, 2, 3, 9, 10].

use super::super::tests::{load_json, memory_of, Case, Oracle};
use super::super::{decide_raw, sync};
use super::Necromancer;
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
    vec![1, 2, 3, 9, 10]
}

#[test]
fn private_necromancer_oracle_cases_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let (mut ok, mut bad) = (0, Vec::new());
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    let mut plans: BTreeMap<u8, usize> = BTreeMap::new();
    for path in o.files("cases-opp6") {
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
        match decide_raw::<Necromancer, _, _>(&duel, &mut memory, &o.cards, &o.tables, case.tier) {
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
                    if let Some(r) = want_mem.records.iter().find(|r| r.key == dec.key) {
                        *plans.entry(r.plan_a).or_default() += 1;
                    }
                }
            }
        }
    }
    let refused_n: usize = refused.values().map(Vec::len).sum();
    eprintln!(
        "necromancer oracle: {ok} match, {} differ, {refused_n} refused; matched action types {kinds:x?}; acting records' primary plans after {plans:x?}",
        bad.len()
    );
    for (why, names) in &refused {
        eprintln!("REFUSED {} x {why}: {:?}", names.len(), names);
    }
    for b in bad.iter().take(40) {
        eprintln!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} Necromancer oracle cases differ", bad.len());
}
