use super::world::World;
use super::*;
use crate::duel::{CardInfo, DuelUnit, SideState, TempUnit};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Private oracle comparisons: set `ROSES_WEEVIL_ORACLE` to the directory
/// holding `data/` (cards.json, card_raw.json, fusions.json, equips.json),
/// `cases/*.json` and `probes/world/*.json` written by the private emulator
/// tools. Skipped when unset. `ROSES_WEEVIL_CASE` filters case names.
pub(super) struct DumpTables {
    fuse: HashMap<(u16, u16), u16>,
    equip: HashMap<(u16, u16), i32>,
    raw: Vec<(u8, u16)>,
}

impl FusionData for DumpTables {
    fn fuse(&self, a: u16, b: u16) -> Option<u16> {
        self.fuse.get(&(a.max(b), a.min(b))).copied()
    }
    fn equip(&self, monster: u16, card: u16) -> i32 {
        match self.equip.get(&(monster, card)) {
            Some(-2) => EQUIP_RANDOM,
            Some(&r) => r,
            None => -1,
        }
    }
    fn category(&self, card: u16) -> u8 {
        self.raw.get(usize::from(card)).map_or(0, |r| r.0)
    }
    fn value(&self, card: u16) -> u16 {
        self.raw.get(usize::from(card)).map_or(0, |r| r.1)
    }
}

#[derive(Clone, serde::Deserialize)]
pub(super) struct CaseDuel {
    terrain: Vec<u8>,
    units: Vec<DuelUnit>,
    sides: [SideState; 2],
    round: u8,
    first: u8,
    active: u8,
    /// Group-2 records ([`Duel::temp_units`]), as the oracle writes them.
    #[serde(default)]
    temp_units: Vec<Option<TempUnit>>,
}

#[derive(serde::Deserialize)]
pub(super) struct CaseMemory {
    records: Vec<[u16; 7]>,
    x1a1: u8,
}

#[derive(serde::Deserialize)]
pub(super) struct CaseDecision {
    pub key: u16,
    #[serde(rename = "type")]
    pub kind: u8,
    pub target: u16,
    pub hand: [u8; 5],
    pub reveal: u32,
    pub position: u8,
}

#[derive(serde::Deserialize)]
pub(super) struct Expected {
    pub synced: CaseMemory,
    pub ret: i32,
    pub decision: CaseDecision,
    pub memory_after: CaseMemory,
}

#[derive(serde::Deserialize)]
pub(super) struct Case {
    pub name: String,
    pub side: u8,
    pub duel: CaseDuel,
    pub tier: i16,
    pub memory: CaseMemory,
    pub expected: Expected,
}

pub(super) fn memory_of(m: &CaseMemory) -> Memory {
    assert_eq!(
        m.records.len(),
        22,
        "oracle memory must contain all 22 records"
    );
    let mut out = Memory {
        x1a1: m.x1a1,
        ..Memory::default()
    };
    for (r, v) in out.records.iter_mut().zip(&m.records) {
        *r = Record {
            key: v[0],
            order: v[1],
            plan_b: v[2] as u8,
            plan_a: v[3] as u8,
            target_b: v[4],
            target_a: v[5],
            bits: v[6] as u8,
        };
    }
    out
}

pub(super) fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The oracle directory and its card data, if configured.
pub(super) struct Oracle {
    pub root: PathBuf,
    pub cards: Vec<CardInfo>,
    pub tables: DumpTables,
}

