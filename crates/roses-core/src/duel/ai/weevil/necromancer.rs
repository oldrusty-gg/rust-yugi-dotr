//! Necromancer's controller (opponent 6, control byte 6, driver 0x12C270 =
//! Weevil's 0x1117D0 by opcode shape, module 0x127550..0x12C438, 29
//! functions). The module is Weevil's template with Pegasus's actor choice
//! (0x127CB0) and charge test (0x12C000), Labyrinth-ruler's summon planner
//! shape (0x128700 = 0x132D30) over Pegasus's hand search, non-monster
//! choice, monster choice and card list (0x128D00 / 0x128DF0 / 0x128FF0 /
//! 0x129420), Keith's monster planner shape (0x128090 = 0x11D9E0 with
//! other constants), Darkness-ruler's executors (0x12A5D0 / 0x12ABB0 /
//! 0x129A90 with radius 5), his plan-0xB executor (0x12B390 = 0x1309F0
//! with radius 5) and his plan-0xC executor shape (0x129FD0), the
//! Labyrinth-ruler's hidden-strength function shape (0x12BA80) and seven
//! novel functions; the shared parts run through the framework in
//! [`super`] with the constants and hooks of [`Necromancer`].
//!
//! Own functions, by EE address: the dispatch [`dispatch`] 0x127550
//! (Weevil's 0x10D210 with the plan codes 0xB / 0xC and the slot clearer
//! [`clear_plan`] 0x12C100 in place of 0x27CAF0 at most sites),
//! [`hidden_value`] 0x12BA80, [`assign`] 0x127AC0, [`monster`] 0x128090
//! (0x128300: 0x27F760 with radius 5 and limit 0x7D0), [`goal_plan`]
//! 0x128360, [`effect_monster`] 0x1283D0, [`support`] 0x1285F0 (magic,
//! trap and ritual cards), [`ritual_ok`] 0x12C180, [`own_strongest_holds`]
//! 0x12B8D0 with [`pick_strongest`] 0x27CF90, [`reveal`] 0x12BAE0, the
//! plan-0xC executor [`plan_c`] 0x129FD0 with the shared-library graveyard
//! helpers [`graveyard_best`] 0x27F380 and [`graveyard_menu_index`]
//! 0x27F500, and the summon planner in [`summon`] (0x128700 and its
//! subtree).
//!
//! Plan 0xC (card 0x2AD, a magic card revived from the graveyard) activates
//! the card on the strongest graveyard monster when its reveal test holds,
//! marks a committed face-up card acted ([`super::Plan::MarkActed`]) or
//! retreats; plan 0xB is only reached through 0x1283D0's escort (card
//! 0x6F on the strongest enemy), which Darkness-ruler's executor walks.

mod summon;

use super::exec;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{atk_def, kind_of, nth_unit, popcount, ritual_spec, tributes, virt};
use super::world::{unsupported, World, R};
use super::{choose_actor, has_effect, stay, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// Pumpking the King of Ghosts, the leader card (0x1283D0: two trap
    /// passes then plan 3 in place; 0x128990: summoned when in hand;
    /// 0x1293B0: never fodder; 0x12BAE0: always reveals).
    pub const PUMPKING: i32 = 0x6C;
    /// Card 0x6F (0x1283D0: plan 0xB on the strongest enemy, 0x27F060;
    /// 0x1293B0: never fodder).
    pub const ESCORT: i32 = 0x6F;
    /// Card 0x68 (0x1293B0: never fodder).
    pub const NO_FODDER: i32 = 0x68;
    /// Cards 0x77, 0x66 and 0x65 (0x12BAE0: reveal with two or more own
    /// copies in the slots 1..=5 of either group).
    pub const PAIR_A: i32 = 0x77;
    pub const PAIR_B: i32 = 0x66;
    pub const PAIR_C: i32 = 0x65;
    /// Card 0x2AD, a magic card (0x1285F0: plan 0xC; 0x128990 / 0x12BAE0:
    /// with a graveyard monster of ATK 2400 or more).
    pub const REBORN: i32 = 0x2AD;
    /// Card 0x2B2, a magic card (0x1285F0: plan 6 to (col 5, row 4)).
    pub const CARD_2B2: i32 = 0x2B2;
    /// Card 0x2BB, a magic card (0x128990 / 0x12BAE0: when the leader is
    /// threatened or the own strongest monster holds, 0x12B8D0).
    pub const CARD_2BB: i32 = 0x2BB;
    /// Card 0x2E9, a magic card (0x1285F0: plan 6 to the goal cell;
    /// 0x12BAE0: reveals with three or more own units not of type 2).
    pub const CARD_2E9: i32 = 0x2E9;
    /// The ritual 0x353 (0x128990: summoned with acceptable tributes).
    pub const RITUAL_353: i32 = 0x353;
}

