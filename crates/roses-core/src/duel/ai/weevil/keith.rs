//! Keith's controller (opponent 4, control byte 4, driver 0x121F00 =
//! Weevil's 0x1117D0 by opcode shape, module 0x11CED0..0x1220C8, 32
//! functions). The module is Weevil's template with Pegasus's actor
//! choice (0x11D600) and monster/non-monster choices, its own constants
//! and plan codes, and twelve novel functions; the shared parts (actor
//! choice, executors 0x120FC0 / 0x1203B0 / 0x11FE10 / 0x27E960, the
//! 0x27xxxx helpers) run through the framework in [`super`] with the
//! constants and hooks of [`Keith`].
//!
//! Own functions, by EE address: the dispatch [`dispatch`] 0x11CED0
//! (plan codes 0xF / 0x10, the lane plans, and 0xB, the escort, which the
//! shared one does not know), [`hidden_value`] 0x120350, [`assign`]
//! 0x11D3F0, [`monster`] 0x11D9E0 (0x11DC50: 0x27F760 with radius 4 and
//! limit 0x5DC), [`effect_monster`] 0x11E280, [`trap`] 0x11E4D0 with the
//! target pickers [`pick_kill`] 0x279610 (over the trap simulator
//! [`trap_kills`] 0x24DDA0) and [`pick_level`] 0x278E30, the lane plan
//! [`lane_plan`] 0x11DCB0 with [`lane_zone`] 0x11DFA0, [`lane_cell`]
//! 0x11DDB0, [`lane_free`] 0x11E080 and the lane goals 0x11E1A0 /
//! 0x11E210, the plan-0xF/0x10 executor [`exec::lane`] (the first two
//! branches of 0x120FC0), the plan-0xB executor [`exec::escort`] 0x120A20,
//! [`reveal`] 0x121A80 with [`pick_strongest`] 0x27CF90, [`charge`]
//! 0x121DB0, and the summon planner in [`summon`] (0x11E880 and its
//! subtree).

pub(super) mod exec;
mod summon;

use super::exec as shared;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Steps, Targets};
use super::summon::{popcount, unit_mask};
use super::world::{unsupported, Unit, World, R};
use super::{choose_actor, has_effect, stay, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x11E280: planned as plan 0xB on the strongest enemy (0x27F060).
    pub const ESCORT: i32 = 0x1FD;
    /// 0x11E280: the two defenders (0x27F760 radius 2, then plan 3 unless
    /// four own kind-0xE monsters are out).
    pub const DEFENDER_A: i32 = 0x1FE;
    pub const DEFENDER_B: i32 = 0x1F0;
    /// 0x121A80: reveals with six or more hidden or empty enemy slots.
    pub const REVEAL_HIDDEN: i32 = 0x20B;
    /// 0x121A80: reveals unless the strongest face-up non-leader is own.
    pub const REVEAL_STRONGEST: i32 = 0x1F6;
    /// 0x121A80 / 0x11F3E0: reveals with three own kind-0xE monsters;
    /// the summon category's third choice.
    pub const MACHINE_KING: i32 = 0x202;
    /// 0x11F830 / 0x11EFB0: never the monster choice.
    pub const SKIPPED_MONSTER: i32 = 0x156;
    /// 0x11EFB0: the non-monster taken with three own kind-0xE monsters.
    pub const MACHINE_MAGIC: i32 = 0x2E2;
    /// 0x11EFB0: the non-monster taken with six hidden or empty slots.
    pub const HIDDEN_TRAP: i32 = 0x337;
    /// 0x11F3E0 / 0x11F620: the power-up planned with its monster.
    pub const PIECE_POWER_UP: i32 = 0x31F;
    /// 0x11FC90: the monster whose power-ups are listed.
    pub const LIST_MONSTER: i32 = 0x1E8;
    /// 0x11FC00: the monsters never used as fodder (with 0x1FD, 0x1FE,
    /// 0x1F0, 0x202).
    pub const NO_FODDER_A: i32 = 0x1F6;
    pub const NO_FODDER_B: i32 = 0x20B;
    /// Paralyzing Potion and Cursebreaker (never listed as power-ups).
    pub const POTION: i32 = 0x31B;
    pub const CURSEBREAKER: i32 = 0x31C;
}

