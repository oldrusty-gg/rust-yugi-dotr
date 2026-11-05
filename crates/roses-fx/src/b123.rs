//! BMODULE b123 (card 482), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E06460, data area a0 = 0x1ED0000) [F] (helper D17,
//! 1 Nov 2026): b097's family (s3 = D, s0 = the OT) with its own records
//! and a register compile of the echoes (`radiate::echoes_b`). On 5 / 20
//! the attacker's echo (+0x11A0: 11 / 14 counts, 6 steps, offset (1, 4),
//! alpha 0x30); on 32..140 three drops an update (+0xA0, 0x44 each, 64
//! slots: 0x14 counts, cell (rand + 2) & 3, 250 wide growing 6, alpha
//! 0x60) at the attacker's nodes 7 / 0xA / 0xD (thrown +-0.1, 0.4 up); on
//! 76 an arc (+0x11C4, nodes 0x16 -> 0x15, 0x28 wide, 8 points); on 108 at
//! the target a halo of 1000 (+0x14A4, 0x1C counts), four bars (+0x14F0,
//! 0x50 each: 0x14 counts, delays 0 / 0 / 1 / 1, 2200 + 300 k long, +45 k
//! turned), spikes (+0x1630, 0x20 counts, a random start) and a shake
//! (+0x40, 80); on 126 the arc's ray bursts (+0x1698 into the ray pool
//! +0x16C4, 14 samples, 0x12 counts). Returns 1 on update 98, 2 on 105,
//! 6 on 108..120, 8 on 160 (and at once for -2 / -3).
//!
//! Library (record addresses): `camera_shake` (ac7dccf5ae36) 0x1E00080,
//! `drops_c` 0x1E00440, `echoes_b` 0x1E00E40, `arcs` 0x1E01630, `halo`
//! 0x1E02A70, `bars_c` 0x1E037C0, `spikes_b` 0x1E04290, `rays_c`
//! 0x1E04FC0, `arc_rays_c` 0x1E05C80 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{first_free, set_counter4}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const DROPS: u32 = D + 0xA0;
const ECHO: u32 = D + 0x11A0;
const ARC: u32 = D + 0x11C4;
const HALO: u32 = D + 0x14A4;
const BARS: u32 = D + 0x14F0;
const SPIKES: u32 = D + 0x1630;
const BURST: u32 = D + 0x1698;
const RAYS: u32 = D + 0x16C4;
const BYTES: u32 = D + 0x1FD4;

/// The entry 0x1E06460 (a0 = D, a1 = param; s3 = D, s1 = param, s0 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x70);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp)
    };
    fx.leave(0x70);
    ret
}

fn init(fx: &mut Fx, _param: i32) -> i32 {
    fx.bzero(D, 0x20E0);
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
    for (at, node, clear) in [(0xCu32, 7, 1), (0x10, 0xA, 0), (0x14, 0xD, 0), (0x18, 0x16, 0), (0x1C, 0x15, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The entry's s0 = the OT, s1 = 2 (the texture loop's), s2 = the
    // runner's 0 [H, as b201].
    rand_bytes(fx, BYTES, 0x100, 0x01E0_65B4, [Some(OT), Some(2), Some(0)]);
    0
}

/// ((rand >> 4) % 20 - 10) / 100 [F].
fn spread(fx: &mut Fx, ra: u32) -> f32 {
    let x = fx.rand_from(OT, ra) >> 4;
    let (_, h) = fx.div(x, 0x14);
    ee::div(ee::from_i32(h - 0xA), 100.0)
}

/// Updates 5 / 0x14 [F 0x1E06630]: the attacker's echo.
fn echo_start(fx: &mut Fx, n: u32) {
    let g = ECHO;
    set_counter4(fx, g, if n == 5 { 0xB } else { 0xE }, 0, 0xFF);
    for (o, v) in [(4u32, 0u8), (8, 6), (9, 1), (0xA, 4), (0xB, 0)] {
        fx.mem.write_u8(g + o, v);
    }
    let h = fx.mem.read_u32(D + 4);
    fx.mem.write_u32(g + 0xC, h);
    for (o, v) in [(0x10u32, 0x80u8), (0x11, 0x80), (0x12, 0x80), (0x13, 0x30)] {
        fx.mem.write_u8(g + o, v);
    }
}

/// Update 0x4C [F 0x1E066C0]: the arc, points +0x1224.
fn arc_start(fx: &mut Fx) {
    let g = ARC;
    set_counter4(fx, g, 0xA0, 0, 0xFF);
    fx.mem.write_u8(g + 4, 1);
    fx.mem.write_u32(g + 8, D + 0x1224);
    fx.mem.write_u8(g + 0x30, 0x28);
    fx.mem.write_u8(g + 0x31, 8);
    let b = fx.mem.read_u8(D + 0x18);
    fx.mem.write_u8(g + 0x32, b);
    let b = fx.mem.read_u8(D + 0x1C);
    fx.mem.write_u8(g + 0x33, b);
    fx.mem.write_u32(g + 0x38, 0x3F80_0000);
    fx.mem.write_u8(g + 0x34, 0);
    fx.mem.set16(g + 0x36, 0);
    for (o, v) in [(0x47u32, 0x80u8), (0x44, 0x80), (0x45, 0x80), (0x46, 0x80), (0x4A, 0x10), (0x49, 0x10), (0x48, 0xC0)] {
        fx.mem.write_u8(g + o, v);
    }
    let mut a = 0u32;
    while a < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
        fx.mem.set16(g + 0x1E + 2 * a, -1);
        fx.mem.set16(g + 0xC + 2 * a, -1);
        a += 1;
    }
}

