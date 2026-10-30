//! Manawyddan fab Llyr's red-route controller (opponent 20 at Stonehenge,
//! control byte 20, driver 0x167FB0 = Weevil's 0x1117D0 by opcode shape,
//! module 0x163A00..0x168178, 30 functions; deck 0x2E, leader 75 Skull
//! Knight at rank 12 with abilities read from the imported row
//! 0x2C26B8 + 75·40; #14 is the opposing leader's race-weakening aura of
//! 0x273420, read by every stat query). The module is Weevil's
//! template with Pegasus's actor choice (0x164140), charge test (0x167E40)
//! and summon subtree (0x1651E0 / 0x1653B0 / 0x1655B0 / 0x1659D0), York's
//! summon planner (0x164CC0 = 0x13B440 with see-all 1), fodder pass
//! (0x165790 = 0x13BED0 with the limit 1 and the card test 0x165970 =
//! 0x13C0B0) and executors (0x166D70 / 0x166790 / 0x1661C0 = 0x13D2E0 /
//! 0x13CD00 / 0x13C730 with radius 5, see-all 1 and strength 0), the
//! Labyrinth-ruler's ritual test (0x167F40 = 0x1358D0), Keith's monster
//! planner shape (0x164520 = 0x11D9E0 with 5 everywhere), an always-0
//! hidden strength (0x167FA0) and nine novel functions; the shared parts
//! run through the framework in [`super`] with the constants and hooks of
//! [`Manawyddan`].
//!
//! Own functions, by EE address: [`assign`] 0x163F30, [`monster`] 0x164520
//! (0x164790: 0x27F760 with radius 5 and limit 0xBB8), [`goal_plan`]
//! 0x1647F0 with [`goal_cell`] 0x164860, [`effect_monster`] 0x1648F0,
//! [`support`] 0x164B30 with [`support_cell`] 0x164C30, [`reveal`]
//! 0x167970, the plan-0xC executor [`plan_c`] 0x1675E0 with its graveyard
//! search [`graveyard_pick`] 0x1677B0, and the summon planner in [`summon`]
//! (0x164CC0 and its subtree: the category 0x164F50, the trap search
//! 0x1652D0, the cell 0x165BC0).
//!
//! Three things the executors of this module do that Weevil's did not: a
//! committed face-up magic or ritual card asked to walk (0x166D70) or
//! retreat (0x1661C0) is marked acted by the module itself (0x1E30F0,
//! [`super::Plan::MarkActed`]) instead of moving (York's block,
//! [`Opponent::FACE_UP_SUPPORT_PASSES`]); the position and reveal
//! requests use see-all 1; plan 0xC (card 0x2B0) activates the card on a
//! graveyard non-monster chosen by deck cost.

mod summon;

use super::exec;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{
    kind_of, nth_unit, popcount, remove_unit, ritual_spec, tributes, unit_mask, virt,
};
use super::world::{unsupported, World, R};
use super::{has_effect, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// Card 0x4F (0x1648F0: plan 2 in place first when the controller's LP
    /// is below 2000).
    pub const LOW_LP_ATTACKER: i32 = 0x4F;
    /// Card 0x129 (0x1648F0: five rounds of the reachable-unit tree over
    /// the enemy's non-leaders, see-all 1, bound 5).
    pub const HUNTER: i32 = 0x129;
    /// Card 0x2B0, a magic card (0x164B30: plan 0xC; 0x1675E0: activated
    /// on a graveyard non-monster).
    pub const GRAVEYARD_MAGIC: i32 = 0x2B0;
    /// Magic cards of entry type 0xD (0x167970): 0x2CA reveals with three
    /// or more enemy units, 0x2C9 always, 0x2C3 while the controller's LP
    /// is below 2000.
    pub const TYPE_D_ENEMY_COUNT: i32 = 0x2CA;
    pub const TYPE_D_ALWAYS: i32 = 0x2C9;
    pub const TYPE_D_LOW_LP: i32 = 0x2C3;
    /// The rituals 0x34A and 0x352 (0x164F50: summoned with choosable
    /// tributes, 0x167F40).
    pub const RITUAL_34A: i32 = 0x34A;
    pub const RITUAL_352: i32 = 0x352;
}