/// 0x128300 / 0x1283D0: the ATK limit of 0x27F760's first pass (0x7D0 =
/// 2000).
const PASS_ATK_LIMIT: i32 = 0x7D0;
/// 0x128090: the move limit of the enemy-leader test (0x27CB40) and the
/// last round and radius of the 0x27F760 passes.
const LEADER_MOVES: i32 = 5;
const PASS_ROUNDS: i32 = 5;
const PASS_RADIUS: i32 = 5;
/// 0x12BA80: the LP test (`slti 0x4B1`) and its answers.
const LOW_LP: i32 = 0x4B1;
const HIDDEN_LOW_LP: i32 = 0x960;
const HIDDEN_FEW_MONSTERS: i32 = 0x7D0;
const HIDDEN_DEFAULT: i32 = 0x3E8;
/// 0x12C180: the tribute ATK limit (`slti 0x7D0`).
const RITUAL_ATK_LIMIT: i32 = 0x7D0;
/// 0x128990 / 0x12BAE0: the graveyard monster's ATK limit for card 0x2AD
/// (`slti 0x960` = 2400).
const REBORN_ATK_LIMIT: i32 = 0x960;
/// 0x12B8D0: the enemy ATK that counts as strong (`slti 0xBB8` = 3000).
const STRONG_ENEMY_ATK: i32 = 0xBB8;
/// 0x12B390: the radius of the plan-0xB executor's danger scores (3 in
/// Darkness-ruler's 0x1309F0).
const TRAP_DANGER_RADIUS: i32 = 5;
/// 0x128360 / 0x1285F0 / 0x129610: the goal cells (row << 8 | col): rows
/// 0..=2 head for (col 1, row 3), the others for (col 2, row 5).
const GOAL_ROW_SPLIT: i32 = 3;
const GOAL_NEAR: i32 = 0x301;
const GOAL_FAR: i32 = 0x502;
/// 0x1285F0: card 0x2B2's cell (col 5, row 4).
const CARD_2B2_CELL: i32 = 0x405;
/// 0x27F500: the menu filters (0x1D monsters, 0x1E non-monsters, 0x1F the
/// raw index).
const MENU_MONSTERS: i32 = 0x1D;
const MENU_OTHERS: i32 = 0x1E;
const MENU_RAW: i32 = 0x1F;

/// Necromancer's module.
pub(super) struct Necromancer;

impl Opponent for Necromancer {
    const CONTROL: u8 = 6;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 1;
    /// 0x12ABB0 / 0x12A5D0: radius 5.
    const DANGER_RADIUS: i32 = 5;
    /// 0x127E2C (as Pegasus's 0x13633C): class 3 only.
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        Ok(hidden_value(w, side))
    }
    /// 0x12C100 at the executors' exits (0x12ABB0, 0x129A90 and the
    /// immobile path of 0x12A5D0).
    fn clear_plan(rec: &mut Record, which: u8) {
        clear_plan(rec, which);
    }
    fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        d: &mut Decision,
    ) -> R<i32> {
        dispatch(w, mem, d)
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
    /// 0x12C000 = Pegasus's 0x139950.
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
    /// 0x127804 / 0x127A04: 0xB → 0x12B390 (Darkness-ruler's 0x1309F0 with
    /// radius 5); 0x12781C / 0x127A50: 0xC → 0x129FD0. Both count as
    /// executed whatever they return.
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
                super::darkness::trap_move::<Necromancer, _, _>(
                    w,
                    rec,
                    side,
                    d,
                    which,
                    TRAP_DANGER_RADIUS,
                )?;
            }
            0xC => plan_c(w, rec, side, d, which)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// 0x12BA80(side): the strength assumed for hidden units: 0x960 (2400)
