//! Pegasus's controller (opponent 9, control byte 9, driver 0x139F90 =
//! Weevil's 0x1117D0 by opcode shape, module 0x135B00..0x13A158). The
//! module is Weevil's template with its own constants and eleven novel
//! functions; the shared parts (dispatch 0x135B00, actor choice 0x136240,
//! executors 0x138880 / 0x1382C0 / 0x137DA0, the 0x27xxxx helpers) run
//! through the framework in [`super`] with the constants of [`Pegasus`].
//!
//! Own functions, by EE address: [`assign`] 0x136060, [`monster`] 0x136620
//! (0x136880: 0x27F760 with see-all 1 and limit 0x898), [`formation`]
//! 0x1368E0 / 0x136930, [`magic`] 0x1369B0, [`reveal`] 0x139A50,
//! [`charge`] 0x139950, the plan-0xC / 0xD executors [`mimicat`] 0x139050
//! and [`change_of_heart`] 0x139220 with their target searches
//! [`control_target`] 0x139430 (0x273B40), [`dragon_jar_useful`] 0x139680
//! and [`mimicat_target`] 0x139770 (0x27F500), and the summon planner in
//! [`summon`] (0x136AE0 and its subtree).
//!
//! The action types 0xC/0xD (Mimicat's graveyard pick, Change of Heart's
//! unit pick) become [`crate::duel::ai::Action::Activate`] through
//! [`super::plan_for`], answered in the engine's selection states (0x2D /
//! 0x30); a unit the module marks acted itself (0x1E30F0) comes back as
//! [`super::Plan::MarkActed`].

pub(super) mod summon;

