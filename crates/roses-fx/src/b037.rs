//! BMODULE b037 (card 155), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E04FA0, data area 0x1ED0000, 0x5370 bytes) [F]
//!: a b129-style program on fixed groups in the
//! data area. Transforms at D+0 / 0x2F0 / 0x570 / 0xE00 / 0xFC0 / 0x1180 /
//! 0x11F0 / 0x22C0; nodes (self) 0x30 (clear) / 0x31 / 0x4A / 0x52 / 0x60 /
//! 0x6A as words at D+0x533C..; left D+0x5330 = 0x154, count D+0x5334.
//!
//! From update 0x19 a streamer (`streamer::push` / `draw`, group D+0x70,
//! points D+0xB0) trails nodes 0x60 / 0x6A on even updates; before 0xD1 a
//! particle burst (D+0xE70); from 0x7B a sprite cloud (D+0x1260) falling
//! and respawned; 0x7C..0xE8 a spiral ribbon (`spiral`, D+0x360, its colour
//! random each update, 0x1E010A0); from 0xD1 a second burst (D+0x1030);
//! from 0xF3 twelve rings (`rings`, 0x1E03410: `ring_draw` with a 31-step
//! open mode, D+0x2330 / 0x3330 / 0x4330 + 0x400 k) through D+0x22C0; from
//! 0xF8 the frame warp (`grid` 0x1E03BC0 + `lens` 0x1E03E80, D+0x5E0: 128
//! spokes x 4 radii of screen points, three rings of frame-textured quads
//! whose UVs turn by D+0xDF0.. per ring). A frame quad every update, a
//! window tint from 0x125, the stage light D+0x5364 down from 0x7B and
//! back from 0x14A. Returns 1 on update 0xAE, 2 on 0x14A, 6 on 0xD1..0x149,
//! 8 once D+0x5330 has run out.
//!
//! Library at its own addresses: `colours4` 0x1E00080, `b262::tint`
//! 0x1E00120, `particle_quads` 0x1E003E0, `frame_quad` 0x1E00F70,
//! `sprites_spawn` / `sprites` 0x1E017D0 / 0x1E01960, `streamer::push` /
//! `draw` 0x1E04520 / 0x1E04600; 0x1E03250 is `ring_setup` without the
//! random heights (bit 0xF is clear in all twelve groups). Constant flags
//! leave these paths dead, not ported: 0x1E00280's 0x105C40 call (D+0xE74
//! bit 0 set), 0x1E010A0's float UVs (D+0x364 bit 0 set), 0x1E03BC0's x / z
//! plane and 0x1E03E80's unturned UVs and mean key (D+0x5E0 = 0xA0081000).
use crate::b076::{node_position_saving, unit_position_saving};
use crate::b258::unit_flags;
use crate::b262::tint;
use crate::b369::unit_flag;
use crate::blib::{afterimage, streamer};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx, OT};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88, ALPHA_ADD};

const D: u32 = crate::bmodule::DATA_AREA;
const REGS_TEX4: u64 = 0x4214_2142_1421;
const LEFT: u32 = D + 0x5330;
const N: u32 = D + 0x5334;
const NODES: u32 = D + 0x533C;
const TINT: u32 = D + 0x5358;
const QUAD: u32 = D + 0x5360;
const LIGHT: u32 = D + 0x5364;
const STREAM: u32 = D + 0x70;
const POINTS: u32 = D + 0xB0;
const SPIRAL: u32 = D + 0x360;
const WARP: u32 = D + 0x5E0;
const BURST_A: u32 = D + 0xE70;
const BURST_B: u32 = D + 0x1030;
const CLOUD: u32 = D + 0x1260;
const RINGS_X: u32 = D + 0x22C0;

/// a1..a3 at the entry's 0x1F8470 `jal`s [run].
const FLAG_ARGS: [u32; 3] = [1, D + 0x22D0, D + 0x22E0];

/// The entry 0x1E04FA0 (D, param), frame 0xB0.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xB0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xB0);
    ret
}

fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let v = fx.rand_from(s0, ra);
    fx.div(v, m).1
}

/// `v` % `k` (a power of two) as the compiler's signed remainder.
fn rem_pow(v: i32, k: i32) -> i32 {
    let r = v & (k - 1);
    if v < 0 && r != 0 {
        r - k
    } else {
        r
    }
}

fn n(fx: &mut Fx) -> i32 {
    fx.mem.i32(N)
}

fn bytes(fx: &mut Fx, at: u32, v: [u8; 4]) {
    for (k, b) in v.into_iter().enumerate() {
        fx.mem.write_u8(at + k as u32, b);
    }
}

