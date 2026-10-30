//! Yugi's controller (opponent 19, white route, control byte 19, driver
//! 0x163830 = Weevil's 0x1117D0 by opcode shape, module 0x15DC20..0x1639F8,
//! 41 functions). The dispatch 0x15DC20 is Bakura's 0x159550 with plans
//! 0xC (0x1624C0, the generic module's 0x172630 with Yugi's graveyard
//! picks) and 0xD (0x162770 = the generic 0x1728D0) in place of plan 0xB,
//! 0xF / 0x10 running as plan 6 and a plan-9 t2 of 5; the actor choice
//! 0x15E3C0 is Pegasus's 0x136240; the executors are York's retreat
//! (0x1610A0, static radius 3), Labyrinth-ruler's attack (0x161670, radius
//! 2) and Seto's move (0x161C50); the planners reuse the generic module's
//! rush / row test / patrol (0x15E900 / 0x15ED10 / 0x15EDF0) with Yugi's
//! hidden strength and Joey's half-board exhaustion; the summon planner
//! 0x15FBD0 is Margaret's 0x1488F0 (+0xC = 2, monster limit 5).
//!
//! Own functions, by EE address: [`assign`] 0x15E1B0, [`planner`]
//! 0x15E7A0, [`rush`] 0x15E900, [`lane_cell`] 0x15EBB0,
//! [`effect_monster`] 0x15F220, [`hold_formation`] 0x15F530 with
//! 0x15F600 / 0x15F660 / 0x15F710 ([`formation_cell`]), [`support`]
//! 0x15F810, [`threat`] 0x162EB0, [`crowded_enemy`] 0x162FC0,
//! [`reveal`] 0x162980, [`hidden_value`] 0x162E50 and the summon subtree
//! in [`summon`].

mod summon;

use super::mako::neighbour_raw;
use super::margaret::graveyard_pick_with;
use super::plans::{self, set_target};
use super::search::{self, Pool, Targets};
use super::seto::executors::plan_c_with;
use super::seto::reveal::control_target;
use super::shadi::{passes, patrol_with, plan_d, row_clear};
use super::summon::{nth_unit, popcount, ritual_ok_with, unit_mask};
use super::world::{World, R};
use super::{has_effect, Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// 0x15EBB0: an own 0x33B sends units to (0, 5), an own 0x2ED to (6, 3).
    pub const CARD_33B: i32 = 0x33B;
    pub const CARD_2ED: i32 = 0x2ED;
    /// 0x15F220: patrols / hunts and holds the formation.
    pub const CARD_129: i32 = 0x129;
    /// 0x15F220: rushes.
    pub const CARD_57: i32 = 0x57;
    /// 0x15F220: plan 0xC in place while not committed face-up.
    pub const CARD_2B: i32 = 0x2B;
    /// 0x15F810 / 0x162980: Change of Heart, Brain Control, 0x2EA.
    pub const CHANGE_OF_HEART: i32 = 0x2DC;
    pub const BRAIN_CONTROL: i32 = 0x2DD;
    pub const CARD_2EA: i32 = 0x2EA;
    /// 0x1630A0 (non-monster plan C) and 0x163340 (monster plan C): the
    /// graveyard cards, in pick order.
    pub const GRAVE_FOR_SUPPORT: [i32; 3] = [0x57, 0x2B, 0x3C];
    pub const GRAVE_FOR_MONSTERS: [i32; 3] = [0x33B, 0x2BC, 0x2CB];
}

