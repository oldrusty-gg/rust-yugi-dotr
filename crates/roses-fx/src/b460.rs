//! BMODULE b460 (card 547), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042D0, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b469's program for six nodes (0x22, 0x38, 0x1E, 0x1C, 0x35,
//! 0x32 at D+0xC..+0x20). On update 2 a long camera shake at the target
//! (+0x40), two long flashes on nodes +0x20 / +0x18 (+0x1904) and four
//! bolts between node pairs (+0x1DC4 slots 0..3); on 108 those bolts thin;
//! on 126 two flashes on nodes +0x18 / +0x20; on 128 the shake again,
//! eight bead beams (+0x3CB4, nodes +0x18 / +0x20) and four jagged bolts
//! from nodes +0xC / +0x10 to the target; on 129 the flashes widen; on 172
//! a wide flash at the target and two segment spawners (+0x2CC4); on even
//! updates 2..219 two ribbons a update from nodes +0x20 / +0x18 (+0xA4); on
//! updates 172..368 b469's flashes and ribbons about the target; the stage
//! light pulsed by 0x1031C0 (+0xA0: down on 2, up on 382). The flicker
//! pool (+0x2EA4) is drawn but never filled. Returns 1 on update 162, 6 on
//! 172..368, 2 on 369, 8 on 388 (and at once for -2 / -3).
//!
//! Library (record addresses): as b469 [`crate::b469`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b469::{bolt_bytes, flash_bytes, jag_at, ribbon_from, sparkle, strike};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x4394;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;

/// The entry 0x1E042D0 (a0 = D, a1 = param; s0 = D, s1 = the OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx)
    } else {
        step(fx, sp)
    };
    fx.leave(0x1A0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x44A0);
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
    for (at, node, clear) in [(0xCu32, 0x22, 1), (0x10, 0x38, 0), (0x14, 0x1E, 0), (0x18, 0x1C, 0), (0x1C, 0x35, 0), (0x20, 0x32, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0x17E);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_444C, [Some(D), Some(OT), Some(2)]);
    0
}

/// The shake at the target (sp + 0x60) for 255 counts [F 0x1E044D4 /
/// 0x1E0487C], its +0x50 = `w`.
fn shake(fx: &mut Fx, sp: u32, w: u32) {
    let s = D + 0x40;
    set_counter4(fx, s, 0xFF, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FCC_CCCD);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, w);
    fx.mem.write_u32(D + 0x68, 0);
}

/// Update 2 [F 0x1E044D4]: the shake, two long flashes on nodes +0x20 /
/// +0x18 and four bolts in slots 0..3 between node pairs.
fn charge(fx: &mut Fx, sp: u32) {
    shake(fx, sp, 0x4080_0000);
    for k in 0..2u32 {
        let r = FLASHES + 0x4C * k;
        set_counter4(fx, r, 0xFF, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(if k == 0 { D + 0x20 } else { D + 0x18 });
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 5, 0x40, 0x20, 0x80, 0xC0);
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
    for j in 0..2u32 {
        for s in 0..2u32 {
            let b = BOLTS + ((s + 2 * j) << 6);
            set_counter4(fx, b, 0xFF, j as i32, 0xFF);
            fx.mem.write_u8(b + 5, 1);
            fx.mem.write_u8(b + 4, 1);
            let (p, q) = match (j, s) {
                (0, 0) => (0x1Cu32, 0x20u32),
                (0, _) => (0x10, 0x1C),
                (_, 0) => (0x14, 0x18),
                _ => (0xC, 0x14),
            };
            let v = fx.mem.read_u8(D + p);
            fx.mem.write_u8(b + 9, v);
            let v = fx.mem.read_u8(D + q);
            fx.mem.write_u8(b + 0xA, v);
            fx.mem.write_u8(b + 8, 6);
            fx.mem.write_u32(b + 0x10, 0x3FC0_0000);
            fx.mem.write_u32(b + 0x14, 0x3ECC_CCCD);
            for (o, v) in [(0xBu32, 1u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x30)] {
                fx.mem.write_u8(b + o, v);
            }
        }
    }
}

/// Update 126 [F 0x1E04794]: a flash on each of nodes +0x18 / +0x20.
fn charge2(fx: &mut Fx) {
    for k in 0..2u32 {
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i < 0 {
            continue;
        }
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x18 + 8 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 0x80, 0x40, 0x20, 0x80, 0xC0);
        fx.mem.write_u32(r + 0x10, 0x4248_0000);
        fx.mem.write_u32(r + 0x14, 0x4270_0000);
        fx.mem.write_u32(r + 0x18, 0x41F0_0000);
    }
}