fn halves(fx: &mut Fx, at: u32, v: &[i32]) {
    for (k, &h) in v.iter().enumerate() {
        fx.mem.set16(at + 2 * k as u32, h);
    }
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x5370);
    for x in [0u32, 0x2F0, 0xFC0, 0x570, 0xE00, 0x1180, 0x11F0, 0x22C0] {
        fx.xform_init(D + x);
    }
    for (k, (node, clear)) in [(0x30, 1), (0x31, 0), (0x4A, 0), (0x52, 0), (0x60, 0), (0x6A, 0)].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x154);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x5338, param);
    fx.mem.write_u8(LIGHT, 0x80);
    bytes(fx, TINT, [0x80, 0x80, 0x80, 0]);
    bytes(fx, D + 0x535C, [0x80, 0x60, 0x30, 0]);
    bytes(fx, QUAD, [0x80, 0x80, 0x80, 0x60]);
    fx.texture5(sp + 0xA0, 0);
    let tex = fx.mem.read_u32(sp + 0xA0);
    // The second burst (D+0x1030): 8 particles at D+0x1060.
    fx.mem.write_u32(BURST_B, tex);
    fx.mem.set32(BURST_B + 4, 7);
    for o in [0x10u32, 0x14, 0x18] {
        fx.mem.set32(BURST_B + o, 0);
    }
    halves(fx, BURST_B + 0x20, &[0, 0, 0x20, 0x20, 0, 0, 0x80, 0x40]);
    fx.mem.write_u32(BURST_B + 0xC, 0x40C0_0000);
    fx.mem.set16(D + 0x1160, 0x10);
    fx.mem.set16(D + 0x116A, 2);
    fx.mem.set16(D + 0x116C, 0);
    fx.mem.set16(D + 0x1170, 8);
    fx.mem.set16(D + 0x1166, 0xA);
    fx.mem.set16(D + 0x1168, 0xA);
    bytes(fx, BURST_B + 8, [0x80, 0x80, 0x80, 0x40]);
    for j in 0..8i32 {
        let p = D + 0x10 * j as u32;
        let x = rmod(fx, D, 0x01E0_51AC, 0x14) - 0xA;
        fx.mem.setf(p + 0x1060, ee::from_i32(x));
        let y = rmod(fx, D, 0x01E0_51D8, 0x14) - 0xA;
        fx.mem.setf(p + 0x1064, ee::from_i32(y));
        let s = fx.mem.i16(D + 0x116A).wrapping_neg();
        let f = fx.mult(s, j);
        fx.mem.set16(p + 0x106E, f);
        fx.mem.set16(p + 0x106C, 0);
        let c = fx.mem.i16(D + 0x1170) - 1;
        fx.mem.set16(D + 0x116E, c);
    }
    // The sprite cloud (D+0x1260): 256 sprites at D+0x12C0.
    let g = CLOUD;
    fx.mem.write_u32(g, tex);
    fx.mem.write_u32(g + 4, 0x5000_0003);
    bytes(fx, g + 8, [0x80; 4]);
    fx.mem.write_u32(g + 0xC, 0x3F80_0000);
    fx.mem.write_u32(g + 0x20, 0);
    fx.mem.write_u32(g + 0x24, 0x800);
    fx.mem.write_u32(g + 0x28, 0);
    halves(fx, g + 0x30, &[0, 0, 0x20, 0x20, 0, 0, 0x80, 0x40]);
    fx.mem.write_u32(g + 0x40, D + 0x12C0);
    fx.mem.write_u32(g + 0x44, 0);
    fx.mem.write_u32(g + 0x48, 0);
    fx.mem.write_u32(g + 0x4C, 0x4120_0000);
    halves(fx, g + 0x5A, &[0x100, 8, 0]);
    fx.sprites_spawn(g, 2.0);
    let mut k = 0;
    while k < fx.mem.i16(D + 0x12BA) {
        let p = D + 0x10 * k as u32;
        let h = rmod(fx, D, 0x01E0_52E8, 0xC);
        fx.mem.set16(p + 0x12CE, h.wrapping_neg());
        fx.mem.write_u8(p + 0x12CC, 0);
        let r = fx.rand_from(D, 0x01E0_530C);
        fx.mem.set8(p + 0x12CD, rem_pow(r, 8));
        let y = ee::add(fx.mem.f(p + 0x12C4), 20.0);
        fx.mem.setf(p + 0x12C4, y);
        k += 1;
    }
    // The rings: look, then `ring_setup` (height 0, width 0).
    for k in 0..4u32 {
        for b in [0x2330u32, 0x3330] {
            let g = D + b + 0x400 * k;
            fx.mem.write_u32(g + 4, 0x6000_0000);
            halves(fx, g + 0x10, &[0, 0x20, 0x20, 0x10]);
            bytes(fx, g + 8, [0x80; 4]);
        }
    }
    for (g, i, o) in [
        (0x2330u32, 0x40C0_0000u32, 0x4080_0000u32),
        (0x3330, 0x4070_0000, 0x4080_0000),
        (0x2730, 0x4100_0000, 0x40C0_0000),
        (0x3730, 0x40B8_0000, 0x40C0_0000),
        (0x2B30, 0x4120_0000, 0x4100_0000),
        (0x3B30, 0x40F8_0000, 0x4100_0000),
        (0x2F30, 0x4140_0000, 0x4120_0000),
        (0x3F30, 0x411C_0000, 0x4120_0000),
    ] {
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, f32::from_bits(i), f32::from_bits(o), 0.0, 0.0);
    }
    fx.texture4(sp + 0xA0, 0xF);
    for k in 0..4u32 {
        let g = D + 0x4330 + 0x400 * k;
        fx.mem.write_u32(g + 4, 0x2000_0000);
        halves(fx, g + 0x10, &[0x8E, 0, 0x20, 0x80]);
        bytes(fx, g + 8, [0xA0, 0x40, 0x70, 0x40]);
    }
    for (g, i, o) in [
        (0x4330u32, 0x4088_0000u32, 0x4070_0000u32),
        (0x4730, 0x40C8_0000, 0x40B8_0000),
        (0x4B30, 0x4104_0000, 0x40F8_0000),
        (0x4F30, 0x4124_0000, 0x411C_0000),
    ] {
        let tex = fx.mem.read_u32(sp + 0xA0);
        fx.ring_setup(D + g, tex, f32::from_bits(i), f32::from_bits(o), 0.0, 0.0);
    }
    let tex = fx.mem.read_u32(sp + 0xA0);
    // The streamer (D+0x70).
    fx.mem.write_u32(STREAM, tex);
    bytes(fx, STREAM + 4, [0x80, 0x60, 0x30, 0x30]);
    fx.mem.write_u32(STREAM + 8, 0x9003);
    fx.mem.write_u32(STREAM + 0xC, 0x4040_0000);
    halves(fx, STREAM + 0x10, &[0x80, 0xC0, 0x20, 0x20, 0x80, 0xC0, 0x80, 0x100]);
    fx.mem.write_u32(D + 0xA0, 0x3E99_999A);
    halves(fx, D + 0xA4, &[0, 0x18, 0x24, 0, 8]);
    // The warp (D+0x5E0).
    fx.mem.write_u32(WARP, 0xA008_1000);
    bytes(fx, WARP + 4, [0x80; 4]);
    // The spiral (D+0x360): 30 unit points at D+0x390.
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(SPIRAL, tex);
    fx.mem.set32(SPIRAL + 4, 1);
    bytes(fx, SPIRAL + 8, [0xB0, 0xA0, 0x20, 0xFF]);
    fx.mem.write_u32(SPIRAL + 0xC, 0x3F4C_CCCD);
    fx.mem.write_u32(SPIRAL + 0x10, 0x3DCC_CCCD);
    fx.mem.write_u32(SPIRAL + 0x14, 0x41F0_0000);
    fx.mem.write_u32(SPIRAL + 0x18, 0x3D4C_CCCD);
    fx.mem.set32(SPIRAL + 0x1C, 0);
    halves(fx, SPIRAL + 0x20, &[0x8E, 0x54, 0x20, 0x2C]);
    for k in 0..0x1Eu32 {
        let a = ee::mul(f32::from_bits(0x3E56_7750), ee::from_i32(k as i32));
        let c = fx.cosf(a);
        fx.mem.setf(D + 0x390 + 0x10 * k, c);
        let s = fx.sinf(a);
        fx.mem.setf(D + 0x398 + 0x10 * k, s);
        fx.mem.set32(D + 0x394 + 0x10 * k, 0);
    }
    // The first burst (D+0xE70): 16 particles.
    fx.texture4(sp + 0xA0, 1);
    let g = BURST_A;
    let tex = fx.mem.read_u32(sp + 0xA0);
    fx.mem.write_u32(g, tex);
    fx.mem.set32(g + 4, 0x403);
    for o in [0x10u32, 0x14, 0x18, 0x1C] {
        fx.mem.set32(g + o, 0);
    }
    halves(fx, g + 0x28, &[0, 0]);
    halves(fx, g + 0x24, &[0x18, 0x20]);
    halves(fx, g + 0x2C, &[0x80, 0x40]);
    fx.mem.write_u32(g + 0xC, 0x4140_0000);
    fx.mem.set16(g + 0x130, 0x10);
    fx.mem.set16(g + 0x13A, 2);
    fx.mem.set16(g + 0x13C, 1);
    fx.mem.set16(g + 0x140, 0xA);
    fx.mem.set16(g + 0x136, 0xA);
    fx.mem.set16(g + 0x138, 0xA);
    bytes(fx, g + 8, [0x80, 0x80, 0x80, 0]);
    burst_spawn(fx, g);
    0
}