impl Oracle {
    pub fn load() -> Option<Self> {
        let root = PathBuf::from(std::env::var_os("ROSES_WEEVIL_ORACLE")?);
        let cards: Vec<CardInfo> = load_json(&root.join("data/cards.json"));
        let raw: Vec<(u8, u16)> = load_json(&root.join("data/card_raw.json"));
        let mut fuse = HashMap::new();
        for [a, b, r] in load_json::<Vec<[u16; 3]>>(&root.join("data/fusions.json")) {
            fuse.entry((a, b)).or_insert(r);
        }
        let equip = load_json::<Vec<[i32; 3]>>(&root.join("data/equips.json"))
            .into_iter()
            .map(|[a, b, r]| ((a as u16, b as u16), r))
            .collect();
        Some(Self {
            root,
            cards,
            tables: DumpTables { fuse, equip, raw },
        })
    }
    pub fn case(&self, name: &str) -> Case {
        load_json(&self.root.join("cases").join(format!("{name}.json")))
    }
    pub fn duel(&self, c: &CaseDuel) -> Duel {
        let mut duel = Duel::from_observed(
            c.terrain.clone(),
            c.units.clone(),
            c.sides.clone(),
            c.round,
            c.first,
            c.active,
            &self.cards,
        )
        .unwrap();
        for (slot, p) in c.temp_units.iter().enumerate().take(11) {
            duel.temp_units[slot] = *p;
        }
        duel
    }
    /// Sorted json files in `dir` (relative to the root), filtered by
    /// `ROSES_WEEVIL_CASE`.
    pub fn files(&self, dir: &str) -> Vec<PathBuf> {
        let filter = std::env::var("ROSES_WEEVIL_CASE").ok();
        let directory = self.root.join(dir);
        oracle_files(&directory, filter.as_deref()).unwrap_or_else(|e| {
            panic!(
                "configured oracle case directory {}: {e}",
                directory.display()
            )
        })
    }
}

/// An explicitly configured oracle must have evidence to check. A missing or
/// empty directory is an input error, never a successful zero-case comparison.
fn oracle_files(directory: &Path, filter: Option<&str>) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = std::fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|p| p.extension().is_some_and(|e| e == "json"));
    if paths.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "no JSON cases",
        ));
    }
    paths.retain(|p| {
        filter.is_none_or(|f| {
            p.file_stem()
                .is_some_and(|s| s.to_string_lossy().contains(f))
        })
    });
    if paths.is_empty() {
        eprintln!(
            "No oracle cases in {} match filter {filter:?}",
            directory.display()
        );
    }
    paths.sort();
    Ok(paths)
}

#[test]
fn configured_oracle_rejects_missing_or_empty_evidence() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("roses-oracle-{}-{suffix}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    assert_eq!(
        oracle_files(&directory.join("absent"), None)
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    assert_eq!(
        oracle_files(&directory, None).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    let note = directory.join("notes.txt");
    std::fs::write(&note, "synthetic evidence test").unwrap();
    assert!(oracle_files(&directory, None).is_err());
    let a = directory.join("a.json");
    let b = directory.join("b.json");
    std::fs::write(&b, "{}").unwrap();
    std::fs::write(&a, "{}").unwrap();
    assert_eq!(
        oracle_files(&directory, None).unwrap(),
        [a.clone(), b.clone()]
    );
    assert_eq!(
        oracle_files(&directory, Some("a")).unwrap(),
        std::slice::from_ref(&a)
    );
    // A filter may intentionally omit this opponent while selecting another.
    assert!(oracle_files(&directory, Some("unmatched"))
        .unwrap()
        .is_empty());
    for path in [note, a, b] {
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
#[should_panic(expected = "oracle memory must contain all 22 records")]
fn oracle_memory_cannot_silently_default_missing_records() {
    memory_of(&CaseMemory {
        records: vec![[0; 7]; 21],
        x1a1: 0,
    });
}

#[test]
fn private_oracle_cases_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let (mut ok, mut bad, mut refused) = (0, Vec::new(), Vec::new());
    for path in o.files("cases") {
        let case: Case = load_json(&path);
        let duel = o.duel(&case.duel);
        let mut memory = memory_of(&case.memory);
        sync(&mut memory, &duel, case.side);
        let synced = memory_of(&case.expected.synced);
        if memory != synced {
            bad.push(format!("{}: sync {:?} vs {:?}", case.name, memory, synced));
            continue;
        }
        match decide_raw::<Weevil, _, _>(&duel, &mut memory, &o.cards, &o.tables, case.tier) {
            Err(e @ DuelError::Unsupported(_)) => refused.push(format!("{}: {e}", case.name)),
            Err(e) => bad.push(format!("{}: unexpected error {e}", case.name)),
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
                }
            }
        }
    }
    eprintln!(
        "weevil oracle: {ok} match, {} differ, {} refused",
        bad.len(),
        refused.len()
    );
    for b in &bad {
        eprintln!("DIFF {b}");
    }
    for r in &refused {
        eprintln!("REFUSED {r}");
    }
    assert!(bad.is_empty(), "{} oracle cases differ", bad.len());
}

