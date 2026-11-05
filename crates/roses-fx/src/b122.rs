//! BMODULE b122 (card 647), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06D90, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family (s2 = D, s1 = the parameter then the loop
//! count, s0 = the OT) with the recompiled node bursts / motes
//! (1e40f1216565 / 90a569cefb1c) and rods (b0fa3a2e1198, `rod::rods_c`),
//! no stage light. Node bursts every update (+0xA0: no cue lists, a
//! scatter on node 0x54 over 32..64, groups +0xE4 (16), sprites +0x464
//! (50)); on 2 a spike (+0xF60), a halo of 1500 (+0xF14) and a shake on the
//! attacker's node 0x3E; on 24 an arc (+0xC34) and a rod (+0x2C3C, 0x96
//! counts) between nodes 0x3E / 0x33; on 42 / 62 a bead-trail emitter
//! (+0x2BB0, `bead_trails_e` into `beads_at 40 44` +0x1030) at node 0x3E;
//! on 45 / 65 at the target two spikes, a halo of 600 and a shake.
//! Returns 1 on 35 / 55, 6 on 42 and 65..85, 2 on 86, 8 on 116 (and at
//! once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes_c`
//! 0x1E00440, `ring_spawn_b` 0x1E00F00, `node_bursts_b` 0x1E01250, `arcs`
//! (fb70e0bee134) 0x1E01730, `halo` 0x1E02B70, `spikes_b` 0x1E038C0,
//! `beads_at 40 44` 0x1E045F0, `bead_trails_e` 0x1E05500, `rods_c`
//! 0x1E05BE0 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BURSTS: u32 = D + 0xA0;
const GROUPS: u32 = D + 0xE4;
const ARC: u32 = D + 0xC34;
const HALO: u32 = D + 0xF14;
const SPIKES: u32 = D + 0xF60;
const BEADS: u32 = D + 0x1030;
const EMITTER: u32 = D + 0x2BB0;
const ROD: u32 = D + 0x2C3C;
const BYTES: u32 = D + 0x2C74;

/// The entry 0x1E06D90 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x50);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x50);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x2D80);
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
    for (o, node, clear) in [(0xCu32, 0x3E, 1), (0x10, 0x33, 0), (0x14, 0x54, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    let p = BURSTS;
    fx.mem.write_u32(p, GROUPS);
    fx.mem.write_u8(p + 0x30, 0x10);
    fx.mem.write_u32(p + 4, D + 0x464);
    fx.mem.write_u8(p + 0x31, 0x32);
    fx.mem.write_u8(p + 0x32, 0);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(p + 0x33, b);
    fx.mem.write_u16(p + 0x34, 0x20);
    fx.mem.write_u16(p + 0x36, 0x40);
    fx.mem.write_u32(p + 0x38, 0x4334_0000);
    fx.mem.write_u32(p + 0x3C, 0x4348_0000);
    fx.mem.write_u32(p + 0x40, 0x4100_0000);
    // The entry's s0 = 3 (the texture loop's), s1 = the parameter, s2 = D.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_6F00, [Some(3), Some(param as u32), Some(D)]);
    0
}

/// A spike's common fields.
fn spike_common(fx: &mut Fx, r: u32, ra: u32) {
    let x = fx.rand_from(OT, ra);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
}

/// Update 2 [F 0x1E06F88]: a spike, a halo and the shake on the attacker's
/// node +0xC.
fn start(fx: &mut Fx) {
    let r = SPIKES;
    set_counter4(fx, r, 0x16, 0, 0xFF);
    fx.mem.write_u8(r + 4, 5);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 6, b);
    for (o, v) in [(0xAu32, 1u8), (0xB, 2), (0xC, 3), (8, 0x64)] {
        fx.mem.write_u8(r + o, v);
    }
    spike_common(fx, r, 0x01E0_6FF0);
    fx.mem.write_u8(r + 0x23, 0x20);
    fx.mem.write_u8(r + 0x27, 0x80);
    for o in [0x58u32, 0x5C, 0x60] {
        fx.mem.write_u32(r + o, 0x180);
    }
    let h = HALO;
    set_counter4(fx, h, 0x12, 0, 0xFF);
    fx.mem.write_u8(h + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(h + 6, b);
    for (o, v) in [(4u32, 0x81u8), (7, 9), (0xA, 0x80), (9, 0x80), (8, 0x20), (0xF, 0xC0), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0)] {
        fx.mem.write_u8(h + o, v);
    }
    fx.mem.write_u32(h + 0x10, 0x44BB_8000);
    fx.mem.write_u32(h + 0x14, 0xC270_0000);
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(s + 6, b);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x4000_0000);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
}

/// Update 0x18 [F 0x1E07128]: the arc and the rod between nodes +0xC /
/// +0x10.
fn arc_rod(fx: &mut Fx) {
    let r = ARC;
    set_counter4(fx, r, 0x82, 0, 0xFF);
    fx.mem.write_u8(r + 4, 1);
    fx.mem.write_u32(r + 8, D + 0xC94);
    fx.mem.write_u8(r + 0x30, 0x28);
    fx.mem.write_u8(r + 0x31, 8);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 0x32, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 0x33, b);
    fx.mem.write_u32(r + 0x38, 0x3F80_0000);
    fx.mem.write_u8(r + 0x34, 0);
    for (o, v) in [(0x47u32, 0xC0u8), (0x44, 0xC0), (0x45, 0xC0), (0x46, 0xC0), (0x4A, 0x80), (0x49, 0x80), (0x48, 0x10)] {
        fx.mem.write_u8(r + o, v);
    }
    let mut j = 0;
    while j < fx.mem.u8(r + 0x31) + 1 {
        fx.mem.set16(r + 0x1E + 2 * j as u32, -1);
        fx.mem.set16(r + 0xC + 2 * j as u32, -1);
        j += 1;
    }
    let r = ROD;
    set_counter4(fx, r, 0x96, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(r + 8, b);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(r + 9, b);
    fx.mem.write_u8(r + 4, 2);
    fx.mem.write_u32(r + 0x10, 0x3E4C_CCCD);
    fx.mem.write_u32(r + 0x14, 0x3DCC_CCCD);
    fx.mem.write_u32(r + 0xC, 0x42A0_0000);
    for (o, v) in [(0x1Bu32, 0xC0u8), (0x18, 0xC0), (0x19, 0xC0), (0x1A, 0xC0), (0x1E, 0x80), (0x1D, 0x80), (0x1C, 0), (0x1F, 0)] {
        fx.mem.write_u8(r + o, v);
    }
}