/// 0x1E00280(g), frame 0x50: particle k (+0x30 + 16 k) at +0x10 / +0x14
/// jittered by rand % 2a - a (a = i16 +0x136 / +0x138), +0x18; delay
/// -(+0x13A) k; its frame 0 (flag 0 is set here) and +0x13E = +0x140 - 1.
fn burst_spawn(fx: &mut Fx, g: u32) {
    fx.enter(0x50);
    let mut k = 0i32;
    while k < fx.mem.i16(g + 0x130) {
        let s = g + 0x10 * k as u32;
        for (o, a, ra) in [(0u32, 0x136u32, 0x01E0_02CC), (4, 0x138, 0x01E0_030C)] {
            let w = fx.mem.read_u32(g + 0x10 + o);
            fx.mem.write_u32(s + 0x30 + o, w);
            let a = fx.mem.i16(g + a);
            if a != 0 {
                let r = rmod(fx, k as u32, ra, a << 1) - a;
                let v = ee::add(fx.mem.f(g + 0x10 + o), ee::from_i32(r));
                fx.mem.setf(s + 0x30 + o, v);
            }
        }
        let w = fx.mem.read_u32(g + 0x18);
        fx.mem.write_u32(s + 0x38, w);
        let m = fx.mem.i16(g + 0x13A).wrapping_neg();
        let f = fx.mult(m, k);
        fx.mem.set16(s + 0x3E, f);
        fx.mem.set16(s + 0x3C, 0);
        let c = fx.mem.i16(g + 0x140) - 1;
        fx.mem.set16(g + 0x13E, c);
        k += 1;
    }
    fx.leave(0x50);
}

