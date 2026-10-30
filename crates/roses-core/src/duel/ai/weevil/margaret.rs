//! Margaret Mai Beaufort's controller (opponent 13, the white route's
//! Canterbury duel, control byte 13, driver 0x14C000 = Weevil's 0x1117D0
//! by opcode shape, module 0x147230..0x14C1C8, 35 functions). The dispatch
//! 0x147230 is Manawyddan's red 0x163A00 (plan 0xC, plan 9 with see-all 0)
//! storing a mode nibble (0x14B570) in side +0x1A1 first; the actor choice
//! 0x1479D0 is Pegasus's 0x136240 handing that mode to the plan
//! assignment; the executors are York's retreat 0x13C730 (0x149F60, the
//! static radius 3), Labyrinth-ruler's attack 0x1344B0 (0x14A530, route
//! radius 2) and Seto's move 0x116BC0 (0x14AB10); the shared parts run
//! through the framework in [`super`] with the constants and hooks of
//! [`Margaret`].
//!
//! Own functions, by EE address: [`mode`] 0x14B570, [`hidden_value`]
//! 0x14BC00, [`assign`] 0x147790, [`monster`] 0x147DC0 (0x1480A0: 0x27F760
//! with limit 0x960), [`harpie`] 0x148100, [`effect_monster`] 0x148390,
//! [`support`] 0x148460, [`front_cell`] 0x1486D0, [`formation_cell`]
//! 0x148780 with [`in_formation`] 0x148720 and [`free_formation_cell`]
//! 0x148810, [`is_harpie`] 0x14BC60, [`cross_watch`] 0x14BCB0,
//! [`graveyard_pick`] 0x14BD60, the plan-0xC executor [`plan_c`] 0x14B380,
//! [`reveal`] 0x14B730, [`ritual_ok`] 0x14BB10 (Pegasus's 0x139860 with
//! the limit 0x960), and the summon planner in [`summon`] (0x1488F0 and
//! its subtree).

pub(super) mod summon;

use super::exec;
use super::plans::{self, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{nth_unit, popcount, remove_unit, ritual_ok_with, unit_mask};
use super::world::{World, R};
use super::{has_effect, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// Harpie Lady (0x110), Harpie Lady Sisters (0x111), card 0x1E and card
    /// 0xB: the mode's boosted units (0x14B570) and the cards
    /// [`super::harpie`] plans for (0x14BC60).
    pub const HARPIE_LADY: i32 = 0x110;
    pub const HARPIE_SISTERS: i32 = 0x111;
    pub const CARD_1E: i32 = 0x1E;
    pub const CARD_B: i32 = 0xB;
    /// 0x148390 / 0x14B730: planned in place / revealed with three or more
    /// own kind-6 monsters.
    pub const CARD_123: i32 = 0x123;
    /// A ritual (0x148BC0: summoned with acceptable tributes).
    pub const RITUAL_34B: i32 = 0x34B;
    /// A magic card summoned with a [`super::graveyard_pick`].
    pub const CARD_2AD: i32 = 0x2AD;
    /// Elegant Egotist (0x148F00: summoned with Harpie Lady in hand).
    pub const EGOTIST: i32 = 0x31D;
    /// Cursebreaker (0x148BC0: summoned while a slot holds a spellbound or
    /// no unit) and Paralyzing Potion (never listed as a power-up).
    pub const CURSEBREAKER: i32 = 0x31C;
    pub const POTION: i32 = 0x31B;
}

/// 0x1480A0 / 0x148100: the ATK limit of 0x27F760's first pass (0x960).
const PASS_ATK_LIMIT: i32 = 0x960;
/// 0x147DC0: the move limit of the enemy-leader test, the last round and
/// the radius of the passes.
const LEADER_MOVES: i32 = 2;
const PASS_ROUNDS: i32 = 2;
const PASS_RADIUS: i32 = 2;
/// 0x148100: the last round and the radius of its passes.
const HARPIE_ROUNDS: i32 = 5;
const HARPIE_RADIUS: i32 = 5;
/// 0x14BC00's LP test (`slti 0x5DD`) and answers.
const LOW_LP: i32 = 0x5DD;
const HIDDEN_LOW_LP: i32 = 0x7D0;
const HIDDEN_FEW_MONSTERS: i32 = 0x5DC;
const HIDDEN_DEFAULT: i32 = 0x2EE;
/// 0x14BB10: the tribute ATK limit (`slti 0x960` = 2400).
const RITUAL_ATK_LIMIT: i32 = 0x960;
/// 0x27F500's raw-index filter 0x14B484 passes (`addiu $a2, 1`).
const MENU_RAW: i32 = 1;
/// Crush terrain (9).
const CRUSH: i32 = 9;

/// Margaret Mai Beaufort's module.
pub(super) struct Margaret;

impl Opponent for Margaret {
    const CONTROL: u8 = 13;
    /// 0x14AB10 (= Seto's 0x116BC0) and 0x14A530: strength 1.
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    /// 0x1474F0 / 0x147700: t0 = 0.
    const EQUIP_SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x14A8C4 (0x1344B0's 4) and Seto's 0x116BC0: radius 2.
    const DANGER_RADIUS: i32 = 2;
    /// 0x149F60 (York's 0x13C730 with 3) and 0x14A530's immobile path.
    const STATIC_DANGER_RADIUS: i32 = 3;
    /// York's block in 0x149F60 and Seto's in 0x14AB10.
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
    /// 0x147260: +0x1A1 = (+0x1A1 & 0xF0) | (0x14B570() & 0xF).
    fn begin<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mem: &mut Memory) -> R<()> {
        mem.x1a1 = (mem.x1a1 & 0xF0) | (mode(w)? & 0xF);
        Ok(())
    }
    /// 0x147790(rec, +0x1A1 & 0xF).
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()> {
        let m = mem.x1a1 & 0xF;
        assign(w, &mut mem.records[idx], m)
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
    /// 0x14BA10 = Pegasus's 0x139950.
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
    /// 0x147504 / 0x147714: 0xC → 0x14B380, executed whatever it did.
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
        code: u8,
    ) -> R<bool> {
        if code != 0xC {
            return Ok(false);
        }
        plan_c(w, rec, side, d, which)?;
        Ok(true)
    }
}

/// 0x14BC00(side): the strength assumed for hidden units: 2000 while the
/// side's LP is below 1501; else 1500 with fewer than four monsters in its
/// slots 1..=5, else 750.
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    if w.lp(side) < LOW_LP {
        return HIDDEN_LOW_LP;
    }
    if w.monsters(side) < 4 {
        HIDDEN_FEW_MONSTERS
    } else {
        HIDDEN_DEFAULT
    }
}

