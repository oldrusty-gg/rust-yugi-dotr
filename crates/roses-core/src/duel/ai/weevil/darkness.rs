//! Darkness-ruler's controller (opponent 7, control byte 7, driver
//! 0x131760 = Weevil's 0x1117D0 by opcode shape, module 0x12C440..0x131928,
//! 29 functions). The module is Weevil's template with Pegasus's actor
//! choice (0x12CCC0), Pegasus's charge test (0x131480), the non-monster
//! and monster choices of the summon subtree (0x12E4B0 / 0x12E6B0), a
//! mode nibble like Weevil's (0x131580, kept in side +0x1A1), the
//! Labyrinth-ruler's hidden-strength function shape (0x131050) and eleven
//! novel functions; the shared parts (dispatch 0x12C440, actor choice,
//! executors 0x130200 / 0x12FC20 / 0x12F0F0 / 0x27E960, the 0x27xxxx
//! helpers) run through the framework in [`super`] with the constants and
//! hooks of [`Darkness`].
//!
//! Own functions, by EE address: [`mode`] 0x131580, [`hidden_value`]
//! 0x131050, [`assign`] 0x12CA70, [`monster`] 0x12D0C0 (0x12D450:
//! 0x27F760 with limit 0x9C4), [`effect_monster`] 0x12D4B0, [`trap`]
//! 0x12D720 with the shared-library pickers [`pick_visible`] 0x279610 and
//! [`pick_near`] 0x278E30, [`reveal`] 0x131210, [`ritual_ok`] 0x1310B0,
//! [`graveyard_pick`] 0x130F30, the plan-0xB executor [`trap_move`]
//! 0x1309F0, the plan-0xC executor [`plan_c`] 0x12F640, and the summon
//! planner in [`summon`] (0x12DB40 and its subtree).
//!
//! Plan 0xB (every trap card) walks the trap towards an enemy unit, or
//! towards an own unit when none is in reach, and keeps its target; plan
//! 0xC (card 0x131 before it is committed face-up) marks a face-up card
//! acted ([`super::Plan::MarkActed`]) or retreats, as its graveyard pick
//! 0x130F30 never finds a card (it requires a card without a record).

mod summon;

use super::exec;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Steps, Targets};
use super::summon::{nth_unit, popcount, ritual_spec, tributes, virt};
use super::world::{unsupported, World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// Card 0x12A (0x12D4B0: a trap-pass monster; 0x131210: always
    /// reveals; 0x1310B0: never a tribute).
    pub const CARD_12A: i32 = 0x12A;
    /// Card 0x131 (0x12D4B0: plan 0xC while set; 0x1310B0: a tribute only
    /// once committed face-up; 0x12DDF0: summoned with a graveyard pick).
    pub const CARD_131: i32 = 0x131;
    /// Card 0x142 (0x12D4B0: a trap-pass monster; 0x131210: reveals in
    /// mode 1).
    pub const CARD_142: i32 = 0x142;
    /// Card 0x146 (0x12D4B0: plan 3 in place; 0x131210: reveals with four
    /// or more enemy-controlled units; 0x1310B0: never a tribute; 0x12DDF0:
    /// summoned in mode 1).
    pub const CARD_146: i32 = 0x146;
    /// The ritual 0x342 (0x12DDF0).
    pub const RITUAL_342: i32 = 0x342;
    /// Card 0x137 (a monster), Paralyzing Potion 0x31B and card 0x2AC (a
    /// magic card): summoned by 0x12E230 when affordable.
    pub const CARD_137: i32 = 0x137;
    pub const POTION: i32 = 0x31B;
    pub const CARD_2AC: i32 = 0x2AC;
}

