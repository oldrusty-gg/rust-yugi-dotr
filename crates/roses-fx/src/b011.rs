//! BMODULE b011 (card 153), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05180, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family (s2 = D, s1 = the parameter then the loop
//! count, s0 = the OT) with the recompiled node bursts / motes
//! (1e40f1216565 / 90a569cefb1c) and frame rays (b1e33d639b16,
//! `window::frame_rays_c`), no stage light. Node bursts every update
//! (+0xA0: cue lists 0x1E05BC8 / 0x1E05BD8 on the attacker's nodes 0x10 /
//! 9, a scatter on node 0x10 over 202..210, groups +0xE4 (32), sprites
//! +0x7E4 (50)); on 40 (0x50 counts) and 210 (0x18) an arc (+0xFB4, nodes
//! 0x2D -> 0x2C, tracks of 8); on 92 the frame rays (+0x1298) on the
//! attacker's point (0x1F65D0 kind 1); on 213 at the target two spikes
//! (+0x1314, records 0 / 1), a halo of 800 (+0x12C8) and a shake. Returns
//! 1 on 203, 2 on 216, 8 on 330 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes_c`
//! 0x1E00440, `ring_spawn_b` 0x1E00F00, `node_bursts_b` 0x1E01250, `arcs`
//! (fb70e0bee134) 0x1E01730, `frame_rays_c` 0x1E02B70, `halo` 0x1E03700,
//! `spikes_b` 0x1E04450 [`crate::blib`].
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::set_counter4};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BURSTS: u32 = D + 0xA0;
const GROUPS: u32 = D + 0xE4;
const ARC: u32 = D + 0xFB4;
const RAYS: u32 = D + 0x1298;
const HALO: u32 = D + 0x12C8;
const SPIKES: u32 = D + 0x1314;
const BYTES: u32 = D + 0x13E4;

/// The entry 0x1E05180 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x50);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x50);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x14F0);
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
    for (o, node, clear) in [(0xCu32, 9, 1), (0x10, 0x10, 0), (0x14, 0x2D, 0), (0x18, 0x2C, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    let p = BURSTS;
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(p + 0x28, b);
    let b = fx.mem.read_u8(D + 0xC);
    fx.mem.write_u8(p + 0x29, b);
    for (o, v) in [(8u32, 0x01E0_5BC8u32), (0xC, 0x01E0_5BD8), (0, GROUPS), (4, D + 0x7E4)] {
        fx.mem.write_u32(p + o, v);
    }
    fx.mem.write_u8(p + 0x30, 0x20);
    fx.mem.write_u8(p + 0x31, 0x32);
    fx.mem.write_u8(p + 0x32, 0);
    let b = fx.mem.read_u8(D + 0x10);
    fx.mem.write_u8(p + 0x33, b);
    fx.mem.write_u16(p + 0x34, 0xCA);
    fx.mem.write_u16(p + 0x36, 0xD2);
    fx.mem.write_u32(p + 0x38, 0x4316_0000);
    fx.mem.write_u32(p + 0x3C, 0x42C8_0000);
    fx.mem.write_u32(p + 0x40, 0x40C0_0000);
    // The entry's s0 = 2 (the texture loop's), s1 = the parameter, s2 = D.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5330, [Some(2), Some(param as u32), Some(D)]);
    0
}

/// Updates 0x28 / 0xD2 [F 0x1E053A0]: the arc from node +0x14 to +0x18.
fn arc(fx: &mut Fx, count: i32) {
    let r = ARC;
    set_counter4(fx, r, count, 0, 0xFF);
    fx.mem.write_u8(r + 4, 1);
    fx.mem.write_u32(r + 8, D + 0x1014);
    fx.mem.write_u8(r + 0x30, 0x28);
    fx.mem.write_u8(r + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(r + 0x32, b);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(r + 0x33, b);
    fx.mem.write_u32(r + 0x38, 0x3F66_6666);
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

/// Update 0xD5 [F 0x1E05520]: two spikes, a halo and the shake at the
/// target (sp + 0x40).
fn strike(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = SPIKES + 0x68 * k;
        fx.mem.write_u8(r + 4, 5);
        if k & 1 != 0 {
            set_counter4(fx, r, 0x18, 0, 0xFF);
            let f = fx.mem.read_u8(r + 4);
            fx.mem.write_u8(r + 4, f | 2);
            fx.mem.write_u32(r + 0x14, 0x4000_0000);
            fx.mem.write_u32(r + 0x1C, 0x4220_0000);
            fx.mem.write_u8(r + 0x23, 0x80);
            fx.mem.write_u8(r + 0x27, 0x80);
        } else {
            set_counter4(fx, r, 0x16, 0, 0xFF);
            fx.mem.write_u32(r + 0x14, 0x3FE6_6666);
            fx.mem.write_u32(r + 0x1C, 0x4270_0000);
            fx.mem.write_u8(r + 0x27, 0xC0);
            fx.mem.write_u8(r + 0x23, 0);
        }
        for (o, v) in [(5u32, 0u8), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0x3C)] {
            fx.mem.write_u8(r + o, v);
        }
        let x = fx.rand_from(OT, 0x01E0_5784);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x18, 0x3F80_0000);
        for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x40, r + 0x28);
    }
    let h = HALO;
    set_counter4(fx, h, 0x1C, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 1), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(h + o, v);
    }
    fx.mem.write_u32(h + 0x14, 0);
    fx.mem.write_u32(h + 0x10, 0x4448_0000);
    copy_v4(fx, sp + 0x40, h + 0x1C);
    let s = D + 0x40;
    set_counter4(fx, s, 0x18, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x40, s + 0x20);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x40, u64::from(OT), 0x01E0_5358);
    match fx.mem.u16(D) {
        0x14A => return 8,
        0xD5 => strike(fx, sp),
        0x5C => {
            let r = RAYS;
            set_counter4(fx, r, 0x18, 0, 0xFF);
            let h = fx.mem.i32(D + 4);
            fx.unit_point(1, h, r + 0xC);
            for (o, v) in [(0x1Cu32, 0x80u8), (0x1D, 0x80), (0x1E, 0x80), (0x1F, 0x20), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0x70), (8, 6), (9, 1), (0xA, 0xFF)] {
                fx.mem.write_u8(r + o, v);
            }
        }
        0xD2 => arc(fx, 0x18),
        0x28 => arc(fx, 0x50),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_59FC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts_b(fx, BURSTS, n);
    blib::mote::motes_c(fx, GROUPS, OT, 0x20);
    let (_, a3) = blib::arc::arcs_a3(fx, 0x01E0_1730, ARC, OT, 1);
    blib::window::frame_rays_c(fx, 0x01E0_2B70, RAYS, OT, 1, a3);
    blib::halo::halo(fx, 0x01E0_3700, HALO, OT, 1);
    blib::spike::spikes_b(fx, 0x01E0_4450, SPIKES, OT, 2);
    match fx.mem.u16(D) {
        0xD8 => 2,
        0xCB => 1,
        _ => 0,
    }
}
