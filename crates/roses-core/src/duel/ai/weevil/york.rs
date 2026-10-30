//! Richard Slysheen of York's controller (opponent 10, control byte 10,
//! driver 0x13E3A0 = Weevil's 0x1117D0 by opcode shape, module
//! 0x13A160..0x13E568, 28 functions). The module is Weevil's template with
//! Pegasus's actor choice, summon subtree and executors, Weevil's
//! constants (see-all 0 everywhere, hidden strength 0), its own mode
//! (a lane cell in the front row) and eight novel functions; the shared
//! parts (dispatch 0x13A160, actor choice 0x13A8E0, executors 0x13D2E0 /
//! 0x13CD00 / 0x13C730, the 0x27xxxx helpers) run through the framework in
//! [`super`] with the constants and hooks of [`York`].
//!
//! Own functions, by EE address: [`mode`] 0x13DB50 (stored in +0x1A1 by the
//! dispatch before the actor choice, [`Opponent::begin`]), [`assign`]
//! 0x13A660, [`monster`] 0x13ACD0 (0x13AF80: 0x27F760 with limit 0x9C4),
//! [`after_passes`] 0x13AFE0, [`lane_cell`] 0x13B140 with [`front_row`]
//! 0x13B270, [`support`] 0x13B310 (magic and trap cards),
//! [`strongest_threat`] 0x13DB90, [`reveal`] 0x13DCA0, [`ritual_ok`]
//! 0x13E250 (Pegasus's 0x139860 with the limit 0xBB8) and the summon
//! planner in [`summon`] (0x13B440 and its subtree). The charge test
//! 0x13E150 is Pegasus's 0x139950 by opcode shape and shared with it; the
//! hidden strength function 0x13E340 answers 0 on every path (as Weevil's
//! 0x10F670, with the thresholds 0x3E9 / 5), so the default
//! [`Opponent::hidden_value`] stands.
//!
//! Two template differences come in through hooks: the dispatch runs a
//! primary plan 6 for a unit that has acted
//! ([`Opponent::PRIMARY_MOVE_IGNORES_ACTED`]), and the executors 0x13D2E0 /
//! 0x13C730 mark a committed face-up magic or ritual card acted themselves
//! instead of moving it ([`Opponent::FACE_UP_SUPPORT_PASSES`], which comes
//! back as [`super::Plan::MarkActed`]).

mod summon;

use super::keith::pick_strongest;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Targets};
use super::summon::ritual_ok_with;
use super::world::{World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// M-Warrior #1 and #2 (0x13DCA0: each reveals with two copies of the
    /// other out).
    pub const M_WARRIOR_1: i32 = 0xAA;
    pub const M_WARRIOR_2: i32 = 0xAB;
    /// Swamp Battleguard and Lava Battleguard (the same pairing).
    pub const SWAMP_BATTLEGUARD: i32 = 0x99;
    pub const LAVA_BATTLEGUARD: i32 = 0xC5;
    /// Revival of Sennen Genjin (0x13B6D0).
    pub const REVIVAL_OF_SENNEN_GENJIN: i32 = 0x348;
    /// Raigeki (0x13B6D0).
    pub const RAIGEKI: i32 = 0x2BC;
    /// Cursebreaker (0x13B6D0).
    pub const CURSEBREAKER: i32 = 0x31C;
}

/// 0x13AF80: the ATK limit of 0x27F760's first pass (0x9C4 = 2500).
const TRAP_PASS_ATK_LIMIT: i32 = 0x9C4;
/// 0x13ACD0: the move limit of the enemy-leader test (0x27CB40).
const LEADER_RUSH_MOVES: i32 = 2;
/// 0x13E250: the tribute ATK limit (`slti 0xBB8` = 3000).
const RITUAL_ATK_LIMIT: i32 = 0xBB8;
/// 0x13B140: the columns of the front row tried in this order.
const LANE_COLUMNS: [i32; 5] = [3, 4, 2, 5, 1];
/// 0x13B270: the front row is never below 2.
const FRONT_ROW_MIN: i32 = 2;
/// 0x13C300: the route limit of the summon cell's searches.
const SUMMON_ROUTE_LIMIT: i32 = 2;
/// The Labyrinth terrain (8) and Crush (9).
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;

