//! Plan assignment for non-leader units (called from 0x10D6F0): plain
//! monsters 0x10DD60, effect monsters 0x10E150, magic 0x10E510, power-ups
//! 0x27E390, traps and rituals 0x10E3D0, and their helpers (0x27F760 target
//! passes, 0x279F10 stance, 0x2800C0 / 0x27F170 mode 0 / 0x24CC30 trap
//! look-ahead, 0x27F060 / 0x27D1A0 / 0x27D080 pickers, 0x27E0B0 open area).
//! The shared helpers (0x27xxxx) also serve the other opponent modules
//! (`super::pegasus`).
//!
//! Record writes follow the original: `sh` of 0xPPFF at +4 sets the primary
//! plan and clears the secondary one; `sw (x << 16) | lhu(+8)` at +8 sets the
//! primary target to `x & 0xFFFF` and keeps the secondary target.

use super::search::{self, Pool, Steps, Targets};
use super::world::{unsupported, Unit, World, R};
use super::{FusionData, Record};
use crate::duel::CardData;

type W<'a, C, F> = World<'a, C, F>;

/// 0x10E0F0 / 0x10E150: the ATK limit of the first 0x27F760 pass (0x960).
const TRAP_PASS_ATK_LIMIT: i32 = 0x960;
/// 0x10E150 card switch: Jirai Gumo, Man-Eater Bug, Great Moth, Perfectly
/// Ultimate Great Moth.
const JIRAI_GUMO: i32 = 0x198;
const MAN_EATER_BUG: i32 = 0x1A1;
const GREAT_MOTH: i32 = 0x192;
const PERFECT_GREAT_MOTH: i32 = 0x193;
/// 0x10E150: Jirai Gumo's LP test (`slti 0x3E8`).
const JIRAI_GUMO_LP: i32 = 1000;
/// 0x24CC30 jump-table cases 6..=10 (kind index): the mover is destroyed
/// when its ATK is below these immediates (`slti 0x3E9` .. `slti 0xFA1`).
const NEAR_TRAP_ATK_BELOW: [i32; 5] = [0x3E9, 0x5DD, 0x7D1, 0xBB9, 0xFA1];
/// 0x249C90: the card whose test is "Zombie" instead of the table bits.
const ZOMBIE_POWER_UP: i32 = 0x304;
/// 0x249C90: first card of the compatibility bit rows (0x29C970).
const POWER_UP_BASE: i32 = 0x2F0;

/// `sh 0xPPFF, 4(rec)`: primary plan `p`, secondary plan cleared.
pub(super) fn set_plan(rec: &mut Record, p: u8) {
    rec.plan_a = p;
    rec.plan_b = 0xFF;
}

/// `sw (x << 16) | lhu(8(rec)), 8(rec)`: primary target = x & 0xFFFF.
pub(super) fn set_target(rec: &mut Record, x: i32) {
    rec.target_a = x as u16;
}