/// 0x11DC50 / 0x11E280: the ATK limit of 0x27F760's first pass (0x5DC).
const TRAP_PASS_ATK_LIMIT: i32 = 0x5DC;
/// 0x11D9E0: the move limit of the enemy-leader test (0x27CB40) and the
/// last round and radius of the 0x27F760 passes.
const LEADER_MOVES: i32 = 4;
const PASS_ROUNDS: i32 = 4;
const PASS_RADIUS: i32 = 4;
/// 0x120350's answers: LP below 1001; else fewer than three monsters; else.
const LOW_LP: i32 = 0x3E9;
const HIDDEN_LOW_LP: i32 = 0x7D0;
const HIDDEN_FEW_MONSTERS: i32 = 0x3E8;
const HIDDEN_DEFAULT: i32 = 0x1F4;
/// 0x11E4D0: rounds and bound of the trap's target searches.
const TRAP_ROUNDS: i32 = 3;
const TRAP_BOUND: i32 = 4;
/// Kind 0xE (the module's own monsters).
const MACHINE: i32 = 0xE;
/// 0x11DDB0: the route limit of the lane searches.
const LANE_ROUTE_LIMIT: i32 = 7;
/// 0x11DCB0: rows below this take plan 0xF, the others 0x10.
const LANE_ROW_SPLIT: i32 = 4;

/// Keith's module.
pub(super) struct Keith;

impl Opponent for Keith {
    const CONTROL: u8 = 4;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 1;
    const DANGER_RADIUS: i32 = 4;
    const ATTACK_MOVE_CLEARS: bool = true;
    /// 0x11D7AC (as Pegasus's 0x13633C): class 3 only.
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        Ok(hidden_value(w, side))
    }
    fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        d: &mut Decision,
    ) -> R<i32> {
        dispatch(w, mem, d)
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
    /// 0x121DB0 as 0x1203B0 calls it (0x120728): its seventh argument t2
    /// is the length 0x27B9D0 returned for the route whose first moves are
    /// scored, and the test needs it to be 0. A route with candidates is
    /// at least one move long (0 only means the unit cannot move, with no
    /// candidates), so the charge never fires in the loop.
    fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
        _w: &W<C, F>,
        _g: i32,
        _s: i32,
        _c: i32,
        _r: i32,
        _tc: i32,
        _tr: i32,
    ) -> R<bool> {
        Ok(false)
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

/// 0x120350(side): the strength assumed for hidden units: 0x7D0 (2000)
/// while the side's LP (0x1E06F0) is below 1001; else 0x3E8 (1000) with
/// fewer than three monsters in its slots 1..=5 (0x1E13D0), else 0x1F4
/// (500). The call sites pass it whole (the plan passes, 0x27E960's
/// search strength), `& 0xFFFF` (the searches and dangers of the
/// executors) or `& 0xFF` (the executors' danger see-all flag, always
/// non-zero, and the power-up planner's hidden strength).
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    if w.lp(side) < LOW_LP {
        return HIDDEN_LOW_LP;
    }
    if w.monsters(side) < 3 {
        HIDDEN_FEW_MONSTERS
    } else {
        HIDDEN_DEFAULT
    }
}

/// 0x121DB0(g, s, c, r, tc, tr, route): the unit at (c, r) may jump onto
/// its target at (tc, tr): Manhattan distance exactly 2, `route` 0, on the
/// board, not acted, and favoured by the terrain of (c, r) (0x24F820 = 1).
/// Kept for reference: see [`Opponent::charge`] for [`Keith`].
#[allow(dead_code, clippy::too_many_arguments)]
fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    c: i32,
    r: i32,
    tc: i32,
    tr: i32,
    route: i32,
) -> R<bool> {
    if (tc - c).abs() + (tr - r).abs() != 2 || route != 0 {
        return Ok(false);
    }
    if w.unit(g, s).is_none() || w.flag(g, s, 0) {
        return Ok(false);
    }
    Ok(w.terrain_favour_of(w.terrain(c, r), g, s)? == 1)
}

