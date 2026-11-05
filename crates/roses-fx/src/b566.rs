//! BMODULE b566 (card 662), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02750, data area 0x1ED0000) [F]: b284's layout (200 tasks of 0x40 bytes at D+0x2F0, the same
//! spawn 0x1E02E70 and node history a72bfb6bb70a at 0x1E00080 for node
//! 0x55) with other steps: on update 41 a grey full-window frame tint
//! (0x207EF0) that holds 0x68 until update 279, then fades by 12; from 71
//! to 241 grey smoke and a green mote leave the attacker's node 0x55,
//! drifting towards the target. Rings by `quads4::ring_facing_i`
//! (0x1E009F0) and `quads4::ring_at_m` (0x1E00F70), UVs by uv_rect
//! 0x1E00150. Returns 1 on update 60, 4 on 285, 2 on 280, 6 on 71..274,
//! 8 once the 290-update timer has run out.
//!
//! The steps 0x1E018E0, 0x1E01BD0, 0x1E02340, 0x1E024B0 and the entry's
//! case 0x1E02570 (an object op through 0x1F8380) are never spawned [F:
//! the emitter's `jal`s name 0x1E025C0 / 0x1E01F90 / 0x1E015D0 only]; not
//! ported.
use crate::b284::{angles, cell, centre, fix, rmod, spawn};
use crate::blib::{quads4, spawn as lib, uv};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::fxlib2 as l2;

const D: u32 = crate::bmodule::DATA_AREA;
const TASKS: u32 = 200;
/// The step functions a task holds (+0x24), as the record's addresses.
const MOTE: u32 = 0x01E0_15D0;
const SMOKE: u32 = 0x01E0_1F90;
const TINT: u32 = 0x01E0_25C0;
const TIMER: u32 = D + 0x3750;
const COLOUR: u32 = D + 0x3754;
const NODE: u32 = D + 0x377C;
const COUNTER: u32 = D + 0x3798;
const BYTES: u32 = D + 0x379A;

