//! BMODULE b638 (card 607), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03E30, data area a0 = 0x1ED0000) [F] (helper C9,
//! 31 Oct 2026): b274's program with four fans and eight more spark
//! clouds. Two points (+0x410 / +0x510) at the attacker's nodes 9 / 0xA
//! until update 0x9B, then closing on the target with streamers (+0x70 /
//! +0x230); discs and facing quads at them from 0x4A; from 0x28 eight
//! spark clouds (+0x37B0 + 0x30 k, local 0x1E05330) about the target, a
//! particle cloud (+0x59A0) and four fans (+0x2270 + 0x470 k, `fan`) at
//! the nodes 5 / 6 / 8 / 7 facing the attacker, flickering (re-set wider
//! on 0x8A); two spark clouds (+0x740 / +0xFE0) and a frame quad on
//! 0x9B..0xBC and a third (+0x1880, lit on 0xBD) from 0x9B; from 0xAD a
//! full-window tint (+0x5B1C). The stage dims until 0xBD. Returns 1 on
//! update 0x1E, 6 on 0x28..0xBC, 2 on 0xBD, 8 after 0xF0.
//!
//! Library (record addresses): `colours4` 0x1E00080, the frame quad
//! 0x1E00120, `tint` 0x1E00240, `disc` 0x1E003A0, `facing::quad`
//! 0x1E00900, `particle_quads` 0x1E01190, `scatter` 0x1E01D20,
//! `trail::sparks` 0x1E01EE0, `fan` setup / draw 0x1E02C40 / 0x1E02DA0,
//! `streamer::push` / `draw` 0x1E033B0 / 0x1E03490.
use crate::b262::tint;
use crate::blib::{facing, spawn, streamer, trail};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const STREAMER: u32 = 0x01E0_3490;
const FAN_TEMPLATE: u32 = 0x01E0_5500;
const N: u32 = D + 0x5AF4;
const LEFT: u32 = D + 0x5AF0;
const NODES: u32 = D + 0x5B04;
const LIGHT: u32 = D + 0x5B20;