/// 0x15E900: the rush (5 moves, rounds 1..=5, radius 5).
const RUSH_MOVES: i32 = 5;
const RUSH_ROUNDS: i32 = 5;
const RUSH_RADIUS: i32 = 5;
/// 0x15E7A0 / 0x15F220: the ATK from which a monster rushes.
const RUSH_ATK: i32 = 0x9C4;
/// Joey-style exhaustion of 0x15E900: the half split and the row taken.
const HALF_ROW: i32 = 4;
const FRONT_ROW: i32 = 3;
/// 0x15F220: the hunt rounds, bound and filter.
const HUNT_ROUNDS: i32 = 2;
const HUNT_BOUND: i32 = 2;
const HUNT_FILTER: i32 = 0x34;
/// 0x162E50's LP test (`slti 0x4B1`), monster test (`slti 4`) and answers.
const LOW_LP: i32 = 0x4B1;
const FEW_MONSTERS: i32 = 4;
const HIDDEN_LOW_LP: i32 = 0x7D0;
const HIDDEN_FEW_MONSTERS: i32 = 0x3E8;
const HIDDEN_DEFAULT: i32 = 0x1F4;
/// 0x162D60: Pegasus's 0x139860 with the limit 0xBB8.
const RITUAL_ATK_LIMIT: i32 = 0xBB8;
/// Crush terrain (9).
const CRUSH: i32 = 9;

/// Yugi's module.
pub(super) struct Yugi;

