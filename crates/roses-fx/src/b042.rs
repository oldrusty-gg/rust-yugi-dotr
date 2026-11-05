//! BMODULE b042 (card 157), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06250, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): b155's program shape (s3 = D, s0 = the OT, s1 the parameter
//! then loop counters / offsets, f20 kept). Node bursts (+0xA0, motes
//! +0xE4) every update; on updates 80..110 a spinning cross of four bars
//! (+0x1C14, `cross::bars`, drawn as 12), a flash (+0xAD4) and a soft
//! shake sliding from the attacker's node 0x3A towards 0x3B; on 195 an arc
//! (+0xA74, `arc::arcs`) between them; two strikes (204 / 230) at the
//! target: a camera shake, a flash (+0xAD4 / +0xB20) and the bead-trail
//! emitter (+0x1B5C, beads +0x147C) at node 0x3B, then ray bursts along
//! the arc (+0x1BE8, `ray::arc_rays`, rays +0xB6C) on 206 / 232. Returns 1
//! on updates 195 / 221, 6 on 202, 7 on 220, 2 on 228, 8 on 320 (and at
//! once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `flash` 0x1E015F0, `bars`
//! 0x1E021A0, `beads` 0x1E02BB0 (drawn twice), `bead_trails` 0x1E03980,
//! `arcs` 0x1E03ED0, `rays` 0x1E05150, `arc_rays` 0x1E05CC0
//! [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x22F4;

/// The entry 0x1E06250 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x2400);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0x14, 0), (0x14, 0xC, 0), (0x18, 0x3B, 0), (0x1C, 0x3A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E06EE0 / 0x1E06EF0, motes +0xE4 /
    // +0x2A4.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_6EE0);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6EF0);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 8);
    fx.mem.write_u32(D + 0xA4, D + 0x2A4);
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

/// Update 195 [F 0x1E06480]: the arc (+0xA74) from node +0x18 to +0x1C,
/// points at +0x1FD4.
fn arc_start(fx: &mut Fx) {
    let g = D + 0xA74;
    set_counter4(fx, g, 0x30, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xB);
    fx.mem.write_u32(g + 8, D + 0x1FD4);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u32(g + 0x3C, 0x4040_0000);
    fx.mem.write_u32(g + 0x40, 0x41A0_0000);
    for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0x80), (0x4A, 0x20), (0x49, 0x20), (0x48, 0x80), (0x4E, 0x20), (0x4D, 0x20), (0x4C, 0x80), (0x4F, 0xF0)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        fx.mem.set16(g + 0xC + 2 * k, -1);
        k += 1;
    }
}

/// Updates 204 / 230 [F 0x1E0655C]: at the target (sp + 0x60) the shake,
/// flash `k` (+0xAD4 + 0x4C k) and the bead-trail emitter (+0x1B5C) at
/// node +0x18. Returns the entry's s1 after (the flash offset).
fn strike(fx: &mut Fx, sp: u32, k: u32) -> u64 {
    let at = sp + 0x60;
    let s = D + 0x40;
    set_counter4(fx, s, 0x10, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
    let r = D + 0xAD4 + 0x4C * k;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4416_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x40), (8, 0x20), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    let e = D + 0x1B5C;
    set_counter4(fx, e, 4, 0, 0xFF);
    fx.mem.write_u8(e + 4, 0);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0x147C);
    for (o, v) in [(0x10u32, 0x14u8), (0x12, 0x1E), (0x11, 0x24), (0x13, 0)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3F19_999A);
    fx.mem.write_u32(e + 0x1C, 0x3DCC_CCCD);
    fx.mem.write_u32(e + 0x20, 0x41A0_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for (o, v) in [(0x2Au32, 0x80u8), (0x29, 0x40), (0x28, 0x20), (0x2B, 0xF0)] {
        fx.mem.write_u8(e + o, v);
    }
    let s1 = u64::from(0x4C * k);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(s1), 0x01E0_6748);
    copy_v4(fx, e + 0x3C, e + 0x4C);
    s1
}

/// Updates 206 / 232 [F 0x1E06770]: the arc-ray spawner (+0x1BE8) on the
/// arc +0xA74, rays into +0xB6C.
fn arc_rays_start(fx: &mut Fx) {
    let g = D + 0x1BE8;
    set_counter4(fx, g, 1, 0, 0xFF);
    fx.mem.write_u8(g + 4, 2);
    fx.mem.write_u32(g + 8, D + 0xA74);
    fx.mem.write_u32(g + 0xC, D + 0xB6C);
    for (o, v) in [(0x14u32, 0x14u8), (0x16, 0xA), (0x15, 0x12), (0x17, 0xC)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x18, 0x4040_0000);
    fx.mem.write_u32(g + 0x1C, 0x3F00_0000);
    fx.mem.write_u32(g + 0x20, 0x4100_0000);
    fx.mem.write_u32(g + 0x10, BYTES);
    for (o, v) in [(0x25u32, 0u8), (0x26, 0), (0x24, 0x80), (0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0x80)] {
        fx.mem.write_u8(g + o, v);
    }
}

