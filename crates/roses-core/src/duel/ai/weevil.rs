//! The computer opponents' controller framework, first ported for Weevil
//! (control byte 2, driver 0x1117D0, module 0x10D210..0x1119A0 plus the
//! shared opponent helpers 0x277640..0x2800C0) and generalised for Pegasus
//! ([`pegasus`]: control byte 9, driver 0x139F90, module
//! 0x135B00..0x13A158). Every opponent module of the original is a copy of
//! the same template (a decide/dispatch, an actor chooser, a plan
//! assigner, a summon planner and the plan executors) over the shared
//! helpers; the shared parts live here ([`World`](world), searches,
//! executors, plan and summon helpers) and each opponent supplies its own
//! functions and constants through [`Opponent`]. [`plan_for`] selects the
//! module by the side's control byte.
//!
//! The driver runs, whenever its key queue is empty: [`sync`] (0x2777C0),
//! [`plan`] (0x10D210, which fills the 0x277BD0 action record) and the key
//! emitter ([`super::keys`]). This port keeps the original's memory (22
//! per-unit records in the side struct), its decision order and its quirks;
//! functions are named after their EE addresses. The traced behaviour is
//! described in the private `WEEVIL-CONTROLLER.md`.
//!
//! Not modelled: the driver's frame watchdog (+0x1A2: Circle at calls
//! 600..602, then Start) and the Timer1 budget (0x1A9080 ≥ 0x2EE1) of the
//! path searches, which is treated as never expiring. [`plan`] returns
//! [`DuelError::Unsupported`] instead of guessing when the original would
//! depend on something the duel state does not hold: states the engine
//! refuses (leader abilities other than #4/#6, continuous effects, control
//! changes, card 0x29F on the field), the summon planner's fusion tree
//! reading an undefined register half past a predicted hand shorter than
//! four cards, Insect Imitation's rand() branch (0x24A960), and 0x27F170
//! mode 0 when its answer depends on hand-object RAM (group 2).

mod bakura;
mod darkness;
mod exec;
mod ishtar;
mod jasper;
mod joey;
mod keith;
mod labyrinth;
mod mako;
mod manawyddan;
mod manawyddan_white;
mod margaret;
mod necromancer;
#[cfg(test)]
mod opponent_cases;
mod pegasus;
mod plans;
mod rex;
mod search;
mod seto;
mod shadi;
mod summon;
mod tea;
mod tristan;
mod world;
mod york;
mod yugi;

use super::{Action, Position};
use crate::duel::{CardData, Duel, DuelError};
use crate::{GridPos, UnitId};
use serde::{Deserialize, Serialize};
use world::{key, World, R};

/// Fusion and power-up tables the planner reads (0x24AEC0, 0x24A880 and
/// the raw power-up bytes). Kept separate from [`CardData`] because the
/// original indexes them with any card id, not only power-ups.
pub trait FusionData {
    /// 0x24AEC0 over table 0x29DC20 (first entry in table order wins):
    /// `None` when no entry matches (the original's 999).
    fn fuse(&self, a: u16, b: u16) -> Option<u16>;
    /// 0x24A880(monster, card) exactly as the original computes it for any
    /// two ids, including its reads outside the power-up rows of the
    /// compatibility table 0x29C970: −1 when the bit is clear or 0x24A960
    /// has no result, 0x10000 | monster for a plain bonus, the new card for
    /// a transformation, or [`EQUIP_RANDOM`] for the Insect Imitation
    /// branch that draws rand().
    fn equip(&self, monster: u16, card: u16) -> i32;
    /// Raw byte 0x29C5E0 + card (0x24AEA0), for any card id.
    fn category(&self, card: u16) -> u8;
    /// Raw u16 0x29C322 + 2·card, for any card id.
    fn value(&self, card: u16) -> u16;
    /// Byte 3 of the card record 0x2BE400 + 20·card (0x1DE350), the deck
    /// cost, for ids below 854. Only the tie-break of 0x278E30 (the trap
    /// targets of Darkness-ruler and Keith) reads it; `None` (tables
    /// without the records) makes that tie-break refuse the decision.
    fn deck_cost(&self, card: u16) -> Option<u8> {
        let _ = card;
        None
    }
}

/// [`FusionData::equip`] result for the rand() branch of 0x24A960.
pub const EQUIP_RANDOM: i32 = i32::MIN;

/// [`FusionData`] read at runtime from the PAL executable (SLES_515.79,
/// file offset = EE address − 0xF9680) plus the imported fusion table.
/// Nothing is embedded: [`Tables::new`] slices these regions of the file:
/// - 0x29C5E0 (file 0x1A2F60): power-up category byte per card id
///   (0x24AEA0, 0x24AC40, 0x24ACC0), 0x400 bytes;
/// - 0x29C322 (file 0x1A2CA2): u16 value per card id (0x24A960's type,
///   0x24ACC0's bonus), 0x400 entries;
/// - 0x29C970 (file 0x1A32F0): 7-byte compatibility rows per card id,
///   read by 0x24A880 at row·7 + (card − 0x2F0) >> 3, which reaches up to 94
///   bytes before the table and past its end for non-power-up ids; sliced
///   from 0x80 bytes before it to 0x40 bytes after 0x400 rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tables {
    /// (low, high) ingredient pair → result (unique pairs, low ≤ high).
    fusions: std::collections::HashMap<(u16, u16), u16>,
    category: Vec<u8>,
    value: Vec<u16>,
    compat: Vec<u8>,
    /// Byte 3 of the 854 card records 0x2BE400 (file 0x1C4D80).
    deck_cost: Vec<u8>,
}

