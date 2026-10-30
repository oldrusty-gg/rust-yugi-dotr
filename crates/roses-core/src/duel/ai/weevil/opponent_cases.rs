//! Private whole-controller oracle comparison shared by the white-route
//! opponents: set `ROSES_WEEVIL_ORACLE` to the port directory holding
//! `cases-opp<N>/*.json` (written by `tools/gen_white.py N ...`, the
//! controller on side 0 or 1) and `data/`; `ROSES_WEEVIL_CASE` filters
//! case names. The leader's abilities come from the ability row of the
//! private EE image next to the tools (the generator copies that row to
//! the leader record), as in York's and Manawyddan's tests.

use super::tests::{load_json, memory_of, Case, DumpTables, Oracle};
use super::{decide_raw_rng, sync, FusionData, Opponent};
use crate::duel::DuelError;
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
struct RngExpected {
    rng_before: u64,
    rng_after: u64,
}

#[derive(serde::Deserialize)]
struct RngCase {
    expected: RngExpected,
}

#[derive(serde::Deserialize)]
struct OpponentCase {
    #[serde(flatten)]
    case: Case,
    opponent: OpponentInfo,
    /// 0x2154C0 of both leaders (the saved ranks; cases written before the
    /// generic module's port lack it).
    #[serde(default)]
    leader_ranks: Option<[u8; 2]>,
    /// The other side's leader when a scenario gave it an ability row
    /// (cases without it keep that side's abilities empty).
    #[serde(default)]
    enemy_leader: Option<OpponentInfo>,
    /// Both sides' saved deck lists as their card blocks held them (the
    /// current deck, 0x215040 / 0x214960); cases written before
    /// REFUSALS-SWEEP.md §7 lack them (unknown).
    #[serde(default)]
    saved_decks: Option<[Vec<u16>; 2]>,
}

/// The dumped tables plus the card records' deck costs (byte 3 of
/// 0x2BE400 + 20·card) read from the private EE image (Joey's 0x154800
/// reads them; the dump predates such consumers).
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
            .then(|| self.image.get(0x2BE400 + usize::from(card) * 20 + 3).copied())
            .flatten()
    }
}

fn image(o: &Oracle) -> Vec<u8> {
    std::fs::read(o.root.join("tools/008-activate.ee"))
        .expect("the oracle needs its private EE image for leader abilities")
}

fn abilities(image: &[u8], info: &OpponentInfo) -> Vec<u8> {
    let at = ABILITY_TABLE_EE + usize::from(info.leader) * 40;
    let bytes = image
        .get(at..at + 40)
        .expect("the oracle EE image is missing the leader ability row");
    let mut row = [0u16; 20];
    for (i, h) in row.iter_mut().enumerate() {
        *h = u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]);
    }
    crate::duel::queries::leader_abilities(&row, info.rank)
}