/// The entry 0x1E03E30 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xD0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xD0);
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
    fx.bzero(D, 0x5B30);
    let tenth = 0x3DCC_CCCDu32;
    for x in [0u32, 0x5930, 0x3F0, 0x4F0, 0x5F0, 0x660, 0x6D0, 0xF70, 0x1810] {
        fx.xform_init(D + x);
        if x == 0x3F0 || x == 0x4F0 {
            for o in [8u32, 4, 0] {
                fx.mem.write_u32(D + x + o, tenth);
            }
        }
    }
    for k in 0..4u32 {
        fx.xform_init(D + 0x20B0 + 0x70 * k);
    }
    for k in 0..8u32 {
        fx.xform_init(D + 0x3430 + 0x70 * k);
    }
    for (k, n) in [9, 0xA, 5, 8, 6, 7].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0xF0);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x5AF8, param);
    fx.mem.write_u8(LIGHT, 0x80);
    fx.mem.set32(D + 0x5AFC, 0x12);
    fx.mem.set32(D + 0x5B00, 0x12);
    bytes(fx, &[(0x5B1E, 0xC0), (0x5B1D, 0x80), (0x5B1C, 0xA0), (0x5B1F, 0)]);
    // Discs (+0x460, +0x560): radius 4.
    for g in [0x460u32, 0x560] {
        fx.mem.write_u32(D + g, 0);
        bytes(fx, &[(g + 6, 0xA0), (g + 5, 0x60), (g + 4, 0x80), (g + 7, 0)]);
        fx.mem.write_u32(D + g + 8, 0x4080_0000);
        halves(fx, &[(g + 0xC, 0x24), (g + 0xE, 0)]);
    }
    // Facing quads (+0x470 / +0x570, quads +0x4D0 / +0x5D0), texture 5 / 2.
    fx.texture5(sp + 0xC0, 2);
    for (g, q) in [(0x470u32, 0x4D0u32), (0x570, 0x5D0)] {
        fx.mem.write_u32(D + g + 0x38, 3);
        halves(fx, &[(g, 0), (g + 2, 0), (g + 4, 0x40), (g + 6, 0x40), (g + 8, 0), (g + 0xA, 0), (g + 0xC, 0x80), (g + 0xE, 0x80)]);
        fx.mem.write_u32(D + g + 0x30, 0x3F80_0000);
        bytes(fx, &[(g + 0x36, 0x80), (g + 0x35, 0x80), (g + 0x34, 0x80), (g + 0x37, 0)]);
        halves(fx, &[(g + 0x48, 0), (g + 0x4A, 2), (g + 0x4C, 3), (g + 0x4E, 4), (g + 0x50, 0), (g + 0x52, 0x3E7)]);
        let tex = fx.mem.read_u32(sp + 0xC0);
        for o in [0x18u32, 0x14, 0x10] {
            fx.mem.write_u32(D + q + o, 0);
        }
        fx.mem.write_u32(D + q + 8, 0x4040_0000);
        fx.mem.write_u32(D + q, tex);
        bytes(fx, &[(q + 0xE, 0x80), (q + 0xD, 0x80), (q + 0xC, 0x80), (q + 0xF, 0x80)]);
    }
    // Streamers (+0x70 / +0x230), texture 5 / 0.
    fx.texture5(sp + 0xC0, 0);
    for g in [0x70u32, 0x230] {
        let tex = fx.mem.read_u32(sp + 0xC0);
        fx.mem.write_u32(D + g, tex);
        bytes(fx, &[(g + 6, 0x80), (g + 5, 0x80), (g + 4, 0x80), (g + 7, 0x30)]);
        fx.mem.write_u32(D + g + 8, 0x8003);
        fx.mem.write_u32(D + g + 0xC, 0x41A0_0000);
        halves(fx, &[(g + 0x10, 0x9C), (g + 0x12, 0), (g + 0x14, 0x40), (g + 0x16, 0x40), (g + 0x18, 0x9C), (g + 0x1A, 0), (g + 0x1C, 0x100), (g + 0x1E, 0x80)]);
        fx.mem.write_u32(D + g + 0x30, 0x3D4C_CCCD);
        halves(fx, &[(g + 0x34, 0), (g + 0x36, 0x10), (g + 0x38, 0x18), (g + 0x3A, 0), (g + 0x3C, 1)]);
    }
    // Spark clouds (+0x740 / +0xFE0 / +0x1880), texture 5 / 1.
    fx.texture5(sp + 0xC0, 1);
    for (g, flags, a) in [(0x740u32, 0x2000_0003u32, 0x80u8), (0xFE0, 0x2000_0003, 0x80), (0x1880, 3, 0)] {
        let tex = fx.mem.read_u32(sp + 0xC0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, flags);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x80), (g + 8, 0x80), (g + 0xB, a)]);
        fx.mem.write_u32(D + g + 0x20, 0x3F00_0000);
        fx.mem.write_u32(D + g + 0x24, 0x40A0_0000);
        halves(fx, &[(g + 0x10, 0), (g + 0x12, 0), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
        fx.mem.write_u32(D + g + 0xC, D + g + 0x30);
        fx.mem.write_u32(D + g + 0x28, 0x40A0_0000);
        halves(fx, &[(g + 0x2C, 0x80)]);
        bytes(fx, &[(g + 0x2E, 8), (g + 0x2F, 0)]);
        spawn::scatter(fx, D + g, 0xC, 4.0);
    }
    // Four fans (+0x2270 + 0x470 k), texture 5 / 3.
    fx.texture5(sp + 0xC0, 3);
    for k in 0..4u32 {
        let g = 0x2270 + 0x470 * k;
        fx.mem.write_u32(D + g + 4, 1);
        halves(fx, &[(g + 0x10, 0x40), (g + 0x12, 0), (g + 0x14, 0x40), (g + 0x16, 0x40)]);
        bytes(fx, &[(g + 0xA, 0x40), (g + 9, 0x40), (g + 8, 0x80), (g + 0xB, 0x10)]);
        fx.mem.write_u32(D + g + 0x20, 0x3E00_0000);
        fx.mem.write_u32(D + g + 0x24, 0x3E80_0000);
        fx.mem.write_u32(D + g + 0x28, if k % 2 == 1 { 0x418C_0000 } else { 0x4170_0000 });
        fx.mem.write_u32(D + g + 0x2C, 0);
        let tex = fx.mem.read_u32(sp + 0xC0);
        fx.fan_setup(D + g, tex, 3.0, FAN_TEMPLATE);
    }
    // The particle cloud (+0x59A0), texture 4 / 0xF.
    fx.texture4(sp + 0xC0, 0xF);
    let tex = fx.mem.read_u32(sp + 0xC0);
    fx.mem.write_u32(D + 0x59A0, tex);
    fx.mem.write_u32(D + 0x59A4, 0x87);
    for o in [0x59B0u32, 0x59B4, 0x59B8, 0x59BC] {
        fx.mem.write_u32(D + o, 0);
    }
    halves(fx, &[(0x59C8, 0x80), (0x59CA, 0xC0), (0x59C4, 0x20), (0x59C6, 0x20), (0x59CC, 0x80), (0x59CE, 0x40)]);
    fx.mem.write_u32(D + 0x59AC, 0x4100_0000);
    halves(fx, &[(0x5AD0, 0x10), (0x5ADA, 3), (0x5ADC, 1), (0x5AE0, 8), (0x5AD6, 0x14), (0x5AD8, 0x14)]);
    bytes(fx, &[(0x59AA, 0x80), (0x59A9, 0x40), (0x59A8, 0x60), (0x59AB, 0x60)]);
    // rand's frame: s0 = the particle's offset base D + 0x10 k.
    for k in 0..0x10i32 {
        let p = D + 0x10 * k as u32;
        for (o, ra) in [(0u32, 0x01E0_4564u32), (4, 0x01E0_4598)] {
            let r = fx.rand_from(p, ra);
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(D + 0x59B0 + o), ee::from_i32(h - 0xA));
            fx.mem.setf(p + 0x59D0 + o, v);
        }
        let z = fx.mem.read_u32(D + 0x59B8);
        fx.mem.write_u32(p + 0x59D8, z);
        let a = fx.mem.i16(D + 0x5ADA).wrapping_neg();
        let f = fx.mult(a, k);
        fx.mem.set16(p + 0x59DE, f);
        fx.mem.set16(p + 0x59DC, 0);
        let c = fx.mem.i16(D + 0x5AE0) - 1;
        fx.mem.set16(D + 0x5ADE, c);
    }
    // Eight spark clouds (+0x37B0 + 0x30 k, particles +0x3930 + 0x400 k).
    for k in 0..8u32 {
        fx.mem.write_u32(D + 0x37BC + 0x30 * k, D + 0x3930 + 0x400 * k);
    }
    for k in 0..8u32 {
        let g = 0x37B0 + 0x30 * k;
        let tex = fx.mem.read_u32(sp + 0xC0);
        fx.mem.write_u32(D + g, tex);
        fx.mem.write_u32(D + g + 4, 3);
        bytes(fx, &[(g + 0xA, 0x80), (g + 9, 0x20), (g + 8, 0x20), (g + 0xB, 0x80)]);
        fx.mem.write_u32(D + g + 0x20, 0x3F00_0000);
        fx.mem.write_u32(D + g + 0x24, 0x40A0_0000);
        halves(fx, &[(g + 0x10, 0xB0), (g + 0x12, 0x60), (g + 0x14, 0x20), (g + 0x16, 0x20), (g + 0x18, 0), (g + 0x1A, 0), (g + 0x1C, 0x80), (g + 0x1E, 0x40)]);
        fx.mem.write_u32(D + g + 0x28, 0x40A0_0000);
        halves(fx, &[(g + 0x2C, 0x40)]);
        bytes(fx, &[(g + 0x2E, 1), (g + 0x2F, 0)]);
        spawn::scatter(fx, D + g, 0xC, 4.0);
    }
    0
}