#[derive(serde::Deserialize)]
struct Probes {
    case: String,
    calls: Vec<(String, Vec<i64>, serde_json::Value)>,
}

fn world_probe<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    f: &str,
    a: &[i64],
) -> R<serde_json::Value> {
    use serde_json::json;
    let i = |n: usize| a[n] as i32;
    let unit = |n: usize| {
        w.unit_k(i(n))
            .cloned()
            .ok_or(DuelError::InvalidCommand("probe unit"))
    };
    let b = |v: bool| json!(i32::from(v));
    Ok(match f {
        "252300" => json!(w.eff_atk(i(0), i(1), i(2), i(3))?),
        "252460" => json!(w.eff_def(i(0), i(1), i(2), i(3))?),
        "247e10" => json!(w.allowance(&unit(0)?, i(1), i(2))?),
        "251760" => b(w.crushed(&unit(0)?, i(1), i(2))?),
        "27fef0" => b(w.shielded(i(0), i(1), i(2))),
        "27d2f0" => b(w.can_step(i(0))?),
        "27e010" => b(w.may_act(i(0))),
        "27d500" => b(w.threatened(i(0), i(1), i(2) != 0)?),
        "26cb20.0" | "26cb20.1" => json!(w.trap_hook(i(0), i(1), i(2), i(3) != 0)?),
        "27f170.4" | "27f170.5" | "27f170.6" => {
            json!(w.classify(i(0), i(1), i(4), i(5) != 0)?)
        }
        "279b90" => json!(w.margin(&unit(0)?, i(1), i(2), i(3))?),
        "279e50" => json!(w.margin_seen(&unit(0)?, i(1), i(2), i(3), i(4), i(5) != 0)?),
        "279d60" => json!(w.margin_vs(&unit(0)?, i(1), i(2), i(3))?),
        "27b200" => b(w.step_ok(&unit(0)?, i(1), i(2), i(3), i(4), i(5))?),
        "27b600" => b(w.move_ok(&unit(0)?, i(1), i(2), i(3), i(4), i(5), i(6) != 0)?),
        "27c050" => json!(w.attack_step(
            &unit(0)?,
            i(1),
            i(2),
            i(3),
            i(4),
            (i(5) >= 0).then_some(i(5) as u32),
            i(6) != 0,
            i(7) != 0,
            i(8)
        )?),
        "27d750" => b(w.cell_threatened(i(0), i(1), i(2), i(3), i(4) != 0)?),
        "278600" => json!(w.danger(i(0), i(1), i(2), i(3), i(4), i(5) != 0)?),
        "27d9b0" => json!(w.list(i(0), i(1), i(2) != 0)),
        "27dc00" => json!(w.list_seen(i(0), i(1), i(2) != 0, i(3) != 0)),
        "26c5c0" => json!(w.far_trap(i(0), i(1), i(2) as u32)),
        "26c4b0" => json!(w.far_trap_for(i(0), i(1) as u32)),
        "271660" => json!(w.predicted_hand(i(0))),
        "271440" => b(w.affordable(i(0), i(1))?),
        "10f900" => json!(i32::from(evolution_mode(w))),
        _ => return Err(DuelError::InvalidCommand("unknown probe")),
    })
}

