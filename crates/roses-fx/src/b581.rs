//! BMODULE b581 (card 385), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E02B00, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): before update 0xEC three jagged strands (+0x70 + 0x30 k,
//! `strand::draw`) between the attacker's nodes 0x2D / 0x2C; a point at
//! node 0x2D (+0x12A0) that, from 0xE3, keeps moving by the nodes' last
//! offset (+0x1FA0) until it falls below 0; a fan of quads (+0x12F0,
//! `fan`) there on 0x32..0xEF, fading in, then flickering; a spark cloud
//! (+0x1760) every update; from 0xEC a frame quad and a ring of blades
//! (+0x1E0, `blade`) about the target. Returns 1 on update 0xE2, 6 on
//! 0xEC..0xF9, 2 on 0xFA, 8 after 0x10E.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the frame quad
//! (0x1E00120), `scatter` (0x1E00240), `trail::sparks` (0x1E00400),
//! `fan` setup / draw (0x1E01160 / 0x1E012C0), `blade` setup / draw
//! (0x1E018D0 / 0x1E01A70), `strand::draw` (0x1E02180).
use crate::blib::{blade, spawn, strand, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STRAND: u32 = 0x01E0_2180;
const BLADE_SETUP: u32 = 0x01E0_18D0;
const FAN_TEMPLATE: u32 = 0x01E0_3300;
const LEFT: u32 = D + 0x1FB0;
const N: u32 = D + 0x1FB4;
const NODES: u32 = D + 0x1FBC;

/// The entry 0x1E02B00 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn hw(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

fn bw(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1FD0);
    for x in [0u32, 0x100, 0x1280, 0x170] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x2D, 0x2C].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x10E);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x1FB8, param);
    fx.texture5(sp + 0xA0, 0);
    // The fan (+0x12F0).
    fx.mem.write_u32(D + 0x12F4, 1);
    hw(fx, &[(0x1300, 0x40), (0x1302, 0), (0x1304, 0x40), (0x1306, 0x40)]);
    bw(fx, &[(0x12FA, 0x80), (0x12F9, 0x80), (0x12F8, 0x80), (0x12FB, 0)]);
    for (o, w) in [(0x1310u32, 0x3F00_0000u32), (0x1314, 0x3F80_0000), (0x1318, 0x4170_0000), (0x131C, 0)] {
        fx.mem.write_u32(D + o, w);
    }
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.fan_setup(D + 0x12F0, tex, 3.0, FAN_TEMPLATE);
    // The spark cloud (+0x1760, particles +0x1790).
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(D + 0x1760, tex);
    fx.mem.write_u32(D + 0x1764, 0x2000_0003);
    bw(fx, &[(0x176A, 0x80), (0x1769, 0x80), (0x1768, 0x80), (0x176B, 0x80)]);
    fx.mem.write_u32(D + 0x1780, 0x3DCC_CCCD);
    fx.mem.write_u32(D + 0x1784, 0x4040_0000);
    hw(fx, &[(0x1770, 0), (0x1772, 0x20), (0x1774, 0x20), (0x1776, 0x20), (0x1778, 0), (0x177A, 0x20), (0x177C, 0x80), (0x177E, 0x40)]);
    fx.mem.write_u32(D + 0x176C, D + 0x1790);
    fx.mem.write_u32(D + 0x1788, 0x4040_0000);
    hw(fx, &[(0x178C, 0x80)]);
    bw(fx, &[(0x178E, 2), (0x178F, 0)]);
    spawn::scatter(fx, D + 0x1760, 0xC, 4.0);
    // The blades (+0x1E0, blades +0x280).
    fx.texture5(sp + 0xA0, 1);
    fx.mem.write_u32(D + 0x1E4, 1);
    hw(fx, &[(0x1F0, 0), (0x1F2, 0), (0x1F4, 0x20), (0x1F6, 0x40)]);
    bw(fx, &[(0x1EA, 0xA0), (0x1E9, 0xB0), (0x1E8, 0x40), (0x1EB, 0x80)]);
    for (o, w) in [(0x260u32, D + 0x280), (0x264, 0x4120_0000), (0x268, 0x41F0_0000), (0x26C, 0x4140_0000), (0x270, 0), (0x274, 0x1_0000), (0x1EC, 0x41F0_0000)] {
        fx.mem.write_u32(D + o, w);
    }
    hw(fx, &[(0x278, 0), (0x27A, 0x100), (0x27C, 0x3E7), (0x27E, 3)]);
    let tex = fx.mem.read_u32(sp + 0xA0);
    blade::setup(fx, BLADE_SETUP, D + 0x1E0, tex);
    // Three strands (+0x70 + 0x30 k).
    fx.texture4(sp + 0xA0, 0xF);
    for k in 0..3u32 {
        let g = 0x70 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 0x8_0000);
        hw(fx, &[(g + 0x10, 0x48), (g + 0x12, 0xC0), (g + 0x14, 0x38), (g + 0x16, 0x40)]);
        bw(fx, &[(g + 0xA, 0xA0), (g + 9, 0xB0), (g + 8, 0x40), (g + 0xB, 0x80)]);
        hw(fx, &[(g + 0xC, 8), (g + 0x28, -4 * k as i32), (g + 0xE, 0xA)]);
        fx.mem.write_u32(D + g + 0x20, 0);
        fx.mem.write_u32(D + g + 0x24, 0);
    }
    0
}