impl Opponent for Yugi {
    const CONTROL: u8 = 19;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const EQUIP_SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x161670 (0x1344B0's 4 → 2) and 0x161C50 (Seto's 2).
    const DANGER_RADIUS: i32 = 2;
    const STATIC_DANGER_RADIUS: i32 = 3;
    const FACE_UP_SUPPORT_PASSES: bool = true;
    fn plan_alias(code: u8) -> u8 {
        match code {
            0xF | 0x10 => 6,
            c => c,
        }
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
    /// 0x162C60 = Pegasus's 0x139950.
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
    /// 0xC → 0x1624C0 (monsters pick with 0x163340, other cards with
    /// 0x1630A0), 0xD → 0x162770.
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
        code: u8,
    ) -> R<bool> {
        let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
        match code {
            0xC => {
                let monster = w.kind(g, s) >> 5 == 0;
                plan_c_with::<Yugi, _, _>(w, rec, side, d, which, true, |_| {
                    Ok(if monster {
                        graveyard_pick_with(w, side, true, id::GRAVE_FOR_MONSTERS)
                    } else {
                        graveyard_pick_with(w, side, false, id::GRAVE_FOR_SUPPORT)
                    })
                })?;
            }
            0xD => plan_d::<Yugi, _, _>(w, rec, side, d, which)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// 0x162E50(side): 2000 while the side's LP is below 1201; else 1000 with
/// fewer than four monsters in its slots 1..=5, else 500.
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

/// 0x15E1B0(rec): Mako's 0x14C6B0 shape: the leader (kept while the +8
/// word is not −1): threatened → 0x50A / 0x5FF, else 0xA05 / 0x5FF by the
/// summon flag; classes 4, 3, 1 → [`support`]; 2 → 0x27E390(rec, 5, 0,
/// 0x162E50(controller) & 0xFF); 0 → [`effect_monster`] with an effect
/// entry, else [`planner`]; others 0x5FF.
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
        4 | 3 | 1 => support(w, rec)?,
        2 => plans::power_up(w, rec, 5, 0, hidden_value(w, ctrl) & 0xFF)?,
        0 => {
            if has_effect(w, g, s) == 1 {
                effect_monster(w, rec)?;
            } else {
                planner(w, rec)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x15E7A0(rec): ATK 2500 or more → [`rush`]; else row 3 clear
/// (0x15ED10) → patrol on row 3; else [`lane_cell`] → plan 6 to it; else
/// row 4 clear → patrol on row 4; else rush.
fn planner<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return Ok(());
    }
    if w.atk(g, s, false)? >= RUSH_ATK {
        return rush(w, rec);
    }
    if row_clear(w, g, s, 1) == 1 {
        return patrol_with(w, rec, 1, hidden_value::<C, F>);
    }
    let v = lane_cell(w, g, s)?;
    if v >= 0 {
        rec.set_plans(0x6FF);
        set_target(rec, v);
        return Ok(());
    }
    if row_clear(w, g, s, 2) == 1 {
        return patrol_with(w, rec, 2, hidden_value::<C, F>);
    }
    rush(w, rec)
}

/// 0x15E900(rec): the generic rush (0x16E670) with 0x162E50; exhausted →
/// plan 6 to the enemy leader's cell when the unit and that leader are on
/// the same half (rows 0..=3 / 4..=6), else to (own column, row 3).
fn rush<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    passes(
        w,
        rec,
        RUSH_MOVES,
        RUSH_ROUNDS,
        RUSH_RADIUS,
        hidden_value::<C, F>,
        |_w, rec, my, lp| {
            rec.set_plans(0x6FF);
            if ((my >> 8) < HALF_ROW) == ((lp >> 8) < HALF_ROW) {
                set_target(rec, lp);
            } else {
                set_target(rec, FRONT_ROW << 8 | (my & 0xFF));
            }
            Ok(())
        },
    )
}

/// 0x15EBB0(g, s): with an own card 0x33B on the field (rel 1): (0, 5) as
/// 0x500 unless another unit of the same controller stands there (the
/// unit itself or an enemy there also give 0x500); then with an own 0x2ED:
/// (6, 3) as 0x306 likewise; −1.
fn lane_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, s: i32) -> R<i32> {
    let ctrl = w.ctrl(g, s);
    let me = g << 8 | s;
    for (card, col, row) in [(id::CARD_33B, 0, 5), (id::CARD_2ED, 6, 3)] {
        let word = (card as u32) << 16 | 0x1C;
        if unit_mask(w, 0, 0, ctrl, 1, word, 0xE)? == 0 {
            continue;
        }
        let k = w.at_key(col, row);
        if k < 0 || k == me || w.ctrl(k >> 8, k & 0xFF) != ctrl {
            return Ok(row << 8 | col);
        }
    }
    Ok(-1)
}

/// 0x15F220(rec): monsters with an effect entry: card 0x129 → patrol on
/// row 3 while it is clear, else hunt rounds 1..=2 (0x278A20 kept tree,
/// 0x278AA0 bound 2, filter 0x34, face 0, def 2, see-all 0) → plan 2 on a
/// pick, else [`hold_formation`]; 0x57 → [`rush`]; 0x2B → plan 0xC in
/// place unless committed face-up (then [`planner`]); others [`planner`].
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let card = w.card_id(g, s);
    let Some(u) = w.unit(g, s) else {
        return planner(w, rec);
    };
    let my = w.pos(g, s);
    match card {
        id::CARD_129 => {
            if row_clear(w, g, s, 1) == 1 {
                return patrol_with(w, rec, 1, hidden_value::<C, F>);
            }
            let (c, r) = (my & 0xFF, my >> 8);
            let mut list = Targets::new();
            let mut pool = Pool::new();
            let mut root = None;
            let other = i32::from(ctrl == 0);
            for n in 1..=HUNT_ROUNDS {
                root = search::reach(
                    w,
                    &mut pool,
                    &mut list,
                    root,
                    Some(u),
                    c,
                    r,
                    other,
                    n,
                    0,
                    hidden_value(w, ctrl),
                )?;
                let k = search::pick_target(
                    w,
                    u,
                    c,
                    r,
                    &list,
                    HUNT_BOUND,
                    HUNT_FILTER,
                    0,
                    2,
                    0,
                    hidden_value(w, ctrl),
                    i32::from(list.count),
                )?;
                if k >= 0 {
                    rec.set_plans(0x2FF);
                    set_target(rec, k);
                    return Ok(());
                }
            }
            hold_formation(w, g, s, rec)
        }
        id::CARD_57 => rush(w, rec),
        id::CARD_2B => {
            if w.flag(g, s, 4) {
                return planner(w, rec);
            }
            rec.set_plans(0xCFF);
            set_target(rec, my);
            Ok(())
        }
        _ => planner(w, rec),
    }
}

