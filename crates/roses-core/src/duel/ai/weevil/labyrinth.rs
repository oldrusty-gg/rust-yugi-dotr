//! Labyrinth-ruler's controller (opponent 8, control byte 8, driver
//! 0x135930 = Weevil's 0x1117D0 by opcode shape, module 0x131930..0x135AF8,
//! 26 functions). The module is Weevil's template with Pegasus's actor
//! choice, summon subtree and executors, its own constants and seven novel
//! functions; the shared parts (dispatch 0x131930, actor choice 0x132020,
//! executors 0x134A90 / 0x1344B0 / 0x133F70, the 0x27xxxx helpers) run
//! through the framework in [`super`] with the constants and hooks of
//! [`Labyrinth`].
//!
//! Own functions, by EE address: [`hidden_value`] 0x1353B0 (the strength
//! assumed for hidden units, which Weevil's 0x10F670 always answers 0
//! for), [`assign`] 0x131E10, [`monster`] 0x132400 (0x1326B0: 0x27F760
//! with limit 0x708), [`after_passes`] 0x132710, [`goal`] 0x132850 (the
//! cell a unit heads for on the Labyrinth board), [`support`] 0x132B50
//! (magic and trap cards), [`labyrinth_cell`] 0x135270, [`reveal`]
//! 0x135410, [`ritual_ok`] 0x1358D0 and the summon planner in [`summon`]
//! (0x132D30 and its subtree). The charge test 0x1357D0 is Pegasus's
//! 0x139950 by opcode shape and shared with it.

mod summon;

use super::plans::{self, set_plan, set_target};
use super::search::{self, attack_path, Pool, Steps, Targets};
use super::summon::{ritual_spec, tributes, virt};
use super::world::{Unit, World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// Jirai Gumo (0x132400: defence when its controller's LP is low).
    pub const JIRAI_GUMO: i32 = 0x198;
    /// Monster Tamer, the leader card (0x135410: a copy on the field
    /// reveals with three or more own Insects).
    pub const MONSTER_TAMER: i32 = 0xBE;
    /// Gate Guardian (0x132FC0 / 0x135410).
    pub const GATE_GUARDIAN: i32 = 0x91;
    /// Gate Guardian Ritual (0x132FC0).
    pub const GATE_GUARDIAN_RITUAL: i32 = 0x340;
    /// Shift (0x132FC0: summoned while a Gate Guardian is out).
    pub const SHIFT: i32 = 0x2EA;
}

/// 0x1326B0: the ATK limit of 0x27F760's first pass (0x708 = 1800).
const TRAP_PASS_ATK_LIMIT: i32 = 0x708;
/// 0x132400: Jirai Gumo's LP test and 0x1353B0's (`slti 0x3E9`).
const LOW_LP: i32 = 0x3E9;
/// 0x1353B0's answers: LP below 1001; else fewer than four monsters; else.
const HIDDEN_LOW_LP: i32 = 0x6A4;
const HIDDEN_FEW_MONSTERS: i32 = 0x5DC;
const HIDDEN_DEFAULT: i32 = 0x2EE;
/// 0x132850: the own ATK limit (`slti 0x5DC`) of the column-0 and (6, 4)
/// goals.
const GOAL_ATK_LIMIT: i32 = 0x5DC;
/// 0x132850: the route limit of its 0x27BA70 searches.
const GOAL_ROUTE_LIMIT: i32 = 7;
/// 0x133BF0: the route limit of the summon cell's searches.
const SUMMON_ROUTE_LIMIT: i32 = 4;
/// Insect (kind 9), counted by 0x135410 for Monster Tamer.
const INSECT: i32 = 9;
/// The Labyrinth terrain (8) and Crush (9).
const LABYRINTH: i32 = 8;
const CRUSH: i32 = 9;

/// Labyrinth-ruler's module.
pub(super) struct Labyrinth;