/// The spark cloud's particles (+0x1790 + 0x10 k): pushed out by 1.2,
/// above 20 respawned on a flat ring of radius 0.1 (rand % (10 x +0x28))
/// at height 0.1 (rand % 40) (rand's frame: the entry's s0 = the OT).
fn sparks(fx: &mut Fx) {
    let g = D + 0x1760;
    let grow = f32::from_bits(0x3F99_999A);
    let tenth = f32::from_bits(0x3DCC_CCCD);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(g + 0x2C) {
        let p = D + 0x1790 + 0x10 * j;
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        for o in [0u32, 4, 8] {
            let v = ee::mul(fx.mem.f(p + o), grow);
            fx.mem.setf(p + o, v);
        }
        if ee_le(fx.mem.f(p + 4), 20.0) {
            continue;
        }
        let v = ee::mul(10.0, fx.mem.f(g + 0x28));
        fx.fix_residue(v, 0x01E0_30D4);
        let spread = ee::fixsfsi(v);
        let a = fx.rand_from(OT, 0x01E0_30DC).wrapping_rem(0x1_0000);
        let r = fx.rand_from(OT, 0x01E0_30F8);
        let (_, h) = fx.div(r, spread);
        let rad = ee::mul(tenth, ee::from_i32(h));
        let r = fx.rand_from(OT, 0x01E0_3120);
        let (_, h) = fx.div(r, 0x28);
        fx.mem.setf(p + 4, ee::mul(tenth, ee::from_i32(h)));
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(rad, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(rad, s));
        let r = fx.rand_from(OT, 0x01E0_3170);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -h);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let early = n(fx) < 0xE3;
    if early {
        node(fx, 0, sp + 0x80);
        node(fx, 1, sp + 0x90);
        fx.look_angles(sp + 0x90, sp + 0x80, D + 0x1290);
        node(fx, 0, D + 0x12A0);
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(sp + 0x90 + 4 * c));
            fx.mem.setf(D + 0x1FA0 + 4 * c, v);
        }
    } else if ee_lt(fx.mem.f(D + 0x12A4), 0.0) {
        fx.mem.write_u32(D + 0x12A4, 0);
    } else {
        fx.add3(D + 0x12A0, D + 0x1FA0);
    }
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x1F90);
    if n(fx) < 0xE3 {
        node(fx, 0, D + 0x120);
    }
    if n(fx) == 0xEC {
        for c in 0..4u32 {
            let v = fx.mem.read_u32(D + 0x12A0 + 4 * c);
            fx.mem.write_u32(D + 0x190 + 4 * c, v);
        }
    }
    // s0 = the OT, s1 = sp + 0x90 after the node reads, else the
    // parameter (-1, 64-bit), s2 = D, s3 the runner's 0.
    let s1 = if early { Some(sp + 0x90) } else { Some(0) };
    let regs = [Some(OT), s1, Some(D), Some(0)];
    for (k, (x, ra)) in [(0u32, 0x01E0_2F40u32), (0x100, 0x01E0_2F48), (0x1280, 0x01E0_2F50), (0x170, 0x01E0_2F58)].into_iter().enumerate() {
        if k == 1 {
            fx.xform_billboard_saving(D + x, ra, regs);
            if !early {
                fx.saved(sp - 0x80, &[(0x10, u64::MAX)]);
            }
        } else {
            fx.xform_world_saving(D + x, ra, regs);
            if !early {
                fx.saved(sp - 0xD0, &[(0x30, u64::MAX)]);
            }
        }
    }
    if n(fx) >= 0xEC {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xEC {
        for k in 0..3u32 {
            strand::draw(fx, STRAND, OT, D + 0x70 + 0x30 * k, D, sp + 0x80, sp + 0x90);
        }
    }
    if n(fx) >= 0xEC {
        blade::draw(fx, OT, D + 0x1E0, D + 0x170);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x27A) {
            let p = fx.mem.read_u32(D + 0x260) + 0x10 * j;
            if fx.mem.i16(p + 0xE) >= 0xB {
                fx.step_down(p + 0xC, 0, 8);
            }
            j += 1;
        }
    }
    trail::sparks(fx, OT, D + 0x1760, D + 0x1280);
    if n(fx) >= 0xFB {
        fx.step_down(D + 0x176B, 0, 8);
    }
    sparks(fx);
    let m = n(fx);
    if (0x32..0xF0).contains(&m) {
        if m < 0x42 {
            fx.step_up(D + 0x12FB, 0x10, 1);
        } else {
            let r = fx.rand_from(OT, 0x01E0_31F0);
            let (_, h) = fx.div(r, 0xC);
            fx.mem.write_u8(D + 0x12FB, (h + 4) as u8);
        }
        fx.fan_draw(D + 0x12F0, D + 0x1280);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xFA {
        2
    } else if m == 0xE2 {
        1
    } else if (0xEC..0xFA).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
