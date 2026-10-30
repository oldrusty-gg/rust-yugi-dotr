//! T. Tristan Grey's controller (opponent 12, the white route's London
//! duel, control byte 12, driver 0x147060 = Weevil's 0x1117D0 by opcode
//! shape, module 0x142D80..0x147228, 28 functions). The module is
//! Labyrinth-ruler's template (dispatch 0x142D80 = 0x131930 with plan
//! codes 0xF and 0x10 run as plan 6, executors 0x145790 / 0x145CD0 /
//! 0x1462B0 = 0x133F70 / 0x1344B0 / 0x134A90 with the route-danger radius
//! 3) with Pegasus's actor choice (0x143440 = 0x136240), charge test
//! (0x146D70 = 0x139950) and ritual test shape (0x146F70 = 0x139860 with
//! the limit 0x9C4), a summon subtree assembled like Tea's from
//! Pegasus's and Darkness-ruler's pieces, and a row-walking monster
//! planner of his own that draws rand(); the shared parts run through the
//! framework in [`super`] with the constants and hooks of [`Tristan`].
//!
//! Own functions, by EE address: [`assign`] 0x143280, [`patrol`]
//! 0x143820 (the monster planner: hold the own row, walk to the column-0
//! cell whose terrain favours the unit, or patrol the row to column 0 or 6
//! as plans 0x10 / 0xF, a coin flip of rand() 0x19AE88 choosing the first
//! leg), [`monster`] 0x143E10 (0x144060: 0x27F760 with limit 0x708),
//! [`row_units`] 0x1440C0, [`favoured_row`] 0x1441D0, [`free_favoured_row`]
//! 0x144240, [`row_card`] 0x144350, [`reveal`] 0x146AF0, [`crowded_front`]
//! 0x146E70 and the summon planner in [`summon`] (0x144470 and its
//! subtree). The hidden-strength function 0x146A90 answers 0 on every
//! path (LP threshold 0x7D1, four monsters), so the default
//! [`Opponent::hidden_value`] stands.

mod summon;

use super::plans::{self, set_plan, set_target};
use super::search::{self, attack_path, Pool, Steps, Targets};
use super::summon::{nth_unit, popcount, ritual_ok_with, unit_mask, virt};
use super::world::{unsupported, Unit, World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x143280: planned by [`super::monster`] (0x143E10) instead of the
    /// row patrol.
    pub const CARD_160: i32 = 0x160;
    /// A ritual (0x1446B0: summoned with acceptable tributes; 0x146AF0).
    pub const RITUAL_34E: i32 = 0x34E;
    /// Magic cards summoned and revealed with [`super::crowded_front`].
    pub const CARD_2DB: i32 = 0x2DB;
    pub const CARD_2EF: i32 = 0x2EF;
    /// 0x146AF0: reveals with three or more own kind-5 monsters.
    pub const CARD_C0: i32 = 0xC0;
    /// 0x146AF0: reveals with three or more enemy units in defence.
    pub const CARD_145: i32 = 0x145;
}

/// 0x144060: the ATK limit of 0x27F760's first pass (0x708 = 1800).
const TRAP_PASS_ATK_LIMIT: i32 = 0x708;
/// 0x143820 / 0x143E10: the move limit of the enemy-leader test.
const LEADER_MOVES: i32 = 2;
/// 0x143E10: the last round and the radius of its 0x27F760 passes.
const PASS_ROUNDS: i32 = 5;
const PASS_RADIUS: i32 = 5;
/// 0x143820: the route limit of its attack searches and the initial
/// "best" length (`7`; a route is kept only when LONGER, so never).
const ATTACK_ROUTE_LIMIT: i32 = 5;
const ATTACK_BEST_START: i32 = 7;
/// 0x143820: an enemy it loses to closer than this holds the unit.
const HOLD_DISTANCE: i32 = 3;
/// 0x146F70: the tribute ATK limit (`slti 0x9C4` = 2500).
const RITUAL_ATK_LIMIT: i32 = 0x9C4;
/// The patrol plans (0x143CDC): 0xF walks to column 6, 0x10 to column 0.
const PATROL_RIGHT: u8 = 0xF;
const PATROL_LEFT: u8 = 0x10;

/// T. Tristan Grey's module.
pub(super) struct Tristan;

