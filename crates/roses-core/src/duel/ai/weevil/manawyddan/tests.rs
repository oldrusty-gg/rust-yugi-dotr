//! Private oracle comparison of Manawyddan fab Llyr's module: set
//! `ROSES_WEEVIL_ORACLE` to the port directory holding `cases-opp20/*.json`
//! (written by `tools/gen_opponent.py 20 ...`) and `data/`;
//! `ROSES_WEEVIL_CASE` filters case names. The cases carry the Weevil base
//! state's side records, so side 1's leader abilities (card 75 Skull Knight
//! at rank 12, copied to the leader record by the generator) are taken from
//! the ability row of the private EE image next to the tools.

use super::super::tests::{load_json, memory_of, Case, CaseDuel, DumpTables, Oracle};
use super::super::world::World;
use super::super::{decide_raw, sync, Decision, FusionData, Record};
use super::Manawyddan;
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

fn abilities(image: &[u8], info: &OpponentInfo) -> Vec<u8> {
    let at = ABILITY_TABLE_EE + usize::from(info.leader) * 40;
    let bytes = image
        .get(at..at + 40)
        .expect("Manawyddan oracle leader ability row is outside the private EE image");
    let mut row = [0u16; 20];
    for (i, h) in row.iter_mut().enumerate() {
        *h = u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
    }
    crate::duel::queries::leader_abilities(&row, info.rank)
}

/// The dumped fusion oracle predates consumers of deck cost. Read those
/// records from the private image too, so plan 0xC is actually compared.
struct WithDeckCosts<'a> {
    tables: &'a DumpTables,
    image: &'a [u8],
}

impl FusionData for WithDeckCosts<'_> {
    fn fuse(&self, a: u16, b: u16) -> Option<u16> {
        self.tables.fuse(a, b)
    }
    fn equip(&self, monster: u16, card: u16) -> i32 {
        self.tables.equip(monster, card)
    }
    fn category(&self, card: u16) -> u8 {
        self.tables.category(card)
    }
    fn value(&self, card: u16) -> u16 {
        self.tables.value(card)
    }
    fn deck_cost(&self, card: u16) -> Option<u8> {
        (card < 854)
            .then(|| {
                self.image
                    .get(0x2BE400 + usize::from(card) * 20 + 3)
                    .copied()
            })
            .flatten()
    }
}

#[test]
fn private_manawyddan_oracle_cases_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let image = std::fs::read(o.root.join("tools/008-activate.ee"))
        .expect("Manawyddan oracle requires the private EE image");
    let tables = WithDeckCosts {
        tables: &o.tables,
        image: &image,
    };
    let (mut ok, mut bad) = (0, Vec::new());
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    for path in o.files("cases-opp20") {
        let pc: OpponentCase = load_json(&path);
        let case = pc.case;
        let mut duel = o.duel(&case.duel);
        let side = usize::from(case.side);
        let s: &mut SideState = &mut duel.sides[side];
        s.leader_abilities = abilities(&image, &pc.opponent);
        let mut memory = memory_of(&case.memory);
        sync(&mut memory, &duel, case.side);
        let synced = memory_of(&case.expected.synced);
        if memory != synced {
            bad.push(format!("{}: sync {:?} vs {:?}", case.name, memory, synced));
            continue;
        }
        match decide_raw::<Manawyddan, _, _>(&duel, &mut memory, &o.cards, &tables, case.tier) {
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
        "manawyddan oracle: {ok} match, {} differ, {refused_n} refused; matched action types {kinds:x?}",
        bad.len()
    );
    for (why, names) in &refused {
        eprintln!("REFUSED {} x {why}: {:?}", names.len(), names);
    }
    for b in bad.iter().take(40) {
        eprintln!("DIFF {b}");
    }
    assert!(
        bad.is_empty(),
        "{} Manawyddan fab Llyr oracle cases differ",
        bad.len()
    );
}

#[derive(serde::Deserialize)]
struct Probe {
    name: String,
    duel: CaseDuel,
    tier: i16,
    opponent: OpponentInfo,
    function: String,
    args: Vec<i32>,
    expected: Vec<i32>,
}

/// Function-level cases run the game's code separately for narrow branches
/// which may never be selected by a full controller decision. The private
/// generator records returned values and mutated plans, not Rust expectations.
#[test]
fn private_manawyddan_branch_probes_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let image = std::fs::read(o.root.join("tools/008-activate.ee"))
        .expect("Manawyddan oracle requires the private EE image");
    let tables = WithDeckCosts {
        tables: &o.tables,
        image: &image,
    };
    let mut matched = BTreeMap::<String, usize>::new();
    let mut refused = Vec::new();
    let mut differences = Vec::new();
    for path in o.files("probes/opp20") {
        let p: Probe = load_json(&path);
        let mut duel = o.duel(&p.duel);
        duel.sides[1].leader_abilities = abilities(&image, &p.opponent);
        let w = World::new(&duel, &o.cards, &tables, p.tier).unwrap();
        let a = &p.args;
        let result = match p.function.as_str() {
            "goal_cell" => Ok(vec![super::goal_cell(a[0], a[1])]),
            "support_cell" => Ok(vec![super::support_cell(a[0])]),
            "reveal" => {
                super::reveal(&w, a[0], a[1], &Record::default()).map(|value| vec![value as i32])
            }
            "graveyard_pick" => super::graveyard_pick(&w, a[0]).map(|value| vec![value]),
            "ritual_ok" => super::ritual_ok(&w, a[0], a[1]).map(|value| vec![value]),
            "attack" => {
                let mut rec = Record {
                    key: a[0] as u16,
                    plan_a: 2,
                    target_a: a[1] as u16,
                    ..Record::default()
                };
                let mut decision = Decision::new();
                super::exec::attack::<Manawyddan, _, _>(&w, &mut rec, 1, &mut decision, 1).map(
                    |ret| {
                        vec![
                            ret,
                            i32::from(decision.key),
                            i32::from(decision.kind),
                            i32::from(decision.target),
                            decision.reveal as i32,
                            i32::from(decision.position),
                            i32::from(rec.plan_b),
                            i32::from(rec.plan_a),
                            i32::from(rec.target_b),
                            i32::from(rec.target_a),
                        ]
                    },
                )
            }
            "assign" => {
                let mut rec = Record {
                    key: a[0] as u16,
                    order: a[1] as u16,
                    plan_b: a[2] as u8,
                    plan_a: a[3] as u8,
                    target_b: a[4] as u16,
                    target_a: a[5] as u16,
                    bits: a[6] as u8,
                };
                super::assign(&w, &mut rec).map(|()| {
                    vec![
                        i32::from(rec.key),
                        i32::from(rec.order),
                        i32::from(rec.plan_b),
                        i32::from(rec.plan_a),
                        i32::from(rec.target_b),
                        i32::from(rec.target_a),
                        i32::from(rec.bits),
                    ]
                })
            }
            _ => panic!("unknown Manawyddan probe {}", p.function),
        };
        match result {
            Ok(actual) if actual == p.expected => {
                *matched.entry(p.function).or_default() += 1;
            }
            Ok(actual) => differences.push(format!(
                "{} {}: {actual:?} vs {:?}",
                p.name, p.function, p.expected
            )),
            Err(DuelError::Unsupported(why)) => refused.push(format!("{}: {why}", p.name)),
            Err(error) => differences.push(format!("{}: {error}", p.name)),
        }
    }
    eprintln!(
        "Manawyddan branch probes: {matched:?}, {} differ, {} refused",
        differences.len(),
        refused.len()
    );
    for refusal in refused {
        eprintln!("REFUSED {refusal}");
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
