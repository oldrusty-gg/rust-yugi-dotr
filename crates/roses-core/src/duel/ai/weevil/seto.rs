//! Seto's PAL controller (driver 0x118560, decision 0x113460).
//! [F/emu] Ported against the original instructions and private decision/
//! helper oracles. Card 736's unterminated stack scan remains explicitly
//! unsupported, as do shared searches that depend on undefined register halves.

pub(super) mod executors;
pub(super) mod reveal;
mod summon;
#[cfg(test)]
mod tests;

use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{neighbour, unit_mask};
use super::world::{World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

pub(super) struct Seto;

impl Opponent for Seto {
    const CONTROL: u8 = 1;
    // 0x116EF0: hidden strength 1; the see-all byte is 0x118500(side).
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    const FACE_UP_SUPPORT_PASSES: bool = true;
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn begin<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mem: &mut Memory) -> R<()> {
        mem.x1a1 = (mem.x1a1 & 0xF0) | mode(w)?;
        Ok(())
    }
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        hidden_value(w, side)
    }
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()> {
        assign(w, &mut mem.records[idx], i32::from(mem.x1a1 & 0xF))
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
        reveal::reveal(w, g, s, rec)
    }
    fn attack<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
    ) -> R<i32> {
        executors::attack(w, rec, side, d, which)
    }
    /// 0x1166F0 has no charge branch; it uses its own attack executor.
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
    /// 0x1136F8 / 0x113940: plan B; 0x113758 / 0x1139A0: plan C.
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
        code: u8,
    ) -> R<bool> {
        match code {
            0xB => {
                super::darkness::trap_move::<Seto, _, _>(w, rec, side, d, which, 3)?;
            }
            0xC => executors::plan_c(w, rec, side, d, which)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// 0x27F660(side, 3, card): both graveyards, wrapping u8 count.
fn grave_copies<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> u8 {
    let mut count = 0u8;
    for gy in [side, i32::from(side == 0)] {
        for i in 0..w.graveyard_count(gy) {
            if w.graveyard(gy, i) == card {
                count = count.wrapping_add(1);
            }
        }
    }
    count
}

/// 0x117C10: graveyard card2/0x345 or own field card2 sets mode1.
fn mode<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> R<u8> {
    let side = i32::from(w.active);
    if grave_copies(w, side, 2) != 0 || grave_copies(w, side, 0x345) != 0 {
        return Ok(1);
    }
    Ok(u8::from(unit_mask(w, 0, 0, side, 1, 0x2001C, 0xE)? != 0))
}

/// 0x118500: below1201 LP ->2500; otherwise <4 monsters ->1500, else750.
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    Ok(if w.lp(side) < 1201 {
        2500
    } else if w.monsters(side) < 4 {
        1500
    } else {
        750
    })
}

/// 0x117CB0: LP>=1500, enough empty non-Labyrinth neighbours AND monster
/// slots for all graveyard copies of card2, or (independently) card0.
/// 0x24FC20(side,n,1) enumerates those neighbours, for n=0..7.
fn f117cb0<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if w.lp(side) < 1500 {
        return Ok(0);
    }
    for card in [2, 0] {
        let count = i32::from(grave_copies(w, side, card));
        if count <= 0 {
            continue;
        }
        let mut empty = 0;
        for i in 0..8 {
            let p = neighbour(w, side, i)?;
            if p >= 0 && w.terrain(p & 0xFF, p >> 8) != 8 && w.at(p & 0xFF, p >> 8).is_none() {
                empty += 1;
            }
        }
        if empty >= count && 5 - w.monsters(side) >= count {
            return Ok(1);
        }
    }
    Ok(0)
}

/// 0x117E20: LP>=1500, at least four monsters and own field card2.
/// The original adds two identical masks (0x117E64/0x117E88), not counts.
fn f117e20<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if w.lp(side) < 1500 {
        return Ok(0);
    }
    let mask = unit_mask(w, 0, 0, side, 1, 0x2001C, 0xE)?;
    Ok(i32::from(
        mask.wrapping_add(mask) != 0 && w.monsters(side) >= 4,
    ))
}

/// 0x117ED0: an own non-leader card2, otherwise card0, on the board.
fn f117ed0<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if unit_mask(w, 0, 0, side, 1, 0x2001C, 0xE)? != 0 {
        return Ok(1);
    }
    Ok(i32::from(unit_mask(w, 0, 0, side, 1, 0x1C, 0xE)? != 0))
}

/// 0x117F50: first graveyard card2 (own then enemy), then card0 in that
/// order. Require a free monster slot and no own field card0x179.
fn f117f50<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if w.monsters(side) >= 5 {
        return Ok(-1);
    }
    if grave_copies(w, side, 0) == 0 && grave_copies(w, side, 2) == 0 {
        return Ok(-1);
    }
    if unit_mask(w, 0, 0, side, 1, 0x179001C, 0xE)? != 0 {
        return Ok(-1);
    }
    for card in [2, 0] {
        for gy in [side, i32::from(side == 0)] {
            for i in 0..w.graveyard_count(gy) {
                if w.graveyard(gy, i) == card {
                    return Ok(gy << 16 | i);
                }
            }
        }
    }
    Ok(-1)
}

