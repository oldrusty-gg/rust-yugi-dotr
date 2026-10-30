//! Private oracle comparison of Seto's module: set
//! `ROSES_WEEVIL_ORACLE` to the port directory holding `cases-opp1/*.json`
//! (written by `tools/gen_opponent.py 1 ...`) and `data/`;
//! `ROSES_WEEVIL_CASE` filters case names. The cases carry the Weevil base
//! state's side records, so side 1's leader abilities (card 0 Blue-Eyes White Dragon
//! at rank 12, copied to the leader record by the generator) are taken from
//! the ability row of the private EE image next to the tools.

use super::super::tests::{
    load_json, memory_of, Case, CaseDecision, CaseDuel, CaseMemory, DumpTables, Oracle,
};
use super::super::world::World;
use super::super::{decide_raw, sync, Decision, FusionData, Opponent};
use super::Seto;
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
        .expect("Seto oracle leader ability row is outside the private EE image");
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
fn private_seto_oracle_cases_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let image = std::fs::read(o.root.join("tools/008-activate.ee"))
        .expect("Seto oracle requires the private EE image");
    let tables = WithDeckCosts {
        tables: &o.tables,
        image: &image,
    };
    let (mut ok, mut bad) = (0, Vec::new());
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    for path in o.files("cases-opp1") {
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
        match decide_raw::<Seto, _, _>(&duel, &mut memory, &o.cards, &tables, case.tier) {
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
        "seto oracle: {ok} match, {} differ, {refused_n} refused; matched action types {kinds:x?}",
        bad.len()
    );
    for (why, names) in &refused {
        eprintln!("REFUSED {} x {why}: {:?}", names.len(), names);
    }
    for b in bad.iter().take(40) {
        eprintln!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} Seto oracle cases differ", bad.len());
}

#[derive(serde::Deserialize)]
struct HelperProbe {
    name: String,
    duel: CaseDuel,
    tier: i16,
    opponent: OpponentInfo,
    call: String,
    args: Vec<i32>,
    memory: CaseMemory,
    expected: i32,
    memory_after: Option<CaseMemory>,
    card_after: Option<u16>,
    decision: Option<CaseDecision>,
    mark_acted: Option<u16>,
}