impl Opponent for Tristan {
    const CONTROL: u8 = 12;
    /// 0x1462B0 / 0x145CD0 (Labyrinth-ruler's copies): strength 1.
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x146550 / 0x146070: radius 3 (4 in 0x134A90 / 0x1344B0).
    const DANGER_RADIUS: i32 = 3;
    /// 0x142E80..0x142E94 / 0x143074..0x143088: 0x10 and 0xF are plan 6.
    fn plan_alias(code: u8) -> u8 {
        match code {
            PATROL_RIGHT | PATROL_LEFT => 6,
            c => c,
        }
    }
    /// As Pegasus's 0x13633C: class 3 only.
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
    /// 0x146D70 = Pegasus's 0x139950.
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

/// 0x143280(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone. The leader (kept while the +8
/// word is not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by
/// the summon flag; else 0xA05 / 0x5FF. Non-leaders by class: 2 →
/// 0x27E390(rec, 2, 0, 0x146A90() & 0xFF = 0); 0 → card 0x160:
/// [`monster`](self::monster), others [`patrol`]; 1, 3, 4, 5 and above
/// → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    // 0x251C80(g, s): the controller (only for 0x146A90 = 0).
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
        2 => plans::power_up(w, rec, 2, 0, 0)?,
        0 => {
            if w.card_id(g, s) == id::CARD_160 {
                monster(w, rec)?;
            } else {
                patrol(w, rec)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x1440C0(list, row, side, mode): the units on row `row` (columns 0..=6,
/// 0x1E4A10) that `side` controls (mode bit 0) and that it does not
/// (mode bit 1), in column order, into `list` from its start (entries
/// past the count keep what they held); the count as a byte.
fn row_units<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    list: &mut [i32; 7],
    row: i32,
    side: i32,
    mode: i32,
) -> usize {
    let mut n = 0;
    for col in 0..7 {
        let k = w.at_key(col, row);
        if k < 0 {
            continue;
        }
        if mode & 1 != 0 && w.ctrl(k >> 8, k & 0xFF) == side {
            list[n] = k;
            n += 1;
        }
        if mode >> 1 & 1 != 0 && w.ctrl(k >> 8, k & 0xFF) != side {
            list[n] = k;
            n += 1;
        }
    }
    n
}

/// 0x24F7D0 on the terrain of (`col`, `row`): 1 when it favours `u`.
fn favours<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    col: i32,
    row: i32,
) -> R<bool> {
    Ok(w.terrain_bonus_kind(w.terrain(col, row), u)? == 1)
}

/// [`favours`] on column 0 of `row`.
fn column0_favours<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    row: i32,
) -> R<bool> {
    favours(w, u, 0, row)
}

/// 0x1441D0(u): the first row of 1..=5 whose column-0 terrain favours
/// `u` ([`column0_favours`]), or −1.
fn favoured_row<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, u: &Unit) -> R<i32> {
    for row in 1..6 {
        if column0_favours(w, u, row)? {
            return Ok(row);
        }
    }
    Ok(-1)
}

/// 0x144240(u): the first row of 0..=6 holding no unit `u`'s controller
/// (0x251C90) controls whose column-0 terrain favours `u`, or −1.
fn free_favoured_row<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
) -> R<i32> {
    let side = u.ctrl();
    for row in 0..7 {
        let own = (0..7)
            .filter(|&col| {
                let k = w.at_key(col, row);
                k >= 0 && w.ctrl(k >> 8, k & 0xFF) == side
            })
            .count();
        if own == 0 && column0_favours(w, u, row)? {
            return Ok(row);
        }
    }
    Ok(-1)
}

/// 0x144350(card, row): 1 for a monster card (0x271610 class 0) when the
/// active side (0x1DF7C0) controls no unit on `row` and the column-0
/// terrain of that row favours the card's virtual record (0x1DDFE0).
fn row_card<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    card: i32,
    row: i32,
) -> R<bool> {
    let u = virt(w, card);
    if u.kind >> 5 != 0 {
        return Ok(false);
    }
    let side = i32::from(w.active);
    let own = (0..7)
        .filter(|&col| {
            let k = w.at_key(col, row);
            k >= 0 && w.ctrl(k >> 8, k & 0xFF) == side
        })
        .count();
    if own != 0 {
        return Ok(false);
    }
    column0_favours(w, &u, row)
}