/// while the side's LP (0x1E06F0) is below 1201; else 0x7D0 (2000) with
/// fewer than four monsters in its slots 1..=5 (0x1E13D0), else 0x3E8
/// (1000). Passed whole (0x27E960's search strength, the plan passes, the
/// summon searches), `& 0xFFFF` (the executors' route searches and the
/// retreat's dangers) or `& 0xFF` (the see-all byte of the executors'
/// dangers, 0x27E390's strength).
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

/// 0x12C100(rec, which): clear plan slot `which` (1 primary, 0 secondary):
/// its plan code becomes 0xFF and its target 0xFFFF (0x27CAF0 clears the
/// target only). Other values of `which` do nothing.
fn clear_plan(rec: &mut Record, which: u8) {
    match which {
        1 => {
            rec.plan_a = 0xFF;
            rec.target_a = 0xFFFF;
        }
        0 => {
            rec.plan_b = 0xFF;
            rec.target_b = 0xFFFF;
        }
        _ => (),
    }
}

/// 0x127550: Weevil's 0x10D210 with plan 0xB (0x12B390) and 0xC
/// (0x129FD0) in both slots, the secondary plan 0xE returning 1, and
/// 0x12C100 instead of 0x27CAF0 after the primary summon (either way), a
/// failed primary attack and a failed secondary summon (a successful one
/// keeps 0x27CAF0).
fn dispatch<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mem: &mut Memory,
    d: &mut Decision,
) -> R<i32> {
    let side = i32::from(w.active);
    let k = choose_actor::<Necromancer, _, _>(w, mem, side, d)?;
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
    let mut done = false;
    match rec.plan_a {
        0xE => {
            stay(w, rec, d);
            done = true;
        }
        0xC => {
            plan_c(w, rec, side, d, 1)?;
            done = true;
        }
        0xB => {
            super::darkness::trap_move::<Necromancer, _, _>(
                w,
                rec,
                side,
                d,
                1,
                TRAP_DANGER_RADIUS,
            )?;
            done = true;
        }
        9 => {
            exec::equip_move::<Necromancer, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        3 if !acted => {
            exec::go_to_cell::<Necromancer, _, _>(w, rec, side, d, 1, true)?;
            done = true;
        }
        6 if !acted => {
            exec::go_to_cell::<Necromancer, _, _>(w, rec, side, d, 1, false)?;
            done = true;
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d)?;
            clear_plan(rec, 1);
            if r < 0 {
                d.kind = 0xFF;
                d.reveal = 0;
            } else {
                done = true;
            }
        }
        2 if !acted => {
            let r = exec::attack::<Necromancer, _, _>(w, rec, side, d, 1)?;
            if r < 0 {
                clear_plan(rec, 1);
                return Ok(1);
            }
            done = true;
        }
        5 if !acted => {
            exec::retreat::<Necromancer, _, _>(w, rec, side, d, 1)?;
            done = true;
        }
        _ => (),
    }
    if done {
        return Ok(1);
    }
    match rec.plan_b {
        0xE => {
            stay(w, rec, d);
            Ok(1)
        }
        0xC => {
            plan_c(w, rec, side, d, 0)?;
            Ok(1)
        }
        9 => {
            exec::equip_move::<Necromancer, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        0xB => {
            super::darkness::trap_move::<Necromancer, _, _>(
                w,
                rec,
                side,
                d,
                0,
                TRAP_DANGER_RADIUS,
            )?;
            Ok(1)
        }
        3 | 6 | 2 | 5 if acted => Ok(0),
        3 => {
            exec::go_to_cell::<Necromancer, _, _>(w, rec, side, d, 0, true)?;
            Ok(1)
        }
        6 => {
            exec::go_to_cell::<Necromancer, _, _>(w, rec, side, d, 0, false)?;
            Ok(1)
        }
        0xA => {
            let r = summon::summon(w, i32::from(rec.key), side, d)?;
            if r < 0 {
                clear_plan(rec, 0);
                d.kind = 0xFF;
                d.reveal = 0;
            } else {
                rec.consume(0);
            }
            Ok(1)
        }
        2 => {
            if exec::attack::<Necromancer, _, _>(w, rec, side, d, 0)? < 0 {
                exec::retreat::<Necromancer, _, _>(w, rec, side, d, 0)?;
            }
            Ok(1)
        }
        5 => {
            exec::retreat::<Necromancer, _, _>(w, rec, side, d, 0)?;
            Ok(1)
        }
        _ => Ok(0),
    }
}

/// 0x127AC0(rec): (re)assign the plans of a record (no mode). Missing,
/// acted or free records are left alone. The leader (kept while the +8
/// word is not −1): threatened (0x27D500, see-all 0) → 0x50A / 0x5FF by
/// the summon flag; else 0xA06 with the secondary target 0, or 0x6FF with
/// the primary target 0 (the cell (col 0, row 0), as 0x110FC0 reads it).
/// Non-leaders by class (jump table 0x2BA390): 0 → [`effect_monster`]
/// when 0x2520F0 answers 1, else [`monster`](self::monster)(rec, 1); 2 →
/// 0x27E390(rec, 5, 0, 0x12BA80(controller) & 0xFF); 1, 3, 4, 5 →
/// [`support`]; 6 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    // 0x251C80(g, s): the controller.
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
        } else if summon {
            rec.set_plans(0xA06);
            rec.target_b = 0;
        } else {
            rec.set_plans(0x6FF);
            rec.target_a = 0;
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        0 => {
            if has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                monster(w, rec, 1)?;
            }
        }
        2 => plans::power_up(w, rec, 5, 0, hidden_value(w, ctrl) & 0xFF)?,
        1 | 3 | 4 | 5 => support(w, rec),
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x128090(rec, a1): monsters without effects (Keith's 0x11D9E0 by
/// shape). `a1` ≠ 0 and 0x27CB40 reaches the enemy leader within 5 moves:
/// plan 2 on its key. Else N = 1..=5: 0x128300 → 0x27F760 (radius 5,
/// see-all 0, hidden 0x12BA80(controller), limit 0x7D0): 1 → planned; 0
/// → plan 5; 2 → plan 3 at the own cell; 3 → plan 6 at the own cell; else
/// next N (the reachable list and the node pool are shared by all rounds,
/// as in 0x10DD60). Exhausted → [`goal_plan`].
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
    // sp+0x140: the reachable list (key 0xFFFF, level 0xFF), once;
    // 0x2800B0(sp+0xD0): a no-op.
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
        let hidden = hidden_value(w, ctrl);
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
            0,
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

/// The goal cell of a unit standing in `row` (0x128360, 0x1285F0,
/// 0x129610): (col 1, row 3) from rows 0..=2, (col 2, row 5) from the
/// others.
fn goal_cell(row: i32) -> i32 {
    if row < GOAL_ROW_SPLIT {
        GOAL_NEAR
    } else {
        GOAL_FAR
    }
}

/// 0x128360(rec): plan 6 (secondary cleared) to the unit's [`goal_cell`]
/// (its row from 0x1E2310: −1 without a unit counts as a low row).
fn goal_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let pos = w.pos(g, s);
    rec.set_plans(0x6FF);
    set_target(rec, goal_cell(pos >> 8));
}