/// `particle_quads`' register saves (frame 0x160: s0..s3 at +0x10.., s4..
/// fp the runner's 0, ra +0xA0) left below the caller's sp [run: a later
/// streamer key reads the s3 slot's upper half].
fn burst_frame(fx: &mut Fx, s: [u64; 4], ra: u32) {
    frame(fx, 0x160, 0x10, s, ra);
}

/// A callee's `sq` saves left below the caller's sp: s0..s3 = `s` from
/// `at` in its frame of `size`, s4..fp the runner's 0, then ra.
fn frame(fx: &mut Fx, size: u32, at: u32, s: [u64; 4], ra: u32) {
    let f = fx.sp - size;
    fx.saved(f, &[(at, s[0]), (at + 0x10, s[1]), (at + 0x20, s[2]), (at + 0x30, s[3]), (at + 0x40, 0), (at + 0x50, 0), (at + 0x60, 0), (at + 0x70, 0), (at + 0x80, 0), (at + 0x90, u64::from(ra))]);
}

/// late's callees' registers: s0 = the OT, s1 = D, the entry's s2 / s3
/// after `cloud`.
const LATE_REGS: [u64; 4] = [OT as u64, D as u64, (D + 0x1000) as u64, 0x100];

/// 0x1F6830 on the self's node word at D+0x533C + 4 `k` (the entry's s0 =
/// D, s1 = the OT).
fn node(fx: &mut Fx, k: u32, out: u32, ra: u32) {
    let h = fx.self_handle();
    let s = fx.mem.i32(NODES + 4 * k);
    node_position_saving(fx, h, s, out, u64::from(D), Some(u64::from(OT)), ra);
}