/// 0x143820(rec): monsters other than card 0x160.
/// 1. 0x27CB40 reaches the enemy leader within 2 moves: plan 2 on its key.
/// 2. The terrain of its own cell does not favour the unit (0x24F7D0):
///    plan 6 to the [`favoured_row`] in the own column, or
///    [`monster`](self::monster) without one.
/// 3. Else for each enemy unit on the own row ([`row_units`] mode 2): if
///    the unit at least ties it (0x279E50, strength 0, see-all 0, quirk:
///    evaluated at (its column, its column)) an attack route (0x27BA70,
///    at most 5) is searched and would be kept only when longer than 7
///    (never); otherwise, at Manhattan distance below 3, plan 3 at the own
///    cell. The same margin is computed for the enemies on the row below
///    (0x252180(6, row − 1)) and dropped.
/// 4. With two or more own units on the row: plan 6 to the
///    [`free_favoured_row`] in the own column, else [`monster`].
/// 5. Else the patrol: plan 0xF (column 6) continues to column 6 until
///    the unit stands there, then turns to 0x10 (column 0), and the other
///    way round; any other plan draws rand() and starts 0x10 on an even
///    number, 0xF on an odd one.
fn patrol<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    // sp+0x180: the step buffer.
    let mut buf = Steps::new();
    // 0x24F790(g, s): the controller.
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let (mc, mr) = (my & 0xFF, my >> 8);
    let lp = w.pos(leader >> 8, leader & 0xFF);
    // sp+0x110: a reachable list, initialised and unused.
    let _ = Targets::new();
    let v = search::moves_to_k(
        w,
        g,
        s,
        mc,
        mr,
        leader >> 8,
        leader & 0xFF,
        lp & 0xFF,
        lp >> 8,
        LEADER_MOVES,
    )?;
    if v > 0 {
        rec.set_plans(0x2FF);
        set_target(rec, leader);
        return Ok(());
    }
    // sp+0x170: seven keys, 0xFFFF.
    let mut list = [0xFFFF; 7];
    if !favours(w, u, mc, mr)? {
        rec.set_plans(0x6FF);
        let v = favoured_row(w, u)?;
        if v >= 0 {
            set_target(rec, v << 8 | mc);
            return Ok(());
        }
        return monster(w, rec);
    }
    let n = row_units(w, &mut list, mr, ctrl, 2);
    let mut best_len = ATTACK_BEST_START;
    let mut best = -1;
    for &k in &list[..n] {
        let kp = w.pos(k >> 8, k & 0xFF);
        let (kc, kr) = (kp & 0xFF, kp >> 8);
        // Quirk: the cell is (column, column).
        if w.margin_seen(u, k, 0, kc, kc, false)? >= 0 {
            let d = attack_path(
                w,
                Some(u),
                &mut buf,
                mc,
                mr,
                kc,
                kr,
                (k & 0xFFFF) as u32,
                0,
                ATTACK_ROUTE_LIMIT,
                0,
            )?;
            if d > 0 && best_len < d {
                best_len = d;
                best = k;
            }
        } else {
            let d = (kr.max(mr) - kr.min(mr)) + (kc.max(mc) - kc.min(mc));
            if d < HOLD_DISTANCE {
                rec.set_plans(0x3FF);
                set_target(rec, my);
                return Ok(());
            }
        }
    }
    if best >= 0 {
        rec.set_plans(0x2FF);
        set_target(rec, best);
    }
    let below = (mr - 1).min(6);
    let n = row_units(w, &mut list, below, ctrl, 2);
    for &k in &list[..n] {
        let kc = w.pos(k >> 8, k & 0xFF) & 0xFF;
        // The best margin is kept in a register and never read.
        w.margin_seen(u, k, 0, kc, kc, false)?;
    }
    if row_units(w, &mut list, mr, ctrl, 1) >= 2 {
        let v = free_favoured_row(w, u)?;
        if v >= 0 {
            rec.set_plans(0x6FF);
            set_target(rec, v << 8 | mc);
            return Ok(());
        }
        return monster(w, rec);
    }
    match rec.plan_a {
        PATROL_RIGHT => {
            if mc < 6 {
                set_target(rec, mr << 8 | 6);
            } else {
                rec.set_plans(0x10FF);
                set_target(rec, mr << 8);
            }
        }
        PATROL_LEFT => {
            if mc > 0 {
                set_target(rec, mr << 8);
            } else {
                rec.set_plans(0xFFF);
                set_target(rec, mr << 8 | 6);
            }
        }
        _ => {
            if w.rand31() & 1 == 0 {
                rec.set_plans(0x10FF);
                set_target(rec, mr << 8);
            } else {
                rec.set_plans(0xFFF);
                set_target(rec, mr << 8 | 6);
            }
        }
    }
    Ok(())
}

