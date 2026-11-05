//! BMODULE b032 (card 154), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E055F0, data area a0 = 0x1ED0000) [F] (helper B6, 31
//! Oct 2026): b063's family (s3 = D, s4 = the OT; s0 the parameter, s1
//! the runner's 0). Node bursts (+0xA4, motes +0xE8, nodes 0x36 / 0x30,
//! updates 106..113) every update; on update 114 two arc pairs (+0xFB8 +
//! 0x60 k, nodes 0x18 / 0x17 and 0x29 / 0x28); on 119, from the
//! attacker's nodes 0x18 and 0x29, two ribbons (+0x15BC) and three cones
//! each (+0x194C, `cone::cones`, delays 4 k), two flashes between them
//! (+0x1398), eight bead beams at the target (+0x168C) and a camera shake
//! there. The stage light pulsed by 0x1031C0 (+0xA0). Returns 1 on update
//! 109, 2 on 116, 8 on 198 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes`
//! 0x1E00410, `node_bursts` 0x1E01130, `arcs` 0x1E015F0, `cones`
//! 0x1E02870, `flash` 0x1E030E0, `ribbon` 0x1E03C90, `beads` 0x1E04820
//! [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x1AFC;

/// The entry 0x1E055F0 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0xB0);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x1C00);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..2 {
        fx.texture(5, k);
    }
    for (at, node, clear) in [(0xCu32, 0x18, 1), (0x10, 0x17, 0), (0x14, 0x29, 0), (0x18, 0x28, 0), (0x1C, 0x30, 0), (0x20, 0x36, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The node bursts (+0xA4): spec 0x1E06190 / 0x1E061A0, motes +0xE8 /
    // +0x7E8.
    let b = fx.mem.read_u8(D + 0x20);
    fx.mem.write_u8(D + 0xCC, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(D + 0xCD, b);
    fx.mem.write_u32(D + 0xAC, 0x01E0_6190);
    fx.mem.write_u32(D + 0xB0, 0x01E0_61A0);
    fx.mem.write_u32(D + 0xA4, D + 0xE8);
    fx.mem.write_u8(D + 0xD4, 0x20);
    fx.mem.write_u32(D + 0xA8, D + 0x7E8);
    fx.mem.write_u8(D + 0xD5, 0x32);
    fx.mem.write_u8(D + 0xD6, 0);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(D + 0xD7, b);
    fx.mem.set16(D + 0xD8, 0x6A);
    fx.mem.set16(D + 0xDA, 0x71);
    fx.mem.write_u32(D + 0xDC, 0x432A_0000);
    fx.mem.write_u32(D + 0xE0, 0x430C_0000);
    fx.mem.write_u32(D + 0xE4, 0x40C0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xC0);
    // The entry's s0 = the parameter, s1 the runner's 0.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_57F0, [Some(param as u32), Some(0), None]);
    0
}

/// rand() with the caller's s0 = `s0` (64 bits) from return address `ra`.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp - 0x10;
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// Update 114 [F 0x1E05848]: two arc pairs, nodes +0xC / +0x10 and +0x14 /
/// +0x18, points +0x1078.
fn arcs_start(fx: &mut Fx) {
    for k in 0..2u32 {
        let g = D + 0xFB8 + 0x60 * k;
        set_counter4(fx, g, 0x15, 0, 0xFF);
        fx.mem.write_u8(g + 4, 1);
        fx.mem.write_u32(g + 8, D + 0x1078);
        fx.mem.write_u8(g + 0x30, 0x32);
        fx.mem.write_u8(g + 0x31, 8);
        let b = fx.mem.read_u8(D + 0xC + 8 * k);
        fx.mem.write_u8(g + 0x32, b);
        let b = fx.mem.read_u8(D + 0x10 + 8 * k);
        fx.mem.write_u8(g + 0x33, b);
        fx.mem.write_u32(g + 0x38, 0x3F80_0000);
        for (o, v) in [(0x44u32, 0x80u8), (0x45, 0x80), (0x46, 0x80), (0x47, 0x80), (0x49, 0x20), (0x4A, 0x20), (0x48, 0x80)] {
            fx.mem.write_u8(g + o, v);
        }
        let mut a = 0u32;
        while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
            fx.mem.set16(g + 0xC + 2 * a, -1);
            fx.mem.set16(g + 0x1E + 2 * a, -1);
            a += 1;
        }
    }
}

/// Update 119 [F 0x1E0593C]: ribbons and cones on nodes +0xC / +0x14 (sp +
/// 0x90 / + 0xA0), flashes between them, beads and the shake at the
/// target (sp + 0x80). Returns the entry's s1 after (the last bead count).
fn strike(fx: &mut Fx, sp: u32, s0: u64) -> u64 {
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0xC);
    node_position_saving(fx, h, n, sp + 0x90, s0, Some(0), 0x01E0_5950);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x14);
    node_position_saving(fx, h, n, sp + 0xA0, s0, Some(0), 0x01E0_5968);
    for k in 0..2u32 {
        let r = D + 0x15BC + 0x68 * k;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x46);
        let x = rand64(fx, s0, 0x01E0_59C0);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x1C, 0x428C_0000);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x4073_3333);
        fx.mem.write_u32(r + 0x18, 0x3FCC_CCCD);
        for (o, v) in [(0x22u32, 0x20u8), (0x21, 0), (0x20, 0x80), (0x23, 0x18), (0x27, 0x60), (0x24, 0x60), (0x25, 0x60), (0x26, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        let at = sp + 0x90 + 16 * k;
        copy_v4(fx, at, r + 0x28);
        let turn = if fx.mem.read_u32(D + 0x30) == 0x8000 { 0x3000 } else { -0x3000 };
        fx.mem.set32(r + 0x48, turn);
        for j in 0..3u32 {
            let c = D + 0x194C + 0x48 * (j + 3 * k);
            set_counter4(fx, c, 0x14, ((j << 2) & 0xFF) as i32, 0xFF);
            for (o, v) in [(4u32, 0u8), (5, 0), (7, 9), (8, 2), (0xA, 0x20), (9, 7)] {
                fx.mem.write_u8(c + o, v);
            }
            fx.mem.write_u32(c + 0xC, 0x40A0_0000);
            fx.mem.write_u32(c + 0x10, 0x3F4C_CCCD);
            fx.mem.write_u32(c + 0x14, 0x4000_0000);
            fx.mem.write_u32(c + 0x18, 0);
            fx.mem.write_u32(c + 0x1C, 0x3E99_999A);
            for (o, v) in [(0x23u32, 0x60u8), (0x20, 0x60), (0x21, 0x60), (0x22, 0x60), (0x25, 0), (0x26, 0), (0x24, 0x80), (0x27, 0x60)] {
                fx.mem.write_u8(c + o, v);
            }
            copy_v4(fx, at, c + 0x28);
            let a = fx.mem.read_u32(D + 0x1604);
            fx.mem.write_u32(c + 0x38, a);
        }
    }
    for k in 0..2u32 {
        let r = D + 0x1398 + 0x4C * k;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u8(r + 7, 9);
        let (w, c, a, b9, b8) = if k != 0 { (0x4416_0000u32, 0xC0u8, 0x80u8, 0u8, 0x10u8) } else { (0x44BB_8000, 0xA0, 0x10, 0, 0x80) };
        fx.mem.write_u32(r + 0x10, w);
        for o in [0xFu32, 0xC, 0xD, 0xE] {
            fx.mem.write_u8(r + o, c);
        }
        fx.mem.write_u8(r + 0xA, a);
        fx.mem.write_u8(r + 9, b9);
        fx.mem.write_u8(r + 8, b8);
        fx.mem.write_u32(r + 0x14, 0);
        for o in 0..3u32 {
            let p = fx.mem.f(sp + 0x90 + 4 * o);
            let q = fx.mem.f(sp + 0xA0 + 4 * o);
            fx.mem.setf(r + 0x1C + 4 * o, ee::add(p, ee::div(ee::sub(q, p), 2.0)));
        }
    }
    let mut s1 = 0u64;
    for k in 0..8u32 {
        let s0 = u64::from(k);
        let count = (((rand64(fx, s0, 0x01E0_5CD0) >> 4) & 3) + 0x1E) & 0xFF;
        s1 = count as u64;
        let delay = (rand64(fx, s0, 0x01E0_5CE8) >> 4) & 7;
        let b = D + 0x168C + 0x58 * k;
        set_counter4(fx, b, count, delay, 0xFF);
        fx.mem.write_u8(b + 4, 7);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x32);
        let x = rand64(fx, s0, 0x01E0_5D3C);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u8(b + 0xA, 1);
        fx.mem.write_u8(b + 0xC, 2);
        fx.mem.write_u8(b + 0xB, 3);
        fx.mem.setf(b + 0x1C, ee::add(25.0, ee::mul(2.0, ee::from_i32(k as i32))));
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        fx.mem.write_u32(b + 0x14, 0x3FB3_3333);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(b + o, 0x80);
        }
        copy_v4(fx, sp + 0x80, b + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x16, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x4000_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x80, D + 0x60);
    s1
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let mut s0 = i64::from(param) as u64;
    // The interpreter runner's s1 is 0 at the entry [run, helper B3].
    let mut s1 = 0u64;
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x80, s0, 0x01E0_5818);
    match fx.mem.u16(D) {
        0xC6 => return 8,
        0x77 => {
            s1 = strike(fx, sp, s0);
            s0 = 8;
        }
        0x72 => arcs_start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, s0, 0x01E0_5E84, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5E94, [Some(s0), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts(fx, D + 0xA4, n);
    blib::mote::motes(fx, D + 0xE8, OT, 0x20);
    blib::arc::arcs(fx, 0x01E0_15F0, D + 0xFB8, OT, 2);
    blib::cone::cones(fx, 0x01E0_2870, D + 0x194C, OT, 6);
    blib::star::flash(fx, 0x01E0_30E0, D + 0x1398, OT, 2);
    blib::beam::beads(fx, 0x01E0_4820, D + 0x168C, OT, 8);
    blib::streak::ribbon(fx, 0x01E0_3C90, D + 0x15BC, OT, 2);
    match fx.mem.u16(D) {
        0x74 => 2,
        0x6D => 1,
        _ => 0,
    }
}