fn target(fx: &mut Fx, out: u32, ra: u32) {
    let h = fx.target_handle();
    unit_position_saving(fx, h, out, u64::from(D), ra);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let s2 = sp + 0x80;
    node(fx, 0, sp + 0x70, 0x01E0_5800);
    node(fx, 1, s2, 0x01E0_581C);
    target(fx, D + 0xFE0, 0x01E0_5830);
    let h = fx.self_handle();
    fx.unit_point(1, h, D + 0xE20);
    fx.saved(sp - 0x50, &[(0, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::from(s2)), (0x30, 0), (0x40, 0x01E0_5848)]);
    node(fx, 0, D + 0x11A0, 0x01E0_5860);
    let c = n(fx);
    if ((0x19..0x20).contains(&c) || (0x32..0x41).contains(&c) || (0x4A..0x7B).contains(&c)) && rem_pow(c, 2) == 0 {
        let r = fx.rand_from(D, 0x01E0_58D4);
        if rem_pow(r, 2) != 0 {
            node(fx, 5, sp + 0x90, 0x01E0_5908);
        } else {
            node(fx, 4, sp + 0x90, 0x01E0_5928);
        }
        streamer::push(fx, STREAM, POINTS, sp + 0x90);
    }
    target(fx, D + 0x590, 0x01E0_594C);
    target(fx, D + 0x22E0, 0x01E0_5960);
    if fx.mem.f(D + 0x22E8) > 0.0 {
        fx.mem.set32(D + 0x22D4, 0x8000);
    }
    if n(fx) == 0x7B {
        node(fx, 0, D + 0x310, 0x01E0_59AC);
    }
    fx.look_angles(sp + 0x70, s2, D + 0x300);
    for k in 0..3 {
        let w = fx.mem.read_u32(s2 + 4 * k);
        fx.mem.write_u32(D + 0x310 + 4 * k, w);
    }
    fx.look_angles(s2, sp + 0x70, D + 0x1190);
    let regs = [Some(D), Some(OT), Some(s2), Some(0)];
    for (x, ra, board) in [
        (0u32, 0x01E0_59ECu32, false),
        (0x570, 0x01E0_59F4, false),
        (0x2F0, 0x01E0_59FC, false),
        (0x1180, 0x01E0_5A04, false),
        (0xFC0, 0x01E0_5A0C, true),
        (0xE00, 0x01E0_5A14, true),
        (0x22C0, 0x01E0_5A1C, false),
    ] {
        if board {
            fx.xform_billboard_saving(D + x, ra, regs);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
        }
    }
    if n(fx) >= 0xE9 {
        unit_flag(fx, 1, u64::from(D), 0x01E0_5A40);
    }
    let c = n(fx);
    if (0xF8..0x14A).contains(&c) {
        unit_flags(fx, 1, FLAG_ARGS);
    } else if c == 0x14A {
        unit_flags(fx, 0, FLAG_ARGS);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if (0x7B..0xF8).contains(&n(fx)) {
        fx.step_down(LIGHT, 0x30, 2);
    }
    if n(fx) >= 0x14A {
        fx.step_up(LIGHT, 0x80, 0x10);
    }
    if n(fx) >= 0x125 {
        tint(fx, TINT);
        fx.step_up(TINT + 3, 0xFF, 8);
    }
    fx.frame_quad(QUAD);
    if n(fx) >= 0xE9 {
        fx.step_down(QUAD + 3, 0, 6);
    }
    if n(fx) < 0xD1 {
        for o in [0x98u32, 0x94, 0x90] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.particle_quads(D + 0xE00, BURST_A, sp + 0x90);
        burst_frame(fx, [u64::from(D), u64::from(OT), u64::from(sp + 0x80), 0], 0x01E0_5B60);
        let c = n(fx);
        if (0x5E..0x7B).contains(&c) {
            fx.step_up(BURST_A + 0xB, 0x20, 2);
        } else if c >= 0x7C {
            fx.step_down(BURST_A + 0xB, 0, 2);
        }
    }
    streamer::draw(fx, 0x01E0_4600, OT, STREAM, POINTS, D);
    let mut k = 0;
    while k < fx.mem.i16(D + 0xAA) {
        fx.step_down(D + 0xBC + 0x10 * k as u32, 0, 2);
        k += 1;
    }
    if n(fx) >= 0x95 {
        fx.step_down(STREAM + 7, 0, 8);
    }
    if n(fx) >= 0xF3 {
        for k in 0..4u32 {
            let o = 0x400 * k;
            let regs = [u64::from(D), u64::from(OT), u64::from(D + o), u64::from(k)];
            for (b, ra) in [(0x4330u32, 0x01E0_5C40), (0x2330, 0x01E0_5C50), (0x3330, 0x01E0_5C60)] {
                rings(fx, D + b + o, RINGS_X);
                frame(fx, 0x180, 0, regs, ra);
            }
            fx.step_down(D + 0x233B + o, 0, 8);
            fx.step_down(D + 0x333B + o, 0, 8);
            fx.step_down(D + 0x433B + o, 0, 4);
        }
        let r = fx.mem.i32(D + 0x22D8);
        let d = if fx.mem.f(D + 0x22E8) > 0.0 { -0x300 } else { 0x300 };
        fx.mem.set32(D + 0x22D8, r.wrapping_add(d));
        fx.approach(RINGS_X, 3.0, f32::from_bits(0x3DCC_CCCD));
    }
    if n(fx) >= 0x7B {
        fx.sprites(CLOUD, D + 0x1180);
        cloud(fx);
        if n(fx) >= 0xD1 {
            fx.step_down(CLOUD + 0xB, 0, 2);
        }
        late(fx);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x14A {
        2
    } else if m == 0xAE {
        1
    } else if (0xD1..0x14A).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}

/// The cloud's sprites (D+0x12C0 + 16 k): falling by 2, alpha up by 8,
/// drawn in by 0.8 while above 0; below 0 respawned on the radius D+0x12AC
/// at 20 + rand % 10.
fn cloud(fx: &mut Fx) {
    let mut k = 0i32;
    while k < fx.mem.i16(D + 0x12BA) {
        let p = D + 0x10 * k as u32;
        k += 1;
        if fx.mem.i16(p + 0x12CE) < 0 {
            continue;
        }
        let y = ee::sub(fx.mem.f(p + 0x12C4), 2.0);
        fx.mem.setf(p + 0x12C4, y);
        fx.step_up(p + 0x12CC, 0x80, 8);
        if fx.mem.f(p + 0x12C4) > 0.0 {
            let s = f32::from_bits(0x3F4C_CCCD);
            for o in [0x12C8u32, 0x12C0] {
                let v = ee::mul(fx.mem.f(p + o), s);
                fx.mem.setf(p + o, v);
            }
        }
        if fx.mem.f(p + 0x12C4) < 0.0 {
            let a = fx.rand_from(D, 0x01E0_5DBC) & 0xFFFF;
            let c = fx.tcos(a);
            let v = ee::mul(fx.mem.f(D + 0x12AC), c);
            fx.mem.setf(p + 0x12C8, v);
            let s = fx.tsin(a);
            let v = ee::mul(fx.mem.f(D + 0x12AC), s);
            fx.mem.setf(p + 0x12C0, v);
            let h = rmod(fx, D, 0x01E0_5E00, 0xA);
            fx.mem.setf(p + 0x12C4, ee::add(20.0, ee::from_i32(h)));
            fx.mem.write_u8(p + 0x12CC, 0);
        }
    }
}

/// 0x1E05F20(D, OT), frame 0x40: the spiral, the second burst, the warp.
fn late(fx: &mut Fx) {
    let sp = fx.enter(0x40);
    if (0x7C..0xE9).contains(&n(fx)) {
        spiral(fx, SPIRAL, D + 0x2F0);
        frame(fx, 0x180, 0, LATE_REGS, 0x01E0_5F60);
        for (o, ra) in [(0xAu32, 0x01E0_5F68), (9, 0x01E0_5F84), (8, 0x01E0_5FA0)] {
            let r = fx.rand_from(OT, ra);
            fx.mem.set8(SPIRAL + o, rem_pow(r, 0x80));
        }
        if n(fx) >= 0xB8 {
            let c = fx.mem.i32(SPIRAL + 0x1C) + 0xA;
            fx.mem.set32(SPIRAL + 0x1C, c);
        }
        if fx.mem.i32(SPIRAL + 0x1C) >= 0x12D {
            fx.step_down(SPIRAL + 0xB, 0, 8);
        }
    }
    if n(fx) >= 0xD1 {
        for o in [0x38u32, 0x34, 0x30] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.particle_quads(D + 0xFC0, BURST_B, sp + 0x30);
        burst_frame(fx, LATE_REGS, 0x01E0_6024);
        if n(fx) >= 0xF3 {
            fx.step_down(BURST_B + 0xB, 0, 4);
        }
    }
    if n(fx) >= 0xF8 {
        let key = afterimage::depth_key(fx, D + 0xFE0, 10.0);
        fx.mem.write_u32(WARP + 0xC, key);
        grid(fx, WARP, D + 0x570, f32::from_bits(0x404C_CCCD), 8.0, 0.0);
        frame(fx, 0xF0, 0x10, LATE_REGS, 0x01E0_608C);
        lens(fx, WARP);
        frame(fx, 0x170, 0, LATE_REGS, 0x01E0_6094);
        let c = n(fx);
        let by = if c < 0x125 {
            2
        } else if c < 0x134 || rem_pow(c, 2) == 0 {
            1
        } else {
            0
        };
        if by != 0 {
            for (o, d) in [(0xDF0u32, by), (0xDF2, -by), (0xDF4, by)] {
                let v = fx.mem.i16(D + o) + d;
                fx.mem.set16(D + o, v);
            }
        }
    }
    fx.leave(0x40);
}

/// A 12.4 UV coordinate at `a` (its top two bits kept).
fn uv14(fx: &mut Fx, a: u32, v: i32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v as u16 & 0x3FFF));
}

