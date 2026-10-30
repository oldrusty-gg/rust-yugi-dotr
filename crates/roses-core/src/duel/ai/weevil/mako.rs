//! Mako's controller (opponent 14, the white route's Strait of Dover duel,
//! control byte 14, driver 0x150440 = Weevil's 0x1117D0 by opcode shape,
//! module 0x14C1D0..0x150608, 30 functions). The dispatch 0x14C1D0 is
//! Labyrinth-ruler's 0x131930; the actor choice 0x14C8A0 Pegasus's
//! 0x136240; the executors are York's retreat 0x13C730 (0x14E850, static
//! radius 3), Labyrinth-ruler's attack 0x1344B0 (0x14EE20) and Seto's move
//! 0x116BC0 (0x14F400), both with the route radius 3; the summon planner
//! 0x14D470 is Labyrinth-ruler's 0x132D30 asking for no reveal (+0xC = 2)
//! with Margaret's card list; the shared parts run through the framework
//! in [`super`] with the constants and hooks of [`Mako`].
//!
//! Own functions, by EE address: [`hidden_value`] 0x150280 (Labyrinth-
//! ruler's 0x1353B0 with other thresholds and answers), [`assign`] 0x14C6B0,
//! [`monster`] 0x14CC80 (0x14CF90: 0x27F760 with limit 0x9C4),
//! [`effect_monster`] 0x14CFF0, [`magic`] 0x14D140, the formation of the
//! leader's neighbours 0, 3 and 4 ([`formation_cell`] 0x14D2A0,
//! [`in_formation`] 0x14D240, [`free_formation_cell`] 0x14D330,
//! [`formation_full`] 0x14FC70), [`pair_test`] 0x1502E0, [`reveal`]
//! 0x14FD60, [`ritual_ok`] 0x150190 (Pegasus's 0x139860 with the limit
//! 0x9C4) and the summon planner in [`summon`] (0x14D470 and its
//! subtree).

mod summon;

use super::plans::{self, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{nth_unit, popcount, ritual_ok_with, unit_mask};
use super::world::{World, R};
use super::{has_effect, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x14CFF0: plan 0x27F060's strongest enemy while the formation is
    /// full ([`super::formation_full`]); 0x23F only while not committed
    /// face-up.
    pub const CARD_23F: i32 = 0x23F;
    pub const CARD_23A: i32 = 0x23A;
    /// 0x14CFF0 / 0x14FD60 / 0x1502E0: held in place or revealed by
    /// [`super::pair_test`].
    pub const CARD_227: i32 = 0x227;
    pub const CARD_264: i32 = 0x264;
    /// A ritual (0x14D700: summoned with acceptable tributes).
    pub const RITUAL_354: i32 = 0x354;
    /// Magic cards 0x14D700 summons: 0x2BD with a full formation and no
    /// own magic or trap card, 0x2E6 with four or more own monsters.
    pub const CARD_2BD: i32 = 0x2BD;
    pub const CARD_2E6: i32 = 0x2E6;
}

/// 0x14CF90: the ATK limit of 0x27F760's first pass (0x9C4).
const PASS_ATK_LIMIT: i32 = 0x9C4;
/// 0x14CC80: the move limit of the enemy-leader test, the last round and
/// the radius of the passes.
const LEADER_MOVES: i32 = 3;
const PASS_ROUNDS: i32 = 3;
const PASS_RADIUS: i32 = 3;
/// 0x150280's LP test (`slti 0x4B1`), monster test (`slti 5`) and
/// answers.
const LOW_LP: i32 = 0x4B1;
const FEW_MONSTERS: i32 = 5;
const HIDDEN_LOW_LP: i32 = 0x960;
const HIDDEN_FEW_MONSTERS: i32 = 0x5DC;
const HIDDEN_DEFAULT: i32 = 0x2EE;
/// 0x150190: the tribute ATK limit (`slti 0x9C4` = 2500).
const RITUAL_ATK_LIMIT: i32 = 0x9C4;
/// 0x14D1F8 / 0x14FF88: 0x27E0B0's terrain, minimum and radius.
const OPEN_TERRAIN: i32 = 4;
const OPEN_MIN: i32 = 9;
const OPEN_RADIUS: i32 = 2;
/// Crush terrain (9).
const CRUSH: i32 = 9;

/// Mako's module.
pub(super) struct Mako;

impl Opponent for Mako {
    const CONTROL: u8 = 14;
    /// 0x14F400 (= Seto's 0x116BC0) and 0x14EE20: strength 1.
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x14F72C / 0x14F1B4: radius 3.
    const DANGER_RADIUS: i32 = 3;
    /// 0x14E850 (York's 0x13C730 with 3).
    const STATIC_DANGER_RADIUS: i32 = 3;
    /// York's block in 0x14E850 and Seto's in 0x14F400.
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
    /// 0x150090 = Pegasus's 0x139950.
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

/// 0x150280(side): the strength assumed for hidden units: 2400 while the
/// side's LP is below 1201; else 1500 with fewer than five monsters in its
/// slots 1..=5, else 750.
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

/// 0x24FD30(side, i) as the word it returns (jump tables 0x2D32A0 /
/// 0x2D3280): (row << 8) | col of the leader's neighbour, −1 when a
/// checked +1 offset leaves the board; the −1 offsets are not checked, so
/// a row of −1 gives (−1 << 8) | col and a column of −1 gives −1.
pub(super) fn neighbour_raw<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    i: i32,
) -> R<i32> {
    if !(0..2).contains(&side) || !(0..8).contains(&i) {
        return Ok(-1);
    }
    let p = w.pos(side, 0);
    if p < 0 {
        return super::world::unsupported("0x24FD30 without a leader");
    }
    let (r, c) = (p >> 8, p & 0xFF);
    let at = |dr: i32, dc: i32| ((r + dr) << 8) | (c + dc);
    let ok = |v: i32| v < 7;
    // (row offset, col offset, row checked, col checked) per entry.
    let (dr, dc) = match (side, i) {
        (0, 0) | (1, 1) => (-1, 0),
        (0, 1) | (1, 0) => (1, 0),
        (0, 2) | (1, 3) => (0, 1),
        (0, 3) | (1, 2) => (0, -1),
        (0, 4) | (1, 7) => (-1, 1),
        (0, 5) | (1, 6) => (-1, -1),
        (0, 6) | (1, 5) => (1, 1),
        _ => (1, -1),
    };
    if (dr == 1 && !ok(r + 1)) || (dc == 1 && !ok(c + 1)) {
        return Ok(-1);
    }
    Ok(at(dr, dc))
}

/// 0x14D2A0(i): the formation cells: the active side's leader's
/// neighbours (0x24FD30, through the stub 0x14E300) 0, 3 and 4, as the
/// raw words ([`neighbour_raw`]); −1 past them.
fn formation_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, i: i32) -> R<i32> {
    let side = i32::from(w.active);
    match i {
        0 => neighbour_raw(w, side, 0),
        1 => neighbour_raw(w, side, 3),
        2 => neighbour_raw(w, side, 4),
        _ => Ok(-1),
    }
}

