//! Tea's controller (opponent 11, the white route's Windsor duel, control
//! byte 11, driver 0x142BB0 = Weevil's 0x1117D0 by opcode shape, module
//! 0x13E570..0x142D78, 30 functions). The module is Labyrinth-ruler's
//! template (dispatch 0x13E570 = 0x131930 plus Darkness-ruler's plan 0xB,
//! executors 0x140BC0 / 0x141100 / 0x1416E0 = 0x133F70 / 0x1344B0 /
//! 0x134A90 with the route-danger radius 3, the trap walk 0x141EC0 =
//! Darkness-ruler's 0x1309F0), Pegasus's actor choice (0x13ECA0 =
//! 0x136240) and charge test (0x142700 = 0x139950), Keith's monster
//! planner shape (0x13F080 = 0x11D9E0 with smaller limits) and a summon
//! subtree assembled from Keith's, Darkness-ruler's and Pegasus's pieces;
//! the shared parts run through the framework in [`super`] with the
//! constants and hooks of [`Tea`].
//!
//! Own functions, by EE address: [`assign`] 0x13EAA0, [`monster`]
//! 0x13F080 (0x13F2F0: 0x27F760 with limit 0x708), [`after_passes`]
//! 0x13F350 with [`grass_cell`] 0x13F3F0, [`effect_monster`] 0x13F5C0,
//! [`reveal`] 0x142460, the card tests [`no_own_kind3`] 0x142800,
//! [`strong_graveyard`] 0x142910, [`many_monsters`] 0x142AF0 and
//! [`weak_trio`] 0x142B50, and the summon planner in [`summon`]
//! (0x13F7B0 and its subtree). The hidden-strength function 0x142400
//! answers 0 on every path (LP threshold 0x7D1, four monsters), so the
//! default [`Opponent::hidden_value`] stands.
//!
//! Two reads of the original depend on stack residue: 0x13F3F0 returns
//! its uninitialised result word when no cell qualifies (see
//! [`grass_cell`]), and the summon planner's first target search reads an
//! uninitialised U (see [`summon`]).

mod summon;

use super::darkness::trap_move;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{atk_def, kind_of, neighbour, nth_unit, popcount, unit_mask};
use super::world::{unsupported, Unit, World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x13F5C0: planned in place unless 0x142800 finds no own kind-3
    /// monster; 0x142460: reveals then.
    pub const CARD_182: i32 = 0x182;
    /// 0x13F5C0 / 0x142460: holds or reveals while its controller's LP is
    /// below the other side's.
    pub const CARD_184: i32 = 0x184;
    /// 0x13F5C0: plan 0xB on the strongest enemy with [`many_monsters`].
    pub const CARD_186: i32 = 0x186;
    /// 0x13F5C0: plan 0xB on the strongest enemy with
    /// [`strong_graveyard`].
    pub const CARD_179: i32 = 0x179;
    /// 0x142460: reveals with three or more attribute-0 monsters.
    pub const CARD_17F: i32 = 0x17F;
    /// 0x13FA40: summoned with [`weak_trio`]; 0x142460: reveals then.
    pub const CARD_170: i32 = 0x170;
    /// Insect Imitation (0x13FC80: summoned with a monster that takes it).
    pub const INSECT_IMITATION: i32 = 0x320;
    /// Paralyzing Potion and Cursebreaker (never with Insect Imitation).
    pub const POTION: i32 = 0x31B;
    pub const CURSEBREAKER: i32 = 0x31C;
}