const EE_TO_FILE: usize = 0xF9680;
const CATEGORY_EE: usize = 0x29C5E0;
const VALUE_EE: usize = 0x29C322;
const COMPAT_EE: usize = 0x29C970;
const COMPAT_BEFORE: usize = 0x80;
const IDS: usize = 0x400;
const RECORDS_EE: usize = 0x2BE400;
const RECORD_BYTES: usize = 20;
const RECORDS: usize = 854;

impl Tables {
    /// `fusions`: the table 0x29DC20 as (low, high, result) records (the
    /// importer's order-checked form); `executable`: the SLES_515.79 bytes.
    /// None when the executable is too short.
    pub fn new(fusions: &[[u16; 3]], executable: &[u8]) -> Option<Self> {
        let slice = |ee: usize, len: usize| executable.get(ee - EE_TO_FILE..ee - EE_TO_FILE + len);
        let category = slice(CATEGORY_EE, IDS)?.to_vec();
        let value = slice(VALUE_EE, IDS * 2)?
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let compat = slice(COMPAT_EE - COMPAT_BEFORE, COMPAT_BEFORE + IDS * 7 + 0x40)?.to_vec();
        let deck_cost = slice(RECORDS_EE, RECORDS * RECORD_BYTES)?
            .chunks_exact(RECORD_BYTES)
            .map(|rec| rec[3])
            .collect();
        Some(Self {
            fusions: fusions.iter().map(|&[l, h, r]| ((l, h), r)).collect(),
            category,
            value,
            compat,
            deck_cost,
        })
    }
}

impl FusionData for Tables {
    fn fuse(&self, a: u16, b: u16) -> Option<u16> {
        self.fusions.get(&(a.min(b), a.max(b))).copied()
    }
    /// 0x24A880 and 0x24A960.
    fn equip(&self, monster: u16, card: u16) -> i32 {
        let d = i32::from(card) - 0x2F0;
        let offset = i32::from(monster) * 7 + (d >> 3) + COMPAT_BEFORE as i32;
        let byte = usize::try_from(offset)
            .ok()
            .and_then(|o| self.compat.get(o))
            .copied()
            .unwrap_or(0);
        // A negative offset with a partial byte shifts by 25..31 (srav): 0.
        let bit = if d < 0 && d & 7 != 0 {
            0
        } else {
            byte >> (d & 7) & 1
        };
        if bit == 0 {
            return -1;
        }
        let a = i32::from(monster);
        // 0x24A960's transformations (code immediates), keyed by the u16
        // value of the power-up.
        match self.value(card) {
            9 if matches!(a, 0x10 | 0x11 | 0xA7 | 0x14C | 0x17B) => EQUIP_RANDOM,
            9 => -1,
            8 => match a {
                7 => 0x1F5,
                0x156 => 0x1E8,
                _ => -1,
            },
            7 => 0x1AB,
            6 => 0x111,
            _ => a | 0x10000,
        }
    }
    fn category(&self, card: u16) -> u8 {
        self.category.get(usize::from(card)).copied().unwrap_or(0)
    }
    fn value(&self, card: u16) -> u16 {
        self.value.get(usize::from(card)).copied().unwrap_or(0)
    }
    fn deck_cost(&self, card: u16) -> Option<u8> {
        self.deck_cost.get(usize::from(card)).copied()
    }
}

/// One of the 22 per-unit records at side +0x000 (16 bytes each).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// +0: unit key (group << 8) | slot; 0xFFFF = free.
    pub key: u16,
    /// +2: summon-order snapshot (0x00FF free).
    pub order: u16,
    /// +4: secondary plan code; +5: primary plan code (0xFF = none).
    pub plan_b: u8,
    pub plan_a: u8,
    /// +8: target of the secondary plan; +0xA: of the primary plan.
    pub target_b: u16,
    pub target_a: u16,
    /// +0xC bits 0..1.
    pub bits: u8,
}

impl Record {
    /// The value 0x277640/0x2777C0 write into a free record.
    pub const FREE: Self = Self {
        key: 0xFFFF,
        order: 0xFF,
        plan_b: 0xFF,
        plan_a: 0xFF,
        target_b: 0xFFFF,
        target_a: 0xFFFF,
        bits: 0,
    };
    /// `sh v, 4(rec)`: primary plan = v >> 8, secondary plan = v & 0xFF.
    pub(super) fn set_plans(&mut self, v: u16) {
        self.plan_a = (v >> 8) as u8;
        self.plan_b = v as u8;
    }
    /// The word at +8 (secondary target low, primary target high).
    pub(super) fn targets(&self) -> u32 {
        u32::from(self.target_a) << 16 | u32::from(self.target_b)
    }
    /// 0x27CAF0(rec, which): 1 clears the primary target, 0 the secondary.
    pub(super) fn consume(&mut self, which: u8) {
        match which {
            1 => self.target_a = 0xFFFF,
            0 => self.target_b = 0xFFFF,
            _ => (),
        }
    }
    /// The target of plan slot `which` (1 = primary +0xA, 0 = secondary +8).
    pub(super) fn target(&self, which: u8) -> u16 {
        if which == 1 {
            self.target_a
        } else {
            self.target_b
        }
    }
    /// Reset plans and targets (the 0x2777C0 re-snapshot and 0x277A60).
    fn reset(&mut self) {
        self.target_a = 0xFFFF;
        self.target_b = 0xFFFF;
        self.plan_a = 0xFF;
        self.plan_b = 0xFF;
        self.bits = 0;
    }
}

impl Default for Record {
    fn default() -> Self {
        Self::FREE
    }
}

/// Weevil's memory: the side struct's records and byte +0x1A1 (low nibble:
/// the 0x10F900 mode). Rebuild it from RAM as 22 × 16 bytes at side +0 (see
/// [`Record`]) plus side +0x1A1.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    pub records: [Record; 22],
    /// Side +0x1A1 (low nibble = mode).
    pub x1a1: u8,
}

