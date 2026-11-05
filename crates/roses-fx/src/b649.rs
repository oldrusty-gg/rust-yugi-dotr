//! BMODULE b649 (card 358), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E043F0, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the "a0 = D" family with s0 = D (rand's and the library
//! frames' saved s0), s1 the parameter, then loop offsets, s2 = the OT.
//! On update 2 six arcs (+0xEC, `arc::arcs`) along the attacker's node
//! pairs 0x46-0x45, 0x45-0x44, 0x44-0x43 and 0x59..0x56; on 50 two more
//! over them; on 68 at the target a ray burst (+0x38C, `ray::rays`), three
//! cones (+0x400, `cone::cones`) aimed from node 0x56, a flash (+0xA0),
//! eight bead beams (+0x4D8) and a camera shake; on 67 the nodes are
//! requested again (the last as node 1). Returns 1 on update 60, 2 on 67,
//! 8 on 110 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads`
//! 0x1E00410, `flash` 0x1E011E0, `rays` 0x1E01D90, `arcs` 0x1E02900,
//! `cones` 0x1E03B80 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x1098;

/// The entry 0x1E043F0 (a0 = D, a1 = param; s0 = D, s1 = param, s2 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x90);
    ret
}

fn nodes(fx: &mut Fx, last: i32) {
    for (at, node, clear) in [(0xCu32, 0x46, 1), (0x10, 0x45, 0), (0x14, 0x44, 0), (0x18, 0x43, 0), (0x1C, 0x59, 0), (0x20, 0x58, 0), (0x24, 0x57, 0), (0x28, last, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x11A0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    // The record's texture-5 loop runs while its counter is negative:
    // never [F 0x1E044B8].
    fx.texture(4, 0xF);
    nodes(fx, 0x56);
    // The entry's s0 = D, s1 = the parameter, s2 = the OT.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4590, [Some(D), Some(param as u32), Some(OT)]);
    0
}

/// An arc record (+0xEC + 0x60 k) between nodes `a` / `b` (slots).
fn arc(fx: &mut Fx, g: u32, count: i32, flags: u8, a: u32, b: u32) {
    set_counter4(fx, g, count, 0, 0xFF);
    fx.mem.write_u8(g + 4, flags);
    fx.mem.write_u32(g + 8, D + 0xA58);
    fx.mem.write_u8(g + 0x30, 0x64);
    fx.mem.write_u8(g + 0x31, 8);
    let v = fx.mem.read_u8(D + a);
    fx.mem.write_u8(g + 0x32, v);
    let v = fx.mem.read_u8(D + b);
    fx.mem.write_u8(g + 0x33, v);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
}

fn arc_heads(fx: &mut Fx, g: u32) {
    let mut j = 0u32;
    while j < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * j, -1);
        fx.mem.set16(g + 0xC + 2 * j, -1);
        j += 1;
    }
}

/// Update 2 [F 0x1E045F4]: six arcs along the node pairs (k, k + 1) of
/// the two chains, alternating direction.
fn chains(fx: &mut Fx) {
    for i in 0..2u32 {
        for j in 0..3u32 {
            let k = j + 4 * i;
            let g = D + 0xEC + 0x60 * k;
            let (a, b) = if j & 1 != 0 { (0x10 + 4 * k, 0xC + 4 * k) } else { (0xC + 4 * k, 0x10 + 4 * k) };
            arc(fx, g, 0x26, 1, a, b);
            fx.mem.write_u8(g + 0x34, 1);
            fx.mem.write_u16(g + 0x36, 0x40);
            for (o, v) in [(0x46u32, 0x80u8), (0x45, 0x40), (0x44, 0x10), (0x47, 0xC0), (0x4A, 0x80), (0x49, 0x40), (0x48, 0x10)] {
                fx.mem.write_u8(g + o, v);
            }
            arc_heads(fx, g);
        }
    }
}

/// Update 50 [F 0x1E04754]: arcs 0 / 4 again, straight (+0x34 = 0).
fn chains2(fx: &mut Fx) {
    for i in 0..2u32 {
        let k = 4 * i;
        let g = D + 0xEC + 0x60 * k;
        arc(fx, g, 0x22, 3, 0xC + 4 * k, 0x10 + 4 * k);
        fx.mem.write_u32(g + 0x40, 0x4220_0000);
        fx.mem.write_u8(g + 0x34, 0);
        for (o, v) in [(0x46u32, 0x80u8), (0x45, 0x40), (0x44, 0x10), (0x47, 0x60), (0x4A, 0x80), (0x49, 0x40), (0x48, 0x10), (0x4E, 0x80), (0x4D, 0x40), (0x4C, 0), (0x4F, 0x80)] {
            fx.mem.write_u8(g + o, v);
        }
        arc_heads(fx, g);
    }
}