/// Richard Slysheen of York's module.
pub(super) struct York;

impl Opponent for York {
    const CONTROL: u8 = 10;
    /// 0x13D2E0 / 0x13CD00: strength 0 for hidden units (`move t0, zero`).
    const DANGER_HIDDEN: i32 = 0;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    const DANGER_RADIUS: i32 = 2;
    /// 0x13C7E4 / 0x13CAE4 / 0x13CECC: radius 2 (3 in Pegasus's copies).
    const STATIC_DANGER_RADIUS: i32 = 2;
    /// 0x13A388: plan 6 in the primary slot runs without the acted test.
    const PRIMARY_MOVE_IGNORES_ACTED: bool = true;
    /// 0x13D400 / 0x13C910.
    const FACE_UP_SUPPORT_PASSES: bool = true;
    /// 0x13A9BC (as Pegasus's 0x13633C): class 3 only.
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    /// 0x13A190: the mode 0x13DB50 goes into the low nibble of side +0x1A1
    /// before the actor choice; the actor choice and the dispatch hand it
    /// to 0x13A660 as its second argument.
    fn begin<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mem: &mut Memory) -> R<()> {
        mem.x1a1 = (mem.x1a1 & 0xF0) | mode(w);
        Ok(())
    }
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()> {
        let mode = i32::from(mem.x1a1 & 0xF);
        assign(w, &mut mem.records[idx], mode)
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
    /// 0x13E150 = Pegasus's 0x139950.
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

/// 0x13DB50(): the mode nibble: 1 when the active side has no
/// [`lane_cell`], else 0.
fn mode<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> u8 {
    u8::from(lane_cell(w, i32::from(w.active)) < 0)
}

/// 0x13B270(side): the front row: the highest row among the units in
/// slots 1..=5 of GROUP `side` (0x1E07C0 / 0x1E2310, a byte), at least 2.
fn front_row<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    let mut row = 0;
    for slot in 1..6 {
        if w.unit(side, slot).is_none() {
            continue;
        }
        let r = (w.pos(side, slot) >> 8) & 0xFF;
        if row < r {
            row = r;
        }
    }
    row.max(FRONT_ROW_MIN)
}

/// 0x13B140(side): the first empty cell (0x1E4960) of the [`front_row`] in
/// the columns 3, 4, 2, 5, 1, as (row << 8) | col; −1 when all five hold a
/// unit.
fn lane_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    let row = front_row(w, side);
    for col in LANE_COLUMNS {
        if w.at(col, row).is_none() {
            return row << 8 | col;
        }
    }
    -1
}

/// 0x13DB90(side): the strongest visible enemy non-leader (0x27CF90 over
/// 0x27DC00(other, 0x1D, 0, 0)) when its attack margin (0x279B30) at its
/// own cell over the side's strongest unit (0x27CF90 over 0x27D9B0(side,
/// 0x1D, 1)) is positive; −1 without either pick or with no margin.
fn strongest_threat<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<i32> {
    let other = i32::from(side == 0);
    let enemy = pick_strongest(w, &w.list_seen(other, 0x1D, false, false))?;
    let own = pick_strongest(w, &w.list(side, 0x1D, true))?;
    if enemy < 0 || own < 0 {
        return Ok(-1);
    }
    let p = w.pos(enemy >> 8, enemy & 0xFF);
    Ok(if w.margin_k(enemy, own, p & 0xFF, p >> 8)? > 0 {
        enemy
    } else {
        -1
    })
}

