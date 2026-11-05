//! BMODULE b410 (card 530), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E05220, data area a0 = 0x1ED0000) [F] (helper B5, 31
//! Oct 2026): b497's program for six nodes (0xB, 0x16, 4, 0xE, 0x14, 5 at
//! D+0xC..+0x20). On update 2 two long flashes on nodes +0xC / +0x10
//! (+0x1904), five bolts between node pairs (+0x1DC4) and a camera shake
//! at the target (+0x40); on 40 four crossing bars (+0x439C) and a flash
//! (+0x199C) on node +0x14; on 82 the fade (+0x4394), the shake again,
//! eight bead beams on nodes +0x10 / +0xC (+0x3CB4), two flashes and four
//! jagged bolts from nodes +0xC / +0x10 to the target; on 76 the flashes
//! widen; on 83 two segment spawners (+0x2CC4) at the target; on even
//! updates 2..209 two ribbons a update from nodes +0xC / +0x10 (+0xA4); on
//! updates 83..219 two flashes and ribbons a update about the target (x, y
//! +- 14, z +- 6); the stage light pulsed by 0x1031C0 (+0xA0). The flicker
//! pool (+0x2EA4) is drawn but never filled. Returns 1 on update 73, 6 on
//! 83..219, 2 on 220, 8 on 250 (and at once for -2 / -3).
//!
//! Library (record addresses): as b497 [`crate::b497`].
use crate::b035::light_ramp;
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::b469::{flash_bytes, jag_at, ribbon_from, spark_ribbon};
use crate::b497::{bolt_still, jitter, shake, spawner_bytes};
use crate::b586::segment_spawns_saving;
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x44DC;
const FLASHES: u32 = D + 0x1904;
const BOLTS: u32 = D + 0x1DC4;
const SPAWNERS: u32 = D + 0x2CC4;
const BARS: u32 = D + 0x439C;

/// The entry 0x1E05220 (a0 = D, a1 = param; s0 = D, s1 = the OT).
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
    fx.bzero(D, 0x45E0);
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
    for (at, node, clear) in [(0xCu32, 0xB, 1), (0x10, 0x16, 0), (0x14, 4, 0), (0x18, 0xE, 0), (0x1C, 0x14, 0), (0x20, 5, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    fx.mem.set16(D + 0xA0, 2);
    fx.mem.set16(D + 0xA2, 0xF4);
    rand_bytes(fx, BYTES, 0x100, 0x01E0_539C, [Some(D), Some(OT), Some(2)]);
    0
}

/// Update 2 [F 0x1E05418]: two long flashes on nodes +0xC / +0x10, five
/// bolts between node pairs (stopping at a full pool) and the shake.
fn charge(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = FLASHES + 0x4C * k;
        set_counter4(fx, r, 0xA0, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0xC + 4 * k);
        fx.mem.write_u8(r + 6, b);
        flash_bytes(fx, r, 0x85, 0x40, 0x20, 0x80, 0xC0);
        fx.mem.write_u32(r + 0x10, 0x4316_0000);
        fx.mem.write_u32(r + 0x14, 0);
    }
    for (p, q) in [(0xCu32, 0x10u32), (0xC, 0x18), (0x10, 0x1C), (0xC, 0x20), (0x10, 0x20)] {
        let i = first_free(fx, BOLTS, 0x40, 0x3C);
        if i < 0 {
            break;
        }
        let b = BOLTS + ((i as u32) << 6);
        set_counter4(fx, b, 0x56, 0, 0xFF);
        fx.mem.write_u8(b + 5, 1);
        fx.mem.write_u8(b + 4, 1);
        let v = fx.mem.read_u8(D + p);
        fx.mem.write_u8(b + 9, v);
        let v = fx.mem.read_u8(D + q);
        fx.mem.write_u8(b + 0xA, v);
        fx.mem.write_u8(b + 8, 5);
        fx.mem.write_u32(b + 0x10, 0x3F80_0000);
        fx.mem.write_u32(b + 0x14, 0x3E99_999A);
        for (o, v) in [(0xBu32, 1u8), (0xD, 4), (0xC, 0), (0x18, 0x80), (0x19, 0x80), (0x1A, 0x80), (0x1B, 0x30)] {
            fx.mem.write_u8(b + o, v);
        }
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x8C, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 8);
    for o in [0x14u32, 0x18, 0x1C] {
        fx.mem.write_u32(s + o, 0x3FE6_6666);
    }
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x4320_0000);
    copy_v4(fx, sp + 0x60, D + 0x60);
    fx.mem.write_u32(s + 0x50, 0x4080_0000);
    fx.mem.write_u32(D + 0x68, 0);
}