/// 0x164790: the ATK limit of 0x27F760's first pass (0xBB8 = 3000).
const PASS_ATK_LIMIT: i32 = 0xBB8;
/// 0x164520: the move limit of the enemy-leader test (0x27CB40) and the
/// last round and radius of the 0x27F760 passes.
const LEADER_MOVES: i32 = 5;
const PASS_ROUNDS: i32 = 5;
const PASS_RADIUS: i32 = 5;
/// 0x1648F0 / 0x167970: the LP test (`slti 0x7D0` = 2000).
const LOW_LP: i32 = 0x7D0;
/// 0x1648F0: the rounds and bound of card 0x129's target search.
const HUNTER_ROUNDS: i32 = 5;
const HUNTER_BOUND: i32 = 5;
/// 0x163F30: the leader's walking target (col 3, row 1).
const LEADER_GOAL: u16 = 0x103;
/// 0x1677B0: the deck cost a graveyard card must reach (`slt` against
/// 0x28 = 40, raised to every accepted card's cost).
const GRAVEYARD_MIN_COST: i32 = 0x28;
/// 0x1677B0: the entry type never picked (0x14).
const REVIVAL_TYPE: u32 = 0x14;
/// 0x167970: the unit count of type-0x18 magic (`slti 5`) and of the
/// enemy for card 0x2CA (`slti 3`).
const TYPE_18_MIN_UNITS: i32 = 5;
const ENEMY_MIN_UNITS: i32 = 3;
/// 0x27F500: the menu filter of non-monsters.
const MENU_OTHERS: i32 = 0x1E;
/// Crush terrain (9).
const CRUSH: i32 = 9;

/// Manawyddan fab Llyr's red-route module.
pub(super) struct Manawyddan;

impl Opponent for Manawyddan {
    const CONTROL: u8 = 20;
    /// 0x166D70 / 0x166790: strength 0 in the route scoring (`move $t0,
    /// $zero` at 0x1670A4 / 0x166B2C / 0x166984).
    const DANGER_HIDDEN: i32 = 0;
    /// Every 0x27FCA0 call of 0x166D70, 0x166790 and 0x1661C0 passes 1.
    const STANCE_SEE_ALL: bool = true;
    /// 0x166A14 / 0x166A1C: plan 2's 0x27B9D0 takes 1 as the first
    /// stack argument (see-all). The hidden strength is separately 0 at
    /// sp+0x10; it is not the search's see-all flag.
    const SEARCH_SEE_ALL: i32 = 1;
    /// The plan-9 call sites 0x163C7C / 0x163E8C pass t0 = 1 (and t2 = 5,
    /// which 0x27E960 ignores).
    const EQUIP_SEARCH_SEE_ALL: i32 = 1;
    /// 0x166278 / 0x1665B8 (`addiu $t1, 1`) and 0x1664B0.
    const RETREAT_SEE_ALL: bool = true;
    const IMMOBILE_STANCE: bool = true;
    /// 0x163CF0: `move $s1, $zero` in the delay slot of the plan-0xE
    /// branch, so the secondary plan 0xE returns 0 (the turn ends).
    const STAY_B_RETURN: i32 = 0;
    /// 0x16709C / 0x166B24: radius 5.
    const DANGER_RADIUS: i32 = 5;
    /// 0x166974 / 0x166274 / 0x1665B4: radius 2 for a unit that stays put.
    const STATIC_DANGER_RADIUS: i32 = 2;
    /// 0x166EFC / 0x16640C: York's block (0x13D400 / 0x13C910).
    const FACE_UP_SUPPORT_PASSES: bool = true;
    /// 0x16421C (as Pegasus's 0x13633C): class 3 only.
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
    /// 0x167E40 = Pegasus's 0x139950.
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
    /// 0x163CA8 / 0x163EB8: 0xC → 0x1675E0 (which = 1 / 0); it counts as
    /// executed whatever it did.
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

/// 0x167F40(side, card): the Labyrinth-ruler's 0x1358D0: 1 when the
/// ritual's tributes can be chosen (0x2561B0 on the 0x2565B0 spec of its
/// entry 0 word 1), without the per-tribute tests of Weevil's 0x110210.
pub(super) fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    // 0x2715C0(card, 0): entry 0 of a virtual record (null for 999).
    if card == 999 {
        return unsupported("0x167F40 reads entry 0 of card 999");
    }
    let word1 = virt(w, card).entries[0].map_or(u32::MAX, |e| e[1]);
    Ok(i32::from(tributes(w, &ritual_spec(word1), side)? >= 0))
}

