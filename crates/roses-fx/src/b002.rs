//! BMODULE b002 (card 209), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06100, data area a0 = 0x1ED0000) [F] (helper A20,
//! 1 Nov 2026): b468's family without a stage light (s4 = D, s1 = the OT,
//! s0 = the parameter then the bead beams' counts) on register-renamed
//! copies of motes_c / arcs / spikes_b / beads_at (5a232c3e2861 /
//! 3da19552fc79 / c57606c8a0fd / a66fb5a7bd82: the XYZ merges' temporaries
//! a0 / a1 for a2 / a3) and smears_d. Node bursts every update (+0xA0: cue
//! lists 0x1E06E60 / 0x1E06E70 on the attacker's nodes +0x24 / +0x28,
//! groups +0xE4 (32), sprites +0x7E4 (50)); on 0x14 a halo of 80 on node
//! +0xC (+0xFB4) and a smear from node +0xC to +0x10 (+0x1128); on 0x76
//! two arcs (+0x1000, nodes +0x14 -> +0x18 and +0x1C -> +0x20); on 0x7A at
//! the target a spike (+0x10C0), a halo of 900, eight bead beams (+0x13F0)
//! and a shake. Returns 1 on 0x70, 2 on 0x7D, 8 on 0xE6 (and at once for
//! -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes_c`
//! 0x1E00440, `ring_spawn_b` 0x1E00F00, `node_bursts_b` 0x1E01250, `arcs`
//! 0x1E01730, `halo` 0x1E02B70, `spikes_b` 0x1E038C0, `beads_at 40 44`
//! 0x1E045F0, `smears_d` 0x1E05500 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const SHAKE: u32 = D + 0x40;
const BURSTS: u32 = D + 0xA0;
const GROUPS: u32 = D + 0xE4;
const HALO: u32 = D + 0xFB4;
const ARCS: u32 = D + 0x1000;
const SPIKE: u32 = D + 0x10C0;
const SMEAR: u32 = D + 0x1128;
const POOL: u32 = D + 0x1170;
const BEADS: u32 = D + 0x13F0;
const BYTES: u32 = D + 0x16B0;

/// The entry 0x1E06100 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x17B0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    for k in 0..3 {
        fx.texture(5, k);
    }
    for (o, node, clear) in [(0xCu32, 0xE, 1), (0x10, 0xF, 0), (0x14, 0x1A, 0), (0x18, 0x1B, 0), (0x1C, 0x24, 0), (0x20, 0x23, 0), (0x24, 0x2C, 0), (0x28, 0x36, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    let p = BURSTS;
    let b = fx.mem.read_u8(D + 0x24);
    fx.mem.write_u8(p + 0x28, b);
    let b = fx.mem.read_u8(D + 0x28);
    fx.mem.write_u8(p + 0x29, b);
    for (o, v) in [(8u32, 0x01E0_6E60u32), (0xC, 0x01E0_6E70), (0, GROUPS)] {
        fx.mem.write_u32(p + o, v);
    }
    fx.mem.write_u8(p + 0x30, 0x20);
    fx.mem.write_u32(p + 4, D + 0x7E4);
    fx.mem.write_u8(p + 0x31, 0x32);
    fx.mem.write_u8(p + 0x32, 0);
    let b = fx.mem.read_u8(D + 0x24);
    fx.mem.write_u8(p + 0x33, b);
    fx.mem.write_u16(p + 0x34, 0x5C);
    fx.mem.write_u16(p + 0x36, 0x6E);
    fx.mem.write_u32(p + 0x38, 0x4348_0000);
    fx.mem.write_u32(p + 0x3C, 0x4316_0000);
    fx.mem.write_u32(p + 0x40, 0x40C0_0000);
    // The entry's s0 = the parameter, s1 = the OT, s2 = 3 (the texture
    // loop's).
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6318, [Some(param as u32), Some(OT), Some(3)]);
    0
}

/// rand() called with the caller's 64-bit s0 from return address `ra`:
/// its frame (s0 +0, ra +8) below the caller's sp.
fn rand64(fx: &mut Fx, s0: u64, ra: u32) -> i32 {
    let f = fx.sp.wrapping_sub(0x10);
    fx.mem.write_u64(f, s0);
    fx.mem.write_u64(f + 8, u64::from(ra));
    fx.rand()
}

/// Update 0x14 [F 0x1E0637C]: a halo of 80 on node +0xC and a smear from
/// node +0xC to +0x10.
fn start(fx: &mut Fx) {
    let h = HALO;
    set_counter4(fx, h, 0xC, 0, 0xFF);
    fx.mem.write_u8(h + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(h + 6, v);
    for (o, v) in [(4u32, 0x80u8), (7, 9), (0xA, 0x80), (9, 0), (8, 0x20), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)] {
        fx.mem.write_u8(h + o, v);
    }
    for (o, v) in [(0x10u32, 0x42A0_0000u32), (0x14, 0x4220_0000), (0x18, 0x4100_0000)] {
        fx.mem.write_u32(h + o, v);
    }
    let r = SMEAR;
    set_counter4(fx, r, 0xC8, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let v = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 0x21, v);
    let v = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 0x22, v);
    fx.mem.write_u32(r + 8, POOL);
    for (o, v) in [(0x1Eu32, 0x28u8), (0x1F, 8), (0x20, 1)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x2C, 0x4316_0000);
    fx.mem.write_u32(r + 0x28, 0x3F11_EB85);
    for (o, v) in [(0x34u32, 0x80u8), (0x35, 0x80), (0x36, 0x80), (0x37, 0x40)] {
        fx.mem.write_u8(r + o, v);
    }
    let mut j = 0;
    while j < fx.mem.u8(r + 0x1F) + 1 {
        fx.mem.set16(r + 0xC + 2 * j as u32, -1);
        j += 1;
    }
    fx.mem.write_u32(r + 0x30, 0xBF80_0000);
}