/// 0x12D450 / 0x12D4B0: the ATK limit of 0x27F760's first pass (0x9C4 =
/// 2500).
const TRAP_PASS_ATK_LIMIT: i32 = 0x9C4;
/// 0x131050: the LP test (`slti 0x7D1`) and its answers.
const LOW_LP: i32 = 0x7D1;
const HIDDEN_LOW_LP: i32 = 0x960;
const HIDDEN_FEW_MONSTERS: i32 = 0x5DC;
const HIDDEN_DEFAULT: i32 = 0x2EE;
/// 0x1310B0 / 0x131210: the tribute ATK limit (`slti 0xBB8` = 3000).
const RITUAL_ATK_LIMIT: i32 = 0xBB8;
/// 0x12D720: the Manhattan bound of its pickers.
const TRAP_PICK_BOUND: i32 = 5;
/// 0x1309F0: the route limit of its 0x27B9D0 search and the radius of its
/// danger scores.
const TRAP_ROUTE_LIMIT: i32 = 0x1E;
const TRAP_DANGER_RADIUS: i32 = 3;
/// The byte 0x1309F0 reads at sp+0x1DF (past its 12-byte step buffer at
/// sp+0xE0) when the first candidates score 0: the "previous best" of its
/// tie-break. Through the dispatch's frame (0x70) and the driver's (0x50)
/// it is byte 0x2F of the driver's caller 0x1CFF80, the last byte of the
/// `sq $s2, 32($sp)` slot, i.e. bits 120..127 of that register as saved
/// (an ancestor's value; those bits are written by 128-bit moves only
/// and are 0 in practice). The oracle runs with 0 there (a fresh emulator
/// stack); no savestate keeps the frame to read the real value [H].
/// `None` would refuse the decision instead.
const TRAP_MOVE_STACK_BYTE: Option<u8> = Some(0);

/// Darkness-ruler's module.
pub(super) struct Darkness;

impl Opponent for Darkness {
    const CONTROL: u8 = 7;
    const DANGER_HIDDEN: i32 = 1;
    const STANCE_SEE_ALL: bool = false;
    const SEARCH_SEE_ALL: i32 = 0;
    const RETREAT_SEE_ALL: bool = false;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 0;
    /// 0x130200 / 0x12FC20: radius 3.
    const DANGER_RADIUS: i32 = 3;
    /// 0x12CDBC (as Pegasus's 0x13633C): class 3 only.
    fn trap_actor(class: i32) -> bool {
        class == 3
    }
    fn hidden_value<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
    ) -> R<i32> {
        hidden_value(w, side)
    }
    /// 0x12C470: +0x1A1 = (+0x1A1 & 0xF0) | 0x131580().
    fn begin<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, mem: &mut Memory) -> R<()> {
        mem.x1a1 = (mem.x1a1 & 0xF0) | (mode(w) & 0xF);
        Ok(())
    }
    /// 0x12CA70(rec, +0x1A1 & 0xF).
    fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        mem: &mut Memory,
        idx: usize,
    ) -> R<()> {
        let m = mem.x1a1 & 0xF;
        assign(w, &mut mem.records[idx], m)
    }
    /// 0x12DB40(key, side, d, 0x131580()).
    fn summon<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        key: i32,
        side: i32,
        d: &mut Decision,
    ) -> R<i32> {
        summon::summon(w, key, side, d, mode(w))
    }
    /// 0x131210(g, s, rec, +0x1A1 & 0xF).
    fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        rec: &Record,
    ) -> R<u32> {
        reveal(w, g, s, rec, mode(w))
    }
    /// 0x131480 = Pegasus's 0x139950.
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
    /// 0x12C744 / 0x12C990: 0xB → 0x1309F0; 0x12C768 / 0x12C9E8: 0xC →
    /// 0x12F640. Both count as executed whatever they return.
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
                trap_move::<Darkness, _, _>(w, rec, side, d, which, TRAP_DANGER_RADIUS)?;
            }
            0xC => plan_c(w, rec, side, d, which)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// 0x131580(): the mode nibble. Over the slots 0..=10 of the active side's
