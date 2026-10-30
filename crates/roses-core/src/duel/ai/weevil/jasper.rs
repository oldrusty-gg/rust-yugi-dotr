//! Jasper Dice's controller (opponent 17, the white route's duel after
//! Shadi, control byte 17, driver 0x159380 = Weevil's 0x1117D0 by opcode
//! shape, module 0x154AF0..0x159548, 32 functions; 0x154AE0 is a jump stub
//! of control 16). Jasper's leader is card 0x3A and the module builds a
//! formation: two fixed cell lists on the board's columns 0 and 6 that his
//! units walk to with a new plan code 7 (a move along attack routes), and
//! it watches the Exodia pieces 0x36..=0x39.
//!
//! The dispatch 0x154AF0 is Tea's 0x13E570 without plan 0xB and with plan
//! 7 (the move executor with its seventh argument set, see
//! [`exec::go_to_cell_with`]); the actor choice 0x1553A0 is Pegasus's
//! 0x136240 with the "any unit that can step" pass moved before the leader
//! ([`choose_actor`]); the executors are Seto's attack 0x1166F0 without its
//! discarded reveal call (0x157890, [`attack`]), York's retreat 0x13C730
//! (0x157D50, static radius 3) and Seto's move 0x116BC0 (0x158320, route
//! radius 3, plus the plan-7 search); the summon planner 0x156040 is Tea's
//! 0x13F7B0 shape ([`summon`]). The shared parts run through the framework
//! in [`super`] with the constants and hooks of [`Jasper`].
//!
//! Own functions, by EE address: [`dispatch`] 0x154AF0, [`assign`]
//! 0x155070, [`choose_actor`] 0x1553A0, [`monster`] 0x155780,
//! [`effect_monster`] 0x155940, [`support`] 0x155B00, [`on_formation`]
//! 0x155D80, [`formation_cell`] 0x155DF0, [`first_free`] 0x155EC0,
//! [`in_formation`] 0x155F40, [`attack`] 0x157890, [`reveal`] 0x158C30,
//! [`hidden_value`] 0x159050, [`is_piece`] 0x1590B0, [`exodia_ready`]
//! 0x1590D0, [`pieces_around_leader`] 0x159230 and the summon planner in
//! [`summon`] (0x156040 and its subtree, 0x156040..0x157888).

mod summon;