/// Update 128 [F 0x1E0487C]: the shake again, eight bead beams on nodes
/// +0x18 (first four) / +0x20, nodes +0xC / +0x10 into sp + 0x80 / + 0x90
/// and four jagged bolts from them (two each) to the target.
fn bolts_start(fx: &mut Fx, sp: u32) {
    shake(fx, sp, 0x4100_0000);
    for k in 0..8i32 {
        let r = D + 0x3CB4 + 0x58 * k as u32;
        set_counter4(fx, r, 0x1C, k & 0xFF, 0xFF);
        let c = fx.mem.read_u8(r).wrapping_sub(4);
        fx.mem.write_u8(r, c);
        fx.mem.write_u8(r + 4, 5);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x19);
        fx.mem.set8(r + 9, k);
        fx.mem.write_u8(r + 0xA, 0);
        fx.mem.write_u8(r + 0xC, 2);
        let x = fx.rand_from(D, 0x01E0_4970) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x420C_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let (slot, ra) = if k < 4 { (D + 0x18, 0x01E0_49D8) } else { (D + 0x20, 0x01E0_49F4) };
        let n = fx.mem.i32(slot);
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), ra);
    }
    for (slot, out, ra) in [(0xCu32, 0x80u32, 0x01E0_4A20u32), (0x10, 0x90, 0x01E0_4A34)] {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + slot);
        node_position_saving(fx, h, n, sp + out, u64::from(D), Some(u64::from(OT)), ra);
    }
    for j in 0..4u32 {
        jag_at(fx, sp, sp + 0x80 + 0x10 * (j >> 1), 0x3F00_0000);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 0x8C, ((j + 9 * s) & 0xFF) as i32, 0xFF);
            bolt_bytes(fx, sp, b, s, 9, [0x3F33_3333, 0x3E4C_CCCD], 3);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_4474);
    match fx.mem.u16(D) {
        0x184 => return 8,
        0xAC => strike(fx, sp, D + 0x199C, [0x84, 0xC8]),
        0x81 => {
            fx.mem.write_u32(D + 0x1960, 0x43C8_0000);
            fx.mem.write_u32(D + 0x1914, 0x43C8_0000);
        }
        0x80 => bolts_start(fx, sp),
        0x7E => charge2(fx),
        0x6C => {
            for k in 0..4u32 {
                let b = BOLTS + (k << 6);
                fx.mem.write_u8(b + 8, 8);
                fx.mem.write_u32(b + 0x10, 0x3F80_0000);
                fx.mem.write_u32(b + 0x14, 0x3E99_999A);
            }
        }
        2 => charge(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xDC).contains(&n) && n & 1 == 0 {
        let look = [(0x3F19_999A, 0x4170_0000, 8), (0x4000_0000, 0x41F0_0000, 0x12)];
        for k in 0..2u32 {
            let node = if k == 0 { D + 0x20 } else { D + 0x18 };
            ribbon_from(fx, node, 0x80, look, [0x01E0_4E80, 0x01E0_4ED0, 0x01E0_4EE8, 0x01E0_4F00], BYTES);
        }
    }
    let n = fx.mem.u16(D);
    if (0xAC..0x171).contains(&n) {
        sparkle(fx, sp, [0x01E0_4F58, 0x01E0_4FA4, 0x01E0_5158]);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_51DC, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_51EC, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, D + 0x2CC4, 8, 0x01E0_5228);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::flicker::flickers(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3500, D + 0x3CB4, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0xAC..0x171).contains(&n) {
        6
    } else if n == 0x171 {
        2
    } else if n == 0xA2 {
        1
    } else {
        0
    }
}