/// 0x11CED0: Weevil's 0x10D210 with plan codes 0xF and 0x10 (the lane
/// plans, executed like plan 6 by [`exec::lane`]) and 0xB (the escort
/// [`exec::escort`], primary only, with no acted test); the secondary
/// plan 0xE returns 1; the summon planner's failure clears the target with
/// 0x121EB0 (= 0x27CAF0).
fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    d: &mut Decision,
) -> R<i32> {
    let side = i32::from(w.active);
    let k = choose_actor::<Keith, _, _>(w, mem, side, d)?;
    if k < 0 {
        return Ok(0);
    }
    let Some(idx) = mem.find(k as u16) else {
        return Ok(0);
    };
    if k >> 16 == 1 {
        let which = u8::from(mem.records[idx].plan_a == 0xA);
        mem.records[idx].consume(which);
        return Ok(1);
    }
    assign(w, &mut mem.records[idx])?;
    let rec = &mut mem.records[idx];
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let acted = w.flag(g, s, 0);
    let mut done = false;
    match rec.plan_a {
        0xE => {
            stay(w, rec, d);
            done = true;
        }
        9 => {
            shared::equip_move::<Keith, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        3 if !acted => {
            shared::go_to_cell::<Keith, _, _>(w, rec, side, d, 1, true)?;
            done = true;
        }
        0xF | 0x10 if !acted => {
            exec::lane::<Keith, _, _>(w, rec, side, d, 1, false)?;
            done = true;
        }
        6 if !acted => {
            shared::go_to_cell::<Keith, _, _>(w, rec, side, d, 1, false)?;
            done = true;
        }
        0xB => {
            exec::escort::<Keith, _, _>(w, rec, side, d, 1, 0)?;
            done = true;
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d)?;
            rec.consume(1);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
            } else {
                done = true;
            }
        }
        2 if !acted => {
            let r = shared::attack::<Keith, _, _>(w, rec, side, d, 1)?;
            if r < 0 {
                rec.consume(1);
                return Ok(1);
            }
            done = true;
        }
        5 if !acted => {
            shared::retreat::<Keith, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        _ => (),
    }
    if done {
        return Ok(1);
    }
    match rec.plan_b {
        9 => {
            shared::equip_move::<Keith, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        0xE => {
            stay(w, rec, d);
            Ok(1)
        }
        3 | 0x10 | 0xF | 6 | 2 | 5 if acted => Ok(0),
        3 => {
            shared::go_to_cell::<Keith, _, _>(w, rec, side, d, 0, true)?;
            Ok(1)
        }
        0x10 | 0xF => {
            exec::lane::<Keith, _, _>(w, rec, side, d, 0, false)?;
            Ok(1)
        }
        6 => {
            shared::go_to_cell::<Keith, _, _>(w, rec, side, d, 0, false)?;
            Ok(1)
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d)?;
            rec.consume(0);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
            }
            Ok(1)
        }
        2 => {
            if shared::attack::<Keith, _, _>(w, rec, side, d, 0)? < 0 {
                shared::retreat::<Keith, _, _>(w, rec, side, d, 0)?;
            }
            Ok(1)
        }
        5 => {
            shared::retreat::<Keith, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        _ => Ok(0),
    }
}

/// 0x11D3F0(rec): (re)assign the plans of a record (no mode, no side
/// struct). Missing, acted or free records are left alone. The leader
/// (kept while the +8 word is not −1): threatened (0x27D500, see-all 0)
/// → 0x50A / 0x5FF by the summon flag, else 0xA05 / 0xFF05 (no walking
/// target). Non-leaders by class: 2 → 0x27E390(rec, 4, 0,
/// 0x120350(controller) & 0xFF); 3 → [`trap`]; 0 → [`effect_monster`]
/// when 0x2520F0 answers 1, else [`monster`](self::monster)(rec, 1); 1,
/// 4, 5 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
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
            rec.set_plans(if summon { 0xA05 } else { 0xFF05 });
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        2 => plans::power_up(w, rec, 4, 0, hidden_value(w, ctrl) & 0xFF)?,
        3 => trap(w, rec)?,
        0 => {
            if has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec, 1)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x11D9E0(rec, a1): monsters. `a1` ≠ 0 and 0x27CB40 reaches the enemy
/// leader within 4 moves: plan 2 on its key. Else N = 1..=4: 0x11DC50 →
/// 0x27F760 (radius 4, see-all 0, hidden 0x120350(controller), limit
/// 0x5DC): 1 → planned; 0 → plan 5; 2 → plan 3 at the own cell; 3 → plan
/// 6 at the own cell; else next N (the reachable list and the node pool
/// are shared by all rounds, as in 0x10DD60). Exhausted → [`lane_plan`].
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
        let hidden = hidden_value(w, ctrl);
        let v = plans::attack_passes(
            w,
            rec,
            u,
            my & 0xFF,
            my >> 8,
            side,
            n,
            hidden,
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
    lane_plan(w, rec)
}