/// 0x163F30(rec): (re)assign the plans of a record (no mode). 0x1E2310 is
/// called and dropped; 0x251C80 gives the controller. Missing, acted or
/// free records are left alone. The leader (kept while the +8 word is not
/// −1): threatened (0x27D500, see-all 1) → 0x50A / 0x5FF by the summon
/// flag; else 0xA06 with the secondary target (col 3, row 1), or 0x6FF
/// with that primary target. Non-leaders by class (jump table 0x2BA410):
/// 0 and 5 → [`effect_monster`] when 0x2520F0 answers 1, else
/// [`monster`](self::monster)(rec, 1) (rituals never have an effect
/// answer, so they plan as monsters); 2 → 0x27E390(rec, 5, 1,
/// 0x167FA0(controller) & 0xFF); 1, 3, 4 → [`support`]; 6 and above →
/// 0x5FF.
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
        if w.threatened(g, 0, true)? {
            rec.set_plans(if summon { 0x50A } else { 0x5FF });
        } else if summon {
            rec.set_plans(0xA06);
            rec.target_b = LEADER_GOAL;
        } else {
            rec.set_plans(0x6FF);
            rec.target_a = LEADER_GOAL;
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        0 | 5 => {
            if has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec, 1)?;
            }
        }
        2 => {
            let hv = Manawyddan::hidden_value(w, ctrl)?;
            plans::power_up(w, rec, 5, 1, hv & 0xFF)?;
        }
        1 | 3 | 4 => support(w, rec),
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x164520(rec, a1): monsters without effects (Keith's 0x11D9E0 by
/// shape). `a1` ≠ 0 and 0x27CB40 reaches the enemy leader within 5 moves:
/// plan 2 on its key. Else N = 1..=5: 0x164790 → 0x27F760 (radius 5,
/// see-all 1, hidden 0x167FA0 = 0, limit 0xBB8): 1 → planned; 0 → plan 5;
/// 2 → plan 3 at the own cell; 3 → plan 6 at the own cell; else next N
/// (the reachable list and the node pool are shared by all rounds, as in
/// 0x10DD60). Exhausted → [`goal_plan`].
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    // 0x24F790(g, s): the controller.
    let ctrl = w.ctrl(g, s);
    let leader = i32::from(ctrl == 0) << 8;
    let my = w.pos(g, s);
    let (lg, ls) = (leader >> 8, leader & 0xFF);
    let lp = w.pos(lg, ls);
    // 0x2800B0(sp+0xD0): a no-op; sp+0x140: the reachable list (key
    // 0xFFFF, level 0xFF), once.
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
    // sp+0x19C / sp+0x19E: the pool counters, zeroed once.
    let mut pool = Pool::new();
    for n in 1..=PASS_ROUNDS {
        let side = i32::from(w.ctrl(g, s) == 0);
        let hidden = Manawyddan::hidden_value(w, ctrl)?;
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
    goal_plan(w, rec);
    Ok(())
}

/// 0x164860(col, row): the cell a unit at (col, row) heads for, (row << 8)
/// | col: row 0 → (col 0, row 1) from columns 0..=2, else (col 6, row 1);
/// rows 1..=5 (and negative rows, `slti`) → (col 1, row 6) from columns
/// 0..=2, else (col 5, row 6); rows 6 and above → (col 5, row 6) from
/// columns 0..=2, else (col 1, row 6).
pub(super) fn goal_cell(col: i32, row: i32) -> i32 {
    let near = col < 3;
    if row == 0 {
        if near {
            0x100
        } else {
            0x106
        }
    } else if row < 6 {
        if near {
            0x601
        } else {
            0x605
        }
    } else if near {
        0x605
    } else {
        0x601
    }
}

/// 0x1647F0(rec): plan 6 (secondary cleared) to the unit's [`goal_cell`]
/// from its position (0x1E2310; −1 without a unit reads as column 0xFF
/// in row −1). 0x251C80 is called and dropped.
fn goal_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.ctrl(g, s);
    let pos = w.pos(g, s);
    rec.set_plans(0x6FF);
    set_target(rec, goal_cell(pos & 0xFF, pos >> 8));
}