/// Update 0x76 [F 0x1E064B4]: two arcs, nodes +0x14 -> +0x18 and +0x1C ->
/// +0x20.
fn arcs(fx: &mut Fx) {
    for s in 0..2u32 {
        let r = ARCS + 0x60 * s;
        set_counter4(fx, r, 0x10, 0, 0xFF);
        fx.mem.write_u8(r + 4, 1);
        fx.mem.write_u32(r + 8, POOL);
        fx.mem.write_u8(r + 0x30, 0x28);
        fx.mem.write_u8(r + 0x31, 6);
        let v = fx.mem.read_u8(D + 0x14 + 8 * s);
        fx.mem.write_u8(r + 0x32, v);
        let v = fx.mem.read_u8(D + 0x18 + 8 * s);
        fx.mem.write_u8(r + 0x33, v);
        fx.mem.write_u32(r + 0x38, 0x3F80_0000);
        fx.mem.write_u8(r + 0x34, 0);
        fx.mem.write_u16(r + 0x36, 0);
        for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0x80), (0x49, 0x80), (0x48, 0x10)] {
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

/// Update 0x7A [F 0x1E066EC]: at the target (sp + 0x60) a spike, a halo of
/// 900, eight bead beams and the shake; returns the entry's s0.
fn strike(fx: &mut Fx, sp: u32, mut s0: u64) -> u64 {
    let r = SPIKE;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x3FC0_0000);
    fx.mem.write_u32(r + 0x1C, 0x4270_0000);
    for (o, v) in [(5u32, 0u8), (0xA, 2), (0xB, 2), (0xC, 3), (8, 0xA0)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = rand64(fx, s0, 0x01E0_6748);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for (o, v) in [(0x24u32, 0x80u8), (0x25, 0x80), (0x26, 0x80), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x20), (0x27, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    copy_v4(fx, sp + 0x60, r + 0x28);
    for o in [0x58u32, 0x5C, 0x60] {
        fx.mem.write_u32(r + o, 0x100);
    }
    let h = HALO;
    set_counter4(fx, h, 0x20, 0, 0xFF);
    fx.mem.write_u8(h + 5, 0);
    for (o, v) in [(4u32, 0x81u8), (7, 9), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(h + o, v);
    }
    fx.mem.write_u32(h + 0x14, 0);
    fx.mem.write_u32(h + 0x10, 0x4461_0000);
    copy_v4(fx, sp + 0x60, h + 0x1C);
    for s in 0..8i32 {
        let i = first_free(fx, BEADS, 0x58, 8);
        if i < 0 {
            continue;
        }
        let x = rand64(fx, s0, 0x01E0_6884);
        let count = ((x >> 4) & 3) + 0x16;
        s0 = (count & 0xFF) as u64;
        let x = rand64(fx, s0, 0x01E0_689C);
        let r = BEADS + 0x58 * i as u32;
        set_counter4(fx, r, count & 0xFF, (x >> 4) & 7, 0xFF);
        fx.mem.write_u8(r + 4, 7);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x28);
        let x = rand64(fx, s0, 0x01E0_6934);
        fx.mem.set8(r + 9, x >> 4);
        for (o, v) in [(0xAu32, 2u8), (0xB, 3), (0xC, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        let v = ee::add(16.0, ee::mul(2.0, ee::from_i32(s)));
        fx.mem.setf(r + 0x1C, v);
        fx.mem.write_u32(r + 0x20, 0);
        fx.mem.write_u32(r + 0x10, BYTES);
        let x = rand64(fx, s0, 0x01E0_6A3C) >> 4;
        let (_, m) = fx.div(x, 100);
        let v = ee::add(f32::from_bits(0x3F99_999A), ee::div(ee::from_i32(m - 0x32), 100.0));
        fx.mem.setf(r + 0x14, v);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x90);
        }
        copy_v4(fx, sp + 0x60, r + 0x28);
    }
    let s = SHAKE;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    for (o, v) in [(0x14u32, 0x3FE6_6666u32), (0x18, 0x3FE6_6666), (0x1C, 0x3F80_0000), (0x30, 0), (0x34, 0x4220_0000), (0x50, 0x4080_0000)] {
        fx.mem.write_u32(s + o, v);
    }
    copy_v4(fx, sp + 0x60, s + 0x20);
    s0
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut s0 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, s0, 0x01E0_6340);
    match fx.mem.u16(D) {
        0xE6 => return 8,
        0x7A => s0 = strike(fx, sp, s0),
        0x76 => arcs(fx),
        0x14 => start(fx),
        _ => {}
    }
    shake_saving(fx, SHAKE, s0, 0x01E0_6C08, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts_b(fx, BURSTS, n);
    blib::mote::motes_c(fx, GROUPS, OT, 0x20);
    blib::smear::smears_d(fx, 0x01E0_5500, SMEAR, OT, 1);
    blib::arc::arcs(fx, 0x01E0_1730, ARCS, OT, 2);
    blib::spike::spikes_b(fx, 0x01E0_38C0, SPIKE, OT, 1);
    blib::halo::halo(fx, 0x01E0_2B70, HALO, OT, 1);
    blib::beam::beads_at(fx, 0x01E0_45F0, 0x40, 0x44, BEADS, OT, 8);
    match fx.mem.u16(D) {
        0x7D => 2,
        0x70 => 1,
        _ => 0,
    }
}