/// 0x1283D0(rec): monsters with an effect entry, by card: Pumpking (0x6C)
/// → N = 1..=2 of 0x27F760 (radius 2, see-all 0, hidden
/// 0x12BA80(controller), limit 0x7D0): 1 → planned, 0 / 2 / 3 → plan 3 at
/// the own cell, else next N; exhausted → plan 3 at the own cell. Card
/// 0x6F → plan 0xB (the secondary cleared, the target left as 0x27F060
/// wrote it) when 0x27F060(rec, 0) planned the strongest enemy, else
/// [`monster`](self::monster)(rec, 1). Others →
/// [`monster`](self::monster)(rec, 1).
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
    // sp+0x130: the reachable list; sp+0x18C / sp+0x18E: the pool
    // counters; 0x2800B0(sp+0xC0): a no-op.
    let mut list = Targets::new();
    let mut pool = Pool::new();
    match w.card_id(g, s) {
        id::PUMPKING => {
            for n in 1..=2 {
                let side = i32::from(ctrl == 0);
                let hidden = hidden_value(w, ctrl);
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
                    2,
                    0,
                    PASS_ATK_LIMIT,
                )?;
                match v {
                    1 => return Ok(()),
                    0 | 2 | 3 => break,
                    _ => (),
                }
            }
            rec.set_plans(0x3FF);
            set_target(rec, my);
            Ok(())
        }
        id::ESCORT => {
            if plans::strongest_enemy(w, rec, 0)? == 1 {
                rec.set_plans(0xBFF);
                return Ok(());
            }
            monster(w, rec, 1)
        }
        _ => monster(w, rec, 1),
    }
}