/// 0x143E10(rec): card 0x160 and the patrol's fallback (Manawyddan's
/// 0x164520 shape). 0x27CB40 reaches the enemy leader within 2 moves:
/// plan 2 on its key. Else N = 1..=5: 0x144060 → 0x27F760 (radius 5,
/// see-all 0, hidden strength 0x146A90 = 0, limit 0x708): 0 → 0x5FF; 1 →
/// planned; 2 → plan 3 at the own cell; 3 → plan 6 at the own cell and
/// (quirk: no return) the next N; else the next N. Exhausted → 0x5FF
/// (the target kept).
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return unsupported("0x143E10 for a missing unit");
    };
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let lp = w.pos(leader >> 8, leader & 0xFF);
    // 0x2800B0(sp+0xC0): a no-op; sp+0x130: the reachable list, once.
    let mut list = Targets::new();
    let v = search::moves_to_k(
        w,
        g,
        s,
        my & 0xFF,
        my >> 8,
        leader >> 8,
        leader & 0xFF,
        lp & 0xFF,
        lp >> 8,
        LEADER_MOVES,
    )?;
    if v > 0 {
        rec.set_plans(0x2FF);
        set_target(rec, leader);
        return Ok(());
    }
    // sp+0x18C / sp+0x18E: the pool counters, zeroed once.
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
            }
            _ => (),
        }
    }
    set_plan(rec, 5);
    Ok(())
}

/// 0x146F70(side, card): Pegasus's 0x139860 with the tribute ATK limit
/// 2500.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, RITUAL_ATK_LIMIT, false)
}

/// 0x146E70(side): 1 when the side has fewer than three non-leader units
/// (0x24E360 rel 1, type 0x34, whole board), the enemy five or more, and
/// one of the enemy's (from bit 21 down) stands on a row below 3.
fn crowded_front<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if popcount(unit_mask(w, 0, 0, side, 1, 0x34, 0xE)?) >= 3 {
        return Ok(0);
    }
    let mask = unit_mask(w, 0, 0, side, 2, 0x34, 0xE)?;
    if popcount(mask) < 5 {
        return Ok(0);
    }
    let mut i = 0;
    while i < popcount(mask) {
        let k = nth_unit(mask, i);
        if w.pos(k >> 8, k & 0xFF) >> 8 < 3 {
            return Ok(1);
        }
        i += 1;
    }
    Ok(0)
}

/// 0x146AF0(g, s): the reveal request (+0xC; 1 presses R2), by card. 0
/// once committed face-up. The ritual 0x34E: 2 under a type-0xE far trap
/// (0x27F170 mode 6, see-all 0); 1 when its tributes are acceptable
/// ([`ritual_ok`] for its controller) and it is not on Crush. 0x2EF and
/// 0x2DB: 2 under a type-0xE trap, else 1 with [`crowded_front`] of the
/// controller. 0xC0: 1 with three or more kind-5 monsters of relation 1 to
/// group `g`. 0x145: 1 with three or more of the units of relation 2 to
/// group `g` in defence now (+0x57 bit 3). Others 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let pos = w.pos(g, s);
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let yes = match card {
        id::RITUAL_34E => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            ritual_ok(w, ctrl, id::RITUAL_34E)? == 1 && w.terrain(pos & 0xFF, pos >> 8) != 9
        }
        id::CARD_2EF | id::CARD_2DB => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            crowded_front(w, ctrl)? == 1
        }
        id::CARD_C0 => popcount(unit_mask(w, 0, 0, g, 1, 5, 0xE)?) >= 3,
        id::CARD_145 => {
            let mask = unit_mask(w, 0, 0, g, 2, 0x34, 0xE)?;
            let mut n = 0u8;
            let mut i = 0;
            while i < popcount(mask) {
                let k = nth_unit(mask, i);
                if w.flag(k >> 8, k & 0xFF, 3) {
                    n = n.wrapping_add(1);
                }
                i += 1;
            }
            n >= 3
        }
        _ => false,
    };
    Ok(if yes { 1 } else { 2 })
}

#[cfg(test)]
mod tests;
