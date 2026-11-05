//! BMODULE b114 (card 63), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05500, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family (s3 = D, s1 = the parameter then the loop
//! counts, s0 = the OT) with the recompiled node bursts / motes
//! (1e40f1216565 / 90a569cefb1c) and no stage light. Node bursts every
//! update (+0xA0: cue lists 0x1E064B0 / 0x1E064B8 on the attacker's nodes
//! 0xA / 0x11, a scatter on node 0x11 over 58..66, groups +0xE4 (8),
//! sprites +0x2A4 (50)); on 32 two arcs (+0xA74, 0x60 each: 0x82 counts,
//! tracks of 4, from nodes 0x26 / 0x27 to 0x22); on 75 a shake (+0x40,
//! mode 8); on 75..144 twice an update at the target +- 14 in x / y a
//! halo of 300 (+0xDB4, 0x4C each), a spike (+0x1734, 5 / 40) and a bead
//! beam (+0x1DB4, `beads_at 40 44`). Returns 1 on 65, 6 on 75..144, 2 on
//! 145, 8 on 200 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes_c`
//! 0x1E00440, `ring_spawn_b` 0x1E00F00, `node_bursts_b` 0x1E01250, `arcs`
//! (fb70e0bee134) 0x1E01730, `halo` 0x1E02B70, `spikes_b` 0x1E038C0,
//! `beads_at 40 44` 0x1E045F0 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BURSTS: u32 = D + 0xA0;
const GROUPS: u32 = D + 0xE4;
const ARCS: u32 = D + 0xA74;
const HALOS: u32 = D + 0xDB4;
const SPIKES: u32 = D + 0x1734;
const BEADS: u32 = D + 0x1DB4;
const BYTES: u32 = D + 0x28B4;