#[test]
fn private_world_probes_match_the_original() {
    let Some(o) = Oracle::load() else {
        return;
    };
    let (mut ok, mut bad, mut refused) = (0usize, Vec::new(), 0usize);
    for path in o.files("probes/world") {
        let probes: Probes = load_json(&path);
        let case = o.case(&probes.case);
        let duel = o.duel(&case.duel);
        let w = World::new(&duel, &o.cards, &o.tables, case.tier).unwrap();
        for (f, a, want) in &probes.calls {
            match world_probe(&w, f, a) {
                Err(DuelError::Unsupported(_)) => refused += 1,
                Err(e) => bad.push(format!("{} {f}{a:?}: error {e}", probes.case)),
                Ok(v) if &v == want => ok += 1,
                Ok(v) => bad.push(format!("{} {f}{a:?}: {v} vs {want}", probes.case)),
            }
        }
    }
    eprintln!(
        "world probes: {ok} match, {} differ, {refused} refused",
        bad.len()
    );
    for b in bad.iter().take(60) {
        eprintln!("DIFF {b}");
    }
    assert!(bad.is_empty(), "{} world probes differ", bad.len());
}

// ---- synthetic tests (no original data) -----------------------------------

struct NoTables;
impl FusionData for NoTables {
    fn fuse(&self, _a: u16, _b: u16) -> Option<u16> {
        None
    }
    fn equip(&self, _monster: u16, _card: u16) -> i32 {
        -1
    }
    fn category(&self, _card: u16) -> u8 {
        0
    }
    fn value(&self, _card: u16) -> u16 {
        0
    }
}

/// 854 plain level-1 monsters (kind 3, earth) with ATK/DEF 1000/1000.
fn synthetic_cards() -> Vec<CardInfo> {
    (0..854u16)
        .map(|id| CardInfo {
            id,
            kind: 3,
            level: 1,
            attribute: 3,
            attack: 1000,
            defense: 1000,
            effects: [false; 5],
            toon: false,
            kind_index: 0,
            entries: [None; 5],
            equip_bits: [0; 7],
            power_up: None,
        })
        .collect()
}

fn unit(side: u8, slot: u8, card: u16, column: u8, row: u8) -> DuelUnit {
    DuelUnit {
        id: crate::UnitId(u16::from(side) * 11 + u16::from(slot)),
        card,
        side,
        slot,
        position: GridPos { column, row },
        face_up: slot == 0,
        defence: false,
        committed_face_up: slot == 0,
        committed_defence: false,
        acted: side == 0,
        spellbind: 0,
        summon_order: if slot == 0 { 0 } else { slot },
        attack_bonus: 0,
        defense_bonus: 0,
        spellbind_fresh: false,
        evo_counter: 0,
        stolen: false,
        discount: 0,
        stolen_by: None,
    }
}

fn side_state() -> SideState {
    SideState {
        lp: 4000,
        stars: 4,
        can_summon: false,
        hand: vec![],
        deck: vec![],
        leader_abilities: vec![],
        summons: 0,
        graveyard: vec![],
        destiny: 0,
    }
}

fn duel_with(units: Vec<DuelUnit>, cards: &[CardInfo]) -> Duel {
    Duel::from_observed(
        vec![7; 49],
        units,
        [side_state(), side_state()],
        2,
        0,
        1,
        &cards.to_vec(),
    )
    .unwrap()
}