/// Update 40 [F 0x1E056B4]: four bars on node +0x14 (the first 1400 long,
/// the others 700 at 45 k degrees) and a flash there.
fn bars_start(fx: &mut Fx) {
    for k in 0..4u32 {
        let r = BARS + 0x50 * k;
        set_counter4(fx, r, 0x1C, 0, 0xFF);
        fx.mem.write_u8(r + 5, 1);
        let b = fx.mem.read_u8(D + 0x14);
        fx.mem.write_u8(r + 6, b);
        fx.mem.write_u8(r + 4, 0x80);
        if k == 0 {
            fx.mem.write_u32(r + 0x10, 0x44AF_0000);
            fx.mem.write_u32(r + 0x18, 0);
            fx.mem.write_u32(r + 0x14, 0x41F0_0000);
        } else {
            fx.mem.write_u32(r + 0x10, 0x442F_0000);
            let a = ee::mul(45.0, ee::from_i32(k as i32));
            fx.mem.setf(r + 0x18, a);
            fx.mem.write_u32(r + 0x14, 0x4170_0000);
        }
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x60), (8, 0x10)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x40C0_0000);
    }
    let r = D + 0x199C;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    fx.mem.write_u8(r + 5, 1);
    let b = fx.mem.read_u8(D + 0x14);
    fx.mem.write_u8(r + 6, b);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x4396_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x60), (9, 0x80), (8, 0x10), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
}

/// Update 82 [F 0x1E05854]: the fade, the shake, eight bead beams on
/// nodes +0x10 (even) / +0xC (odd), nodes +0xC / +0x10 into sp + 0x80 /
/// + 0x90, a flash at the target and one at node +0xC, four jagged bolts
/// from the nodes (two each) to the target.
fn bolts_start(fx: &mut Fx, sp: u32) {
    set_counter4(fx, D + 0x4394, 0x10, 0, 0xFF);
    fx.mem.write_u8(D + 0x4398, 1);
    shake(fx, sp, 8, 0x4100_0000);
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
        let x = fx.rand_from(D, 0x01E0_5968) >> 4;
        fx.mem.set8(r + 0xB, x & 3);
        fx.mem.write_u32(r + 0x1C, 0x420C_0000);
        fx.mem.write_u32(r + 0x20, 0xBF19_999A);
        fx.mem.write_u32(r + 0x10, BYTES);
        fx.mem.write_u32(r + 0x14, 0x3F80_0000);
        for o in [0x27u32, 0x24, 0x25, 0x26] {
            fx.mem.write_u8(r + o, 0x80);
        }
        let h = fx.mem.i32(D + 4);
        let (slot, ra) = if k & 1 != 0 { (D + 0xC, 0x01E0_59D0) } else { (D + 0x10, 0x01E0_59EC) };
        let n = fx.mem.i32(slot);
        node_position_saving(fx, h, n, r + 0x28, u64::from(D), Some(u64::from(OT)), ra);
    }
    for (slot, out, ra) in [(0xCu32, 0x80u32, 0x01E0_5A18u32), (0x10, 0x90, 0x01E0_5A2C)] {
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + slot);
        node_position_saving(fx, h, n, sp + out, u64::from(D), Some(u64::from(OT)), ra);
    }
    for k in 0..2u32 {
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i < 0 {
            continue;
        }
        let r = FLASHES + 0x4C * i as u32;
        set_counter4(fx, r, 0x18, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        flash_bytes(fx, r, 0x80, 0x40, 0x20, 0x80, 0xC0);
        fx.mem.write_u32(r + 0x10, 0x42C8_0000);
        fx.mem.write_u32(r + 0x14, 0x4316_0000);
        fx.mem.write_u32(r + 0x18, 0x4270_0000);
        copy_v4(fx, if k == 0 { sp + 0x60 } else { sp + 0x80 }, r + 0x1C);
    }
    for j in 0..4u32 {
        jag_at(fx, sp, sp + 0x80 + 0x10 * (j >> 1), 0x3F00_0000);
        for s in 0..6u32 {
            let i = first_free(fx, BOLTS, 0x40, 0x3C);
            if i < 0 {
                break;
            }
            let b = BOLTS + ((i as u32) << 6);
            set_counter4(fx, b, 0x8C, (j & 1) as i32, 0xFF);
            bolt_still(fx, sp, b, s);
        }
    }
}