/// 0x15F660(i): the formation: the active side's leader's neighbours 0, 3,
/// 4 and 5 (0x24FD30 raw words), −1 past them.
fn formation_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, i: i32) -> R<i32> {
    let side = i32::from(w.active);
    match i {
        0 => neighbour_raw(w, side, 0),
        1 => neighbour_raw(w, side, 3),
        2 => neighbour_raw(w, side, 4),
        3 => neighbour_raw(w, side, 5),
        _ => Ok(-1),
    }
}

/// 0x15F530(g, s, rec): plan 3 in place (whether or not 0x15F600 finds the
/// own cell in the formation); then the first formation cell holding no
/// unit (0x15F710; an off-board word counts) → plan 3 to it when not
/// negative, else [`planner`].
fn hold_formation<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    rec: &mut Record,
) -> R<()> {
    let my = w.pos(g, s);
    rec.set_plans(0x3FF);
    set_target(rec, my);
    let mut free = -1;
    for i in 0..4 {
        let v = formation_cell(w, i)?;
        if w.at(v & 0xFF, v >> 8).is_none() {
            free = v;
            break;
        }
    }
    if free >= 0 {
        rec.set_plans(0x3FF);
        set_target(rec, free);
        return Ok(());
    }
    planner(w, rec)
}

/// 0x15F810(rec): classes 4, 3, 1 (entry 0 by 0x252060; none → 0x5FF).
/// Classes 4 / 3: card 0x2ED → plan 6 to (0, 6); else row 3 clear → patrol
/// on row 3; [`lane_cell`] → plan 6 to it; row 4 clear → patrol on row 4;
/// 0x27F060 planned → 0xBFF; else 0x5FF. Magic by entry 0's type: 0x1F →
/// 0xDFF for Change of Heart, else 0x5FF; 0x25 → card 0x2ED: plan 6 to
/// (6, 4); else 0xBFF when 0x27F060 planned, else 0x5FF; 4 → [`threat`] →
/// plan 6 to (its column, own row), else 0x5FF; 0x14 → 0xCFF; others
/// 0x5FF.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let class = w.kind(g, s) >> 5;
    let card = w.card_id(g, s);
    let ctrl = w.ctrl(g, s);
    let entry = w.unit(g, s).and_then(|u| u.entry(0));
    let my = w.pos(g, s);
    let Some([w0, _]) = entry else {
        rec.set_plans(0x5FF);
        return Ok(());
    };
    let strongest = |rec: &mut Record| -> R<()> {
        if plans::strongest_enemy(w, rec, 0)? == 1 {
            rec.set_plans(0xBFF);
        } else {
            rec.set_plans(0x5FF);
        }
        Ok(())
    };
    match class {
        4 | 3 => {
            if card == id::CARD_2ED {
                rec.set_plans(0x6FF);
                set_target(rec, 0x600);
                return Ok(());
            }
            if row_clear(w, g, s, 1) == 1 {
                return patrol_with(w, rec, 1, hidden_value::<C, F>);
            }
            let v = lane_cell(w, g, s)?;
            if v >= 0 {
                rec.set_plans(0x6FF);
                set_target(rec, v);
                return Ok(());
            }
            if row_clear(w, g, s, 2) == 1 {
                return patrol_with(w, rec, 2, hidden_value::<C, F>);
            }
            strongest(rec)
        }
        1 => match w0 >> 8 {
            0x1F => {
                rec.set_plans(if card == id::CHANGE_OF_HEART { 0xDFF } else { 0x5FF });
                Ok(())
            }
            0x25 => {
                if card == id::CARD_2ED {
                    rec.set_plans(0x6FF);
                    set_target(rec, 0x406);
                    return Ok(());
                }
                strongest(rec)
            }
            4 => {
                let v = threat(w, ctrl)?;
                if v < 0 {
                    rec.set_plans(0x5FF);
                } else {
                    rec.set_plans(0x6FF);
                    let p = w.pos(v >> 8, v & 0xFF);
                    set_target(rec, (my >> 8) << 8 | (p & 0xFF));
                }
                Ok(())
            }
            0x14 => {
                rec.set_plans(0xCFF);
                Ok(())
            }
            _ => {
                rec.set_plans(0x5FF);
                Ok(())
            }
        },
        _ => {
            rec.set_plans(0x5FF);
            Ok(())
        }
    }
}