/// 0x1285F0(rec): magic, trap and ritual cards (classes 1, 3, 4, 5), by
/// card (0x251C80 and 0x1E2310 are called and dropped): 0x2AD → plan 0xC
/// with the primary target 0xFFFF; 0x2B2 → plan 6 to (col 5, row 4);
/// 0x2E9 → plan 6 to the [`goal_cell`]; others → plan 5. The secondary
/// plan is cleared in every case.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.ctrl(g, s);
    let _ = w.pos(g, s);
    match w.card_id(g, s) {
        id::REBORN => {
            rec.set_plans(0xCFF);
            set_target(rec, 0xFFFF);
        }
        id::CARD_2B2 => {
            rec.set_plans(0x6FF);
            set_target(rec, CARD_2B2_CELL);
        }
        id::CARD_2E9 => {
            let pos = w.pos(g, s);
            rec.set_plans(0x6FF);
            set_target(rec, goal_cell(pos >> 8));
        }
        _ => rec.set_plans(0x5FF),
    }
}

/// 0x12C180(side, card): 1 when the ritual's tributes can be chosen
/// (0x2561B0 on the 0x2565B0 spec of its entry 0 word 1) and every one of
/// them has an effective ATK (0x252300 at its own cell) below 2000; else
/// 0.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    // 0x2715C0(card, 0): entry 0 of a virtual record (null for 999).
    if card == 999 {
        return unsupported("0x12C180 reads entry 0 of card 999");
    }
    let word1 = virt(w, card).entries[0].map_or(u32::MAX, |e| e[1]);
    let m = tributes(w, &ritual_spec(word1), side)?;
    if m < 0 {
        return Ok(0);
    }
    let mut i = 0;
    while i < popcount(m) {
        let k = nth_unit(m, i);
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        if w.eff_atk(g, s, p & 0xFF, p >> 8)? >= RITUAL_ATK_LIMIT {
            return Ok(0);
        }
        i += 1;
    }
    Ok(1)
}

/// 0x27CF90(list): the key with the highest 0x252300 ATK at its own cell
/// among list[1..] (quirk: entry 0 is never considered and fewer than two
/// entries give −1) whose +0x57 bit 4 is set; ties keep the earlier one.
fn pick_strongest<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    list: &[i32],
) -> R<i32> {
    if list.len() < 2 {
        return Ok(-1);
    }
    let (mut best, mut pick) = (-1, -1);
    for &k in &list[1..] {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        if !w.flag(g, s, 4) {
            continue;
        }
        let v = w.eff_atk(g, s, p & 0xFF, p >> 8)?;
        if best < v {
            best = v;
            pick = k;
        }
    }
    Ok(pick)
}