#[test]
fn sync_frees_missing_units_adds_new_ones_and_compacts() {
    let cards = synthetic_cards();
    let duel = duel_with(
        vec![
            unit(0, 0, 10, 3, 6),
            unit(1, 0, 11, 3, 0),
            unit(1, 1, 12, 3, 1),
            unit(1, 2, 13, 2, 1),
        ],
        &cards,
    );
    let mut m = Memory::new(&duel, 1);
    // A record for a unit that is gone, then a gap.
    m.records[1] = Record {
        key: 0x104,
        order: 4,
        plan_b: 6,
        plan_a: 3,
        target_b: 1,
        target_a: 2,
        bits: 1,
    };
    sync(&mut m, &duel, 1);
    let keys: Vec<u16> = m.records.iter().map(|r| r.key).collect();
    assert_eq!(&keys[..4], &[0x100, 0x101, 0x102, 0xFFFF]);
    assert_eq!(m.records[1].order, 1);
    assert_eq!(m.records[1].plan_a, 0xFF);
    // Player units are never added while Weevil (side 1) is active.
    assert!(m.records.iter().all(|r| r.key >> 8 != 0 || r.key == 0xFFFF));
}

#[test]
fn sync_quirk_resets_plans_when_slot_one_has_a_summon_order() {
    let cards = synthetic_cards();
    let duel = duel_with(
        vec![
            unit(0, 0, 10, 3, 6),
            unit(1, 0, 11, 3, 0),
            unit(1, 1, 12, 3, 1),
            unit(1, 2, 13, 2, 1),
        ],
        &cards,
    );
    let mut m = Memory::new(&duel, 1);
    sync(&mut m, &duel, 1);
    // Slot 2's snapshot (2) equals its slot, so the probe reads the leader
    // (order 0): plans survive. Slot 1's snapshot (1) equals its slot too.
    m.records[2].plan_a = 3;
    m.records[2].target_a = 0x203;
    sync(&mut m, &duel, 1);
    assert_eq!((m.records[2].plan_a, m.records[2].target_a), (3, 0x203));
    // A snapshot that differs from the slot makes the probe read slot 1,
    // whose summon order (1) is non-zero: the plans are wiped.
    m.records[2].order = 7;
    sync(&mut m, &duel, 1);
    assert_eq!((m.records[2].plan_a, m.records[2].order), (0xFF, 2));
}

#[test]
fn world_accepts_the_campaign_ability_lists_and_refuses_the_rest() {
    // Weevil [1, 3], Rex [0, 1, 3, 12], Keith [0, 2, 3, 12]: the planner
    // never queries #0..#3 (ABILITY-3-CPU.md), so such a side plays on.
    // [F] A call-graph walk from every CPU module (0x10D210..0x172DD4 and
    // their callees) finds ability reads only in 0x247E10 (#4, #12),
    // 0x24CC30 (#9, #11, #13, #15, #16), 0x24DDA0 / 0x26CB20 / 0x273B40
    // (#11), 0x273160 (#8) and 0x273420 (#9, #10, #14), each modelled or
    // refused where it is read; #5 and #7 are never read, so Shadi's
    // [0, 2, 3, 9, 10, 11, 16] and lists with #5 are accepted too.
    let cards = synthetic_cards();
    let mut duel = duel_with(vec![unit(0, 0, 10, 3, 6), unit(1, 0, 11, 3, 0)], &cards);
    for list in [
        vec![1, 3],
        vec![0, 1, 3, 12],
        vec![0, 2, 3, 12],
        vec![1, 3, 5],
        vec![0, 2, 3, 9, 10, 11, 16],
    ] {
        duel.sides[1].leader_abilities = list;
        assert!(World::new(&duel, &cards, &NoTables, 0).is_ok());
    }
    duel.sides[1].leader_abilities = vec![1, 3, 17];
    assert!(matches!(
        World::new(&duel, &cards, &NoTables, 0),
        Err(DuelError::Unsupported(_))
    ));
}

