//! White Manawyddan fab Llyr's controller (opponent 21, the white route's
//! last duel, control byte 21, driver 0x16BAC0 = Weevil's 0x1117D0 by
//! opcode shape, module 0x168180..0x16BC88, 22 functions). The dispatch
//! 0x168180 is Tristan's 0x142D80 without plan 3 (code 3 is not run in
//! either slot) and with 0xF / 0x10 / 6 as moves, plan 9 with see-all 1;
//! the actor choice 0x168750 is Jasper's 0x1553A0 (the step pass before
//! the leader); the executors are the red Manawyddan's 0x1661C0 / 0x166790
//! / 0x166D70 with the route radius 4; the monster planner 0x168B30 is
//! Keith's 0x11D9E0 (0x168DA0: 0x27F760 with see-all 1 and limit 0xBB8)
//! ending in [`lane`] 0x168E00; the summon planner 0x169070 is the red
//! 0x164CC0 with its own category 0x169300 and cell 0x169EE0.
//!
//! Own functions, by EE address: [`assign`] 0x168600, [`monster`]
//! 0x168B30, [`lane`] 0x168E00, [`watch`] 0x16B9C0, [`reveal`] 0x16B700;
//! the hidden strength 0x16B9B0 answers 0 (the default).

mod summon;

use super::ishtar::leader_plan;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{nth_unit, unit_mask};
use super::world::{World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x168600: the leader's held cell (0x27FB50(rec, 3, 1, see-all 1)).
const HOLD_COL: i32 = 3;
const HOLD_ROW: i32 = 1;
/// 0x168B30 (Keith's 0x11D9E0): moves, rounds, radius; 0x168DA0: limit.
const LEADER_MOVES: i32 = 4;
const PASS_ROUNDS: i32 = 4;
const PASS_RADIUS: i32 = 4;
const PASS_ATK_LIMIT: i32 = 0xBB8;
/// 0x16B9C0: the strongest enemy's ATK and the own unit count.
const STRONG_ATK: i32 = 0xBB8;
const UNITS_NEEDED: i32 = 3;
/// 0x16B700 / 0x169300: cards revealed / summoned below four stars.
pub(super) const CARD_16C: i32 = 0x16C;
pub(super) const CARD_E0: i32 = 0xE0;

/// White Manawyddan's module.
pub(super) struct ManawyddanWhite;

impl Opponent for ManawyddanWhite {
    const CONTROL: u8 = 21;
    const DANGER_HIDDEN: i32 = 0;
    const STANCE_SEE_ALL: bool = true;
    const SEARCH_SEE_ALL: i32 = 1;
    const EQUIP_SEARCH_SEE_ALL: i32 = 1;
    const RETREAT_SEE_ALL: bool = true;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x16AC44 / 0x16B1BC: radius 4 (the red copies: 5).
    const DANGER_RADIUS: i32 = 4;
    const STATIC_DANGER_RADIUS: i32 = 2;
    const FACE_UP_SUPPORT_PASSES: bool = true;
    const STEP_PASS_BEFORE_LEADER: bool = true;
    /// 0x168278..: 0xF / 0x10 run as plan 6; plan 3 is not handled.
    fn plan_alias(code: u8) -> u8 {
        match code {
            0xF | 0x10 => 6,
            3 => 0xFF,
            c => c,
        }
    }
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
    /// 0x16B8B0 = Pegasus's 0x139950.
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

/// 0x168600(rec): the leader (kept while the +8 word is not −1):
/// 0x27FB50 on (col 3, row 1) with see-all 1; class 2 → 0x27E390(rec, 4,
/// 1, 0); class 0 → [`monster`](self::monster)(rec, 1); others 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    if w.unit(g, s).is_none() || w.flag(g, s, 0) || rec.key == 0xFFFF {
        return Ok(());
    }
    if s == 0 {
        if rec.targets() != 0xFFFF_FFFF {
            return Ok(());
        }
        return leader_plan(w, rec, HOLD_COL, HOLD_ROW, true);
    }
    match w.kind(g, s) >> 5 {
        2 => plans::power_up(w, rec, 4, 1, 0)?,
        0 => monster(w, rec, 1)?,
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x168B30(rec, a1): Keith's 0x11D9E0: `a1` ≠ 0 and 0x27CB40 reaches the
/// enemy leader within 4 moves → plan 2 on its key; else N = 1..=4:
/// 0x168DA0 → 0x27F760 (radius 4, see-all 1, hidden 0, limit 0xBB8): 1 →
/// planned; 0 → plan 5; 2 → plan 3 at the own cell; 3 → plan 6 at the own
/// cell. Exhausted → [`lane`].
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
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
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
            1,
            PASS_ATK_LIMIT,
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
    lane(w, rec)
}

/// 0x168E00(rec): the exhausted monster's lane. Primary plan 0x10: on row
/// 6 → 0xFFF to (col, 0), else 0x10FF to (col, 6); 0xF: on row 0 → 0x10FF
/// to (col, 6), else 0xFFF to (col, 0). Otherwise rows 0..=3: row 0 →
/// 0x10FF to (col, 0); column 3 → 0x6FF to (3, 0); columns below 4 →
/// 0x6FF to (0, 0) (the target word keeps only its low half); others 0x6FF
/// to (6, 0). Rows 4..=6: (3, 3) empty → 0x6FF to it; (2, 3) empty → 0x6FF
/// to it; (2, 3) asked again (quirk: the (4, 3) answer 0x304 is never
/// reached) → else 0x6FF to the enemy leader's cell.
fn lane<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let p = w.pos(g, s);
    let (col, row) = (p & 0xFF, p >> 8);
    match rec.plan_a {
        0x10 => {
            if row == 6 {
                rec.set_plans(0xFFF);
                set_target(rec, col);
            } else {
                rec.set_plans(0x10FF);
                set_target(rec, 6 << 8 | col);
            }
        }
        0xF => {
            if row == 0 {
                rec.set_plans(0x10FF);
                set_target(rec, 6 << 8 | col);
            } else {
                rec.set_plans(0xFFF);
                set_target(rec, col);
            }
        }
        _ if row < 4 => {
            if row == 0 {
                rec.set_plans(0x10FF);
                set_target(rec, col);
            } else if col == 3 {
                rec.set_plans(0x6FF);
                set_target(rec, 3);
            } else if col < 4 {
                rec.set_plans(0x6FF);
                set_target(rec, 0);
            } else {
                rec.set_plans(0x6FF);
                set_target(rec, 6);
            }
        }
        _ => {
            if w.at(3, 3).is_none() {
                rec.set_plans(0x6FF);
                set_target(rec, 0x303);
            } else if w.at(2, 3).is_none() {
                rec.set_plans(0x6FF);
                set_target(rec, 0x302);
            } else {
                rec.set_plans(0x6FF);
                set_target(rec, w.pos(i32::from(ctrl == 0), 0));
            }
        }
    }
    Ok(())
}