/// 0x10DD60(rec, a1, mode): monsters without effects.
/// 1. `a1` ≠ 0 and 0x27CB40 reaches the enemy leader within 2 moves: plan 2
///    on the leader key.
/// 2. N = 1..L (mode 0: L = 2, radius 2; else L = 5, radius 6): 0x10E0F0 →
///    0x27F760; 1 = planned, 0 → 0x10E3D0(rec, 1, 1), 2 → plan 3 at the own
///    cell, 3 → plan 6 at the own cell, else next N (the reachable list and
///    the node-pool counters are shared by all N).
/// 3. Exhausted: 0x10E3D0(rec, 1, 0).
pub(super) fn monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
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
    // sp+0x150: the reachable list, initialised once (key 0xFFFF, level 0xFF).
    let mut list = Targets::new();
    if a1 != 0 {
        let v = search::moves_to_k(w, g, s, my & 0xFF, my >> 8, lg, ls, lp & 0xFF, lp >> 8, 2)?;
        if v > 0 {
            set_plan(rec, 2);
            set_target(rec, leader);
            return Ok(());
        }
    }
    // sp+0x1AC / sp+0x1AE: node-pool counters zeroed once for every N.
    let mut pool = Pool::new();
    let (last, radius) = if mode == 0 { (2, 2) } else { (5, 6) };
    for n in 1..=last {
        let side = i32::from(w.ctrl(g, s) == 0);
        // 0x10F670(ctrl) always returns 0: the strength assumed for hidden
        // units.
        let hidden = 0;
        let v = attack_passes(
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
                formation(w, rec, 1, 1);
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
    formation(w, rec, 1, 0);
    Ok(())
}

/// 0x27F760(rec, 0, u, c, r, side, depth, hidden, [list, count = 0, &bytes,
/// &nodes, radius, see_all, limit, buf]) (0x10E0F0 adds limit 0x960): one
/// deepening round of the reachable-unit tree (0x278A20; the count is this
/// call's own slot, the list entries persist), then three target passes
/// (0x278AA0 with bound = radius):
/// 1. face-up cards in slots 6..10 (filter 0x1E): plan 2 on it when the own
///    ATK here (0x252380) ≤ `limit`, return 1;
/// 2. face-up monsters (0x1D), else 3. face-down non-leaders (0x34): the
///    primary target is written first; margin 0x279E50 > 0 → 0x2800C0,
///    return 1; else unless spellbound ≥ 2 or +0x4A → 0x279F10.
///
/// −1 when no pass finds a target.
#[allow(clippy::too_many_arguments)]
pub(super) fn attack_passes<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    u: &Unit,
    c: i32,
    r: i32,
    side: i32,
    depth: i32,
    hidden: i32,
    list: &mut Targets,
    pool: &mut Pool,
    radius: i32,
    see_all: i32,
    limit: i32,
) -> R<i32> {
    list.count = 0;
    search::reach(
        w,
        pool,
        list,
        None,
        Some(u),
        c,
        r,
        side,
        depth,
        see_all,
        hidden,
    )?;
    let count = i32::from(list.count);
    let pick = |filter: i32, face: i32| {
        search::pick_target(
            w, u, c, r, list, radius, filter, face, 2, see_all, hidden, count,
        )
    };
    let k = pick(0x1E, 1)?;
    if k >= 0 && w.eff_atk_of(u, c, r)? <= limit {
        set_plan(rec, 2);
        set_target(rec, k);
        return Ok(1);
    }
    let mut k = pick(0x1D, 1)?;
    if k < 0 {
        k = pick(0x34, 0)?;
        if k < 0 {
            return Ok(-1);
        }
    }
    let (tg, ts) = (k >> 8, k & 0xFF);
    let tp = w.pos(tg, ts);
    let (tc, tr) = (tp & 0xFF, tp >> 8);
    set_target(rec, k);
    if w.margin_seen(u, k, hidden, tc, tr, see_all != 0)? > 0 {
        attack_or_hold(w, rec, k, see_all)?;
        return Ok(1);
    }
    if w.spellbind(tg, ts) >= 2 || w.x4a(tg, ts) != 0 {
        return Ok(-1);
    }
    stance(w, u, k, hidden, tc, tr, see_all)
}

/// 0x279F10(u, key, hidden, tc, tr, see_all), for a target this unit would
/// not beat: A / D = own ATK / DEF at (tc, tr) (+ race bonus), E = the
/// target's ATK there (+ race bonus; `hidden` when not visible, 0 for
/// leaders and non-monsters). A > D: 0 if A < E else 3; A ≤ D: 2 if E < D
/// else 0.
fn stance<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    key: i32,
    hidden: i32,
    tc: i32,
    tr: i32,
    see_all: i32,
) -> R<i32> {
    if key == -1 {
        return w.margin_vs(u, hidden, tc, tr);
    }
    let (tg, ts) = (key >> 8, key & 0xFF);
    let (a, d, e);
    if !w.visible(tg, ts, see_all & 0xFF != 0) {
        a = w.eff_atk_of(u, tc, tr)?;
        d = w.eff_def_of(u, tc, tr)?;
        e = hidden;
    } else {
        let t = w
            .unit(tg, ts)
            .ok_or(crate::duel::DuelError::InvalidCommand(
                "0x279F10 without a target",
            ))?;
        if u.is_leader() || u.class() != 0 {
            (a, d) = (0, 0);
        } else {
            a = w.eff_atk_of(u, tc, tr)? + W::<C, F>::race_bonus(u, t);
            d = w.eff_def_of(u, tc, tr)? + W::<C, F>::race_bonus(u, t);
        }
        e = if t.is_leader() || w.kind(tg, ts) >> 5 != 0 {
            0
        } else {
            w.eff_atk(tg, ts, tc, tr)? + W::<C, F>::race_bonus(t, u)
        };
    }
    Ok(if d < a {
        if a < e {
            0
        } else {
            3
        }
    } else if e < d {
        2
    } else {
        0
    })
}

/// 0x2800C0(rec, key, see_all): plan 2 on `key`, or plan 6 onto its cell
/// when 0x27F170 mode 0 answers 1 or 2.
fn attack_or_hold<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    key: i32,
    see_all: i32,
) -> R<()> {
    let tp = w.pos(key >> 8, key & 0xFF);
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let q = trap_ahead(w, g, s, tp & 0xFF, tp >> 8, see_all)?;
    if q == 2 || q == 1 {
        set_plan(rec, 6);
        set_target(rec, tp);
    } else {
        set_plan(rec, 2);
        set_target(rec, key);
    }
    Ok(())
}

