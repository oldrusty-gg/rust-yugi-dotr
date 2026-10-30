//! Plan executors: turn a record's plan into a type-6 action (0x110FC0,
//! 0x1108C0, 0x110330, 0x27E960) with their helpers 0x27FCA0 (position
//! request, +0x10) and 0x10F6D0 (reveal request, +0xC).
//!
//! Each executor writes the 0x14-byte action record exactly as the EE does;
//! fields it leaves unwritten keep the caller's values. Quirks kept:
//! - 0x110FC0, 0x1108C0 and 0x27E960 pass the SIDE NUMBER as the reference
//!   key of the danger score 0x278600 (so the reference is unit (0, side));
//!   only 0x110330 passes the unit's own key.
//! - The see-all flag of the executors' danger calls is the module's
//!   hidden-strength function & 0xFF ([`see_all`]): Weevil's inlined
//!   (LP ≥ 0x5DD && monsters ≥ 4) ? 0 : 0, i.e. always 0.
//! - 0x27E960 passes (best score so far & 0xFF) as both the radius and the
//!   see-all flag of 0x278600, its search target is the whole word at
//!   rec +8, and its tie-break reads the target KEY as a position (row =
//!   group, col = slot).
//! - 0x1108C0 leaves +0xC unwritten on its immobile path and clears the
//!   primary target (+0xA) when the target is gone, whatever `which` is.
//!
//! The other opponent modules carry their own copies of these executors
//! that differ only in constants and callees (Pegasus: 0x138880, 0x1382C0,
//! 0x137DA0 with the reveal test 0x139A50 and the charge test 0x139950);
//! those differences come in through [`Opponent`].

use super::search::{self, Steps};
use super::world::{flag, unsupported, Unit, World, R};
use super::{Decision, FusionData, Opponent, Record};
use crate::duel::{CardData, DuelError};

type W<'a, C, F> = World<'a, C, F>;

/// The see-all flag the executors pass to 0x278600: the module's hidden
/// strength function `& 0xFF` ([`Opponent::hidden_value`]): Weevil's
/// 0x10F670 is inlined as (LP(side) ≥ 0x5DD && monsters(side) ≥ 4) ? 0 : 0
/// (dead code, always 0), Labyrinth-ruler's 0x1353B0 gives a non-zero
/// byte (0x279AF0 tests the flag with `beqz`, so any non-zero value sees
/// everything).
fn see_all<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
) -> R<bool> {
    Ok(O::hidden_value(w, side)? & 0xFF != 0)
}

/// (group, slot) of the record's unit.
fn unit_of(rec: &Record) -> (i32, i32) {
    (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF))
}

/// 0x1E07C0 for the acting unit (the executors only run for units on the
/// board).
fn mover<'w, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &'w W<C, F>,
    g: i32,
    s: i32,
) -> R<&'w Unit> {
    w.unit(g, s).ok_or(DuelError::InvalidCommand(
        "plan executor for a missing unit",
    ))
}

/// 0x13D400 / 0x13C910 (York's copies of 0x110FC0 / 0x110330, under
/// [`Opponent::FACE_UP_SUPPORT_PASSES`]): a committed face-up (+0x57 bit 4)
/// magic (class 1) or ritual (class 5) card taking the movement path is marked acted
/// by the module itself (0x1E30F0(g, s, 0, 1)) and the record becomes an
/// empty type-0xE one; the plan slot is left as it is. True when it applied.
fn face_up_support_passes<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &Record,
    d: &mut Decision,
) -> bool {
    if !O::FACE_UP_SUPPORT_PASSES {
        return false;
    }
    let (g, s) = unit_of(rec);
    if !w.flag(g, s, flag::COMMITTED_FACE_UP) {
        return false;
    }
    let class = w.kind(g, s) >> 5;
    if class != 1 && class != 5 {
        return false;
    }
    d.mark_acted = Some(rec.key);
    d.key = rec.key;
    d.kind = 0xE;
    true
}

/// (col, row) of a candidate byte (row << 4) | col.
fn nibbles(b: u8) -> (i32, i32) {
    (i32::from(b & 0xF), i32::from(b >> 4))
}

/// (row << 8) | col of a candidate byte, as the executors store it at +4.
fn cell_of(b: u8) -> u16 {
    u16::from(b >> 4) << 8 | u16::from(b & 0xF)
}