use super::exec;
use super::plans::{self, set_plan, set_target};
use super::search::{self, Pool, Targets};
use super::summon::{nth_unit, popcount, remove_unit, unit_mask};
use super::world::{unsupported, World, R};
use super::{Decision, FusionData, Memory, Opponent, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// Card ids the module tests as immediates.
pub(super) mod id {
    /// Millennium Shield (a monster planned like a trap, 0x136060).
    pub const MILLENNIUM_SHIELD: i32 = 0xBD;
    /// Dragon Piper (0x139A50).
    pub const DRAGON_PIPER: i32 = 0x265;
    /// Dragon Capture Jar.
    pub const DRAGON_CAPTURE_JAR: i32 = 0x2AB;
    /// Mimicat.
    pub const MIMICAT: i32 = 0x2AF;
    /// Toon World.
    pub const TOON_WORLD: i32 = 0x2B7;
    /// Tremendous Fire (the graveyard card Mimicat copies, 0x139770).
    pub const TREMENDOUS_FIRE: i32 = 0x2C9;
    /// Change of Heart.
    pub const CHANGE_OF_HEART: i32 = 0x2DC;
    /// Brain Control.
    pub const BRAIN_CONTROL: i32 = 0x2DD;
    /// Curse of Millennium Shield, Garma Sword Oath (rituals).
    pub const CURSE_OF_MILLENNIUM_SHIELD: i32 = 0x33E;
    pub const GARMA_SWORD_OATH: i32 = 0x351;
}

/// 0x136880: the ATK limit of 0x27F760's first pass (0x898 = 2200).
const TRAP_PASS_ATK_LIMIT: i32 = 0x898;
/// 0x139860 / 0x139A50: the tribute ATK limit (`slti 0xA28` = 2600).
const RITUAL_ATK_LIMIT: i32 = 0xA28;
/// 0x1369B0 / 0x139A50: Toon World's open-area query (terrain 6, more
/// than 5 cells, radius 2) and the row limit of its centre.
const TOON_TERRAIN: i32 = 6;
const TOON_MIN_CELLS: i32 = 5;
const TOON_RADIUS: i32 = 2;
const TOON_ROW_LIMIT: i32 = 4;

/// Pegasus's module.
pub(super) struct Pegasus;

impl Opponent for Pegasus {
    const CONTROL: u8 = 9;
    const DANGER_HIDDEN: i32 = 0;
    const STANCE_SEE_ALL: bool = true;
    const SEARCH_SEE_ALL: i32 = 1;
    const RETREAT_SEE_ALL: bool = true;
    const IMMOBILE_STANCE: bool = true;
    const STAY_B_RETURN: i32 = 1;
    /// 0x13633C: class 3 only.
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
    fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        g: i32,
        s: i32,
        c: i32,
        r: i32,
        tc: i32,
        tr: i32,
    ) -> R<bool> {
        charge(w, g, s, c, r, tc, tr)
    }
    /// 0x135BF0 / 0x135E0C: 0xD → 0x139220, 0xC → 0x139050.
    fn extra_plan<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        rec: &mut Record,
        side: i32,
        d: &mut Decision,
        which: u8,
        code: u8,
    ) -> R<bool> {
        match code {
            0xC => mimicat(w, rec, side, d, which)?,
            0xD => change_of_heart(w, rec, side, d, which)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// 0x136060(rec): (re)assign the plans of a record (no mode, no side
/// struct). Missing, acted or free records are left alone. The leader
/// (kept while the +8 word is not −1): threatened (0x27D500 with see-all 1)
/// → 0x50A / 0x5FF by the summon flag, else 0xA05 / 0x5FF (no walking
/// target). Non-leaders by class: 3 → [`formation`]; 1 → [`magic`]; 2 →
/// 0x27E390(rec, 2, 1, 0); 0 → [`formation`] for Millennium Shield, else
/// [`monster`](self::monster)(rec, 1); 4, 5 and above → 0x5FF.
fn assign<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    // 0x251C80(g, s) is called and its result dropped.
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
        } else {
            rec.set_plans(if summon { 0xA05 } else { 0x5FF });
        }
        return Ok(());
    }
    match w.kind(g, s) >> 5 {
        3 => formation(w, rec),
        1 => magic(w, rec)?,
        2 => plans::power_up(w, rec, 2, 1, 0)?,
        0 => {
            if w.card_id(g, s) == id::MILLENNIUM_SHIELD {
                formation(w, rec);
            } else {
                monster(w, rec, 1)?;
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x136930(): Pegasus's formation cell: (col 2, row 2), else (col 5,
/// row 2), whichever is first empty and not Labyrinth; −1 when neither.
fn formation_cell<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>) -> i32 {
    if w.at(2, 2).is_none() && w.terrain(2, 2) != 8 {
        return 0x202;
    }
    if w.at(5, 2).is_none() && w.terrain(5, 2) != 8 {
        return 0x205;
    }
    -1
}

/// 0x1368E0(rec): plan 3 (defence) to the formation cell, else plan 5.
fn formation<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) {
    let v = formation_cell(w);
    if v < 0 {
        rec.set_plans(0x5FF);
    } else {
        rec.set_plans(0x3FF);
        set_target(rec, v);
    }
}

/// 0x1369B0(rec): magic cards by id: Change of Heart → plan 0xD, Mimicat →
/// plan 0xC (both with target 0xFFFF); Toon World → plan 6 to the open
/// Toon-terrain centre 0x27E0B0(u, c, r, 6, 5, 2) when it lies in rows
/// 0..3, else plan 5; every other magic card plan 5.
fn magic<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, rec: &mut Record) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    // 0x251C80(g, s) is called and its result dropped.
    let p = w.pos(g, s);
    match w.card_id(g, s) {
        id::CHANGE_OF_HEART => {
            rec.set_plans(0xDFF);
            set_target(rec, 0xFFFF);
        }
        id::MIMICAT => {
            rec.set_plans(0xCFF);
            set_target(rec, 0xFFFF);
        }
        id::TOON_WORLD => {
            let Some(u) = w.unit(g, s) else {
                return unsupported("0x27E0B0 with a null unit");
            };
            let v = plans::open_area(
                w,
                u,
                p & 0xFF,
                p >> 8,
                TOON_TERRAIN,
                TOON_MIN_CELLS,
                TOON_RADIUS,
            )?;
            if v < 0 || v >> 8 >= TOON_ROW_LIMIT {
                rec.set_plans(0x5FF);
            } else {
                rec.set_plans(0x6FF);
                set_target(rec, v & 0xFFFF);
            }
        }
        _ => rec.set_plans(0x5FF),
    }
    Ok(())
}