/// group and of the other group (that order per slot): N = units that are
/// spellbound (0x1E2800 > 0), have +0x4A (0x1E28C0 > 0) or are not
/// controlled by the active side (0x251C80; an empty slot counts as its
/// group's); E = slots not controlled by the active side; O = slots it
/// controls. 1 when N < 4, E < 2 and O ≥ 4; else 0. The other group's
/// empty slots count for N and E, so the answer is 0 unless control has
/// changed hands.
fn mode<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> u8 {
    let side = i32::from(w.active);
    let other = i32::from(side == 0);
    let mut n = 0u8;
    for slot in 0..11 {
        for grp in [side, other] {
            if w.spellbind(grp, slot) > 0 || w.x4a(grp, slot) > 0 || w.ctrl(grp, slot) != side {
                n = n.wrapping_add(1);
            }
        }
    }
    if n >= 4 {
        return 0;
    }
    let mut e = 0u8;
    for slot in 0..11 {
        for grp in [side, other] {
            if w.ctrl(grp, slot) != side {
                e = e.wrapping_add(1);
            }
        }
    }
    let mut o = 0u8;
    for slot in 0..11 {
        for grp in [side, other] {
            if w.ctrl(grp, slot) == side {
                o = o.wrapping_add(1);
            }
        }
    }
    if e >= 2 {
        return 0;
    }
    u8::from(o >= 4)
}

/// 0x131050(side): the strength assumed for hidden units: 0x960 (2400)
/// while the side's LP (0x1E06F0) is below 2001; else 0x5DC (1500) with
/// fewer than four monsters in its slots 1..=5 (0x1E13D0), else 0x2EE
/// (750). Passed whole (0x27E960's search strength, the plan passes, the
/// summon target search), `& 0xFFFF` (the executors' route searches and
/// the retreat's dangers) or `& 0xFF` (the see-all byte of the executors'
/// dangers, 0x27E390's strength).
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

/// 0x2520F0: −1 for non-monsters, 1 when any effect entry is present.
fn has_effect<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, g: i32, s: i32) -> i32 {
    if w.kind(g, s) >> 5 != 0 {
        return -1;
    }
    match w.unit(g, s) {
        Some(u) if u.entries.iter().any(Option::is_some) => 1,
        _ => 0,
    }
}

