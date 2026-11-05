//! BMODULE b437 (card 531), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05CC0, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): the "a0 = D" family with s3 = D, s0 = the OT (rand's and the
//! library frames' saved s0), s1 the parameter. Node bursts (+0xA4, motes
//! +0xE8) every update and the stage light pulsed by 0x1031C0 (+0xA0: from
//! updates 2 / 0x4A); on update 28 an arc (+0xFB8, `arc::arcs`) between
//! the attacker's nodes 0x16 / 0x15; on 39 at the target three spiral
//! swirls (+0x221C, `swirl::arms`), two ribbons (+0x238C), two flashes
//! (+0x2184) and a camera shake; on 40 the bead-trail emitter (+0x20F8,
//! `emit::bead_trails`, beads +0x1338) at node 0x16. Returns 1 on update
//! 31, 2 on 38, 8 on 80 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `beads` 0x1E015F0, `arcs`
//! 0x1E023C0, `flash` 0x1E03640, `bead_trails` 0x1E041F0, `arms`
//! 0x1E04740, `ribbon` 0x1E05130 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x2594;

/// The entry 0x1E05CC0 (a0 = D, a1 = param; s3 = D, s0 = the OT, s1 =
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
    fx.bzero(D, 0x26A0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (at, node, clear) in [(0xCu32, 0xB, 1), (0x10, 0x11, 0), (0x14, 0x16, 0), (0x18, 0x15, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): spec 0x1E067A8 twice, motes +0xE8 / +0x7E8.
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xB0, 0x01E0_67A8);
    fx.mem.write_u32(D + 0xAC, 0x01E0_67A8);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(D + 0xD7, b);
    fx.mem.write_u16(D + 0xD8, 0xE);
    fx.mem.write_u16(D + 0xDA, 0x19);
    fx.mem.write_u32(D + 0xDC, 0x4320_0000);
    fx.mem.write_u32(D + 0xE0, 0x4320_0000);
    fx.mem.write_u32(D + 0xE4, 0x40C0_0000);
    fx.mem.write_u16(D + 0xA0, 2);
    fx.mem.write_u16(D + 0xA2, 0x4A);
    // The entry's s0 = the OT, s1 = 1 (the texture loop's), s2 the
    // runner's.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5E78, [Some(OT), Some(1), None]);
    0
}

/// Update 28 [F 0x1E05EDC]: the arc (+0xFB8) from node +0x14 to node
/// +0x18, points at +0x1018.
fn arc_start(fx: &mut Fx) {
    let g = D + 0xFB8;
    set_counter4(fx, g, 0x18, 0, 0xFF);
    fx.mem.write_u8(g + 4, 0xB);
    fx.mem.write_u32(g + 8, D + 0x1018);
    fx.mem.write_u8(g + 0x30, 0x32);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u32(g + 0x3C, 0x4080_0000);
    fx.mem.write_u32(g + 0x40, 0x42A0_0000);
    for (k, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0xD0), (0x4A, 0x80), (0x49, 0x10), (0x48, 0x10), (0x4E, 0x80), (0x4D, 0x30), (0x4C, 0x30), (0x4F, 0xC0)] {
        fx.mem.write_u8(g + k, v);
    }
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0xC + 2 * k, -1);
        fx.mem.set16(g + 0x1E + 2 * k, -1);
        k += 1;
    }
}