/// 0x1648F0(rec): monsters with an effect entry, by card. 0x4F: plan 2 at
/// the own cell (the target is the position) while the controller's LP
/// (0x1E06F0) is below 2000, then [`monster`](self::monster)(rec, 1).
/// 0x129: N = 1..=5 rounds of the reachable-unit tree over the enemy's
/// units (0x278A20 from the own cell, see-all 1, hidden 0x167FA0 = 0; the
/// root, list and count carry over), after each the first non-leader
/// 0x278AA0 accepts (bound 5, filter 0x34, face-mode 0, def-mode 2,
/// see-all 1): plan 2 on it; none → [`monster`](self::monster)(rec, 1).
/// Others → [`monster`](self::monster)(rec, 1).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let my = w.pos(g, s);
    // sp+0x120: the reachable list; sp+0x17A / sp+0x17C / sp+0x17E: the
    // pool counters and the list count, zeroed once; 0x2800B0(sp+0xB0): a
    // no-op.
    let mut list = Targets::new();
    let mut pool = Pool::new();
    match w.card_id(g, s) {
        id::LOW_LP_ATTACKER => {
            if w.lp(ctrl) < LOW_LP {
                rec.set_plans(0x2FF);
                set_target(rec, my);
            }
            monster(w, rec, 1)
        }
        id::HUNTER => {
            let (c, r) = (my & 0xFF, my >> 8);
            let enemy = i32::from(ctrl == 0);
            let mut root = None;
            for n in 1..=HUNTER_ROUNDS {
                let hidden = Manawyddan::hidden_value(w, ctrl)?;
                root = search::reach(
                    w,
                    &mut pool,
                    &mut list,
                    root,
                    Some(u),
                    c,
                    r,
                    enemy,
                    n,
                    1,
                    hidden,
                )?;
                let hidden = Manawyddan::hidden_value(w, ctrl)?;
                let k = search::pick_target(
                    w,
                    u,
                    c,
                    r,
                    &list,
                    HUNTER_BOUND,
                    0x34,
                    0,
                    2,
                    1,
                    hidden,
                    i32::from(list.count),
                )?;
                if k >= 0 {
                    rec.set_plans(0x2FF);
                    set_target(rec, k);
                    return Ok(());
                }
            }
            monster(w, rec, 1)
        }
        _ => monster(w, rec, 1),
    }
}

/// 0x164C30(i): the cells a support card heads for, in order: (col 3,
/// row 2), (col 1, row 3), (col 5, row 3), (col 2, row 2), (col 4, row 2);
/// (col 3, row 2) for any other index.
pub(super) fn support_cell(i: i32) -> i32 {
    match i & 0xFF {
        0 => 0x203,
        1 => 0x301,
        2 => 0x305,
        3 => 0x202,
        4 => 0x204,
        _ => 0x203,
    }
}

/// 0x164B30(rec): magic (class 1) and trap (classes 3, 4) cards. Nothing
/// without a unit. The plan is 0xC (secondary cleared) for card 0x2B0,
/// else 3; the primary target becomes the first of the five
/// [`support_cell`]s that is empty (0x1E4A10 negative) or holds this
/// unit; none leaves the target as it was. 0x24F790 and 0x1E2310 are
/// called and dropped.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return;
    }
    let _ = w.ctrl(g, s);
    let _ = w.pos(g, s);
    if w.card_id(g, s) == id::GRAVEYARD_MAGIC {
        rec.set_plans(0xCFF);
    } else {
        rec.set_plans(0x3FF);
    }
    for i in 0..5 {
        let cell = support_cell(i) & 0xFFFF;
        let k = w.at_key(cell & 0xFF, cell >> 8);
        if k == i32::from(rec.key) {
            set_target(rec, cell);
            return;
        }
        if k >= 0 {
            continue;
        }
        set_target(rec, cell);
        return;
    }
}