/// Manhattan distance between a candidate byte and the u16 `t` read as
/// (row << 8) | col (unsigned byte compares in the EE).
fn dist_to(b: u8, t: u16) -> i32 {
    let (c, r) = nibbles(b);
    (c - i32::from(t & 0xFF)).abs() + (r - i32::from(t >> 8)).abs()
}

/// The common part of every exit: actor, type 6, destination.
fn act(d: &mut Decision, rec: &Record, dest: u16) {
    d.key = rec.key;
    d.kind = 6;
    d.target = dest;
}

/// 0x27FCA0(g, s, dest, defflag, see_all): the position request (+0x10):
/// 0 attack, 1 defence, 2 keep. Keep for a leader or a move; otherwise
/// from the Gorgon's Eye / Shadow of Eyes class (0x27F170 mode 4; mode 5
/// is evaluated and its result dropped), committed defence and ATK < DEF.
pub(super) fn stance<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    dest: u16,
    defflag: u8,
    see_all: bool,
) -> R<u8> {
    if s == 0 || i32::from(dest) != w.pos(g, s) {
        return Ok(2);
    }
    let s0 = w.classify(g, s, 4, see_all)?;
    // 0x27fd34: the mode-5 result is overwritten unused.
    w.classify(g, s, 5, see_all)?;
    let trapped = s0 == 2 || s0 == 5;
    if w.flag(g, s, flag::COMMITTED_DEFENCE) {
        // Monsters also compare ATK and DEF here; every branch agrees.
        return Ok(if trapped { 0 } else { 2 });
    }
    if w.kind(g, s) >> 5 != 0 && !trapped {
        return Ok(1);
    }
    // 0x1E1510 / 0x1E15F0 with the +0x38 / +0x3A bonuses.
    if w.atk(g, s, false)? < w.def(g, s, false)? || defflag == 1 {
        return Ok(if trapped { 2 } else { 1 });
    }
    Ok(2)
}

/// 0x10F6D0(g, s, rec): the reveal request (+0xC; the emitter only acts on
/// 1). 0 once committed face-up; by class (jump table 0x2BA2F0): monsters
/// 1 for cards 0x1AB, 0x1AC, 0x192, 0x193; magic 1 for effect type 0xD, or
/// type 2 when the primary target (+0xA) is the own cell, unless a type-0xE
/// trap applies (0x27F170 mode 6); rituals 1 when 0x110210 allows it and
/// the own cell is not Crush; everything else 2.
pub(super) fn reveal<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    rec: &Record,
) -> R<u32> {
    let pos = w.pos(g, s);
    if w.flag(g, s, flag::COMMITTED_FACE_UP) {
        return Ok(0);
    }
    let card = w.card_id(g, s);
    Ok(match w.kind(g, s) >> 5 {
        0 => {
            if (0x1AB..=0x1AC).contains(&card) || (0x192..=0x193).contains(&card) {
                1
            } else {
                2
            }
        }
        1 => {
            if w.classify(g, s, 6, false)? == 5 {
                2
            } else {
                // 0x252060(g, s, 0): null when absent.
                match w.unit(g, s).and_then(|u| u.entry(0)) {
                    None => 2,
                    Some([w0, _]) => match w0 >> 8 {
                        2 if i32::from(rec.target_a) == w.pos(g, s) => 1,
                        0xD => 1,
                        _ => 2,
                    },
                }
            }
        }
        5 => {
            if w.classify(g, s, 6, false)? == 5
                || ritual_ready(w, w.ctrl(g, s), card)? != 1
                || w.terrain(pos & 0xFF, pos >> 8) == 9
            {
                2
            } else {
                1
            }
        }
        // Classes 2..4, and ≥ 6 unsigned (incl. −1 without a unit).
        _ => 2,
    })
}

/// 0x110210(side, card) (`summon::ritual_ok`).
fn ritual_ready<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    side: i32,
    card: i32,
) -> R<i32> {
    #[cfg(test)]
    if let Some(v) = tests::replay(&[0x110210, side.into(), card.into()], None)? {
        return Ok(v);
    }
    super::summon::ritual_ok(w, side, card)
}