/// The entry 0x1E02750.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xC0);
    fx.mem.write_u32(D + 0x80, OT);
    fx.mem.write_u32(D + 0x375C, OT);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(0xC0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x3B10);
    l2::sine_table(fx, D + 0x399C);
    l2::sine_table(fx, D + 0xD8);
    fx.mem.set16(D + 0x3796, param);
    fx.mem.set32(TIMER, 0x122);
    for (k, node) in [0x55, 0x3A, 0x40, 0x47, 0x4E].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set16(NODE + 2 * k as u32, r);
    }
    for k in 0xA..0x10 {
        let t = fx.texture(4, k);
        fx.mem.write_u32(D + 0x2D8 + 4 * (k as u32 - 0xA), t);
    }
    let t = fx.texture(4, 2);
    fx.mem.write_u32(D + 0x2B8, t);
    for k in 0..3 {
        let t = fx.texture(5, k);
        fx.mem.write_u32(D + 0x34F0 + 4 * k as u32, t);
    }
    for k in 0..0x100u32 {
        let v = fx.rand_from(k, 0x01E0_2894);
        let (_, h) = fx.div(v, 0x1E);
        fx.mem.set8(BYTES + k, h - 0xF);
    }
    for k in 0..8u32 {
        let v = fx.rand_from(k, 0x01E0_28C8);
        fx.mem.set16(D + 0xC0 + 2 * k, v % 4);
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    emit(fx);
    if fx.mem.i16(COUNTER) == 1 {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, sp + 0xA0);
        for k in 0..9u32 {
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0xA0 + 4 * c);
                fx.mem.write_u32(D + 0x3560 + 0x10 * k + 4 * c, w);
            }
        }
    }
    let slot = fx.mem.i16(NODE);
    lib::node_history(fx, D + 0x35F0, 0xA, slot);
    for k in 0..TASKS {
        let s = D + 0x40 * k;
        let t = s + 0x2F0;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        if fx.mem.read_u32(t + 0x24) == MOTE {
            mote(fx, t);
        }
        if fx.mem.read_u32(t + 0x24) == SMOKE {
            smoke(fx, t, s);
        }
        if fx.mem.read_u32(t + 0x24) == TINT {
            tint(fx, t);
        }
        debug_assert!(!matches!(fx.mem.read_u32(t + 0x24), 0x01E0_18E0 | 0x01E0_1BD0 | 0x01E0_2340 | 0x01E0_24B0 | 0x01E0_2570));
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x50);
    // 0x1F71A0's frame below the entry's (its saved ra, the entry's s0).
    fx.saved(sp - 0x20, &[(0x10, 0x01E0_2AD4), (0, 0)]);
    // The entry's s0: the live tasks counted in the draw loop.
    let mut s0 = 0u32;
    for k in 0..TASKS {
        let t = D + 0x2F0 + 0x40 * k;
        if fx.mem.i16(t + 0x36) == 0 {
            continue;
        }
        s0 += 1;
        if fx.mem.read_u32(t + 0x24) == MOTE {
            cell(fx, t, sp + 0x60);
            fx.mem.set16(D + 0xB8, 3);
            let v = fx.rand_from(s0, 0x01E0_2B48);
            fx.mem.set16(D + 0xBE, v % 4);
            quads4::ring_facing_i(fx, uv::uv_rect, D, t, 0, BYTES);
            fx.mem.set16(D + 0xB8, 0);
        }
        if fx.mem.read_u32(t + 0x24) == SMOKE {
            centre(fx, t);
            cell(fx, t, sp + 0x90);
            if fx.mem.i16(t + 0x32) >= 4 {
                fx.mem.set16(D + 0xB8, 3);
                let v = fx.rand_from(s0, 0x01E0_2D44);
                fx.mem.set16(D + 0xBE, v % 4);
                centre(fx, t);
                quads4::ring_at_m(fx, uv::uv_rect, D, t, 0x20, BYTES);
                fx.mem.set16(D + 0xB8, 0);
            }
        }
    }
    let left = fx.mem.read_u32(TIMER);
    if left != 0 {
        fx.mem.write_u32(TIMER, left - 1);
    }
    match fx.mem.i16(COUNTER) {
        0x3C => 1,
        0x11D => 4,
        0x118 => 2,
        0x47..=0x112 => 6,
        _ if fx.mem.i32(TIMER) > 0 => 0,
        _ => 8,
    }
}

/// 0x1E014D0(D, 0): the spawn colour (0x80, 0, 0, 0), the update counter
/// and the emitters (frame 0x40: ra, the entry's s1 = D / s0 = 0).
fn emit(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0x20, 0x01E0_2904), (0x10, u64::from(D)), (0, 0)]);
    fx.mem.write_u8(COLOUR, 0x80);
    for k in 1..4 {
        fx.mem.write_u8(COLOUR + k, 0);
    }
    let n = fx.mem.i16(COUNTER) + 1;
    fx.mem.set16(COUNTER, n);
    if fx.mem.i16(COUNTER) == 0x29 {
        spawn(fx, TINT);
    }
    let c = fx.mem.i16(COUNTER);
    if (0x47..0xF2).contains(&c) {
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, sp + 0x30);
        for k in 0..3 {
            let w = fx.mem.read_u32(sp + 0x30 + 4 * k);
            fx.mem.write_u32(D + 0x250 + 4 * k, w);
        }
        spawn(fx, SMOKE);
        spawn(fx, MOTE);
    }
    fx.leave(0x40);
}