/// (row + 1, col) of a packed position, as the module builds it.
fn ahead(pos: i32) -> i32 {
    ((pos >> 8) + 1) << 8 | (pos & 0xFF)
}

/// 0x13A660(rec, mode): (re)assign the plans of a record. Missing, acted
/// or free records are left alone. The leader (kept while the +8 word is
/// not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by the summon
/// flag; else 0xA06 with the walking target in the secondary slot, or 0x6FF
/// with it in the primary slot: the cell ahead (row + 1, own column) in
/// mode 1, the own cell otherwise. Non-leaders by class: 3 and 1 →
/// [`support`]; 2 → 0x27E390(rec, 2, 0, 0x13E340() & 0xFF = 0); 0 →
/// [`monster`](self::monster)(rec, 1, mode); 4, 5 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let pos = w.pos(g, s);
    // 0x251C80(g, s) is called and its result dropped.
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
        let target = if mode & 0xFF == 1 { ahead(pos) } else { pos } as u16;
        if summon {
            rec.set_plans(0xA06);
            rec.target_b = target;
        } else {
            rec.set_plans(0x6FF);
            rec.target_a = target;
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        3 | 1 => support(w, rec)?,
        2 => plans::power_up(w, rec, 2, 0, 0)?,
        0 => monster(w, rec, 1, mode)?,
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x13ACD0(rec, a1, mode): monsters. `a1` ≠ 0 and 0x27CB40 reaches the
/// enemy leader within 2 moves: plan 2 on its key. Else N = 1..=3 in mode
/// 0 (N = 1 only in mode 1): 0x13AF80 → 0x27F760 (radius 3 in mode 0, 1
/// in mode 1; see-all 0; hidden strength 0x13E340 = 0; limit 0x9C4): 1 →
/// planned; 0 and 2 → plan 3 at the own cell; 3 → plan 6 at the own cell;
/// else next N (the reachable list and the node pool are shared by all
/// rounds, as in 0x10DD60). Exhausted → [`after_passes`].
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
    mode: i32,
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
    // sp+0x150: the reachable list (key 0xFFFF, level 0xFF), once.
    let mut list = Targets::new();
    // 0x2800B0(sp+0xE0): a no-op.
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
            LEADER_RUSH_MOVES,
        )?;
        if v > 0 {
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // sp+0x1AC / sp+0x1AE: the pool counters, zeroed once.
    let mut pool = Pool::new();
    let rounds = if mode == 0 { 3 } else { 1 };
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
            0,
            &mut list,
            &mut pool,
            rounds,
            0,
            TRAP_PASS_ATK_LIMIT,
        )?;
        match v {
            0 | 2 => {
                set_plan(rec, 3);
                set_target(rec, my);
                return Ok(());
            }
            1 => return Ok(()),
            3 => {
                set_plan(rec, 6);
                set_target(rec, my);
                return Ok(());
            }
            _ => (),
        }
    }
    after_passes(w, rec, mode);
    Ok(())
}

/// 0x13AFE0(rec, mode): a monster no pass planned for. Mode 1: plan 2 on
/// the unit in the cell ahead (0x1E4A10(col, row + 1)), or plan 6 to that
/// cell when it is empty. Mode 0: plan 6 to the own cell when the unit
/// stands in its controller's [`front_row`]; else plan 6 to the
/// controller's [`lane_cell`], or to the cell ahead when there is none.
fn after_passes<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: i32,
) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let pos = w.pos(g, s);
    if mode & 0xFF == 1 {
        let k = w.at_key(pos & 0xFF, (pos >> 8) + 1);
        if k >= 0 {
            rec.set_plans(0x2FF);
            set_target(rec, k);
        } else {
            rec.set_plans(0x6FF);
            set_target(rec, ahead(pos));
        }
        return;
    }
    rec.set_plans(0x6FF);
    if pos >> 8 == front_row(w, ctrl) {
        set_target(rec, pos);
        return;
    }
    let v = lane_cell(w, ctrl);
    set_target(rec, if v < 0 { ahead(pos) } else { v });
}