/// 0x1E010A0(ot, g, x), frame 0x180: a ribbon winding out from the
/// centre: step s (< word +0x1C) from point s % 30 (+0x30 + 16 k) x +0x18
/// x s, height +0x10 x s, to the next point at s + 1; two quads, up and
/// down by +0xC, their far edge at alpha 0; a quad's near alpha halved when
/// it faces away. The projected flags are never cleared here (sic: once
/// all four have projected every later quad draws).
fn spiral(fx: &mut Fx, g: u32, x: u32) {
    let sp = fx.enter(0x180);
    let d = sp + 0xF0;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    fx.desc_alpha(d, ALPHA_ADD);
    fx.colours4(d, g + 8);
    fx.mem.write_u8(sp + 0x14B, 0);
    fx.mem.write_u8(sp + 0x133, 0);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let (u, v, w, h) = (fx.mem.i16(g + 0x20), fx.mem.i16(g + 0x22), fx.mem.i16(g + 0x24), fx.mem.i16(g + 0x26));
    let (r, b) = (u + w - 1, v + h - 1);
    for (a, cu, cv) in [(0x108u32, u, v), (0x120, r, v), (0x138, u, b), (0x150, r, b)] {
        uv14(fx, sp + a, cu << 4);
        uv14(fx, sp + a + 2, cv << 4);
    }
    let m = fx.read_m4(x + 0x30);
    let ws = [sp + 0xA0, sp + 0xA4, sp + 0xA8, sp + 0xAC];
    let mut s = 0i32;
    while s < fx.mem.i32(g + 0x1C) {
        let i = fx.div(s, 0x1E).1;
        let j = if i + 1 == 0x1E { 0 } else { i + 1 };
        for (k, at, step, xyz, w, bitn) in [(i, 0xB0u32, s, 0x110u32, ws[0], 0x14), (j, 0xC0, s + 1, 0x128, ws[1], 0x15)] {
            let p = g + 0x10 * k as u32;
            let f = ee::from_i32(step);
            let r = fx.mem.f(g + 0x18);
            let (px, pz, hy) = (fx.mem.f(p + 0x30), fx.mem.f(p + 0x38), fx.mem.f(g + 0x10));
            fx.mem.setf(sp + at, ee::mul(f, ee::mul(px, r)));
            fx.mem.setf(sp + at + 8, ee::mul(f, ee::mul(pz, r)));
            fx.mem.setf(sp + at + 4, ee::mul(hy, f));
            if fx.project_at(sp + at, &m, sp + xyz, w) {
                fx.flag(g + 4, bitn);
            }
        }
        for o in [0u32, 8] {
            let a = fx.mem.read_u32(sp + 0xB0 + o);
            fx.mem.write_u32(sp + 0xD0 + o, a);
            let a = fx.mem.read_u32(sp + 0xC0 + o);
            fx.mem.write_u32(sp + 0xE0 + o, a);
        }
        for up in [true, false] {
            for (from, to, xyz, w, bitn) in [(0xB4u32, 0xD4u32, 0x140u32, ws[2], 0x16), (0xC4, 0xE4, 0x158, ws[3], 0x17)] {
                let (y, c) = (fx.mem.f(sp + from), fx.mem.f(g + 0xC));
                fx.mem.setf(sp + to, if up { ee::add(y, c) } else { ee::sub(y, c) });
                if fx.project_at(sp + to - 4, &m, sp + xyz, w) {
                    fx.flag(g + 4, bitn);
                }
            }
            if fx.all_projected(g + 4) {
                let c = |fx: &mut Fx, a: u32| i32::from(fx.mem.read_u16(sp + a));
                let (x0, y0, x1, y1, x2, y2) = (c(fx, 0x110), c(fx, 0x112), c(fx, 0x128), c(fx, 0x12A), c(fx, 0x140), c(fx, 0x142));
                let cross = (x2 - x0).wrapping_mul(y1 - y0).wrapping_sub((x1 - x0).wrapping_mul(y2 - y0));
                let a = fx.mem.read_u8(g + 0xB);
                let half = if up { cross > 0 } else { cross <= 0 };
                let a = if half { a >> 1 } else { a };
                fx.mem.write_u8(sp + 0x11B, a);
                fx.mem.write_u8(sp + 0x103, a);
                let rh2 = fx.mem.read_u32(g);
                fx.desc_texture(d, rh2, 0);
                let key = fx.mean_key4(ws);
                fx.submit(d, OT, key);
            }
        }
        s += 1;
    }
    fx.leave(0x180);
}

