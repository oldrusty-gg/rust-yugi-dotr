//! Bakura's controller (opponent 18, white route, control byte 18, driver
//! 0x15DA50 = Weevil's 0x1117D0 by opcode shape, module 0x159550..0x15DC18,
//! 29 functions). The dispatch 0x159550 is Manawyddan's red 0x163A00 with
//! plan 0xB (Darkness-ruler's trap walk 0x1309F0 = 0x15CF40) in place of
//! plan 0xC and a plan-9 see-all of 0; the actor choice 0x159C50 is
//! Pegasus's 0x136240; the executors are York's retreat 0x13C730
//! (0x15BB20) and Labyrinth-ruler's attack 0x1344B0 (0x15C0F0) and Seto's
//! move 0x116BC0 (0x15C6D0), all with radius 3; the summon planner
//! 0x15AC30 is Mako's 0x14D470 with Bakura's hidden strength, whose
//! monster choice is his own 0x15B3E0.
//!
//! Own functions, by EE address: [`assign`] 0x159A80, [`leader`] 0x15A030
//! with [`leader_cell`] 0x15A200 (0x15A4B0 / 0x15A9F0 / 0x15AA50 /
//! 0x15ABB0 / 0x15ABF0, a terrain-9 flood fill whose answer 0x15A030
//! overwrites, are not ported), [`monster`] 0x15A5D0 (0x15A850: 0x27F760
//! with limit 0x9C4), [`effect_monster`] 0x15A8B0, [`reveal`] 0x15D480,
//! [`ritual_ok`] 0x15D900, [`hidden_value`] 0x15D9F0 and the summon subtree
//! in [`summon`].

mod summon;

use super::darkness::trap_move;
use super::plans::{self, set_target};
use super::search::{self, Pool, Targets};
use super::seto::reveal::global_mask;
use super::summon::{popcount, ritual_ok_with, unit_mask};
use super::world::{World, R};
use super::{has_effect, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x15A8B0 / 0x15D480: held / revealed with six or more enemy units.
    pub const CARD_28F: i32 = 0x28F;
    /// 0x15A8B0: plan 0xB on the strongest enemy.
    pub const CARD_287: i32 = 0x287;
    /// 0x15A8B0: kept on the strongest enemy while not committed face-up.
    pub const CARD_293: i32 = 0x293;
    /// 0x15D480: revealed by the terrain-0 counts.
    pub const CARD_28C: i32 = 0x28C;
    /// 0x15D480: revealed with fewer than two stars.
    pub const CARD_E0: i32 = 0xE0;
    /// 0x15AEC0: the ritual summoned when 0x15D900 allows it.
    pub const RITUAL_343: i32 = 0x343;
}

/// 0x15A5D0: the enemy-leader move limit, the last round and the radius.
const MOVES: i32 = 3;
const ROUNDS: i32 = 3;
const RADIUS: i32 = 3;
/// 0x15A850: the first pass's ATK limit.
const PASS_ATK_LIMIT: i32 = 0x9C4;
/// 0x15D9F0's LP test (`slti 0x4B1`), monster test (`slti 5`) and answers.
const LOW_LP: i32 = 0x4B1;
const FEW_MONSTERS: i32 = 5;
const HIDDEN_LOW_LP: i32 = 0x7D0;
const HIDDEN_FEW_MONSTERS: i32 = 0x5DC;
const HIDDEN_DEFAULT: i32 = 0x2EE;
/// 0x15CF40 (Darkness-ruler's 0x1309F0): its danger radius.
const TRAP_DANGER_RADIUS: i32 = 3;
/// 0x15A200: the terrain the leader moves to (9), the danger radius, and
/// the answer without a cell.
const LEADER_TERRAIN: i32 = 9;
const LEADER_DANGER_RADIUS: i32 = 3;
const LEADER_NO_CELL: i32 = 0x303;
/// Crush terrain (9).
const CRUSH: i32 = 9;

/// Bakura's module.
pub(super) struct Bakura;

impl Opponent for Bakura {
    const CONTROL: u8 = 18;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    /// 0x159838: t0 = 0.
    const EQUIP_SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x15C484 / 0x15C9FC: radius 3.
    const DANGER_RADIUS: i32 = 3;
    /// 0x15BB20 (York's 0x13C730 with 3).
    const STATIC_DANGER_RADIUS: i32 = 3;
    const FACE_UP_SUPPORT_PASSES: bool = true;
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        Ok(hidden_value(w, side))
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
    /// 0x15D800 = Pegasus's 0x139950.
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
    /// 0x1597C0 / 0x1599A4: 0xB → 0x15CF40.
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
        trap_move::<Bakura, _, _>(w, rec, side, d, which, TRAP_DANGER_RADIUS)?;
        Ok(true)
    }
}