impl Memory {
    /// 0x277640 (duel init): all records free, record 0 = the leader.
    pub fn new(duel: &Duel, side: u8) -> Self {
        let mut m = Self::default();
        m.records[0].key = key(side, 0);
        m.records[0].order = duel
            .units
            .iter()
            .find(|u| u.side == side && u.slot == 0)
            .map_or(0xFFFF, |u| u16::from(u.summon_order));
        m
    }
    /// 0x2785C0: index of the record for `key`.
    fn find(&self, k: u16) -> Option<usize> {
        self.records.iter().position(|r| r.key == k)
    }
    /// The 0x10F900 mode nibble (1 = no evolution piece left or one done).
    pub fn mode(&self) -> u8 {
        self.x1a1 & 0xF
    }
}

/// The action record 0x1117D0 hands from 0x10D210 to 0x277BD0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Decision {
    /// +0 actor key.
    pub key: u16,
    /// +2 type: 6 act, 0xA summon, 0xC graveyard pick, 0xD unit pick
    /// (Pegasus), 0xFF/0xE nothing.
    pub kind: u8,
    /// +4 destination (row << 8) | col (types 0xC/0xD: the pick).
    pub target: u16,
    /// +6..+0xA hand indices (0xFF unused).
    pub hand: [u8; 5],
    /// +0xC reveal (R2) when 1.
    pub reveal: u32,
    /// +0x10 position request.
    pub position: u8,
    /// Not part of the record: the unit whose acted flag the module set
    /// itself (0x1E30F0(g, s, 0, 1) in Pegasus's 0x139050/0x139220), a
    /// side effect on the duel state the caller must apply.
    pub mark_acted: Option<u16>,
}

impl Decision {
    /// As 0x1117D0 initialises it.
    fn new() -> Self {
        Self {
            key: 0xFFFF,
            kind: 0xFF,
            target: 0xFFFF,
            hand: [0xFF; 5],
            reveal: 0,
            position: 0xFF,
            mark_acted: None,
        }
    }
}

/// What 0x10D210 decided.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plan {
    /// An action for the key emitter.
    Act(Action),
    /// 0x10D210 returned 1 with an empty action record (type 0xFF): the
    /// driver presses nothing this frame and plans again on the next one.
    NoKey,
    /// As [`Plan::NoKey`], after the module set the unit's acted flag in
    /// the duel state itself (0x1E30F0 from Pegasus's 0x139050/0x139220):
    /// the caller marks the unit acted before planning again.
    MarkActed(UnitId),
    /// 0x10D210 returned 0: the driver returns Start (end of turn).
    EndTurn,
}

/// 0x2777C0: refresh the records for `duel.active` (called by the driver
/// before every plan and at turn start via 0x277770). `side` selects whose
/// memory this is (the driver passes that side's struct).
pub fn sync(memory: &mut Memory, duel: &Duel, side: u8) {
    let _ = side;
    let active = i32::from(duel.active);
    let exists = |g: i32, s: i32| {
        (0..2).contains(&g)
            && (0..11).contains(&s)
            && duel
                .units
                .iter()
                .any(|u| i32::from(u.side) == g && i32::from(u.slot) == s)
    };
    let order = |g: i32, s: i32| {
        duel.units
            .iter()
            .find(|u| i32::from(u.side) == g && i32::from(u.slot) == s)
            .map_or(-1, |u| i32::from(u.summon_order))
    };
    // 0x24F790: the group, or the other side for a control-changed unit.
    let ctrl = |g: i32, s: i32| {
        duel.units
            .iter()
            .find(|u| i32::from(u.side) == g && i32::from(u.slot) == s)
            .map_or(g, |u| i32::from(u.controller()))
    };
    for rec in memory.records.iter_mut() {
        if rec.key == 0xFFFF {
            continue;
        }
        let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
        if !exists(g, s) || ctrl(g, s) != active {
            *rec = Record::FREE;
            continue;
        }
        // Quirk: the "unit replaced" test reads the summon order of slot
        // (slot != snapshot) — the leader or slot 1 — instead of the unit.
        let probe = i32::from(s != i32::from(rec.order));
        if order(g, probe) != 0 {
            rec.order = order(g, s) as u16;
            rec.reset();
        }
    }
    for s in 0..11 {
        for g in 0..2 {
            let k = key(g as u8, s as u8);
            if ctrl(g, s) != active || memory.find(k).is_some() || !exists(g, s) {
                continue;
            }
            // 0x277A60: the first free record.
            if let Some(r) = memory.records.iter_mut().find(|r| r.key == 0xFFFF) {
                r.key = k;
                r.order = order(g, s) as u16;
                r.reset();
            }
        }
    }
    // 0x277B10: move later records down into free ones.
    for i in 0..22 {
        if memory.records[i].key != 0xFFFF {
            continue;
        }
        if let Some(j) = (i + 1..22).find(|&j| memory.records[j].key != 0xFFFF) {
            memory.records[i] = memory.records[j];
            memory.records[j] = Record::FREE;
        }
    }
}

/// Control bytes whose modules are ported ([`plan_for`]).
pub const PORTED_CONTROLLERS: [u8; 21] = [
    Weevil::CONTROL,
    pegasus::Pegasus::CONTROL,
    labyrinth::Labyrinth::CONTROL,
    darkness::Darkness::CONTROL,
    keith::Keith::CONTROL,
    rex::Rex::CONTROL,
    necromancer::Necromancer::CONTROL,
    ishtar::Ishtar::CONTROL,
    york::York::CONTROL,
    manawyddan::Manawyddan::CONTROL,
    seto::Seto::CONTROL,
    tea::Tea::CONTROL,
    tristan::Tristan::CONTROL,
    margaret::Margaret::CONTROL,
    mako::Mako::CONTROL,
    joey::Joey::CONTROL,
    shadi::Shadi::CONTROL,
    jasper::Jasper::CONTROL,
    bakura::Bakura::CONTROL,
    yugi::Yugi::CONTROL,
    manawyddan_white::ManawyddanWhite::CONTROL,
];

