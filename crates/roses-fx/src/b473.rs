//! BMODULE b473 (card 588), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06250, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): b155's program shape (s3 = D, s0 = the OT, s1 the parameter
//! then loop counters, s2 offsets) with b437's stage light (+0xA0, from
//! updates 2 / 0x9A) and node bursts (+0xA4, motes +0xE8). Two strikes
//! (updates 52.. and 70..): an arc (+0x1144, `arc::arcs`) between the
//! attacker's nodes 0x24 / 0x22 (then 0x1C / 0x1A), the bead-trail emitter
//! (+0x2500, beads +0x1740) at node 0x22, at the target a camera shake, a
//! flash (+0xFB8) and four bars (+0x1004, `cross::bars`), then ray bursts
//! along the arc (+0x11A4, `ray::arc_rays`, rays +0x11D0). Returns 1 on
//! updates 45 / 65, 6 on 52, 7 on 64, 2 on 72, 8 on 160 (and at once for
//! -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `beads` 0x1E015F0, `arcs`
//! 0x1E023C0, `bars` 0x1E03640, `flash` 0x1E04050, `bead_trails`
//! 0x1E04C00, `rays` 0x1E05150, `arc_rays` 0x1E05CC0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x28AC;

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
    fx.bzero(D, 0x29B0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 1, 1), (0x10, 0xF, 0), (0x14, 9, 0), (0x18, 0x24, 0), (0x1C, 0x22, 0), (0x20, 0x1C, 0), (0x24, 0x1A, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): specs 0x1E06CE0 / 0x1E06CF0, motes +0xE8 /
    // +0x7E8.
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6CE0);
    fx.mem.write_u32(D + 0xB0, 0x01E0_6CF0);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(D + 0xD7, b);
    fx.mem.write_u16(D + 0xD8, 0x29);
    fx.mem.write_u16(D + 0xDA, 0x34);
    fx.mem.write_u32(D + 0xDC, 0x4302_0000);
    fx.mem.write_u32(D + 0xE0, 0x4302_0000);
    fx.mem.write_u32(D + 0xE4, 0x40A0_0000);
    fx.mem.write_u16(D + 0xA0, 2);
    fx.mem.write_u16(D + 0xA2, 0x9A);
    // The entry's s0 = the OT, s1 = 1 (the texture loop's), s2 the
    // runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6458, [Some(OT), Some(1), None]);
    0
}

/// Updates 52 / 70 [F 0x1E064F8]: the arc (+0x1144) from node +0x18 to
/// +0x1C (`second`: +0x20 to +0x24), points at +0x258C.
fn arc_start(fx: &mut Fx, second: bool) {
    let g = D + 0x1144;
    set_counter4(fx, g, 0x16, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x258C);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let (a, b) = if second { (0x20u32, 0x24u32) } else { (0x18, 0x1C) };
    let v = fx.mem.read_u8(D + a);
    fx.mem.write_u8(g + 0x32, v);
    let v = fx.mem.read_u8(D + b);
    fx.mem.write_u8(g + 0x33, v);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    for (o, v) in [(0x44u32, 0xC0u8), (0x45, 0xC0), (0x46, 0xC0), (0x47, 0xC0), (0x4A, 0x40), (0x49, 0), (0x48, 0x40)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * k, -1);
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        k += 1;
    }
}