/// 0x113A10: leader goal(1,1), effect-specific monsters, support and equip.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
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
            rec.set_plans(if summon { 0xA06 } else { 0x6FF });
            if summon {
                rec.target_b = 0x101;
            } else {
                rec.target_a = 0x101;
            }
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        0 if super::has_effect(w, g, s) == 1 => effect_monster(w, rec, mode),
        0 => monster(w, rec, 1, mode),
        1 | 3 => support(w, rec),
        2 => plans::power_up(w, rec, 5, 0, hidden_value(w, w.ctrl(g, s))? & 0xFF),
        _ => {
            rec.set_plans(0x5FF);
            Ok(())
        }
    }
}

/// 0x114030 and wrapper0x1142D0: mode0 has two rounds/radius2;
/// mode1 has five/radius5. Trap-pass ATK limit3000, see-all0.
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    rush: i32,
    mode: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let enemy = i32::from(ctrl == 0);
    let pos = w.pos(g, s);
    let lp = w.pos(enemy, 0);
    if rush != 0
        && search::moves_to_k(
            w,
            g,
            s,
            pos & 0xFF,
            pos >> 8,
            enemy,
            0,
            lp & 0xFF,
            lp >> 8,
            2,
        )? > 0
    {
        set_plan(rec, 2);
        set_target(rec, enemy << 8);
        return Ok(());
    }
    let mut list = Targets::new();
    let mut pool = Pool::new();
    let rounds = if mode == 0 { 2 } else { 5 };
    for n in 1..=rounds {
        let result = plans::attack_passes(
            w,
            rec,
            u,
            pos & 0xFF,
            pos >> 8,
            enemy,
            n,
            hidden_value(w, ctrl)?,
            &mut list,
            &mut pool,
            rounds,
            0,
            3000,
        )?;
        match result {
            0 => {
                set_plan(rec, 5);
                return Ok(());
            }
            1 => return Ok(()),
            2 | 3 => {
                set_plan(rec, if result == 2 { 3 } else { 6 });
                set_target(rec, pos);
                return Ok(());
            }
            _ => (),
        }
    }
    goal_plan(w, rec)
}

/// 0x114430: favoured-terrain goal, signed comparisons in original order.
pub(super) fn goal_cell(c: i32, r: i32) -> i32 {
    if r >= 5 {
        0x602
    } else if c >= 5 {
        0x206
    } else if c < r {
        0x501
    } else {
        0x105
    }
}

/// 0x114330: move to the terrain3 goal, otherwise toward center/corners.
fn goal_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
    rec.set_plans(0x6FF);
    let goal = if w.terrain_favour_of(3, g, s)? == 1 {
        goal_cell(c, r)
    } else if c < 3 && r < 3 {
        0x303
    } else if c >= r {
        0x305
    } else {
        0x503
    };
    set_target(rec, goal);
    Ok(())
}

/// 0x114480: named-card activation plans and card0x89's five reach passes.
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let pos = w.pos(g, s);
    match w.card_id(g, s) {
        0x53 => {
            if f117ed0(w, ctrl)? == 1 {
                set_plan(rec, 3);
                set_target(rec, pos);
            } else {
                set_plan(rec, 5);
            }
            return Ok(());
        }
        0x89 => {
            let mut list = Targets::new();
            let mut pool = Pool::new();
            let mut root = None;
            for n in 1..=5 {
                let hidden = hidden_value(w, ctrl)?;
                root = search::reach(
                    w,
                    &mut pool,
                    &mut list,
                    root,
                    Some(u),
                    pos & 0xFF,
                    pos >> 8,
                    i32::from(ctrl == 0),
                    n,
                    0,
                    hidden,
                )?;
                let k = search::pick_target(
                    w,
                    u,
                    pos & 0xFF,
                    pos >> 8,
                    &list,
                    5,
                    0x34,
                    0,
                    2,
                    0,
                    hidden,
                    i32::from(list.count),
                )?;
                if k >= 0 {
                    set_plan(rec, 2);
                    set_target(rec, k);
                    return Ok(());
                }
            }
        }
        card @ (0xBF | 0x179 | 0x186) => {
            let eligible = match card {
                0x179 => f117cb0(w, ctrl)? == 1,
                0x186 => f117e20(w, ctrl)? == 1,
                _ => true,
            };
            if eligible && plans::strongest_enemy(w, rec, 0)? == 1 {
                rec.set_plans(0xBFF);
                return Ok(());
            }
        }
        _ => (),
    }
    monster(w, rec, 1, mode)
}

/// 0x1147A0: type2 magic approaches(3,3), type14 uses graveyard planC,
/// type4 uses112ED0; a trap with a strongest target uses planB.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(entry) = w.unit(g, s).and_then(|u| u.entries[0]) else {
        rec.set_plans(0x5FF);
        return Ok(());
    };
    match w.kind(g, s) >> 5 {
        1 => match entry[0] >> 8 {
            2 => {
                set_plan(rec, 6);
                set_target(rec, 0x303);
                return Ok(());
            }
            0x14 => {
                set_plan(rec, 0xC);
                return Ok(());
            }
            4 => {
                let target = reveal::f112ed0(w, w.ctrl(g, s), entry)?;
                if target >= 0 {
                    let target = if entry[0] & 0x3F == 0 {
                        (w.pos(g, s) & !0xFF) | (target & 0xFF)
                    } else {
                        target
                    };
                    set_plan(rec, 6);
                    set_target(rec, target);
                    return Ok(());
                }
            }
            _ => (),
        },
        3 if plans::strongest_enemy(w, rec, 0)? == 1 => {
            set_plan(rec, 0xB);
            return Ok(());
        }
        _ => (),
    }
    rec.set_plans(0x5FF);
    Ok(())
}