/// 0x13F2F0: the ATK limit of 0x27F760's first pass (0x708 = 1800).
const TRAP_PASS_ATK_LIMIT: i32 = 0x708;
/// 0x13F080: the move limit of the enemy-leader test (0x27CB40), the last
/// round and the radius of the 0x27F760 passes.
const LEADER_MOVES: i32 = 2;
const PASS_ROUNDS: i32 = 2;
const PASS_RADIUS: i32 = 2;
/// 0x141EC0 (Darkness-ruler's 0x1309F0): the radius of its danger scores.
const TRAP_DANGER_RADIUS: i32 = 3;
/// 0x13F3F0: the terrain it looks for (2) and its distance bound.
const GOAL_TERRAIN: i32 = 2;
const GOAL_DISTANCE_LIMIT: i32 = 0xE;
/// 0x13F350: the word 0x13F3F0 returns when no cell qualifies: its
/// result slot sp+0xA0 is never written then, and holds the return address
/// 0x1E07C0 (frame 0x10, `sq $ra, 0($sp)`) saved there as the last call
/// 0x13F350 made before it (0x13F390 → 0x13F398). [F]
const AFTER_PASSES_RESIDUE: i32 = 0x13F398;
/// 0x142910 / 0x142AF0: LP thresholds (`slti 0x4B0`) and the graveyard
/// ATK threshold (`slti 0x7D1`).
const LOW_LP: i32 = 0x4B0;
const STRONG_ATK: i32 = 0x7D1;
/// 0x142460: the LP limit of the type-0xD magic (`slti 0x7D0`).
const MAGIC_LOW_LP: i32 = 0x7D0;

/// Tea's module.
pub(super) struct Tea;

impl Opponent for Tea {
    const CONTROL: u8 = 11;
    /// 0x1416E0 / 0x141100 (Labyrinth-ruler's copies): strength 1.
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x141494 / 0x141980: radius 3 (4 in 0x1344B0 / 0x134A90).
    const DANGER_RADIUS: i32 = 3;
    /// 0x13ED9C (as Pegasus's 0x13633C): class 3 only.
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()> {
        assign(w, &mut mem.records[idx])
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
        reveal(w, g, s, rec)
    }
    /// 0x142700 = Pegasus's 0x139950.
    fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        c: i32,
        r: i32,
        tc: i32,
        tr: i32,
    ) -> R<bool> {
        super::pegasus::charge(w, g, s, c, r, tc, tr)
    }
    /// 0x13E818 / 0x13E9F4: 0xB → 0x141EC0, executed whatever it returns.
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
        code: u8,
    ) -> R<bool> {
        if code != 0xB {
            return Ok(false);
        }
        trap_move::<Tea, _, _>(w, rec, side, d, which, TRAP_DANGER_RADIUS)?;
        Ok(true)
    }
}