#[test]
fn ai_reads_the_stored_leader_level_discount_for_cost_and_aura() {
    let mut cards = synthetic_cards();
    cards[11].kind = 4;
    cards[11].level = 6;
    cards[12].kind = 4;
    cards[12].level = 4;
    let mut duel = duel_with(vec![unit(0, 0, 10, 3, 6), unit(1, 0, 11, 3, 0)], &cards);
    duel.sides[1].leader_abilities = vec![8];
    let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
    assert_eq!(w.leader_discount(1), 2);
    assert_eq!(w.cost(12, 1).unwrap(), 2);
    duel.units[1].discount = 1;
    let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
    assert_eq!(w.leader_discount(1), 1);
    assert_eq!(w.cost(12, 1).unwrap(), 3);

    // Original 0x273420 uses the same effective leader level for its aura.
    cards[11].level = 4;
    duel.sides[1].leader_abilities = vec![10];
    duel.units.push(unit(1, 1, 12, 3, 1));
    let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
    assert_eq!(
        w.eff_atk(1, 1, 3, 1).unwrap(),
        i32::from(cards[12].attack) + 300
    );
    duel.units[1].discount = 9;
    let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
    assert_eq!(w.eff_atk(1, 1, 3, 1).unwrap(), i32::from(cards[12].attack));
}

#[test]
#[ignore = "requires original-instruction discount cases in ROSES_ABILITY_ORACLE"]
fn private_ai_discount_matches_original_instruction_oracle() {
    #[derive(serde::Deserialize)]
    struct Discount {
        kind: u8,
        level: u8,
        stored: u8,
        target_kind: u8,
        discount: u8,
        leader_discount: u8,
    }
    #[derive(serde::Deserialize)]
    struct Cases {
        discounts: Vec<Discount>,
    }
    let cases: Cases = load_json(&PathBuf::from(
        std::env::var_os("ROSES_ABILITY_ORACLE").expect("ROSES_ABILITY_ORACLE"),
    ));
    assert!(
        !cases.discounts.is_empty(),
        "missing original discount evidence"
    );
    let mut cards = synthetic_cards();
    for c in cases.discounts {
        cards[11].kind = c.kind;
        cards[11].level = c.level;
        cards[12].kind = c.target_kind;
        cards[12].level = 4;
        let mut duel = duel_with(vec![unit(0, 0, 10, 3, 6), unit(1, 0, 11, 3, 0)], &cards);
        duel.units[1].discount = c.stored;
        duel.sides[1].leader_abilities = vec![8];
        let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
        assert_eq!(
            w.leader_discount(1),
            i32::from(c.leader_discount),
            "kind {}, level {}, stored {}",
            c.kind,
            c.level,
            c.stored
        );
        assert_eq!(
            w.cost(12, 1).unwrap(),
            4 - i32::from(c.discount),
            "kind {}, level {}, stored {}, target {}",
            c.kind,
            c.level,
            c.stored,
            c.target_kind
        );
    }
}

#[test]
fn evolution_mode_follows_the_pieces_on_field_hand_and_deck() {
    let cards = synthetic_cards();
    let mut duel = duel_with(
        vec![
            unit(0, 0, 10, 3, 6),
            unit(1, 0, 11, 3, 0),
            unit(1, 1, 12, 3, 1),
        ],
        &cards,
    );
    let mode = |duel: &Duel| {
        let w = World::new(duel, &cards, &NoTables, 0).unwrap();
        evolution_mode(&w)
    };
    assert_eq!(mode(&duel), 1);
    duel.sides[1].deck = vec![5, EVOLUTION_PIECES[0] as u16];
    assert_eq!(mode(&duel), 0);
    duel.sides[1].deck.clear();
    duel.sides[1].hand = vec![EVOLUTION_PIECES[1] as u16];
    assert_eq!(mode(&duel), 0);
    duel.units[2].card = EVOLVED[2] as u16;
    assert_eq!(mode(&duel), 1);
}