/// 0x14D240(c, r): (c, r) is one of the formation cells.
fn in_formation<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    c: i32,
    r: i32,
) -> R<bool> {
    for i in 0..3 {
        if formation_cell(w, i)? == r << 8 | c {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 0x14D330(): the first formation cell holding no unit (0x1E4960; an
/// off-board cell holds none), negative when that is off the board; −1
/// when all hold one.
fn free_formation_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> R<i32> {
    for i in 0..3 {
        let v = formation_cell(w, i)?;
        if w.at(v & 0xFF, v >> 8).is_none() {
            return Ok(v);
        }
    }
    Ok(-1)
}

/// 0x14FC70(): 1 when [`free_formation_cell`] is negative (no free cell on
/// the board), else 0.
fn formation_full<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> R<i32> {
    Ok(i32::from(free_formation_cell(w)? < 0))
}

/// 0x14C6B0(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone. The leader (kept while the +8
/// word is not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by the
/// summon flag; else 0xA05 / 0x5FF. Non-leaders by class: 1 → [`magic`];
/// 2 → 0x27E390(rec, 5, 0, 0x150280(controller) & 0xFF); 0 →
/// [`effect_monster`] with an effect entry (0x2520F0 = 1), else
/// [`monster`](self::monster); 3, 4, 5 and above → 0x5FF.
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
        let summon = w.can_summon(g);
        if w.threatened(g, 0, false)? {
            rec.set_plans(if summon { 0x50A } else { 0x5FF });
        } else {
            rec.set_plans(if summon { 0xA05 } else { 0x5FF });
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        1 => magic(w, rec)?,
        2 => plans::power_up(w, rec, 5, 0, hidden_value(w, ctrl) & 0xFF)?,
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

/// 0x14CC80(rec): monsters. 0x27CB40 reaches the enemy leader within 3
/// moves: plan 2 on its key. Else N = 1..=3: 0x14CF90 → 0x27F760 (radius
/// 3, see-all 0, hidden strength 0x150280(controller), limit 0x9C4): 0 and
/// 2 → plan 3 at the own cell; 1 → planned; 3 → plan 6 at the own cell;
/// else next N. Exhausted: on a formation cell, plan 2 on the enemy
/// leader's key when the formation is full ([`formation_full`]), else
/// plan 6 at the own cell; elsewhere plan 6 to the
/// [`free_formation_cell`], or plan 2 on the enemy leader without one.
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
    // sp+0x140: the reachable list, once; 0x2800B0(sp+0xD0): a no-op.
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
        rec.set_plans(0x2FF);
        set_target(rec, leader);
        return Ok(());
    }
    let mut pool = Pool::new();
    for n in 1..=PASS_ROUNDS {
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
            PASS_RADIUS,
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
    if in_formation(w, c, r)? {
        if formation_full(w)? == 1 {
            rec.set_plans(0x2FF);
            set_target(rec, leader);
        } else {
            rec.set_plans(0x6FF);
            set_target(rec, my);
        }
        return Ok(());
    }
    let v = free_formation_cell(w)?;
    if v >= 0 {
        rec.set_plans(0x6FF);
        set_target(rec, v);
    } else {
        rec.set_plans(0x2FF);
        set_target(rec, leader);
    }
    Ok(())
}

/// 0x1502E0(side, card): card 0x227: 1 when the side has no kind-0xF
/// monster (0x24E360 rel 1, type 0xF, whole board) and two or more of the
/// enemy's are visible (0x279AF0, see-all 0); card 0x264: 1 with four or
/// more own kind-0x10 monsters; else 0.
fn pair_test<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    match card {
        id::CARD_227 => {
            if popcount(unit_mask(w, 0, 0, side, 1, 0xF, 0xE)?) > 0 {
                return Ok(0);
            }
            let mask = unit_mask(w, 0, 0, side, 2, 0xF, 0xE)?;
            let mut seen = 0u8;
            let mut i = 0;
            while i < popcount(mask) {
                let k = nth_unit(mask, i);
                if w.visible(k >> 8, k & 0xFF, false) {
                    seen = seen.wrapping_add(1);
                }
                i += 1;
            }
            Ok(i32::from(seen >= 2))
        }
        id::CARD_264 => Ok(i32::from(
            popcount(unit_mask(w, 0, 0, side, 1, 0x10, 0xE)?) >= 4,
        )),
        _ => Ok(0),
    }
}

/// 0x14CFF0(rec): monsters with an effect entry, by card: 0x23F (while not
/// committed face-up) and 0x23A with a full formation keep the plan
/// 0x27F060(rec, 0) made on the strongest enemy; 0x227 and 0x264 with
/// [`pair_test`] → plan 3 at the own cell; otherwise
/// [`monster`](self::monster).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let card = w.card_id(g, s);
    match card {
        id::CARD_23F => {
            if formation_full(w)? == 1
                && !w.flag(g, s, 4)
                && plans::strongest_enemy(w, rec, 0)? == 1
            {
                return Ok(());
            }
        }
        id::CARD_23A => {
            if formation_full(w)? == 1 && plans::strongest_enemy(w, rec, 0)? == 1 {
                return Ok(());
            }
        }
        id::CARD_227 | id::CARD_264 => {
            if pair_test(w, ctrl, card)? == 1 {
                rec.set_plans(0x3FF);
                set_target(rec, w.pos(g, s));
                return Ok(());
            }
        }
        _ => (),
    }
    monster(w, rec)
}

/// 0x14D140(rec): magic cards, by entry 0's type (0x252060; none →
/// 0x5FF): 2 → with a full formation the target becomes the open-area
/// centre 0x27E0B0(u, c, r, 4, 9, 2) when there is one; the plan is 0x5FF
/// either way (quirk: the 0x6FF write falls through to the 0x5FF one);
/// others → 0x5FF.
fn magic<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let entry = w.unit(g, s).and_then(|u| u.entry(0));
    let pos = w.pos(g, s);
    if let Some([w0, _]) = entry {
        if w0 >> 8 == 2 && formation_full(w)? == 1 {
            if let Some(u) = w.unit(g, s) {
                let v = plans::open_area(
                    w,
                    u,
                    pos & 0xFF,
                    pos >> 8,
                    OPEN_TERRAIN,
                    OPEN_MIN,
                    OPEN_RADIUS,
                )?;
                if v >= 0 {
                    set_target(rec, v);
                }
            }
        }
    }
    rec.set_plans(0x5FF);
    Ok(())
}

/// 0x150190(side, card): Pegasus's 0x139860 with the tribute ATK limit
/// 2500.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, RITUAL_ATK_LIMIT, false)
}