/// The affected-unit mask of a magic card's entry 0 as 0x167970 builds it:
/// 0x24E360(c, r, controller, side mode (word0 >> 6) & 3, filter word0 &
/// 0x3F, 0xE) without the card itself (0x24E0F0).
fn affected<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    c: i32,
    r: i32,
    ctrl: i32,
    word0: u32,
    g: i32,
    s: i32,
) -> R<i32> {
    let mask = unit_mask(w, c, r, ctrl, ((word0 >> 6) & 3) as i32, word0 & 0x3F, 0xE)?;
    Ok(remove_unit(mask, g, s))
}

/// 0x167B68 keeps t1 = 0x12 for 0x24E360 at 0x167B94. That mode's
/// handler (0x24E974..0x24EAD0) includes only non-leaders sharing the
/// origin's column OR row and accepts only filters 0x1D/0x1E/0x1F.
/// Its class and controller tests are the same as the global mask's.
fn cross_affected<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    c: i32,
    r: i32,
    ctrl: i32,
    word0: u32,
    g: i32,
    s: i32,
) -> R<i32> {
    if !matches!(word0 & 0x3F, 0x1D..=0x1F) {
        return Ok(0);
    }
    let candidates = affected(w, c, r, ctrl, word0, g, s)?;
    let mut mask = candidates;
    for i in 0..popcount(candidates) {
        let k = nth_unit(candidates, i);
        let pos = w.pos(k >> 8, k & 0xFF);
        if pos & 0xFF != c && pos >> 8 != r {
            mask = remove_unit(mask, k >> 8, k & 0xFF);
        }
    }
    Ok(mask)
}

/// 0x167BD4 / 0x167C98: over the units of `mask` (0x250570, skipping
/// negative keys): 2 as soon as one is controlled by `ctrl`; else the
/// number of the others (a byte).
fn count_enemies<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mask: i32,
    ctrl: i32,
) -> Option<u32> {
    let mut count = 0u8;
    for i in 0..popcount(mask) {
        let k = nth_unit(mask, i);
        if k < 0 {
            continue;
        }
        if w.ctrl(k >> 8, k & 0xFF) == ctrl {
            return None;
        }
        count = count.wrapping_add(1);
    }
    Some(u32::from(count))
}

/// 0x167970(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters, power-ups and traps 2. Rituals: 1 when
/// 0x167F40(controller, card) allows it and the own cell is not Crush,
/// else 2. Magic: 2 under a type-0xE far trap (0x27F170 mode 6, see-all
/// 1) or without an entry; by entry-0 type:
/// - 0x26 → 1 while the controller's summon flag (0x1DF950) is clear;
/// - 0x18 → 1 when the [`affected`] units (from (0, 0)) left by 0x273B40
///   number five or more;
/// - 0xD → card 0x2CA: 1 with three or more enemy units in slots 1..=10
///   (0x1E1390); 0x2C9: 1; 0x2C3: 1 while the controller's LP is below
///   2000; others 2;
/// - 4 → entry-0 word 1 >> 16 = 0x12: over [`cross_affected`] units in
///   the own row or column without either leader, 2 when one is own, else 1;
///   otherwise the units from (0, 0) left by 0x273B40: 2 when one is own,
///   else their COUNT (quirk: only a count of 1 presses R2);
/// - 0x17 → 1 when the first [`affected`] unit (from (0, 0)) left by
///   0x273B40 exists and is not own;
/// - other types 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    // 0x1E1DF0(g, s, 0): null without a unit or an entry.
    let entry0 = w.unit(g, s).and_then(|u| u.entry(0));
    let pos = w.pos(g, s);
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    match w.kind(g, s) >> 5 {
        5 => {
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
            if w.classify(g, s, 6, true)? == 5 {
                return Ok(2);
            }
            let Some([w0, w1]) = entry0 else {
                return Ok(2);
            };
            match w0 >> 8 {
                0x26 => Ok(if w.can_summon(ctrl) { 2 } else { 1 }),
                0x18 => {
                    let mask = affected(w, 0, 0, ctrl, w0, g, s)?;
                    let mask = super::pegasus::protected_filter(w, mask, ctrl)?;
                    Ok(if popcount(mask) < TYPE_18_MIN_UNITS {
                        2
                    } else {
                        1
                    })
                }
                0xD => match card {
                    id::TYPE_D_ENEMY_COUNT => {
                        let other = i32::from(ctrl == 0);
                        let units = w.monsters(other) + w.others(other);
                        Ok(if units < ENEMY_MIN_UNITS { 2 } else { 1 })
                    }
                    id::TYPE_D_ALWAYS => Ok(1),
                    id::TYPE_D_LOW_LP => Ok(if w.lp(ctrl) < LOW_LP { 1 } else { 2 }),
                    _ => Ok(2),
                },
                4 => {
                    if w1 >> 16 == 0x12 {
                        let mask = cross_affected(w, pos & 0xFF, pos >> 8, ctrl, w0, g, s)?;
                        let mask = remove_unit(mask, g, 0);
                        let mask = remove_unit(mask, i32::from(g == 0), 0);
                        return Ok(match count_enemies(w, mask, ctrl) {
                            None => 2,
                            Some(_) => 1,
                        });
                    }
                    let mask = affected(w, 0, 0, ctrl, w0, g, s)?;
                    let mask = super::pegasus::protected_filter(w, mask, ctrl)?;
                    Ok(count_enemies(w, mask, ctrl).unwrap_or(2))
                }
                0x17 => {
                    let mask = affected(w, 0, 0, ctrl, w0, g, s)?;
                    let mask = super::pegasus::protected_filter(w, mask, ctrl)?;
                    let k = nth_unit(mask, 0);
                    if k < 0 || w.ctrl(k >> 8, k & 0xFF) == ctrl {
                        return Ok(2);
                    }
                    Ok(1)
                }
                _ => Ok(2),
            }
        }
        _ => Ok(2),
    }
}