/// The VS-mode control bytes Custom Duel's init 0x1C6BC0 writes for the
/// computer side (selectors 1..=4: 22 with a copy of the player's block,
/// 25 deck 0x32, 24 deck 0x31, 23 deck 0x30). The driver table 0x2BA230
/// sends all four to the generic module (j-stub 0x154AE0 → driver 0x172C40,
/// decide 0x16DA10), ported as Shadi's [`shadi`]: the module never reads its
/// control byte, so they plan exactly as control 16 does [F
/// `work/research/modes/custom-duel/NOTES.md`].
pub const VS_CONTROLLERS: [u8; 4] = [22, 23, 24, 25];

/// Whether `controller` has a ported module (a campaign controller or a
/// VS control byte).
pub fn is_ported(controller: u8) -> bool {
    PORTED_CONTROLLERS.contains(&controller) || VS_CONTROLLERS.contains(&controller)
}

/// 0x10D210: pick the acting unit and turn its plans into an action for
/// `side` (the active side). Call [`sync`] first, as the driver does.
/// `tier` is the s16 at 0x29922E (save data +0x3E of 0x2991F0, incremented
/// at duel end by 0x227F00) that the summon planner reads via 0x23BCD0.
pub fn plan(
    duel: &Duel,
    side: u8,
    memory: &mut Memory,
    cards: &impl CardData,
    fusions: &impl FusionData,
    tier: i16,
) -> Result<Plan, DuelError> {
    plan_for(Weevil::CONTROL, duel, side, memory, cards, fusions, tier)
}

/// [`plan`] with the module of control byte `controller` (side +0x33C,
/// the index into the driver table 0x2BA230). [`PORTED_CONTROLLERS`] lists
/// the implemented modules; other bytes are refused explicitly.
pub fn plan_for(
    controller: u8,
    duel: &Duel,
    side: u8,
    memory: &mut Memory,
    cards: &impl CardData,
    fusions: &impl FusionData,
    tier: i16,
) -> Result<Plan, DuelError> {
    let (plan, rng) = plan_for_rng(controller, duel, side, memory, cards, fusions, tier)?;
    if rng != duel.rng {
        return Err(DuelError::Unsupported(
            "the decision drew rand(): plan with plan_for_rng and keep its LCG state".into(),
        ));
    }
    Ok(plan)
}

/// [`plan_for`] that also returns the rand() 0x19AE88 state after the
/// decision: modules that draw from it (Tea's Insect Imitation test,
/// 0x24A960) advance `duel.rng`, which the caller must store back.
#[allow(clippy::too_many_arguments)]
pub fn plan_for_rng(
    controller: u8,
    duel: &Duel,
    side: u8,
    memory: &mut Memory,
    cards: &impl CardData,
    fusions: &impl FusionData,
    tier: i16,
) -> Result<(Plan, u64), DuelError> {
    macro_rules! go {
        ($o:ty) => {
            plan_as_rng::<$o, _, _>(duel, side, memory, cards, fusions, tier)
        };
    }
    match controller {
        seto::Seto::CONTROL => go!(seto::Seto),
        Weevil::CONTROL => go!(Weevil),
        pegasus::Pegasus::CONTROL => go!(pegasus::Pegasus),
        labyrinth::Labyrinth::CONTROL => go!(labyrinth::Labyrinth),
        darkness::Darkness::CONTROL => go!(darkness::Darkness),
        keith::Keith::CONTROL => go!(keith::Keith),
        rex::Rex::CONTROL => go!(rex::Rex),
        necromancer::Necromancer::CONTROL => go!(necromancer::Necromancer),
        ishtar::Ishtar::CONTROL => go!(ishtar::Ishtar),
        york::York::CONTROL => go!(york::York),
        manawyddan::Manawyddan::CONTROL => go!(manawyddan::Manawyddan),
        tea::Tea::CONTROL => go!(tea::Tea),
        tristan::Tristan::CONTROL => go!(tristan::Tristan),
        margaret::Margaret::CONTROL => go!(margaret::Margaret),
        mako::Mako::CONTROL => go!(mako::Mako),
        joey::Joey::CONTROL => go!(joey::Joey),
        shadi::Shadi::CONTROL => go!(shadi::Shadi),
        // Custom Duel's computer (VS_CONTROLLERS): the same module.
        22..=25 => go!(shadi::Shadi),
        jasper::Jasper::CONTROL => go!(jasper::Jasper),
        bakura::Bakura::CONTROL => go!(bakura::Bakura),
        yugi::Yugi::CONTROL => go!(yugi::Yugi),
        manawyddan_white::ManawyddanWhite::CONTROL => go!(manawyddan_white::ManawyddanWhite),
        _ => Err(DuelError::Unsupported(format!(
            "the computer controller of control byte {controller} is not ported (only {PORTED_CONTROLLERS:?} and {VS_CONTROLLERS:?} are)"
        ))),
    }
}

fn plan_as_rng<O: Opponent, C: CardData, F: FusionData>(
    duel: &Duel,
    side: u8,
    memory: &mut Memory,
    cards: &C,
    fusions: &F,
    tier: i16,
) -> Result<(Plan, u64), DuelError> {
    let _ = side;
    let world = World::new(duel, cards, fusions, tier)?;
    let mut d = Decision::new();
    let ret = O::dispatch(&world, memory, &mut d)?;
    let rng = world.rng.get();
    plan_of(duel, &world, ret, &d).map(|p| (p, rng))
}

