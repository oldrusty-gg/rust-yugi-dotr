//! Rex Raptor's controller (opponent 3, control byte 3, driver 0x11CD00 =
//! Weevil's 0x1117D0 by opcode shape, module 0x118730..0x11CEC8, 27
//! functions). The module is Keith's template (0x11CED0..) with Weevil's
//! always-0 hidden-strength function, its own constants (danger radius 3,
//! two trap rounds with bound 3, three attack rounds with radius 3) and
//! six novel functions: the lane plan 0x119490 with its lane test
//! 0x1195B0 and the lane goals 0x1196A0 / 0x119760, the type-counter card
//! search 0x11A800 and the reveal test 0x11CA10. The shared parts (actor
//! choice 0x118DE0, the executors 0x11BF50 / 0x11B340 / 0x11ADA0 /
//! 0x11B9B0 / 0x27E960, the 0x27xxxx helpers) run through the framework in
//! [`super`] and Keith's generic executors [`super::keith::exec`] with the
//! constants and hooks of [`Rex`].
//!
//! Own functions, by EE address: the dispatch [`dispatch`] 0x118730 (plan
//! codes 0xF / 0x10 / 6 through 0x11BF50, 0xB through 0x11B9B0; no plan 3
//! branch), [`hidden_value`] 0x11B2E0, [`assign`] 0x118C00, [`monster`]
//! 0x1191C0 (0x119430: 0x27F760 with see-all 0 and limit 0x5DC), the trap
//! planner 0x119820 (Keith's 0x11E4D0 with rounds 2 and bound 3),
//! [`lane_plan`] 0x119490, [`lane_free`] 0x1195B0, [`lane_goal`] 0x1196A0 /
//! 0x119760, [`reveal`] 0x11CA10 with [`type_card`] 0x11A800, the charge
//! test 0x11CBB0 (= Pegasus's 0x139950) and the summon planner in
//! [`summon`] (0x119BD0 and its subtree).

mod summon;