/// 0x27F170(g, s, tc, tr, 0, see_all): 5 when an opposing Block Attack
/// applies (0x26CB20 which 0); else, quirk, the cell given to the trap
/// simulator is 0x1E2310(tc, tr), the position of the UNIT in group `tc`,
/// slot `tr` (−1 → 0; columns 0..1 read a board unit, 3..6 give −1, column 2
/// reads the temporary record of the hand display, slot `tr`: the cell of
/// the last summon menu, or −1 once the chain consumed it, see
/// [`crate::duel::Duel::temp_units`]); 0x24CC30 event 0x2E, 0x2D or 0x71 →
/// 1, 0x2C → 2, else 0.
fn trap_ahead<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    tc: i32,
    tr: i32,
    see_all: i32,
) -> R<i32> {
    if w.trap_hook(g, s, 0, see_all != 0)? & 0xFFFF == 1 {
        return Ok(5);
    }
    let answer = |c: i32, r: i32| -> R<i32> {
        Ok(match near_trap(w, g, s, c, r, see_all)? {
            Some(0x2E | 0x2D | 0x71) => 1,
            Some(0x2C) => 2,
            _ => 0,
        })
    };
    let p = w.pos(tc, tr);
    if p < 0 {
        return Ok(0);
    }
    answer(p & 0xFF, p >> 8)
}

/// 0x24CC30's candidate scan without its distance test (applied by
/// [`near_trap`]): units controlled by the side opposing (g, s), class 3
/// (limited-range traps), with +0x57 bit 4 unless `see_all` (flag bit 16 of
/// `(see_all + 1) << 16`); column outer, row inner.
fn near_trap_candidates<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    see_all: i32,
) -> Vec<i32> {
    let ctrl = w.ctrl(g, s);
    let face_only = ((see_all & 0xFF) + 1) & 1 == 1;
    let mut out = Vec::new();
    for col in 0..7 {
        for row in 0..7 {
            let k = w.at_key(col, row);
            if k < 0 {
                continue;
            }
            let (tg, ts) = (k >> 8, k & 0xFF);
            if w.ctrl(tg, ts) == ctrl || w.kind(tg, ts) >> 5 != 3 {
                continue;
            }
            if face_only && !w.flag(tg, ts, 4) {
                continue;
            }
            out.push(k);
        }
    }
    out
}