/// 0x16B9C0(side): 0 when the board's strongest monster (rel 3, type
/// 0x21) is not controlled by `side`, has an ATK of 3000 or more at its
/// cell and the side has three or more units in its slots 1..=10; else 1
/// (`slti $at, count, 3` decides every exit).
pub(super) fn watch<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<i32> {
    let k = nth_unit(unit_mask(w, 0, 0, side, 3, 0x21, 0xE)?, 0);
    let (kg, ks) = (k >> 8, k & 0xFF);
    if w.ctrl(kg, ks) == side {
        return Ok(1);
    }
    let p = w.pos(kg, ks);
    if w.eff_atk(kg, ks, p & 0xFF, p >> 8)? < STRONG_ATK {
        return Ok(1);
    }
    let n = (1..11).filter(|&slot| w.unit(side, slot).is_some()).count() as i32;
    Ok(i32::from(n < UNITS_NEEDED))
}

/// 0x16B700(g, s): 0 once committed face-up. Monsters 0x16C / 0xE0 → 1
/// with fewer than four stars of group `g`, else 2. Magic: 2 under a
/// type-0xE far trap (0x27F170 mode 6, see-all 1) or without an entry;
/// type 4 → [`watch`] of group `g`; 0x27 → fewer than four stars; others
/// 2. Others 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let entry0 = w.unit(g, s).and_then(|u| u.entry(0));
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let yes = |b: bool| if b { 1 } else { 2 };
    Ok(match w.kind(g, s) >> 5 {
        0 => {
            if card == CARD_16C || card == CARD_E0 {
                yes(w.stars(g) < 4)
            } else {
                2
            }
        }
        1 => {
            if w.classify(g, s, 6, true)? == 5 {
                return Ok(2);
            }
            let Some([w0, _]) = entry0 else {
                return Ok(2);
            };
            match w0 >> 8 {
                4 => yes(watch(w, g)? == 1),
                0x27 => yes(w.stars(g) < 4),
                _ => 2,
            }
        }
        _ => 2,
    })
}

#[cfg(test)]
mod tests;