/// 0x11E280(rec): monsters with an effect entry.
/// - 0x1FD: plan 0xB (the escort) when 0x27F060(rec, 0) planned the
///   strongest enemy; else [`monster`](self::monster)(rec, 0).
/// - 0x1FE / 0x1F0: 0x27F760 for N = 1, 2 (radius 2, see-all 0, hidden
///   0x120350(controller), limit 0x5DC): 1 → planned, 0 / 2 / 3 → plan 3
///   at the own cell; exhausted: plan 3 at the own cell with four or more
///   own kind-0xE monsters (0x24E360 rel 1, type 0xE), else
///   [`monster`](self::monster)(rec, 0).
/// - Others: [`monster`](self::monster)(rec, 0).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let my = w.pos(g, s);
    // sp+0x130: the reachable list; sp+0x18C / sp+0x18E: pool counters.
    let mut list = Targets::new();
    let mut pool = Pool::new();
    match w.card_id(g, s) {
        id::ESCORT => {
            if plans::strongest_enemy(w, rec, 0)? == 1 {
                rec.set_plans(0xBFF);
                return Ok(());
            }
            monster(w, rec, 0)
        }
        id::DEFENDER_A | id::DEFENDER_B => {
            let side = i32::from(ctrl == 0);
            for n in 1..=2 {
                let hidden = hidden_value(w, ctrl);
                let v = plans::attack_passes(
                    w,
                    rec,
                    u,
                    my & 0xFF,
                    my >> 8,
                    side,
                    n,
                    hidden,
                    &mut list,
                    &mut pool,
                    2,
                    0,
                    TRAP_PASS_ATK_LIMIT,
                )?;
                match v {
                    1 => return Ok(()),
                    0 | 2 | 3 => {
                        set_plan(rec, 3);
                        set_target(rec, my);
                        return Ok(());
                    }
                    _ => (),
                }
            }
            // 0x24E360(0, 0, controller, 1, 0xE, 0xE).
            let own = unit_mask(w, 0, 0, ctrl, 1, MACHINE as u32, 0xE)?;
            if popcount(own) < 4 {
                return monster(w, rec, 0);
            }
            set_plan(rec, 3);
            set_target(rec, my);
            Ok(())
        }
        _ => monster(w, rec, 0),
    }
}

/// 0x11E4D0(rec): traps. The plan is 0xB (the escort) at once; the target:
/// rounds N = 1..=3 of the reachable-unit tree over the enemy's units
/// (0x278A20 from the own cell, see-all 0, hidden 0x120350(controller)),
/// after each the first unit [`pick_kill`] accepts among face-up ones,
/// then among face-down ones (bound 4, filter 0x1F, def-mode 2, mode 1);
/// else rounds 1..=3 over the own units (a fresh pool and count, the list
/// entries kept) and [`pick_level`] for the own leader (filter 0x22,
/// face-mode 2), a face-up own unit, then a face-down one (bound 4,
/// def-mode 2). Nothing → plan 5.
fn trap<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let hidden = hidden_value(w, w.ctrl(g, s));
    trap_with(w, rec, TRAP_ROUNDS, TRAP_BOUND, hidden)
}