#[test]
fn plan_ends_the_turn_when_every_unit_has_acted() {
    let cards = synthetic_cards();
    let mut duel = duel_with(
        vec![
            unit(0, 0, 10, 3, 6),
            unit(1, 0, 11, 3, 0),
            unit(1, 1, 12, 3, 1),
        ],
        &cards,
    );
    for u in duel.units.iter_mut() {
        u.acted = true;
    }
    let mut m = Memory::new(&duel, 1);
    sync(&mut m, &duel, 1);
    assert_eq!(
        plan(&duel, 1, &mut m, &cards, &NoTables, 0).unwrap(),
        Plan::EndTurn
    );
}

#[test]
fn world_margins_use_terrain_and_committed_defence() {
    let mut cards = synthetic_cards();
    cards[12].attack = 1500;
    cards[13].attack = 900;
    cards[13].defense = 1600;
    let mut duel = duel_with(
        vec![
            unit(0, 0, 10, 3, 6),
            unit(1, 0, 11, 3, 0),
            unit(1, 1, 12, 3, 1),
            unit(0, 1, 13, 3, 2),
        ],
        &cards,
    );
    // Warriors are favoured on Meadow (3).
    duel.terrain[2 * 7 + 3] = 3;
    let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
    let att = w.unit(1, 1).unwrap().clone();
    // 1500 + 500 vs 900 + 500 on the defender's cell.
    assert_eq!(w.margin(&att, 0x001, 3, 2).unwrap(), 600);
    duel.units[3].committed_defence = true;
    let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
    assert_eq!(w.margin(&att, 0x001, 3, 2).unwrap(), 2000 - 2100);
    // Face-down defender with an assumed strength.
    assert_eq!(w.margin_seen(&att, 0x001, 1200, 3, 2, false).unwrap(), 800);
}

#[test]
fn world_steps_respect_labyrinth_and_occupants() {
    let cards = synthetic_cards();
    let mut duel = duel_with(
        vec![
            unit(0, 0, 10, 3, 6),
            unit(1, 0, 11, 3, 0),
            unit(1, 1, 12, 3, 1),
            unit(0, 1, 13, 3, 3),
        ],
        &cards,
    );
    duel.terrain[7 + 4] = 8;
    let w = World::new(&duel, &cards, &NoTables, 0).unwrap();
    let u = w.unit(1, 1).unwrap().clone();
    assert!(w.step_ok(&u, 3, 1, 3, 2, 0xFFFF).unwrap());
    assert!(!w.step_ok(&u, 3, 1, 4, 1, 0xFFFF).unwrap());
    // Two cells onto the enemy only when it is the allowed occupant.
    assert!(!w.step_ok(&u, 3, 1, 3, 3, 0xFFFF).unwrap());
    assert!(w.step_ok(&u, 3, 1, 3, 3, 0x001).unwrap());
    // Attack paths: an enemy it beats (tie here) is not passable.
    assert_eq!(
        w.attack_step(&u, 3, 2, 3, 3, None, true, false, 0).unwrap(),
        2
    );
}

#[test]
fn private_runtime_tables_match_the_dumped_tables() {
    let Some(o) = Oracle::load() else {
        return;
    };
    // The EE image's static data equals the executable's (file offset =
    // EE − 0xF9680), so an image shifted by that amount stands in for it.
    let Ok(image) = std::fs::read(o.root.join("tools/008-activate.ee")) else {
        return;
    };
    let fusions: Vec<[u16; 3]> = load_json::<Vec<[u16; 3]>>(&o.root.join("data/fusions.json"))
        .into_iter()
        .map(|[high, low, r]| [low, high, r])
        .collect();
    let t = super::Tables::new(&fusions, &image[0xF9680..]).unwrap();
    for a in 0..854u16 {
        for b in 0..854u16 {
            assert_eq!(t.equip(a, b), o.tables.equip(a, b), "equip({a}, {b})");
            assert_eq!(t.fuse(a, b), o.tables.fuse(a, b), "fuse({a}, {b})");
        }
        assert_eq!(t.category(a), o.tables.category(a));
        assert_eq!(t.value(a), o.tables.value(a));
    }
}