/// The [`Plan`] of the action record `d` that the dispatch returned `ret`
/// with.
fn plan_of<C: CardData, F: FusionData>(
    duel: &Duel,
    world: &World<C, F>,
    ret: i32,
    d: &Decision,
) -> Result<Plan, DuelError> {
    if ret == 0 {
        return Ok(Plan::EndTurn);
    }
    let unit_of = |k: u16| {
        let (g, s) = (i32::from(k >> 8), i32::from(k & 0xFF));
        world
            .unit(g, s)
            .and_then(|u| u.id)
            .ok_or(DuelError::InvalidCommand("decision for a missing unit"))
    };
    if let Some(k) = d.mark_acted {
        return Ok(Plan::MarkActed(unit_of(k)?));
    }
    Ok(match d.kind {
        6 => Plan::Act(Action::Act {
            unit: unit_of(d.key)?,
            target: GridPos {
                column: d.target as u8,
                row: (d.target >> 8) as u8,
            },
            reveal: d.reveal == 1,
            position: match d.position {
                0 => Position::Attack,
                1 => Position::Defence,
                _ => Position::Keep,
            },
        }),
        0xA => Plan::Act(Action::Summon {
            cell: GridPos {
                column: d.target as u8,
                row: (d.target >> 8) as u8,
            },
            hand: d
                .hand
                .iter()
                .filter(|&&i| i < 0xFF)
                .map(|&i| usize::from(i))
                .collect(),
        }),
        // 0x277BD0's types 0xC/0xD: activate the set card and answer its
        // menu (Mimicat: +4 = graveyard << 8 | index; Change of Heart: +4 =
        // the Down count).
        0xC | 0xD => {
            let unit = unit_of(d.key)?;
            let at = duel
                .unit(unit)
                .ok_or(DuelError::InvalidCommand("decision for a missing unit"))?
                .position;
            let (graveyard, down) = if d.kind == 0xC {
                (Some((d.target >> 8) as u8), d.target & 0xF)
            } else {
                (None, d.target)
            };
            Plan::Act(Action::Activate {
                unit,
                at,
                graveyard,
                down,
            })
        }
        _ => Plan::NoKey,
    })
}

/// The raw decide result (for oracle comparisons): return value and the
/// action record.
#[cfg(test)]
fn decide_raw<O: Opponent, C: CardData, F: FusionData>(
    duel: &Duel,
    memory: &mut Memory,
    cards: &C,
    fusions: &F,
    tier: i16,
) -> R<(i32, Decision)> {
    decide_raw_rng::<O, C, F>(duel, memory, cards, fusions, tier).map(|(r, d, _)| (r, d))
}

/// [`decide_raw`] with the rand() state after the decision.
#[cfg(test)]
fn decide_raw_rng<O: Opponent, C: CardData, F: FusionData>(
    duel: &Duel,
    memory: &mut Memory,
    cards: &C,
    fusions: &F,
    tier: i16,
) -> R<(i32, Decision, u64)> {
    let world = World::new(duel, cards, fusions, tier)?;
    let mut d = Decision::new();
    let ret = O::dispatch(&world, memory, &mut d)?;
    Ok((ret, d, world.rng.get()))
}

type W<'a, C, F> = World<'a, C, F>;