use super::exec;
use super::pegasus::summon::category_of;
use super::search::{self, Steps};
use super::summon::{kind_of, neighbour, nth_unit, popcount, unit_mask, Hand};
use super::world::{unsupported, World, R};
use super::{attacker, stay, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x1590B0: the Exodia pieces 0x36..=0x39 (planned in place by
    /// 0x155940, never fodder, summoned next to a leader on (3, 4)).
    pub const FIRST_PIECE: i32 = 0x36;
    pub const LAST_PIECE: i32 = 0x39;
    /// 0x26A010(i) for i = 0..=3: the pieces 0x159230 looks for, in order.
    pub const PIECES: [i32; 4] = [0x38, 0x39, 0x36, 0x37];
    /// 0x159230: the leader card for which the pieces are counted.
    pub const EXODIA_LEADER: i32 = 0x3A;
    /// 0x155940 / 0x158C30: held (plan 3) or revealed while its
    /// controller's LP is below 1200.
    pub const CARD_5F: i32 = 0x5F;
    /// 0x155940 / 0x158C30: held or revealed with two or more face-up
    /// enemy units of type word 3.
    pub const CARD_40: i32 = 0x40;
    /// 0x155B00: class-4 cards that walk to the formation.
    pub const CARD_334: i32 = 0x334;
    pub const CARD_337: i32 = 0x337;
    /// 0x157410: summoned next to the leader on (3, 4) only (0x157840).
    pub const CARD_335: i32 = 0x335;
    pub const CARD_338: i32 = 0x338;
    /// 0x1590D0: the four cards that must each be on the own field or in
    /// the own graveyard.
    pub const READY: [i32; 4] = [0x32B, 0x325, 0x335, 0x338];
    /// Paralyzing Potion and Cursebreaker (never appended by 0x156560).
    pub const POTION: i32 = 0x31B;
    pub const CURSEBREAKER: i32 = 0x31C;
}

/// The cell (row << 8) | col the leader holds (0x155070) and from which
/// the pieces and cards 0x335 / 0x338 are summoned (0x157410).
const LEADER_CELL: i32 = 0x403;
/// 0x155070: the cell whose own occupant the leader attacks (col 3, row 5).
const GUARD_COL: i32 = 3;
const GUARD_ROW: i32 = 5;
/// 0x155780 / 0x155B00 / 0x157410: the target when both formations are
/// full.
const FALLBACK_CELL: i32 = 0x503;
/// 0x155780: rows below this send a unit off the formation straight to
/// row 6 of its column.
const BACK_ROW: i32 = 6;
/// 0x155DF0 (jump table 0x2BA3F0): the formation cells for mode 0 (column
/// 0 rows 2..=6, then row 6 columns 1 and 2) and mode ≠ 0 (column 6, then
/// row 6 columns 5 and 4), as (row << 8) | col.
const FORMATION: [[i32; 7]; 2] = [
    [0x200, 0x300, 0x400, 0x500, 0x600, 0x601, 0x602],
    [0x206, 0x306, 0x406, 0x506, 0x606, 0x605, 0x604],
];
/// 0x155940 / 0x158C30: the LP limit of card 0x5F (`slti 0x4B0`).
const LOW_LP: i32 = 0x4B0;
/// 0x159050: LP test (`slti 0x4B1`), monster test (`slti 5`) and answers.
const HIDDEN_LP: i32 = 0x4B1;
const HIDDEN_MONSTERS: i32 = 5;
const HIDDEN_LOW_LP: i32 = 0x960;
const HIDDEN_FEW_MONSTERS: i32 = 0x5DC;
const HIDDEN_DEFAULT: i32 = 0x2EE;
/// 0x158C30: the type-0xD magic's sub-type (`andi 0x3F` = 0x32) and enemy
/// unit limit (0x2500A0(other, 3) `slti 3`), the type-0x22 magic's enemy
/// count (`slti 4`), and the type-0x17 magic's row limit (`slti 2`).
const MAGIC_D_SUBTYPE: u32 = 0x32;
const MAGIC_D_ENEMIES: i32 = 3;
const MAGIC_22_ENEMIES: i32 = 4;
const MAGIC_17_ROW: i32 = 2;
/// 0x157890 (Seto's 0x1166F0): the danger radii of the immobile path and of
/// the scored moves, and the route limit.
const ATTACK_STATIC_RADIUS: i32 = 3;
const ATTACK_MOVE_RADIUS: i32 = 2;
const ATTACK_ROUTE_LIMIT: i32 = 30;

/// Jasper Dice's module.
pub(super) struct Jasper;

impl Opponent for Jasper {
    const CONTROL: u8 = 17;
    /// 0x158320 (`addiu $t0, 1`) and 0x157D50.
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x1586F0 (`addiu $a3, 3`).
    const DANGER_RADIUS: i32 = 3;
    /// 0x157D50 (York's 0x13C730 with 3).
    const STATIC_DANGER_RADIUS: i32 = 3;
    /// York's block in 0x157D50 and Seto's in 0x158320.
    const FACE_UP_SUPPORT_PASSES: bool = true;
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
    /// 0x155484: class 3 only.
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
        // Not reached for Jasper (his own dispatch calls the planner).
        summon::summon(w, key, side, d, false)
    }
    fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        rec: &Record,
    ) -> R<u32> {
        reveal(w, g, s, rec)
    }
    fn attack<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
    ) -> R<i32> {
        attack(w, rec, side, d, which)
    }
    /// 0x157890 has no charge branch.
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
    /// The dispatch knows no codes 0xB..=0xD.
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

/// 0x158320 for plan slot `which`: plans 3 (`defence`), 6 and 7
/// (`attack_route`).
fn go<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    defence: bool,
    attack_route: bool,
) -> R<()> {
    exec::go_to_cell_with::<Jasper, _, _>(w, rec, side, d, which, defence, attack_route)
}