/// 0x15D9F0(side): 2000 while the side's LP is below 1201; else 1500 with
/// fewer than five monsters in its slots 1..=5, else 750.
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    if w.lp(side) < LOW_LP {
        return HIDDEN_LOW_LP;
    }
    if w.monsters(side) < FEW_MONSTERS {
        HIDDEN_FEW_MONSTERS
    } else {
        HIDDEN_DEFAULT
    }
}

/// 0x15D900(side, card): Pegasus's 0x139860 without an ATK limit test:
/// 1 when the ritual's tribute set exists and every tribute's ATK at its
/// cell is negative (i.e. only for an empty set), else 0.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, 0, false)
}

/// 0x159A80(rec): (re)assign the plans of a record. Missing, acted or free
/// records are left alone. The leader (kept while the +8 word is not −1):
/// [`leader`]. Non-leaders by class: 5 → [`monster`](self::monster), then
/// a plan 2 becomes plan 6 to its target's cell; 2 → 0x27E390(rec, 3, 0,
/// 0x15D9F0(controller) & 0xFF); 0 → [`effect_monster`] with an effect
/// entry, else [`monster`](self::monster); others 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    let ctrl = w.ctrl(g, s);
    if w.unit(g, s).is_none() || w.flag(g, s, 0) || rec.key == 0xFFFF {
        return Ok(());
    }
    if s == 0 {
        if rec.targets() != 0xFFFF_FFFF {
            return Ok(());
        }
        return leader(w, rec);
    }
    match w.kind(g, s) >> 5 {
        5 => {
            monster(w, rec)?;
            if rec.plan_a == 2 {
                let t = i32::from(rec.target_a);
                let p = w.pos(t >> 8, t & 0xFF);
                rec.set_plans(0x6FF);
                set_target(rec, p);
            }
        }
        2 => plans::power_up(w, rec, 3, 0, hidden_value(w, ctrl) & 0xFF)?,
        0 => {
            if has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x15A030(rec): the leader: threatened (0x27D500, see-all 0) → 0x50A /
/// 0x5FF by the summon flag. Otherwise 0x15AEC0 (the summon category) is
/// asked and, by its answer, a monster choice and 0x15A4B0's cell are
/// computed, but every path ends in 0x60A / 0x6FF by the summon flag with
/// [`leader_cell`] as the primary target (the earlier target write is
/// overwritten). The category is still run (it may refuse).
fn leader<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.threatened(g, s, false)? {
        rec.set_plans(if w.can_summon(g) { 0x50A } else { 0x5FF });
        return Ok(());
    }
    let mut u = None;
    summon::category(w, g, &mut u)?;
    rec.set_plans(if w.can_summon(g) { 0x60A } else { 0x6FF });
    set_target(rec, leader_cell(w, g, s)?);
    Ok(())
}

/// 0x15A200(g, s): among the cells within the unit's allowance (0x247E10
/// & 0xF, Manhattan, clipped to the board; columns outer, rows inner) on
/// terrain 9 that 0x27B190 can reach (no occupant allowed [H: the original
/// passes a stack address]) and, for a non-leader, that no enemy threatens
/// (0x27D750, see-all 0): the lowest 0x278600 danger (key (g, s), radius 3,
/// hidden 0x15D9F0(g), see-all 0); on a tie an empty cell replaces an
/// occupied best. 0x303 without one.
fn leader_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
) -> R<i32> {
    let p = w.pos(g, s);
    let (c, r) = (p & 0xFF, p >> 8);
    let Some(u) = w.unit(g, s) else {
        return Ok(LEADER_NO_CELL);
    };
    let allow = w.allowance(u, c, r)? & 0xF;
    let (mut best, mut pick) = (0xFFFF, 0xFF);
    for col in (c - allow).max(0)..=(c + allow).min(6) {
        for row in (r - allow).max(0)..=(r + allow).min(6) {
            if (col - c).abs() + (row - r).abs() > allow {
                continue;
            }
            if w.terrain(col, row) != LEADER_TERRAIN {
                continue;
            }
            if !w.step_ok_k(g, s, c, r, col, row, 0xFFFF)? {
                continue;
            }
            if w.cell_threatened(g, s, col, row, false)? && s != 0 {
                continue;
            }
            let d = w.danger(
                col,
                row,
                g << 8 | s,
                LEADER_DANGER_RADIUS,
                hidden_value(w, g) & 0xFFFF,
                false,
            )?;
            if d < best {
                best = d;
                pick = row << 8 | col;
            } else if d == best
                && w.at(col, row).is_none()
                && w.at(pick & 0xFF, pick >> 8).is_some()
            {
                best = d;
                pick = row << 8 | col;
            }
        }
    }
    Ok(if pick == 0xFF { LEADER_NO_CELL } else { pick })
}