/// 0x136620(rec, a1): monsters. `a1` ≠ 0 and 0x27CB40 reaches the enemy
/// leader within 2 moves: plan 2 on its key. Else N = 1, 2: 0x136880 →
/// 0x27F760 (radius 2, see-all 1, limit 0x898): 1 → planned; 0 → plan 5;
/// 2 → plan 3 at the own cell; 3 → plan 6 at the own cell; else next N.
/// Exhausted → plan 5. (The reachable list and the node pool are shared
/// by both rounds, as in 0x10DD60.)
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
    // sp+0x130: the reachable list (key 0xFFFF, level 0xFF), once.
    let mut list = Targets::new();
    // 0x2800B0(sp+0xC0): a no-op.
    if a1 != 0 {
        let v = search::moves_to_k(w, g, s, my & 0xFF, my >> 8, lg, ls, lp & 0xFF, lp >> 8, 2)?;
        if v > 0 {
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // sp+0x18C / sp+0x18E: the pool counters, zeroed once.
    let mut pool = Pool::new();
    for n in 1..=2 {
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
            2,
            1,
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

/// 0x139950(g, s, c, r, tc, tr): the unit at (c, r) may jump onto its
/// target at (tc, tr): Manhattan distance exactly 2, on the board, not
/// acted, and favoured by the terrain of (c, r) (0x24F820 = 1). Unlike
/// 0x110C2C there is no card exception.
#[allow(clippy::too_many_arguments)]
pub(super) fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    c: i32,
    r: i32,
    tc: i32,
    tr: i32,
) -> R<bool> {
    if (tc - c).abs() + (tr - r).abs() != 2 {
        return Ok(false);
    }
    if w.unit(g, s).is_none() || w.flag(g, s, 0) {
        return Ok(false);
    }
    Ok(w.terrain_favour_of(w.terrain(c, r), g, s)? == 1)
}

/// 0x139A50's counting loops (0x139B24 for Dragon Piper with `positive`,
/// 0x139EC8 for type-0x20 magic with any non-zero value): over slots
/// 1..=11 of group `g` and of the other group, units whose 0x1E2800 value
/// passes (−1 for an empty slot, or slot 11, passes the non-zero test) and
/// whose controller 0x251C80 is `g`; 2 below two, else 1.
fn bound_count<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    positive: bool,
) -> u32 {
    let other = i32::from(g == 0);
    let mut count = 0u8;
    for slot in 1..12 {
        for grp in [g, other] {
            let v = w.spellbind(grp, slot);
            let hit = if positive { v > 0 } else { v != 0 };
            if hit && w.ctrl(grp, slot) == g {
                count = count.wrapping_add(1);
            }
        }
    }
    if count < 2 {
        2
    } else {
        1
    }
}

/// 0x139A50(g, s, rec): the reveal request (+0xC; 1 presses R2). 0 once
/// committed face-up. Monsters: Dragon Piper reveals with two or more own
/// spellbound units ([`bound_count`]), others 2. Rituals: 2 under a type-0xE
/// far trap (0x27F170 mode 6, see-all 1), when the tributes fail
/// 0x139860's test (limit 2600, inlined) or on Crush; else 1. Magic: 2
/// under a type-0xE trap or without an entry; by entry-0 type: 0x20 →
/// [`bound_count`] (any non-zero), 0x1F → 1 only for Brain Control with a
/// strongest-unit target ([`control_target`] low half), 0x17 →
/// [`dragon_jar_useful`], 0xD → 1, 2 → 1 when the card stands on the Toon
/// open-area centre (rows 0..3); other types 2. Power-ups and traps 2.
fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    _rec: &Record,
) -> R<u32> {
    let card = w.card_id(g, s);
    // 0x251C80(g, s) is called and its result dropped.
    let entry0 = w.unit(g, s).and_then(|u| u.entry(0));
    let pos = w.pos(g, s);
    if w.flag(g, s, 4) {
        return Ok(0);
    }
    match w.kind(g, s) >> 5 {
        0 => Ok(if card == id::DRAGON_PIPER {
            bound_count(w, g, true)
        } else {
            2
        }),
        5 => {
            if w.classify(g, s, 6, true)? == 5 {
                return Ok(2);
            }
            let ctrl = w.ctrl(g, s);
            if super::summon::ritual_ok_with(w, ctrl, card, RITUAL_ATK_LIMIT, false)? != 1 {
                return Ok(2);
            }
            Ok(if w.terrain(pos & 0xFF, pos >> 8) == 9 {
                2
            } else {
                1
            })
        }
        1 => {
            if w.classify(g, s, 6, true)? == 5 {
                return Ok(2);
            }
            let Some([w0, _]) = entry0 else {
                return Ok(2);
            };
            match w0 >> 8 {
                0x20 => Ok(bound_count(w, g, false)),
                0x1F => {
                    if card != id::BRAIN_CONTROL {
                        return Ok(2);
                    }
                    let v = control_target(w, g)?;
                    Ok(if v < 0 || v & 0xFFFF == 0xFFFF { 2 } else { 1 })
                }
                0x17 => Ok(if dragon_jar_useful(w, g)? { 1 } else { 2 }),
                0xD => Ok(1),
                2 => {
                    let Some(u) = w.unit(g, s) else {
                        return unsupported("0x27E0B0 with a null unit");
                    };
                    let v = plans::open_area(
                        w,
                        u,
                        pos & 0xFF,
                        pos >> 8,
                        TOON_TERRAIN,
                        TOON_MIN_CELLS,
                        TOON_RADIUS,
                    )?;
                    Ok(if v < 0 || v >> 8 >= TOON_ROW_LIMIT || v != pos {
                        2
                    } else {
                        1
                    })
                }
                _ => Ok(2),
            }
        }
        _ => Ok(2),
    }
}