/// 0x154AF0(mem, side struct, d): the dispatch (0x10D210's shape). The
/// actor ([`choose_actor`]); a leader summon it already planned consumes
/// the slot holding plan 0xA. Otherwise [`assign`], then the primary plan:
/// 0xE stays (0x278800), 9 → 0x27E960 (see-all 0, hidden strength
/// 0x159050), 3 / 7 / 6 → 0x158320 (plan 3 asks defence, plan 7 searches
/// attack routes), 0xA → the summon planner (the slot consumed; a failed
/// summon clears +2 / +0xC and falls through), 5 → 0x157D50, 2 → 0x157890
/// (a gone target consumes the slot and returns 1); 2, 3, 5, 6 and 7 only
/// for a unit that has not acted. Then the secondary plan the same way,
/// where an acted unit, 0xE (after acting in place) and unknown codes
/// return 0 and plan 2 retreats when its target is gone.
fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    d: &mut Decision,
) -> R<i32> {
    let side = i32::from(w.active);
    let k = choose_actor(w, mem, side, d)?;
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
    let done = match rec.plan_a {
        0xE => {
            stay(w, rec, d);
            true
        }
        9 => {
            exec::equip_move::<Jasper, _, _>(w, rec, side, d, 1)?;
            true
        }
        3 if !acted => {
            go(w, rec, side, d, 1, true, false)?;
            true
        }
        7 if !acted => {
            go(w, rec, side, d, 1, false, true)?;
            true
        }
        6 if !acted => {
            go(w, rec, side, d, 1, false, false)?;
            true
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d, false)?;
            rec.consume(1);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
                false
            } else {
                true
            }
        }
        5 if !acted => {
            exec::retreat::<Jasper, _, _>(w, rec, side, d, 1)?;
            true
        }
        2 if !acted => {
            if attack(w, rec, side, d, 1)? < 0 {
                rec.consume(1);
                return Ok(1);
            }
            true
        }
        _ => false,
    };
    if done {
        return Ok(1);
    }
    match rec.plan_b {
        // 0x155010: acts in place, then returns 0.
        0xE => {
            stay(w, rec, d);
            Ok(0)
        }
        9 => {
            exec::equip_move::<Jasper, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        2 | 3 | 5 | 6 | 7 if acted => Ok(0),
        3 => {
            go(w, rec, side, d, 0, true, false)?;
            Ok(1)
        }
        7 => {
            go(w, rec, side, d, 0, false, true)?;
            Ok(1)
        }
        6 => {
            go(w, rec, side, d, 0, false, false)?;
            Ok(1)
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d, false)?;
            rec.consume(0);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
            }
            Ok(1)
        }
        5 => {
            exec::retreat::<Jasper, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        2 => {
            if attack(w, rec, side, d, 0)? < 0 {
                exec::retreat::<Jasper, _, _>(w, rec, side, d, 0)?;
            }
            Ok(1)
        }
        _ => Ok(0),
    }
}

/// 0x1553A0(mem, side, d): the unit to act next, or −1; bit 16 marks a
/// leader summon (the planner has then filled `d`). Pegasus's 0x136240
/// with the third pass moved before the leader: 1. a not-acted unit with a
/// reachable plan-2 target (0x27CCA0 over 0x27D9B0(side, 0x1D, 1));
/// 2. a class-3 card that can step (0x27D2F0) and may act (0x27E010);
/// 3. any unit, the leader included (filter 0x1F), that can step and may
/// act; 4. the leader after [`assign`]: plan 0xA → summon (when allowed)
/// or step; otherwise, once acted, summon, else step; 5. any not-acted
/// unit that may act.
fn choose_actor<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    side: i32,
    d: &mut Decision,
) -> R<i32> {
    let list = w.list(side, 0x1D, true);
    let r = attacker(w, mem, &list)?;
    if r >= 0 {
        return Ok(r);
    }
    for k in w.list(side, 0x1E, true) {
        let class = w.kind(k >> 8, k & 0xFF) >> 5;
        if Jasper::trap_actor(class) && w.can_step(k)? && w.may_act(k) {
            return Ok(k);
        }
    }
    // 0x155520: before the leader.
    for k in w.list(side, 0x1F, true) {
        if w.can_step(k)? && w.may_act(k) {
            return Ok(k);
        }
    }
    let lk = side << 8;
    let Some(idx) = mem.find(lk as u16) else {
        return Err(crate::duel::DuelError::InvalidCommand(
            "no record for the leader",
        ));
    };
    assign(w, &mut mem.records[idx])?;
    if mem.records[idx].plan_a == 0xA {
        if w.can_summon(side) && summon::summon(w, lk, side, d, true)? >= 0 {
            return Ok(lk | 0x10000);
        }
        if w.can_step(lk)? {
            return Ok(lk);
        }
    } else if w.flag(side, 0, 0) {
        if w.can_summon(side) && summon::summon(w, lk, side, d, true)? >= 0 {
            return Ok(lk | 0x10000);
        }
    } else if w.can_step(lk)? {
        return Ok(lk);
    }
    for k in w.list(side, 0x1F, true) {
        if !w.flag(k >> 8, k & 0xFF, 0) && w.may_act(k) {
            return Ok(k);
        }
    }
    Ok(-1)
}