/// [`trap`] with the module's constants: `rounds` (3 here, 2 in Rex
/// Raptor's 0x119820), `bound` (4 here, 3 there) and the hidden strength
/// of the searches and the pickers (0x120350(controller) here, 0x11B2E0 =
/// 0 there).
pub(super) fn trap_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    rounds: i32,
    bound: i32,
    hidden: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let my = w.pos(g, s);
    let (c, r) = (my & 0xFF, my >> 8);
    rec.set_plans(0xBFF);
    // sp+0x130: the list (key 0xFFFF, level 0xFF), once; sp+0x18E its
    // count, sp+0x18A / sp+0x18C the pool counters, sp+0xC0 the root.
    let mut list = Targets::new();
    let mut pool = Pool::new();
    let mut root = None;
    for n in 1..=rounds {
        let other = i32::from(w.ctrl(g, s) == 0);
        root = search::reach(
            w,
            &mut pool,
            &mut list,
            root,
            Some(u),
            c,
            r,
            other,
            n,
            0,
            hidden,
        )?;
        let count = i32::from(list.count);
        for face_mode in [1, 0] {
            let k = pick_kill(
                w, g, s, c, r, &list, bound, 0x1F, face_mode, 2, false, hidden, count, 1,
            )?;
            if k >= 0 {
                set_target(rec, k);
                return Ok(());
            }
        }
    }
    list.count = 0;
    let mut pool = Pool::new();
    let mut root = None;
    for n in 1..=rounds {
        let own = w.ctrl(g, s);
        root = search::reach(
            w,
            &mut pool,
            &mut list,
            root,
            Some(u),
            c,
            r,
            own,
            n,
            0,
            hidden,
        )?;
        let count = i32::from(list.count);
        for (filter, face_mode) in [(0x22, 2), (0x1F, 1), (0x1F, 0)] {
            let k = pick_level(w, c, r, &list, bound, filter, face_mode, 2, count)?;
            if k >= 0 {
                set_target(rec, k);
                return Ok(());
            }
        }
    }
    rec.set_plans(0x5FF);
    Ok(())
}

/// 0x279610(g, s, c, r, list, bound, filter, face_mode, [def_mode,
/// see_all, hidden, count, mode]): 0x278AA0 for the trap (g, s): the
/// target among the first `count` list entries: unit present,
/// Manhattan(unit, (c, r)) ≤ bound, 0x279A90 on +0x57 bit 2 with
/// `face_mode` and bit 5 with `def_mode`, 0x27DEE0(filter), visible
/// (0x279AF0) and either destroyed by the trap at its cell
/// ([`trap_kills`]) or `mode` = 1, 1 ≤ level ≤ bound; taken when level <
/// best (initially 7; best becomes the chosen unit's Manhattan distance
/// and bound its level), or on level == best when its 0x279B90 margin
/// against the trap (`hidden` for a unit the trap cannot read) exceeds
/// below the current best's. The key, or −1.
#[allow(clippy::too_many_arguments)]
fn pick_kill<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    c: i32,
    r: i32,
    list: &Targets,
    bound: i32,
    filter: i32,
    face_mode: i32,
    def_mode: i32,
    see_all: bool,
    hidden: i32,
    count: i32,
    mode: i32,
) -> R<i32> {
    let mut bound = bound & 0xFF;
    let (face_mode, def_mode) = (face_mode & 0xFF, def_mode & 0xFF);
    let count = count & 0xFFFF;
    let mut best = 7;
    let mut best_key = 0xFFFF;
    let mut best_pos = 0xFFFF;
    for i in 0..count {
        let Some(&e) = list.entries.get(i as usize) else {
            return unsupported("0x279610 count beyond the 22-entry list");
        };
        let (eg, es) = (i32::from(e.key >> 8), i32::from(e.key & 0xFF));
        if w.unit(eg, es).is_none() {
            continue;
        }
        let p = w.pos(eg, es);
        let (col, row) = (p & 0xFF, p >> 8);
        let dist = (col - c).abs() + (row - r).abs();
        if bound < dist {
            continue;
        }
        if search::flag_mode(i32::from(w.flag(eg, es, 2)), face_mode) != 1 {
            continue;
        }
        if search::flag_mode(i32::from(w.flag(eg, es, 5)), def_mode) != 1 {
            continue;
        }
        if !W::<C, F>::filter_slot(es, filter) {
            continue;
        }
        let visible = w.visible(eg, es, see_all);
        if !visible || (trap_kills(w, g, s, eg, es, col, row)? != 1 && mode != 1) {
            continue;
        }
        let lv = i32::from(e.level);
        if bound < lv || lv <= 0 {
            continue;
        }
        if lv < best {
            best_key = i32::from(e.key);
            best = dist & 0xFF;
            best_pos = p & 0xFFFF;
            bound = lv & 0xFF;
        } else if lv == best {
            let Some(me) = w.unit(g, s) else {
                return unsupported("0x279610 without the trap's unit");
            };
            let v = if visible {
                w.margin(me, i32::from(e.key), col, row)?
            } else {
                hidden
            };
            let (bg, bs) = (best_key >> 8, best_key & 0xFF);
            let mb = if w.visible(bg, bs, see_all) {
                w.margin(me, best_key, best_pos & 0xFF, best_pos >> 8)?
            } else {
                hidden
            };
            if v < mb {
                best_key = i32::from(e.key);
                best = dist & 0xFF;
                best_pos = p & 0xFFFF;
                bound = lv & 0xFF;
            }
        }
    }
    Ok(if best_key == 0xFFFF { -1 } else { best_key })
}