/// 0x24CC30(g, s, c, r, &out, (see_all + 1) << 16), the limited-range trap
/// simulator, as 0x27F170 mode 0 calls it: the event it would write when
/// its result's low byte is 1 (the out record is local to the caller).
/// - leaders, a mover whose entry 1 has type 7, or a continuous type-0xF
///   unit: nothing;
/// - candidates: opposing class-3 cards within Manhattan 1 of (c, r) (bit 4
///   required unless `see_all`), none → nothing; Royal Decree (0x26C4B0(0,
///   0x2E)) → 0x66;
/// - sorted by 0x24DC50 (summon order, descending), the first that fires
///   by its kind index: 0..=5 → 0x2C (bind), 6..=10 → 0x2D when a monster
///   mover's ATK at (c, r) is below the case's limit, 11 → 0x2E, 12 → 0x2D
///   for a non-monster mover; a covered mover (0x269020) → nothing.
///
/// The leader-ability branch (0x273370 with abilities 0x10 / 0xF / 0xD)
/// answers event 0x71; ability #11 (0x2738C0) answers event 0xFE.
pub(super) fn near_trap<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    c: i32,
    r: i32,
    see_all: i32,
) -> R<Option<u32>> {
    let ctrl = w.ctrl(g, s);
    if s == 0 {
        return Ok(None);
    }
    let enemy = i32::from(ctrl == 0);
    // 0x24CC90..0x24CE30 (the low flag half is 0 for the planner): inside
    // the window of the mover's enemy's leader L (0x273370), a monster
    // whose kind L's #16 (0x273A20) or #15 (0x273980) table names → event
    // 0x71 (result 0x101); else #13 for a mover neither shown (+0x57 bit 2)
    // nor committed face-up (bit 4) → event 0x71 (result 0x10201).
    if w.leader_window(enemy, c, r)? {
        let kind = w.kind(g, s);
        if kind >> 5 == 0 {
            let lk = w.leader_kind(enemy) & 0x1F;
            let k = kind & 0x1F;
            let t16 = match lk {
                0x13 => k == 0x12,
                0x12 => k == 0xA,
                0x11 => k == 2 || k == 0x13,
                0xB => k == 9,
                8 => k == 7,
                _ => false,
            };
            let t15 = match lk {
                0x13 => k == 0,
                0x11 => k == 5 || k == 6,
                0xB => k == 0x10,
                8 => k == 2,
                _ => false,
            };
            if (w.ability(0x10, enemy) && t16) || (w.ability(0xF, enemy) && t15) {
                return Ok(Some(0x71));
            }
        }
        if w.ability(0xD, enemy) && !w.flag(g, s, 2) && !w.flag(g, s, 4) {
            return Ok(Some(0x71));
        }
    }
    // 0x24CE34: 0x252060(g, s, 1) — entry 1 (a monster's), type 7 [F; emu
    // probe o00127: card 242 with entry 1 0x75B answers nothing].
    if w.unit(g, s)
        .and_then(|u| u.entry(1))
        .is_some_and(|[w0, _]| w0 >> 8 == 7)
    {
        return Ok(None);
    }
    if w.continuous_count(0xF) > 0 {
        return Ok(None);
    }
    let mut traps: Vec<i32> = near_trap_candidates(w, g, s, see_all)
        .into_iter()
        .filter(|&k| {
            let p = w.pos(k >> 8, k & 0xFF);
            ((p >> 8) - r).abs() + ((p & 0xFF) - c).abs() < 2
        })
        .collect();
    if traps.is_empty() {
        return Ok(None);
    }
    if w.far_trap_for(0, 0x2E) >= 0 {
        return Ok(Some(0x66));
    }
    let hi = (traps.len() as i32 - 1).max(0);
    sort_by_order(w, &mut traps, 0, hi);
    let mover_class = w.kind(g, s) >> 5;
    for &k in &traps {
        let (tg, ts) = (k >> 8, k & 0xFF);
        // 0x1E1A40: byte 1 of the card record (kind index), −1 without one.
        let index = w
            .unit(tg, ts)
            .and_then(|t| w.card(t.card))
            .map_or(-1, |info| i32::from(info.kind_index));
        let event = match index {
            0..=5 => 0x2C,
            6..=10 => {
                if mover_class != 0 {
                    continue;
                }
                if w.eff_atk(g, s, c, r)? >= NEAR_TRAP_ATK_BELOW[(index - 6) as usize] {
                    continue;
                }
                0x2D
            }
            11 => 0x2E,
            12 => {
                if mover_class == 0 {
                    continue;
                }
                0x2D
            }
            _ => continue,
        };
        // 0x2738C0(g, s, c, r, 0xB): leader ability #11 resists (event
        // 0xFE, after a glow request 0x261DB0 that changes no duel state).
        if w.leader_guard(g, s, c, r, 0xB)? {
            return Ok(Some(0xFE));
        }
        if w.trap_immunity(g, s)? {
            return Ok(None);
        }
        return Ok(Some(event));
    }
    Ok(None)
}

/// 0x24DC50(list, left, right): K&R quicksort of unit keys by summon order
/// (0x1E2640), larger orders first (pivot = middle element).
fn sort_by_order<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    v: &mut [i32],
    left: i32,
    right: i32,
) {
    if left >= right {
        return;
    }
    v.swap(left as usize, ((left + right) >> 1) as usize);
    let mut last = left;
    for i in left + 1..=right {
        let pivot = v[left as usize];
        let x = v[i as usize];
        if w.order(pivot >> 8, pivot & 0xFF) < w.order(x >> 8, x & 0xFF) {
            last += 1;
            v.swap(last as usize, i as usize);
        }
    }
    v.swap(left as usize, last as usize);
    sort_by_order(w, v, left, last - 1);
    sort_by_order(w, v, last + 1, right);
}

