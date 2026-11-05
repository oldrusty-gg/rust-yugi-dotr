//! BMODULE b540 (card 257), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E03150, data area 0x1ED0000) [F] (helper C9, 31
//! Oct 2026): two sprite fountains every update (local 0x1E03AF0): one
//! (+0x280, turning sprites) rising from the attacker's node 1 and thrown
//! out again on a ring above 100, one (+0x2350) rising to 20 at the node
//! and re-thrown; a swath (+0x70, `swath`) between the nodes 7 / 6 drawn
//! on 0x125..0x12E (filled on 0x124); a frame quad on 0xDC..0xFD; from
//! 0x126 a disc (+0x3880) and a quad group (+0x3420) swelling at the
//! target. The stage dims until 0xFE and brightens from 0x126. Returns 1
//! on update 0x53, 6 on 0x5D..0x125, 2 on 0x126, 8 after 0x140.
//!
//! Library at its own addresses: `colours4` (0x1E00080), the frame quad
//! (0x1E00120), `disc` (0x1E00240), `sprites` (0x1E007A0), `swath` push
//! / draw (0x1E02090 / 0x1E021E0), `quads_setup` / `quads_draw`
//! (0x1E029E0 / 0x1E02B40).
use crate::blib::swath;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const QUAD_TEMPLATE: u32 = 0x01E0_3F80;
const LEFT: u32 = D + 0x3890;
const N: u32 = D + 0x3894;
const NODES: u32 = D + 0x389C;
const LIGHT: u32 = D + 0x38AC;

/// The entry 0x1E03150.
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0xA0);
    // swc1 f20: the runner's (0) [H].
    fx.mem.write_u32(sp, 0);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0xA0);
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

/// rand % m (HI) with rand's frame s0 = `s0`, return address `ra`.
pub(crate) fn rmod(fx: &mut Fx, s0: u32, ra: u32, m: i32) -> i32 {
    let r = fx.rand_from(s0, ra);
    fx.div(r, m).1
}

/// A particle `p` (x +0, y +4, z +8) thrown out on a flat ring of radius
/// 0.1 (rand % (10 x `spread`)) at angle rand & 0xFFFF; `ras` the
/// conversion's and the two rands' return addresses; returns the angle
/// and the radius (the y and the life set by the caller in between).
pub(crate) fn ring_throw(fx: &mut Fx, s0: u32, spread: u32, ras: [u32; 3]) -> (i32, f32) {
    let v = ee::mul(10.0, fx.mem.f(spread));
    fx.fix_residue(v, ras[0]);
    let m = ee::fixsfsi(v);
    let a = fx.rand_from(s0, ras[1]).wrapping_rem(0x1_0000);
    let h = rmod(fx, s0, ras[2], m);
    (a, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)))
}

pub(crate) fn place(fx: &mut Fx, p: u32, a: i32, rad: f32) {
    let c = fx.tcos(a);
    fx.mem.setf(p + 8, ee::mul(rad, c));
    let s = fx.tsin(a);
    fx.mem.setf(p, ee::mul(rad, s));
}

fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x3C30);
    for x in [0u32, 0x1A0, 0x210, 0x22E0, 0x33B0] {
        fx.xform_init(D + x);
    }
    for (k, n) in [1, 7, 6].into_iter().enumerate() {
        let h = fx.self_handle();
        let r = fx.request_node(h, n, i32::from(k == 0));
        fx.mem.set32(NODES + 4 * k as u32, r);
    }
    fx.mem.set32(LEFT, 0x140);
    fx.mem.set32(N, 1);
    fx.mem.set32(D + 0x3898, param);
    let r = crate::b107::target_radius(fx);
    fx.mem.setf(D + 0x38A8, r);
    fx.mem.write_u8(LIGHT, 0x80);
    // The disc (+0x3880): radius 30.
    fx.mem.write_u32(D + 0x3880, 0x8_0000);
    bytes(fx, &[(0x3886, 0x80), (0x3885, 0x40), (0x3884, 0x40), (0x3887, 0xFF)]);
    fx.mem.write_u32(D + 0x3888, 0x41F0_0000);
    halves(fx, &[(0x388C, 0x24), (0x388E, 0)]);
    fx.texture4(sp + 0x90, 0xF);
    // The swath (+0x70, points +0x38B0 / +0x3A70) at nodes 7 / 6.
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(D + 0x70, tex);
    fx.mem.write_u32(D + 0x74, 1);
    halves(fx, &[(0x7C, 8), (0x180, 0x8E), (0x182, 0x54), (0x184, 0x20), (0x186, 1)]);
    bytes(fx, &[(0x7A, 0x80), (0x79, 0xA0), (0x78, 0xE0), (0x7B, 0xFF)]);
    fx.mem.write_u32(D + 0x190, D + 0x38B0);
    fx.mem.write_u32(D + 0x194, D + 0x3A70);
    node(fx, 1, sp + 0x70);
    node(fx, 2, sp + 0x80);
    swath_fill(fx, sp);
    // The quad group (+0x3420).
    fx.mem.write_u32(D + 0x3424, 0x8_0001);
    halves(fx, &[(0x3430, 0x8E), (0x3432, 0), (0x3434, 0x22), (0x3436, 0x40)]);
    bytes(fx, &[(0x342A, 0x80), (0x3429, 0x40), (0x3428, 0x40), (0x342B, 0x20)]);
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.quads_setup(D + 0x3420, tex, 3.0, QUAD_TEMPLATE);
    // Sprite fountain A (+0x280, particles +0x2E0, angles +0x12E0).
    fx.texture5(sp + 0x90, 0);
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(D + 0x280, tex);
    fx.mem.write_u32(D + 0x284, 0xD000_0003);
    bytes(fx, &[(0x28A, 0x80), (0x289, 0x40), (0x288, 0x40), (0x28B, 0)]);
    for (o, w) in [(0x28Cu32, 0x4000_0000u32), (0x2A0, 0), (0x2A4, 0x800), (0x2A8, 0)] {
        fx.mem.write_u32(D + o, w);
    }
    halves(fx, &[(0x2B0, 0), (0x2B2, 0), (0x2B4, 0x80), (0x2B6, 0x40), (0x2B8, 0), (0x2BA, 0), (0x2BC, 0x80), (0x2BE, 0x40)]);
    for (o, w) in [(0x2C0u32, D + 0x2E0), (0x2C4, D + 0x12E0), (0x2C8, 0), (0x2CC, 0x4120_0000), (0x2D4, 0x41C8_0000)] {
        fx.mem.write_u32(D + o, w);
    }
    halves(fx, &[(0x2D8, 0x400), (0x2DA, 0x100), (0x2DC, 1), (0x2DE, 0)]);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x2DA) {
        let p = D + 0x2E0 + 0x10 * j;
        let (a, rad) = ring_throw(fx, D, D + 0x2CC, [0x01E0_346C, 0x01E0_3474, 0x01E0_3490]);
        let h = rmod(fx, D, 0x01E0_34B8, 0x3E8);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
        place(fx, p, a, rad);
        let h = rmod(fx, D, 0x01E0_3508, 0x14);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0);
        fx.mem.write_u8(p + 0xD, 0);
        for (o, ra) in [(0u32, 0x01E0_3530u32), (4, 0x01E0_354C), (8, 0x01E0_3568)] {
            let r = fx.rand_from(D, ra).wrapping_rem(0x1_0000);
            fx.mem.set32(p + 0x1000 + o, r);
        }
        j += 1;
    }
    // Sprite fountain B (+0x2350, particles +0x23B0, angles +0x2BB0).
    fx.texture5(sp + 0x90, 1);
    let tex = fx.mem.read_u32(sp + 0x90);
    fx.mem.write_u32(D + 0x2350, tex);
    fx.mem.write_u32(D + 0x2354, 0x7800_0003);
    bytes(fx, &[(0x235A, 0x80), (0x2359, 0x40), (0x2358, 0x40), (0x235B, 0)]);
    for (o, w) in [(0x235Cu32, 0x41A0_0000u32), (0x2370, 0), (0x2374, 0), (0x2378, 0)] {
        fx.mem.write_u32(D + o, w);
    }
    halves(fx, &[(0x2380, 0), (0x2382, 0), (0x2384, 0x20), (0x2386, 0x20), (0x2388, 0), (0x238A, 0), (0x238C, 0x80), (0x238E, 0x40)]);
    for (o, w) in [(0x2390u32, D + 0x23B0), (0x2394, D + 0x2BB0), (0x2398, 0), (0x239C, 0x4120_0000), (0x23A4, 0), (0x23A0, 0x3F80_0000)] {
        fx.mem.write_u32(D + o, w);
    }
    halves(fx, &[(0x23A8, 0), (0x23AA, 0x80), (0x23AC, 8), (0x23AE, 0)]);
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x23AA) {
        let p = D + 0x23B0 + 0x10 * j;
        let (a, rad) = ring_throw(fx, D, D + 0x239C, [0x01E0_3664, 0x01E0_366C, 0x01E0_3688]);
        let h = rmod(fx, D, 0x01E0_36B0, 0x28);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::neg(ee::from_i32(h - 0x14))));
        place(fx, p, a, rad);
        let h = rmod(fx, D, 0x01E0_3708, 0xC);
        fx.mem.set16(p + 0xE, -h);
        fx.mem.write_u8(p + 0xC, 0);
        let r = fx.rand_from(D, 0x01E0_372C).wrapping_rem(8);
        fx.mem.write_u8(p + 0xD, r as u8);
        for (o, ra) in [(0u32, 0x01E0_3748u32), (4, 0x01E0_3764)] {
            let r = fx.rand_from(D, ra).wrapping_rem(0x1_0000);
            fx.mem.set32(p + 0x800 + o, r);
        }
        j += 1;
    }
    0
}