/// 0x13EAA0(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone. The leader (kept while the +8
/// word is not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by
/// the summon flag; else 0xA05 / 0x5FF (no walking target). Non-leaders
/// by class: 3 → plan 0xB when 0x27F060(rec, 0) planned the strongest
/// enemy (its target kept), else 0x5FF; 2 → 0x27E390(rec, 2, 0,
/// 0x142400() & 0xFF = 0); 0 → [`effect_monster`] with an effect entry
/// (0x2520F0 = 1), else [`monster`](self::monster)(rec, 1); 1, 4, 5 and
/// above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    // 0x251C80(g, s): the controller, unused but for the power-up's
    // hidden strength (0x142400 = 0).
    if w.unit(g, s).is_none() || w.flag(g, s, 0) || rec.key == 0xFFFF {
        return Ok(());
    }
    if s == 0 {
        if rec.targets() != 0xFFFF_FFFF {
            return Ok(());
        }
        let summon = w.can_summon(g);
        if w.threatened(g, 0, false)? {
            rec.set_plans(if summon { 0x50A } else { 0x5FF });
        } else {
            rec.set_plans(if summon { 0xA05 } else { 0x5FF });
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        3 => {
            if plans::strongest_enemy(w, rec, 0)? == 1 {
                rec.set_plans(0xBFF);
            } else {
                rec.set_plans(0x5FF);
            }
        }
        2 => plans::power_up(w, rec, 2, 0, 0)?,
        0 => {
            if super::has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec, 1)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x13F080(rec, a1): monsters. `a1` ≠ 0 and 0x27CB40 reaches the enemy
/// leader within 2 moves: plan 2 on its key. Else N = 1..=2: 0x13F2F0 →
/// 0x27F760 (radius 2, see-all 0, hidden strength 0x142400 = 0, limit
/// 0x708): 1 → planned; 0 → plan 5; 2 → plan 3 at the own cell; 3 → plan
/// 6 at the own cell; else next N (the reachable list and the node pool
/// are shared by all rounds, as in 0x10DD60). Exhausted →
/// [`after_passes`].
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let (lg, ls) = (leader >> 8, leader & 0xFF);
    let lp = w.pos(lg, ls);
    // sp+0x140: the reachable list (key 0xFFFF, level 0xFF), once.
    let mut list = Targets::new();
    // 0x2800B0(sp+0xD0): a no-op.
    if a1 != 0 {
        let v = search::moves_to_k(
            w,
            g,
            s,
            my & 0xFF,
            my >> 8,
            lg,
            ls,
            lp & 0xFF,
            lp >> 8,
            LEADER_MOVES,
        )?;
        if v > 0 {
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // sp+0x19C / sp+0x19E: the pool counters, zeroed once.
    let mut pool = Pool::new();
    for n in 1..=PASS_ROUNDS {
        let side = i32::from(w.ctrl(g, s) == 0);
        let v = plans::attack_passes(
            w,
            rec,
            u,
            my & 0xFF,
            my >> 8,
            side,
            n,
            0,
            &mut list,
            &mut pool,
            PASS_RADIUS,
            0,
            TRAP_PASS_ATK_LIMIT,
        )?;
        match v {
            0 => {
                set_plan(rec, 5);
                return Ok(());
            }
            1 => return Ok(()),
            2 => {
                set_plan(rec, 3);
                set_target(rec, my);
                return Ok(());
            }
            3 => {
                set_plan(rec, 6);
                set_target(rec, my);
                return Ok(());
            }
            _ => (),
        }
    }
    after_passes(w, rec)
}

/// 0x13F350(rec): a monster no pass planned for: plan 6 to its
/// [`grass_cell`], or plan 5 when the result is negative. Without a cell
/// the result is [`AFTER_PASSES_RESIDUE`] (positive): plan 6 to 0xF398.
fn after_passes<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let pos = w.pos(g, s);
    // 0x251C80(g, s) is called and its result dropped.
    let Some(u) = w.unit(g, s) else {
        return super::world::unsupported("0x13F350 passes a null unit to 0x13F3F0");
    };
    let v = grass_cell(w, u, pos & 0xFF, pos >> 8)?.unwrap_or(AFTER_PASSES_RESIDUE);
    if v < 0 {
        rec.set_plans(0x5FF);
    } else {
        rec.set_plans(0x6FF);
        set_target(rec, v);
    }
    Ok(())
}

/// 0x13F3F0(u, c, r): the nearest cell (Manhattan from (c, r), below 14;
/// columns outer, rows inner, the first of equals) of terrain 2 that is
/// empty or holds a unit `u`'s controller (0x251C90) does not control and
/// that `u` beats there (0x279E50, hidden strength 0x142400 = 0, see-all
/// 0), as (row << 8) | col. `None` when no cell qualifies: the original
/// then returns its uninitialised result slot (stack residue the callers
/// interpret: [`AFTER_PASSES_RESIDUE`] in 0x13F350, the return address
/// 0x1409E4 of 0x24FD30 — far off the board — in the summon cell).
fn grass_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    c: i32,
    r: i32,
) -> R<Option<i32>> {
    let mut best = GOAL_DISTANCE_LIMIT;
    let mut out = None;
    for col in 0..7 {
        for row in 0..7 {
            if w.terrain(col, row) != GOAL_TERRAIN {
                continue;
            }
            let k = w.at_key(col, row);
            if k >= 0 {
                if w.ctrl(k >> 8, k & 0xFF) == u.ctrl() {
                    continue;
                }
                if w.margin_seen(u, k, 0, col, row, false)? <= 0 {
                    continue;
                }
            }
            let d = (row.max(r) - row.min(r)) + (col.max(c) - col.min(c));
            if d >= 0 && d < best {
                best = d;
                out = Some(row << 8 | col);
            }
        }
    }
    Ok(out)
}

/// 0x13F5C0(rec): monsters with an effect entry, by card (0x1E22B0):
/// - 0x186: plan 0xB when [`many_monsters`] and 0x27F060(rec, 0) planned
///   the strongest enemy;
/// - 0x179: the same with [`strong_graveyard`];
/// - 0x184: plan 6 at the own cell while its controller's LP is below the
///   other side's;
/// - 0x182: plan 6 at the own cell with [`no_own_kind3`];
///
/// otherwise (and for every other card) [`monster`](self::monster)(rec,
/// 1).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return Ok(());
    }
    // 0x24F790(g, s): the controller.
    let ctrl = w.ctrl(g, s);
    let my = w.pos(g, s);
    // sp+0xC0: a reachable list, initialised and unused; 0x2800B0: a no-op.
    match w.card_id(g, s) {
        id::CARD_186 => {
            if many_monsters(w, ctrl) == 1 && plans::strongest_enemy(w, rec, 0)? == 1 {
                rec.set_plans(0xBFF);
                return Ok(());
            }
        }
        id::CARD_179 => {
            if strong_graveyard(w, ctrl)? == 1 && plans::strongest_enemy(w, rec, 0)? == 1 {
                rec.set_plans(0xBFF);
                return Ok(());
            }
        }
        id::CARD_184 => {
            let other = i32::from(ctrl == 0);
            if w.lp(ctrl) < w.lp(other) {
                rec.set_plans(0x6FF);
                set_target(rec, my);
                return Ok(());
            }
        }
        id::CARD_182 => {
            if no_own_kind3(w, ctrl)? == 1 {
                rec.set_plans(0x6FF);
                set_target(rec, my);
                return Ok(());
            }
        }
        _ => (),
    }
    monster(w, rec, 1)
}