/// 0x1677B0(side): the graveyard card plan 0xC revives, (graveyard side
/// << 16) | index, or −1. −1 at once with five cards in the side's slots
/// 6..=10 (0x1E1470). Over the own graveyard, then the enemy's: every
/// non-monster (0x271610 class ≠ 0) whose record's deck cost (byte 3)
/// reaches the best so far (from 40, ties accepted) and whose entry 0 is
/// not of type 0x14 becomes the pick and raises the bar.
fn graveyard_pick<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if w.others(side) >= 5 {
        return Ok(-1);
    }
    let mut best = GRAVEYARD_MIN_COST;
    let mut pick = -1;
    for gy in [side, i32::from(side == 0)] {
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
                return unsupported("0x1677B0 reads a card record's deck cost");
            };
            if i32::from(cost) < best {
                continue;
            }
            let Some([w0, _]) = u.entries[0] else {
                return unsupported("0x1677B0 reads a null entry 0");
            };
            if w0 >> 8 == REVIVAL_TYPE {
                continue;
            }
            best = i32::from(cost);
            pick = (gy << 16) | (i - 1);
        }
    }
    Ok(pick)
}

/// 0x1675E0(rec, side, d, which): plan 0xC (card 0x2B0). With a graveyard
/// card ([`graveyard_pick`]) and no type-0xE far trap (0x27F170 mode 6,
/// see-all 1): type 0xC, +0x10 from 0x27FCA0(defflag 1, see-all 1) on the
/// stale +4, +0xC = 1, +4 = (graveyard side << 8) | its menu index among
/// the non-monsters (0x27F500 with filter 0x1E), the slot cleared
/// (0x27CAF0). Else a card committed face-up is marked acted by the module
/// itself (0x1E30F0(g, s, 0, 1)) with an empty type-0xE record, and a set
/// one walks to its plan target as plan 3 does (0x166D70 with defflag 1).
fn plan_c<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    let mut t = graveyard_pick(w, side)?;
    if w.classify(g, s, 6, true)? == 5 {
        t = -1;
    }
    if t < 0 {
        if w.flag(g, s, 4) {
            d.mark_acted = Some(rec.key);
            d.key = rec.key;
            d.kind = 0xE;
            return Ok(());
        }
        return exec::go_to_cell::<Manawyddan, _, _>(w, rec, side, d, which, true);
    }
    let gy = (t >> 16) & 0xFFFF;
    let menu = super::necromancer::graveyard_menu_index(w, gy, t & 0xFF, MENU_OTHERS);
    d.key = rec.key;
    d.kind = 0xC;
    d.position = exec::stance(w, g, s, d.target, 1, true)?;
    d.reveal = 1;
    d.target = ((gy << 8) | menu) as u16;
    rec.consume(which);
    Ok(())
}

#[cfg(test)]
mod tests;