/// 0x14FD60(g, s): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters 0x227 / 0x264: 1 with [`pair_test`] of the
/// controller, else 2; other monsters 2. Rituals: 2 under a type-0xE far
/// trap (0x27F170 mode 6, see-all 0); 1 when the tributes are acceptable
/// ([`ritual_ok`] for the controller) and the card is not on Crush. Magic:
/// 2 under a type-0xE trap; by entry 0's type: 0x12 → 1 with four or more
/// monsters of group `g`; 4 → 1 with a full formation and no magic or
/// trap card of group `g` (0x1E1470); 2 → 1 when the open-area centre
/// 0x27E0B0(u, c, r, 4, 9, 2) is the card's own cell; 0xD → 1; others 2.
/// Others 2.
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
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let yes = match w.kind(g, s) >> 5 {
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let Some(u) = w.unit(g, s) else {
                return Ok(2);
            };
            match u.entry_type(0) {
                0x12 => w.monsters(g) >= 4,
                4 => formation_full(w)? == 1 && w.others(g) == 0,
                2 => plans::open_area(w, u, c, r, OPEN_TERRAIN, OPEN_MIN, OPEN_RADIUS)? == pos,
                0xD => true,
                _ => false,
            }
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            ritual_ok(w, ctrl, card)? == 1 && w.terrain(c, r) != CRUSH
        }
        0 => match card {
            id::CARD_227 | id::CARD_264 => pair_test(w, ctrl, card)? == 1,
            _ => false,
        },
        _ => false,
    };
    Ok(if yes { 1 } else { 2 })
}

#[cfg(test)]
mod tests;