/// 0x155070(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone. The leader (kept while the +8
/// word is not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by
/// the summon flag; else, standing on (3, 4) with another unit of its
/// controller on (3, 5): plan 2 on that unit (0x20A / 0x2FF, the
/// secondary target kept); else 0xA06 with the secondary target (3, 4), or
/// 0x6FF with the primary target (3, 4) without the summon flag.
/// Non-leaders by class: 1..=4 → [`support`]; 0 → [`effect_monster`] with
/// an effect entry (0x2520F0 = 1) or for an Exodia piece, else
/// [`monster`]; 5 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    // 0x1E2310 (dropped) and 0x251C80: the controller.
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
            return Ok(());
        }
        let k = w.at_key(GUARD_COL, GUARD_ROW);
        if k >= 0
            && w.pos(g, s) == LEADER_CELL
            && w.ctrl(k >> 8, k & 0xFF) == ctrl
            && k != i32::from(rec.key)
        {
            rec.set_plans(if summon { 0x20A } else { 0x2FF });
            rec.target_a = k as u16;
            return Ok(());
        }
        if summon {
            rec.set_plans(0xA06);
            rec.target_b = LEADER_CELL as u16;
        } else {
            rec.set_plans(0x6FF);
            rec.target_a = LEADER_CELL as u16;
        }
        return Ok(());
    }
    let card = w.card_id(g, s);
    match w.kind(g, s) >> 5 {
        1..=4 => support(w, rec)?,
        0 => {
            if super::has_effect(w, g, s) == 1 || is_piece(card) {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x155DF0(i, mode): formation cell `i` (0..=6) of mode 0 or ≠ 0
/// ([`FORMATION`]); −1 for other `i`. (0x1DF7C0 is called and dropped.)
fn formation_cell(i: usize, mode: i32) -> i32 {
    FORMATION[usize::from(mode != 0)].get(i).copied().unwrap_or(-1)
}

/// 0x155D80(col, row, mode): the cell is one of the formation's.
fn on_formation(col: i32, row: i32, mode: i32) -> bool {
    (0..7).any(|i| formation_cell(i, mode) == (row << 8 | col))
}

/// 0x155EC0(mode): the first formation cell holding no unit (0x1E4960).
fn first_free<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mode: i32) -> Option<i32> {
    (0..7)
        .map(|i| formation_cell(i, mode))
        .find(|&c| w.at(c & 0xFF, c >> 8).is_none())
}

/// 0x155F40(g, s): the unit stands in a formation: the formation of its
/// class (mode = class) cell by cell, and only when that one is full
/// (quirk: the first empty cell answers 0 at once) the other (mode 1 for
/// monsters, 0 for the others) the same way.
fn in_formation<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, s: i32) -> bool {
    let class = w.kind(g, s) >> 5;
    let other = i32::from(class == 0);
    let key = g << 8 | s;
    for mode in [class, other] {
        for i in 0..7 {
            let c = formation_cell(i, mode);
            let k = w.at_key(c & 0xFF, c >> 8);
            if k < 0 {
                return false;
            }
            if k == key {
                return true;
            }
        }
    }
    false
}

/// The formation walk shared by 0x155780 (monsters, mode 0) and 0x155B00
/// (classes 1..=3, mode = class): plan 7. In a formation
/// ([`in_formation`]) → the own cell. Off both formations on a row below 6
/// → row 6 of the own column. Else the first free cell of the formation of
/// `mode`, else of mode 0, else plan 0x6FF to (3, 5). Only the primary
/// target is written.
fn formation_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: i32,
) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let my = w.pos(g, s);
    rec.set_plans(0x7FF);
    if in_formation(w, g, s) {
        rec.target_a = my as u16;
        return;
    }
    let (c, r) = (my & 0xFF, my >> 8);
    if !on_formation(c, r, mode) && !on_formation(c, r, 0) && r < BACK_ROW {
        rec.target_a = (BACK_ROW << 8 | c) as u16;
        return;
    }
    walk_to_free(w, rec, mode);
}