/// 0x14B570(): 1 when one of the active side's Harpie Lady Sisters, then
/// card 0x1E units (0x24E360 rel 1, type 0x1C, whole board, from bit 21
/// down) has an ATK above its printed one (0x1E1510 with and without
/// +0x38); else 1 when it controls a card 0xB; else 0.
fn mode<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> R<u8> {
    let side = i32::from(w.active);
    for card in [id::HARPIE_SISTERS, id::CARD_1E] {
        let mask = unit_mask(w, 0, 0, side, 1, (card as u32) << 16 | 0x1C, 0xE)?;
        let mut i = 0;
        while i < popcount(mask) {
            let k = nth_unit(mask, i);
            let (g, s) = (k >> 8, k & 0xFF);
            if w.atk(g, s, true)? < w.atk(g, s, false)? {
                return Ok(1);
            }
            i += 1;
        }
    }
    let mask = unit_mask(w, 0, 0, side, 1, (id::CARD_B as u32) << 16 | 0x1C, 0xE)?;
    Ok(u8::from(mask != 0))
}

/// 0x14BC60(g, s): the unit is Harpie Lady Sisters, card 0x1E or card 0xB.
fn is_harpie<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, s: i32) -> bool {
    matches!(
        w.card_id(g, s),
        id::HARPIE_SISTERS | id::CARD_1E | id::CARD_B
    )
}