/// One opponent module: the functions and constants of its own copy of the
/// controller template that the shared dispatch ([`decide`]), actor choice
/// ([`choose_actor`]) and executors ([`exec`]) call into.
trait Opponent {
    /// The side's control byte (index into the driver table 0x2BA230).
    const CONTROL: u8;
    /// Assumed strength of hidden units in the executors' 0x278600 danger
    /// scores (plans 2/3/6): 1 in 0x110FC0/0x1108C0, 0 in 0x138880/0x1382C0.
    const DANGER_HIDDEN: i32;
    /// The see-all flag the executors pass to 0x27FCA0: 0 (Weevil), 1
    /// (Pegasus).
    const STANCE_SEE_ALL: bool;
    /// The see-all flag of the attack searches 0x27B9D0 (plan 2) and
    /// 0x27E960's call (plan 9, argument t0): 0 (Weevil), 1 (Pegasus).
    const SEARCH_SEE_ALL: i32;
    /// The see-all flag of 0x27E960's call sites alone (plan 9, argument
    /// t0), independently traced from plan 2's first stack argument.
    /// Manawyddan's 0x163A00 and 0x166790 both pass 1; the latter's
    /// separate hidden-strength argument is 0.
    const EQUIP_SEARCH_SEE_ALL: i32 = Self::SEARCH_SEE_ALL;
    /// The see-all flag of the retreat's danger and threat queries
    /// (0x278600, 0x27D500, 0x27D750): 0 (0x110330), 1 (0x137DA0).
    const RETREAT_SEE_ALL: bool;
    /// Plan 2 without a move: 0x1108C0 writes the position itself and keeps
    /// the target; 0x1382C0 asks 0x27FCA0 and clears the target (also when
    /// the search found no candidate).
    const IMMOBILE_STANCE: bool;
    /// What the dispatch returns after a secondary plan 0xE: 0 in
    /// 0x10D210 (the turn ends), 1 in 0x135B00.
    const STAY_B_RETURN: i32;
    /// The radius of the executors' route scoring 0x278600 in plans 2/3/6:
    /// 2 in 0x110FC0 / 0x1108C0 (and Pegasus's copies), 4 in
    /// Labyrinth-ruler's 0x134A90 / 0x1344B0.
    const DANGER_RADIUS: i32 = 2;
    /// The route-scoring radius of plans 2/3/6 for this decision:
    /// [`Self::DANGER_RADIUS`] in every module but Joey's, whose executors
    /// 0x152FA0 / 0x1535A0 ask 0x154380 first (5 with three or more own
    /// units in slots 1..=10, else 2).
    fn danger_radius<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> i32 {
        let _ = w;
        Self::DANGER_RADIUS
    }
    /// Plan 2 after a move: 0x1108C0 / 0x1382C0 keep the target, Keith's
    /// 0x1203B0 clears it (0x121EB0 at 0x120990).
    const ATTACK_MOVE_CLEARS: bool = false;
    /// The radius of the 0x278600 danger queries of a unit that stays put:
    /// the retreat's two queries and plan 2's immobile path. 3 in every
    /// module but Ishtar's (0x125830 / 0x125DD0: 2).
    const STATIC_DANGER_RADIUS: i32 = 3;
    /// A primary plan 6 runs for a unit that has acted: never in 0x10D210
    /// and its copies (the acted test guards plans 2, 3, 5 and 6); Richard
    /// Slysheen of York's 0x13A160 omits the test for plan 6 only.
    const PRIMARY_MOVE_IGNORES_ACTED: bool = false;
    /// A committed face-up magic or ritual card taking the movement path
    /// (plans 3/6 with a valid target, including its own cell, or plan 5
    /// with somewhere to go) is
    /// marked acted by the module itself (0x1E30F0(g, s, 0, 1)) with an
    /// empty type-0xE record: 0x13D400 / 0x13C910 in York's copies of
    /// 0x110FC0 / 0x110330, also present in Manawyddan's copies.
    const FACE_UP_SUPPORT_PASSES: bool = false;
    /// The module's strength function for hidden units, 0x10F670(side) in
    /// Weevil (always 0; the executors inline it as the dead see-all
    /// computation), 0x1353B0(side) in Labyrinth-ruler (LP and monster
    /// count thresholds); Pegasus has none (0). Its value feeds the
    /// executors' danger queries (see-all `& 0xFF`, or the hidden strength
    /// in the retreat) and the plan-9 search's hidden strength.
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        let _ = (w, side);
        Ok(0)
    }
    /// What the executors call to drop a plan slot once it is executed
    /// (the exits of 0x110FC0 / 0x110330 and the immobile path of
    /// 0x1108C0, and the moved path of 0x1203B0 under
    /// `ATTACK_MOVE_CLEARS`): 0x27CAF0, which clears the target, in every
    /// module but Necromancer's, whose 0x12C100 also clears the plan code,
    /// and Ishtar's (0x127210, the same; its dispatch copy calls it too).
    fn clear_plan(rec: &mut Record, which: u8) {
        rec.consume(which);
    }
    /// What the dispatch does before choosing the actor: nothing in
    /// 0x10D210 / 0x135B00; Darkness-ruler's 0x12C440 stores its mode
    /// (0x131580) in the low nibble of side +0x1A1 first.
    fn begin<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mem: &mut Memory) -> R<()> {
        let _ = (w, mem);
        Ok(())
    }
    /// The dispatch 0x10D210 and the copies that share its plan codes
    /// ([`decide`]); Keith's 0x11CED0 knows plan codes of its own and
    /// supplies its own copy.
    fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        d: &mut Decision,
    ) -> R<i32>
    where
        Self: Sized,
    {
        decide::<Self, C, F>(w, mem, d)
    }
    /// The plan slot the dispatch clears after a leader summon chosen by
    /// the actor choice: `None` = the primary when the primary plan is 0xA,
    /// else the secondary (0x10D210 and its copies); the generic module's
    /// 0x16DA10 (Shadi and the VS controllers) always clears the primary.
    const LEADER_SUMMON_SLOT: Option<u8> = None;
    /// The actor choice runs its "any unit that can step" pass before the
    /// leader (Jasper's 0x1553A0, white Manawyddan's 0x168750).
    const STEP_PASS_BEFORE_LEADER: bool = false;
    /// The plan code the dispatch runs a record's code as: Tristan's
    /// 0x142D80 runs 0xF and 0x10 (his row patrol) as plan 6 in both
    /// slots; every other module runs a code as itself.
    fn plan_alias(code: u8) -> u8 {
        code
    }
    /// The card classes the actor choice steps before the leader (0x10D950:
    /// 3 and 4; 0x136240: 3).
    fn trap_actor(class: i32) -> bool;
    /// The plan assignment (0x10D6F0 / 0x136060) for record `idx`.
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()>;
    /// The summon planner (0x10E620 / 0x136AE0).
    fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        key: i32,
        side: i32,
        d: &mut Decision,
    ) -> R<i32>;
    /// The reveal request +0xC (0x10F6D0 / 0x139A50).
    fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        rec: &Record,
    ) -> R<u32>;
    /// Plan 2 executor. Seto's 0x1166F0 omits charge and writes different
    /// reveal/position requests; other modules share the parameterized copy.
    fn attack<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
    ) -> R<i32>
    where
        Self: Sized,
    {
        exec::attack::<Self, _, _>(w, rec, side, d, which)
    }
    /// Plan 2's charge test: the unit at (c, r) jumps onto its target at
    /// (tc, tr) two cells away (0x110C2C / 0x139950).
    #[allow(clippy::too_many_arguments)]
    fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        c: i32,
        r: i32,
        tc: i32,
        tr: i32,
    ) -> R<bool>;
    /// Plan codes the shared dispatch does not know (0xB/0xC/0xD): true
    /// when the module executed `code` for plan slot `which`.
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
        code: u8,
    ) -> R<bool>;
}

/// Weevil's module (0x10D210..0x1119A0).
struct Weevil;

