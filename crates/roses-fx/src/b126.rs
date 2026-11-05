//! BMODULE b126 (card 165), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05100, data area a0 = 0x1ED0000) [F] (helper A19,
//! 1 Nov 2026): b468's family (s3 = D, s1 = the parameter then the loop
//! counts, s0 = the OT) with the recompiled node bursts / motes
//! (1e40f1216565 / 90a569cefb1c). Node bursts every update (+0xA4: cue
//! lists 0x1E05A40 / 0x1E05A50 / 0x1E05A60 on the attacker's nodes 0x44 /
//! 0x4E / 0x30, groups +0xE8 (20), sprites +0x548 (50)); on 66..98 every
//! fourth update an object op 0x1F83B0 (mode 0x18, colour 0x80 / 0x70); on
//! 98 at the target the fade (+0xE1C), a spike (+0xD18, 2 / 40), a halo of
//! 500 (+0xD80), a bar 3000 long (+0xDCC), eight bead beams (+0xE24,
//! `beads_at 40 44`, delays 2 + k) and a shake. The stage light by
//! 0x1031C0 (+0xA0: down on 2, up on 0xB8). Returns 1 on 88, 2 on 95, 8 on
//! 190 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `motes_c`
//! 0x1E00440, `ring_spawn_b` 0x1E00F00, `node_bursts_b` 0x1E01250,
//! `spikes_b` 0x1E01730, `halo` 0x1E02460, `bars_c` 0x1E031B0, `fade_c`
//! 0x1E03C80, `beads_at 40 44` 0x1E041F0 [`crate::blib`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BURSTS: u32 = D + 0xA4;
const GROUPS: u32 = D + 0xE8;
const SPIKE: u32 = D + 0xD18;
const HALO: u32 = D + 0xD80;
const BAR: u32 = D + 0xDCC;
const FADE: u32 = D + 0xE1C;
const BEADS: u32 = D + 0xE24;
const BYTES: u32 = D + 0x10E4;

/// The entry 0x1E05100 (a0 = D, a1 = param).
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
    fx.bzero(D, 0x11F0);
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
    for (o, node, clear) in [(0xCu32, 0x30, 1), (0x10, 0x44, 0), (0x14, 0x4E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + o, r);
    }
    let p = BURSTS;
    for (o, from) in [(0x28u32, 0x10u32), (0x29, 0x14), (0x2A, 0xC)] {
        let b = fx.mem.read_u8(D + from);
        fx.mem.write_u8(p + o, b);
    }
    for (o, v) in [(8u32, 0x01E0_5A40u32), (0xC, 0x01E0_5A50), (0x10, 0x01E0_5A60), (0, GROUPS), (4, D + 0x548)] {
        fx.mem.write_u32(p + o, v);
    }
    fx.mem.write_u8(p + 0x30, 0x14);
    fx.mem.write_u8(p + 0x31, 0x32);
    fx.mem.write_u8(p + 0x32, 0);
    fx.mem.write_u16(p + 0x34, 0);
    fx.mem.write_u32(p + 0x38, 0x430C_0000);
    fx.mem.write_u32(p + 0x40, 0x40A0_0000);
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xB8);
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_52A8, [Some(OT), Some(2), Some(0)]);
    0
}

/// Update 0x62 [F 0x1E052F4]: the fade, a spike, a halo, a bar, eight
/// bead beams and the shake at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    set_counter4(fx, FADE, 2, 0, 0xFF);
    fx.mem.write_u8(FADE + 4, 4);
    let r = SPIKE;
    fx.mem.write_u8(r + 4, 5);
    set_counter4(fx, r, 0x18, 0, 0xFF);
    fx.mem.write_u32(r + 0x14, 0x4000_0000);
    fx.mem.write_u32(r + 0x1C, 0x4220_0000);
    for (o, v) in [(0x27u32, 0xC0u8), (0x23, 0), (5, 0), (0xA, 1), (0xB, 2), (0xC, 4), (8, 0x3C)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_537C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    let h = HALO;
    set_counter4(fx, h, 0x1A, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 1), (7, 9), (0xF, 0xF0), (0xC, 0xF0), (0xD, 0xF0), (0xE, 0xF0), (0xA, 0x60), (9, 0x10), (8, 0xC0)] {
        fx.mem.write_u8(h + o, v);
    }
    fx.mem.write_u32(h + 0x14, 0);
    fx.mem.write_u32(h + 0x10, 0x43FA_0000);
    copy_v4(fx, sp + 0x50, h + 0x1C);
    let b = BAR;
    fx.mem.write_u8(b + 5, 0);
    copy_v4(fx, sp + 0x50, b + 0x20);
    set_counter4(fx, b, 0x18, 1, 0xFF);
    fx.mem.write_u8(b + 4, 0x81);
    fx.mem.write_u32(b + 0x10, 0x453B_8000);
    fx.mem.write_u32(b + 0x14, 0x42A0_0000);
    for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (9, 0), (0xA, 0), (8, 0x40), (0xF, 0x80)] {
        fx.mem.write_u8(b + o, v);
    }
    fx.mem.write_u32(b + 0x1C, 0);
    fx.mem.write_u32(b + 0x18, 0);
    for s1 in 0..8i32 {
        let i = first_free(fx, BEADS, 0x58, 8);
        if i < 0 {
            continue;
        }
        let r = BEADS + 0x58 * i as u32;
        set_counter4(fx, r, 0x16, (s1 + 2) & 0xFF, 0xFF);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x14);
        let x = fx.rand_from(OT, 0x01E0_55AC);
        fx.mem.set8(r + 9, x >> 4);
        for (o, v) in [(0xAu32, 1u8), (0xB, 3), (0xC, 2)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x41F0_0000);
        fx.mem.write_u32(r + 0x20, 0);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3FCC_CCCD);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        copy_v4(fx, sp + 0x50, r + 0x28);
    }
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
    copy_v4(fx, sp + 0x50, s + 0x20);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    // The entry's s1: the parameter, then the beads loop's count.
    let mut s1 = param as i64 as u64;
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_52D0);
    match fx.mem.u16(D) {
        0xBE => return 8,
        0x62 => {
            strike(fx, sp);
            s1 = 8;
        }
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x42..0x63).contains(&n) && n & 3 == 0 {
        for o in [0x6Cu32, 0x6D, 0x6E, 0x68, 0x69, 0x6A] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        fx.mem.write_u8(sp + 0x6B, 0x70);
        fx.mem.write_u8(sp + 0x6F, 0);
        let h = fx.mem.read_u32(D + 4);
        fx.boundary(0x001F_83B0, "object op 0x1F83B0", [h, 0x18, 0, sp + 0x68]);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_5884, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_5894, [Some(u64::from(OT)), Some(s1)]);
    let n = fx.mem.u16(D);
    blib::burst::node_bursts_b(fx, BURSTS, n);
    blib::mote::motes_c(fx, GROUPS, OT, 0x14);
    blib::spike::spikes_b(fx, 0x01E0_1730, SPIKE, OT, 1);
    blib::halo::halo(fx, 0x01E0_2460, HALO, OT, 1);
    blib::cross::bars_c(fx, 0x01E0_31B0, BAR, OT, 1);
    blib::fade::fade_c(fx, FADE, OT);
    blib::beam::beads_at(fx, 0x01E0_41F0, 0x40, 0x44, BEADS, OT, 8);
    match fx.mem.u16(D) {
        0x5F => 2,
        0x58 => 1,
        _ => 0,
    }
}
