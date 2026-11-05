//! BMODULE b187 (card 239), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E042E0, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): before update 0x69 a sprite fountain (+0x960, falling and
//! re-thrown on a ring) at the attacker's node 0x1F and, from 0x1E, the
//! attacker zooming (+0x380, `zoom::sprites`, its spread +0x388 rising to
//! 40 by 2 then falling from 0x3D); from 0x69 a frame quad, a disc
//! (+0x400) flashing at the node and a ribbon (+0x70, `ribbon`) from the
//! node closing on the target by 1 / 19 a step; from 0x6A a fan (+0x1A30,
//! `fan`) swelling to 3 there; from 0x7B a disc (+0x8E0) and a quad group
//! (+0x480) at the target, growing to 2. The stage dims (0x1F7F10) until
//! 0xCC. Returns 1 on update 0x6F, 6 on 0x7B..0xCB, 2 on 0xCC, 8 after
//! 0xE6.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the frame quad
//! (0x1E00120), `sprites_spawn` / `sprites` (0x1E00240 / 0x1E003D0),
//! `disc` (0x1E01CC0), `zoom::sprites` (0x1E02220), `quads_setup` /
//! `quads_draw` (0x1E028F0 / 0x1E02A50), `ribbon` push / draw (0x1E03060 /
//! 0x1E031B0), `fan` setup / draw (0x1E03B70 / 0x1E03CD0).
use crate::b142::fstep_up;
use crate::b436::fstep_down;
use crate::blib::zoom;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const D: u32 = crate::bmodule::DATA_AREA;
const ZOOM: u32 = 0x01E0_2220;
const FAN_TEMPLATE: u32 = 0x01E0_4DC0;
const QUAD_TEMPLATE: u32 = 0x01E0_4D80;
const LEFT: u32 = D + 0x1EA0;
const N: u32 = D + 0x1EA4;
const LIGHT: u32 = D + 0x1EAC;
const NODES: u32 = D + 0x1EC8;

/// The entry 0x1E042E0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x90);
    ret
}

fn bytes(fx: &mut Fx, at: &[(u32, u8)]) {
    for &(o, v) in at {
        fx.mem.write_u8(D + o, v);
    }
}

fn halves(fx: &mut Fx, at: &[(u32, i32)]) {
    for &(o, v) in at {
        fx.mem.set16(D + o, v);
    }
}