/// 0x27AB70 (`search::path_k`).
#[allow(clippy::too_many_arguments)]
fn path<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    occ: i32,
    maxlen: i32,
) -> R<i32> {
    #[cfg(test)]
    {
        let k = [0x27AB70, g, s, fc, fr, tc, tr, occ & 0xFFFF, maxlen & 0xFF].map(i64::from);
        if let Some(v) = tests::replay(&k, Some(buf))? {
            return Ok(v);
        }
    }
    search::path_k(w, g, s, buf, fc, fr, tc, tr, occ, maxlen)
}

/// 0x27B9D0 (`search::attack_path_k`).
#[allow(clippy::too_many_arguments)]
fn attack_path<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    buf: &mut Steps,
    fc: i32,
    fr: i32,
    tc: i32,
    tr: i32,
    target: u32,
    see_all: i32,
    maxlen: i32,
    hidden: i32,
) -> R<i32> {
    #[cfg(test)]
    {
        let k = [
            0x27B9D0,
            i64::from(g),
            i64::from(s),
            i64::from(fc),
            i64::from(fr),
            i64::from(tc),
            i64::from(tr),
            i64::from(target),
            i64::from(see_all as u32),
            i64::from(maxlen & 0xFF),
            i64::from(hidden & 0xFFFF),
        ];
        if let Some(v) = tests::replay(&k, Some(buf))? {
            return Ok(v);
        }
    }
    search::attack_path_k(
        w, g, s, buf, fc, fr, tc, tr, target, see_all, maxlen, hidden,
    )
}

/// 0x110FC0(rec, side, d, which, defflag): plans 3 (`defence`) and 6. Move
/// towards the cell in the plan target: of the first moves of the shortest
/// routes (0x27AB70, at most 7 moves; the target cell's occupant may be
/// entered), the empty one with the lowest danger (0x278600 radius 2,
/// assumed strength `O::DANGER_HIDDEN`), ties to the one closer to the
/// target cell. Stays when there is none. The target is always cleared.
/// Pegasus's 0x138880 is the same with strength 0, 0x27FCA0 see-all 1 and
/// its own reveal test.
pub(super) fn go_to_cell<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    defence: bool,
) -> R<()> {
    go_to_cell_with::<O, _, _>(w, rec, side, d, which, defence, false)
}

/// [`go_to_cell`] with the route search selectable: Jasper's 0x158320
/// (Seto's 0x116BC0 plus a seventh argument t1) searches with 0x27B9D0
/// instead of 0x27AB70 when `attack_route` is set (his plan 7): see-all 0,
/// at most 30 moves, the target argument 0xFFFF and the module's hidden
/// strength (`O::hidden_value(side) & 0xFFFF`); everything else is shared.
pub(super) fn go_to_cell_with<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    defence: bool,
    attack_route: bool,
) -> R<()> {
    let (g, s) = unit_of(rec);
    let defflag = u8::from(defence);
    let pos = w.pos(g, s);
    let t = rec.target(which);
    let mut pick = None;
    // 0x111040: a free target stays at once only when the position is
    // 0xFFFF too; otherwise it searches towards (255, 255) and fails.
    if !(t == 0xFFFF && pos == 0xFFFF) {
        // 0x13D364 / 0x13D400: after the invalid-target guard, before the
        // search. A valid target equal to the current cell also takes this path.
        if face_up_support_passes::<O, _, _>(w, rec, d) {
            return Ok(());
        }
        let (tc, tr) = (i32::from(t & 0xFF), i32::from(t >> 8));
        let occ = match w.at_key(tc, tr) {
            k if k < 0 => 0xFFFF,
            k => k,
        };
        let mut buf = Steps::new();
        if attack_route {
            // 0x1585B4..0x158614: the occupant above is computed and unused.
            let hidden = O::hidden_value(w, side)? & 0xFFFF;
            attack_path(
                w,
                g,
                s,
                &mut buf,
                pos & 0xFF,
                pos >> 8,
                tc,
                tr,
                0xFFFF,
                0,
                30,
                hidden,
            )?;
        } else {
            path(
                w,
                g,
                s,
                &mut buf,
                pos & 0xFF,
                pos >> 8,
                tc,
                tr,
                occ & 0xFFFF,
                7,
            )?;
        }
        let n = search::step_count(&buf);
        let mut best = 0xFFFF;
        let mut bi: Option<usize> = None;
        for i in 0..n.max(0) as usize {
            let b = buf.0[i];
            let (c, r) = nibbles(b);
            if w.at(c, r).is_some() {
                continue;
            }
            // Quirk: the side number as the reference key.
            let all = see_all::<O, _, _>(w, side)?;
            let score = w.danger(c, r, side, O::danger_radius(w), O::DANGER_HIDDEN, all)?;
            if score < best {
                best = score;
                bi = Some(i);
            } else if score == best {
                let Some(j) = bi else {
                    return unsupported("0x110FC0: tie with no best candidate");
                };
                if dist_to(b, t) < dist_to(buf.0[j], t) {
                    bi = Some(i);
                }
            }
        }
        pick = bi.map(|i| cell_of(buf.0[i]));
    }
    act(d, rec, pick.unwrap_or(pos as u16));
    d.position = stance(w, g, s, d.target, defflag, O::STANCE_SEE_ALL)?;
    d.reveal = O::reveal(w, g, s, rec)?;
    O::clear_plan(rec, which);
    Ok(())
}