/// 0x24DDA0(tg, ts, g, s, c, r): the limited-range trap (tg, ts) would
/// destroy the unit (g, s) standing on (c, r), by the trap's kind index
/// (card byte 1, jump table 0x2D30F0): 0..=5 a monster, 6 / 7 / 9 / 10 a
/// monster whose ATK at (c, r) (0x252300) is below 501 / 1001 / 2001 /
/// 3001, 8 a monster whose ATK with bonuses (0x1E1510) is below 1501, 11
/// anything, 12 a non-monster; 13 and above nothing. A hit is cancelled
/// by the leader guard 0x2738C0 (ability #11).
fn trap_kills<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    tg: i32,
    ts: i32,
    g: i32,
    s: i32,
    c: i32,
    r: i32,
) -> R<i32> {
    // 0x1E1A40: byte 1 of the card record (kind index), −1 without one.
    let index = w
        .unit(tg, ts)
        .and_then(|t| w.card(t.card))
        .map_or(-1, |info| i32::from(info.kind_index));
    if !(0..13).contains(&index) {
        return Ok(0);
    }
    let monster = w.kind(g, s) >> 5 == 0;
    let guard = || -> R<i32> {
        // 0x2738C0(g, s, c, r, 0xB) == 0 → 1.
        Ok(i32::from(!w.leader_guard(g, s, c, r, 0xB)?))
    };
    let hit = match index {
        0..=5 => monster,
        6 => monster && w.eff_atk(g, s, c, r)? < 0x1F5,
        7 => monster && w.eff_atk(g, s, c, r)? < 0x3E9,
        8 => monster && w.atk(g, s, false)? < 0x5DD,
        9 => monster && w.eff_atk(g, s, c, r)? < 0x7D1,
        10 => monster && w.eff_atk(g, s, c, r)? < 0xBB9,
        11 => return Ok(1),
        _ => !monster,
    };
    if hit {
        guard()
    } else {
        Ok(0)
    }
}

/// 0x278E30(c, r, list, bound, [filter, face_mode, def_mode, count]): the
/// unit among the first `count` list entries: present, 0x279A90 on +0x57
/// bit 2 with `face_mode` and bit 5 with `def_mode`, 0x27DEE0(filter),
/// 1 ≤ level ≤ bound; taken when level < best (initially 7; best becomes
/// its Manhattan distance to (c, r) and bound its level), or on level ==
/// best when its card's deck cost (card byte 3) exceeds the best's. The
/// key, or −1.
#[allow(clippy::too_many_arguments)]
fn pick_level<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    c: i32,
    r: i32,
    list: &Targets,
    bound: i32,
    filter: i32,
    face_mode: i32,
    def_mode: i32,
    count: i32,
) -> R<i32> {
    let mut bound = bound & 0xFF;
    let (face_mode, def_mode) = (face_mode & 0xFF, def_mode & 0xFF);
    let count = count & 0xFFFF;
    let mut best = 7;
    let mut best_key = 0xFFFF;
    let deck_cost = |k: i32| -> R<i32> {
        let card = w.card_id(k >> 8, k & 0xFF);
        // 0x1DE350(card) + 3.
        match w
            .record_of(card)
            .and_then(|_| w.fusions.deck_cost(card as u16))
        {
            Some(v) => Ok(i32::from(v)),
            None => unsupported("0x278E30 reads the deck cost of a card without a record"),
        }
    };
    for i in 0..count {
        let Some(&e) = list.entries.get(i as usize) else {
            return unsupported("0x278E30 count beyond the 22-entry list");
        };
        let (eg, es) = (i32::from(e.key >> 8), i32::from(e.key & 0xFF));
        if w.unit(eg, es).is_none() {
            continue;
        }
        let p = w.pos(eg, es);
        let (col, row) = (p & 0xFF, p >> 8);
        if search::flag_mode(i32::from(w.flag(eg, es, 2)), face_mode) != 1 {
            continue;
        }
        if search::flag_mode(i32::from(w.flag(eg, es, 5)), def_mode) != 1 {
            continue;
        }
        if !W::<C, F>::filter_slot(es, filter) {
            continue;
        }
        let lv = i32::from(e.level);
        if bound < lv || lv <= 0 {
            continue;
        }
        let dist = ((col - c).abs() + (row - r).abs()) & 0xFF;
        if lv < best || (lv == best && deck_cost(best_key)? < deck_cost(i32::from(e.key))?) {
            best_key = i32::from(e.key);
            best = dist;
            bound = lv & 0xFF;
        }
    }
    Ok(if best_key == 0xFFFF { -1 } else { best_key })
}

