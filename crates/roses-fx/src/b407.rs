//! BMODULE b407 (card 515), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04DC0, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family with s0 = D, s1 = the OT, s2 = the
//! parameter then the loop counts, and the recompiled swirl arms
//! (1c4c37ddd46b), no stage light. On 51 a bead beam (+0xA0, `beads_at 40
//! 44`) at the attacker's node 0x34; on 51..109 a swirl arm an update by
//! the local 0x1E04980 (6842519a314a: +0x1664, 0x5C each, 60 slots, turned
//! -+0x1000 by the facing); on 74 at the target two halos (+0x15CC, 1500 /
//! 600), two spikes (+0x2C44), a bar 6000 long (+0x2BF4), four bead beams
//! (turned +-0x4000) and a shake; the object ops of the table 0x1E05D40
//! every update. The bead-trail emitter +0x1540 runs but is never set.
//! Returns 1 on 64, 2 on 71, 8 on 195 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads_at 40
//! 44` 0x1E00440, `bead_trails_e` 0x1E01350, `halo` 0x1E01A30, `arms_c`
//! 0x1E02780, `bars_c` 0x1E03180, `spikes_b` 0x1E03C50 [`crate::blib`].
use crate::b035::object_ops;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BEADS: u32 = D + 0xA0;
const EMITTER: u32 = D + 0x1540;
const HALOS: u32 = D + 0x15CC;
const ARMS: u32 = D + 0x1664;
const BAR: u32 = D + 0x2BF4;
const SPIKES: u32 = D + 0x2C44;
const BYTES: u32 = D + 0x2D14;
/// The object-op table (rodata).
const OPS: u32 = 0x01E0_5D40;

/// The entry 0x1E04DC0 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x2E20);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    for (o, node, clear) in [(0xCu32, 1, 1), (0x10, 0x34, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    // The entry's s0 = D, s1 = the OT, s2 = 1 (the texture loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_4EC8, [Some(D), Some(OT), Some(1)]);
    0
}

/// The local 0x1E04980(D) [F]: a swirl arm at the attacker's node slot
/// +0x10 (its frame 0x40: the caller's s0 = D / s1 = the OT / ra saved).
fn arm(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, 0x01E0_5A7C)]);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, slot, sp + 0x30, u64::from(D), Some(u64::from(D)), 0x01E0_49A8);
    let i = first_free(fx, ARMS, 0x5C, 0x3C);
    if i >= 0 {
        let r = ARMS + 0x5C * i as u32;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        for (o, v) in [(4u32, 0u8), (5, 0), (8, 1), (9, 0), (0xA, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x4100_0000);
        for (o, v) in [(0x2Au32, 0x60u8), (0x29, 0), (0x28, 0x80), (0x2B, 0x40), (0x27, 0x90), (0x24, 0x90), (0x25, 0x90), (0x26, 0x90)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, sp + 0x30, r + 0x2C);
        fx.mem.write_u32(r + 0x18, 0x4170_0000);
        fx.mem.write_u32(r + 0x14, 0x4140_0000);
        let x = fx.rand_from(i as u32, 0x01E0_4C20) >> 4;
        let (_, h) = fx.div(x, 0x168);
        let a = ee::add(360.0, ee::div(ee::from_i32(h.wrapping_mul(200).wrapping_sub(0x8CA0)), 100.0));
        fx.mem.setf(r + 0xC, a);
        let e = ee::add(60.0, fx.mem.f(r + 0xC));
        fx.mem.setf(r + 0x10, e);
        fx.mem.write_u32(r + 0x20, 0x3F80_0000);
        let back = fx.mem.i32(D + 0x30) == 0x8000;
        let (a, b) = if back { (0x1000, -0x1000) } else { (-0x1000, 0x1000) };
        fx.mem.set32(r + 0x4C, a);
        fx.mem.set32(r + 0x54, b);
    }
    fx.leave(0x40);
}