/// Point `p` at node `k` until update 0x9B, then 1 / `steps` of the way to
/// `to` (counting down) with the streamer `g` (points `pts`) pushed.
fn mover(fx: &mut Fx, p: u32, k: u32, to: u32, steps: u32, g: u32, pts: u32) {
    if fx.mem.i32(N) < 0x9B {
        node(fx, k, p);
    } else {
        let m = fx.mem.i32(steps);
        fx.approach_by(p, to, m);
        if m != 0 {
            fx.mem.set32(steps, m - 1);
        }
        streamer::push(fx, g, pts, p);
    }
}

/// What 0x1038D0's 0x20C0B0 leaves at sp - 0xD0..: its `swc1 f20..f24`,
/// the entry's f20 (the runner's 0) [as b244 / b419 / b274].
fn floats_saved(fx: &mut Fx, sp: u32) {
    for k in 0..5u32 {
        fx.mem.write_u32(sp - 0xD0 + 4 * k, 0);
    }
}

/// A spark cloud `g` at the transform `x` (fading from 0xBE), each live
/// particle (from `base`) pushed out by 1.2 and respawned in a flat ring
/// at height 4 above 30 (rand's frame: the entry's s0 = the OT; `ras` the
/// conversion's and the three rands' return addresses) [b274's].
fn sparks(fx: &mut Fx, g: u32, x: u32, base: u32, ras: [u32; 4]) {
    trail::sparks(fx, OT, g, x);
    if fx.mem.i32(N) >= 0xBE {
        fx.step_down(g + 0xB, 0, 8);
    }
    let grow = f32::from_bits(0x3F99_999A);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(g + 0x2C) {
        let p = base + 0x10 * j;
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        for o in [0u32, 4, 8] {
            let v = ee::mul(fx.mem.f(p + o), grow);
            fx.mem.setf(p + o, v);
        }
        if ee_le(fx.mem.f(p + 4), 30.0) {
            continue;
        }
        let v = ee::mul(10.0, fx.mem.f(g + 0x28));
        fx.fix_residue(v, ras[0]);
        let spread = ee::fixsfsi(v);
        let a = fx.rand_from(OT, ras[1]).wrapping_rem(0x1_0000);
        let r = fx.rand_from(OT, ras[2]);
        let (_, h) = fx.div(r, spread);
        fx.mem.write_u32(p + 4, 0x4080_0000);
        let rad = ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h));
        let c = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(rad, c));
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(rad, s));
        let r = fx.rand_from(OT, ras[3]);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -h);
    }
}