/// 0x12CA70(rec, mode): (re)assign the plans of a record. Missing, acted
/// or free records are left alone. The leader (kept while the +8 word is
/// not −1): threatened (0x27D500, see-all 0), or standing in a row of 2 or
/// more → 0x50A / 0x5FF by the summon flag; in rows 0..1 → 0x60A / 0x6FF
/// with the enemy leader's cell (0x1E2310(other, 0), other = the
/// controller's opponent) as the walking target. Non-leaders by class: 3 →
/// [`trap`]; 2 → 0x27E390(rec, 5, 0, 0x131050(controller) & 0xFF); 0 →
/// [`effect_monster`](self::effect_monster)(rec, mode) with an effect
/// entry (0x2520F0 = 1), else [`monster`](self::monster)(rec, 1, mode);
/// 1, 4, 5 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: u8,
) -> R<()> {
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
        let threatened = w.threatened(g, 0, false)?;
        if !threatened && w.pos(g, s) >> 8 < 2 {
            let other = i32::from(ctrl == 0);
            rec.set_plans(if summon { 0x60A } else { 0x6FF });
            set_target(rec, w.pos(other, 0));
        } else {
            rec.set_plans(if summon { 0x50A } else { 0x5FF });
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        3 => trap(w, rec)?,
        2 => {
            let hv = hidden_value(w, ctrl)?;
            plans::power_up(w, rec, 5, 0, hv & 0xFF)?;
        }
        0 => {
            if has_effect(w, g, s) == 1 {
                effect_monster(w, rec, mode)?;
            } else {
                monster(w, rec, 1, mode)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x12D0C0(rec, a1, mode): monsters without effects. `a1` ≠ 0 and
/// 0x27CB40 reaches the enemy leader within 5 moves (mode 1) or 2: plan 2
/// on its key. Else N = 1..=2 with radius 2 (mode 0) or 1..=4 with radius
/// 5: 0x12D450 → 0x27F760 (see-all 0, hidden strength
/// 0x131050(controller), limit 0x9C4): 1 → planned; 0 → plan 5; 2 → plan
/// 3 at the own cell; 3 → plan 6 at the own cell; else next N (the
/// reachable list and the node pool are shared by all rounds, as in
/// 0x10DD60). Exhausted → plan 5.
fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
    mode: u8,
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
    // 0x2800B0(sp+0xE0): a no-op; sp+0x150: the reachable list, once.
    let mut list = Targets::new();
    if a1 != 0 {
        let moves = if mode == 1 { 5 } else { 2 };
        let v = search::moves_to_k(w, g, s, my & 0xFF, my >> 8, lg, ls, lp & 0xFF, lp >> 8, moves)?;
        if v > 0 {
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // sp+0x1AC / sp+0x1AE: the pool counters, zeroed once.
    let mut pool = Pool::new();
    let (last, radius) = if mode == 0 { (2, 2) } else { (4, 5) };
    for n in 1..=last {
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
            radius,
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
    set_plan(rec, 5);
    Ok(())
}

/// 0x12D4B0(rec, mode): monsters with an effect entry, by card: 0x146 →
/// plan 3 at the own cell; 0x131 → plan 0xC at the own cell while not
/// committed face-up, else [`monster`](self::monster)(rec, 1, mode);
/// 0x142 and 0x12A → N = 1..=2 of 0x27F760 (radius 2, see-all 0, hidden
/// strength 0x131050(controller), limit 0x9C4): 1 → planned, 0 / 2 / 3 →
/// plan 3 at the own cell, else next N; exhausted → plan 3 at the own
/// cell; others → [`monster`](self::monster)(rec, 1, mode).
fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    mode: u8,
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
        id::CARD_146 => {
            rec.set_plans(0x3FF);
            set_target(rec, my);
            Ok(())
        }
        id::CARD_131 => {
            if w.flag(g, s, 4) {
                return monster(w, rec, 1, mode);
            }
            rec.set_plans(0xCFF);
            set_target(rec, my);
            Ok(())
        }
        id::CARD_142 | id::CARD_12A => {
            for n in 1..=2 {
                let side = i32::from(ctrl == 0);
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
                    2,
                    0,
                    TRAP_PASS_ATK_LIMIT,
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
        _ => monster(w, rec, 1, mode),
    }
}

/// 0x279610(g, s, c, r, list, bound, filter, face_mode, [def_mode, see_all,
/// hidden, count, any]): the target among the first `count` list entries
/// for the unit (g, s) at (c, r): unit present, Manhattan(unit, (c, r)) ≤
/// bound, 0x279A90 on +0x57 bit 2 with `face_mode` and bit 5 with
/// `def_mode`, 0x27DEE0(filter), visible (0x279AF0), and 0x24DDA0 = 1 or
/// `any` (every call passes 1, so 0x24DDA0 is not ported); 1 ≤ level ≤
/// bound; taken when level < best (initially 7; best becomes the chosen
/// unit's Manhattan distance and bound its level), or on level == best
/// when its 0x279B90 margin against the unit (`hidden` for a card the AI
/// may not read) is below the current best's. The key, or −1.
#[allow(clippy::too_many_arguments)]
fn pick_visible<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    c: i32,
    r: i32,
    list: &Targets,
    bound: i32,
    filter: i32,
    face_mode: i32,
    def_mode: i32,
    see_all: bool,
    hidden: i32,
    count: i32,
    any: bool,
) -> R<i32> {
    if !any {
        return unsupported("0x279610 without its accept flag needs 0x24DDA0");
    }
    let mut bound = bound & 0xFF;
    let (face_mode, def_mode) = (face_mode & 0xFF, def_mode & 0xFF);
    let mut best = 7u8;
    let mut best_key = 0xFFFFu16;
    let mut best_pos = 0xFFFF;
    for i in 0..(count & 0xFFFF) {
        let Some(&e) = list.entries.get(i as usize) else {
            return unsupported("0x279610 count beyond the 22-entry list");
        };
        let (eg, es) = (i32::from(e.key >> 8), i32::from(e.key & 0xFF));
        if w.unit(eg, es).is_none() {
            continue;
        }
        let p = w.pos(eg, es);
        let (col, row) = (p & 0xFF, p >> 8);
        let dist = (col - c).abs() + (row - r).abs();
        if bound < dist {
            continue;
        }
        if search::flag_mode(i32::from(w.flag(eg, es, 2)), face_mode) != 1 {
            continue;
        }
        if search::flag_mode(i32::from(w.flag(eg, es, 5)), def_mode) != 1 {
            continue;
        }
        if !W::<C, F>::filter_slot(es, filter) {
            continue;
        }
        if !w.visible(eg, es, see_all) {
            continue;
        }
        let lv = e.level;
        if bound < i32::from(lv) || lv == 0 {
            continue;
        }
        if lv < best {
            best_key = e.key;
            best_pos = p & 0xFFFF;
            best = dist as u8;
            bound = i32::from(lv);
        } else if lv == best {
            let Some(u) = w.unit(g, s) else {
                return unsupported("0x279610 compares margins for a missing unit");
            };
            let m = if w.visible(eg, es, see_all) {
                w.margin(u, i32::from(e.key), col, row)?
            } else {
                hidden
            };
            let (bg, bs) = (i32::from(best_key >> 8), i32::from(best_key & 0xFF));
            let mb = if w.visible(bg, bs, see_all) {
                w.margin(u, i32::from(best_key), best_pos & 0xFF, best_pos >> 8)?
            } else {
                hidden
            };
            // 0x279990: the entry wins when its margin is LOWER (the
            // stronger enemy, for a trap whose own strength counts 0).
            if m < mb {
                best_key = e.key;
                best = dist as u8;
                bound = i32::from(lv);
                best_pos = p & 0xFFFF;
            }
        }
    }
    Ok(if best_key == 0xFFFF {
        -1
    } else {
        i32::from(best_key)
    })
}

/// 0x278E30(c, r, list, bound, filter, face_mode, def_mode, count): the
/// target among the first `count` list entries: unit present, 0x279A90 on
/// +0x57 bit 2 with `face_mode` and bit 5 with `def_mode`, 0x27DEE0(filter),
/// 1 ≤ level ≤ bound; taken when level < best (initially 7; best becomes
/// the chosen unit's Manhattan distance to (c, r) and bound its level), or
/// on level == best when its card record's byte 3 (0x1DE350, the deck
/// cost) exceeds the current best's. The key, or −1.
#[allow(clippy::too_many_arguments)]
fn pick_near<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    c: i32,
    r: i32,
    list: &Targets,
    bound: i32,
    filter: i32,
    face_mode: i32,
    def_mode: i32,
    count: i32,
) -> R<i32> {
    let mut bound = bound & 0xFF;
    let (face_mode, def_mode) = (face_mode & 0xFF, def_mode & 0xFF);
    let mut best = 7u8;
    let mut best_key = 0xFFFFu16;
    let deck_cost = |k: u16| -> R<u8> {
        let card = w.card_id(i32::from(k >> 8), i32::from(k & 0xFF));
        // 0x1DE350(card) + 3.
        match w.record_of(card).and_then(|_| w.fusions.deck_cost(card as u16)) {
            Some(v) => Ok(v),
            None => unsupported("0x278E30 reads the deck cost of a card without a record"),
        }
    };
    for i in 0..(count & 0xFFFF) {
        let Some(&e) = list.entries.get(i as usize) else {
            return unsupported("0x278E30 count beyond the 22-entry list");
        };
        let (eg, es) = (i32::from(e.key >> 8), i32::from(e.key & 0xFF));
        if w.unit(eg, es).is_none() {
            continue;
        }
        let p = w.pos(eg, es);
        let (col, row) = (p & 0xFF, p >> 8);
        if search::flag_mode(i32::from(w.flag(eg, es, 2)), face_mode) != 1 {
            continue;
        }
        if search::flag_mode(i32::from(w.flag(eg, es, 5)), def_mode) != 1 {
            continue;
        }
        if !W::<C, F>::filter_slot(es, filter) {
            continue;
        }
        let lv = e.level;
        if bound < i32::from(lv) || lv == 0 {
            continue;
        }
        let dist = (col - c).abs() + (row - r).abs();
        let take = if lv < best {
            true
        } else if lv == best {
            deck_cost(best_key)? < deck_cost(e.key)?
        } else {
            false
        };
        if take {
            best_key = e.key;
            best = dist as u8;
            bound = i32::from(lv);
        }
    }
    Ok(if best_key == 0xFFFF {
        -1
    } else {
        i32::from(best_key)
    })
}

/// 0x12D720(rec): trap cards: plan 0xB on a target. Round N = 1..=2 of the
/// reachable-unit tree over the enemy's units (0x278A20, see-all 0,
/// hidden strength 0x131050(controller); the root, list and pool carry
/// over): [`pick_visible`] (bound 5, filter 0x1F, def-mode 2, see-all 0,
/// `any` 1) with face-mode 1, then 0 — a pick that is neither spellbound
/// (0x1E2800) nor +0x4A (0x1E28C0) is the target. Then, with the count and
/// pool reset but the list entries kept, N = 1..=2 over the units the
/// controller controls: [`pick_near`] (bound 5, def-mode 2) with filter
/// 0x22 and face-mode 2, then 0x1F with 1, then 0x1F with 0. Nothing →
/// the controller's leader key.
fn trap<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    // 0x24F790(g, s): the controller.
    let ctrl = w.ctrl(g, s);
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
    rec.set_plans(0xBFF);
    // 0x2800B0(sp+0xE0): a no-op; sp+0x150: the reachable list, once;
    // sp+0x1AE: the count; sp+0x1AC / sp+0x1AA: the pool counters; sp+0xDC:
    // the root.
    let mut list = Targets::new();
    let mut pool = Pool::new();
    let mut root = None;
    for n in 1..=2 {
        let enemy = i32::from(w.ctrl(g, s) == 0);
        let hidden = hidden_value(w, ctrl)?;
        root = search::reach(w, &mut pool, &mut list, root, Some(u), c, r, enemy, n, 0, hidden)?;
        for face_mode in [1, 0] {
            let hidden = hidden_value(w, ctrl)?;
            let k = pick_visible(
                w,
                g,
                s,
                c,
                r,
                &list,
                TRAP_PICK_BOUND,
                0x1F,
                face_mode,
                2,
                false,
                hidden,
                i32::from(list.count),
                true,
            )?;
            if k >= 0 && w.spellbind(k >> 8, k & 0xFF) == 0 && w.x4a(k >> 8, k & 0xFF) == 0 {
                set_target(rec, k);
                return Ok(());
            }
        }
    }
    list.count = 0;
    let mut pool = Pool::new();
    let mut root = None;
    for n in 1..=2 {
        let own = w.ctrl(g, s);
        let hidden = hidden_value(w, ctrl)?;
        root = search::reach(w, &mut pool, &mut list, root, Some(u), c, r, own, n, 0, hidden)?;
        for (filter, face_mode) in [(0x22, 2), (0x1F, 1), (0x1F, 0)] {
            let k = pick_near(
                w,
                c,
                r,
                &list,
                TRAP_PICK_BOUND,
                filter,
                face_mode,
                2,
                i32::from(list.count),
            )?;
            if k >= 0 {
                set_target(rec, k);
                return Ok(());
            }
        }
    }
    set_target(rec, ctrl << 8);
    Ok(())
}

/// 0x1310B0(side, card): 1 when the ritual's tributes can be chosen
/// (0x2561B0 on the 0x2565B0 spec of its entry 0 word 1) and none of them
/// has an effective ATK (0x252300 at its own cell) of 3000 or more, is card
/// 0x12A or 0x146, or is card 0x131 not committed face-up; else 0.
fn ritual_ok<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    // 0x2715C0(card, 0): entry 0 of a virtual record (null for 999).
    if card == 999 {
        return unsupported("0x1310B0 reads entry 0 of card 999");
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
        let c = w.card_id(g, s);
        if w.eff_atk(g, s, p & 0xFF, p >> 8)? >= RITUAL_ATK_LIMIT {
            return Ok(0);
        }
        if c == id::CARD_12A || (c == id::CARD_131 && !w.flag(g, s, 4)) || c == id::CARD_146 {
            return Ok(0);
        }
        i += 1;
    }
    Ok(1)
}

/// 0x130F30(side): the graveyard card plan 0xC and 0x12DDF0 would pick:
/// −1 with five cards in the own slots 6..=10 (0x1E1470); else the first
/// non-monster of the own graveyard whose entry 0 (0x2715C0) exists and
/// whose card record (0x1DE350) is NULL — never, for the cards a graveyard
/// holds — would answer (side << 16) | index after reading RAM address 3
/// and testing its entry type; so −1.
fn graveyard_pick<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    if w.others(side) >= 5 {
        return Ok(-1);
    }
    let mut i = 0;
    while i < w.graveyard_count(side) {
        let card = w.graveyard(side, i);
        if super::summon::kind_of(w, card) >> 5 != 0 && w.record_of(card).is_none() {
            return unsupported("0x130F30 reads RAM address 3 for a card without a record");
        }
        i += 1;
    }
    Ok(-1)
}

/// 0x131210(g, s, rec, mode): the reveal request (+0xC; 1 presses R2). 0
/// once committed face-up. Monsters: 0x146 reveals with four or more
/// units of either group not controlled by the group (slots 0..=10),
/// 0x142 in mode 1, 0x12A always; others 2. Magic: 2 under a type-0xE far
/// trap (0x27F170 mode 6, see-all 0), else 1. Rituals: 2 under a type-0xE
/// trap, when the tributes fail [`ritual_ok`] or on Crush; else 1.
/// Power-ups and traps 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
    mode: u8,
) -> R<u32> {
    let card = w.card_id(g, s);
    // 0x251C80(g, s) is called and its result dropped.
    let pos = w.pos(g, s);
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    match w.kind(g, s) >> 5 {
        0 => match card {
            id::CARD_146 => {
                let mut n = 0u8;
                for slot in 0..11 {
                    for grp in [0, 1] {
                        if w.unit(grp, slot).is_some() && w.ctrl(grp, slot) != g {
                            n = n.wrapping_add(1);
                        }
                    }
                }
                Ok(if n < 4 { 2 } else { 1 })
            }
            id::CARD_142 => Ok(if mode == 1 { 1 } else { 2 }),
            id::CARD_12A => Ok(1),
            _ => Ok(2),
        },
        1 => Ok(if w.classify(g, s, 6, false)? == 5 { 2 } else { 1 }),
        5 => {
            if w.classify(g, s, 6, false)? == 5 {
                return Ok(2);
            }
            if ritual_ok(w, w.ctrl(g, s), card)? != 1 {
                return Ok(2);
            }
            Ok(if w.terrain(pos & 0xFF, pos >> 8) == 9 {
                2
            } else {
                1
            })
        }
        _ => Ok(2),
    }
}