impl Opponent for Labyrinth {
    const CONTROL: u8 = 8;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    const DANGER_RADIUS: i32 = 4;
    /// 0x13211C (as Pegasus's 0x13633C): class 3 only.
    fn trap_actor(class: i32) -> bool {
        class == 3
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
    /// 0x1357D0 = Pegasus's 0x139950.
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

/// 0x1353B0(side): the strength assumed for hidden units: 0x6A4 (1700)
/// while the side's LP (0x1E06F0) is below 1001; else 0x5DC (1500) with
/// fewer than four monsters in its slots 1..=5 (0x1E13D0), else 0x2EE
/// (750). The call sites pass it whole (0x27E960's search strength, the
/// plan passes of 0x132400), `& 0xFFFF` (the searches and dangers of
/// 0x132850, 0x133BF0, 0x133F70, 0x1344B0) or `& 0xFF` (the see-all byte
/// of the danger queries in 0x134A90 / 0x1344B0 and 0x27E390's strength).
fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if w.lp(side) < LOW_LP {
        return Ok(HIDDEN_LOW_LP);
    }
    Ok(if w.monsters(side) < 4 {
        HIDDEN_FEW_MONSTERS
    } else {
        HIDDEN_DEFAULT
    })
}

/// 0x131E10(rec): (re)assign the plans of a record (no mode, no side
/// struct). Missing, acted or free records are left alone. The leader
/// (kept while the +8 word is not −1): threatened (0x27D500, see-all 0)
/// → 0x50A / 0x5FF by the summon flag; else 0xA06 with the enemy leader's
/// COLUMN (0x1E2310(other, 0) & 0xFF, read as a cell in row 0) as the
/// walking target, or 0x6FF on that cell without the summon flag.
/// Non-leaders by class: 3 and 1 → [`support`]; 2 → 0x27E390(rec, 4, 0,
/// 0x1353B0(controller) & 0xFF); 0 → [`monster`](self::monster)(rec, 1);
/// 4, 5 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    // 0x251C80(g, s): the controller, used by the leader and class 2.
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
        let other = i32::from(ctrl == 0);
        let column = (w.pos(other, 0) & 0xFF) as u16;
        if summon {
            rec.set_plans(0xA06);
            rec.target_b = column;
        } else {
            rec.set_plans(0x6FF);
            rec.target_a = column;
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        3 | 1 => support(w, rec)?,
        2 => {
            let hv = hidden_value(w, ctrl)?;
            plans::power_up(w, rec, 4, 0, hv & 0xFF)?;
        }
        0 => monster(w, rec, 1)?,
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x132400(rec, a1): monsters. Jirai Gumo with its controller's LP below
/// 1001 first gets plan 3 at its own cell (overwritten below unless
/// nothing else plans). `a1` ≠ 0 and 0x27CB40 reaches the enemy leader
/// within 4 moves: plan 2 on its key. Else N = 1..=4: 0x1326B0 → 0x27F760
/// (radius 4, see-all 0, hidden strength 0x1353B0(controller), limit
/// 0x708): 1 → planned; 0 → plan 5; 2 → plan 3 at the own cell; 3 → plan
/// 6 at the own cell; else next N (the reachable list and the node pool
/// are shared by all rounds, as in 0x10DD60). Exhausted →
/// [`after_passes`].
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
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
    // sp+0x140: the reachable list (key 0xFFFF, level 0xFF), once.
    let mut list = Targets::new();
    // 0x2800B0(sp+0xD0): a no-op.
    if w.card_id(g, s) == id::JIRAI_GUMO && w.lp(ctrl) < LOW_LP {
        rec.set_plans(0x3FF);
        set_target(rec, my);
    }
    if a1 != 0 {
        let v = search::moves_to_k(w, g, s, my & 0xFF, my >> 8, lg, ls, lp & 0xFF, lp >> 8, 4)?;
        if v > 0 {
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // sp+0x19C / sp+0x19E: the pool counters, zeroed once.
    let mut pool = Pool::new();
    for n in 1..=4 {
        let side = i32::from(w.ctrl(g, s) == 0);
        let hidden = hidden_value(w, ctrl)?;
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
            4,
            0,
            TRAP_PASS_ATK_LIMIT,
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
    after_passes(w, rec)
}

/// 0x132710(rec): a monster no pass planned for. With an effect entry 1 of
/// type 8 (0x1E1DF0(g, s, 1), word 0 >> 8): plan 2 on the enemy leader's
/// key. Else plan 6 to the unit's [`goal`], or plan 5 when it has none.
fn after_passes<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let pos = w.pos(g, s);
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    if u.entry_type(1) == 8 {
        set_plan(rec, 2);
        set_target(rec, i32::from(ctrl == 0) << 8);
        return Ok(());
    }
    set_plan(rec, 6);
    goal_target(w, rec, u, pos)
}

/// The tail of 0x132710 / 0x132B50: the primary target becomes the unit's
/// [`goal`] from `pos`, or the plan becomes 5 when there is none.
fn goal_target<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    u: &Unit,
    pos: i32,
) -> R<()> {
    let v = goal(w, u, pos & 0xFF, pos >> 8)?;
    if v < 0 {
        set_plan(rec, 5);
    } else {
        set_target(rec, v);
    }
    Ok(())
}

/// 0x132850(u, c, r): the cell a unit at (c, r) heads for, (row << 8) |
/// col. Three candidates in turn, each taken when 0x27BA70 (see-all 0, at
/// most 7 moves, hidden strength 0x1353B0(controller) & 0xFFFF, no
/// enterable target; one step buffer for every search) finds a route:
/// 1. columns 3..6 in rows 0, 1, 4, 5: column 6, row + 1 (odd rows) or
///    row + 2 (even rows); columns 0..2 in rows 2, 3: (col 0, row 4);
/// 2. columns 0..2 in rows 0, 1, 4, 5 when the unit's ATK at (col 0, row
///    rounded up to even) is below 1500: column 0, the row as in 1;
///    columns 3..6 in row 2 when its ATK at (6, 3) is below 1500: (6, 4)
///    (quirk: a failed first candidate is searched again here);
/// 3. the enemy leader's cell.
///
/// Without a route to any: the unit's own column in the enemy leader's
/// row (a negative value when that leader is missing). −1 without a unit.
fn goal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    c: i32,
    r: i32,
) -> R<i32> {
    // 0x251C90(u).
    let ctrl = u.ctrl();
    // sp+0x80: the step buffer, filled once and never reset.
    let mut buf = Steps::new();
    let search = |w: &W<C, F>, buf: &mut Steps, t: i32| -> R<bool> {
        let hidden = hidden_value(w, ctrl)? & 0xFFFF;
        let v = attack_path(
            w,
            Some(u),
            buf,
            c,
            r,
            t & 0xFF,
            t >> 8,
            u32::MAX,
            0,
            GOAL_ROUTE_LIMIT,
            hidden,
        )?;
        Ok(v >= 0)
    };
    let side_row = |r: i32| if r & 1 != 0 { r + 1 } else { r + 2 };
    let edge_row = (0..2).contains(&r) || (4..6).contains(&r);
    let mut t = -1;
    if c >= 3 {
        if edge_row {
            t = side_row(r) << 8 | 6;
        }
    } else if (2..4).contains(&r) {
        t = 0x400;
    }
    if t >= 0 && search(w, &mut buf, t)? {
        return Ok(t);
    }
    if c < 3 {
        if edge_row {
            let probe = if r & 1 == 0 { r } else { r + 1 };
            if w.eff_atk_of(u, 0, probe)? < GOAL_ATK_LIMIT {
                t = side_row(r) << 8;
            }
        }
    } else if r == 2 && w.eff_atk_of(u, 6, 3)? < GOAL_ATK_LIMIT {
        t = 0x406;
    }
    if t >= 0 && search(w, &mut buf, t)? {
        return Ok(t);
    }
    let other = i32::from(ctrl == 0);
    t = w.pos(other, 0);
    if t >= 0 && search(w, &mut buf, t)? {
        return Ok(t);
    }
    Ok(((t >> 8) << 8) | c)
}

/// 0x132B50(rec): magic (class 1) and trap (class 3) cards. Traps: plan 3
/// to the card's [`goal`] (plan 5 without one). Magic by entry 0's type:
/// 0x25 → as a trap; 0x48 and 2 → plan 6 to the [`labyrinth_cell`] of the
/// controller, or to (col 6, row 0) when there is none; others plan 5
/// (a magic card without an effect record is left alone). Other classes
/// plan 5.
fn support<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    let class = (w.kind(g, s) >> 5) & 0xFF;
    let pos = w.pos(g, s);
    match class {
        1 => {
            // 0x1E1DF0(g, s, 0) is null only without a unit.
            let Some(u) = w.unit(g, s) else {
                return Ok(());
            };
            match u.entry_type(0) {
                0x25 => {
                    set_plan(rec, 3);
                    goal_target(w, rec, u, pos)
                }
                0x48 | 2 => {
                    set_plan(rec, 6);
                    let v = labyrinth_cell(w, ctrl);
                    set_target(rec, if v < 0 { 6 } else { v });
                    Ok(())
                }
                _ => {
                    set_plan(rec, 5);
                    Ok(())
                }
            }
        }
        3 => {
            set_plan(rec, 3);
            let Some(u) = w.unit(g, s) else {
                set_plan(rec, 5);
                return Ok(());
            };
            goal_target(w, rec, u, pos)
        }
        _ => {
            set_plan(rec, 5);
            Ok(())
        }
    }
}