/// Update 83 [F 0x1E05CE8]: two segment spawners (slots 0 / 1) at the
/// target.
fn strike(fx: &mut Fx, sp: u32) {
    for k in 0..2u32 {
        let r = SPAWNERS + 0x3C * k;
        set_counter4(fx, r, 0x80, 0, 3);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0);
        spawner_bytes(fx, r, 0xC, 0x1770);
        for c in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x60 + 4 * c);
            fx.mem.write_u32(r + 0x24 + 4 * c, w);
        }
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(r + o, 0);
        }
    }
}

/// Updates 83..219 [F 0x1E05FB8]: twice a update, a point about the target
/// (x, y +- 14, z +- 6, sp + 0x70; its w the stale sp + 0x7C) gets a flash
/// and a ribbon.
fn sparkle(fx: &mut Fx, sp: u32) {
    for _ in 0..2 {
        for (c, ra, m, h) in [(0u32, 0x01E0_5FC0u32, 0xAF0, 0x578), (4, 0x01E0_600C, 0xAF0, 0x578), (8, 0x01E0_6058, 0x4B0, 0x258)] {
            let j = jitter(fx, ra, m, h);
            let v = ee::add(fx.mem.f(sp + 0x60 + c), j);
            fx.mem.setf(sp + 0x70 + c, v);
        }
        let i = first_free(fx, FLASHES, 0x4C, 0x10);
        if i >= 0 {
            let r = FLASHES + 0x4C * i as u32;
            set_counter4(fx, r, 0xC, 0, 0xFF);
            fx.mem.write_u8(r + 5, 0);
            flash_bytes(fx, r, 1, 0x60, 0x10, 0x40, 0xC0);
            fx.mem.write_u32(r + 0x14, 0);
            fx.mem.write_u32(r + 0x10, 0x4396_0000);
            copy_v4(fx, sp + 0x70, r + 0x1C);
        }
        spark_ribbon(fx, sp, 0x01E0_6200, BYTES);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x60, u64::from(D), 0x01E0_53C4);
    match fx.mem.u16(D) {
        0xFA => return 8,
        0x53 => strike(fx, sp),
        0x4C => {
            fx.mem.write_u32(D + 0x1960, 0x4348_0000);
            fx.mem.write_u32(D + 0x1914, 0x4348_0000);
        }
        0x52 => bolts_start(fx, sp),
        0x28 => bars_start(fx),
        2 => charge(fx, sp),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (2..0xD2).contains(&n) && n & 1 == 0 {
        let look = [(0x3F80_0000, 0x41A0_0000, 0xC), (0x4040_0000, 0x4220_0000, 0x14)];
        for k in 0..2u32 {
            ribbon_from(fx, D + 0xC + 4 * k, 0x52, look, [0x01E0_5EE8, 0x01E0_5F38, 0x01E0_5F50, 0x01E0_5F68], BYTES);
        }
    }
    let n = fx.mem.u16(D);
    if (0x53..0xDC).contains(&n) {
        sparkle(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_6284, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    let n = fx.mem.u16(D);
    light_ramp(fx, D + 0xA0, n, 0x01E0_6294, [Some(u64::from(D)), Some(u64::from(OT))]);
    blib::fade::fade(fx, D + 0x4394, OT);
    blib::star::flash(fx, 0x01E0_0410, FLASHES, OT, 0x10);
    blib::streak::ribbon(fx, 0x01E0_0FC0, D + 0xA4, OT, 0x3C);
    segment_spawns_saving(fx, 0x01E0_26A0, SPAWNERS, 8, 0x01E0_62E0);
    blib::bolt::bolts(fx, 0x01E0_1B50, BOLTS, OT, 0x28);
    blib::cross::bars(fx, 0x01E0_4810, BARS, OT, 4);
    blib::flicker::flickers(fx, 0x01E0_2A20, D + 0x2EA4, OT, 0x3C);
    blib::beam::beads(fx, 0x01E0_3500, D + 0x3CB4, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x53..0xDC).contains(&n) {
        6
    } else if n == 0xDC {
        2
    } else if n == 0x49 {
        1
    } else {
        0
    }
}