/// 0x1E015D0 (D, task): a green mote ring (frame 0x50: ra, the entry's s0
/// = 0): size 70 growing by 1 (and 2 from age 24), radius growing from 0
/// by 5..9 (twice that more from age 24), row 0..24, life 26..35; fades in
/// by 0x30 under 0x61, out by 9; the cell frame counting to 5 on odd
/// ages; its z drifts towards the target by the distance / 40 (/ 100 from
/// age 24); random angles +0x10 / +0x14 / +0x18.
fn mote(fx: &mut Fx, t: u32) {
    let s = MOTE;
    let sp = fx.enter(0x50);
    fx.saved(sp, &[(0x10, 0x01E0_299C), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.set16(t + 0x38, 0x46);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x20, 0x70, 0x20);
        fx.mem.set16(t + 0x30, 0);
        let v = rmod(fx, t, s + 0x3C, 0x168);
        fx.mem.set16(t + 0x3A, v);
        let v = rmod(fx, t, s + 0x58, 5);
        fx.mem.set16(t + 0xE, v + 5);
        let v = rmod(fx, t, s + 0x78, 0x19);
        fx.mem.set16(t + 8, v);
        let v = rmod(fx, t, s + 0x94, 0xA);
        fx.mem.set16(t + 6, v + 0x1A);
        fx.mem.set16(t + 0xC, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        if a < 0x61 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x30));
        }
    } else if a >= 0xB {
        fx.mem.write_u8(t + 0x2B, a - 9);
    } else {
        fx.mem.set16(t + 0x36, 0);
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x20);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x30);
    let dz = ee::sub(fx.mem.f(sp + 0x28), fx.mem.f(sp + 0x38));
    fx.mem.setf(sp + 0x28, dz);
    let young = fx.mem.i16(t + 0x32) < 0x18;
    let dz = ee::div(dz, if young { 40.0 } else { 100.0 });
    fx.mem.setf(sp + 0x28, dz);
    let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
    let z = fix(fx, z, if young { s + 0x18C } else { s + 0x1C8 });
    fx.mem.set16(t + 4, z);
    if !young {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + 2);
        let r = fx.mem.i16(t + 0x30) + 2 * fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let x = fx.mem.i16(t) + fx.mem.i16(t + 0xC);
        fx.mem.set16(t, x);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
        fx.mem.set16(t + 0x30, r);
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + 1);
    }
    angles(fx, t, s + 0x298);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x50);
}