/// 0x12B8D0(side): 1 unless the side's strongest face-up monster is
/// weaker than the enemy's and that enemy's ATK is 3000 or more and the
/// side has three or more units in its slots 1..=10 (0x1E07C0); then 0.
/// The two strongest are [`pick_strongest`] over 0x27DC00(other, 0x1D, 0,
/// 0) and 0x27DC00(side, 0x1D, 1, 1), their ATK 0x252300 at their cell (a
/// missing pick, key −1, reads as ATK 0).
fn own_strongest_holds<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<i32> {
    let other = i32::from(side == 0);
    let enemy = pick_strongest(w, &w.list_seen(other, 0x1D, false, false))?;
    let own = pick_strongest(w, &w.list_seen(side, 0x1D, true, true))?;
    let atk = |k: i32| -> R<i32> {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        w.eff_atk(g, s, p & 0xFF, p >> 8)
    };
    let e = atk(enemy)?;
    let o = atk(own)?;
    if o >= e || e < STRONG_ENEMY_ATK {
        return Ok(1);
    }
    let mut count = 0;
    for s in 1..11 {
        if w.unit(side, s).is_some() {
            count += 1;
        }
    }
    Ok(i32::from(count < 3))
}

/// 0x27F380(side, mode): the graveyard monster with the highest printed
/// ATK (0x2714E0 >> 16; strict >, so the first of equals wins) over the
/// own graveyard (mode bit 0) and then the enemy's (mode bit 1), as
/// (graveyard side << 24) | (index << 16) | ATK; −1 when neither holds a
/// monster.
pub(super) fn graveyard_best<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    mode: i32,
) -> i32 {
    let mut gy = -1;
    let mut index = -1;
    let mut best = -1;
    let other = i32::from(side == 0);
    for (bit, g) in [(1, side), (2, other)] {
        if mode & bit == 0 {
            continue;
        }
        let mut i = 0;
        while i < w.graveyard_count(g) {
            let card = w.graveyard(g, i);
            if kind_of(w, card) >> 5 == 0 {
                let v = (atk_def(w, card) >> 16) as i32;
                if best < v {
                    best = v;
                    index = i;
                    gy = g;
                }
            }
            i += 1;
        }
    }
    (gy << 24) | (index << 16) | best
}

/// 0x27F500(gy, index, filter): the menu index of graveyard entry `index`
/// of side `gy`: with filter 0x1D the number of monsters before it, with
/// 0x1E the number of non-monsters before it (both: the raw index when
/// `index` is past the entries), with 0x1F and anything else the raw
/// index (& 0xFF).
pub(super) fn graveyard_menu_index<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    gy: i32,
    index: i32,
    filter: i32,
) -> i32 {
    let index = index & 0xFF;
    let monsters = match filter & 0xFF {
        MENU_MONSTERS => true,
        MENU_OTHERS => false,
        _ => return index,
    };
    let mut count = 0;
    let mut i = 0;
    while i < w.graveyard_count(gy) {
        if i == index {
            return count & 0xFF;
        }
        if (kind_of(w, w.graveyard(gy, i)) >> 5 == 0) == monsters {
            count += 1;
        }
        i += 1;
    }
    let _ = MENU_RAW;
    index
}

/// 0x12BAE0's counting loops: over the slots 1..=5 of group `g` and of the
/// other group, the units whose card is `card` (0x1E22B0) and whose
/// controller (0x251C80) is `g`.
fn own_copies<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, card: i32) -> i32 {
    let other = i32::from(g == 0);
    let mut count = 0u8;
    for slot in 1..6 {
        for grp in [g, other] {
            if w.card_id(grp, slot) == card && w.ctrl(grp, slot) == g {
                count = count.wrapping_add(1);
            }
        }
    }
    i32::from(count)
}

