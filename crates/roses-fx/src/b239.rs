//! BMODULE b239 (card 325), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E052B0, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the "a0 = D" family with s0 = D, s1 = the OT (rand's and the
//! library frames' saved s0 is D), s2 the parameter, then loop counters.
//! Node bursts (+0xA0, motes +0xE4) every update; on update 72 three arcs
//! (+0x1000, `arc::arcs`) from the attacker's nodes 0x13 / 0x19 / 0x25 to
//! 0x12 / 0x18 / 0x24; on 76 three bead-trail emitters (+0x2250,
//! `emit::bead_trails`, beads +0x1120) at nodes 0x13 / 0x19 / 0x25; on 77
//! a ray burst (+0x24DC, `ray::rays`) at node 0x1F, a camera shake and a
//! flash (+0xFB4) at the target; on 78 two more ray bursts (+0x23F4: the
//! target, node 0x13). Returns 1 on update 68, 2 on 75, 8 on 130 (and at
//! once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `beads` 0x1E015F0, `arcs`
//! 0x1E023C0, `rays` 0x1E03640, `flash` 0x1E041B0, `bead_trails`
//! 0x1E04D60 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2870;

/// The entry 0x1E052B0 (a0 = D, a1 = param; s0 = D, s1 = the OT, s2 =
/// param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x2970);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 0xD, 1), (0x10, 0x13, 0), (0x14, 0x19, 0), (0x18, 0x1F, 0), (0x1C, 0x25, 0), (0x20, 0x12, 0), (0x24, 0x18, 0), (0x28, 0x24, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA0): specs 0x1E05D40 / 0x1E05D60, motes +0xE4 /
    // +0x7E4.
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xC8, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xC9, b);
    fx.mem.write_u32(D + 0xA8, 0x01E0_5D40);
    fx.mem.write_u32(D + 0xAC, 0x01E0_5D60);
    fx.mem.write_u32(D + 0xA0, D + 0xE4);
    fx.mem.write_u8(D + 0xD0, 0x20);
    fx.mem.write_u32(D + 0xA4, D + 0x7E4);
    fx.mem.write_u8(D + 0xD1, 0x32);
    fx.mem.write_u8(D + 0xD2, 0);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(D + 0xD3, b);
    fx.mem.write_u16(D + 0xD4, 0x3C);
    fx.mem.write_u16(D + 0xD6, 0x49);
    fx.mem.write_u32(D + 0xD8, 0x430C_0000);
    fx.mem.write_u32(D + 0xDC, 0x430C_0000);
    fx.mem.write_u32(D + 0xE0, 0x40C0_0000);
    // The entry's s0 = D, s1 = the OT, s2 = 1 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_54C0, [Some(D), Some(OT), Some(1)]);
    0
}

/// Node slot (D + `slot`) position into `out` (0x1F6830, its frame).
fn node(fx: &mut Fx, slot: u32, out: u32, ra: u32) {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + slot);
    node_position_saving(fx, h, n, out, u64::from(D), Some(u64::from(OT)), ra);
}

/// Update 72 [F 0x1E05530]: three arcs (+0x1000, 0x60 each), points at
/// +0x2550.
fn arcs_start(fx: &mut Fx) {
    for k in 0..3u32 {
        let g = D + 0x1000 + 0x60 * k;
        set_counter4(fx, g, 0x10, 0, 0xFF);
        fx.mem.write_u8(g + 4, 0xB);
        fx.mem.write_u32(g + 8, D + 0x2550);
        fx.mem.write_u8(g + 0x30, 0x32);
        fx.mem.write_u8(g + 0x31, 8);
        let (a, b) = [(0x10u32, 0x20u32), (0x14, 0x24), (0x1C, 0x28)][k as usize];
        let v = fx.mem.read_u8(D + a);
        fx.mem.write_u8(g + 0x32, v);
        let v = fx.mem.read_u8(D + b);
        fx.mem.write_u8(g + 0x33, v);
        fx.mem.write_u32(g + 0x38, 0x3F99_999A);
        fx.mem.write_u32(g + 0x3C, 0x4059_999A);
        fx.mem.write_u32(g + 0x40, 0x4240_0000);
        for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0xA0), (0x49, 0), (0x48, 0x80), (0x4E, 0xA0), (0x4D, 0), (0x4C, 0x80), (0x4F, 0x80)] {
            fx.mem.write_u8(g + o, v);
        }
        let mut j = 0u32;
        while j < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
            fx.mem.set16(g + 0xC + 2 * j, -1);
            fx.mem.set16(g + 0x1E + 2 * j, -1);
            j += 1;
        }
    }
}