/// 0x10E150(rec, a1, mode): monsters with an effect entry (`a1` unused).
/// - Jirai Gumo: 0x10E3D0(rec, 0, 0) when its controller's LP < 1000, then
///   always 0x10DD60(rec, 1, mode) (which overwrites the plan; a target the
///   formation wrote survives only when 0x10DD60 writes none).
/// - Man-Eater Bug: 0x27F060(rec, 0); unless it planned, 0x10DD60.
/// - Great Moth / Perfectly Ultimate Great Moth: 0x27F760 for N = 1, 2 with
///   radius 2 (whatever the mode); results 0, 2, 3 and exhaustion all give
///   plan 3 at the own cell.
/// - Others: 0x10DD60(rec, 1, mode).
pub(super) fn effect_monster<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    _a1: i32,
    mode: u8,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let ctrl = w.ctrl(g, s);
    let my = w.pos(g, s);
    // sp+0x130: the reachable list; sp+0x18C / sp+0x18E: pool counters.
    let mut list = Targets::new();
    let mut pool = Pool::new();
    match w.card_id(g, s) {
        JIRAI_GUMO => {
            if w.lp(ctrl) < JIRAI_GUMO_LP {
                formation(w, rec, 0, 0);
            }
            monster(w, rec, 1, mode)
        }
        MAN_EATER_BUG => {
            if strongest_enemy(w, rec, 0)? == 1 {
                return Ok(());
            }
            monster(w, rec, 1, mode)
        }
        GREAT_MOTH | PERFECT_GREAT_MOTH => {
            let side = i32::from(ctrl == 0);
            for n in 1..=2 {
                // 0x10F670(ctrl) = 0: hidden strength.
                let v = attack_passes(
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
                    0,
                    TRAP_PASS_ATK_LIMIT,
                )?;
                match v {
                    1 => return Ok(()),
                    0 | 2 | 3 => break,
                    _ => continue,
                }
            }
            set_plan(rec, 3);
            set_target(rec, my);
            Ok(())
        }
        _ => monster(w, rec, 1, mode),
    }
}

/// 0x27F060(rec, see_all): Man-Eater Bug's (and Paralyzing Potion's)
/// target. 0 for a monster with +0x57 bit 4 (already flipped); else the
/// strongest opposing non-leader of 0x27DC00(other side, 0x34, 0, 0) by
/// 0x27D1A0; none → 0; else 0x2800C0 on it and 1.
pub(super) fn strongest_enemy<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    see_all: i32,
) -> R<i32> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let ctrl = w.ctrl(g, s);
    if w.kind(g, s) >> 5 == 0 && w.flag(g, s, 4) {
        return Ok(0);
    }
    let enemy = i32::from(ctrl == 0);
    let list = w.list_seen(enemy, 0x34, false, false);
    let k = strongest_of(w, &list, enemy, true)?;
    if k < 0 {
        return Ok(0);
    }
    attack_or_hold(w, rec, k, see_all)?;
    Ok(1)
}

/// 0x27D1A0(list, side) (`bound_ok` = true) and 0x27D080(list, side)
/// (false): the key with the highest 0x252300 ATK at its own cell among
/// list[1..] (quirk: entry 0 is never considered and fewer than two
/// entries give −1) whose +0x57 bit 4 is set, or that `side` controls (and,
/// for 0x27D1A0, with +0x49 and +0x4A zero); ties keep the earlier one.
fn strongest_of<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    list: &[i32],
    side: i32,
    bound_ok: bool,
) -> R<i32> {
    // 0x27DFB0: entries below 0xFFFF.
    let n = list.iter().filter(|&&k| k < 0xFFFF).count();
    if n < 2 {
        return Ok(-1);
    }
    let (mut best, mut pick) = (-1, -1);
    for &k in &list[1..n] {
        let (g, s) = (k >> 8, k & 0xFF);
        let p = w.pos(g, s);
        if !w.flag(g, s, 4) {
            if w.ctrl(g, s) != side {
                continue;
            }
            if bound_ok && (w.spellbind(g, s) != 0 || w.x4a(g, s) != 0) {
                continue;
            }
        }
        let v = w.eff_atk(g, s, p & 0xFF, p >> 8)?;
        if best < v {
            best = v;
            pick = k;
        }
    }
    Ok(pick)
}

/// 0x10E510(rec): magic cards, by entry 0's type (word0 >> 8): 2 → plan 6
/// to the open-area centre 0x27E0B0(u, c, r, terrain 0, more than 6 cells,
/// radius 2) (quirk: the `bgez` tests the result & 0xFFFF, so −1 becomes
/// target 0xFFFF instead of the own cell); 0xD → plan 5; others unchanged.
pub(super) fn magic<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let Some([w0, _]) = u.entry(0) else {
        return Ok(());
    };
    match w0 >> 8 {
        2 => {
            let p = w.pos(g, s) & 0xFFFF;
            let v = open_area(w, u, p & 0xFF, p >> 8, 0, 6, 2)?;
            set_plan(rec, 6);
            set_target(rec, v & 0xFFFF);
        }
        0xD => set_plan(rec, 5),
        _ => (),
    }
    Ok(())
}