/// Run every case of `dir` through `O`'s dispatch and compare the synced
/// memory, the return value, the action record and the memory after with
/// the original's. Panics on any difference; refusals are reported.
pub(super) fn check<O: Opponent>(dir: &str, label: &str) {
    let Some(o) = Oracle::load() else {
        return;
    };
    let image = image(&o);
    let tables = WithDeckCosts {
        tables: &o.tables,
        image: &image,
    };
    let (mut ok, mut bad) = (0, Vec::new());
    let mut refused: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kinds: BTreeMap<u8, usize> = BTreeMap::new();
    let mut sides = [0usize; 2];
    for path in o.files(dir) {
        let pc: OpponentCase = load_json(&path);
        let rng: RngCase = load_json(&path);
        let case = pc.case;
        let mut duel = o.duel(&case.duel);
        duel.rng = rng.expected.rng_before;
        if let Some(ranks) = pc.leader_ranks {
            duel.set_leader_ranks(ranks);
        }
        if let Some(decks) = &pc.saved_decks {
            duel.saved_decks = decks.to_vec();
        }
        let side = usize::from(case.side);
        duel.sides[side].leader_abilities = abilities(&image, &pc.opponent);
        if let Some(enemy) = &pc.enemy_leader {
            duel.sides[side ^ 1].leader_abilities = abilities(&image, enemy);
        }
        let mut memory = memory_of(&case.memory);
        sync(&mut memory, &duel, case.side);
        let synced = memory_of(&case.expected.synced);
        if memory != synced {
            bad.push(format!("{}: sync {:?} vs {:?}", case.name, memory, synced));
            continue;
        }
        match decide_raw_rng::<O, _, _>(&duel, &mut memory, &o.cards, &tables, case.tier) {
            Err(DuelError::Unsupported(why)) => {
                refused.entry(why).or_default().push(case.name.clone());
            }
            Err(e) => bad.push(format!("{}: error {e}", case.name)),
            Ok((ret, dec, rng_after)) => {
                let e = &case.expected.decision;
                let want_mem = memory_of(&case.expected.memory_after);
                let same_d = dec.key == e.key
                    && dec.kind == e.kind
                    && dec.target == e.target
                    && dec.hand == e.hand
                    && dec.reveal == e.reveal
                    && dec.position == e.position;
                if ret != case.expected.ret
                    || !same_d
                    || memory != want_mem
                    || rng_after != rng.expected.rng_after
                {
                    bad.push(format!(
                        "{}: ret {ret} vs {}; rng {rng_after:#x} vs {:#x}; D {:?} vs key {:#x} type {:#x} target {:#x} hand {:?} reveal {} pos {}; memory {}",
                        case.name,
                        case.expected.ret,
                        rng.expected.rng_after,
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
                    sides[side] += 1;
                    *kinds.entry(dec.kind).or_default() += 1;
                }
            }
        }
    }
    let refused_n: usize = refused.values().map(Vec::len).sum();
    eprintln!(
        "{label} oracle: {ok} match (side 0: {}, side 1: {}), {} differ, {refused_n} refused; matched action types {kinds:x?}",
        sides[0],
        sides[1],
        bad.len()
    );
    for (why, names) in &refused {
        eprintln!("REFUSED {} x {why}: {:?}", names.len(), names);
    }
    for b in bad.iter().take(40) {
        eprintln!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} {label} oracle cases differ", bad.len());
}

#[derive(serde::Deserialize)]
struct NearTrapProbes {
    opp: u8,
    case: String,
    /// [group, slot, col, row, see_all, event or null]
    calls: Vec<(i32, i32, i32, i32, i32, Option<u32>)>,
}

/// 0x24CC30 (the trap simulator, as 0x27F170 mode 0 calls it) against
/// `probes/neartrap/*.json` (tools/gen_neartrap.py): boards whose enemy
/// leader has abilities #13 / #15 / #16, every computer unit stepping to
/// every cell near that leader.
#[test]
fn private_near_trap_probes_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let image = image(&o);
    let tables = WithDeckCosts {
        tables: &o.tables,
        image: &image,
    };
    let (mut ok, mut events, mut bad, mut refused) = (0, 0, Vec::new(), 0);
    for path in o.files("probes/neartrap") {
        let p: NearTrapProbes = load_json(&path);
        let pc: OpponentCase =
            load_json(&o.root.join(format!("cases-opp{}/{}.json", p.opp, p.case)));
        let case = pc.case;
        let mut duel = o.duel(&case.duel);
        if let Some(ranks) = pc.leader_ranks {
            duel.set_leader_ranks(ranks);
        }
        if let Some(decks) = &pc.saved_decks {
            duel.saved_decks = decks.to_vec();
        }
        let side = usize::from(case.side);
        duel.sides[side].leader_abilities = abilities(&image, &pc.opponent);
        if let Some(enemy) = &pc.enemy_leader {
            duel.sides[side ^ 1].leader_abilities = abilities(&image, enemy);
        }
        let w = match super::world::World::new(&duel, &o.cards, &tables, case.tier) {
            Ok(w) => w,
            Err(_) => {
                refused += p.calls.len();
                continue;
            }
        };
        for (g, s, c, r, see_all, want) in p.calls {
            match super::plans::near_trap(&w, g, s, c, r, see_all) {
                Err(DuelError::Unsupported(_)) => refused += 1,
                Err(e) => bad.push(format!("{} {g} {s} ({c},{r}) {see_all}: error {e}", p.case)),
                Ok(v) if v == want => {
                    ok += 1;
                    events += usize::from(v == Some(0x71));
                }
                Ok(v) => bad.push(format!(
                    "{} {g} {s} ({c},{r}) {see_all}: {v:x?} vs {want:x?}",
                    p.case
                )),
            }
        }
    }
    eprintln!(
        "near-trap probes: {ok} match ({events} event 0x71), {} differ, {refused} refused",
        bad.len()
    );
    for b in bad.iter().take(40) {
        eprintln!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} near-trap probes differ", bad.len());
}