/// Update 76 [F 0x1E056A4]: three bead-trail emitters (+0x2250, 0x8C
/// each) at nodes +0x10 / +0x14 / +0x1C, beads into +0x1120.
fn emitters(fx: &mut Fx) {
    for k in 0..3u32 {
        let e = D + 0x2250 + 0x8C * k;
        set_counter4(fx, e, 4, 0, 0xFF);
        fx.mem.write_u8(e + 4, 0);
        fx.mem.write_u8(e + 5, 1);
        let v = fx.mem.read_u8(D + [0x10u32, 0x14, 0x1C][k as usize]);
        fx.mem.write_u8(e + 6, v);
        fx.mem.write_u32(e + 8, D + 0x1120);
        for (o, v) in [(0x10u32, 0x32u8), (0x12, 0xF), (0x11, 0x14), (0x13, 0)] {
            fx.mem.write_u8(e + o, v);
        }
        fx.mem.write_u32(e + 0x18, 0x3F99_999A);
        fx.mem.write_u32(e + 0x1C, 0x3ECC_CCCD);
        fx.mem.write_u32(e + 0x20, 0x41C8_0000);
        fx.mem.write_u32(e + 0xC, BYTES);
        for (o, v) in [(0x2Au32, 0xC0u8), (0x29, 0x20), (0x28, 0x80), (0x2B, 0xF0)] {
            fx.mem.write_u8(e + o, v);
        }
        node(fx, 0xC + 4 * k, e + 0x3C, 0x01E0_57CC);
        copy_v4(fx, e + 0x3C, e + 0x4C);
    }
}

/// A ray burst's common fields (0x74): count 0x12, 3 table bytes / ray.
fn rays_common(fx: &mut Fx, r: u32, n: u8, ra: u32) {
    set_counter4(fx, r, 0x12, 0, 0xFF);
    fx.mem.write_u8(r + 4, 1);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 8, n);
    let x = fx.rand_from(D, ra);
    fx.mem.set8(r + 9, x >> 4);
}

/// Update 77 [F 0x1E0580C]: a ray burst at node +0x1C, the shake and a
/// flash at the target (sp + 0x60).
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x60;
    let r = D + 0x24DC;
    rays_common(fx, r, 0x32, 0x01E0_5840);
    fx.mem.write_u32(r + 0x18, 0x4270_0000);
    fx.mem.write_u32(r + 0xC, BYTES);
    fx.mem.write_u32(r + 0x10, 0x3FC0_0000);
    for (o, v) in [(0x1Du32, 0u8), (0x1E, 0), (0x1C, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    node(fx, 0x1C, r + 0x24, 0x01E0_5894);
    for o in [0x6Cu32, 0x68, 0x64] {
        fx.mem.set32(r + o, 0x200);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3F80_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
    let r = D + 0xFB4;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4448_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xA, 0), (9, 0), (8, 0x80), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
}

/// Update 78 [F 0x1E059A8]: two ray bursts (+0x23F4): the first at the
/// target (width 90, height 3), the second at node +0x10.
fn bursts(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = D + 0x23F4 + 0x74 * k;
        rays_common(fx, r, 0x3C, 0x01E0_5A04);
        fx.mem.write_u32(r + 0xC, BYTES);
        for (o, v) in [(0x1Eu32, 0u8), (0x1D, 0), (0x1C, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        if k != 0 {
            fx.mem.write_u32(r + 0x18, 0x4270_0000);
            fx.mem.write_u32(r + 0x10, 0x3FC0_0000);
            node(fx, 0x10, r + 0x24, 0x01E0_5A64);
        } else {
            fx.mem.write_u32(r + 0x18, 0x42B4_0000);
            fx.mem.write_u32(r + 0x10, 0x4040_0000);
            copy_v4(fx, sp + 0x60, r + 0x24);
        }
        for o in [0x6Cu32, 0x68, 0x64] {
            fx.mem.set32(r + o, 0x200);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_54E8);
    // The entry's s2 / s3 at the library calls: the parameter (64 bits) and
    // the runner's 0, or the last loop's counter and record offset.
    let (s2, s3) = match fx.mem.u16(D) {
        0x82 => return 8,
        0x4E => {
            bursts(fx, sp);
            (2, 0x74)
        }
        0x4D => {
            strike(fx, sp);
            (i64::from(param) as u64, 0)
        }
        0x4C => {
            emitters(fx);
            (3, 0x118)
        }
        0x48 => {
            arcs_start(fx);
            (3, 0xC0)
        }
        _ => (i64::from(param) as u64, 0),
    };
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5AE4, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA0, n);
    crate::b075::motes_saving(fx, D + 0xE4, OT, 0x20, [u64::from(D), u64::from(OT), s2, s3]);
    blib::emit::bead_trails(fx, 0x01E0_4D60, D + 0x2250, 3);
    blib::beam::beads(fx, 0x01E0_15F0, D + 0x1120, OT, 0x32);
    blib::star::flash(fx, 0x01E0_41B0, D + 0xFB4, OT, 1);
    blib::arc::arcs(fx, 0x01E0_23C0, D + 0x1000, OT, 3);
    blib::ray::rays(fx, 0x01E0_3640, D + 0x23F4, OT, 3);
    match fx.mem.u16(D) {
        0x4B => 2,
        0x44 => 1,
        _ => 0,
    }
}