/// 0x11DCB0(rec): the lane plan of a monster no pass could place. On a
/// lane ([`lane_zone`] ≥ 0): plan 0xF (rows 0..3) or 0x10 (rows 4..6)
/// with no target, unless it already holds one of them. Else plan 6 to
/// the nearest lane goal ([`lane_cell`]), else plan 5.
fn lane_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let p = w.pos(g, s);
    if lane_zone(p) >= 0 {
        if rec.plan_a == 0xF || rec.plan_a == 0x10 {
            return Ok(());
        }
        let code = if p >> 8 < LANE_ROW_SPLIT { 0xF } else { 0x10 };
        rec.set_plans(code << 8 | 0xFF);
        set_target(rec, 0xFFFF);
        return Ok(());
    }
    let u = w.unit(g, s);
    let (goal, _) = lane_cell(w, u, p & 0xFF, p >> 8)?;
    if goal < 0 {
        rec.set_plans(0x5FF);
    } else {
        rec.set_plans(0x6FF);
        set_target(rec, goal & 0xFFFF);
    }
    Ok(())
}

/// 0x11DFA0 (on 0x1E2310's answer, as a position): the lane a cell lies
/// on: 4 for column 6, 1 for column 4 rows 2..4, 0 for column 2 rows
/// 2..4, 3 for column 0; −1 elsewhere (and for −1, column 255).
fn lane_zone(p: i32) -> i32 {
    let (col, row) = (p & 0xFF, p >> 8);
    match col {
        6 if (0..7).contains(&row) => 4,
        4 if (2..5).contains(&row) => 1,
        2 if (2..5).contains(&row) => 0,
        0 if (0..7).contains(&row) => 3,
        _ => -1,
    }
}

/// 0x11E080(side, i): the strip of lane `i` (column 2 rows 2..4, column 4
/// rows 2..4, column 0 rows 1..5, column 6 rows 1..5; others column 6)
/// holds no monster controlled by `side`. The callers pass side 0.
fn lane_free<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32, i: i32) -> i32 {
    let (col, lo, hi) = match i {
        0 => (2, 2, 4),
        1 => (4, 2, 4),
        2 => (0, 1, 5),
        _ => (6, 1, 5),
    };
    for row in lo..=hi {
        let k = w.at_key(col, row);
        if k < 0 {
            continue;
        }
        let (kg, ks) = (k >> 8, k & 0xFF);
        if w.ctrl(kg, ks) == side && w.kind(kg, ks) >> 5 == 0 {
            return 0;
        }
    }
    1
}

/// 0x11E1A0(_, i) / 0x11E210(_, i): the two goals of lane `i` as (row <<
/// 8) | col: (4, 2) and (2, 4) for lane 0, (2, 2) and (4, 4) for lane 1,
/// (0, 0) and (0, 6) for lane 2, (6, 0) and (6, 6) for lane 3 and above.
fn lane_goals(i: i32) -> [i32; 2] {
    match i {
        0 => [0x204, 0x402],
        1 => [0x202, 0x404],
        2 => [0x000, 0x600],
        _ => [0x006, 0x606],
    }
}

/// 0x11DDB0(u, c, r, &out): the goal cell (and its route length, 0xFF
/// when none) with the fewest moves from (c, r): for each free lane
/// ([`lane_free`] with side 0) its two goals in turn, 0x27AC00 (at most 7
/// moves, the goal's occupant enterable) in a step buffer shared by every
/// search, strict < from 0xFF, negative results skipped. (−1, 0xFF) when
/// no goal has a route.
pub(super) fn lane_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: Option<&Unit>,
    c: i32,
    r: i32,
) -> R<(i32, u8)> {
    let mut out = -1;
    let mut len = 0xFFu8;
    let mut buf = Steps::new();
    for i in 0..4 {
        if lane_free(w, 0, i) != 1 {
            continue;
        }
        for goal in lane_goals(i) {
            let (gc, gr) = (goal & 0xFF, goal >> 8);
            let occ = match w.at_key(gc, gr) {
                k if k < 0 => 0xFFFF,
                k => k,
            };
            let d = search::path(w, u, &mut buf, c, r, gc, gr, occ & 0xFFFF, LANE_ROUTE_LIMIT)?;
            if d < i32::from(len) && d >= 0 {
                len = d as u8;
                out = goal;
            }
        }
    }
    Ok((out, len))
}