/// 0x273B40(mask, side): `mask` without the units not controlled by
/// `side` that a leader ability #11 covers (0x273770) or a continuous
/// type-0xB unit protects (0x269020).
pub(super) fn protected_filter<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    mask: i32,
    side: i32,
) -> R<i32> {
    let mut out = mask;
    for i in 0..popcount(mask) {
        let k = nth_unit(mask, i);
        let (g, s) = (k >> 8, k & 0xFF);
        let ctrl = w.ctrl(g, s);
        if w.leader_kind_aura_of(g, s, 0xB)? && ctrl != side {
            out = remove_unit(out, g, s);
            continue;
        }
        if !w.trap_immunity(g, s)? {
            continue;
        }
        if ctrl == side {
            continue;
        }
        out = remove_unit(out, g, s);
    }
    Ok(out)
}

/// 0x139430(side): Change of Heart / Brain Control targets, or −1 while a
/// continuous type-0x39 unit is out. Over the enemy's non-leaders
/// (0x27D9B0(other, 0x34, see-all 1)) next to the enemy leader (Manhattan
/// 1), not control-changed (+0x57 bit 1, the leftover argument of
/// 0x1E3380), +0x49 and +0x4A zero, with an effective ATK at their cell
/// no lower than the last accepted one's (from 1): the high half is the
/// last accepted key, the low half the last accepted key that is the
/// board's strongest monster (0x24E360 crit 0x21 over the enemy's units,
/// filtered by [`protected_filter`]) or 0xFFFF. −1 when none is accepted.
fn control_target<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> R<i32> {
    let other = i32::from(side == 0);
    let lp = w.pos(other, 0);
    if w.continuous_count(0x39) > 0 {
        return Ok(-1);
    }
    let strongest = unit_mask(w, 0, 0, side, 2, 0x21, 0xE)?;
    let strongest = protected_filter(w, strongest, side)?;
    let list = w.list(other, 0x34, true);
    let (lr, lc) = (lp >> 8, lp & 0xFF);
    let mut best = -1;
    let mut threshold = 1;
    let mut last = -1;
    for &k in &list {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        let (r, c) = (p >> 8, p & 0xFF);
        if (r - lr).abs() + (c - lc).abs() != 1 {
            continue;
        }
        // 0x139584: 0x1E3380(k, 1) — the bit index is the leftover a2 = 1
        // (control-changed), not the acted bit.
        if w.flag(g, s, 1) || w.spellbind(g, s) != 0 || w.x4a(g, s) != 0 {
            continue;
        }
        let atk = w.eff_atk(g, s, c, r)?;
        if atk < threshold {
            continue;
        }
        last = k;
        threshold = atk;
        if nth_unit(strongest, 0) == k {
            best = k;
        }
    }
    if last < 0 {
        return Ok(-1);
    }
    Ok((last & 0xFFFF) << 16 | (best & 0xFFFF))
}