/// 0x27E0B0(u, c, r, terrain, min, radius): the centre (row << 8 | col),
/// scanning columns then rows, whose Manhattan-`radius` diamond holds the
/// most cells that are neither `terrain` nor Labyrinth (8): taken when its
/// count ≥ the best count and > `min`, and 0x27AC00(u, buf, c, r, centre,
/// occ 0, 30 moves) is below the best length (initially 14). Quirk: an
/// unreachable centre (−1) wins. −1 when none.
pub(super) fn open_area<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    u: &Unit,
    c: i32,
    r: i32,
    terrain: i32,
    min: i32,
    radius: i32,
) -> R<i32> {
    let (terrain, min, radius) = (terrain & 0xFF, min & 0xFF, radius & 0xFF);
    // sp+0x140: one step buffer for every search (never reset).
    let mut buf = Steps::new();
    let (mut best, mut best_count, mut best_len) = (-1, 0, 14);
    for cc in 0..7 {
        for cr in 0..7 {
            let lo_c = (cc - radius).max(0) & 0xFF;
            let hi_c = if cc + radius < 7 { cc + radius } else { 6 } & 0xFF;
            let lo_r = (cr - radius).max(0) & 0xFF;
            let hi_r = if cr + radius < 7 { cr + radius } else { 6 } & 0xFF;
            let mut count = 0;
            for x in lo_c..=hi_c {
                for y in lo_r..=hi_r {
                    if radius < (y - cr).abs() + (x - cc).abs() {
                        continue;
                    }
                    if w.terrain(x, y) == terrain || w.terrain(x, y) == 8 {
                        continue;
                    }
                    count += 1;
                }
            }
            if count < best_count || count <= min {
                continue;
            }
            // Timer1 is not checked here (0x27AC00 checks it).
            let v = search::path(w, Some(u), &mut buf, c, r, cc, cr, 0, 0x1E)?;
            if v < best_len {
                best_len = v;
                best_count = count;
                best = cr << 8 | cc;
            }
        }
    }
    Ok(best)
}

/// 0x27E390(rec, rounds, see_all, hidden) (called with 2, 0, 0): power-up
/// cards, by the category byte 0x24AEA0(card):
/// - 2 (Cursebreaker): plan 9 on the last own non-leader (0x24E230) bound
///   for good (+0x49 = 0xFF); else on the last visible opposing monster
///   (0x27DC00(other, 0x1D, 0, 0)) whose ATK is not below its printed
///   ATK; else on the last own non-leader (0x27DC00(own, 0x34, 1, 1));
///   else plan 5.
/// - 3 (Paralyzing Potion): 0x27F060(rec, see_all); plan 5 unless it
///   planned and the primary target (a key, or a cell read as a key) is a
///   monster that is not of kind 0xE.
/// - others: rounds 0..`rounds` of the reachable-unit tree over units the
///   card's side controls (0x278A20; the root node, list and count carry
///   over between rounds), keep those 0x249C90 accepts, 0x27D080 → plan 9;
///   else plan 5. (Own units are never enterable on a first move, so only
///   the card itself is ever listed.)
pub(super) fn power_up<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    a1: i32,
    a2: i32,
    a3: i32,
) -> R<()> {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    let Some(u) = w.unit(g, s) else {
        return Ok(());
    };
    let card = w.card_id(g, s);
    let own = w.ctrl(g, s);
    // sp+0x1A0: the reachable list (key 0xFFFF, level 0xFF), once.
    let mut list = Targets::new();
    match w.fusions.category(card as u16) {
        2 => {
            let mut pick = -1;
            for k in own_units(w, own) {
                let (kg, ks) = (k >> 8, k & 0xFF);
                if w.spellbind(kg, ks) != 0xFF {
                    continue;
                }
                let p = w.pos(kg, ks);
                if w.eff_atk(kg, ks, p & 0xFF, p >> 8)? >= 0 {
                    pick = k;
                }
            }
            if pick > 0 {
                set_plan(rec, 9);
                set_target(rec, pick);
                return Ok(());
            }
            let mut pick = -1;
            let enemy = i32::from(own == 0);
            for k in w.list_seen(enemy, 0x1D, a2 & 0xFF != 0, a2 != 0) {
                let (kg, ks) = (k >> 8, k & 0xFF);
                if w.atk(kg, ks, false)? - w.atk(kg, ks, true)? >= 0 {
                    pick = k;
                }
            }
            if pick > 0 {
                set_plan(rec, 9);
                set_target(rec, pick);
                return Ok(());
            }
            let mut pick = -1;
            for k in w.list_seen(own, 0x34, true, true) {
                if w.spellbind(k >> 8, k & 0xFF) >= 0 {
                    pick = k;
                }
            }
            if pick > 0 {
                set_plan(rec, 9);
                set_target(rec, pick);
            } else {
                set_plan(rec, 5);
            }
        }
        3 => {
            if strongest_enemy(w, rec, a2)? != 1 {
                set_plan(rec, 5);
                return Ok(());
            }
            let t = i32::from(rec.target_a);
            let kind = w.kind(t >> 8, t & 0xFF);
            if kind >> 5 != 0 || kind & 0x1F == 0xE {
                set_plan(rec, 5);
            }
        }
        _ => {
            let own = w.ctrl(g, s);
            let my = w.pos(g, s);
            // sp+0x23A / sp+0x23C: pool counters; sp+0xD8: the root node.
            let mut pool = Pool::new();
            let mut root = None;
            for n in 0..(a1 & 0xFF) {
                root = search::reach(
                    w,
                    &mut pool,
                    &mut list,
                    root,
                    Some(u),
                    my & 0xFF,
                    my >> 8,
                    own,
                    n,
                    a2 & 0xFF,
                    a3 & 0xFF,
                )?;
                // Every one of the 22 entries is tested; 0xFFFF keys (no
                // unit) would read the table row before card 0, and the
                // entries they add are never counted by 0x27DFB0 nor reached
                // by 0x27D080, so they are skipped here.
                let mut fits = Vec::new();
                for e in &list.entries {
                    let k = i32::from(e.key);
                    if w.unit(k >> 8, k & 0xFF).is_some()
                        && fits_power_up(w, k >> 8, k & 0xFF, card)?
                    {
                        fits.push(k);
                    }
                }
                let v = strongest_of(w, &fits, own, false)?;
                if v >= 0 {
                    set_plan(rec, 9);
                    set_target(rec, v);
                    return Ok(());
                }
            }
            set_plan(rec, 5);
        }
    }
    Ok(())
}