/// Update 80 [F 0x1E0683C]: the cross of four bars (+0x1C14) at a random
/// angle, the flash +0xAD4 and the soft shake.
fn cross_start(fx: &mut Fx) {
    let x = fx.rand_from(OT, 0x01E0_6844) >> 4;
    let v = fx.div(x, 0x9C40).1 - 0x4E20;
    let a = ee::div(ee::from_i32(v), 100.0);
    for k in 0..4i32 {
        let r = D + 0x1C14 + 0x50 * k as u32;
        set_counter4(fx, r, 0x1E, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        let len = match k {
            0 => 0x4348_0000u32,
            2 => 0x42B4_0000,
            _ => 0x4270_0000,
        };
        fx.mem.write_u32(r + 0x10, len);
        fx.mem.write_u32(r + 0x14, 0x4000_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x40), (0xA, 0x20), (9, 0x20), (8, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.setf(r + 0x18, ee::add(a, ee::mul(45.0, ee::from_i32(k))));
        fx.mem.write_u32(r + 0x1C, 0x4140_0000);
    }
    let r = D + 0xAD4;
    set_counter4(fx, r, 0x1E, 0, 0xFF);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u32(r + 0x10, 0x42C8_0000);
    fx.mem.write_u32(r + 0x14, 0);
    fx.mem.write_u8(r + 0xF, 0x70);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 7, 9);
    for (o, v) in [(0xCu32, 0xF0u8), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x20), (9, 0x20), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x1E, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 2);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4220_0000);
    fx.mem.write_u32(s + 0x50, 0x3FE6_6666);
}

/// Updates 80..110 [F 0x1E06A48]: the point sliding from node +0x1C to
/// node +0x18 (t = 0.015 (n - 79)) into the twelve bars' +0x20, the
/// flash's +0x1C and the shake's +0x20.
fn slide(fx: &mut Fx, sp: u32, s1: u64) {
    let t = ee::mul(f32::from_bits(0x3C75_C28F), ee::sub(ee::from_i32(fx.mem.u16(D)), 79.0));
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x18);
    node_position_saving(fx, h, n, sp + 0x70, u64::from(OT), Some(s1), 0x01E0_6ABC);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x1C);
    node_position_saving(fx, h, n, sp + 0x80, u64::from(OT), Some(u64::from(sp + 0x80)), 0x01E0_6AD4);
    for c in 0..3u32 {
        let (a, b) = (fx.mem.f(sp + 0x70 + 4 * c), fx.mem.f(sp + 0x80 + 4 * c));
        fx.mem.setf(sp + 0x70 + 4 * c, ee::add(b, ee::mul(t, ee::sub(a, b))));
    }
    for k in 0..0xCu32 {
        copy_v4(fx, sp + 0x70, D + 0x1C34 + 0x50 * k);
    }
    copy_v4(fx, sp + 0x70, D + 0xAF0);
    copy_v4(fx, sp + 0x70, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let ot = u64::from(OT);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, ot, 0x01E0_642C);
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x140 => return 8,
        0xE8 | 0xCE => arc_rays_start(fx),
        n @ (0xE6 | 0xCC) => s1 = strike(fx, sp, u32::from(n == 0xE6)),
        0xC3 => arc_start(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x50..0x6F).contains(&n) {
        if n == 0x50 {
            cross_start(fx);
            s1 = 4;
        }
        slide(fx, sp, s1);
        s1 = 0xC;
    }
    shake_saving(fx, D + 0x40, ot, 0x01E0_6BC4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    // motes' saved s0..s3 (the OT, s1, the runner's 0, D) stay below the
    // entry's frame: the bead trail's curve samples keep them as w [run].
    crate::b075::motes_saving(fx, D + 0xE4, OT, 8, [ot, s1, 0, u64::from(D)]);
    blib::beam::beads(fx, 0x01E0_2BB0, D + 0x147C, OT, 0x14);
    blib::emit::bead_trails(fx, 0x01E0_3980, D + 0x1B5C, 1);
    blib::cross::bars(fx, 0x01E0_21A0, D + 0x1C14, OT, 0xC);
    blib::star::flash(fx, 0x01E0_15F0, D + 0xAD4, OT, 2);
    blib::arc::arcs(fx, 0x01E0_3ED0, D + 0xA74, OT, 1);
    blib::ray::arc_rays(fx, 0x01E0_5CC0, D + 0x1BE8, 1);
    blib::ray::rays(fx, 0x01E0_5150, D + 0xB6C, OT, 0x14);
    blib::beam::beads(fx, 0x01E0_2BB0, D + 0x147C, OT, 0x14);
    match fx.mem.u16(D) {
        0xE4 => 2,
        0xDD | 0xC3 => 1,
        0xDC => 7,
        0xCA => 6,
        _ => 0,
    }
}