impl Opponent for Weevil {
    const CONTROL: u8 = 2;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = false;
    const STAY_B_RETURN: i32 = 0;
    fn trap_actor(class: i32) -> bool {
        class == 3 || class == 4
    }
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()> {
        assign(w, mem, idx)
    }
    fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        key: i32,
        side: i32,
        d: &mut Decision,
    ) -> R<i32> {
        summon::summon(w, key, side, d)
    }
    fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        rec: &Record,
    ) -> R<u32> {
        exec::reveal(w, g, s, rec)
    }
    /// 0x110C2C: two cells away (measured from the own cell), then 0x110C40.
    fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        c: i32,
        r: i32,
        tc: i32,
        tr: i32,
    ) -> R<bool> {
        Ok((c - tc).abs() + (r - tr).abs() == 2 && exec::charge(w, g, s, c, r)?)
    }
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        _w: &W<C, F>,
        _rec: &mut Record,
        _side: i32,
        _d: &mut Decision,
        _which: u8,
        _code: u8,
    ) -> R<bool> {
        Ok(false)
    }
}

/// 0x10D210 (and the other modules' copies: Pegasus 0x135B00).
fn decide<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    d: &mut Decision,
) -> R<i32> {
    let side = i32::from(w.active);
    O::begin(w, mem)?;
    let k = choose_actor::<O, _, _>(w, mem, side, d)?;
    if k < 0 {
        return Ok(0);
    }
    let Some(idx) = mem.find(k as u16) else {
        return Ok(0);
    };
    if k >> 16 == 1 {
        let which = O::LEADER_SUMMON_SLOT.unwrap_or(u8::from(mem.records[idx].plan_a == 0xA));
        mem.records[idx].consume(which);
        return Ok(1);
    }
    O::assign(w, mem, idx)?;
    let rec = &mut mem.records[idx];
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let acted = w.flag(g, s, 0);
    let mut done = false;
    match O::plan_alias(rec.plan_a) {
        9 => {
            exec::equip_move::<O, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        0xE => {
            stay(w, rec, d);
            done = true;
        }
        code @ (0xB..=0xD) => {
            done = O::extra_plan(w, rec, side, d, 1, code)?;
        }
        3 if !acted => {
            exec::go_to_cell::<O, _, _>(w, rec, side, d, 1, true)?;
            done = true;
        }
        6 if !acted || O::PRIMARY_MOVE_IGNORES_ACTED => {
            exec::go_to_cell::<O, _, _>(w, rec, side, d, 1, false)?;
            done = true;
        }
        0xA => {
            let r = O::summon(w, i32::from(rec.key), side, d)?;
            rec.consume(1);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
            } else {
                done = true;
            }
        }
        2 if !acted => {
            let r = O::attack(w, rec, side, d, 1)?;
            if r < 0 {
                rec.consume(1);
                return Ok(1);
            }
            done = true;
        }
        5 if !acted => {
            exec::retreat::<O, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        _ => (),
    }
    if done {
        return Ok(1);
    }
    match O::plan_alias(rec.plan_b) {
        9 => {
            exec::equip_move::<O, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        // Quirk: plan 0xE in the secondary slot acts but (in Weevil's
        // module) ends the turn.
        0xE => {
            stay(w, rec, d);
            Ok(O::STAY_B_RETURN)
        }
        code @ (0xB..=0xD) => Ok(i32::from(O::extra_plan(w, rec, side, d, 0, code)?)),
        3 | 6 | 2 | 5 if acted => Ok(0),
        3 => {
            exec::go_to_cell::<O, _, _>(w, rec, side, d, 0, true)?;
            Ok(1)
        }
        6 => {
            exec::go_to_cell::<O, _, _>(w, rec, side, d, 0, false)?;
            Ok(1)
        }
        0xA => {
            let r = O::summon(w, i32::from(rec.key), side, d)?;
            rec.consume(0);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
            }
            Ok(1)
        }
        2 => {
            if O::attack(w, rec, side, d, 0)? < 0 {
                exec::retreat::<O, _, _>(w, rec, side, d, 0)?;
            }
            Ok(1)
        }
        5 => {
            exec::retreat::<O, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        _ => Ok(0),
    }
}

/// 0x278800: act in place (type 6 to the own cell, no position change).
fn stay<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &Record, d: &mut Decision) {
    d.key = rec.key;
    d.kind = 6;
    d.target = w.pos(i32::from(rec.key >> 8), i32::from(rec.key & 0xFF)) as u16;
    d.position = 2;
}

/// 0x10D950 (Pegasus: 0x136240): the unit to act next, or −1; bit 16 marks
/// a leader summon (the summon planner has then filled `d`).
fn choose_actor<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    side: i32,
    d: &mut Decision,
) -> R<i32> {
    // 1. Units with a reachable plan-2 target (0x27CCA0).
    let list = w.list(side, 0x1D, true);
    let r = attacker(w, mem, &list)?;
    if r >= 0 {
        return Ok(r);
    }
    // 2. Trap cards that can still step.
    for k in w.list(side, 0x1E, true) {
        let class = w.kind(k >> 8, k & 0xFF) >> 5;
        if O::trap_actor(class) && w.can_step(k)? && w.may_act(k) {
            return Ok(k);
        }
    }
    // Jasper's 0x1553A0 and white Manawyddan's copy 0x168750 run the
    // "any unit that can step" pass before the leader.
    if O::STEP_PASS_BEFORE_LEADER {
        for k in w.list(side, 0x1F, true) {
            if w.can_step(k)? && w.may_act(k) {
                return Ok(k);
            }
        }
    }
    // 3. The leader: summon or move.
    let lk = side << 8;
    let idx = mem.find(lk as u16).unwrap_or(usize::MAX);
    if idx != usize::MAX {
        O::assign(w, mem, idx)?;
    } else {
        return Err(DuelError::InvalidCommand("no record for the leader"));
    }
    if mem.records[idx].plan_a == 0xA {
        if w.can_summon(side) && O::summon(w, lk, side, d)? >= 0 {
            return Ok(lk | 0x10000);
        }
        if w.can_step(lk)? {
            return Ok(lk);
        }
    } else if w.flag(side, 0, 0) {
        if w.can_summon(side) && O::summon(w, lk, side, d)? >= 0 {
            return Ok(lk | 0x10000);
        }
    } else if w.can_step(lk)? {
        return Ok(lk);
    }
    // 4. Any unit that can step; 5. any unit that has not acted.
    for k in w.list(side, 0x1F, true) {
        if w.can_step(k)? && w.may_act(k) {
            return Ok(k);
        }
    }
    for k in w.list(side, 0x1F, true) {
        if !w.flag(k >> 8, k & 0xFF, 0) && w.may_act(k) {
            return Ok(k);
        }
    }
    Ok(-1)
}