/// Updates 0x2A / 0x3E [F 0x1E07278]: the bead-trail emitter at node +0xC.
fn emitter(fx: &mut Fx, param: i32) {
    let e = EMITTER;
    set_counter4(fx, e, 0xA, 0, 0xFF);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(e + 6, b);
    for (o, v) in [(4u32, 2u8), (5, 1), (0x14, 3), (0x16, 2)] {
        fx.mem.write_u8(e + o, v);
    }
    fx.mem.write_u32(e + 8, BEADS);
    fx.mem.write_u8(e + 0x10, 0x50);
    fx.mem.write_u8(e + 0x12, 0xA);
    let c = if fx.mem.u16(D) == 0x56 { 0x1C } else { 0x12 };
    fx.mem.write_u8(e + 0x11, c);
    fx.mem.write_u8(e + 0x13, 1);
    fx.mem.write_u32(e + 0x18, 0x3F19_999A);
    fx.mem.write_u32(e + 0x1C, 0x3F19_999A);
    fx.mem.write_u32(e + 0x20, 0x41F0_0000);
    fx.mem.write_u32(e + 0xC, BYTES);
    for o in [0x2Bu32, 0x28, 0x29, 0x2A] {
        fx.mem.write_u8(e + o, 0x80);
    }
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0xC));
    node_position_saving(fx, h, slot, e + 0x3C, u64::from(OT), Some(param as i64 as u64), 0x01E0_7350);
    copy_v4(fx, e + 0x3C, e + 0x4C);
}

/// Updates 0x2D / 0x41 [F 0x1E07378]: two spikes, a halo and the shake at
/// the target (sp + 0x40).
fn strike(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = SPIKES + 0x68 * k;
        fx.mem.write_u8(r + 4, 5);
        set_counter4(fx, r, 0x16, 0, 0xFF);
        if k & 1 != 0 {
            let f = fx.mem.read_u8(r + 4);
            fx.mem.write_u8(r + 4, f | 2);
            fx.mem.write_u32(r + 0x14, 0x3FB3_3333);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x80);
        } else {
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4270_0000);
            fx.mem.write_u8(r + 0x27, 0xC0);
            fx.mem.write_u8(r + 0x23, 0);
        }
        for (o, v) in [(5u32, 0u8), (0xA, 2), (0xB, 2), (0xC, 3), (8, 0x3C)] {
            fx.mem.write_u8(r + o, v);
        }
        spike_common(fx, r, 0x01E0_75DC);
        copy_v4(fx, sp + 0x40, r + 0x28);
    }
    let h = HALO;
    set_counter4(fx, h, 0x18, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(h + o, v);
    }
    fx.mem.write_u32(h + 0x14, 0);
    fx.mem.write_u32(h + 0x10, 0x4416_0000);
    copy_v4(fx, sp + 0x40, h + 0x1C);
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
    copy_v4(fx, sp + 0x40, s + 0x20);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the spikes loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x40, u64::from(OT), 0x01E0_6F28);
    match fx.mem.u16(D) {
        0x74 => return 8,
        0x41 | 0x2D => {
            strike(fx, sp);
            s1 = 2;
        }
        0x3E | 0x2A => emitter(fx, param),
        0x18 => arc_rod(fx),
        2 => start(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_7854, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts_b(fx, BURSTS, n);
    blib::mote::motes_c(fx, GROUPS, OT, 0x10);
    blib::arc::arcs(fx, 0x01E0_1730, ARC, OT, 1);
    blib::halo::halo(fx, 0x01E0_2B70, HALO, OT, 1);
    // The entry never sets f22: the runner's 0 [H].
    blib::rod::rods_c(fx, 0x01E0_5BE0, ROD, OT, 1, 0.0);
    blib::spike::spikes_b(fx, 0x01E0_38C0, SPIKES, OT, 2);
    // spikes_b's prologue saves (its 0x220 frame: f20..f23 +0..+0xC, s0..s6
    // +0x10..+0x70, ra +0x80) are the bead-trail path's w (run b122); the
    // entry's s3..s6 / f20..f23 the runner's 0 [H].
    let base = fx.sp - 0x220;
    fx.saved(base, &[(0x10, u64::from(OT)), (0x20, s1), (0x30, u64::from(D)), (0x40, 0), (0x50, 0), (0x60, 0), (0x70, 0), (0x80, 0x01E0_78C8)]);
    for o in [0u32, 4, 8, 0xC] {
        fx.mem.write_u32(base + o, 0);
    }
    blib::emit::bead_trails_e(fx, 0x01E0_5500, EMITTER, 1);
    blib::beam::beads_at(fx, 0x01E0_45F0, 0x40, 0x44, BEADS, OT, 0x50);
    let n = fx.mem.u16(D);
    if (0x41..0x56).contains(&n) {
        6
    } else if n == 0x56 {
        2
    } else if n == 0x2A {
        6
    } else if n == 0x37 || n == 0x23 {
        1
    } else {
        0
    }
}