/// 0x1E03410(ot, g, x), frame 0x180: `ring_draw` with the colour and UVs
/// set once and, with flag 0x1E, 31 steps in order 0 joining inner point
/// k - 1 (twice) to outer points k + 1 and k (sic), the last step
/// (+0x200, +0x200, +0x220, +0x30).
fn rings(fx: &mut Fx, g: u32, x: u32) {
    let sp = fx.enter(0x180);
    let d = sp + 0xF0;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    fx.alpha_for(d, g + 4, &[(0xB, ALPHA_88), (0xA, ALPHA_42)]);
    fx.colours4(d, g + 8);
    let open = bit(fx.mem.read_u32(g + 4), 0x1E);
    let count = if open { 0x1F } else { 0x1E };
    fx.uv_rect(d, g + 0x10, if open { 0 } else { 2 });
    fx.mem.set32(sp + 0xA0, count);
    fx.mem.write_u32(sp + 0xD8, OT);
    let m = fx.read_m4(x + 0x30);
    let xyz = [sp + 0x110, sp + 0x128, sp + 0x140, sp + 0x158];
    let ws = [sp + 0xE0, sp + 0xE4, sp + 0xE8, sp + 0xEC];
    for k in 0..count as u32 {
        let (s, p) = (g + 0x10 * k, g.wrapping_add(0x10 * k).wrapping_sub(0x10));
        let v = if bit(fx.mem.read_u32(g + 4), 0x1E) {
            if k == 0x1E {
                [g + 0x200, g + 0x200, g + 0x220, g + 0x30]
            } else {
                let a = if k == 0 { g + 0x20 } else { p + 0x40 };
                let c = if k == 0x1D { g + 0x30 } else { s + 0x230 };
                [a, a, c, s + 0x220]
            }
        } else {
            let (a, b) = if k == 0 { (g + 0x20, g + 0x30) } else { (p + 0x40, p + 0x220) };
            let (c, e) = if k == 0x1D { (g + 0x20, g + 0x30) } else { (s + 0x40, s + 0x220) };
            [a, b, c, e]
        };
        for (i, a) in v.into_iter().enumerate() {
            if fx.project_at(a, &m, xyz[i], ws[i]) {
                fx.flag(g + 4, 0x14 + i as u32);
            }
        }
        fx.region_clamp(d, g + 0x10, g + 0x14, g + 0x12, g + 0x16);
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 1);
        if fx.all_projected(g + 4) {
            if bit(fx.mem.read_u32(g + 4), 0x13) {
                fx.far_z(&xyz);
                fx.submit(d, OT, 0);
            } else {
                let key = fx.mean_key4(ws);
                fx.submit(d, OT, key);
            }
        }
        fx.clear_projected(g + 4);
    }
    fx.leave(0x180);
}

