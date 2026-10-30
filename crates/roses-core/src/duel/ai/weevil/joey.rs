//! Joey's controller (opponent 15, the white route's Stonehenge duel,
//! control byte 15, driver 0x154910 = Weevil's 0x1117D0 by opcode shape,
//! module 0x150610..0x154AD8, 25 functions). The dispatch 0x150610 is
//! Tea's 0x13E570 (Labyrinth-ruler's 0x131930 plus Darkness-ruler's plan
//! 0xB); the actor choice 0x150D40 is Pegasus's 0x136240; the executors
//! are York's retreat 0x13C730 (0x1529D0, static radius 3),
//! Labyrinth-ruler's attack 0x1344B0 (0x152FA0), Seto's move 0x116BC0
//! (0x1535A0) and Darkness-ruler's plan 0xB 0x1309F0 (0x153E20), the last
//! three with a route-danger radius chosen per call by [`crowded`] (5 or
//! 2); the summon planner 0x151510 is Margaret's 0x1488F0 with Joey's
//! hidden strength and a monster limit of 6; the shared parts run through
//! the framework in [`super`] with the constants and hooks of [`Joey`].
//!
//! Own functions, by EE address: [`crowded`] 0x154380, [`hidden_value`]
//! 0x1547A0, [`assign`] 0x150B40, [`planner`] 0x151120 (0x1514B0:
//! 0x27F760 with limit 0x7D0), [`graveyard_pick`] 0x154800, [`reveal`]
//! 0x1543B0 and the summon planner in [`summon`] (0x151510 and its
//! subtree).

mod summon;

