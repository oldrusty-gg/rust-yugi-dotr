//! Keith's own plan executors: the lane plans 0xF / 0x10 (the first two
//! branches of 0x120FC0, which is otherwise the shared 0x110FC0 with
//! radius 4 and the 0x120350 see-all quirk) and the escort plan 0xB
//! (0x120A20). The other executors are the shared ones of
//! [`super::super::exec`] with the constants and hooks of [`super::Keith`].
//! Rex Raptor's module carries the same two executors (0x11BF50's lane
//! branches, 0x11B9B0) with its own constants, so both are generic over
//! the [`Opponent`] (`O::reveal`, `O::hidden_value`, `O::DANGER_RADIUS`).

use super::super::exec::{self as shared, stance};
use super::super::search::{self, Steps};
use super::super::world::{World, R};
use super::super::{Decision, FusionData, Opponent, Record};
use crate::duel::{CardData, DuelError};

type W<'a, C, F> = World<'a, C, F>;

/// (group, slot) of the record's unit.
fn unit_of(rec: &Record) -> (i32, i32) {
    (i32::from(rec.key >> 8), i32::from(rec.key & 0xFF))
}

/// (col, row) of a candidate byte (row << 4) | col.
fn nibbles(b: u8) -> (i32, i32) {
    (i32::from(b & 0xF), i32::from(b >> 4))
}

/// (row << 8) | col of a candidate byte, as the executors store it at +4.
fn cell_of(b: u8) -> u16 {
    u16::from(b >> 4) << 8 | u16::from(b & 0xF)
}

/// The common part of every exit: actor, type 6, destination.
fn act(d: &mut Decision, rec: &Record, dest: u16) {
    d.key = rec.key;
    d.kind = 6;
    d.target = dest;
}

/// 0x120FC0(rec, side, d, which, defflag) for a primary plan 0xF or 0x10
/// (0x1210D8 / 0x121244): the unit walks its lane. Destination = its own
/// column at row min(7, row + range) for 0xF, max(0, row − range) for
/// 0x10 (range = 0x247E10 & 0xF; 0x252180), taken as is when 0x27AB70
/// finds a route of at most 2 moves to it (the plan target's cell, 0xFFFF,
/// as the enterable occupant), else the own cell. +0x10 from 0x27FCA0
/// (see-all 0), +0xC from 0x121A80; the target is cleared. The
/// 0x121040 test of a free target at position 0xFFFF comes first, as in
/// 0x110FC0.
pub(in super::super) fn lane<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    defence: bool,
) -> R<()> {
    let (g, s) = unit_of(rec);
    let defflag = u8::from(defence);
    let pos = w.pos(g, s);
    let t = rec.target(which);
    let (c, r) = (pos & 0xFF, pos >> 8);
    if !(t == 0xFFFF && pos == 0xFFFF) {
        // 0x1210D8 / 0x12123C: the branch reads the PRIMARY plan code
        // whichever plan runs; a secondary 0xF / 0x10 under another
        // primary (which 0x11DCB0 never produces) takes the shared
        // plan-3/6 path of 0x121390.
        let down = match rec.plan_a {
            0xF => true,
            0x10 => false,
            _ => return shared::go_to_cell::<O, _, _>(w, rec, side, d, which, defence),
        };
        d.key = rec.key;
        d.kind = 6;
        let u = w.unit(g, s).ok_or(DuelError::InvalidCommand(
            "plan executor for a missing unit",
        ))?;
        let range = w.allowance(u, c, r)? & 0xF;
        let row = if down {
            (r + range).min(7)
        } else {
            (r - range).max(0)
        };
        d.target = ((row << 8) | c) as u16;
        let occ = match w.at_key(i32::from(t & 0xFF), i32::from(t >> 8)) {
            k if k < 0 => 0xFFFF,
            k => k,
        };
        let mut buf = Steps::new();
        search::path_k(
            w,
            g,
            s,
            &mut buf,
            c,
            r,
            i32::from(d.target & 0xFF),
            i32::from(d.target >> 8),
            occ & 0xFFFF,
            2,
        )?;
        // 0x1212A4 / 0x1211A8: `sh pos, 4(d)` when 0x27C900(buf) is 0. The
        // step buffer (sp+0xC0, the top 16 bytes of the frame) is never
        // initialised and a failed search leaves it alone, so it still
        // holds the $ra the dispatch's 0x1E3380 call saved there (frame
        // 0x20, `sq $ra, 16($sp)`: 4 address bytes + 12 zero bytes; the
        // same layout in Rex Raptor's 0x11BF50 at 0x11C2CC): at least nine
        // of the twelve bytes have no 0xF nibble, so the count is never 0
        // and the destination stands even without a route (traced in the
        // EE for Rex's cases-opp3 t00096 / t00165, where the dispatch is
        // the only caller). A fresh buffer would wrongly reset the target.
        let _ = search::step_count(&buf);
    } else {
        act(d, rec, pos as u16);
    }
    d.position = stance(w, g, s, d.target, defflag, false)?;
    d.reveal = O::reveal(w, g, s, rec)?;
    rec.consume(which);
    Ok(())
}