/// 0x135270(side): the first cell of rows 1, 3, 5 (columns 1..=5) that is
/// not Labyrinth terrain, (row << 8) | col; else (col 6, row 0) when a
/// unit `side` does not control stands in rows 0..=2 (columns scanned
/// first) and neither (6, 0) nor (5, 0) is Labyrinth; else −1.
fn labyrinth_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    for row in [1, 3, 5] {
        for col in 1..6 {
            if w.terrain(col, row) != LABYRINTH {
                return row << 8 | col;
            }
        }
    }
    for col in 0..7 {
        for row in 0..7 {
            let k = w.at_key(col, row);
            if k < 0 || w.ctrl(k >> 8, k & 0xFF) == side || row >= 3 {
                continue;
            }
            if w.terrain(6, 0) == LABYRINTH || w.terrain(5, 0) == LABYRINTH {
                continue;
            }
            return 6;
        }
    }
    -1
}

/// 0x1358D0(side, card): 1 when the ritual's tributes can be chosen
/// (0x2561B0 on the 0x2565B0 spec of its entry 0 word 1), without the
/// per-tribute tests of Weevil's 0x110210.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    // 0x2715C0(card, 0): entry 0 of a virtual record (null for 999).
    if card == 999 {
        return super::world::unsupported("0x1358D0 reads entry 0 of card 999");
    }
    let word1 = virt(w, card).entries[0].map_or(u32::MAX, |e| e[1]);
    Ok(i32::from(tributes(w, &ritual_spec(word1), side)? >= 0))
}