/// The tail of 0x155780 / 0x155B00: the first free cell of the formation
/// of `mode`, else of mode 0 (primary target), else plan 0x6FF to (3, 5).
fn walk_to_free<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: i32,
) {
    match first_free(w, mode).or_else(|| first_free(w, 0)) {
        Some(c) => rec.target_a = c as u16,
        None => {
            rec.set_plans(0x6FF);
            rec.target_a = FALLBACK_CELL as u16;
        }
    }
}

/// 0x155780(rec): monsters without an effect entry (and the fallback of
/// [`effect_monster`]): [`formation_plan`] with mode = class (0). The
/// controller, the enemy leader's position and 0x1E07C0 are read and
/// dropped.
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return Ok(());
    }
    let class = (w.kind(g, s) >> 5) & 0xFF;
    formation_plan(w, rec, class);
    Ok(())
}

/// 0x155940(rec): monsters with an effect entry and the Exodia pieces, by
/// card (0x1E22B0): 0x5F → [`monster`], then plan 0x3FF (its target kept)
/// while its controller's LP is below 1200; 0x40 → the same with two or
/// more visible (0x279AF0, see-all 0) units of relation 2 and type word 3
/// (0x24E360); the pieces 0x36..=0x39 → 0x6FF on the own cell; others
/// [`monster`].
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let hold = match w.card_id(g, s) {
        id::CARD_5F => w.lp(ctrl) < LOW_LP,
        id::CARD_40 => visible_count(w, unit_mask(w, 0, 0, ctrl, 2, 3, 0xE)?) >= 2,
        c if is_piece(c) => {
            rec.set_plans(0x6FF);
            rec.target_a = w.pos(g, s) as u16;
            return Ok(());
        }
        _ => false,
    };
    monster(w, rec)?;
    if hold {
        rec.set_plans(0x3FF);
    }
    Ok(())
}

/// The units of `mask` that are visible (0x279AF0 with see-all 0), counted
/// in a byte.
fn visible_count<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mask: i32) -> u8 {
    let mut n = 0u8;
    let mut i = 0;
    while i < popcount(mask) {
        let k = nth_unit(mask, i);
        if w.visible(k >> 8, k & 0xFF, false) {
            n = n.wrapping_add(1);
        }
        i += 1;
    }
    n
}

/// 0x155B00(rec): classes 1..=4. Class 4: cards 0x334 and 0x337 → plan
/// 0x6FF to the first free cell of the formation of mode 4, else of mode 0,
/// else (3, 5); other class-4 cards → 0x3FF on the own cell. Classes 1, 2
/// and 3 → [`formation_plan`] with mode = class. Others 0x5FF.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let class = (w.kind(g, s) >> 5) & 0xFF;
    let my = w.pos(g, s);
    match class {
        4 => {
            if matches!(w.card_id(g, s), id::CARD_334 | id::CARD_337) {
                rec.set_plans(0x6FF);
                walk_to_free(w, rec, class);
            } else {
                rec.set_plans(0x3FF);
                rec.target_a = my as u16;
            }
        }
        1..=3 => formation_plan(w, rec, class),
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x1590B0(card): an Exodia piece (0x36..=0x39).
fn is_piece(card: i32) -> bool {
    (id::FIRST_PIECE..=id::LAST_PIECE).contains(&card)
}

/// 0x159050(side): the strength assumed for hidden units: 2400 while the
/// side's LP is below 1201; else 1500 with fewer than five monsters in its
/// slots 1..=5, else 750.
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    if w.lp(side) < HIDDEN_LP {
        HIDDEN_LOW_LP
    } else if w.monsters(side) < HIDDEN_MONSTERS {
        HIDDEN_FEW_MONSTERS
    } else {
        HIDDEN_DEFAULT
    }
}

/// 0x27F660(side, 1, card): copies of `card` in the side's own graveyard
/// (a byte count; only its sign is asked).
fn own_grave_copies<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> u8 {
    let mut n = 0u8;
    for i in 0..w.graveyard_count(side) {
        if w.graveyard(side, i) == card {
            n = n.wrapping_add(1);
        }
    }
    n
}