/// 0x147790(rec, mode): (re)assign the plans of a record. Missing, acted
/// or free records are left alone. The leader (kept while the +8 word is
/// not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by the summon
/// flag; else 0xA05 / 0x5FF. Non-leaders by class: 3 and 1 → [`support`];
/// 2 → 0x27E390(rec, 5, 0, 0x14BC00(controller) & 0xFF); 0 → [`harpie`]
/// for the [`is_harpie`] cards, else [`effect_monster`] with an effect
/// entry (0x2520F0 = 1), else [`monster`](self::monster); 4, 5 and above
/// → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: u8,
) -> R<()> {
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
        3 | 1 => support(w, rec, mode)?,
        2 => plans::power_up(w, rec, 5, 0, hidden_value(w, ctrl) & 0xFF)?,
        0 => {
            if is_harpie(w, g, s) {
                harpie(w, rec)?;
            } else if has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x148780(i): the formation cells (row 4, columns 3, 2, 4, 1, 5), −1
/// past them.
fn formation_cell(i: i32) -> i32 {
    match i {
        0 => 0x403,
        1 => 0x402,
        2 => 0x404,
        3 => 0x401,
        4 => 0x405,
        _ => -1,
    }
}

/// 0x148720(c, r): (c, r) is a formation cell.
fn in_formation(c: i32, r: i32) -> bool {
    (0..5).any(|i| formation_cell(i) == r << 8 | c)
}

/// 0x148810(): the first formation cell holding no unit (0x1E4960), −1.
fn free_formation_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> i32 {
    for i in 0..5 {
        let v = formation_cell(i);
        if w.at(v & 0xFF, v >> 8).is_none() {
            return v;
        }
    }
    -1
}

/// 0x1486D0(side, c, r): from rows 0..2 the corner (col 0 or 6, row 3) on
/// the unit's half (col < 3 → 0x300, else 0x306); from row 3 on the
/// enemy leader's cell (0x1E2310(other, 0)).
fn front_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    c: i32,
    r: i32,
) -> i32 {
    if r < 3 {
        return if c < 3 { 0x300 } else { 0x306 };
    }
    w.pos(i32::from(side == 0), 0)
}

/// 0x147DC0(rec): monsters. 0x27CB40 reaches the enemy leader within 2
/// moves: plan 2 on its key. Else N = 1..=2: 0x1480A0 → 0x27F760 (radius
/// 2, see-all 0, hidden strength 0x14BC00(controller), limit 0x960): 0 →
/// plan 3 at the own cell; 1 → planned; 2 → plan 3 at the own cell; 3 →
/// plan 6 at the own cell; else next N. Exhausted: plan 6 to the own cell
/// when it is a formation cell, else to the first free one, else to the
/// [`front_cell`].
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    if passes(w, rec, PASS_ROUNDS, PASS_RADIUS)? {
        return Ok(());
    }
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let my = w.pos(g, s);
    let (c, r) = (my & 0xFF, my >> 8);
    rec.set_plans(0x6FF);
    if in_formation(c, r) {
        set_target(rec, my);
        return Ok(());
    }
    let v = free_formation_cell(w);
    if v >= 0 {
        set_target(rec, v);
        return Ok(());
    }
    set_target(rec, front_cell(w, ctrl, c, r));
    Ok(())
}

/// 0x148100(rec): Harpie Lady Sisters, card 0x1E and card 0xB: as
/// [`monster`](self::monster) with N = 1..=5 (radius 5, limit 0x960);
/// exhausted → plan 6 to the [`front_cell`].
fn harpie<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    if passes(w, rec, HARPIE_ROUNDS, HARPIE_RADIUS)? {
        return Ok(());
    }
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let my = w.pos(g, s);
    rec.set_plans(0x6FF);
    set_target(rec, front_cell(w, ctrl, my & 0xFF, my >> 8));
    Ok(())
}

/// The common part of 0x147DC0 and 0x148100: the enemy-leader test (2
/// moves) and the 0x27F760 rounds; true when they planned (the caller
/// returns), false when the original falls through to its own tail.
fn passes<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    rounds: i32,
    radius: i32,
) -> R<bool> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(true);
    };
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let lp = w.pos(leader >> 8, leader & 0xFF);
    // sp+0x130 / sp+0x140: the reachable list, once; 0x2800B0: a no-op.
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
        return Ok(true);
    }
    let mut pool = Pool::new();
    for n in 1..=rounds {
        let side = i32::from(w.ctrl(g, s) == 0);
        let v = plans::attack_passes(
            w,
            rec,
            u,
            my & 0xFF,
            my >> 8,
            side,
            n,
            hidden_value(w, ctrl),
            &mut list,
            &mut pool,
            radius,
            0,
            PASS_ATK_LIMIT,
        )?;
        match v {
            0 | 2 => {
                rec.set_plans(0x3FF);
                set_target(rec, my);
                return Ok(true);
            }
            1 => return Ok(true),
            3 => {
                rec.set_plans(0x6FF);
                set_target(rec, my);
                return Ok(true);
            }
            _ => (),
        }
    }
    Ok(false)
}

/// 0x148390(rec): monsters with an effect entry: card 0x123 with three or
/// more own kind-6 monsters (0x24E360 rel 1, type 6, whole board) → plan 3
/// at the own cell; otherwise [`monster`](self::monster).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    if w.card_id(g, s) == id::CARD_123 && popcount(unit_mask(w, 0, 0, ctrl, 1, 6, 0xE)?) >= 3 {
        rec.set_plans(0x3FF);
        set_target(rec, w.pos(g, s));
        return Ok(());
    }
    monster(w, rec)
}