/// 0x13B310(rec, pos): magic (class 1) and trap (class 3) cards. Traps:
/// plan 3 to the own column in the controller's leader's row. Magic: plan
/// 6 to the [`strongest_threat`]'s column in the own row, or plan 5 when
/// there is none. Other classes plan 5.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let pos = w.pos(g, s);
    match w.kind(g, s) >> 5 {
        3 => {
            rec.set_plans(0x3FF);
            let lp = w.pos(ctrl, 0);
            set_target(rec, (lp >> 8) << 8 | (pos & 0xFF));
        }
        1 => {
            let k = strongest_threat(w, ctrl)?;
            if k < 0 {
                rec.set_plans(0x5FF);
            } else {
                rec.set_plans(0x6FF);
                let kp = w.pos(k >> 8, k & 0xFF);
                set_target(rec, (pos >> 8) << 8 | (kp & 0xFF));
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x13E250(side, card): Pegasus's 0x139860 (0x110210 without the moth
/// exclusions) with the tribute ATK limit 3000.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, RITUAL_ATK_LIMIT, false)
}

/// The counting loops of 0x13DCA0 over slots 1..=5 of group `g` and of the
/// other group, in that order per slot: units controlled by `g` that `hit`
/// accepts, as a byte.
fn count_own<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    hit: impl Fn(i32, i32) -> bool,
) -> u8 {
    let other = i32::from(g == 0);
    let mut count = 0u8;
    for slot in 1..6 {
        for grp in [g, other] {
            if hit(grp, slot) && w.ctrl(grp, slot) == g {
                count = count.wrapping_add(1);
            }
        }
    }
    count
}

/// 0x13DCA0(g, s): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters: M-Warrior #1 / #2 and Swamp / Lava
/// Battleguard reveal with two or more own copies of their partner in the
/// groups' slots 1..=5 ([`count_own`]), others 2. Rituals: 2 under a
/// type-0xE far trap (0x27F170 mode 6, see-all 0), when the tributes fail
/// [`ritual_ok`] or on Crush; else 1. Magic: 2 under a type-0xE trap; else
/// 1 when the card shares its column or its row with the controller's
/// [`strongest_threat`] (a missing threat, key −1, reads as position −1 and
/// matches nothing). Power-ups and traps 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    let pos = w.pos(g, s);
    // 0x1E1DF0(g, s, 0) is called and its result dropped.
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    match w.kind(g, s) >> 5 {
        0 => {
            let partner = match card {
                id::M_WARRIOR_2 => id::M_WARRIOR_1,
                id::M_WARRIOR_1 => id::M_WARRIOR_2,
                id::SWAMP_BATTLEGUARD => id::LAVA_BATTLEGUARD,
                id::LAVA_BATTLEGUARD => id::SWAMP_BATTLEGUARD,
                _ => return Ok(2),
            };
            let n = count_own(w, g, |grp, slot| w.card_id(grp, slot) == partner);
            Ok(if n < 2 { 2 } else { 1 })
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            if ritual_ok(w, ctrl, card)? != 1 {
                return Ok(2);
            }
            Ok(if w.terrain(pos & 0xFF, pos >> 8) == CRUSH {
                2
            } else {
                1
            })
        }
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            let k = strongest_threat(w, ctrl)?;
            // 0x1E2310(−1, 0xFF): 0x1DE3A0 maps the negative group through
            // 0x1F3B90(0xFF), which answers −1 (0xFF ≥ 0x54): no unit.
            let kp = w.pos(k >> 8, k & 0xFF);
            Ok(if pos & 0xFF == kp & 0xFF || pos >> 8 == kp >> 8 {
                1
            } else {
                2
            })
        }
        _ => Ok(2),
    }
}

#[cfg(test)]
mod tests;