/// 0x24E230(side, 1, buf): non-leaders (slots 1..10) controlled by `side`,
/// group `side` first, then the other group.
fn own_units<C: CardData + ?Sized, F: FusionData + ?Sized>(w: &W<C, F>, side: i32) -> Vec<i32> {
    let mut out = Vec::new();
    for g in [side, i32::from(side == 0)] {
        for s in 1..11 {
            if w.unit(g, s).is_some() && w.ctrl(g, s) == side {
                out.push(g << 8 | s);
            }
        }
    }
    out
}

/// 0x249C90(g, s, card): whether power-up `card` fits the unit: never a
/// leader of group 0/1; card 0x304 needs a monster of kind 2 (Zombie);
/// other cards read bit (card − 0x2F0) of the unit card's row in 0x29C970
/// (whatever its class). Rows outside the 56 power-up bits are refused.
fn fits_power_up<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    card: i32,
) -> R<bool> {
    if (0..2).contains(&g) && s == 0 {
        return Ok(false);
    }
    if card == ZOMBIE_POWER_UP {
        let kind = w.kind(g, s);
        return Ok(kind >> 5 == 0 && kind & 0x1F == 2);
    }
    let bit = card - POWER_UP_BASE;
    let Some(info) = w.unit(g, s).and_then(|u| w.card(u.card)) else {
        return unsupported("0x249C90 without a unit");
    };
    if !(0..56).contains(&bit) {
        return unsupported(format!("0x249C90 bit {bit} outside the power-up rows"));
    }
    Ok(info.equip_bits[(bit >> 3) as usize] >> (bit & 7) & 1 == 1)
}

/// Weevil's formation cells (0x110120), (row << 8) | col: kind 1 (monsters
/// and rituals) and kind 2 (traps).
const FORMATION_1: [i32; 7] = [0x203, 0x302, 0x301, 0x300, 0x104, 0x203, 0x004];
const FORMATION_2: [i32; 4] = [0x102, 0x201, 0x200, 0x002];

/// 0x110120(kind, i).
pub(super) fn formation_cell(kind: i32, i: i32) -> i32 {
    if kind == 1 {
        return FORMATION_1.get(i as usize).copied().unwrap_or(0x203);
    }
    FORMATION_2.get(i as usize).copied().unwrap_or(0x102)
}

/// 0x10E3D0(rec, kind, defence): primary plan 3 (defence) or 6 towards the
/// first formation cell that is empty or holds this unit.
pub(super) fn formation<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &World<C, F>,
    rec: &mut Record,
    kind: i32,
    defence: i32,
) {
    let (g, s) = (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF));
    if w.unit(g, s).is_none() {
        return;
    }
    rec.plan_a = if defence == 1 { 3 } else { 6 };
    rec.plan_b = 0xFF;
    let n = if kind == 1 { 6 } else { 4 };
    for i in 0..n {
        let cell = formation_cell(kind, i);
        let k = w.at_key(cell & 0xFF, cell >> 8);
        if k == i32::from(rec.key) || k < 0 {
            rec.target_a = cell as u16;
            return;
        }
    }
}