/// Updates 53 / 73 [F 0x1E065D4]: the bead-trail emitter (+0x2500) at
/// node +0x1C, beads into +0x1740.
fn emitter(fx: &mut Fx, second: bool, s1: u64) {
    let e = D + 0x2500;
    if second {
        set_counter4(fx, e, 6, 0, 0xFF);
        let b = fx.mem.read_u8(D + 0x20);
        fx.mem.write_u8(e + 6, b);
    } else {
        set_counter4(fx, e, 9, 0, 0xFF);
        let b = fx.mem.read_u8(D + 0x18);
        fx.mem.write_u8(e + 6, b);
    }
    fx.mem.write_u8(e + 4, 0);
    fx.mem.write_u8(e + 5, 1);
    fx.mem.write_u32(e + 8, D + 0x1740);
    for (o, v) in [(0x10u32, 0x28u8), (0x12, 0x14), (0x11, 0x13), (0x13, 0)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3F99_999A);
    fx.mem.write_u32(e + 0x1C, 0x3F4C_CCCD);
    fx.mem.write_u32(e + 0x20, 0x41C8_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for (o, v) in [(0x2Au32, 0x80u8), (0x29, 0x10), (0x28, 0x40), (0x2B, 0xA0)] {
        fx.mem.write_u8(e + o, v);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x1C);
    node_position_saving(fx, h, n, e + 0x3C, u64::from(OT), Some(s1), 0x01E0_66B4);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Updates 54 / 74 [F 0x1E067A8]: at the target (sp + 0x50) the shake, a
/// flash and four bars (the first the largest; the others 45 degrees
/// apart from what they held).
fn strike(fx: &mut Fx, sp: u32, second: bool) {
    let at = sp + 0x50;
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
    let r = D + 0xFB8;
    set_counter4(fx, r, 0x14, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4416_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0xA0), (9, 0x20), (8, 0x80), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    for k in 0..4i32 {
        let r = D + 0x1004 + 0x50 * k as u32;
        set_counter4(fx, r, 0x12, (k >> 1) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x44E1_0000);
            fx.mem.write_u32(r + 0x18, if second { 0x42A0_0000 } else { 0x432A_0000 });
        } else {
            fx.mem.write_u32(r + 0x10, 0x442F_0000);
            let v = ee::add(fx.mem.f(r + 0x18), ee::mul(45.0, ee::from_i32(k)));
            fx.mem.setf(r + 0x18, v);
        }
        fx.mem.write_u32(r + 0x14, 0x4248_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x20), (0xA, 0x80), (9, 0x10), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        copy_v4(fx, at, r + 0x20);
    }
}

/// Updates 58 / 76 [F 0x1E066DC]: the arc-ray spawner (+0x11A4) on the
/// arc +0x1144, rays into +0x11D0.
fn arc_rays_start(fx: &mut Fx, second: bool) {
    let g = D + 0x11A4;
    set_counter4(fx, g, 1, 0, 0xFF);
    fx.mem.write_u8(g + 4, 2);
    fx.mem.write_u32(g + 8, D + 0x1144);
    fx.mem.write_u32(g + 0xC, D + 0x11D0);
    fx.mem.write_u8(g + 0x14, 0x14);
    fx.mem.write_u8(g + 0x16, 0x14);
    fx.mem.write_u8(g + 0x15, 0xE);
    fx.mem.write_u8(g + 0x17, if second { 8 } else { 0xB });
    fx.mem.write_u32(g + 0x18, 0x40A0_0000);
    fx.mem.write_u32(g + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(g + 0x20, 0x41F0_0000);
    fx.mem.write_u32(g + 0x10, BYTES);
    for (o, v) in [(0x25u32, 0u8), (0x26, 0), (0x24, 0x80), (0x28, 0x80), (0x29, 0x80), (0x2A, 0x80), (0x2B, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let ot = u64::from(OT);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, ot, 0x01E0_6480);
    // The entry's s1 / s2 at the library calls: the parameter (64 bits)
    // and the runner's 0, or the bar loop's counter and offset.
    let p = i64::from(param) as u64;
    let (s1, s2) = match fx.mem.u16(D) {
        0xA0 => return 8,
        n @ (0x4A | 0x36) => {
            strike(fx, sp, n == 0x4A);
            (4, 0xF0)
        }
        n @ (0x4C | 0x3A) => {
            arc_rays_start(fx, n == 0x4C);
            (p, 0)
        }
        n @ (0x49 | 0x35) => {
            emitter(fx, n == 0x49, p);
            (p, 0)
        }
        n @ (0x46 | 0x34) => {
            arc_start(fx, n == 0x46);
            (p, 0)
        }
        _ => (p, 0),
    };
    shake_saving(fx, D + 0x40, ot, 0x01E0_6A04, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6A14, [Some(ot), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    crate::b075::motes_saving(fx, D + 0xE8, OT, 0x20, [ot, s1, s2, u64::from(D)]);
    blib::emit::bead_trails(fx, 0x01E0_4C00, D + 0x2500, 1);
    blib::beam::beads(fx, 0x01E0_15F0, D + 0x1740, OT, 0x28);
    blib::star::flash(fx, 0x01E0_4050, D + 0xFB8, OT, 1);
    blib::arc::arcs(fx, 0x01E0_23C0, D + 0x1144, OT, 1);
    blib::cross::bars(fx, 0x01E0_3640, D + 0x1004, OT, 4);
    blib::ray::arc_rays(fx, 0x01E0_5CC0, D + 0x11A4, 1);
    blib::ray::rays(fx, 0x01E0_5150, D + 0x11D0, OT, 0xC);
    match fx.mem.u16(D) {
        0x48 => 2,
        0x41 | 0x2D => 1,
        0x40 => 7,
        0x34 => 6,
        _ => 0,
    }
}