/// A burst cloud `g` (particles from `*(g + 0xC)` or `base`) at `x`: pushed
/// out by 1.4, from life 9 thrown again in a random direction of length 1
/// (0x1043F0 into `dir`) with the cloud's alpha; `s0(j)` rand's saved s0.
fn burst(fx: &mut Fx, g: u32, x: u32, dir: u32, ra: u32, s0: &dyn Fn(u32) -> u32) {
    trail::sparks(fx, OT, g, x);
    if fx.mem.i32(N) >= 0xBE {
        fx.step_down(g + 0xB, 0, 8);
    }
    let grow = f32::from_bits(0x3FB3_3333);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(g + 0x2C) {
        let p = fx.mem.read_u32(g + 0xC) + 0x10 * j;
        let s = s0(j);
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        for o in [0u32, 4, 8] {
            let v = ee::mul(fx.mem.f(p + o), grow);
            fx.mem.setf(p + o, v);
        }
        if fx.mem.i16(p + 0xE) < 9 {
            continue;
        }
        fx.random_dir(dir, 1.0);
        for (o, s) in [(4u32, 4u32), (8, 8), (0, 0)] {
            let w = fx.mem.read_u32(dir + s);
            fx.mem.write_u32(p + o, w);
        }
        let r = fx.rand_from(s, ra);
        let (_, h) = fx.div(r, 0xC);
        fx.mem.set16(p + 0xE, -h);
        let a = fx.mem.read_u8(g + 0xB);
        fx.mem.write_u8(p + 0xC, a);
    }
}