/// 0x1108C0(rec, side, d, which): plan 2 (the target is a unit key);
/// −1 when the target is gone. An immobile unit stays and asks for attack
/// position when its cell's danger (radius 3) is below its ATK there.
/// Otherwise it takes the first moves of the shortest attack routes
/// (0x27B9D0, at most 30 moves), occupied ones included, with the lowest
/// danger (radius 2), ties to the one closer to the target; a unit two
/// cells from its target on favoured terrain (not card 0x1A1, not acted)
/// jumps onto it and asks to reveal. The target is kept.
///
/// Pegasus's 0x1382C0: danger strength 0, search and 0x27FCA0 see-all 1,
/// the charge test 0x139950, and an immobile unit asks 0x27FCA0 (defence
/// unless its cell's danger is below its ATK 0x1E1510) and clears its
/// target (`O::IMMOBILE_STANCE`).
pub(super) fn attack<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<i32> {
    let t = rec.target(which);
    let (tg, ts) = (i32::from(t >> 8), i32::from(t & 0xFF));
    if w.unit(tg, ts).is_none() {
        // 0x1E07C0 accepts group 2: a written hand-display record (a stale
        // cell word as the plan-2 target) is attacked as a unit.
        if tg == 2 && w.pos(tg, ts) >= 0 {
            return unsupported("plan 2 on a group-2 record (the hand display)");
        }
        // Quirk (0x110950): always the primary target.
        rec.consume(1);
        return Ok(-1);
    }
    let (g, s) = unit_of(rec);
    let flag0 = O::reveal(w, g, s, rec)?;
    let pos = w.pos(g, s);
    let tpos = w.pos(tg, ts);
    let (c, r) = (pos & 0xFF, pos >> 8);
    if w.allowance(mover(w, g, s)?, c, r)? == 0 {
        act(d, rec, pos as u16);
        let all = see_all::<O, _, _>(w, side)?;
        let danger = w.danger(c, r, side, O::STATIC_DANGER_RADIUS, O::DANGER_HIDDEN, all)?;
        if O::IMMOBILE_STANCE {
            let defflag = u8::from(danger >= w.atk(g, s, false)?);
            d.position = stance(w, g, s, d.target, defflag, O::STANCE_SEE_ALL)?;
            O::clear_plan(rec, which);
        } else {
            d.position = if danger < w.eff_atk(g, s, c, r)? {
                0
            } else {
                1
            };
        }
        // +0xC is not written on this path.
        return Ok(0);
    }
    let (tc, tr) = (tpos & 0xFF, tpos >> 8);
    let mut buf = Steps::new();
    // The search's hidden strength: 0 in 0x1108C0 / 0x1382C0, 0x1353B0(side)
    // & 0xFFFF in 0x1344B0.
    let hidden = O::hidden_value(w, side)? & 0xFFFF;
    attack_path(
        w,
        g,
        s,
        &mut buf,
        c,
        r,
        tc,
        tr,
        u32::from(t),
        O::SEARCH_SEE_ALL,
        30,
        hidden,
    )?;
    let n = search::step_count(&buf);
    if n <= 0 {
        act(d, rec, pos as u16);
        d.reveal = flag0;
        d.position = stance(w, g, s, d.target, 0, O::STANCE_SEE_ALL)?;
        if O::IMMOBILE_STANCE {
            rec.consume(which);
        }
        return Ok(0);
    }
    let from_target = |b: u8| {
        let (cc, cr) = nibbles(b);
        (cc - tc).abs() + (cr - tr).abs()
    };
    let mut best = 0xFFFF;
    let mut bi: Option<usize> = None;
    let mut reveal1 = 0;
    for i in 0..n as usize {
        // 0x110c2c: two cells from the target (own cell, not the move).
        if O::charge(w, g, s, c, r, tc, tr)? {
            reveal1 = 1;
            buf.0[i] = ((tr & 0xF) << 4 | (tc & 0xF)) as u8;
            bi = Some(i);
            break;
        }
        let b = buf.0[i];
        let (cc, cr) = nibbles(b);
        let all = see_all::<O, _, _>(w, side)?;
        let score = w.danger(cc, cr, side, O::danger_radius(w), O::DANGER_HIDDEN, all)?;
        if score < best {
            best = score;
            bi = Some(i);
        } else if score == best {
            let Some(j) = bi else {
                return unsupported("0x1108C0: tie with no best candidate");
            };
            if from_target(b) < from_target(buf.0[j]) {
                bi = Some(i);
            }
        }
    }
    match bi {
        Some(i) => {
            act(d, rec, cell_of(buf.0[i]));
            d.reveal = reveal1;
            d.position = stance(w, g, s, d.target, 0, O::STANCE_SEE_ALL)?;
            if O::ATTACK_MOVE_CLEARS {
                O::clear_plan(rec, which);
            }
        }
        None => {
            act(d, rec, pos as u16);
            d.reveal = reveal1;
            d.position = stance(w, g, s, d.target, 0, O::STANCE_SEE_ALL)?;
            // 0x138838: Pegasus clears the target when no candidate won.
            if O::IMMOBILE_STANCE {
                rec.consume(which);
            }
        }
    }
    Ok(0)
}