/// 0x121A80's counting loop (0x121AD8), also 0x11EFB0's (0x11F10C): over
/// slots 1..=4 of group `g` and of the other group, units of kind 0xE
/// (any class) whose controller 0x251C80 is `g`.
pub(super) fn machines<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32) -> i32 {
    let other = i32::from(g == 0);
    let mut count = 0u8;
    for slot in 1..5 {
        for grp in [g, other] {
            if w.kind(grp, slot) & 0x1F == MACHINE && w.ctrl(grp, slot) == g {
                count = count.wrapping_add(1);
            }
        }
    }
    i32::from(count)
}

/// 0x121A80's second loop (0x121C54): over slots 1..=10 of group `side`
/// and of the other group, the slots whose controller 0x251C80 is not
/// `side` and whose +0x57 bit 4 is clear (empty slots of the other group
/// included).
fn hidden_enemy_slots<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    let other = i32::from(side == 0);
    let mut count = 0;
    for slot in 1..11 {
        for grp in [side, other] {
            if w.ctrl(grp, slot) != side && !w.flag(grp, slot, 4) {
                count += 1;
            }
        }
    }
    count
}

/// 0x24E230(side, 3): every non-leader (slots 1..10), group `side` first.
fn all_units<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> Vec<i32> {
    let mut out = Vec::new();
    for g in [side, i32::from(side == 0)] {
        for s in 1..11 {
            if w.unit(g, s).is_some() {
                out.push(g << 8 | s);
            }
        }
    }
    out
}

/// 0x27CF90(list): the key with the highest 0x252300 ATK at its own cell
/// among list[1..] (quirk: entry 0 is never considered and fewer than two
/// entries give −1) whose +0x57 bit 4 is set; ties keep the earlier one.
pub(super) fn pick_strongest<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    list: &[i32],
) -> R<i32> {
    if list.len() < 2 {
        return Ok(-1);
    }
    let (mut best, mut pick) = (-1, -1);
    for &k in &list[1..] {
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

/// 0x121A80(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. By class (jump table 0x2BA330): monsters 0x20B →
/// 1 with six or more hidden or empty enemy slots
/// ([`hidden_enemy_slots`]), 0x1F6 → 1 unless the strongest face-up
/// non-leader ([`pick_strongest`] over 0x24E230(enemy, 3)) is controlled
/// by the own side, 0x202 → 1 with three or more own kind-0xE units
/// ([`machines`]), others 2; magic: 2 under a type-0xE far trap (0x27F170
/// mode 6) or without an entry, entry-0 type 0x12 → as 0x202, others 2;
/// power-ups, traps and rituals 0.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let ctrl = w.ctrl(g, s);
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let count = machines(w, g);
    let by_count = || if count < 3 { 2 } else { 1 };
    match w.kind(g, s) >> 5 {
        0 => match w.card_id(g, s) {
            id::REVEAL_HIDDEN => Ok(if hidden_enemy_slots(w, ctrl) < 6 {
                2
            } else {
                1
            }),
            id::REVEAL_STRONGEST => {
                let enemy = i32::from(ctrl == 0);
                let k = pick_strongest(w, &all_units(w, enemy))?;
                Ok(if w.ctrl(k >> 8, k & 0xFF) == ctrl {
                    2
                } else {
                    1
                })
            }
            id::MACHINE_KING => Ok(by_count()),
            _ => Ok(2),
        },
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let Some([w0, _]) = w.unit(g, s).and_then(|u| u.entry(0)) else {
                return Ok(2);
            };
            Ok(if w0 >> 8 == 0x12 { by_count() } else { 2 })
        }
        2..=5 => Ok(0),
        _ => Ok(0),
    }
}

#[cfg(test)]
mod tests;