/// 0x27CCA0: not-acted units whose primary (else secondary) plan is 2 with
/// a target reachable this turn (0x27CE60), picked by 0x27CF90.
fn attacker<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &Memory,
    list: &[i32],
) -> R<i32> {
    let mut cands = Vec::new();
    for &k in list {
        let (g, s) = (k >> 8, k & 0xFF);
        if w.unit(g, s).is_none() || w.flag(g, s, 0) {
            continue;
        }
        let Some(idx) = mem.find(k as u16) else {
            continue;
        };
        let rec = &mem.records[idx];
        if rec.plan_a == 2 && reachable_target(w, rec, 1)? {
            cands.push(k);
            continue;
        }
        if rec.plan_b == 2 && reachable_target(w, rec, 0)? {
            cands.push(k);
        }
    }
    if cands.is_empty() {
        return Ok(-1);
    }
    // 0x27CF90: quirk — needs two candidates and never considers the first.
    if cands.len() < 2 {
        return Ok(-1);
    }
    let (mut best, mut pick) = (-1, -1);
    for &k in &cands[1..] {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        if !w.flag(g, s, 4) {
            continue;
        }
        let v = w.eff_atk(g, s, p & 0xFF, p >> 8)?;
        if best < v {
            best = v;
            pick = k;
        }
    }
    Ok(pick)
}

/// 0x27CE60(rec, which): the plan target's cell is reachable in one move
/// (0x27B190 with the target as the allowed occupant).
fn reachable_target<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &Record,
    which: u8,
) -> R<bool> {
    let t = i32::from(rec.target(which));
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let p = w.pos(g, s);
    let tp = w.pos(t >> 8, t & 0xFF);
    w.step_ok_k(g, s, p & 0xFF, p >> 8, tp & 0xFF, tp >> 8, t)
}

/// 0x10D6F0: (re)assign the plans of record `idx`.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    idx: usize,
) -> R<()> {
    let mode = evolution_mode(w);
    mem.x1a1 = (mem.x1a1 & 0xF0) | (mode & 0xF);
    let x1a1 = mem.x1a1;
    let rec = &mut mem.records[idx];
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() || w.flag(g, s, 0) || rec.key == 0xFFFF {
        return Ok(());
    }
    if s == 0 {
        if rec.targets() != 0xFFFF_FFFF {
            return Ok(());
        }
        if w.threatened(g, 0, false)? {
            rec.set_plans(if w.can_summon(g) { 0x50A } else { 0x5FF });
        } else if w.can_summon(g) {
            rec.set_plans(0xA06);
            rec.target_b = 1;
        } else {
            rec.set_plans(0x6FF);
            rec.target_a = 1;
        }
        return Ok(());
    }
    let class = w.kind(g, s) >> 5;
    match class {
        0 => {
            if has_effect(w, g, s) == 1 {
                plans::effect_monster(w, rec, 1, x1a1 & 0xF)?;
            } else {
                plans::monster(w, rec, 1, x1a1 & 0xF)?;
            }
        }
        1 => plans::magic(w, rec)?,
        2 => plans::power_up(w, rec, 2, 0, 0)?,
        3 | 4 => plans::formation(w, rec, 2, 0),
        5 => plans::formation(w, rec, 1, 0),
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x2520F0: −1 for non-monsters, 1 when any effect entry is present.
fn has_effect<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, s: i32) -> i32 {
    if w.kind(g, s) >> 5 != 0 {
        return -1;
    }
    match w.unit(g, s) {
        Some(u) if u.entries.iter().any(Option::is_some) => 1,
        _ => 0,
    }
}

/// Weevil's evolution pieces (0x10F900): Great Moth, Perfectly Ultimate
/// Great Moth, Javelin Beetle; Javelin Beetle Pact, Cocoon of Evolution.
const EVOLVED: [i32; 3] = [0x192, 0x193, 0x1A6];
const EVOLUTION_PIECES: [i32; 2] = [0x350, 0x31E];

/// 0x10F900: 1 if an evolved monster is out; else 0 while a Pact or Cocoon
/// is on the field, in hand or in the deck; else 1.
fn evolution_mode<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> u8 {
    let side = i32::from(w.active);
    // 0x24E230(side, 1): own non-leader units, own group first.
    let mut own = Vec::new();
    for g in [side, i32::from(side == 0)] {
        for s in 1..11 {
            if w.unit(g, s).is_some() && w.ctrl(g, s) == side {
                own.push((g, s));
            }
        }
    }
    if own
        .iter()
        .any(|&(g, s)| EVOLVED.contains(&(w.card_id(g, s) & 0xFFFF)))
    {
        return 1;
    }
    if own
        .iter()
        .any(|&(g, s)| EVOLUTION_PIECES.contains(&(w.card_id(g, s) & 0xFFFF)))
    {
        return 0;
    }
    if (0..5).any(|i| EVOLUTION_PIECES.contains(&(w.hand(side, i) & 0xFFF))) {
        return 0;
    }
    for i in 0..w.deck_count(side) {
        if EVOLUTION_PIECES.contains(&(w.deck(side, i) & 0xFFFF)) {
            return 0;
        }
    }
    1
}

#[cfg(test)]
mod tests;