/// Private function-level probes: with `ROSES_WEEVIL_PROBES` set (and
/// `ROSES_WEEVIL_ORACLE` = the oracle directory), every
/// `probes/b/<case>.json` (written by the private emulator tools) lists
/// calls of 0x10D6F0 and its callees on the synced memory of a case, with
/// the record (and +0x1A1) the original left.
#[cfg(test)]
mod probe_tests {
    use super::super::tests::{load_json, memory_of, CaseDuel, CaseMemory, Oracle};
    use super::*;
    use crate::duel::DuelError;

    type Call = (String, usize, i32, i32, Option<[u16; 7]>, serde_json::Value);

    /// `duel` / `tier` are embedded for synthetic boards (no case file).
    #[derive(serde::Deserialize)]
    struct Probes {
        case: String,
        #[serde(default)]
        duel: Option<CaseDuel>,
        #[serde(default)]
        tier: Option<i16>,
        synced: CaseMemory,
        calls: Vec<Call>,
    }

    fn row(r: &Record) -> [u16; 7] {
        [
            r.key,
            r.order,
            u16::from(r.plan_b),
            u16::from(r.plan_a),
            r.target_b,
            r.target_a,
            u16::from(r.bits),
        ]
    }

    #[test]
    fn private_plan_probes_match_the_original() {
        if std::env::var_os("ROSES_WEEVIL_PROBES").is_none() {
            return;
        }
        let Some(o) = Oracle::load() else {
            return;
        };
        let (mut ok, mut bad, mut refused) =
            (0usize, Vec::new(), std::collections::BTreeMap::new());
        let mut per_fn = std::collections::BTreeMap::<String, [usize; 3]>::new();
        for path in o.files("probes/b") {
            let p: Probes = load_json(&path);
            let (duel, tier) = match &p.duel {
                Some(d) => (o.duel(d), p.tier.unwrap_or(0)),
                None => {
                    let case = o.case(&p.case);
                    (o.duel(&case.duel), case.tier)
                }
            };
            let w = World::new(&duel, &o.cards, &o.tables, tier).unwrap();
            let synced = memory_of(&p.synced);
            for (f, idx, a1, a2, want, x1a1) in &p.calls {
                let Some(want) = want else { continue };
                let mut m = synced.clone();
                let i = *idx;
                let res = match f.as_str() {
                    "10d6f0" => super::super::assign(&w, &mut m, i),
                    "10dd60" => monster(&w, &mut m.records[i], *a1, *a2 as u8),
                    "10e150" => effect_monster(&w, &mut m.records[i], *a1, *a2 as u8),
                    "10e510" => magic(&w, &mut m.records[i]),
                    "27e390" => power_up(&w, &mut m.records[i], *a1, *a2, 0),
                    "10e3d0" => {
                        formation(&w, &mut m.records[i], *a1, *a2);
                        Ok(())
                    }
                    _ => continue,
                };
                let e = per_fn.entry(f.clone()).or_default();
                match res {
                    Err(DuelError::Unsupported(why)) => {
                        e[2] += 1;
                        // Grouped by reason and case family (t, r, c, syn...).
                        let family: String =
                            p.case.chars().take_while(|c| !c.is_ascii_digit()).collect();
                        *refused.entry(format!("{why} [{family}]")).or_insert(0usize) += 1;
                    }
                    Err(err) => {
                        e[1] += 1;
                        bad.push(format!("{} {f}[{i}] {a1} {a2}: error {err}", p.case));
                    }
                    Ok(()) => {
                        let got = row(&m.records[i]);
                        let x_ok = f != "10d6f0" || x1a1.as_u64() == Some(u64::from(m.x1a1));
                        if &got == want && x_ok {
                            ok += 1;
                            e[0] += 1;
                        } else {
                            e[1] += 1;
                            bad.push(format!(
                                "{} {f}[{i}] {a1} {a2}: {got:x?} vs {want:x?} (x1a1 {} vs {x1a1})",
                                p.case, m.x1a1
                            ));
                        }
                    }
                }
            }
        }
        eprintln!(
            "plan probes: {ok} match, {} differ, {} refused",
            bad.len(),
            refused.values().sum::<usize>()
        );
        for (f, [a, b, c]) in &per_fn {
            eprintln!("  {f}: {a} match, {b} differ, {c} refused");
        }
        for (why, n) in &refused {
            eprintln!("REFUSED {n} × {why}");
        }
        for b in bad.iter().take(80) {
            eprintln!("DIFF {b}");
        }
        assert!(bad.is_empty(), "{} plan probes differ", bad.len());
    }
}