/// 0x142800(side): 1 when no kind-3 monster of either group (0x24E360
/// rel 3, type 3, whole board) is controlled by `side` and at least one of
/// them is visible (0x279AF0, see-all 0); else 0. The units are visited
/// from bit 21 down and the first one `side` controls answers 0.
fn no_own_kind3<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    let mask = unit_mask(w, 0, 0, side, 3, 3, 0xE)?;
    let mut seen = 0u8;
    let mut i = 0;
    while i < popcount(mask) {
        let k = nth_unit(mask, i);
        let (g, s) = (k >> 8, k & 0xFF);
        if w.ctrl(g, s) == side {
            return Ok(0);
        }
        if w.visible(g, s, false) {
            seen = seen.wrapping_add(1);
        }
        i += 1;
    }
    Ok(i32::from(seen > 0))
}

/// 0x142910(side): 0 while the side's LP is below 1200. Else the monsters
/// (0x271610 class 0) with printed ATK (0x2714E0 >> 16) of 2001 or more in
/// the side's graveyard, then in the other side's, counted as a byte: with
/// three or more, 1 when at least three neighbours of the side's leader
/// are free (0x24FC20(side, n, 1) for n = 0..=7: not Labyrinth, empty) and
/// the side has room for at least three more monsters (5 − 0x1E13D0 ≥ 3).
fn strong_graveyard<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<i32> {
    if w.lp(side) < LOW_LP {
        return Ok(0);
    }
    let mut strong = 0u8;
    for gy in [side, i32::from(side == 0)] {
        let mut i = 0;
        while i < w.graveyard_count(gy) {
            let card = w.graveyard(gy, i);
            if (atk_def(w, card) >> 16) as i32 >= STRONG_ATK && kind_of(w, card) >> 5 == 0 {
                strong = strong.wrapping_add(1);
            }
            i += 1;
        }
    }
    if strong < 3 {
        return Ok(0);
    }
    let mut free = 0u8;
    for n in 0..8 {
        let cell = neighbour(w, side, n)?;
        if cell < 0 {
            continue;
        }
        let (col, row) = (cell & 0xFF, cell >> 8);
        if w.terrain(col, row) == 8 || w.at_key(col, row) >= 0 {
            continue;
        }
        free = free.wrapping_add(1);
    }
    if free < 3 {
        return Ok(0);
    }
    Ok(i32::from(5 - w.monsters(side) >= 3))
}