/// 0x1E01F90 (D, task): grey smoke (frame 0x70: the entry's s2 = the slot
/// `s`, s1 = D, s0 = 0): size 12..23 growing by 2 (under 60) and 2 on odd
/// ages, the radius +0x30 kept in +0xA and growing from 0 by 3..7, row
/// 0..34, life 32..56, +0xC the x distance of nodes 0x3A - 0x55; fades in
/// by 0x40 under 0x81, out by 7 before update 260, by 9 after it; the cell
/// frame counting to 5 on odd ages; its z drifts towards the target by the
/// distance / 75; random angles, +0x3A turning by 2.
fn smoke(fx: &mut Fx, t: u32, s: u32) {
    let f = SMOKE;
    let sp = fx.enter(0x70);
    fx.saved(sp, &[(0x30, 0x01E0_2A00), (0x20, u64::from(s)), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(t + 0x36) == 1 {
        let r = rmod(fx, t, f + 0x28, 0xC);
        fx.mem.set16(t + 0x38, r + 0xC);
        fx.mem.set16(t + 0x34, 0);
        l2::rgb(fx, t + 0x28, 0x50, 0x50, 0x50);
        let r = fx.mem.i16(t + 0x30);
        fx.mem.set16(t + 0xA, r);
        fx.mem.set16(t + 0x30, 0);
        let r = rmod(fx, t, f + 0x64, 0x168);
        fx.mem.set16(t + 0x3A, r);
        let r = rmod(fx, t, f + 0x80, 5);
        fx.mem.set16(t + 0xE, r + 3);
        let r = rmod(fx, t, f + 0xA0, 0x23);
        fx.mem.set16(t + 8, r);
        let r = rmod(fx, t, f + 0xBC, 0x19);
        fx.mem.set16(t + 6, r + 0x20);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE);
        fx.node_position(h, slot, sp + 0x40);
        let h = fx.self_handle();
        let slot = fx.mem.i16(NODE + 2);
        fx.node_position(h, slot, sp + 0x50);
        let dx = ee::sub(fx.mem.f(sp + 0x50), fx.mem.f(sp + 0x40));
        let dx = fix(fx, dx, f + 0x118);
        fx.mem.set16(t + 0xC, dx);
        fx.mem.set16(t + 0x36, 2);
    }
    if fx.mem.i16(t + 0x32) < fx.mem.i16(t + 6) {
        let a = fx.mem.read_u8(t + 0x2B);
        if fx.mem.i16(COUNTER) < 0x104 && a < 0x81 {
            fx.mem.write_u8(t + 0x2B, a.wrapping_add(0x40));
        }
    } else if fx.mem.i16(COUNTER) < 0x104 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 9 {
            fx.mem.write_u8(t + 0x2B, a - 7);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(COUNTER) >= 0x105 {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xB {
            fx.mem.write_u8(t + 0x2B, a - 9);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    if fx.mem.i16(t + 0x32) != 0 {
        let g = fx.mem.i16(t + 0x38);
        if g < 0x3C {
            fx.mem.set16(t + 0x38, g + 2);
        }
    }
    let h = fx.target_handle();
    fx.unit_position(h, sp + 0x40);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x50);
    if fx.mem.i16(t + 0x32) != 0 {
        let dz = ee::sub(fx.mem.f(sp + 0x48), fx.mem.f(sp + 0x58));
        fx.mem.setf(sp + 0x48, dz);
        let dz = ee::div(dz, 75.0);
        fx.mem.setf(sp + 0x48, dz);
        let z = ee::add(ee::from_i32(fx.mem.i16(t + 4)), dz);
        let z = fix(fx, z, f + 0x264);
        fx.mem.set16(t + 4, z);
    }
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let c = fx.mem.i16(t + 0x34);
        if c < 5 {
            fx.mem.set16(t + 0x34, c + 1);
        }
    }
    let r = fx.mem.i16(t + 0x30) + fx.mem.i16(t + 0xE);
    fx.mem.set16(t + 0x30, r);
    if fx.mem.i16(t + 0x32) % 2 == 1 {
        let g = fx.mem.i16(t + 0x38);
        fx.mem.set16(t + 0x38, g + 2);
    }
    for (o, site) in [(0x10u32, f + 0x2EC), (0x14, f + 0x308)] {
        let v = rmod(fx, t, site, 0x168);
        fx.mem.set32(t + o, v);
    }
    let a = fx.mem.i16(t + 0x3A);
    let turn = if fx.mem.i16(t + 0xE) % 2 == 1 { 2 } else { -2 };
    fx.mem.set16(t + 0x3A, a + turn);
    let v = rmod(fx, t, f + 0x368, 0x168);
    fx.mem.set32(t + 0x18, v);
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x70);
}

/// 0x1E025C0 (D, task): the frame tint (frame 0x90; its descriptor at
/// sp + 0x38 = the library frame quad's, [`Fx::frame_quad_rgba`] from
/// this caller): the frame drawn over the window in (0x80, 0x80, 0x80,
/// alpha +0x2B), ALPHA 0x44; the alpha 0x68 until update 279, then down
/// by 12 (the task ends below 14).
fn tint(fx: &mut Fx, t: u32) {
    if fx.mem.i16(t + 0x36) == 1 {
        fx.mem.write_u8(t + 0x2B, 0);
        fx.mem.set16(t + 0x36, 2);
    }
    let a = fx.mem.read_u8(t + 0x2B);
    fx.frame_quad_rgba([0x80, 0x80, 0x80, a]);
    let sp = fx.enter(0x90);
    fx.saved(sp, &[(0x20, 0x01E0_2AB0), (0x10, u64::from(D)), (0, 0)]);
    if fx.mem.i16(COUNTER) < 0x118 {
        fx.mem.write_u8(t + 0x2B, 0x68);
    } else {
        let a = fx.mem.read_u8(t + 0x2B);
        if a >= 0xE {
            fx.mem.write_u8(t + 0x2B, a - 0xC);
        } else {
            fx.mem.set16(t + 0x36, 0);
        }
    }
    let n = fx.mem.i16(t + 0x32) + 1;
    fx.mem.set16(t + 0x32, n);
    fx.leave(0x90);
}