/// 0x1590D0(side): each of the cards 0x32B, 0x325, 0x335, 0x338 is a unit
/// of relation 1 to the side (0x24E360 type word card << 16 | 0x1C) or in
/// its own graveyard.
fn exodia_ready<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<bool> {
    for card in id::READY {
        if unit_mask(w, 0, 0, side, 1, (card as u32) << 16 | 0x1C, 0xE)? != 0 {
            continue;
        }
        if own_grave_copies(w, side, card) == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 0x159230(side): with the side's leader being card 0x3A, the sum over
/// the four pieces of [`id::PIECES`] of a word set to 1 when a unit of
/// group `side` on a cell next to the leader (0x24FD30) holds the piece and
/// is neither spellbound (0x1E2800) nor marked +0x4A (0x1E28C0); 0 for
/// other leaders. The words of pieces not found are never written: they
/// are 0x159230's stack (sp+0x90..), where the caller 0x158C30's last call
/// 0x1E3380 saved its return address 0x158CB0 as a quad (`sq $ra,
/// 16($sp)`): 0x158CB0, 0, and the upper half of $ra (0 [H]). So the sum
/// is 4 exactly when all four pieces are found, which is all the caller
/// asks; the port answers 4 or not-4 (−1). The generic module's 0x16D8C0
/// is the same function; its caller 0x16BEE0 calls 0x16D800 just before,
/// whose `sq $ra, 16($sp)` leaves 0x16BFA4, 0, 0, 0 there (the same
/// answer).
pub(super) fn pieces_around_leader<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<i32> {
    if w.card_id(side, 0) != id::EXODIA_LEADER {
        return Ok(0);
    }
    for piece in id::PIECES {
        let mut found = false;
        for i in 0..8 {
            let cell = neighbour(w, side, i)?;
            if cell < 0 {
                continue;
            }
            let (col, row) = (cell & 0xFF, cell >> 8);
            let k = w.at_key(col, row);
            if k < 0 {
                continue;
            }
            let (kg, ks) = (k >> 8, k & 0xFF);
            if w.card_id(kg, ks) == piece
                && w.spellbind(kg, ks) == 0
                && w.x4a(kg, ks) == 0
                && kg == side
            {
                found = true;
            }
        }
        if !found {
            return Ok(-1);
        }
    }
    Ok(4)
}

/// 0x158C30(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. An Exodia piece: 1 when [`pieces_around_leader`] of
/// its controller is 4, else by class like any card. Monsters: 0x40 → 1
/// with two or more visible units of relation 2 and type word 3; 0x5F → 1
/// while group `g`'s LP is below 1200; others 2. Magic: 2 under a
/// type-0xE far trap (0x27F170 mode 6, see-all 0); by entry 0's type
/// (0x1E1DF0, both words −1 without an entry): 0xD → sub-type 0x32: 1 with
/// three or more units in the enemy group's slots 1..=10 (0x2500A0), else
/// 2; other sub-types 1; 0x26 → 1 with a trap (0x1DF290 category 2) in the
/// predicted hand, else 1 when [`exodia_ready`] and the piece list
/// [`summon::exodia_list`] finds a piece, else 2; 0x17 → 1 when a unit of
/// relation 2 and type 0x34 stands on row 0 or 1; 0x22 → 1 with four or
/// more units of relation 2 and type 0x1D; others 2. Other classes 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    let entry0 = w
        .unit(g, s)
        .map(|u| u.entry(0).unwrap_or([u32::MAX; 2]));
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    if is_piece(card) && pieces_around_leader(w, ctrl)? == 4 {
        return Ok(1);
    }
    let yes = |b: bool| if b { 1 } else { 2 };
    match w.kind(g, s) >> 5 {
        0 => Ok(match card {
            id::CARD_40 => yes(visible_count(w, unit_mask(w, 0, 0, ctrl, 2, 3, 0xE)?) >= 2),
            id::CARD_5F => yes(w.lp(g) < LOW_LP),
            _ => 2,
        }),
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let Some([w0, _]) = entry0 else {
                return Ok(2);
            };
            Ok(match w0 >> 8 {
                0xD => {
                    if w0 & 0x3F == MAGIC_D_SUBTYPE {
                        let other = i32::from(ctrl == 0);
                        let n = (1..11).filter(|&sl| w.unit(other, sl).is_some()).count() as i32;
                        yes(n >= MAGIC_D_ENEMIES)
                    } else {
                        1
                    }
                }
                0x26 => {
                    if g != ctrl {
                        return unsupported(
                            "0x158C30 reads group g's predicted hand with its controller's count",
                        );
                    }
                    let hand = Hand::of(w, g);
                    for i in 0..w.next_hand_size(ctrl) {
                        if category_of(kind_of(w, hand.get(i)?) >> 5) == 2 {
                            return Ok(1);
                        }
                    }
                    let mut local = Decision::new();
                    let mut u = None;
                    yes(exodia_ready(w, ctrl)?
                        && summon::exodia_list(w, ctrl, &mut local, &mut u, summon::Site::Reveal)?
                            >= 0)
                }
                0x17 => {
                    let mask = unit_mask(w, 0, 0, ctrl, 2, 0x34, 0xE)?;
                    let mut i = 0;
                    let mut hit = false;
                    while i < popcount(mask) {
                        let k = nth_unit(mask, i);
                        if w.pos(k >> 8, k & 0xFF) >> 8 < MAGIC_17_ROW {
                            hit = true;
                            break;
                        }
                        i += 1;
                    }
                    yes(hit)
                }
                0x22 => yes(popcount(unit_mask(w, 0, 0, ctrl, 2, 0x1D, 0xE)?) >= MAGIC_22_ENEMIES),
                _ => 2,
            })
        }
        _ => Ok(2),
    }
}