/// 0x110c2c..0x110cc4: the unit may jump onto its target: on the board,
/// not card 0x1A1, not acted, and favoured by the terrain of its own cell
/// (0x24F820 = 1).
pub(super) fn charge<C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    g: i32,
    s: i32,
    c: i32,
    r: i32,
) -> R<bool> {
    if w.unit(g, s).is_none() || w.card_id(g, s) == 0x1A1 || w.flag(g, s, flag::ACTED) {
        return Ok(false);
    }
    Ok(w.terrain_favour_of(w.terrain(c, r), g, s)? == 1)
}

/// 0x110330(rec, side, d, which): plan 5 (retreat to the safest cell).
/// Stays when the own cell's danger (0x278600 with the unit's own key,
/// radius 3) is 0 or the unit cannot move. Otherwise scans the cells
/// within its range column by column (the own cell included) that
/// 0x27B580 allows (own cells enterable for a threatened leader) and, for
/// non-leaders, that no enemy threatens (0x27D750); lowest danger wins,
/// ties to an empty cell over an occupied best. The target is cleared.
/// Pegasus's 0x137DA0 passes see-all 1 to the danger, threat and position
/// queries (`O::RETREAT_SEE_ALL`, `O::STANCE_SEE_ALL`).
pub(super) fn retreat<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    retreat_with::<O, _, _>(w, rec, side, d, which, None)
}