/// 0x1E05330(D, OT): the eight spark clouds (+0x37B0 + 0x30 k at the
/// transforms +0x3430 + 0x70 k) as bursts (frame 0x90; rand's s0 = k).
fn clouds8(fx: &mut Fx) {
    let sp = fx.enter(0x90);
    for k in 0..8u32 {
        burst(fx, D + 0x37B0 + 0x30 * k, D + 0x3430 + 0x70 * k, sp + 0x80, 0x01E0_5458, &|_| k);
    }
    fx.leave(0x90);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    for out in [0x5950u32, 0x610, 0x680] {
        let t = fx.target_handle();
        fx.unit_position(t, D + out);
    }
    // The clouds' transforms about the target (rand's frame: s0 = the OT).
    for k in 0..8u32 {
        let x = D + 0x3430 + 0x70 * k;
        for (o, ra) in [(0u32, 0x01E0_4734u32), (4, 0x01E0_4768)] {
            let r = fx.rand_from(OT, ra);
            let (_, h) = fx.div(r, 0x14);
            let v = ee::add(fx.mem.f(D + 0x5950 + o), ee::from_i32(h - 0xA));
            fx.mem.setf(x + 0x20 + o, v);
        }
        let z = fx.mem.read_u32(D + 0x5958);
        fx.mem.write_u32(x + 0x28, z);
    }
    mover(fx, D + 0x410, 0, D + 0x610, D + 0x5AFC, D + 0x70, D + 0xB0);
    mover(fx, D + 0x510, 1, D + 0x680, D + 0x5B00, D + 0x230, D + 0x270);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x80);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0x410 + 4 * c);
        fx.mem.write_u32(D + 0x1830 + 4 * c, v);
    }
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0x1830 + 4 * c);
        fx.mem.write_u32(D + 0x6F0 + 4 * c, v);
    }
    fx.look_angles(sp + 0x80, D + 0x610, D + 0x6E0);
    let h = fx.self_handle();
    fx.unit_position(h, sp + 0x90);
    for c in 0..4u32 {
        let v = fx.mem.read_u32(D + 0x510 + 4 * c);
        fx.mem.write_u32(D + 0xF90 + 4 * c, v);
    }
    fx.look_angles(sp + 0x90, D + 0x680, D + 0xF80);
    for (k, out) in [(2u32, 0x20D0u32), (4, 0x2140), (3, 0x21B0), (5, 0x2220)] {
        node(fx, k, D + out);
    }
    // The fans face the attacker: from (x, y, the attacker's z).
    for k in 0..4u32 {
        let x = D + 0x20B0 + 0x70 * k;
        let (a, b, z) = (fx.mem.read_u32(x + 0x20), fx.mem.read_u32(x + 0x24), fx.mem.read_u32(sp + 0x98));
        fx.mem.write_u32(sp + 0xB0, a);
        fx.mem.write_u32(sp + 0xB4, b);
        fx.mem.write_u32(sp + 0xB8, z);
        fx.look_angles(sp + 0xB0, x + 0x20, x + 0x10);
    }
    // s0 = the OT, s1 / s3 the loops' (D + 0x70 k / k; D + 0x1C0 / 4 before).
    let regs = |s1: u32, s3: u32| [Some(OT), Some(s1), Some(D), Some(s3)];
    let r0 = regs(D + 0x1C0, 4);
    fx.xform_world_saving(D, 0x01E0_49B0, r0);
    floats_saved(fx, sp);
    for (x, ra) in [(0x5930u32, 0x01E0_49B8u32), (0x3F0, 0x01E0_49C0), (0x4F0, 0x01E0_49C8), (0x5F0, 0x01E0_49D0), (0x660, 0x01E0_49D8)] {
        fx.xform_billboard_saving(D + x, ra, r0);
    }
    for (x, ra) in [(0x6D0u32, 0x01E0_49E0u32), (0xF70, 0x01E0_49E8), (0x1810, 0x01E0_49F0)] {
        fx.xform_world_saving(D + x, ra, r0);
        floats_saved(fx, sp);
    }
    for k in 0..4u32 {
        fx.xform_world_saving(D + 0x20B0 + 0x70 * k, 0x01E0_4A00, regs(D + 0x70 * k, k));
        floats_saved(fx, sp);
    }
    for k in 0..8u32 {
        fx.xform_world_saving(D + 0x3430 + 0x70 * k, 0x01E0_4A24, regs(D + 0x70 * k, k));
        floats_saved(fx, sp);
    }
    if n(fx) < 0xBD {
        fx.step_down(LIGHT, 0x20, 4);
    } else {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0x28 {
        clouds8(fx);
        for o in [0xA8u32, 0xA4, 0xA0] {
            fx.mem.write_u32(sp + o, 0x3F80_0000);
        }
        fx.particle_quads(D + 0x5930, D + 0x59A0, sp + 0xA0);
        if n(fx) >= 0x9B {
            fx.step_down(D + 0x59AB, 0, 6);
        }
        let a = match n(fx).wrapping_rem(2) {
            0 => Some(0x10u8),
            1 => Some(8),
            _ => None,
        };
        if let Some(a) = a {
            for k in 0..4u32 {
                fx.mem.write_u8(D + 0x227B + 0x470 * k, a);
            }
        }
        for k in 0..4u32 {
            let g = D + 0x2270 + 0x470 * k;
            if n(fx) == 0x8A {
                fx.mem.write_u32(g + 0x28, if k % 2 == 1 { 0x4399_C000 } else { 0x4396_0000 });
                let tex = fx.mem.read_u32(g);
                fx.fan_setup(g, tex, 3.0, FAN_TEMPLATE);
            }
            fx.fan_draw(g, D + 0x20B0 + 0x70 * k);
            if n(fx) >= 0x9B {
                fx.step_down(g + 0xB, 0, 1);
            }
        }
    }
    if (0x9B..0xBD).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) >= 0xAD {
        tint(fx, D + 0x5B1C);
        if n(fx) < 0xBD {
            fx.step_up(D + 0x5B1F, 0xFF, 0x10);
        } else {
            fx.step_down(D + 0x5B1F, 0, 0x20);
        }
    }
    if n(fx) >= 0x4A {
        fx.disc(D + 0x460, D + 0x3F0);
        fx.disc(D + 0x560, D + 0x4F0);
        facing::quad(fx, OT, D + 0x470, D + 0x4D0, D + 0x3F0);
        facing::quad(fx, OT, D + 0x570, D + 0x5D0, D + 0x4F0);
        if n(fx) < 0x9B {
            for (x, disc, g) in [(0x3F0u32, 0x460u32, 0x470u32), (0x4F0, 0x560, 0x570)] {
                fx.approach(D + x, 1.0, f32::from_bits(0x3D4C_CCCD));
                fx.step_up(D + disc + 7, 0xFF, 0x20);
                fx.step_up(D + g + 0x37, 0x80, 0x10);
            }
        }
    }
    if (0x9B..0xBD).contains(&n(fx)) {
        sparks(fx, D + 0x740, D + 0x6D0, D + 0x770, [0x01E0_4E08, 0x01E0_4E10, 0x01E0_4E2C, 0x01E0_4E7C]);
        sparks(fx, D + 0xFE0, D + 0xF70, D + 0x1010, [0x01E0_4F5C, 0x01E0_4F64, 0x01E0_4F80, 0x01E0_4FD0]);
        for (s, pts, cnt) in [(0x70u32, 0xB0u32, 0xAAu32), (0x230, 0x270, 0x26A)] {
            streamer::draw(fx, STREAMER, OT, D + s, D + pts, D);
            let mut j = 0u32;
            while (j as i32) < fx.mem.i16(D + cnt) {
                fx.step_down(D + pts + 0xC + 0x10 * j, 0, 3);
                j += 1;
            }
        }
    }
    if n(fx) == 0xBD {
        fx.mem.write_u8(D + 0x188B, 0x80);
        let mut j = 0u32;
        while (j as i32) < fx.mem.i16(D + 0x18AC) {
            fx.mem.write_u8(D + 0x18BC + 0x10 * j, 0x80);
            j += 1;
        }
    }
    if n(fx) >= 0x9B {
        // rand's frame: s0 = the particle's offset base D + 0x10 j.
        burst(fx, D + 0x1880, D + 0x1810, sp + 0xA0, 0x01E0_51A0, &|j| D + 0x10 * j);
    }
    for (steps, x, disc, g, s) in [(0x5AFCu32, 0x3F0u32, 0x460u32, 0x470u32, 0x70u32), (0x5B00, 0x4F0, 0x560, 0x570, 0x230)] {
        if fx.mem.i32(D + steps) == 0 {
            fx.approach(D + x, 10.0, 1.0);
            fx.step_down(D + disc + 7, 0, 0x10);
            fx.step_down(D + g + 0x37, 0, 8);
            fx.step_down(D + s + 7, 0, 5);
        }
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0xBD {
        2
    } else if m == 0x1E {
        1
    } else if (0x28..0xBD).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