/// Update 0x6C [F 0x1E0678C]: the halo, the four bars, the spikes and the
/// shake at the target (sp + 0x50).
fn strike(fx: &mut Fx, sp: u32) {
    let r = HALO;
    set_counter4(fx, r, 0x1C, 0, 0xFF);
    for (o, v) in [(5u32, 0u8), (4, 0x81), (7, 9)] {
        fx.mem.write_u8(r + o, v);
    }
    fx.mem.write_u32(r + 0x10, 0x447A_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xCu32, 0xC0u8), (0xD, 0xC0), (0xE, 0xC0), (0xA, 0x80), (9, 0x20), (8, 0xA0), (0xF, 0xD0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, sp + 0x50, r + 0x1C);
    for k in 0..4u32 {
        let r = BARS + 0x50 * k;
        set_counter4(fx, r, 0x14, ((k >> 1) & 0xFF) as i32, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        let l = ee::add(2200.0, ee::mul(300.0, ee::from_i32(k as i32)));
        fx.mem.setf(r + 0x10, l);
        let a = ee::add(fx.mem.f(r + 0x18), ee::mul(45.0, ee::from_i32(k as i32)));
        fx.mem.setf(r + 0x18, a);
        fx.mem.write_u32(r + 0x14, 0x42A0_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x30), (0xA, 0x80), (9, 0x10), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x3F80_0000);
        copy_v4(fx, sp + 0x50, r + 0x20);
    }
    let r = SPIKES;
    fx.mem.write_u8(r + 4, 5);
    set_counter4(fx, r, 0x20, 0, 0xFF);
    let b = fx.mem.read_u8(r + 4) | 2;
    fx.mem.write_u8(r + 4, b);
    fx.mem.write_u32(r + 0x14, 0x3FCC_CCCD);
    fx.mem.write_u32(r + 0x1C, 0x42A0_0000);
    for (o, v) in [(0x23u32, 0x80u8), (0x27, 0x80), (5, 0), (0xA, 1), (0xB, 2), (0xC, 3), (8, 0x80)] {
        fx.mem.write_u8(r + o, v);
    }
    let x = fx.rand_from(OT, 0x01E0_6A8C);
    fx.mem.set8(r + 9, x >> 4);
    fx.mem.write_u32(r + 0x10, BYTES);
    fx.mem.write_u32(r + 0x18, 0x3F80_0000);
    for o in [0x24u32, 0x25, 0x26, 0x20, 0x21, 0x22] {
        fx.mem.write_u8(r + o, 0x80);
    }
    copy_v4(fx, sp + 0x50, r + 0x28);
    let s = D + 0x40;
    set_counter4(fx, s, 0x20, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3F80_0000);
    fx.mem.write_u32(s + 0x1C, 0x3FC0_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x40C0_0000);
    copy_v4(fx, sp + 0x50, s + 0x20);
}