/// (col, row) of a candidate byte (row << 4) | col.
fn nibbles(b: u8) -> (i32, i32) {
    (i32::from(b & 0xF), i32::from(b >> 4))
}

/// 0x1309F0(rec, side, d, which, mode): plan 0xB (the target is a unit
/// key); −1 when the target is gone (the primary target is then cleared,
/// as in 0x1108C0). The unit stays (type 6 to its own cell, 0x27FCA0 with
/// defflag 1) when it cannot move (0x247E10), is threatened (0x27D500(g,
/// s, 0)) or the attack routes (0x27B9D0, see-all 0, at most 30 moves,
/// hidden strength 0x131050(side) & 0xFFFF) give no first move. Else of
/// the empty first moves the one with the HIGHEST danger (0x278600 radius
/// 3, strength 1, see-all 0x131050(side) & 0xFF), ties to the one closer
/// to the target's cell, starting from score 0 with no candidate: a tie
/// at 0 before any candidate compares with the byte past the step buffer
/// ([`TRAP_MOVE_STACK_BYTE`]). +0xC is not written; the target is kept.
///
/// Generic over the module (`O::hidden_value`) and the danger radius:
/// Necromancer's 0x12B390 is this function with radius 5.
pub(super) fn trap_move<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    radius: i32,
) -> R<i32> {
    let t = rec.target(which);
    let (tg, ts) = (i32::from(t >> 8), i32::from(t & 0xFF));
    if w.unit(tg, ts).is_none() {
        // 0x130A80: always the primary target.
        rec.consume(1);
        return Ok(-1);
    }
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let pos = w.pos(g, s);
    let tpos = w.pos(tg, ts);
    let stay = |d: &mut Decision| -> R<i32> {
        d.key = rec.key;
        d.kind = 6;
        d.target = pos as u16;
        d.position = exec::stance(w, g, s, d.target, 1, false)?;
        Ok(0)
    };
    // 0x130B28: a free target never reaches this test (no unit (255, 255)).
    if t == 0xFFFF {
        return stay(d);
    }
    let Some(u) = w.unit(g, s) else {
        return unsupported("0x1309F0 for a missing unit");
    };
    let (c, r) = (pos & 0xFF, pos >> 8);
    if w.allowance(u, c, r)? == 0 || w.threatened(g, s, false)? {
        return stay(d);
    }
    let hidden = O::hidden_value(w, side)? & 0xFFFF;
    let (tc, tr) = (tpos & 0xFF, tpos >> 8);
    let mut buf = Steps::new();
    search::attack_path_k(
        w,
        g,
        s,
        &mut buf,
        c,
        r,
        tc,
        tr,
        u32::from(t),
        0,
        TRAP_ROUTE_LIMIT,
        hidden,
    )?;
    let n = search::step_count(&buf);
    if n <= 0 {
        return stay(d);
    }
    let dist = |b: u8| {
        let (cc, cr) = nibbles(b);
        (cc - tc).abs() + (cr - tr).abs()
    };
    let mut best = 0;
    let mut bi: Option<usize> = None;
    for i in 0..n as usize {
        let b = buf.0[i];
        let (cc, cr) = nibbles(b);
        if w.at(cc, cr).is_some() {
            continue;
        }
        let see_all = O::hidden_value(w, side)? & 0xFF != 0;
        let score = w.danger(cc, cr, side, radius, 1, see_all)?;
        if best < score {
            best = score;
            bi = Some(i);
        } else if score == best {
            let prev = match bi {
                Some(j) => buf.0[j],
                None => match TRAP_MOVE_STACK_BYTE {
                    Some(b) => b,
                    None => {
                        return unsupported(
                            "0x1309F0 ties at danger 0 with no candidate (reads past its step buffer)",
                        )
                    }
                },
            };
            if dist(b) < dist(prev) {
                best = score;
                bi = Some(i);
            }
        }
    }
    let Some(i) = bi else {
        return stay(d);
    };
    let (cc, cr) = nibbles(buf.0[i]);
    d.key = rec.key;
    d.kind = 6;
    d.target = ((cr << 8) | cc) as u16;
    d.position = exec::stance(w, g, s, d.target, 0, false)?;
    Ok(0)
}

/// 0x12F640(rec, side, d, which, mode): plan 0xC. With a graveyard pick
/// ([`graveyard_pick`], never): type 0xC with the pick (0x27F500 filter
/// 0x1E). Else a card committed face-up is marked acted by the module
/// itself (0x1E30F0(g, s, 0, 1)) with an empty type-0xE record, and a set
/// one retreats as 0x110330 does with +0xC = 2 ([`exec::retreat_with`]).
fn plan_c<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    if graveyard_pick(w, side)? >= 0 {
        return unsupported("0x12F640 with a graveyard pick (0x27F500 filter 0x1E)");
    }
    if w.flag(g, s, 4) {
        d.mark_acted = Some(rec.key);
        d.key = rec.key;
        d.kind = 0xE;
        return Ok(());
    }
    exec::retreat_with::<Darkness, _, _>(w, rec, side, d, which, Some(2))
}

#[cfg(test)]
mod tests;