fn node(fx: &mut Fx, k: u32, out: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    fx.node_position(h, s, out);
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x1EE0);
    for x in [0u32, 0x2A0, 0x8F0, 0x390, 0x410, 0x19C0, 0x310] {
        fx.xform_init(D + x);
    }
    for (k, n) in [0x1F, 8, 0x40, 0x48].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0xE6);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x1EA8, param);
    fx.mem.set32(D + 0x1EB0, 0x13);
    fx.mem.set32(LIGHT, 0x80);
    bytes(fx, &[(0x1EB6, 0xE0), (0x1EB5, 0x70), (0x1EB4, 0xA0), (0x1EB7, 0)]);
    // Discs (+0x400 radius 10, +0x8E0 radius 15).
    for (g, flags, r) in [(0x400u32, 0x8_0000u32, 0x4120_0000u32), (0x8E0, 0, 0x4170_0000)] {
        fx.mem.write_u32(D + g, flags);
        bytes(fx, &[(g + 6, 0xE0), (g + 5, 0x50), (g + 4, 0x60), (g + 7, 0)]);
        fx.mem.write_u32(D + g + 8, r);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    // The zoom (+0x380) of the attacker.
    fx.mem.write_u32(D + 0x380, 0);
    bytes(fx, &[(0x386, 0x80), (0x385, 0x80), (0x384, 0x80), (0x387, 0x30)]);
    fx.mem.write_u32(D + 0x388, 0);
    halves(fx, &[(0x38C, 5)]);
    let h = fx.self_handle();
    halves(fx, &[(0x38E, h)]);
    fx.texture5(sp + 0x80, 0);
    // The ribbon (+0x70, points +0xA0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x78, 0x8003);
    bytes(fx, &[(0x76, 0x80), (0x75, 0x80), (0x74, 0x80), (0x77, 0x80)]);
    halves(fx, &[(0x80, 0), (0x82, 0), (0x84, 0x10), (0x86, 0x20), (0x88, 0), (0x8A, 0), (0x8C, 0x80), (0x8E, 0x40)]);
    fx.mem.write_u32(D + 0x7C, 0x4316_0000);
    halves(fx, &[(0x92, 8), (0x94, 0), (0x96, 0x30), (0x98, 0x20), (0x9A, 0), (0x9C, 1)]);
    // The fan (+0x1A30).
    fx.mem.write_u32(D + 0x1A34, 1);
    halves(fx, &[(0x1A40, 0x40), (0x1A42, 0), (0x1A44, 0x40), (0x1A46, 0x40)]);
    bytes(fx, &[(0x1A3A, 0x80), (0x1A39, 0x80), (0x1A38, 0x80), (0x1A3B, 0x40)]);
    for (o, w) in [(0x1A50u32, 0x3F00_0000u32), (0x1A54, 0x3F80_0000), (0x1A58, 0x4100_0000), (0x1A5C, 0)] {
        fx.mem.write_u32(D + o, w);
    }
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.fan_setup(D + 0x1A30, tex, 3.0, FAN_TEMPLATE);
    // The sprite fountain (+0x960, particles +0x9C0).
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.mem.write_u32(D + 0x960, tex);
    fx.mem.write_u32(D + 0x964, 0x7000_0003);
    bytes(fx, &[(0x96A, 0x80), (0x969, 0x80), (0x968, 0x80), (0x96B, 0)]);
    fx.mem.write_u32(D + 0x96C, 0x3F00_0000);
    for o in [0x980u32, 0x984, 0x988] {
        fx.mem.write_u32(D + o, 0);
    }
    halves(fx, &[(0x990, 0), (0x992, 0x20), (0x994, 0x20), (0x996, 0x20), (0x998, 0), (0x99A, 0x20), (0x99C, 0x80), (0x99E, 0x40)]);
    fx.mem.write_u32(D + 0x9A0, D + 0x9C0);
    fx.mem.write_u32(D + 0x9A4, 0);
    fx.mem.write_u32(D + 0x9A8, 0);
    fx.mem.write_u32(D + 0x9AC, 0x4170_0000);
    halves(fx, &[(0x9BA, 0x100), (0x9BC, 1), (0x9BE, 0)]);
    fx.sprites_spawn(D + 0x960, 0.5);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x9BA) {
        let p = D + 0x10 * j;
        let r = fx.rand_from(D, 0x01E0_4608);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0x9CE, -h);
        fx.mem.write_u8(p + 0x9CC, 0x30);
        fx.mem.write_u8(p + 0x9CD, 0);
        let y = ee::add(fx.mem.f(p + 0x9C4), 10.0);
        fx.mem.setf(p + 0x9C4, y);
        j += 1;
    }
    // The quad group (+0x480), texture 4 / 0xF.
    fx.texture4(sp + 0x80, 0xF);
    fx.mem.write_u32(D + 0x484, 1);
    halves(fx, &[(0x490, 0x8E), (0x492, 0), (0x494, 0x22), (0x496, 0x40)]);
    bytes(fx, &[(0x48A, 0xE0), (0x489, 0x50), (0x488, 0x60), (0x48B, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0x80);
    fx.quads_setup(D + 0x480, tex, 2.0, QUAD_TEMPLATE);
    for o in [0x418u32, 0x414, 0x410] {
        fx.mem.write_u32(D + o, 0x3DCC_CCCD);
    }
    0
}

/// The fountain's particles (+0x9C0 + 0x10 k): falling by 1, fading in by
/// 8, drawn in by 0.8 while above 0; below 0 re-thrown on a ring of radius
/// +0x9AC at height 10 + rand % 10 (rand's frame: the entry's s0 = D).
fn fountain(fx: &mut Fx) {
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x9BA) {
        let p = D + 0x10 * j;
        j += 1;
        if fx.mem.i16(p + 0x9CE) < 0 {
            continue;
        }
        let y = ee::sub(fx.mem.f(p + 0x9C4), 1.0);
        fx.mem.setf(p + 0x9C4, y);
        fx.step_up(p + 0x9CC, 0x80, 8);
        if !ee_le(fx.mem.f(p + 0x9C4), 0.0) {
            let s = f32::from_bits(0x3F4C_CCCD);
            for o in [0x9C8u32, 0x9C0] {
                let v = ee::mul(fx.mem.f(p + o), s);
                fx.mem.setf(p + o, v);
            }
        }
        if ee_lt(fx.mem.f(p + 0x9C4), 0.0) {
            let a = fx.rand_from(D, 0x01E0_4B84).wrapping_rem(0x1_0000);
            let c = fx.tcos(a);
            let r = fx.mem.f(D + 0x9AC);
            fx.mem.setf(p + 0x9C8, ee::mul(r, c));
            let s = fx.tsin(a);
            let r = fx.mem.f(D + 0x9AC);
            fx.mem.setf(p + 0x9C0, ee::mul(r, s));
            let r = fx.rand_from(D, 0x01E0_4BC8);
            let (_, h) = fx.div(r, 0xA);
            fx.mem.setf(p + 0x9C4, ee::add(10.0, ee::from_i32(h)));
            fx.mem.write_u8(p + 0x9CC, 0);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0x330);
    if n(fx) == 0x69 {
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x60);
        node(fx, 0, sp + 0x70);
        fx.look_angles(sp + 0x60, sp + 0x70, D + 0x19D0);
        for c in 0..4u32 {
            let v = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(D + 0x19E0 + 4 * c, v);
        }
    }
    node(fx, 0, sp + 0x60);
    node(fx, 1, sp + 0x70);
    fx.look_angles(sp + 0x60, sp + 0x70, D + 0x900);
    for out in [0x1EB8u32, 0x430] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    node(fx, 0, D + 0x910);
    node(fx, 0, D + 0x3B0);
    if n(fx) < 0x6A {
        node(fx, 0, D + 0x2C0);
    }
    if n(fx) >= 0x69 {
        fx.ribbon_push(D + 0x70, D + 0xA0, D + 0x2C0);
        let m = fx.mem.i32(D + 0x1EB0);
        fx.approach_by(D + 0x2C0, D + 0x1EB8, m);
        let m = fx.mem.i32(D + 0x1EB0);
        if m != 0 {
            fx.mem.set32(D + 0x1EB0, m - 1);
        }
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x70, s3 the runner's 0.
    let regs = [Some(D), Some(crate::exe::OT), Some(sp + 0x70), Some(0)];
    for (x, ra, board) in [(0u32, 0x01E0_4874u32, false), (0x2A0, 0x01E0_487C, true), (0x8F0, 0x01E0_4884, false), (0x390, 0x01E0_488C, true), (0x410, 0x01E0_4894, true), (0x19C0, 0x01E0_489C, false)] {
        if board {
            fx.xform_billboard_saving(D + x, ra, regs);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
        }
    }
    if n(fx) >= 0x69 {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xCC {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if (0x1E..0x69).contains(&n(fx)) {
        zoom::sprites(fx, ZOOM, D + 0x380, D + 0x330);
        if n(fx) >= 0x3D {
            fstep_down(fx, D + 0x388, 0.0, 2.0);
        } else {
            fstep_up(fx, D + 0x388, 40.0, 2.0);
        }
    }
    if n(fx) >= 0x7B {
        if n(fx) < 0xB2 {
            fx.approach(D + 0x410, 2.0, f32::from_bits(0x3D4C_CCCD));
        } else {
            for o in [0x418u32, 0x414, 0x410] {
                fx.mem.write_u32(D + o, 0x3F80_0000);
            }
        }
        fx.disc(D + 0x8E0, D + 0x410);
        fx.quads_draw(D + 0x480, D + 0x410);
        if n(fx) >= 0xBE {
            fx.step_down(D + 0x8E7, 0, 0x10);
            fx.step_down(D + 0x48B, 0, 2);
        } else {
            fx.step_up(D + 0x8E7, 0xFF, 8);
        }
        for k in 0..0x40u32 {
            for c in 0..3u32 {
                let a = D + 0x4E0 + 0x10 * k + 4 * c;
                let w = fx.mem.read_u32(a).wrapping_add(0x80);
                fx.mem.write_u32(a, w);
            }
        }
    }
    if n(fx) >= 0x69 {
        if n(fx) == 0x69 {
            fx.mem.write_u8(D + 0x407, 0xFF);
        }
        fx.disc(D + 0x400, D + 0x390);
        fx.step_down(D + 0x407, 0, 4);
    }
    if n(fx) < 0x69 {
        fx.sprites(D + 0x960, D + 0x8F0);
        if n(fx) >= 0x51 {
            fx.step_up(D + 0x96B, 0x80, 8);
        }
        fountain(fx);
    }
    if n(fx) >= 0x6A {
        fx.fan_draw(D + 0x1A30, D + 0x19C0);
        fx.step_down(D + 0x1A3B, 0, 1);
        fx.approach(D + 0x19C0, 3.0, f32::from_bits(0x3ECC_CCCD));
        fx.ribbon_draw(D + 0x70, D + 0xA0, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xCC {
        2
    } else if m == 0x6F {
        1
    } else if (0x7B..0xCC).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