/// 0x15A5D0(rec): 0x27CB40 reaches the enemy leader within 3 moves → plan
/// 2 on its key; else N = 1..=3: 0x15A850 → 0x27F760 (radius 3, see-all 0,
/// hidden 0x15D9F0(controller), limit 0x9C4): 0 and 2 → plan 3 at the own
/// cell; 1 → planned; 3 → plan 6 at the own cell. Exhausted → plan 6 to
/// the enemy leader's cell.
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let (c, r) = (my & 0xFF, my >> 8);
    let lp = w.pos(leader >> 8, leader & 0xFF);
    let mut list = Targets::new();
    let v = search::moves_to_k(
        w,
        g,
        s,
        c,
        r,
        leader >> 8,
        leader & 0xFF,
        lp & 0xFF,
        lp >> 8,
        MOVES,
    )?;
    if v > 0 {
        rec.set_plans(0x2FF);
        set_target(rec, leader);
        return Ok(());
    }
    let mut pool = Pool::new();
    for n in 1..=ROUNDS {
        let side = i32::from(w.ctrl(g, s) == 0);
        let v = plans::attack_passes(
            w,
            rec,
            u,
            c,
            r,
            side,
            n,
            hidden_value(w, ctrl),
            &mut list,
            &mut pool,
            RADIUS,
            0,
            PASS_ATK_LIMIT,
        )?;
        match v {
            0 | 2 => {
                rec.set_plans(0x3FF);
                set_target(rec, my);
                return Ok(());
            }
            1 => return Ok(()),
            3 => {
                rec.set_plans(0x6FF);
                set_target(rec, my);
                return Ok(());
            }
            _ => (),
        }
    }
    rec.set_plans(0x6FF);
    set_target(rec, lp);
    Ok(())
}

/// 0x15A8B0(rec): card 0x28F → plan 3 in place with six or more enemy
/// units (rel 2, filter 0x34); 0x287 → 0xBFF when 0x27F060 planned the
/// strongest enemy; 0x293 → kept when not committed face-up and 0x27F060
/// planned; otherwise [`monster`](self::monster).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    match w.card_id(g, s) {
        id::CARD_28F => {
            if popcount(unit_mask(w, 0, 0, ctrl, 2, 0x34, 0xE)?) >= 6 {
                rec.set_plans(0x3FF);
                set_target(rec, w.pos(g, s));
                return Ok(());
            }
        }
        id::CARD_287 => {
            if plans::strongest_enemy(w, rec, 0)? == 1 {
                rec.set_plans(0xBFF);
                return Ok(());
            }
        }
        id::CARD_293 => {
            if !w.flag(g, s, 4) && plans::strongest_enemy(w, rec, 0)? == 1 {
                return Ok(());
            }
        }
        _ => (),
    }
    monster(w, rec)
}

/// 0x15D480(g, s): the reveal request. 0 once committed face-up. Magic: 2
/// under a type-0xE far trap or without an entry; type 0x12 → 2 when an
/// own monster stands on terrain 9 (0x24E360 mode 0x1D), else 1 with three
/// or more own kind-0x13 monsters; 0x18 → 1 with six or more units of the
/// enemy of group `g`; others 2. Rituals: 2 under a far trap; 1 when
/// [`ritual_ok`] and the own cell is not Crush. Monsters: 0x28F → six or
/// more units of the enemy of group `g`; 0x28C → 2 with two or more own
/// units on terrain 0 (mode 0x14), else 1 with six or more enemy ones
/// there; 0xE0 → fewer than two stars; others 2. Others 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
    let ctrl = w.ctrl(g, s);
    let entry0 = w.unit(g, s).and_then(|u| u.entry(0));
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let yes = |b: bool| if b { 1 } else { 2 };
    Ok(match w.kind(g, s) >> 5 {
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let Some([w0, _]) = entry0 else {
                return Ok(2);
            };
            match w0 >> 8 {
                0x12 => {
                    if popcount(unit_mask(w, c, r, ctrl, 1, 0x1D, 0x1D)?) > 0 {
                        2
                    } else {
                        yes(popcount(global_mask(w, ctrl, 1, 0x13)?) >= 3)
                    }
                }
                0x18 => yes(popcount(unit_mask(w, 0, 0, g, 2, 0x34, 0xE)?) >= 6),
                _ => 2,
            }
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            yes(ritual_ok(w, ctrl, card)? == 1 && w.terrain(c, r) != CRUSH)
        }
        0 => match card {
            id::CARD_28F => yes(popcount(unit_mask(w, 0, 0, g, 2, 0x34, 0xE)?) >= 6),
            id::CARD_28C => {
                if popcount(unit_mask(w, c, r, ctrl, 1, 0x34, 0x14)?) >= 2 {
                    2
                } else {
                    yes(popcount(unit_mask(w, c, r, ctrl, 2, 0x34, 0x14)?) >= 6)
                }
            }
            id::CARD_E0 => yes(w.stars(ctrl) < 2),
            _ => 2,
        },
        _ => 2,
    })
}

#[cfg(test)]
mod tests;