/// 0x162EB0(side): York's 0x13DB90 with swapped margin arguments: the
/// strongest visible enemy non-leader (0x27CF90 over 0x27DC00(other,
/// 0x1D, 0, 0)) when the side's strongest (over 0x27D9B0(side, 0x1D, 1))
/// has a NEGATIVE margin (0x279B30) against it at its cell; −1 otherwise.
pub(super) fn threat<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<i32> {
    let other = i32::from(side == 0);
    let enemy = super::keith::pick_strongest(w, &w.list_seen(other, 0x1D, false, false))?;
    let own = super::keith::pick_strongest(w, &w.list(side, 0x1D, true))?;
    if enemy < 0 || own < 0 {
        return Ok(-1);
    }
    let p = w.pos(enemy >> 8, enemy & 0xFF);
    Ok(if w.margin_k(own, enemy, p & 0xFF, p >> 8)? < 0 {
        enemy
    } else {
        -1
    })
}

/// 0x162FC0(side): 1 when the side's leader is threatened (0x27D500,
/// see-all 0) or five or more enemy units (rel 2, filter 0x34) are not
/// committed face-up; else 0.
pub(super) fn crowded_enemy<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<i32> {
    if w.threatened(side, 0, false)? {
        return Ok(1);
    }
    let mask = unit_mask(w, 0, 0, side, 2, 0x34, 0xE)?;
    let n = (0..popcount(mask))
        .filter(|&i| {
            let k = nth_unit(mask, i);
            !w.flag(k >> 8, k & 0xFF, 4)
        })
        .count();
    Ok(i32::from(n >= 5))
}

/// 0x162D60(side, card): Pegasus's 0x139860 with the limit 3000.
pub(super) fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    ritual_ok_with(w, side, card, RITUAL_ATK_LIMIT, false)
}

/// 0x162980(g, s): the reveal request. 0 once committed face-up. Magic: 2
/// under a type-0xE far trap or without an entry; type 0x1F → Brain
/// Control with Seto's control target word equal to 1 (quirk: `bne v0,
/// 1`); 0x25 → card 0x2ED with the group's leader threatened, card 0x2EA
/// with itself threatened; 0x49 → [`crowded_enemy`]; 4 → [`threat`]'s cell
/// shares the card's column or row; others 2. Rituals: 2 under a far trap;
/// 1 when [`ritual_ok`] and the own cell is not Crush. Others 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    let pos = w.pos(g, s);
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
                0x1F => yes(card == id::BRAIN_CONTROL && control_target(w, ctrl)? == 1),
                0x25 => {
                    if card == id::CARD_2ED && w.threatened(g, 0, false)? {
                        1
                    } else {
                        yes(card == id::CARD_2EA && w.threatened(g, s, false)?)
                    }
                }
                0x49 => yes(crowded_enemy(w, ctrl)? == 1),
                4 => {
                    let v = threat(w, ctrl)?;
                    let p = w.pos(v >> 8, v & 0xFF);
                    yes(pos & 0xFF == p & 0xFF || pos >> 8 == p >> 8)
                }
                _ => 2,
            }
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            yes(ritual_ok(w, ctrl, card)? == 1 && w.terrain(pos & 0xFF, pos >> 8) != CRUSH)
        }
        _ => 2,
    })
}

#[cfg(test)]
mod tests;