use super::exec as shared;
use super::keith::{self, pick_strongest, trap_with};
use super::plans::{self, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{unit_list, Hand};
use super::world::{World, R};
use super::{choose_actor, stay, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x11A800: the counter card for a strongest enemy of kind 0x12.
    pub const COUNTER_KIND_12: i32 = 0x2D4;
    /// 0x11A800: the counter card for kind 2.
    pub const COUNTER_KIND_2: i32 = 0x2D1;
    /// 0x11A800: the counter card for kind 0xE.
    pub const COUNTER_KIND_E: i32 = 0x2D2;
}

/// 0x119430: the ATK limit of 0x27F760's first pass (0x5DC).
const PASS_ATK_LIMIT: i32 = 0x5DC;
/// 0x1191C0: the move limit of the enemy-leader test (0x27CB40) and the
/// last round and radius of the 0x27F760 passes.
const LEADER_MOVES: i32 = 3;
const PASS_ROUNDS: i32 = 3;
const PASS_RADIUS: i32 = 3;
/// 0x119820: rounds and bound of the trap's target searches.
const TRAP_ROUNDS: i32 = 2;
const TRAP_BOUND: i32 = 3;
/// 0x118C00: the a1 argument of 0x27E390 for power-ups.
const POWER_UP_MODE: i32 = 3;
/// 0x11B9B0: the defflag of 0x27FCA0 when no first move scored (0x11BED8).
const ESCORT_HOLD_DEFFLAG: u8 = 1;
/// 0x119490: rows below this take plan 0xF, the others 0x10.
const LANE_ROW_SPLIT: i32 = 4;
/// 0x119490: the rows whose units walk to a lane goal instead.
const BACK_ROW: i32 = 0;
const FRONT_ROW: i32 = 6;
/// 0x1196A0 / 0x119760: the rows of the lane goals.
const GOAL_ROW_A: i32 = 1;
const GOAL_ROW_B: i32 = 6;

/// Rex Raptor's module.
pub(super) struct Rex;

impl Opponent for Rex {
    const CONTROL: u8 = 3;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 1;
    const DANGER_RADIUS: i32 = 3;
    const ATTACK_MOVE_CLEARS: bool = true;
    /// 0x118F8C (0x118DE0 = Keith's 0x11D600): class 3 only.
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
    /// 0x11CBB0 = Pegasus's 0x139950 (no route-length argument, unlike
    /// Keith's 0x121DB0).
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

/// 0x11B2E0(side): Weevil's 0x10F670 by opcode shape: 0 while the side's
/// LP (0x1E06F0) is below 1501, 0 with fewer than three monsters in its
/// slots 1..=5 (0x1E13D0), else 0 — always 0. The executors inline the
/// same dead computation (0x11B340, 0x11B9B0, 0x11BF50), so every see-all
/// byte and hidden strength of the module is 0.
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    let _ = (w.lp(side) < 0x5DD, w.monsters(side) < 3);
    0
}

/// 0x118730: Keith's 0x11CED0 without the plan-3 branch (a primary plan 3
/// falls through to the secondary plan; a secondary plan 3 ends the turn)
/// and with every plan 0xF / 0x10 / 6 executed by 0x11BF50 with defflag 0
/// ([`keith::exec::lane`]); plan 0xB by the escort 0x11B9B0 (primary
/// without an acted test); the secondary plan 0xE returns 1; the summon
/// planner's failure clears the target with 0x11CCB0 (= 0x27CAF0).
fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    d: &mut Decision,
) -> R<i32> {
    let side = i32::from(w.active);
    let k = choose_actor::<Rex, _, _>(w, mem, side, d)?;
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
            shared::equip_move::<Rex, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        0x10 | 0xF | 6 if !acted => {
            keith::exec::lane::<Rex, _, _>(w, rec, side, d, 1, false)?;
            done = true;
        }
        0xB => {
            keith::exec::escort::<Rex, _, _>(w, rec, side, d, 1, ESCORT_HOLD_DEFFLAG)?;
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
            let r = shared::attack::<Rex, _, _>(w, rec, side, d, 1)?;
            if r < 0 {
                rec.consume(1);
                return Ok(1);
            }
            done = true;
        }
        5 if !acted => {
            shared::retreat::<Rex, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        _ => (),
    }
    if done {
        return Ok(1);
    }
    match rec.plan_b {
        0xE => {
            stay(w, rec, d);
            Ok(1)
        }
        9 => {
            shared::equip_move::<Rex, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        0x10 | 0xF | 6 | 2 | 5 if acted => Ok(0),
        0x10 | 0xF | 6 => {
            keith::exec::lane::<Rex, _, _>(w, rec, side, d, 0, false)?;
            Ok(1)
        }
        0xB => {
            keith::exec::escort::<Rex, _, _>(w, rec, side, d, 0, ESCORT_HOLD_DEFFLAG)?;
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
            if shared::attack::<Rex, _, _>(w, rec, side, d, 0)? < 0 {
                shared::retreat::<Rex, _, _>(w, rec, side, d, 0)?;
            }
            Ok(1)
        }
        5 => {
            shared::retreat::<Rex, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        _ => Ok(0),
    }
}

/// 0x118C00(rec): (re)assign the plans of a record (no mode, no side
/// struct). Missing, acted or free records are left alone. The leader
/// (kept while the +8 word is not −1): threatened (0x27D500, see-all 0)
/// → 0x50A / 0x5FF by the summon flag, else 0xA05 / 0xFF05 (no walking
/// target). Non-leaders by class: 2 → 0x27E390(rec, 3, 0,
/// 0x11B2E0(controller) & 0xFF); 3 → the trap planner 0x119820; 0 →
/// [`monster`](self::monster)(rec, 1) (no effect-monster split); 1, 4, 5
/// and above → 0x5FF.
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
        2 => plans::power_up(w, rec, POWER_UP_MODE, 0, hidden_value(w, ctrl) & 0xFF)?,
        3 => trap_with(w, rec, TRAP_ROUNDS, TRAP_BOUND, hidden_value(w, ctrl))?,
        0 => monster(w, rec, 1)?,
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x1191C0(rec, a1): monsters. `a1` ≠ 0 and 0x27CB40 reaches the enemy
/// leader within 3 moves: plan 2 on its key. Else N = 1..=3: 0x119430 →
/// 0x27F760 (radius 3, see-all 0, hidden 0x11B2E0(controller) = 0, limit
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
            plans::set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // 0x2800B0(sp+0xD0): a no-op; sp+0x19C / sp+0x19E: the pool counters,
    // zeroed once.
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
            PASS_ATK_LIMIT,
        )?;
        match v {
            0 => {
                plans::set_plan(rec, 5);
                return Ok(());
            }
            1 => return Ok(()),
            2 => {
                plans::set_plan(rec, 3);
                set_target(rec, my);
                return Ok(());
            }
            3 => {
                plans::set_plan(rec, 6);
                set_target(rec, my);
                return Ok(());
            }
            _ => (),
        }
    }
    lane_plan(w, rec)
}