use super::darkness::trap_move;
use super::ishtar::leader_plan;
use super::plans::{self, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{kind_of, popcount, unit_mask, virt};
use super::world::{unsupported, World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x151120: planned 0xC (which the dispatch does not run: the turn
    /// ends when it is chosen); 0x1517E0: summoned when
    /// [`super::graveyard_pick`] answers exactly 1.
    pub const CARD_2AE: i32 = 0x2AE;
    /// 0x1543B0: revealed with two or more own monsters on terrain 2.
    pub const CARD_D5: i32 = 0xD5;
    /// 0x1543B0: revealed with two or more own kind-0 monsters of ATK
    /// 2399 or less.
    pub const CARD_24: i32 = 0x24;
    /// 0x151A40: card 0x31F with an affordable card 7 summons card 0x1F5.
    pub const PAIR_LEAD: i32 = 0x31F;
    pub const PAIR_BASE: i32 = 7;
    pub const PAIR_RESULT: i32 = 0x1F5;
}

/// 0x154380: own units in slots 1..=10 (0x1E1390) from which the module
/// is [`crowded`].
const CROWD: i32 = 3;
/// 0x152FA0 / 0x1535A0 / 0x153E20 / 0x151120: the route-danger radius and
/// the pass radius and rounds when crowded, else.
const CROWDED_RADIUS: i32 = 5;
const SPARSE_RADIUS: i32 = 2;
/// 0x151120: the move limit of the enemy-leader test (0x27CB40).
const LEADER_MOVES: i32 = 2;
/// 0x1514B0: the ATK limit of 0x27F760's first pass (0x7D0).
const PASS_ATK_LIMIT: i32 = 0x7D0;
/// 0x151120: the row splitting the board for the exhausted planner.
const HALF_ROW: i32 = 4;
/// 0x151120: the row of the exhausted planner's target across the half.
const FRONT_ROW: i32 = 3;
/// 0x150BEC: the leader's held cell (0x27FB50(rec, 3, 3, 0)).
const HOLD_COL: i32 = 3;
const HOLD_ROW: i32 = 3;
/// 0x1547A0's LP test (`slti 0x4B1`), monster test (`slti 4`) and
/// answers.
const LOW_LP: i32 = 0x4B1;
const FEW_MONSTERS: i32 = 4;
const HIDDEN_LOW_LP: i32 = 0x5DC;
const HIDDEN_FEW_MONSTERS: i32 = 0x3E8;
const HIDDEN_DEFAULT: i32 = 0x1F4;
/// 0x154800: the starting deck-cost bar (`addiu 0x28`) and the entry type
/// it passes over (0x14).
const GRAVEYARD_MIN_COST: i32 = 0x28;
const REVIVAL_TYPE: u32 = 0x14;
/// 0x1543B0: the strongest enemy's ATK from which the type-0x21 magic is
/// revealed (`slti 0x7D0`), and the words of the monster tests.
const STRONG_ENEMY_ATK: i32 = 0x7D0;
const DRAGON_WORD: u32 = 0x095F_0037;
const DRAGON_MODE: i32 = 0xE;
const TERRAIN2_WORD: u32 = 0x1D;
const TERRAIN2_MODE: i32 = 0x16;

/// Joey's module.
pub(super) struct Joey;

impl Opponent for Joey {
    const CONTROL: u8 = 15;
    /// 0x1535A0 (= Seto's 0x116BC0) and 0x152FA0: strength 1.
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x1529D0 (York's 0x13C730 with 3).
    const STATIC_DANGER_RADIUS: i32 = 3;
    /// York's block in 0x1529D0 and Seto's in 0x1535A0.
    const FACE_UP_SUPPORT_PASSES: bool = true;
    /// 0x152FE0 / 0x153004..: `movz` 5 / 2 on 0x154380.
    fn danger_radius<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> i32 {
        radius(w)
    }
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
    /// 0x1546A0 = Pegasus's 0x139950.
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
    /// 0x150884 / 0x150A94: 0xB → 0x153E20 (Darkness-ruler's 0x1309F0
    /// with the radius of [`radius`]), executed whatever it returns.
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
        trap_move::<Joey, _, _>(w, rec, side, d, which, radius(w))?;
        Ok(true)
    }
}

/// 0x154380(): 1 when the active side (0x1DF7C0) has three or more units
/// in its slots 1..=10 (0x1E1390), else 0.
fn crowded<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> bool {
    let side = i32::from(w.active);
    w.monsters(side) + w.others(side) >= CROWD
}

/// The radius (and the planner's last round) [`crowded`] selects.
fn radius<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> i32 {
    if crowded(w) {
        CROWDED_RADIUS
    } else {
        SPARSE_RADIUS
    }
}

/// 0x1547A0(side): the strength assumed for hidden units: 1500 while the
/// side's LP is below 1201; else 1000 with fewer than four monsters in its
/// slots 1..=5, else 500.
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

/// 0x150B40(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone. The leader (kept while the +8
/// word is not −1): [`crowded`] → 0x27FB50 on (col 3, row 3), see-all 0;
/// else threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by the summon
/// flag, else 0xA05 / 0x5FF. Non-leaders by class: 2 → 0x27E390(rec, 5,
/// 0, 0x1547A0(controller) & 0xFF); 0, 1, 3 and 4 → [`planner`]; 5 and
/// above → 0x5FF.
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
        if crowded(w) {
            return leader_plan(w, rec, HOLD_COL, HOLD_ROW, false);
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
        2 => plans::power_up(w, rec, 5, 0, hidden_value(w, ctrl) & 0xFF)?,
        0 | 1 | 3 | 4 => planner(w, rec)?,
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x151120(rec): every unit but power-ups and rituals. Card 0x2AE → plan
/// 0xCFF (target kept). 0x27CB40 reaches the enemy leader within 2 moves:
/// plan 2 (monsters) or 0xB (others) on its key. Else N = 1..=R (R and
/// the radius 5 when [`crowded`], else 2): 0x1514B0 → 0x27F760 (see-all
/// 0, hidden strength 0x1547A0(controller), limit 0x7D0; the reachable
/// list and node pool shared by the rounds): 0 and 2 → plan 3 at the own
/// cell; 1 → planned (a non-monster's plan becomes 0xBFF, its target
/// kept); 3 → plan 6 at the own cell; else next N. Exhausted → plan 6 to
/// the enemy leader's cell when the unit and that leader are on the same
/// half of the board (rows 0..=3 or 4..=6), else to (own column, row 3).
fn planner<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let crowd = crowded(w);
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let (c, r) = (my & 0xFF, my >> 8);
    let lp = w.pos(leader >> 8, leader & 0xFF);
    // 0x2800B0(sp+0xF0): a no-op.
    if w.card_id(g, s) == id::CARD_2AE {
        rec.set_plans(0xCFF);
        return Ok(());
    }
    // sp+0x160: the reachable list, once.
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
        LEADER_MOVES,
    )?;
    if v > 0 {
        rec.set_plans(if w.kind(g, s) >> 5 == 0 { 0x2FF } else { 0xBFF });
        set_target(rec, leader);
        return Ok(());
    }
    let rounds = if crowd { CROWDED_RADIUS } else { SPARSE_RADIUS };
    let mut pool = Pool::new();
    for n in 1..=rounds {
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
            rounds,
            0,
            PASS_ATK_LIMIT,
        )?;
        match v {
            0 | 2 => {
                rec.set_plans(0x3FF);
                set_target(rec, my);
                return Ok(());
            }
            1 => {
                if w.kind(g, s) >> 5 != 0 {
                    rec.set_plans(0xBFF);
                }
                return Ok(());
            }
            3 => {
                rec.set_plans(0x6FF);
                set_target(rec, my);
                return Ok(());
            }
            _ => (),
        }
    }
    rec.set_plans(0x6FF);
    let lr = lp >> 8;
    if (r < HALF_ROW) == (lr < HALF_ROW) {
        set_target(rec, lp);
    } else {
        set_target(rec, FRONT_ROW << 8 | c);
    }
    Ok(())
}