/// The counting loops of 0x135410 over slots 1..=5 of group `g` and of
/// the other group, in that order per slot: units controlled by `g` that
/// `hit` accepts, as a byte.
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

/// 0x135410(g, s): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters: a Monster Tamer reveals with three or more
/// own Insects (kind 9) in the groups' slots 1..=5, others 2. Rituals: 2
/// under a type-0xE far trap (0x27F170 mode 6, see-all 0), when the
/// tributes fail [`ritual_ok`] or on Crush; else 1. Magic: 2 under a
/// type-0xE trap or without an entry; by entry-0 type: 0x25 → 1 with any
/// own non-Gate-Guardian unit in the slots 1..=5 (the other group counted
/// when its unit exists), 0xD → 1, 0x48 and 2 → 1 when the card stands on
/// the group's [`labyrinth_cell`]; other types 2. Power-ups and traps 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    // 0x251C80(g, s) is called and its result dropped.
    let pos = w.pos(g, s);
    let entry0 = w.unit(g, s).and_then(|u| u.entry(0));
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    match w.kind(g, s) >> 5 {
        0 => {
            if card != id::MONSTER_TAMER {
                return Ok(2);
            }
            let insects = count_own(w, g, |grp, slot| w.kind(grp, slot) & 0x1F == INSECT);
            Ok(if insects < 3 { 2 } else { 1 })
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            if ritual_ok(w, w.ctrl(g, s), card)? != 1 {
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
            let Some([w0, _]) = entry0 else {
                return Ok(2);
            };
            match w0 >> 8 {
                0x25 => {
                    let n = count_own(w, g, |grp, slot| {
                        w.unit(grp, slot).is_some() && w.card_id(grp, slot) != id::GATE_GUARDIAN
                    });
                    Ok(if n > 0 { 1 } else { 2 })
                }
                0xD => Ok(1),
                0x48 | 2 => {
                    // Quirk: the group, not the controller.
                    let v = labyrinth_cell(w, g);
                    Ok(if v < 0 || v != pos { 2 } else { 1 })
                }
                _ => Ok(2),
            }
        }
        _ => Ok(2),
    }
}

#[cfg(test)]
mod tests;