/// The entry 0x1E05500 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x29C0);
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
    for (o, node, clear) in [(0xCu32, 0x27, 1), (0x10, 0x26, 0), (0x14, 0x22, 0), (0x18, 0x11, 0), (0x1C, 0xA, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    let p = BURSTS;
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(p + 0x28, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(p + 0x29, b);
    for (o, v) in [(8u32, 0x01E0_64B0u32), (0xC, 0x01E0_64B8), (0, GROUPS), (4, D + 0x2A4)] {
        fx.mem.write_u32(p + o, v);
    }
    fx.mem.write_u8(p + 0x30, 8);
    fx.mem.write_u8(p + 0x31, 0x32);
    fx.mem.write_u8(p + 0x32, 0);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(p + 0x33, b);
    fx.mem.write_u16(p + 0x34, 0x3A);
    fx.mem.write_u16(p + 0x36, 0x42);
    fx.mem.write_u32(p + 0x38, 0x4334_0000);
    fx.mem.write_u32(p + 0x3C, 0x4316_0000);
    fx.mem.write_u32(p + 0x40, 0x40E0_0000);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_56D0, [Some(OT), Some(2), Some(0)]);
    0
}

/// Update 0x20 [F 0x1E05728]: two arcs from the attacker's nodes +0x10 /
/// +0xC to +0x14.
fn arcs(fx: &mut Fx) {
    for k in 0..2u32 {
        let r = ARCS + 0x60 * k;
        set_counter4(fx, r, 0x82, 0, 0xFF);
        fx.mem.write_u8(r + 4, 1);
        fx.mem.write_u32(r + 8, D + 0xB34);
        fx.mem.write_u8(r + 0x30, 0x28);
        fx.mem.write_u8(r + 0x31, 4);
        let b = fx.mem.read_u8(D + if k == 0 { 0x10 } else { 0xC });
        fx.mem.write_u8(r + 0x32, b);
        let b = fx.mem.read_u8(D + 0x14);
        fx.mem.write_u8(r + 0x33, b);
        fx.mem.write_u32(r + 0x38, 0x3F80_0000);
        fx.mem.write_u8(r + 0x34, 1);
        fx.mem.write_u16(r + 0x36, 0x10);
        for (o, v) in [(0x47u32, 0xA0u8), (0x44, 0xA0), (0x45, 0xA0), (0x46, 0xA0), (0x4A, 0xC0), (0x49, 0x10), (0x48, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        let mut j = 0;
        while j < fx.mem.u8(r + 0x31) + 1 {
            fx.mem.set16(r + 0x1E + 2 * j as u32, -1);
            fx.mem.set16(r + 0xC + 2 * j as u32, -1);
            j += 1;
        }
    }
}

/// x + ((rand >> 4) % 2800 - 1400) / 100 [F].
fn jitter(fx: &mut Fx, ra: u32, x: f32) -> f32 {
    let r = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(r, 0xAF0);
    ee::add(x, ee::div(ee::from_i32(h - 0x578), 100.0))
}

/// Updates 0x4B..0x90 [F 0x1E05A38]: twice a halo, a spike and a bead beam
/// near the target (sp + 0x60).
fn sparks(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        let x = fx.mem.f(sp + 0x50);
        let v = jitter(fx, 0x01E0_5A40, x);
        fx.mem.setf(sp + 0x60, v);
        let y = fx.mem.f(sp + 0x54);
        let v = jitter(fx, 0x01E0_5A8C, y);
        fx.mem.setf(sp + 0x64, v);
        let z = fx.mem.read_u32(sp + 0x58);
        fx.mem.write_u32(sp + 0x68, z);
        let i = first_free(fx, HALOS, 0x4C, 0x20);
        if i >= 0 {
            let r = HALOS + 0x4C * i as u32;
            set_counter4(fx, r, 0xC, 0, 0xFF);
            for (o, v) in [(5u32, 0u8), (4, 1), (7, 9), (0xF, 0xA0), (0xC, 0xA0), (0xD, 0xA0), (0xE, 0xA0), (0xA, 0x60), (9, 0x10), (8, 0x40)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            copy_v4(fx, sp + 0x60, r + 0x1C);
        }
        let i = first_free(fx, SPIKES, 0x68, 0x10);
        if i >= 0 {
            let r = SPIKES + 0x68 * i as u32;
            fx.mem.write_u8(r + 4, 5);
            set_counter4(fx, r, 8, 0, 0xFF);
            fx.mem.write_u32(r + 0x14, 0x40A0_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            for (o, v) in [(0x27u32, 0xC0u8), (0x23, 0), (5, 0), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0x28)] {
                fx.mem.write_u8(r + o, v);
            }
            let x = fx.rand_from(OT, 0x01E0_5E60);
            fx.mem.set8(r + 9, x >> 4);
            fx.mem.write_u32(r + 0x10, BYTES);
            fx.mem.write_u32(r + 0x18, 0x3F80_0000);
            for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
                fx.mem.write_u8(r + o, 0x80);
            }
            copy_v4(fx, sp + 0x60, r + 0x28);
        }
        let i = first_free(fx, BEADS, 0x58, 0x20);
        if i >= 0 {
            let r = BEADS + 0x58 * i as u32;
            set_counter4(fx, r, 0x10, 0, 0xFF);
            fx.mem.write_u8(r + 4, 5);
            fx.mem.write_u8(r + 5, 0);
            fx.mem.write_u8(r + 8, 0x1E);
            let x = fx.rand_from(OT, 0x01E0_607C);
            fx.mem.set8(r + 9, x >> 4);
            for (o, v) in [(0xAu32, 1u8), (0xB, 3), (0xC, 2)] {
                fx.mem.write_u8(r + o, v);
            }
            fx.mem.write_u32(r + 0x1C, 0x420C_0000);
            fx.mem.write_u32(r + 0x20, 0);
            fx.mem.write_u32(r + 0x10, BYTES);
            fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
            for o in [0x27u32, 0x24, 0x25, 0x26] {
                fx.mem.write_u8(r + o, 0x80);
            }
            copy_v4(fx, sp + 0x60, r + 0x28);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_56F8);
    match fx.mem.u16(D) {
        0xC8 => return 8,
        0x4B => {
            let s = D + 0x40;
            set_counter4(fx, s, 0x49, 0, 0xFF);
            fx.mem.write_u8(s + 5, 0);
            fx.mem.write_u8(s + 4, 8);
            fx.mem.write_u32(s + 0x14, 0x4000_0000);
            fx.mem.write_u32(s + 0x18, 0x3F80_0000);
            fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
            fx.mem.write_u32(s + 0x30, 0);
            fx.mem.write_u32(s + 0x34, 0x42A0_0000);
            fx.mem.write_u32(s + 0x50, 0x40C0_0000);
            copy_v4(fx, sp + 0x50, s + 0x20);
        }
        0x20 => arcs(fx),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x4B..0x91).contains(&n) {
        sparks(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_624C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts_b(fx, BURSTS, n);
    blib::mote::motes_c(fx, GROUPS, OT, 8);
    blib::arc::arcs(fx, 0x01E0_1730, ARCS, OT, 2);
    blib::halo::halo(fx, 0x01E0_2B70, HALOS, OT, 0x20);
    blib::spike::spikes_b(fx, 0x01E0_38C0, SPIKES, OT, 0x10);
    blib::beam::beads_at(fx, 0x01E0_45F0, 0x40, 0x44, BEADS, OT, 0x20);
    let n = fx.mem.u16(D);
    if (0x4B..0x91).contains(&n) {
        6
    } else if n == 0x91 {
        2
    } else if n == 0x41 {
        1
    } else {
        0
    }
}