/// The byte 0x120A20 (and Rex Raptor's copy 0x11B9B0) reads as buf[0xFF]
/// while no candidate has won: sp+0x1CF of its 0xE0 frame, past the
/// 0x70 frame of the decide entry (0x11CED0 / 0x118730) and the 0x50
/// frame of the driver (0x121F00 / 0x11CD00), i.e. byte 0x2F of the 0x50
/// frame of 0x1CFF80, the top byte of the 128-bit $s2 it saves with
/// `sq $s2, 0x20(sp)`. 0x1CFF80 is only called by the duel step 0x1D00F0
/// (at 0x1D3570 / 0x1D3590 / 0x1D35A0), whose $s2 upper half is the one
/// it was entered with (its only 128-bit writes to $s2, 0x1D0604 and
/// 0x1D50C4..0x1D7E14, are not on a path to those calls in the same
/// invocation), and 0x1D00F0's only caller 0x1AABB0 clears $s2 with
/// `move $s2, $zero` (PADDUB, all 128 bits; 0x1AAD6C on every path to the
/// call at 0x1AAF44) and afterwards writes it only with 64-bit ADDIU.
/// Every function in the executable that writes $s2's upper half saves
/// and restores it with sq / lq, so the half stays 0 through all callees.
/// See the project notes. [F, static]
const CALLER_S2_TOP: u8 = 0;

/// 0x120A20(rec, side, d, which): plan 0xB (the target is a unit key);
/// −1 when the target is gone. The unit stays (defence requested through
/// 0x27FCA0 with defflag 1) unless it can move and is not threatened
/// (0x27D500, see-all 0); then, of the first moves of the shortest attack
/// routes to the target (0x27B9D0, at most 30 moves, hidden strength
/// 0x120350(side)), the empty one with the HIGHEST danger (0x278600 radius
/// 4, strength 1, see-all from 0x120350; strict > from 0), ties to the one
/// closer to the target (the first tie against the caller's byte 0x00,
/// see [`CALLER_S2_TOP`]); none → stays with defflag `none_defflag` (0 here,
/// 1 in Rex Raptor's 0x11B9B0). +0xC is not written and the target is
/// kept. The module's hidden-strength function and danger radius come
/// from `O` (0x120350 / 4 here, 0x11B2E0 = 0 / 3 in 0x11B9B0).
pub(in super::super) fn escort<O: Opponent, C: CardData + ?Sized, F: FusionData + ?Sized>(
    w: &W<C, F>,
    rec: &mut Record,
    side: i32,
    d: &mut Decision,
    which: u8,
    none_defflag: u8,
) -> R<i32> {
    let t = rec.target(which);
    let (tg, ts) = (i32::from(t >> 8), i32::from(t & 0xFF));
    if w.unit(tg, ts).is_none() {
        // 0x120AB4: always the primary target.
        rec.consume(1);
        return Ok(-1);
    }
    let (g, s) = unit_of(rec);
    let pos = w.pos(g, s);
    let tpos = w.pos(tg, ts);
    let (c, r) = (pos & 0xFF, pos >> 8);
    let (tc, tr) = (tpos & 0xFF, tpos >> 8);
    let hold = |d: &mut Decision, defflag: u8| -> R<i32> {
        act(d, rec, pos as u16);
        d.position = stance(w, g, s, d.target, defflag, false)?;
        Ok(0)
    };
    if t == 0xFFFF {
        return hold(d, 1);
    }
    let u = w.unit(g, s).ok_or(DuelError::InvalidCommand(
        "plan executor for a missing unit",
    ))?;
    if w.allowance(u, c, r)? == 0 || w.threatened(g, s, false)? {
        return hold(d, 1);
    }
    let mut buf = Steps::new();
    let hidden = O::hidden_value(w, side)? & 0xFFFF;
    search::attack_path_k(w, g, s, &mut buf, c, r, tc, tr, u32::from(t), 0, 30, hidden)?;
    let n = search::step_count(&buf);
    if n <= 0 {
        return hold(d, 1);
    }
    let from_target = |b: u8| {
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
        let all = O::hidden_value(w, side)? & 0xFF != 0;
        let score = w.danger(cc, cr, side, O::DANGER_RADIUS, 1, all)?;
        if best < score {
            best = score;
            bi = Some(i);
        } else if score == best {
            // 0x120DCC: while no candidate has won yet (best 0, index
            // 0xFF) the tie compares with the byte at buf[0xFF], 0x2F
            // into 0x1CFF80's frame: the top byte of the $s2 it saved,
            // which is always 0 (see CALLER_S2_TOP).
            let rival = bi.map_or(CALLER_S2_TOP, |j| buf.0[j]);
            if from_target(b) < from_target(rival) {
                bi = Some(i);
            }
        }
    }
    match bi {
        Some(i) => {
            act(d, rec, cell_of(buf.0[i]));
            d.position = stance(w, g, s, d.target, 0, false)?;
            Ok(0)
        }
        None => hold(d, none_defflag),
    }
}