fn helper_probes(directory: &str) {
    let Some(o) = Oracle::load() else {
        return;
    };
    let image = std::fs::read(o.root.join("tools/008-activate.ee"))
        .expect("Seto helper oracle requires its private EE image");
    let tables = WithDeckCosts {
        tables: &o.tables,
        image: &image,
    };
    let mut matched = BTreeMap::<String, usize>::new();
    let mut refused = BTreeMap::<String, Vec<String>>::new();
    let mut differences = Vec::new();
    for path in o.files(directory) {
        let p: HelperProbe = load_json(&path);
        let mut duel = o.duel(&p.duel);
        duel.sides[1].leader_abilities = abilities(&image, &p.opponent);
        let w = match World::new(&duel, &o.cards, &tables, p.tier) {
            Ok(world) => world,
            Err(DuelError::Unsupported(why)) => {
                refused.entry(why).or_default().push(p.name.clone());
                continue;
            }
            Err(error) => {
                differences.push(format!("{}: world error {error}", p.name));
                continue;
            }
        };
        let mut memory = memory_of(&p.memory);
        let mut decision = Decision::new();
        let mut virtual_unit = None;
        let a = &p.args;
        let result = match p.call.as_str() {
            "mode" => super::mode(&w).map(i32::from),
            "hidden_value" => Seto::hidden_value(&w, a[0]),
            "f117cb0" => super::f117cb0(&w, a[0]),
            "f117e20" => super::f117e20(&w, a[0]),
            "f117ed0" => super::f117ed0(&w, a[0]),
            "f117f50" => super::f117f50(&w, a[0]),
            "goal_cell" => Ok(super::goal_cell(a[0], a[1])),
            "assign" => super::assign(&w, &mut memory.records[0], a[0]).map(|()| 0),
            "monster" => super::monster(&w, &mut memory.records[0], a[0], a[1]).map(|()| 0),
            "support" => super::support(&w, &mut memory.records[0]).map(|()| 0),
            "effect_monster" => super::effect_monster(&w, &mut memory.records[0], a[0]).map(|()| 0),
            "plan_c" => super::executors::plan_c(
                &w,
                &mut memory.records[0],
                a[0],
                &mut decision,
                a[1] as u8,
            )
            .map(|()| 0),
            "trap_move" => super::super::darkness::trap_move::<Seto, _, _>(
                &w,
                &mut memory.records[0],
                a[0],
                &mut decision,
                a[1] as u8,
                3,
            ),
            "reveal" => {
                super::reveal::reveal(&w, a[0], a[1], &memory.records[0]).map(|value| value as i32)
            }
            "special_reveal" => super::reveal::special_reveal(&w, a[0], a[1], &memory.records[0])
                .map(|value| value as i32),
            "ritual_ok" => super::reveal::ritual_ok(&w, a[0], a[1]),
            "center_reveal" => Ok(super::reveal::center_reveal(&w, a[0])),
            "control_target" => super::reveal::control_target(&w, a[0]),
            "f112c90" => super::reveal::f112c90(&w, a[0], a[1], a[2] as u32),
            "global_mask" => super::reveal::global_mask(&w, a[0], a[1], a[2] as u32),
            "f112ed0" => {
                let unit = super::super::summon::virt(&w, a[1]);
                let entry = unit.entries[0].expect("Seto f112ed0 probe requires an effect entry");
                super::reveal::f112ed0(&w, a[0], entry)
            }
            "actor" => {
                super::super::choose_actor::<Seto, _, _>(&w, &mut memory, a[0], &mut decision)
            }
            "attack" => super::executors::attack(
                &w,
                &mut memory.records[0],
                a[0],
                &mut decision,
                a[1] as u8,
            ),
            "go_to_cell" => super::super::exec::go_to_cell::<Seto, _, _>(
                &w,
                &mut memory.records[0],
                a[0],
                &mut decision,
                a[1] as u8,
                a[2] != 0,
            )
            .map(|()| 0),
            "retreat" => super::super::exec::retreat::<Seto, _, _>(
                &w,
                &mut memory.records[0],
                a[0],
                &mut decision,
                a[1] as u8,
            )
            .map(|()| 0),
            "category" => super::summon::category(&w, a[0], &mut virtual_unit),
            "cell" => super::summon::cell(&w, a[0], &super::super::summon::virt(&w, a[1]), a[2]),
            // Retain the historical fixture name: 0x115080 selects class-4
            // counter traps, rather than terrain-changing field spells.
            "field_spell" => {
                super::summon::priority_trap(&w, a[0], &mut decision, &mut virtual_unit)
            }
            "card_list" => super::summon::card_list(
                &w,
                a[0],
                &mut decision,
                &Some(super::super::summon::virt(&w, a[1])),
            ),
            "summon" => super::summon::summon(&w, a[0], a[1], &mut decision),
            "summon_monster" => {
                virtual_unit = Some(super::super::summon::virt(&w, 999));
                super::summon::monster(&w, a[0], &mut decision, &mut virtual_unit)
            }
            call => panic!("unknown Seto helper probe {call}"),
        };
        match result {
            Err(DuelError::Unsupported(why)) => {
                refused.entry(why).or_default().push(p.name.clone());
            }
            Err(error) => differences.push(format!("{}: error {error}", p.name)),
            Ok(value) => {
                let decision_same = p.decision.as_ref().is_none_or(|e| {
                    decision.key == e.key
                        && decision.kind == e.kind
                        && decision.target == e.target
                        && decision.hand == e.hand
                        && decision.reveal == e.reveal
                        && decision.position == e.position
                        && decision.mark_acted == p.mark_acted
                });
                let memory_same = p
                    .memory_after
                    .as_ref()
                    .is_none_or(|expected| memory == memory_of(expected));
                let card_same = p
                    .card_after
                    .is_none_or(|card| virtual_unit.as_ref().map(|unit| unit.card) == Some(card));
                if value != p.expected || !decision_same || !memory_same || !card_same {
                    differences.push(format!(
                        "{}: value {value} vs {}; decision {decision:?} vs {:?}; memory {:?} vs {:?}; virtual card {:?} vs {:?}",
                        p.name, p.expected,
                        p.decision.as_ref().map(|e| (e.key, e.kind, e.target, e.hand, e.reveal, e.position, p.mark_acted)),
                        (!memory_same).then_some(&memory),
                        (!memory_same).then(|| p.memory_after.as_ref().map(memory_of)),
                        virtual_unit.as_ref().map(|unit| unit.card), p.card_after
                    ));
                } else {
                    *matched.entry(p.call).or_default() += 1;
                }
            }
        }
    }
    eprintln!(
        "Seto {directory} probes: {matched:?}; {} differ; {} refused",
        differences.len(),
        refused.values().map(Vec::len).sum::<usize>()
    );
    for (why, names) in refused {
        eprintln!("REFUSED {} x {why}: {names:?}", names.len());
    }
    for difference in differences.iter().take(40) {
        eprintln!("DIFF {difference}");
    }
    assert!(differences.is_empty(), "Seto original helper probes differ");
}

#[test]
fn private_seto_executor_probes_match_the_original() {
    helper_probes("probes/codex-seto-executors");
}

#[test]
fn private_seto_summon_probes_match_the_original() {
    helper_probes("probes/codex-seto-summon");
}

#[test]
fn private_seto_reveal_probes_match_the_original() {
    helper_probes("probes/codex-seto-reveal");
}

#[test]
fn private_seto_planning_probes_match_the_original() {
    helper_probes("probes/codex-seto-planning");
}