/// Update 0x7E [F 0x1E06B60]: the arc's ray bursts.
fn burst_start(fx: &mut Fx) {
    let g = BURST;
    set_counter4(fx, g, 1, 0, 0xFF);
    fx.mem.write_u8(g + 4, 2);
    fx.mem.write_u32(g + 8, ARC);
    fx.mem.write_u32(g + 0xC, RAYS);
    fx.mem.write_u8(g + 0x14, 0x14);
    fx.mem.write_u8(g + 0x16, 0x28);
    fx.mem.write_u8(g + 0x15, 0x12);
    fx.mem.write_u32(g + 0x18, 0x40D0_0000);
    fx.mem.write_u32(g + 0x1C, 0x3F99_999A);
    fx.mem.write_u8(g + 0x17, 0xE);
    fx.mem.write_u32(g + 0x20, 0x4180_0000);
    fx.mem.write_u32(g + 0x10, BYTES);
    for (o, v) in [(0x26u32, 0x20u8), (0x25, 0x20), (0x24, 0xC0), (0x2B, 0x60), (0x28, 0x60), (0x29, 0x60), (0x2A, 0x60)] {
        fx.mem.write_u8(g + o, v);
    }
}

/// Updates 0x20..0x8C [F 0x1E06C34]: three drops at the nodes +0xC /
/// +0x10 / +0x14.
fn drops(fx: &mut Fx, sp: u32) {
    for s1 in 0..3u32 {
        let i = first_free(fx, DROPS, 0x44, 0x40);
        if i < 0 {
            continue;
        }
        let r = DROPS + 0x44 * i as u32;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        let x = fx.rand_from(OT, 0x01E0_6C90);
        fx.mem.set8(r + 4, (((x >> 4) & 3) << 4) | 4);
        fx.mem.write_u8(r + 9, 0);
        fx.mem.write_u8(r + 8, 1);
        let x = fx.rand_from(OT, 0x01E0_6CE8);
        fx.mem.set8(r + 0xA, ((x >> 4) + 2) & 3);
        for (o, v) in [(0xBu32, 0u8), (0xC, 0x80), (0xD, 0x80), (0xE, 0x80), (0xF, 0x60)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x10, 0x437A_0000);
        fx.mem.write_u32(r + 0x14, 0x40C0_0000);
        fx.mem.write_u32(r + 0x18, 0xBCA3_D70A);
        fx.mem.write_u32(r + 0x1C, 0);
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0xC + 4 * s1);
        node_position_saving(fx, h, slot, sp + 0x60, u64::from(OT), Some(u64::from(s1)), 0x01E0_6DF0);
        for k in 0..3 {
            let v = fx.mem.read_u32(sp + 0x60 + 4 * k);
            fx.mem.write_u32(r + 0x20 + 4 * k, v);
        }
        let v = spread(fx, 0x01E0_6E40);
        fx.mem.setf(r + 0x2C, v);
        fx.mem.write_u32(r + 0x30, 0x3ECC_CCCD);
        let v = spread(fx, 0x01E0_6EB0);
        fx.mem.setf(r + 0x34, v);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x50, u64::from(OT), 0x01E0_65DC);
    let n = fx.mem.u16(D) as u32;
    match n {
        0xA0 => return 8,
        0x7E => burst_start(fx),
        0x6C => strike(fx, sp),
        0x4C => arc_start(fx),
        0x14 | 5 => echo_start(fx, n),
        _ => {}
    }
    let n = fx.mem.u16(D);
    if (0x20..0x8D).contains(&n) {
        drops(fx, sp);
    }
    shake_saving(fx, D + 0x40, u64::from(OT), 0x01E0_6F24, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::drop::drops_c(fx, DROPS, OT, 0x40);
    // a3 = what the arcs left (the echoes' first TEXFLUSH reports it).
    let (_, a3) = blib::arc::arcs_a3(fx, 0x01E0_1630, ARC, OT, 1);
    blib::radiate::echoes_b(fx, 0x01E0_0E40, ECHO, OT, 1, a3);
    blib::halo::halo(fx, 0x01E0_2A70, HALO, OT, 1);
    blib::cross::bars_c(fx, 0x01E0_37C0, BARS, OT, 4);
    blib::spike::spikes_b(fx, 0x01E0_4290, SPIKES, OT, 1);
    blib::ray::arc_rays_c(fx, 0x01E0_5C80, BURST, 1);
    blib::ray::rays_c(fx, 0x01E0_4FC0, RAYS, OT, 0x14);
    let n = fx.mem.u16(D);
    if (0x6C..0x79).contains(&n) {
        6
    } else if n == 0x69 {
        2
    } else if n == 0x62 {
        1
    } else {
        0
    }
}
