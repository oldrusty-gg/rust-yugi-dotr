//! BMODULE b155 (card 169), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06250, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the "a0 = D" family with s3 = D, s0 = the OT (rand's and the
//! library frames' saved s0), s1 the parameter. Node bursts (+0xA0, motes
//! +0xE4) every update; on update 210 an arc (+0xFB4, `arc::arcs`) between
//! the attacker's nodes 0x28 / 0x27; on 214 the bead-trail emitter
//! (+0x20F0, beads +0x16A0) at node 0x28; on 216 at the target a camera
//! shake, a flash (+0x13D4) and eight bars (+0x1420, `cross::bars`); on
//! 218 ray bursts along the arc (+0x2A8C, `ray::arc_rays`, rays +0x217C).
//! Returns 1 on update 208, 2 on 215, 8 on 300 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `flash` 0x1E015F0, `bars`
//! 0x1E021A0, `beads` 0x1E02BB0, `bead_trails` 0x1E03980, `arcs`
//! 0x1E03ED0, `rays` 0x1E05150, `arc_rays` 0x1E05CC0 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2AB8;

/// The entry 0x1E06250 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x60);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x60);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x2BC0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xD, 0), (0x14, 0x12, 0), (0x18, 0x28, 0), (0x1C, 0x27, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E06B60 / 0x1E06B80, motes +0xE4 /
    // +0x7E4.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_6B60);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6B80);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    fx.mem.write_u16(D + 0xD4, 0);
    fx.mem.write_u32(D + 0xD8, 0x430C_0000);
    fx.mem.write_u32(D + 0xDC, 0);
    fx.mem.write_u32(D + 0xE0, 0x40C0_0000);
    // The entry's s0 = the OT, s1 = 1 (the texture loop's), s2 the
    // runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6404, [Some(OT), Some(1), None]);
    0
}

/// Update 210 [F 0x1E06474]: the arc (+0xFB4) from node +0x18 to node
/// +0x1C, points at +0x1014.
fn arc_start(fx: &mut Fx) {
    let g = D + 0xFB4;
    set_counter4(fx, g, 0x1A, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xB);
    fx.mem.write_u32(g + 8, D + 0x1014);
    fx.mem.write_u8(g + 0x30, 0x3C);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u32(g + 0x3C, 0x4040_0000);
    fx.mem.write_u32(g + 0x40, 0x41A0_0000);
    fx.mem.write_u8(g + 0x34, 0);
    for (k, v) in [(0x44u32, 0xC0u8), (0x45, 0xC0), (0x46, 0xC0), (0x47, 0x80), (0x4A, 0x10), (0x49, 0x10), (0x48, 0x80), (0x4E, 0x10), (0x4D, 0x10), (0x4C, 0x80), (0x4F, 0xC0)] {
        fx.mem.write_u8(g + k, v);
    }
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        fx.mem.set16(g + 0xC + 2 * k, -1);
        k += 1;
    }
}

/// Update 214 [F 0x1E06554]: the bead-trail emitter (+0x20F0) at node
/// +0x18, beads into +0x16A0.
fn emitter(fx: &mut Fx, s1: u64) {
    let e = D + 0x20F0;
    set_counter4(fx, e, 6, 0, 0xFF);
    fx.mem.write_u8(e + 4, 0);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0x16A0);
    for (o, v) in [(0x10u32, 0x1Eu8), (0x12, 0x14), (0x11, 0x20), (0x13, 0)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3F99_999A);
    fx.mem.write_u32(e + 0x1C, 0x3E4C_CCCD);
    fx.mem.write_u32(e + 0x20, 0x41C8_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for (o, v) in [(0x2Au32, 0x80u8), (0x29, 0x40), (0x28, 0x20), (0x2B, 0xF0)] {
        fx.mem.write_u8(e + o, v);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(s1), 0x01E0_65FC);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Update 216 [F 0x1E06624]: at the target (sp + 0x50) the shake, a
/// flash and eight bars (22.5 degrees apart, the first the largest).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x4000_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
    let r = D + 0x13D4;
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x442F_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x20), (0xF, 0xE0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    for k in 0..8i32 {
        let r = D + 0x1420 + 0x50 * k as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        let (size, w) = if k != 0 { (0x4448_0000u32, 0x41A0_0000u32) } else { (0x44BB_8000, 0x41F0_0000) };
        fx.mem.write_u32(r + 0x10, size);
        fx.mem.write_u32(r + 0x14, w);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x40), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::mul(22.5, ee::from_i32(k)));
        fx.mem.write_u32(r + 0x1C, 0);
        copy_v4(fx, at, r + 0x20);
    }
}

/// Update 218 [F 0x1E06834]: the arc-ray spawner (+0x2A8C) on the arc
/// +0xFB4, rays into +0x217C.
fn arc_rays_start(fx: &mut Fx) {
    let g = D + 0x2A8C;
    set_counter4(fx, g, 1, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0xFB4);
    fx.mem.write_u32(g + 0xC, D + 0x217C);
    for (o, v) in [(0x14u32, 0x14u8), (0x15, 0x14), (0x16, 0xC), (0x17, 0xE)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x18, 0x4080_0000);
    fx.mem.write_u32(g + 0x1C, 0x3F00_0000);
    fx.mem.write_u32(g + 0x20, 0x4100_0000);
    fx.mem.write_u32(g + 0x10, BYTES);
    for (o, v) in [(0x26u32, 0x10u8), (0x25, 0x10), (0x24, 0x80), (0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let ot = u64::from(OT);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, ot, 0x01E0_642C);
    match fx.mem.u16(D) {
        0x12C => return 8,
        0xDA => arc_rays_start(fx),
        0xD8 => strike(fx, sp),
        0xD6 => emitter(fx, i64::from(param) as u64),
        0xD2 => arc_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, ot, 0x01E0_68E8, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    blib::mote::motes(fx, D + 0xE4, OT, 0x20);
    blib::star::flash(fx, 0x01E0_15F0, D + 0x13D4, OT, 1);
    blib::cross::bars(fx, 0x01E0_21A0, D + 0x1420, OT, 8);
    // bars' saved s4..s7 / fp (the runner's 0) and ra below the entry's
    // frame: the bead trail's curve samples keep them as their w [run].
    let f = sp - 0x230;
    fx.saved(f + 0x60, &[(0, 0), (0x10, 0), (0x20, 0), (0x30, 0), (0x40, 0), (0x50, 0x01E0_6934)]);
    blib::emit::bead_trails(fx, 0x01E0_3980, D + 0x20F0, 1);
    blib::arc::arcs(fx, 0x01E0_3ED0, D + 0xFB4, OT, 1);
    blib::ray::arc_rays(fx, 0x01E0_5CC0, D + 0x2A8C, 1);
    blib::beam::beads(fx, 0x01E0_2BB0, D + 0x16A0, OT, 0x1E);
    blib::ray::rays(fx, 0x01E0_5150, D + 0x217C, OT, 0x14);
    match fx.mem.u16(D) {
        0xD7 => 2,
        0xD0 => 1,
        _ => 0,
    }
}