/// The units of 0x24E360(c, r, side, rel, type, 0x12) (mode 0x12: the
/// non-leaders sharing the column or the row of (c, r); filters 0x1D,
/// 0x1E and 0x1F only), as Manawyddan's 0x167B94 uses it.
fn cross_mask<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    c: i32,
    r: i32,
    side: i32,
    rel: i32,
    ty: u32,
) -> R<i32> {
    if !matches!(ty, 0x1D..=0x1F) {
        return Ok(0);
    }
    let all = unit_mask(w, c, r, side, rel, ty, 0xE)?;
    let mut mask = all;
    for i in 0..popcount(all) {
        let k = nth_unit(all, i);
        let p = w.pos(k >> 8, k & 0xFF);
        if p & 0xFF != c && p >> 8 != r {
            mask = remove_unit(mask, k >> 8, k & 0xFF);
        }
    }
    Ok(mask)
}

/// 0x14BCB0(side, c, r): 1 in mode 1 when no non-monster of the side
/// shares the column or row of (c, r) and an enemy non-monster does
/// (0x24E360 mode 0x12, type 0x1E); else 0.
fn cross_watch<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    c: i32,
    r: i32,
) -> R<i32> {
    if mode(w)? == 0 {
        return Ok(0);
    }
    if cross_mask(w, c, r, side, 1, 0x1E)? != 0 {
        return Ok(0);
    }
    Ok(i32::from(cross_mask(w, c, r, side, 2, 0x1E)? != 0))
}

/// 0x148460(rec, mode): magic and trap cards, by entry 0's type (0x252060;
/// none → 0x5FF):
/// - 4 (mode 1 only): over the enemy's non-monsters (0x24E360 rel 2, type
///   0x1E, whole board), the first whose row seen from the own column, or
///   whose column seen from the own row, passes [`cross_watch`] → plan 6
///   to that cell; else 0x5FF;
/// - 2 (mode 1 only): the target becomes the open-area centre
///   0x27E0B0(u, c, r, 2, 6, 2) when there is one; the plan is 0x5FF
///   either way (quirk: the 0x6FF write falls through to the 0x5FF one);
/// - 0x14 → 0xCFF; others → 0x5FF.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let entry = w.unit(g, s).and_then(|u| u.entry(0));
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
    let Some([w0, _]) = entry else {
        rec.set_plans(0x5FF);
        return Ok(());
    };
    match w0 >> 8 {
        4 => {
            if mode == 1 {
                let mask = unit_mask(w, 0, 0, ctrl, 2, 0x1E, 0xE)?;
                let mut i = 0;
                while i < popcount(mask) {
                    let k = nth_unit(mask, i);
                    let kp = w.pos(k >> 8, k & 0xFF);
                    let (kc, kr) = (kp & 0xFF, kp >> 8);
                    if cross_watch(w, ctrl, c, kr)? == 1 {
                        rec.set_plans(0x6FF);
                        set_target(rec, kr << 8 | c);
                        return Ok(());
                    }
                    if cross_watch(w, ctrl, kc, r)? == 1 {
                        rec.set_plans(0x6FF);
                        set_target(rec, r << 8 | kc);
                        return Ok(());
                    }
                    i += 1;
                }
            }
            rec.set_plans(0x5FF);
        }
        2 => {
            if mode == 1 {
                if let Some(u) = w.unit(g, s) {
                    let v = plans::open_area(w, u, c, r, 2, 6, 2)?;
                    if v >= 0 {
                        set_target(rec, v);
                    }
                }
            }
            rec.set_plans(0x5FF);
        }
        0x14 => rec.set_plans(0xCFF),
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x14BB10(side, card): Pegasus's 0x139860 with the tribute ATK limit
/// 2400.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, RITUAL_ATK_LIMIT, false)
}

/// 0x27F660(side, 3, card): copies of `card` in both graveyards (a
/// count > 0 is all the module asks).
fn grave_copies<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> i32 {
    let mut n = 0;
    for gy in [side, i32::from(side == 0)] {
        for i in 0..w.graveyard_count(gy) {
            if w.graveyard(gy, i) == card {
                n += 1;
            }
        }
    }
    n
}

