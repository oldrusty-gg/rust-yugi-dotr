//! Private oracle comparison of Richard Slysheen of York's module: set
//! `ROSES_WEEVIL_ORACLE` to the port directory holding `cases-opp10/*.json`
//! (written by `tools/gen_opponent.py 10 ...`) and `data/`;
//! `ROSES_WEEVIL_CASE` filters case names. The cases carry the Weevil base
//! state's side records, so side 1's leader abilities (card 210 Battle
//! Steer at rank 12, copied to the leader record by the generator) are
//! taken from the ability row of the private EE image next to the tools.
//! The helper probes in `probes/york-review` call the original functions
//! directly so a controller's actor selection cannot hide a branch.

use super::super::tests::{load_json, memory_of, Case, Oracle};
use super::super::{decide_raw, sync};
use super::York;
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
    let image = std::fs::read(o.root.join("tools/008-activate.ee"))
        .expect("York oracle needs its private EE image for leader abilities");
    let at = ABILITY_TABLE_EE + usize::from(info.leader) * 40;
    let bytes = image
        .get(at..at + 40)
        .expect("York oracle EE image is missing the leader ability row");
    let mut row = [0u16; 20];
    for (i, h) in row.iter_mut().enumerate() {
        *h = u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
    }
    crate::duel::queries::leader_abilities(&row, info.rank)
}

#[derive(serde::Deserialize)]
struct HelperProbe {
    name: String,
    duel: super::super::tests::CaseDuel,
    tier: i16,
    opponent: OpponentInfo,
    call: String,
    args: Vec<i32>,
    memory: super::super::tests::CaseMemory,
    expected: i32,
    memory_after: super::super::tests::CaseMemory,
    card_after: Option<u16>,
    decision: Option<super::super::tests::CaseDecision>,
    mark_acted: Option<u16>,
}

#[test]
fn private_york_helper_probes_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let mut counts = BTreeMap::new();
    for path in o.files("probes/york-review") {
        let p: HelperProbe = load_json(&path);
        let mut duel = o.duel(&p.duel);
        duel.sides[1].leader_abilities = abilities(&o, &p.opponent);
        let w = super::W::new(&duel, &o.cards, &o.tables, p.tier).unwrap();
        let mut memory = memory_of(&p.memory);
        let rec = &mut memory.records[0];
        let mut dec = super::super::Decision::new();
        let a = &p.args;
        let result = match p.call.as_str() {
            "mode" => Ok(i32::from(super::mode(&w))),
            "front_row" => Ok(super::front_row(&w, a[0])),
            "lane_cell" => Ok(super::lane_cell(&w, a[0])),
            "strongest_threat" => super::strongest_threat(&w, a[0]),
            "reveal" => super::reveal(&w, a[0], a[1], rec).map(|v| v as i32),
            "ritual_ok" => super::ritual_ok(&w, a[0], a[1]),
            "after_passes" => {
                super::after_passes(&w, rec, a[0]);
                Ok(0)
            }
            "assign" => super::assign(&w, rec, a[0]).map(|()| 0),
            "support" => super::support(&w, rec).map(|()| 0),
            "monster" => super::monster(&w, rec, a[0], a[1]).map(|()| 0),
            "go_to_cell" => super::super::exec::go_to_cell::<York, _, _>(
                &w,
                rec,
                a[0],
                &mut dec,
                a[1] as u8,
                a[2] != 0,
            )
            .map(|()| 0),
            "retreat" => {
                super::super::exec::retreat::<York, _, _>(&w, rec, a[0], &mut dec, a[1] as u8)
                    .map(|()| 0)
            }
            "category" => {
                let mut u = None;
                let result = super::summon::category(&w, a[0], &mut u);
                if let Some(card) = p.card_after {
                    assert_eq!(u.as_ref().map(|u| u.card), Some(card), "{}", p.name);
                }
                result
            }
            "cell" => super::summon::cell(&w, a[0], &super::super::summon::virt(&w, a[1]), a[2]),
            call => panic!("unknown York probe {call}"),
        };
        assert_eq!(
            result.unwrap_or_else(|e| panic!("{}: {e}", p.name)),
            p.expected,
            "{}",
            p.name
        );
        assert_eq!(memory, memory_of(&p.memory_after), "{}", p.name);
        if let Some(e) = p.decision {
            assert_eq!(
                (
                    dec.key,
                    dec.kind,
                    dec.target,
                    dec.hand,
                    dec.reveal,
                    dec.position,
                    dec.mark_acted
                ),
                (
                    e.key,
                    e.kind,
                    e.target,
                    e.hand,
                    e.reveal,
                    e.position,
                    p.mark_acted
                ),
                "{}",
                p.name,
            );
        }
        *counts.entry(p.call).or_insert(0usize) += 1;
    }
    eprintln!("York direct original-function probes: {counts:?}");
}

#[test]
fn private_york_oracle_cases_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let (mut ok, mut bad) = (0, Vec::new());
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    for path in o.files("cases-opp10") {
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
        match decide_raw::<York, _, _>(&duel, &mut memory, &o.cards, &o.tables, case.tier) {
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
        "york oracle: {ok} match, {} differ, {refused_n} refused; matched action types {kinds:x?}",
        bad.len()
    );
    for (why, names) in &refused {
        eprintln!("REFUSED {} x {why}: {:?}", names.len(), names);
    }
    for b in bad.iter().take(40) {
        eprintln!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} York oracle cases differ", bad.len());
}