/// 0x157890(rec, side, d, which): plan 2, Seto's 0x1166F0 without its
/// (discarded) reveal call, with Jasper's hidden strength 0x159050. A gone
/// target consumes the PRIMARY slot and returns −1. An immobile unit stays,
/// asks defence when its cell's danger (radius 3, strength 1, see-all =
/// hidden & 0xFF) is at least its ATK (0x1E1510), leaves +0xC alone and
/// consumes the slot. Otherwise the first moves of the attack routes
/// (0x27B9D0, see-all 0, at most 30, hidden strength & 0xFFFF): lowest
/// danger (radius 2) wins, ties to the one closer to the target; +0xC =
/// +0x10 = 2; without a candidate it stays, consumes the slot and returns
/// −1.
fn attack<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<i32> {
    let target = rec.target(which);
    let (tg, ts) = (i32::from(target >> 8), i32::from(target & 0xFF));
    if w.unit(tg, ts).is_none() {
        rec.consume(1);
        return Ok(-1);
    }
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
    let tp = w.pos(tg, ts);
    let (tc, tr) = (tp & 0xFF, tp >> 8);
    let Some(u) = w.unit(g, s) else {
        return unsupported("0x157890 without its acting unit");
    };
    let see_all = hidden_value(w, side) & 0xFF != 0;
    if w.allowance(u, c, r)? == 0 {
        d.key = rec.key;
        d.kind = 6;
        d.target = pos as u16;
        let danger = w.danger(c, r, side, ATTACK_STATIC_RADIUS, 1, see_all)?;
        d.position = u8::from(danger >= w.atk(g, s, false)?);
        rec.consume(which);
        return Ok(0);
    }
    let mut steps = Steps::new();
    search::attack_path_k(
        w,
        g,
        s,
        &mut steps,
        c,
        r,
        tc,
        tr,
        u32::from(target),
        0,
        ATTACK_ROUTE_LIMIT,
        hidden_value(w, side) & 0xFFFF,
    )?;
    let count = search::step_count(&steps);
    let dist = |b: u8| (i32::from(b & 0xF) - tc).abs() + (i32::from(b >> 4) - tr).abs();
    let (mut best, mut pick) = (0xFFFF, None);
    for i in 0..count.max(0) as usize {
        let b = steps.0[i];
        let score = w.danger(
            i32::from(b & 0xF),
            i32::from(b >> 4),
            side,
            ATTACK_MOVE_RADIUS,
            1,
            see_all,
        )?;
        if score < best {
            best = score;
            pick = Some(i);
        } else if score == best {
            let Some(j) = pick else {
                return unsupported("0x157890 tie reads past its step buffer");
            };
            if dist(b) < dist(steps.0[j]) {
                pick = Some(i);
            }
        }
    }
    d.key = rec.key;
    d.kind = 6;
    d.reveal = 2;
    d.position = 2;
    if let Some(i) = pick {
        let b = steps.0[i];
        d.target = u16::from(b >> 4) << 8 | u16::from(b & 0xF);
        Ok(0)
    } else {
        d.target = pos as u16;
        rec.consume(which);
        Ok(-1)
    }
}

#[cfg(test)]
mod tests;