/// [`retreat`] with a fixed reveal request instead of the module's reveal
/// function: Darkness-ruler's 0x12F640 (plan 0xC) is 0x110330 writing
/// +0xC = 2 on every exit.
pub(super) fn retreat_with<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    reveal: Option<u32>,
) -> R<()> {
    // `side` feeds the hidden-strength function: Weevil's inlined 0x10F670
    // (its dead see-all computation), Labyrinth-ruler's 0x1353B0 whose
    // value & 0xFFFF is the danger queries' hidden strength (0x133F70).
    let see_all = O::RETREAT_SEE_ALL;
    let hidden = O::hidden_value(w, side)? & 0xFFFF;
    let (g, s) = unit_of(rec);
    let key = i32::from(rec.key);
    let pos = w.pos(g, s);
    let (c, r) = (pos & 0xFF, pos >> 8);
    let mut pick = None;
    if w.danger(c, r, key, O::STATIC_DANGER_RADIUS, hidden, see_all)? != 0 {
        let allow = w.allowance(mover(w, g, s)?, c, r)?;
        if allow != 0 {
            // 0x13C910: before the scan of the cells in range.
            if face_up_support_passes::<O, _, _>(w, rec, d) {
                return Ok(());
            }
            let range = allow & 0xF;
            // 0x252180(0, x) = max(0, x); the far edges are capped at 6.
            let c0 = (c - range).max(0) & 0xFF;
            let c1 = if c + range < 7 { c + range } else { 6 } & 0xFF;
            let r0 = (r - range).max(0) & 0xFF;
            let r1 = if r + range < 7 { r + range } else { 6 } & 0xFF;
            // 0x27D500(g, 0, see_all) for the leader, 0 otherwise.
            let own_ok = s == 0 && w.threatened(g, 0, see_all)?;
            let mut best = 0xFFFF;
            let mut bc: i32 = 0xFF;
            for cc in c0..=c1 {
                for rr in r0..=r1 {
                    if range < (cc - c).abs() + (rr - r).abs() {
                        continue;
                    }
                    // 0x27B580: the occupant argument points at the record,
                    // whose first u16 is the unit's own key.
                    if !w.move_ok_k(g, s, c, r, cc, rr, key, own_ok)? {
                        continue;
                    }
                    if w.cell_threatened(g, s, cc, rr, see_all)? && s != 0 {
                        continue;
                    }
                    let score = w.danger(cc, rr, key, O::STATIC_DANGER_RADIUS, hidden, see_all)?;
                    if score < best
                        || (score == best
                            && w.at(cc, rr).is_none()
                            && w.at(bc & 0xFF, bc >> 8).is_some())
                    {
                        best = score;
                        bc = rr << 8 | cc;
                    }
                }
            }
            if bc != 0xFF {
                pick = Some(bc as u16);
            }
        }
    }
    act(d, rec, pick.unwrap_or(pos as u16));
    // (0x1381E0: Pegasus asks with defflag 0 after a move to another cell;
    // 0x27FCA0 answers 2 for any other cell before reading the flag.)
    d.position = stance(w, g, s, d.target, 1, O::STANCE_SEE_ALL)?;
    d.reveal = match reveal {
        Some(v) => v,
        None => O::reveal(w, g, s, rec)?,
    };
    O::clear_plan(rec, which);
    Ok(())
}