/// 0x14BD60(side): the graveyard card to revive: −1 with five monsters
/// (0x1E13D0) or when neither graveyard holds Harpie Lady Sisters, card
/// 0x1E or card 0xB; else the first card 0xB of the own graveyard, then of
/// the other, then card 0x1E likewise, then Harpie Lady Sisters, as
/// (graveyard side << 16) | index; −1 without one.
fn graveyard_pick<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    graveyard_pick_with(
        w,
        side,
        false,
        [id::CARD_B, id::CARD_1E, id::HARPIE_SISTERS],
    )
}

/// The shape of 0x14BD60 with its card immediates (`cards` in pick order)
/// and its zone gate (`others`: five cards in slots 6..=10, 0x1E1470,
/// instead of five monsters; Yugi's 0x163340).
pub(super) fn graveyard_pick_with<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    others: bool,
    cards: [i32; 3],
) -> i32 {
    let full = if others { w.others(side) } else { w.monsters(side) };
    if full >= 5 {
        return -1;
    }
    if cards.iter().all(|&c| grave_copies(w, side, c) <= 0) {
        return -1;
    }
    for card in cards {
        for gy in [side, i32::from(side == 0)] {
            for i in 0..w.graveyard_count(gy) {
                if w.graveyard(gy, i) == card {
                    return gy << 16 | i;
                }
            }
        }
    }
    -1
}

/// 0x14B380(rec, side, d, which): plan 0xC. With a [`graveyard_pick`]
/// (none when the card stands on Crush or a type-0xE far trap is out,
/// 0x27F170 mode 6, see-all 0): type 0xC, +0x10 from 0x27FCA0(defflag 1,
/// see-all 0) on the stale +4, +0xC = 1, +4 = (graveyard side << 8) | the
/// raw index (0x27F500 filter 1), the slot cleared (0x27CAF0). Else a card
/// committed face-up is marked acted by the module itself (0x1E30F0(g, s,
/// 0, 1)) with an empty type-0xE record, and a set one retreats (0x149F60).
fn plan_c<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let pos = w.pos(g, s);
    let mut t = graveyard_pick(w, side);
    if w.terrain(pos & 0xFF, pos >> 8) == CRUSH {
        t = -1;
    }
    if w.classify(g, s, 6, false)? == 5 {
        t = -1;
    }
    if t >= 0 {
        let gy = (t >> 16) & 0xFFFF;
        let menu = super::necromancer::graveyard_menu_index(w, gy, t & 0xFF, MENU_RAW);
        d.key = rec.key;
        d.kind = 0xC;
        d.position = exec::stance(w, g, s, d.target, 1, false)?;
        d.reveal = 1;
        d.target = ((gy << 8) | menu) as u16;
        rec.consume(which);
        return Ok(());
    }
    if w.flag(g, s, 4) {
        d.mark_acted = Some(rec.key);
        d.key = rec.key;
        d.kind = 0xE;
        return Ok(());
    }
    exec::retreat::<Margaret, _, _>(w, rec, side, d, which)
}

/// 0x14B730(g, s): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters: Harpie Lady Sisters and Harpie Lady → 1
/// with an own card 0x1E (0x24E360 rel 1 to group `g`); card 0x123 → 1
/// with three or more own kind-6 monsters; others 2. Rituals: 2 under a
/// type-0xE far trap (0x27F170 mode 6, see-all 0); 1 when the tributes
/// are acceptable ([`ritual_ok`] for the controller) and the card is not
/// on Crush. Magic: 2 under a type-0xE trap; by entry 0's type: 4 → 1
/// with [`cross_watch`] of group `g` at the card's cell; 2 → 1 when the
/// open-area centre 0x27E0B0(u, c, r, 2, 6, 2) is the card's own cell; 0xD
/// → 1; others 2. Others 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
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
                4 => cross_watch(w, g, c, r)? == 1,
                2 => plans::open_area(w, u, c, r, 2, 6, 2)? == pos,
                0xD => true,
                _ => false,
            }
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            ritual_ok(w, w.ctrl(g, s), card)? == 1 && w.terrain(c, r) != CRUSH
        }
        0 => match card {
            id::HARPIE_SISTERS | id::HARPIE_LADY => {
                popcount(unit_mask(
                    w,
                    0,
                    0,
                    g,
                    1,
                    (id::CARD_1E as u32) << 16 | 0x1C,
                    0xE,
                )?) > 0
            }
            id::CARD_123 => popcount(unit_mask(w, 0, 0, g, 1, 6, 0xE)?) >= 3,
            _ => false,
        },
        _ => false,
    };
    Ok(if yes { 1 } else { 2 })
}

#[cfg(test)]
mod tests;