/// Update 39 [F 0x1E05FBC]: at the target (sp + 0x50) three swirls, two
/// ribbons, two flashes and the shake.
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x50;
    let back = fx.mem.read_u32(D + 0x30) == 0x8000;
    for k in 0..3u32 {
        let r = D + 0x221C + 0x5C * k;
        set_counter4(fx, r, 0x12, 0, 0xFF);
        for (o, v) in [(4u32, 1u8), (5, 0), (8, 3), (9, 6), (0xA, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0);
        for (o, v) in [(0x2Au32, 0xC0u8), (0x29, 0), (0x28, 0x10), (0x2B, 0x40), (0x27, 0xC0), (0x24, 0xC0), (0x25, 0xC0), (0x26, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x2C);
        fx.mem.write_u32(r + 0x18, 0x4080_0000);
        fx.mem.write_u32(r + 0x14, 0x4100_0000);
        let (start, end, grow) = match k {
            0 => (0x43B4_0000u32, 0x4334_0000u32, 0x4060_0000u32),
            1 => (0x43C0_8000, 0x434D_0000, 0x4080_0000),
            _ => (0x43CD_0000, 0x4366_0000, 0x4090_0000),
        };
        fx.mem.write_u32(r + 0xC, start);
        fx.mem.write_u32(r + 0x10, end);
        fx.mem.write_u32(r + 0x20, grow);
        if back {
            fx.mem.set32(r + 0x54, 0x4000);
        } else {
            fx.mem.set32(r + 0x54, -0x4000);
            fx.mem.set32(r + 0x50, 0x8000);
        }
    }
    for k in 0..2u32 {
        let r = D + 0x238C + 0x68 * k;
        set_counter4(fx, r, 0x16, 0, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x3C);
        let x = fx.rand_from(OT, 0x01E0_61B0);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x4040_0000);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for (o, v) in [(0x22u32, 0x80u8), (0x21, 0x10), (0x20, 0x10), (0x23, 0x40), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (0x27, 0x80)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x28);
        fx.mem.set32(r + 0x48, if back { -0x4000 } else { 0x4000 });
        fx.mem.set32(r + 0x4C, if k & 1 != 0 { 0x3800 } else { -0x3800 });
    }
    for k in 0..2u32 {
        let r = D + 0x2184 + 0x4C * k;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u8(r + 7, 9);
        let (size, c) = if k != 0 { (0x4416_0000u32, 0xC0u8) } else { (0x44BB_8000, 0xA0) };
        fx.mem.write_u32(r + 0x10, size);
        for (o, v) in [(0xFu32, c), (0xC, c), (0xD, c), (0xE, c), (0xA, 0x80), (9, 0), (8, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x14, 0);
        copy_v4(fx, at, r + 0x1C);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, at, D + 0x60);
}

/// Update 40 [F 0x1E06420]: the bead-trail emitter (+0x20F8) at node
/// +0x14, beads into +0x1338.
fn emitter(fx: &mut Fx, s1: u64) {
    let e = D + 0x20F8;
    set_counter4(fx, e, 4, 0, 0xFF);
    fx.mem.write_u8(e + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(e + 6, b);
    fx.mem.write_u32(e + 8, D + 0x1338);
    for (o, v) in [(0x10u32, 0x28u8), (0x12, 0x1E), (0x11, 0x1A), (0x13, 0)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 0x18, 0x3FB3_3333);
    fx.mem.write_u32(e + 0x1C, 0x3E99_999A);
    fx.mem.write_u32(e + 0x20, 0x41F0_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for (o, v) in [(0x2Au32, 0x80u8), (0x28, 0x10), (0x29, 0x10), (0x2B, 0xC0)] {
        fx.mem.write_u8(e + o, v);
    }
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x14);
    node_position_saving(fx, h, n, e + 0x2C, u64::from(OT), Some(s1), 0x01E0_64C4);
    copy_v4(fx, e + 0x2C, e + 0x4C);
    copy_v4(fx, e + 0x4C, e + 0x3C);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let ot = u64::from(OT);
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, ot, 0x01E0_5EA0);
    // s1: the parameter (64-bit) unless update 39's loops ran.
    let mut s1 = i64::from(param) as u64;
    match fx.mem.u16(D) {
        0x50 => return 8,
        0x28 => emitter(fx, s1),
        0x27 => {
            strike(fx, sp);
            s1 = 2;
        }
        0x1C => arc_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, ot, 0x01E0_6520, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6534, [Some(ot), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_23C0, D + 0xFB8, OT, 1);
    blib::emit::bead_trails(fx, 0x01E0_41F0, D + 0x20F8, 1);
    blib::beam::beads(fx, 0x01E0_15F0, D + 0x1338, OT, 0x28);
    blib::star::flash(fx, 0x01E0_3640, D + 0x2184, OT, 2);
    blib::swirl::arms(fx, 0x01E0_4740, D + 0x221C, OT, 4);
    blib::streak::ribbon(fx, 0x01E0_5130, D + 0x238C, OT, 5);
    match fx.mem.u16(D) {
        0x26 => 2,
        0x1F => 1,
        _ => 0,
    }
}