#[test]
fn a_monster_two_steps_from_the_enemy_leader_attacks_it() {
    let cards = synthetic_cards();
    let mut units = vec![
        unit(0, 0, 10, 3, 4),
        unit(1, 0, 11, 3, 0),
        unit(1, 1, 12, 3, 2),
    ];
    units[1].acted = true;
    units[2].face_up = true;
    units[2].committed_face_up = true;
    let duel = duel_with(units, &cards);
    let mut m = Memory::new(&duel, 1);
    sync(&mut m, &duel, 1);
    let p = plan(&duel, 1, &mut m, &cards, &NoTables, 0).unwrap();
    // 0x10DD60: the enemy leader is within two moves → plan 2 on its key;
    // 0x1108C0 moves one cell (allowance 1) toward it.
    assert_eq!(
        p,
        Plan::Act(Action::Act {
            unit: crate::UnitId(12),
            target: GridPos { column: 3, row: 3 },
            reveal: false,
            position: Position::Keep,
        })
    );
    let rec = m.records.iter().find(|r| r.key == 0x101).unwrap();
    assert_eq!((rec.plan_a, rec.target_a), (2, 0x0000));
}

#[test]
fn the_leader_summons_toward_the_first_formation_cell() {
    let mut cards = synthetic_cards();
    // Four level-5 cards the 4 stars cannot pay for (and too strong to be
    // discarded as fodder) next to one affordable level-1 monster.
    for c in &mut cards[21..25] {
        c.level = 5;
        c.attack = 2000;
    }
    let mut duel = duel_with(vec![unit(0, 0, 10, 3, 6), unit(1, 0, 11, 3, 0)], &cards);
    duel.sides[1].can_summon = true;
    duel.sides[1].hand = vec![20];
    duel.sides[1].deck = vec![21, 22, 23, 24];
    let mut m = Memory::new(&duel, 1);
    sync(&mut m, &duel, 1);
    let p = plan(&duel, 1, &mut m, &cards, &NoTables, 0).unwrap();
    // No enemy within three moves of the leader: the monster goes to the
    // leader's neighbour nearest the formation cell (3,2).
    assert_eq!(
        p,
        Plan::Act(Action::Summon {
            cell: GridPos { column: 3, row: 1 },
            hand: vec![0],
        })
    );
    // The leader's plan: summon now, then walk to (1,0).
    assert_eq!((m.records[0].plan_a, m.records[0].plan_b), (0xA, 6));
    assert_eq!(m.records[0].target_b, 1);
}

#[test]
fn the_vs_control_bytes_plan_with_the_generic_module() {
    // Custom Duel's computer (22..=25) is the generic module 0x16DA10, the
    // same as Shadi's 16: equal plans and memories on the same board; 26 is
    // not registered.
    let cards = synthetic_cards();
    let mut units = vec![unit(0, 0, 10, 3, 4), unit(1, 0, 11, 3, 0), unit(1, 1, 12, 3, 2)];
    units[1].acted = true;
    units[2].face_up = true;
    units[2].committed_face_up = true;
    let mut duel = duel_with(units, &cards);
    duel.sides[1].can_summon = true;
    duel.sides[1].hand = vec![20];
    duel.sides[1].deck = vec![21, 22, 23, 24];
    let run = |control: u8| {
        let mut m = Memory::new(&duel, 1);
        sync(&mut m, &duel, 1);
        let p = plan_for_rng(control, &duel, 1, &mut m, &cards, &NoTables, 0);
        (p, m)
    };
    let (shadi, memory) = run(16);
    let shadi = shadi.unwrap();
    for control in VS_CONTROLLERS {
        assert!(is_ported(control));
        let (p, m) = run(control);
        assert_eq!(p.unwrap(), shadi, "control {control}");
        assert_eq!(m, memory, "control {control}");
    }
    assert!(!is_ported(26));
    assert!(matches!(run(26).0, Err(crate::duel::DuelError::Unsupported(_))));
}