/// 0x154800(side): the enemy graveyard's non-monster (0x271610 class ≠ 0)
/// with the highest record deck cost (byte 3, from 40, ties to the later
/// one) whose entry 0 is not of type 0x14, as (graveyard side << 16) |
/// index; −1 with five cards in the side's slots 6..=10 (0x1E1470) or
/// none.
fn graveyard_pick<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if w.others(side) >= 5 {
        return Ok(-1);
    }
    let gy = i32::from(side == 0);
    let mut best = GRAVEYARD_MIN_COST;
    let mut pick = -1;
    let mut i = 0;
    while i < w.graveyard_count(gy) {
        let card = w.graveyard(gy, i);
        i += 1;
        if kind_of(w, card) >> 5 == 0 {
            continue;
        }
        // 0x1DDFE0 then 0x1E1C60(U, 0): the record and its entry 0.
        let u = virt(w, card);
        let Some(cost) = w.fusions.deck_cost(card as u16) else {
            return unsupported("0x154800 reads a card record's deck cost");
        };
        if i32::from(cost) < best {
            continue;
        }
        let Some([w0, _]) = u.entries[0] else {
            return unsupported("0x154800 reads a null entry 0");
        };
        if w0 >> 8 == REVIVAL_TYPE {
            continue;
        }
        best = i32::from(cost);
        pick = (gy << 16) | (i - 1);
    }
    Ok(pick)
}

/// 0x1543B0(g, s): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters: 0xD5 → 1 with two or more own monsters on
/// terrain 2 (0x24E360 mode 0x16, type 0x1D); 0x24 → 1 with two or more
/// own kind-0 monsters of ATK 2399 or less (type 0x37); others 2. Magic:
/// 2 under a type-0xE far trap (0x27F170 mode 6, see-all 0) or without
/// an entry; entry type 0x21 → 1 when the enemy's strongest face-up
/// monster ([`pick_strongest`] over 0x27DC00(other, 0x1D, 0, 0)) has an
/// ATK of 2000 or more at its cell and the own strongest (over
/// 0x27D9B0(controller, 0x1D, 1)) is weaker; 0xD → 1; others 2. Others 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    let entry0 = w.unit(g, s).and_then(|u| u.entry(0));
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    match w.kind(g, s) >> 5 {
        0 => {
            let mask = match card {
                id::CARD_D5 => unit_mask(w, 0, 0, ctrl, 1, TERRAIN2_WORD, TERRAIN2_MODE)?,
                id::CARD_24 => unit_mask(w, 0, 0, ctrl, 1, DRAGON_WORD, DRAGON_MODE)?,
                _ => return Ok(2),
            };
            Ok(if popcount(mask) >= 2 { 1 } else { 2 })
        }
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let Some([w0, _]) = entry0 else {
                return Ok(2);
            };
            match w0 >> 8 {
                0x21 => {
                    let other = i32::from(ctrl == 0);
                    let enemy = pick_strongest(w, &w.list_seen(other, 0x1D, false, false))?;
                    let own = pick_strongest(w, &w.list(ctrl, 0x1D, true))?;
                    let atk = |k: i32| -> R<i32> {
                        let (kg, ks) = (k >> 8, k & 0xFF);
                        let p = w.pos(kg, ks);
                        w.eff_atk(kg, ks, p & 0xFF, p >> 8)
                    };
                    let e = atk(enemy)?;
                    if e < STRONG_ENEMY_ATK {
                        return Ok(2);
                    }
                    Ok(if atk(own)? < e { 1 } else { 2 })
                }
                0xD => Ok(1),
                _ => Ok(2),
            }
        }
        _ => Ok(2),
    }
}

/// 0x27CF90 ([`super::keith::pick_strongest`]).
fn pick_strongest<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    list: &[i32],
) -> R<i32> {
    super::keith::pick_strongest(w, list)
}

#[cfg(test)]
mod tests;