/// 0x1E03BC0(g, x, r0, r1, h), frame 0xF0: 128 spokes x 4 radii (r0 ..
/// r1) in the x / y plane at z `h`, their screen x / y (0x204D80 through
/// `x` + 0x30; kept from the last point when off screen) at +0x10 + 4 j +
/// 0x200 k.
fn grid(fx: &mut Fx, g: u32, x: u32, r0: f32, r1: f32, h: f32) {
    let sp = fx.enter(0xF0);
    let step = ee::div(ee::sub(r1, r0), 3.0);
    fx.mem.setf(sp + 0xB8, h);
    for k in 0..4u32 {
        fx.mem.setf(sp + 0xD0 + 4 * k, ee::add(r0, ee::mul(step, ee::from_i32(k as i32))));
    }
    let m = fx.read_m4(x + 0x30);
    for j in 0..0x80u32 {
        let a = ee::mul(f32::from_bits(0x3D49_0FDB), ee::from_i32(j as i32));
        let c = fx.cosf(a);
        fx.mem.setf(sp + 0xB0, c);
        let s = fx.sinf(a);
        fx.mem.setf(sp + 0xB4, s);
        for k in 0..4u32 {
            let r = fx.mem.f(sp + 0xD0 + 4 * k);
            fx.mem.setf(sp + 0xC0, ee::mul(c, r));
            fx.mem.setf(sp + 0xC4, ee::mul(s, r));
            fx.mem.setf(sp + 0xC8, h);
            fx.project_at(sp + 0xC0, &m, sp + 0xE8, sp + 0xBC);
            let p = g + 4 * j + 0x200 * k;
            let (sx, sy) = (fx.mem.read_u16(sp + 0xE8), fx.mem.read_u16(sp + 0xEA));
            fx.mem.write_u16(p + 0x10, sx);
            fx.mem.write_u16(p + 0x12, sy);
        }
    }
    fx.leave(0xF0);
}

/// 0x1E03E80(g), frame 0x170: in a packet layer, the frame copied as a
/// texture and three rings of 128 quads on the grid, far Z, into the
/// layer; each ring's UVs the grid's own points turned by its i16 at
/// +0x810 + 2 r (kept within +-127).
fn lens(fx: &mut Fx, g: u32) {
    let sp = fx.enter(0x170);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xAC, layer);
    let d = sp + 0xE0;
    fx.desc_init(d, 12, 0x34, REGS_TEX4);
    fx.desc_alpha(d, ALPHA_44);
    fx.colours4(d, g + 4);
    // a0..a3 as 0x1E00080 leaves them (a3: 0x207E20's flags).
    let regs = crate::lens::frame_copy(fx, [d, g + 4, 0x3F80_0000, 0x34], 0, sp + 0x158, sp + 0x160);
    fx.texflush(regs);
    for r in 0..4u32 {
        let v = rem_pow(fx.mem.i16(g + 0x810 + 2 * r), 0x80);
        fx.mem.set16(g + 0x810 + 2 * r, v);
    }
    let xyz = [sp + 0x100, sp + 0x118, sp + 0x130, sp + 0x148];
    let uvs = [sp + 0xF8, sp + 0x110, sp + 0x128, sp + 0x140];
    for j in 0..0x80i32 {
        for r in 0..3u32 {
            let base = g + 0x200 * r;
            let jn = if j + 1 == 0x80 { 0 } else { j + 1 };
            let off = fx.mem.i16(g + 0x810 + 2 * r);
            let wrap = |v: i32| {
                let m = rem_pow(v, 0x80);
                if m < 0 {
                    m + 0x80
                } else {
                    m
                }
            };
            let (ja, jb) = (wrap(j + off), wrap(j + 1 + off));
            let at = |k: i32, o: u32| base + 4 * k as u32 + o;
            for (i, p) in [at(j, 0x10), at(j, 0x210), at(jn, 0x10), at(jn, 0x210)].into_iter().enumerate() {
                let (sx, sy) = (fx.mem.read_u16(p), fx.mem.read_u16(p + 2));
                fx.mem.write_u16(xyz[i], sx);
                fx.mem.write_u16(xyz[i] + 2, sy);
            }
            let fl = fx.mem.read_u16(d + 2);
            fx.mem.write_u16(d + 2, fl | 0x800);
            for (i, p) in [at(ja, 0x10), at(ja, 0x210), at(jb, 0x10), at(jb, 0x210)].into_iter().enumerate() {
                let (u, v) = (fx.mem.i16(p), fx.mem.i16(p + 2));
                uv14(fx, uvs[i], u - 0x7000);
                uv14(fx, uvs[i] + 2, v - 0x7000);
            }
            fx.far_z(&xyz);
            fx.submit(d, 0, 0);
        }
    }
    let layer = fx.mem.read_u32(sp + 0xAC);
    fx.layer_end(layer, OT, 0);
    fx.leave(0x170);
}