/// 0x27E960(rec, side, d, which, see_all, 0, 2, 1): plan 9 (move onto a
/// unit to equip it; the target is a unit key). Of the first moves of the
/// shortest attack routes towards the target's cell (0x27B9D0, at most 3
/// moves), the empty one with the lowest danger, ties to the one closer to
/// the target key read as a position. +0x10 = +0xC = 2; the target is
/// cleared. Weevil's call sites pass see-all 0, Pegasus's 1
/// (`O::EQUIP_SEARCH_SEE_ALL`).
pub(super) fn equip_move<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
) -> R<()> {
    // The call site's arguments: t0 = the module's search see-all, t1 =
    // the module's hidden strength function (0x10F670(side) = 0 in Weevil,
    // 0 in Pegasus, 0x1353B0(side) in Labyrinth-ruler; the search's hidden
    // strength), t2 unused, t3 = 1 (danger hidden).
    let search_see_all: i32 = O::EQUIP_SEARCH_SEE_ALL;
    let search_hidden = O::hidden_value(w, side)?;
    const HIDDEN: i32 = 1;
    let (g, s) = unit_of(rec);
    let pos = w.pos(g, s);
    let t = rec.target(which);
    let tpos = w.pos(i32::from(t >> 8), i32::from(t & 0xFF));
    let mut pick = None;
    if !(t == 0xFFFF && pos == 0xFFFF) {
        let mut buf = Steps::new();
        // Quirk (0x27eb2c): the target argument is the whole word at +8.
        attack_path(
            w,
            g,
            s,
            &mut buf,
            pos & 0xFF,
            pos >> 8,
            tpos & 0xFF,
            tpos >> 8,
            rec.targets(),
            search_see_all,
            3,
            search_hidden,
        )?;
        let n = search::step_count(&buf);
        let mut best: i32 = 0xFFFF;
        let mut bi: Option<usize> = None;
        for i in 0..n.max(0) as usize {
            let b = buf.0[i];
            let (c, r) = nibbles(b);
            if w.at(c, r).is_some() {
                continue;
            }
            // Quirk (0x27eb9c): radius and see-all = best score & 0xFF.
            let score = w.danger(c, r, side, best & 0xFF, HIDDEN, best & 0xFF != 0)?;
            if score < best {
                best = score;
                bi = Some(i);
            } else if score == best {
                let Some(j) = bi else {
                    return unsupported("0x27E960: tie with no best candidate");
                };
                // Quirk: the key as (row << 8) | col.
                if dist_to(b, t) < dist_to(buf.0[j], t) {
                    bi = Some(i);
                }
            }
        }
        pick = bi.map(|i| cell_of(buf.0[i]));
    }
    act(d, rec, pick.unwrap_or(pos as u16));
    d.position = 2;
    d.reveal = 2;
    rec.consume(which);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    type Replay = Vec<(Vec<i64>, Option<[u8; 12]>, i32)>;

    thread_local! {
        /// Recorded EE results of the searches / 0x110210 for the call
        /// under test; `None` = call the real functions.
        static REPLAY: RefCell<Option<Replay>> = const { RefCell::new(None) };
    }

    /// The recorded result of the call `k` (filling `buf`), `None` when no
    /// replay is active; an error when the port makes a call the EE did
    /// not make.
    pub(super) fn replay(k: &[i64], buf: Option<&mut Steps>) -> R<Option<i32>> {
        REPLAY.with(|r| {
            let r = r.borrow();
            let Some(list) = r.as_ref() else {
                return Ok(None);
            };
            match list.iter().find(|(a, _, _)| a.as_slice() == k) {
                Some((_, b, v)) => {
                    if let (Some(dst), Some(src)) = (buf, b) {
                        dst.0 = *src;
                    }
                    Ok(Some(*v))
                }
                None => Err(DuelError::InvalidCommand("replay: call not made by the EE")),
            }
        })
    }

    // ---- private probes (ROSES_WEEVIL_PROBES) -------------------------------

    use crate::duel::{CardInfo, Duel, DuelUnit, SideState};

    /// The executors never read the fusion tables.
    struct NoFusions;
    impl FusionData for NoFusions {
        fn fuse(&self, _a: u16, _b: u16) -> Option<u16> {
            None
        }
        fn equip(&self, _monster: u16, _card: u16) -> i32 {
            -1
        }
        fn category(&self, _card: u16) -> u8 {
            0
        }
        fn value(&self, _card: u16) -> u16 {
            0
        }
    }

    #[derive(serde::Deserialize)]
    struct BoardDuel {
        terrain: Vec<u8>,
        units: Vec<DuelUnit>,
        sides: [SideState; 2],
        round: u8,
        first: u8,
        active: u8,
    }

    #[derive(serde::Deserialize)]
    struct Call {
        ex: String,
        rec: usize,
        which: u8,
        target: u16,
        def: u8,
        before: [u16; 7],
        ret: i32,
        d: String,
        after: [u16; 7],
        hooks: Vec<(i64, Vec<i64>, Option<String>, i32)>,
    }

    #[derive(serde::Deserialize)]
    struct Board {
        name: String,
        side: u8,
        tier: i16,
        duel: BoardDuel,
        calls: Vec<Call>,
    }

    fn record(v: &[u16; 7]) -> Record {
        Record {
            key: v[0],
            order: v[1],
            plan_b: v[2] as u8,
            plan_a: v[3] as u8,
            target_b: v[4],
            target_a: v[5],
            bits: v[6] as u8,
        }
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    /// The EE action record fields (+0xB is never written by the
    /// executors and not initialised by the probe).
    fn decision(b: &[u8]) -> Decision {
        Decision {
            key: u16::from_le_bytes([b[0], b[1]]),
            kind: b[2],
            target: u16::from_le_bytes([b[4], b[5]]),
            hand: [b[6], b[7], b[8], b[9], b[10]],
            reveal: u32::from_le_bytes([b[12], b[13], b[14], b[15]]),
            position: b[16],
            mark_acted: None,
        }
    }

    fn run_call<C: CardData + ?Sized, F: FusionData + ?Sized>(
        w: &W<C, F>,
        side: i32,
        c: &Call,
        replay: bool,
    ) -> (R<i32>, Record, Decision) {
        let mut rec = record(&c.before);
        let mut d = Decision::new();
        let list = replay.then(|| {
            c.hooks
                .iter()
                .map(|(a, args, buf, v)| {
                    let mut k = vec![*a];
                    k.extend(args);
                    let b = buf.as_ref().map(|s| hex(s).try_into().unwrap());
                    (k, b, *v)
                })
                .collect()
        });
        REPLAY.with(|r| *r.borrow_mut() = list);
        use super::super::Weevil;
        let res = match c.ex.as_str() {
            "go" => go_to_cell::<Weevil, _, _>(w, &mut rec, side, &mut d, c.which, c.def == 1)
                .map(|()| 0),
            "attack" => attack::<Weevil, _, _>(w, &mut rec, side, &mut d, c.which),
            "retreat" => retreat::<Weevil, _, _>(w, &mut rec, side, &mut d, c.which).map(|()| 0),
            "equip" => equip_move::<Weevil, _, _>(w, &mut rec, side, &mut d, c.which).map(|()| 0),
            _ => Err(DuelError::InvalidCommand("unknown executor")),
        };
        REPLAY.with(|r| *r.borrow_mut() = None);
        (res, rec, d)
    }

    /// Set `ROSES_WEEVIL_PROBES` to the port directory (holding
    /// `data/cards.json` and `probes/a2/*.json` from `probes_exec.py`);
    /// `ROSES_WEEVIL_CASE` filters board names. Replay mode feeds the EE's
    /// search / 0x110210 results and must match; the real-search mode only
    /// reports.
    #[test]
    fn private_executor_probes_match_the_original() {
        let Some(root) = std::env::var_os("ROSES_WEEVIL_PROBES") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let read = |p: &std::path::Path| std::fs::read_to_string(p).unwrap();
        let cards: Vec<CardInfo> =
            serde_json::from_str(&read(&root.join("data/cards.json"))).unwrap();
        let filter = std::env::var("ROSES_WEEVIL_CASE").ok();
        let mut paths: Vec<_> = std::fs::read_dir(root.join("probes/a2"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .filter(|p| {
                let n = p.file_stem().unwrap().to_string_lossy();
                filter.as_ref().is_none_or(|f| n.contains(f.as_str()))
            })
            .collect();
        paths.sort();
        let mut stats = [[0usize; 3]; 2];
        let mut diffs = [Vec::new(), Vec::new()];
        let mut refusals: std::collections::BTreeMap<String, usize> = Default::default();
        for p in paths {
            let b: Board = serde_json::from_str(&read(&p)).unwrap();
            let bd = &b.duel;
            let duel = Duel::from_observed(
                bd.terrain.clone(),
                bd.units.clone(),
                bd.sides.clone(),
                bd.round,
                bd.first,
                bd.active,
                &cards,
            )
            .unwrap();
            let w = World::new(&duel, &cards, &NoFusions, b.tier).unwrap();
            for c in &b.calls {
                let want_d = decision(&hex(&c.d));
                let want_rec = record(&c.after);
                for (m, replay) in [(0, true), (1, false)] {
                    let (res, rec, d) = run_call(&w, i32::from(b.side), c, replay);
                    let what = format!(
                        "{} {} rec {} which {} target {:#x} def {}",
                        b.name, c.ex, c.rec, c.which, c.target, c.def
                    );
                    match res {
                        Err(DuelError::Unsupported(e)) => {
                            stats[m][2] += 1;
                            *refusals.entry(format!("[{m}] {e}")).or_default() += 1;
                        }
                        Err(e) => {
                            stats[m][1] += 1;
                            diffs[m].push(format!("{what}: error {e}"));
                        }
                        Ok(v) => {
                            let same = v == c.ret
                                && rec == want_rec
                                && (d.key, d.kind, d.target, d.hand, d.reveal, d.position)
                                    == (
                                        want_d.key,
                                        want_d.kind,
                                        want_d.target,
                                        want_d.hand,
                                        want_d.reveal,
                                        want_d.position,
                                    );
                            if same {
                                stats[m][0] += 1;
                            } else {
                                stats[m][1] += 1;
                                diffs[m].push(format!(
                                    "{what}: ret {v} vs {}; {d:?} vs {want_d:?}; {rec:?} vs {want_rec:?}",
                                    c.ret
                                ));
                            }
                        }
                    }
                }
            }
        }
        for (m, name) in ["replayed searches", "real searches"].iter().enumerate() {
            let [ok, bad, refused] = stats[m];
            eprintln!("executor probes ({name}): {ok} match, {bad} differ, {refused} refused");
            for x in diffs[m].iter().take(40) {
                eprintln!("DIFF[{m}] {x}");
            }
        }
        for (r, n) in &refusals {
            eprintln!("REFUSED {r}: {n}");
        }
        assert!(
            diffs[0].is_empty(),
            "{} executor probes differ",
            diffs[0].len()
        );
    }
}