/// The swath's 8 point pairs at sp + 0x70 / sp + 0x80.
fn swath_fill(fx: &mut Fx, sp: u32) {
    for k in 0..8u32 {
        for c in 0..3u32 {
            let a = fx.mem.read_u32(sp + 0x70 + 4 * c);
            fx.mem.write_u32(D + 0x80 + 0x10 * k + 4 * c, a);
            let b = fx.mem.read_u32(sp + 0x80 + 4 * c);
            fx.mem.write_u32(D + 0x100 + 0x10 * k + 4 * c, b);
        }
    }
    fx.mem.set16(D + 0x7E, 0);
}

/// 0x1E03AF0(D, OT): the two fountains drawn and stepped (frame 0x80;
/// rand's s0: the loop counter for A, the particle's offset base for B).
fn fountains(fx: &mut Fx) {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let sp = fx.enter(0x80);
    // swc1 f20: the entry's f20 (the runner's 0) [H].
    fx.mem.write_u32(sp, 0);
    if n(fx) == 0x34 {
        fx.mem.write_u8(D + 0x28B, 0x80);
    }
    fx.sprites(D + 0x280, D + 0x210);
    if n(fx) >= 0xFE {
        fx.step_down(D + 0x28B, 0, 4);
    }
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x2DA) {
        let p = D + 0x2E0 + 0x10 * j;
        let k = j;
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let up = if n(fx).wrapping_rem(2) == 0 { 3.0 } else { 2.0 };
        let y = ee::add(fx.mem.f(p + 4), up);
        fx.mem.setf(p + 4, y);
        let s = if ee_lt(fx.mem.f(p + 4), 0.0) { f32::from_bits(0x3F4C_CCCD) } else { f32::from_bits(0x3F8B_851F) };
        for o in [8u32, 0] {
            let v = ee::mul(fx.mem.f(p + o), s);
            fx.mem.setf(p + o, v);
        }
        for o in [0x1000u32, 0x1004] {
            let w = fx.mem.read_u32(p + o).wrapping_add(0x100);
            fx.mem.write_u32(p + o, w);
        }
        if ee_lt(fx.mem.f(p + 4), 100.0) {
            continue;
        }
        let (a, rad) = ring_throw(fx, k, D + 0x2CC, [0x01E0_3C88, 0x01E0_3C90, 0x01E0_3CAC]);
        let h = rmod(fx, k, 0x01E0_3CD4, 0x14);
        fx.mem.set16(p + 0xE, -h);
        place(fx, p, a, rad);
        let h = rmod(fx, k, 0x01E0_3D10, 0x64);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(h)));
        let alpha = if n(fx) < 0x34 { 0 } else { 0x80 };
        fx.mem.write_u8(p + 0xC, alpha);
    }
    if n(fx) == 0x34 {
        fx.mem.write_u8(D + 0x235B, 0x80);
    }
    fx.sprites(D + 0x2350, D + 0x22E0);
    if n(fx) >= 0xFE {
        fx.step_down(D + 0x235B, 0, 4);
    }
    let mut j = 0u32;
    while (j as i32) < fx.mem.i16(D + 0x23AA) {
        let p = D + 0x23B0 + 0x10 * j;
        let s0 = D + 0x10 * j;
        j += 1;
        if fx.mem.i16(p + 0xE) < 0 {
            continue;
        }
        let y = ee::add(fx.mem.f(p + 4), 2.0);
        fx.mem.setf(p + 4, y);
        let g = f32::from_bits(0x3F8C_CCCD);
        for o in [0u32, 8] {
            let v = ee::mul(fx.mem.f(p + o), g);
            fx.mem.setf(p + o, v);
        }
        fx.step_down(p + 0xC, 0, 8);
        if ee_le(fx.mem.f(p + 4), 20.0) {
            continue;
        }
        let (a, rad) = ring_throw(fx, s0, D + 0x239C, [0x01E0_3E54, 0x01E0_3E5C, 0x01E0_3E78]);
        let h = rmod(fx, s0, 0x01E0_3EA0, 0x28);
        fx.mem.setf(p + 4, ee::mul(f32::from_bits(0x3DCC_CCCD), ee::neg(ee::from_i32(h - 0x14))));
        place(fx, p, a, rad);
        let h = rmod(fx, s0, 0x01E0_3EF8, 0xC);
        fx.mem.set16(p + 0xE, -h);
        let alpha = if n(fx) < 0x34 { 0 } else { 0x80 };
        fx.mem.write_u8(p + 0xC, alpha);
    }
    fx.leave(0x80);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let n = |fx: &mut Fx| fx.mem.i32(N);
    let t = fx.target_handle();
    fx.unit_position(t, D + 0x33D0);
    if n(fx) < 0x35 {
        node(fx, 0, D + 0x230);
        let t = fx.target_handle();
        fx.unit_position(t, sp + 0x70);
        node(fx, 0, sp + 0x80);
        fx.look_angles(sp + 0x70, sp + 0x80, D + 0x220);
    }
    node(fx, 0, D + 0x2300);
    let t = fx.target_handle();
    fx.unit_position(t, sp + 0x70);
    node(fx, 0, sp + 0x80);
    fx.look_angles(sp + 0x70, sp + 0x80, D + 0x22F0);
    node(fx, 1, sp + 0x70);
    node(fx, 2, sp + 0x80);
    if n(fx) == 0x124 {
        swath_fill(fx, sp);
    } else {
        swath::push(fx, D + 0x70, sp + 0x70, sp + 0x80);
    }
    // s0 = D, s1 = the OT, s2 = sp + 0x80, s3 the runner's 0.
    let regs = [Some(D), Some(OT), Some(sp + 0x80), Some(0)];
    for (x, ra, board) in [(0u32, 0x01E0_391Cu32, false), (0x1A0, 0x01E0_3924, true), (0x210, 0x01E0_392C, false), (0x22E0, 0x01E0_3934, false), (0x33B0, 0x01E0_393C, true)] {
        if board {
            fx.xform_billboard_saving(D + x, ra, regs);
        } else {
            fx.xform_world_saving(D + x, ra, regs);
        }
    }
    if (0xDC..0xFE).contains(&n(fx)) {
        fx.frame_quad_rgba([0x80, 0x80, 0x80, 0x60]);
    }
    if n(fx) < 0xFE {
        fx.step_down(LIGHT, 0x20, 4);
    } else if n(fx) >= 0x126 {
        fx.step_up(LIGHT, 0x80, 4);
    }
    let l = fx.mem.read_u8(LIGHT);
    fx.set_stage_light(i32::from(l));
    if n(fx) >= 0x126 {
        fx.disc(D + 0x3880, D + 0x33B0);
        fx.step_down(D + 0x3887, 0, 0x10);
        fx.quads_draw(D + 0x3420, D + 0x33B0);
        // 0x1E02B40's prologue saves (s0..s7, fp, ra; the entry's s0 = D,
        // s1 = the OT, s2 = sp + 0x80, the rest the runner's 0) [run: the
        // upper half of its s7 slot and its fp slot are the fountains'
        // missing-texture TEX0 / TEX1 at entry sp - 0x128 / - 0x120].
        let regs = [(0u32, u64::from(D)), (0x10, u64::from(OT)), (0x20, u64::from(sp + 0x80)), (0x30, 0), (0x40, 0), (0x50, 0), (0x60, 0), (0x70, 0), (0x80, 0), (0x90, 0x01E0_39F0)];
        fx.saved(sp - 0x1A0, &regs);
        fx.step_down(D + 0x342B, 0, 2);
        fx.approach(D + 0x33B0, 2.0, f32::from_bits(0x3DCC_CCCD));
    }
    fountains(fx);
    if (0x125..0x12F).contains(&n(fx)) {
        swath::draw(fx, D + 0x70, D);
    }
    let m = n(fx) + 1;
    fx.mem.set32(N, m);
    let left = fx.mem.i32(LEFT) - 1;
    fx.mem.set32(LEFT, left);
    if m == 0x126 {
        2
    } else if m == 0x53 {
        1
    } else if (0x5D..0x126).contains(&m) {
        6
    } else if left > 0 {
        0
    } else {
        8
    }
}