/// Update 68 [F 0x1E04874]: at the target (sp + 0x70) the ray burst,
/// three cones aimed from node +0x28, the flash, eight bead beams and the
/// shake.
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x70;
    let r = D + 0x38C;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 4, 1);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 8, 0x3C);
    let x = fx.rand_from(D, 0x01E0_48A8);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x18, 0x42A0_0000);
    fx.mem.write_u32(r + 0xC, BYTES);
    fx.mem.write_u32(r + 0x10, 0x4040_0000);
    for (o, v) in [(0x1Eu32, 0x80u8), (0x1D, 0x80), (0x1C, 0x10), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, at, r + 0x24);
    for k in 0..3u32 {
        let g = D + 0x400 + 0x48 * k;
        set_counter4(fx, g, ((0x13 - 4 * k) & 0xFF) as i32, ((4 * k) & 0xFF) as i32, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 1), (0xA, 0x20), (9, 7)] {
            fx.mem.write_u8(g + o, v);
        }
        fx.mem.write_u32(g + 0xC, 0x40A0_0000);
        fx.mem.write_u32(g + 0x10, 0x4000_0000);
        fx.mem.write_u32(g + 0x14, 0x4060_0000);
        fx.mem.write_u32(g + 0x18, 0);
        fx.mem.write_u32(g + 0x1C, 0x3F19_999A);
        for (o, v) in [(0x21u32, 0x10u8), (0x22, 0x10), (0x20, 0x80), (0x23, 0), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0xC0)] {
            fx.mem.write_u8(g + o, v);
        }
        copy_v4(fx, at, g + 0x28);
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x28);
        node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(0x48 * k)), 0x01E0_4A0C);
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x70 + 4 * c), fx.mem.f(sp + 0x80 + 4 * c));
            fx.mem.setf(sp + 0x80 + 4 * c, v);
        }
        aim_angles(fx, sp + 0x80, g + 0x38);
    }
    let r = D + 0xA0;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x442F_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xAu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (8, 0x20), (9, 0x20), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    for j in 0..8i32 {
        let i = first_free(fx, D + 0x4D8, 0x58, 0x10);
        if i < 0 {
            continue;
        }
        let count = (((fx.rand_from(D, 0x01E0_4B24) >> 4) & 3) + 0x18) & 0xFF;
        let delay = (fx.rand_from(D, 0x01E0_4B3C) >> 4) & 7;
        let b = D + 0x4D8 + 0x58 * i as u32;
        set_counter4(fx, b, count, delay, 0xFF);
        fx.mem.write_u8(b + 4, 1);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x1E);
        let x = fx.rand_from(D, 0x01E0_4B90);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.setf(b + 0x1C, ee::add(10.0, ee::mul(2.0, ee::from_i32(j))));
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        let x = fx.rand_from(D, 0x01E0_4BE0) >> 4;
        let h = fx.div(x, 0x64).1 - 0x32;
        fx.mem.setf(b + 0x14, ee::add(f32::from_bits(0x3F99_999A), ee::div(ee::from_i32(h), 100.0)));
        for (o, v) in [(0x26u32, 0x80u8), (0x25, 0x30), (0x24, 0x10), (0x27, 0xF0)] {
            fx.mem.write_u8(b + o, v);
        }
        copy_v4(fx, at, b + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_45B8);
    match fx.mem.u16(D) {
        0x6E => return 8,
        0x44 => strike(fx, sp),
        0x32 => chains2(fx),
        2 => chains(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_4D24, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::star::flash(fx, 0x01E0_11E0, D + 0xA0, OT, 1);
    blib::arc::arcs(fx, 0x01E0_2900, D + 0xEC, OT, 7);
    blib::ray::rays(fx, 0x01E0_1D90, D + 0x38C, OT, 1);
    blib::beam::beads(fx, 0x01E0_0410, D + 0x4D8, OT, 8);
    blib::cone::cones(fx, 0x01E0_3B80, D + 0x400, OT, 3);
    if fx.mem.u16(D) == 0x43 {
        nodes(fx, 1);
    }
    match fx.mem.u16(D) {
        0x43 => 2,
        0x3C => 1,
        _ => 0,
    }
}