/// Update 0x33 [F 0x1E04F20]: a bead beam at the attacker's node slot +0x10.
fn bead(fx: &mut Fx, sp: u32) {
    let r = BEADS;
    set_counter4(fx, r, 0x10, 0, 0xFF);
    fx.mem.write_u32(r + 0x1C, 0x41A0_0000);
    fx.mem.write_u32(r + 0x14, 0x3FB3_3333);
    for (o, v) in [(0xAu32, 0u8), (8, 0x50), (0x26, 0x80), (0x25, 0x40), (0x24, 0x20), (0x27, 0xF0), (4, 1), (5, 0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(D, 0x01E0_4F8C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    copy_v4(fx, sp + 0x50, r + 0x28);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x10));
    node_position_saving(fx, h, slot, r + 0x28, u64::from(D), Some(u64::from(OT)), 0x01E0_4FD4);
}

/// Update 0x4A [F 0x1E04FDC]: halos, spikes, a bar, four bead beams and
/// the shake at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = HALOS + 0x4C * k;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x81);
        fx.mem.write_u8(r + 7, 9);
        if k != 0 {
            fx.mem.write_u32(r + 0x10, 0x4416_0000);
            for (o, v) in [(0xFu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0), (8, 0x10)] {
                fx.mem.write_u8(r + o, v);
            }
        } else {
            fx.mem.write_u32(r + 0x10, 0x44BB_8000);
            for (o, v) in [(0xFu32, 0xA0u8), (0xC, 0xA0), (0xD, 0xA0), (0xE, 0xA0), (0xA, 0x10), (9, 0), (8, 0x80)] {
                fx.mem.write_u8(r + o, v);
            }
        }
        fx.mem.write_u32(r + 0x14, 0);
        copy_v4(fx, sp + 0x50, r + 0x1C);
    }
    for k in 0..2u32 {
        let r = SPIKES + 0x68 * k;
        fx.mem.write_u8(r + 4, 5);
        if k & 1 != 0 {
            set_counter4(fx, r, 0x1C, 0, 0xFF);
            let f = fx.mem.read_u8(r + 4);
            fx.mem.write_u8(r + 4, f | 2);
            fx.mem.write_u32(r + 0x14, 0x4020_0000);
            fx.mem.write_u32(r + 0x1C, 0x4270_0000);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x80);
        } else {
            set_counter4(fx, r, 0x18, 0, 0xFF);
            fx.mem.write_u32(r + 0x14, 0x4040_0000);
            fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
            fx.mem.write_u8(r + 0x27, 0xC0);
            fx.mem.write_u8(r + 0x23, 0);
        }
        for (o, v) in [(5u32, 0u8), (0xA, 0), (0xB, 2), (0xC, 3), (8, 0x3C)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_5530);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
    }
    let b = BAR;
    set_counter4(fx, b, 0x16, 4, 0xFF);
    fx.mem.write_u8(b + 5, 0);
    copy_v4(fx, sp + 0x50, b + 0x20);
    fx.mem.write_u8(b + 4, 0x81);
    fx.mem.write_u32(b + 0x10, 0x45BB_8000);
    fx.mem.write_u32(b + 0x14, 0x42A0_0000);
    for (o, v) in [(0xDu32, 0x40u8), (0xE, 0x40), (0xC, 0x80), (9, 0x10), (0xA, 0x10), (8, 0x40), (0xF, 0x60)] {
        fx.mem.write_u8(b + o, v);
    }
    fx.mem.write_u32(b + 0x1C, 0);
    fx.mem.write_u32(b + 0x18, 0);
    for k in 0..4u32 {
        let r = BEADS + 0x58 * k;
        set_counter4(fx, r, 0x1C, ((k & 1) << 1) as i32, 0xFF);
        fx.mem.write_u32(r + 0x1C, 0x4220_0000);
        fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
        fx.mem.write_u32(r + 0x18, 0x3E99_999A);
        for (o, v) in [(0xAu32, 0u8), (0xC, 2), (8, 0x50), (0x27, 0x80), (0x24, 0x80), (0x25, 0x80), (0x26, 0x80), (4, 6), (5, 0)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(D, 0x01E0_58E0);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        copy_v4(fx, sp + 0x50, r + 0x28);
        fx.mem.set32(r + 0x50, if k < 2 { 0x4000 } else { -0x4000 });
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FB3_3333);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, s + 0x20);
}

fn step(fx: &mut Fx, sp: u32, _param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(D), 0x01E0_4EF0);
    match fx.mem.u16(D) {
        0xC3 => return 8,
        0x4A => strike(fx, sp),
        0x33 => bead(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x33..0x6E).contains(&n) {
        arm(fx);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_5A88, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    object_ops(fx, OPS, n);
    blib::emit::bead_trails_e(fx, 0x01E0_1350, EMITTER, 1);
    blib::beam::beads_at(fx, 0x01E0_0440, 0x40, 0x44, BEADS, OT, 0x3C);
    blib::halo::halo(fx, 0x01E0_1A30, HALOS, OT, 2);
    blib::swirl::arms_c(fx, 0x01E0_2780, ARMS, OT, 0x3C);
    blib::cross::bars_c(fx, 0x01E0_3180, BAR, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_3C50, SPIKES, OT, 2);
    match fx.mem.u16(D) {
        0x47 => 2,
        0x40 => 1,
        _ => 0,
    }
}