/// 0x1195B0(side, i): the strip of lane `i` (column 2 for 0, 4 for 1, 0
/// for 2, 6 for 3 and above; rows 1..=5) holds no monster controlled by
/// `side`. The callers pass the active side (0x1DF7C0) or the summoning
/// side.
fn lane_free<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32, i: i32) -> i32 {
    let col = match i & 0xFF {
        0 => 2,
        1 => 4,
        2 => 0,
        _ => 6,
    };
    for row in 1..6 {
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

/// 0x1196A0(side) (`row` = 1, `free` = true) / 0x119760(side) (`row` =
/// 6, `free` = false): the goal of the first lane free of the side's
/// monsters ([`lane_free`]) — or, for 0x119760 (`bnel` at 0x119784 where
/// 0x1196A0 has `beql`), of the first lane HOLDING one — as (row << 8) |
/// col with the lane's column (2, 4, 0, 6); −1 when no lane qualifies.
fn lane_goal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    row: i32,
    free: bool,
) -> i32 {
    for i in 0..4 {
        if (lane_free(w, side, i) != 0) != free {
            continue;
        }
        let col = match i {
            0 => 2,
            1 => 4,
            2 => 0,
            _ => 6,
        };
        return row << 8 | col;
    }
    -1
}

/// 0x119490(rec): the lane plan of a monster no pass could place, for the
/// active side (0x1DF7C0). In row 6: plan 0x10 (no change to the targets)
/// on the row-6 lane goal (0x119760: the first lane HOLDING an own
/// monster), else plan 6 to that goal; in row 0: plan 0xF on the row-1
/// goal (0x1196A0: the first free lane), else plan 6 to it (a goal of −1
/// gives the target 0xFFFF); in rows 1..5: plan 0xF (rows below 4) or
/// 0x10 with the primary target cleared, unless the record already holds
/// one of them.
fn lane_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let side = i32::from(w.active);
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let p = w.pos(g, s);
    let goal_a = lane_goal(w, side, GOAL_ROW_A, true);
    let goal_b = lane_goal(w, side, GOAL_ROW_B, false);
    let row = p >> 8;
    if row == FRONT_ROW {
        if p == goal_b {
            rec.set_plans(0x10FF);
        } else {
            rec.set_plans(0x6FF);
            set_target(rec, goal_b);
        }
        return Ok(());
    }
    if row == BACK_ROW {
        if p == goal_a {
            rec.set_plans(0xFFF);
        } else {
            rec.set_plans(0x6FF);
            set_target(rec, goal_a);
        }
        return Ok(());
    }
    if rec.plan_a == 0xF || rec.plan_a == 0x10 {
        return Ok(());
    }
    let code: u16 = if row < LANE_ROW_SPLIT { 0xF } else { 0x10 };
    rec.set_plans(code << 8 | 0xFF);
    set_target(rec, 0xFFFF);
    Ok(())
}

/// 0x11A800(side, d, U): the counter card for the strongest enemy: over
/// the units the enemy of `side` controls (0x24E230 rel 2) the strongest
/// face-up one by 0x27CF90 ([`pick_strongest`], which never considers the
/// first entry); by its kind (0x1E1980 & 0x1F): 0x12 → 0x2D4, 2 → 0x2D1,
/// 0xE → 0x2D2, others → −1. d+6.. = 0xFF, then d+6 = the LAST
/// predicted-hand index holding the card (it may stay 0xFF). Returns the
/// card, or −1 (no strongest unit, another kind). `U` is only tested for
/// null, which a stack buffer never is.
pub(super) fn type_card<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    d: &mut Decision,
) -> R<i32> {
    let list = unit_list(w, side, 2);
    let hand = Hand::of(w, side);
    d.hand = [0xFF; 5];
    let k = pick_strongest(w, &list)?;
    if k < 0 {
        return Ok(-1);
    }
    let card = match w.kind(k >> 8, k & 0xFF) & 0x1F {
        0x12 => id::COUNTER_KIND_12,
        2 => id::COUNTER_KIND_2,
        0xE => id::COUNTER_KIND_E,
        _ => return Ok(-1),
    };
    for i in 0..w.next_hand_size(side) {
        if hand.get(i)? == card {
            d.hand[0] = i as u8;
        }
    }
    Ok(card)
}

/// 0x11CA10(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Magic cards: 2 under a type-0xE far trap (0x27F170
/// mode 6) or without an entry; by entry-0 type: 0xD → 1, 2 → 1 when the
/// primary target (+0xA) is the own cell, others → 1 when the card is the
/// counter card of the strongest enemy of its controller ([`type_card`]);
/// else 2. Monsters, power-ups, traps and rituals 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    rec: &Record,
) -> R<u32> {
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    if w.kind(g, s) >> 5 != 1 {
        return Ok(2);
    }
    if w.classify(g, s, 6, false)? == 5 {
        return Ok(2);
    }
    let Some([w0, _]) = w.unit(g, s).and_then(|u| u.entry(0)) else {
        return Ok(2);
    };
    match w0 >> 8 {
        0xD => Ok(1),
        2 => Ok(if i32::from(rec.target_a) == w.pos(g, s) {
            1
        } else {
            2
        }),
        _ => {
            let card = w.card_id(g, s);
            let ctrl = w.ctrl(g, s);
            // 0x1E07C0(g, s): the unit's record, the non-null U of 0x11A800.
            let mut local = Decision::new();
            let v = type_card(w, ctrl, &mut local)?;
            Ok(if card == v { 1 } else { 2 })
        }
    }
}

#[cfg(test)]
mod tests;