/// 0x139680(side): Dragon Capture Jar is worth playing: the enemy controls
/// two or more Dragon-kind monsters (0x24E360 rel 2, type 0), or the
/// board's strongest monster (crit 0x21 over every unit, its first bit) is
/// an enemy Dragon.
fn dragon_jar_useful<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<bool> {
    if popcount(unit_mask(w, 0, 0, side, 2, 0, 0xE)?) >= 2 {
        return Ok(true);
    }
    let k = nth_unit(unit_mask(w, 0, 0, side, 3, 0x21, 0xE)?, 0);
    let (g, s) = (k >> 8, k & 0xFF);
    if w.ctrl(g, s) == side {
        return Ok(false);
    }
    let kind = w.kind(g, s);
    Ok(kind >> 5 == 0 && kind & 0x1F == 0)
}

/// 0x139770(side): the graveyard card Mimicat copies: −1 with five cards
/// in the own slots 6..=10 (0x1E1470); else (side << 16) | index of the
/// first Tremendous Fire in the own graveyard, then in the enemy's; −1
/// when neither holds one.
fn mimicat_target<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> i32 {
    if w.others(side) >= 5 {
        return -1;
    }
    for gy in [side, i32::from(side == 0)] {
        let mut i = 0;
        while i < w.graveyard_count(gy) {
            if w.graveyard(gy, i) == id::TREMENDOUS_FIRE {
                return gy << 16 | i;
            }
            i += 1;
        }
    }
    -1
}

/// The failure exit shared by 0x139050 and 0x139220 (0x139194 /
/// 0x1393A4): a committed face-up card is marked acted by the module
/// itself (0x1E30F0(g, s, 0, 1)) with an empty type-0xE record; a set one
/// retreats (0x137DA0).
fn no_pick<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.flag(g, s, 4) {
        d.mark_acted = Some(rec.key);
        d.key = rec.key;
        d.kind = 0xE;
        return Ok(());
    }
    exec::retreat::<Pegasus, _, _>(w, rec, side, d, which)
}

/// 0x139050(rec, side, d, which): plan 0xC (Mimicat). With a graveyard
/// card ([`mimicat_target`]) and no type-0xE far trap (0x27F170 mode 6,
/// see-all 1): type 0xC, +0x10 from 0x27FCA0(defflag 1, see-all 1) on the
/// stale +4, +0xC = 1, +4 = (graveyard side << 8) | index (0x27F500 with
/// filter 0 returns the index), target cleared. Else [`no_pick`].
fn mimicat<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    let mut t = mimicat_target(w, side);
    if w.classify(g, s, 6, true)? == 5 {
        t = -1;
    }
    if t < 0 {
        return no_pick(w, rec, side, d, which);
    }
    let gy = (t >> 16) & 0xFFFF;
    let index = t & 0xFF;
    d.key = rec.key;
    d.kind = 0xC;
    d.position = exec::stance(w, g, s, d.target, 1, true)?;
    d.reveal = 1;
    d.target = (gy << 8 | index) as u16;
    rec.consume(which);
    Ok(())
}

/// 0x139220(rec, side, d, which): plan 0xD (Change of Heart). With a
/// target ([`control_target`]'s high half, not 0xFFFF) and no type-0xE far
/// trap: type 0xD, +0x10 from 0x27FCA0(defflag 1, see-all 1) on the stale
/// +4, +0xC = 1, +4 = the number of units in the enemy group's slots 1..
/// up to the target's KEY (compared with the slot number, so a group-1 key
/// counts every slot) minus one; the target is cleared. Else [`no_pick`].
fn change_of_heart<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let _ = w.pos(g, s);
    let mut t = control_target(w, side)?;
    if w.classify(g, s, 6, true)? == 5 {
        t = -1;
    }
    let key = (t >> 16) & 0xFFFF;
    if t < 0 || key == 0xFFFF {
        return no_pick(w, rec, side, d, which);
    }
    let other = i32::from(side == 0);
    let mut count: i32 = 0;
    let mut slot = 1;
    while slot < 11 {
        if w.unit(other, slot).is_some() {
            count += 1;
        }
        if slot == key {
            break;
        }
        slot += 1;
    }
    d.key = rec.key;
    d.kind = 0xD;
    d.position = exec::stance(w, g, s, d.target, 1, true)?;
    d.reveal = 1;
    d.target = (count - 1) as u16;
    rec.consume(which);
    Ok(())
}

#[cfg(test)]
mod tests;