/// 0x142AF0(side): 1 when the side's LP is 1200 or more and it has four
/// or more monsters (0x1E13D0); else 0.
fn many_monsters<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    if w.lp(side) < LOW_LP {
        return 0;
    }
    i32::from(w.monsters(side) >= 4)
}

/// 0x142B50(side): 1 when the side controls three or more monsters
/// (0x24E360 rel 1, type 0x25 with 0x3E8, whole board) with an effective
/// ATK of 1000 or less at their cells.
fn weak_trio<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    let mask = unit_mask(w, 0, 0, side, 1, 0x3E8_0025, 0xE)?;
    Ok(i32::from(popcount(mask) >= 3))
}

/// 0x142460(g, s): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters by card: 0x182 → 1 with [`no_own_kind3`]
/// of its controller; 0x17F → 1 with three or more units of group `g`'s
/// relation 1 of attribute 0 (0x24E360 type 0x14, whole board); 0x170 →
/// 1 with [`weak_trio`] of its controller; 0x184 → 1 while group `g`'s LP
/// is below the other group's; else 2. Magic: 2 under a type-0xE far trap
/// (0x27F170 mode 6, see-all 0) or without an entry 0 (0x252060); by its
/// type: 0x25 → 1 while the controller's leader is threatened (0x27D500
/// see-all 0); 0x12 → 1 with four or more kind-8 monsters of relation 1 to
/// group `g`; 0xD → 1 while the controller's LP is below 2000; others 2.
/// Other classes 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    match w.kind(g, s) >> 5 {
        0 => {
            let yes = match card {
                id::CARD_182 => no_own_kind3(w, ctrl)? == 1,
                id::CARD_17F => popcount(unit_mask(w, 0, 0, g, 1, 0x14, 0xE)?) >= 3,
                id::CARD_170 => weak_trio(w, ctrl)? == 1,
                id::CARD_184 => w.lp(g) < w.lp(i32::from(g == 0)),
                _ => false,
            };
            Ok(if yes { 1 } else { 2 })
        }
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let Some([w0, _]) = w.unit(g, s).and_then(|u| u.entry(0)) else {
                return Ok(2);
            };
            let yes = match w0 >> 8 {
                0x25 => w.threatened(ctrl, 0, false)?,
                0x12 => popcount(unit_mask(w, 0, 0, g, 1, 8, 0xE)?) >= 4,
                0xD => w.lp(ctrl) < MAGIC_LOW_LP,
                _ => false,
            };
            Ok(if yes { 1 } else { 2 })
        }
        _ => Ok(2),
    }
}

/// The U the first target search of 0x13F7B0 (and of Tristan's copy
/// 0x144470) reads when the category left
/// it unwritten (its first-affordable-monster answer, with no earlier
/// write): the 0x5C bytes at sp+0x60 of 0x13F7B0's frame are stack
/// residue. The port models the residue the oracle shows on that path
/// (0x27D500's unit list and saved registers of the actor choice): card
/// 0, kind 0, side 0, slot 0, flags 0, no effect entries, (col, row) =
/// (0x60, 0xFF) (bytes of a saved stack pointer), and a record pointer at
/// +0 that reads zeros, i.e. ATK 0 (+0x38 is 0; the printed value is
/// cancelled here because the port reads stats through the card id).
/// [H: the game's residue may differ; measured on the oracle's natural
/// residue, an all-zero U and this model give the same decisions.]
pub(super) fn residue_u<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> R<Unit> {
    let Some(info) = w.card(0) else {
        return unsupported("0x13F7B0's residue U needs card 0's record");
    };
    Ok(Unit {
        card: 0,
        kind: 0,
        attribute: 0,
        col: 0x60,
        row: 0xFF,
        side: 0,
        slot: 0,
        order: 0,
        discount: 0,
        spellbind: 0,
        x4a: 0,
        flags: 0,
        atk_mod: -((info.attack & 0x1FFF) as i16),
        def_mod: -((info.defense & 0x1FFF) as i16),
        entries: [None; 5],
        id: None,
    })
}

#[cfg(test)]
mod tests;