/// 0x12BAE0(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up (0x251C80 is called and dropped). By class (jump
/// table 0x2BA3B0): monsters 0x77, 0x66 and 0x65 reveal with two or more
/// own copies in the slots 1..=5 of either group ([`own_copies`]),
/// Pumpking always, others 2. Magic: 2 under a type-0xE far trap (0x27F170
/// mode 6, see-all 0); 0x2E9 → 1 with three or more own units in the
/// slots 1..=5 of either group whose kind & 0x1F is not 2 (0x1E07C0,
/// 0x1E1980, 0x251C80); 0x2AD → 1 when [`graveyard_best`](g, 3) holds a
/// monster of ATK 2400 or more, the group's slots 1..=5 hold fewer than
/// five units and the own cell is not Crush; 0x2BB → 1 when the group's
/// leader is threatened (0x27D500, see-all 0) or [`own_strongest_holds`];
/// others 2. Rituals: 1 when [`ritual_ok`](controller, card) and the own
/// cell is not Crush, else 2. Power-ups and traps 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let _ = w.ctrl(g, s);
    let pos = w.pos(g, s);
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    let pair = |c: i32| if own_copies(w, g, c) < 2 { 2 } else { 1 };
    let crush = w.terrain(pos & 0xFF, pos >> 8) == 9;
    match w.kind(g, s) >> 5 {
        0 => Ok(match card {
            id::PAIR_A | id::PAIR_B | id::PAIR_C => pair(card),
            id::PUMPKING => 1,
            _ => 2,
        }),
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            match card {
                id::CARD_2E9 => {
                    let other = i32::from(g == 0);
                    let mut count = 0u8;
                    for slot in 1..6 {
                        for grp in [g, other] {
                            if w.unit(grp, slot).is_some()
                                && w.kind(grp, slot) & 0x1F != 2
                                && w.ctrl(grp, slot) == g
                            {
                                count = count.wrapping_add(1);
                            }
                        }
                    }
                    Ok(if count < 3 { 2 } else { 1 })
                }
                id::REBORN => {
                    let v = graveyard_best(w, g, 3);
                    if v < 0 || v & 0xFFFF < REBORN_ATK_LIMIT || w.monsters(g) >= 5 || crush {
                        return Ok(2);
                    }
                    Ok(1)
                }
                id::CARD_2BB => {
                    if w.threatened(g, 0, false)? || own_strongest_holds(w, g)? == 1 {
                        return Ok(1);
                    }
                    Ok(2)
                }
                _ => Ok(2),
            }
        }
        5 => {
            if ritual_ok(w, w.ctrl(g, s), card)? == 1 && !crush {
                return Ok(1);
            }
            Ok(2)
        }
        _ => Ok(2),
    }
}

/// 0x129FD0(rec, side, d, which): plan 0xC (card 0x2AD). When the card's
/// reveal test ([`reveal`]) answers 1: type 0xC on the strongest graveyard
/// monster ([`graveyard_best`](side, 3)), +0x10 from 0x27FCA0(defflag 1,
/// see-all 0) on the stale +4, +0xC = 1, +4 = ((graveyard side & 0xF) <<
/// 8) | its menu index among the monsters ([`graveyard_menu_index`] with
/// filter 0x1D), and the slot cleared by 0x12C100. Else a card committed
/// face-up is marked acted by the module itself (0x1E30F0(g, s, 0, 1))
/// with an empty type-0xE record, and a set one retreats as 0x110330 does
/// with +0xC = 2 ([`exec::retreat_with`], whose exits clear the slot with
/// 0x12C100).
fn plan_c<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    if reveal(w, g, s, rec)? == 1 {
        let v = graveyard_best(w, side, 3);
        let gy = v >> 24;
        let index = (v >> 16) & 0xFF;
        let menu = graveyard_menu_index(w, gy & 0xFF, index, MENU_MONSTERS);
        d.key = rec.key;
        d.kind = 0xC;
        d.position = exec::stance(w, g, s, d.target, 1, false)?;
        d.reveal = 1;
        d.target = (((gy & 0xF) << 8) | menu) as u16;
        clear_plan(rec, which);
        return Ok(());
    }
    if w.flag(g, s, 4) {
        d.mark_acted = Some(rec.key);
        d.key = rec.key;
        d.kind = 0xE;
        return Ok(());
    }
    exec::retreat_with::<Necromancer, _, _>(w, rec, side, d, which, Some(2))
}

#[cfg(test)]
mod tests;
