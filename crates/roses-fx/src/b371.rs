//! BMODULE b371 (card 99), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x6830
//! bytes) [F]: a single unscheduled function in
//! a 0x3E0 frame (s1 = D, s0 = the OT, the return at sp + 0xC0). Every
//! update the camera turn with roll (sp + 0x1E0) and the attacker's node
//! 4 (D+0x30) lowered 3 and pushed 8 x the side; the target's position at
//! D+0x40. On 0x29..0xE7 a record an update (12 round robin, +0x50, 0x60)
//! of 4 sprites (1.5 round the node, 12 updates, sizes 15 (0.5 + 0.5 age /
//! 12), spread 0.1 + 0.9 age / 12, alpha 64 cos((age << 14) / 12) x
//! clamp(0xE8 - n, 0, 32) / 32, white, ALPHA 0, texture D+0x1C) and one
//! (32 round robin, +0x30B0, 0x170) of 16 spinning sprites (4.5 round the
//! node, spins rand x 0x8AE >> 8, 32 updates, sizes 17 (0.5 + 0.5 age /
//! 32), pulled 2.7 age / 32 to the target, alpha 128 ratio_below(age, 3) x
//! ramp_above(age, 32, 6) x clamp(0xE8 - n, 0, 8) / 8). From update 8 for
//! 0x108 updates two tubes (+0x4D0: 33 rings of 16 + 1 points, radius 220
//! + 60 pass + 30 sin(row phase + pass phase + 0x8AE age), z -3000..3000,
//! 50 up) drawn as strips (prim 0x24, ALPHA 0, texture D+0x14) scrolled
//! in s / t by the age, alphas 32 / 48 x ratio_below(age, 16) x
//! ramp_above(age, 264, 232) sin(32768 k / 32 >> 5), (69, 69, 69), key
//! 0xFFFFFF. While clamp(n - 8, 0, 32) x clamp(0x110 - n, 0, 32) > 0 a
//! full screen flat sprite of the frame (0x207EF0) jittered by rand % 21 -
//! 10 texels, alpha 70 x that / 1024, (AF, AF, AF). On 1..0x15 a burst (8
//! round robin, +0x5EB0, 0x130) of 16 points 3.0 round the node (z the
//! away side), 8 updates, each drawn twice (texture D+0x10 ALPHA 1 (FF,
//! E8, E0) size 3, then D+0x1C ALPHA 0 white size 15, the cell + 4) at the
//! node + (point - node) x (0.1 + 0.9 / 0.7 (8 - age) / 8), alpha 128
//! sin((age << 14) >> 3). The node is lowered 5 more after the spinning
//! sprites; the stage light 0x80 - clamp(0x118 - n) clamp(n) 128 / 64.
//! Returns 1 on 0x2D, 6 on 0x38..0xE7, 2 on 0xE8, 8 once the count
//! reaches 0x119.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::far_xy;

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x3E0;
/// The sprite descriptor.
const DESC: u32 = 0x308;
/// The tube strips' descriptor (prim 0x24, 9 registers).
const STRIP: u32 = 0x360;
/// The entry's s0 (0x206E40's return), rand's saved s0.
const S0: u32 = OT;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

/// The compiler's signed remainder by a power of two.
fn rem2(x: i32, m: i32) -> i32 {
    let r = x & (m - 1);
    if x < 0 && r != 0 {
        r - m
    } else {
        r
    }
}

fn words(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x6830);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..4u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 4, 1);
    fx.mem.set32(D + 0x20, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x40);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x50 + 0x60 * k, 0);
    }
    for row in 0..0x21u32 {
        for j in 0..0x11u32 {
            // rand's saved s0 is the column counter.
            let r = fx.rand_from(j, 0x01E0_01A8) >> 23;
            fx.mem.set32(D + 0x27E0 + 0x44 * row + 4 * j, (rem2(r, 256) << 16) >> 8);
        }
    }
    fx.mem.write_u8(D + 0x30A4, 0);
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x30B0 + 0x170 * k, 0);
    }
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x5EB0 + 0x130 * k, 0);
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's two UVs (sp + 0x320, the pointer sp + 0x130 (0x322),
/// sp + 0x330, the pointer sp + 0x140 (0x332)).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x320, u0);
    let p = fx.mem.read_u32(sp + 0x130);
    uv(fx, p, v0);
    uv(fx, sp + 0x330, u1);
    let p = fx.mem.read_u32(sp + 0x140);
    uv(fx, p, v1);
}

/// A 4 x 8 cell `b` of the texture.
fn cell_uvs(fx: &mut Fx, sp: u32, b: u32) {
    let (c, r) = (b & 3, b >> 2);
    uvs(fx, sp, (c << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The view-projection (x identity) at sp + 0x160.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    let id = fx.read_m4(sp + 0x1A0);
    fx.mul_into(Some(sp + 0x160), &id);
}

/// The sprite descriptor rewritten through its pointers (sp + 0x100 /
/// 0x110 / 0x120), | 0x800.
fn desc_reset(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    let p = fx.mem.read_u32(sp + 0x100);
    fx.mem.write_u16(p, 0x26);
    fx.mem.write_u16(d, 5);
    let p = fx.mem.read_u32(sp + 0x110);
    fx.mem.write_u64(p, 0x4_2421);
    let p = fx.mem.read_u32(sp + 0x120);
    fx.mem.write_u32(p, 0);
    let p = fx.mem.read_u32(sp + 0x100);
    let f = fx.mem.read_u16(p);
    fx.mem.write_u16(p, f | 0x800);
}

/// The corners (s, s, 0, 1) at sp + 0x220 and (-s, -s, 0, 1) at sp +
/// 0x230 (`neg_first`: the other way round) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, s: f32, neg_first: bool) {
    let n = ee::neg(s);
    let (a, b) = if neg_first { (n, s) } else { (s, n) };
    fx.mem.setf(sp + 0x220, a);
    fx.mem.setf(sp + 0x224, a);
    fx.mem.write_u32(sp + 0x228, 0);
    fx.mem.write_u32(sp + 0x22C, 0x3F80_0000);
    fx.mem.setf(sp + 0x230, b);
    fx.mem.setf(sp + 0x234, b);
    fx.mem.write_u32(sp + 0x238, 0);
    fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x220, sp + 0x1E0, sp + 0x220);
    fx.vu_apply_to(sp + 0x230, sp + 0x1E0, sp + 0x230);
}

/// The colour (c, a, Q = 1.0) at `p`.
fn colour(fx: &mut Fx, p: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

/// The vertices = the corners + sp + 0x2E0.
fn verts(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + 0x260, sp + 0x220, sp + 0x2E0);
    fx.vu_add4(sp + 0x270, sp + 0x230, sp + 0x2E0);
}

/// Projected and drawn at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + 0x260, PROJ_OUT, sp + 0x160, sp + DESC, sp + 0x3DC) {
        let w = fx.mem.f(sp + 0x3DC);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// x / 12 as compiled (the multiply by 0x2AAAAAAB).
fn div12(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 1).wrapping_add(((x as u32) >> 31) as i32)
}

/// The 4-sprite records [F 0x1E00424].
fn sparks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0xE8 - n, 0, 0x20)), 32.0);
    fx.mem.write_u32(sp + 0x130, sp + 0x322);
    fx.mem.write_u32(sp + 0x140, sp + 0x332);
    for i in 0..0xCu32 {
        let r = D + 0x50 + 0x60 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let q = div12(fx, age << 14);
        let c = fx.tcos(q);
        let a = fix(fx, ee::mul(f20, ee::mul(64.0, c)), 0x01E0_0478);
        colour(fx, sp + 0x318, [0xFF; 3], a as u8);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::mul(15.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 12.0)));
        corners(fx, sp, s, false);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f21 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 12.0));
        for j in 0..4u32 {
            let b = u32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            fx.vu_scale4(sp + 0x2E0, r + 0x20 + 0x10 * j, f21);
            fx.vu_add4(sp + 0x2E0, sp + 0x2E0, r + 0x10);
            verts(fx, sp);
            draw(fx, sp, 0x01E0_0680);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The two tubes [F 0x1E00704].
fn tubes(fx: &mut Fx, sp: u32) {
    let d = sp + STRIP;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let age = ee::from_i32(fx.mem.i32(D + 0x30A8));
    let f20 = Fx::ratio_below(age, 16.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x30A8));
    let f0 = Fx::ramp_above(age, 264.0, 232.0);
    let f2 = ee::mul(f20, f0);
    let f22 = ee::mul(32.0, f2);
    let f23 = ee::mul(48.0, f2);
    fx.mem.write_u32(sp + 0xD0, 0);
    fx.mem.write_u32(sp + 0xF0, 0);
    let (vp, m, w) = (sp + 0x160, sp + 0x1A0, sp + 0x3DC);
    for p in 0..2i32 {
        fx.camera_vp_to(vp);
        fx.identity(m);
        fx.mem.write_u32(sp + 0x2E0, 0);
        fx.mem.setf(sp + 0x2E4, 50.0);
        fx.mem.write_u32(sp + 0x2E8, 0);
        fx.mem.write_u32(sp + 0x2EC, 0x3F80_0000);
        fx.translate_rows(m, m, sp + 0x2E0);
        let mm = fx.read_m4(m);
        fx.mul_into(Some(vp), &mm);
        let f2 = ee::mul(30.0, ee::from_i32(p));
        let phase = fx.mem.i32(sp + 0xF0) >> 1;
        let f20 = ee::add(220.0, ee::mul(2.0, f2));
        fx.mem.write_u32(sp + 0x150, 0);
        fx.mem.write_u32(sp + 0xE0, D);
        for k in 0..0x21u32 {
            let f21 = ee::add(f32::from_bits(0xC53B_8000), ee::div(ee::mul(f32::from_bits(0x45BB_8000), ee::from_i32(k as i32)), 32.0));
            let row = D + 0x44 * k;
            let s7 = D + 0x110 * k;
            for j in 0..0x10u32 {
                let age = fx.mem.i32(D + 0x30A8);
                let t = fx.mem.i32(row + 0x27E0 + 4 * j);
                let mm = fx.mult(age, 0x8AE);
                let a = t.wrapping_add(phase).wrapping_add(mm) & 0xFFFF;
                let sn = fx.tsin(a);
                let f24 = ee::add(f20, ee::mul(30.0, sn));
                let ang = (j << 12) as i32;
                let pt = s7 + 0x4D0 + 0x10 * j;
                let sn = fx.tsin(ang);
                fx.mem.setf(pt, ee::mul(f24, sn));
                let cs = fx.tcos(ang);
                fx.mem.setf(pt + 4, ee::mul(f24, cs));
                fx.mem.setf(pt + 8, f21);
                fx.mem.write_u32(pt + 0xC, 0x3F80_0000);
            }
            fx.mem.write_u32(sp + 0xE0, row + 0x44);
            words(fx, s7 + 0x5D0, s7 + 0x4D0);
            fx.mem.write_u32(sp + 0x150, k + 1);
        }
        let age = fx.mem.i32(D + 0x30A8);
        let f1 = ee::div(ee::from_i32(rem2(age, 0x20)), 32.0);
        let sign = rem2(p, 2) * 2 - 1;
        let f25 = ee::mul(f1, ee::from_i32(sign));
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let f24 = ee::mul(ee::div(ee::mul(2.0, ee::from_i32(rem2(age, 0x10))), 16.0), side);
        for k in 0..0x20i32 {
            let v = rem2(k, 2);
            let f20 = ee::div(ee::from_i32(v), 2.0);
            let f21 = ee::div(ee::from_i32(v + 1), 2.0);
            let sn = fx.tsin((k << 15) >> 5);
            let a = if p == 0 { fix(fx, ee::mul(f22, sn), 0x01E0_0A88) } else { fix(fx, ee::mul(f23, sn), 0x01E0_0A9C) };
            let f20 = ee::add(f20, f24);
            colour(fx, sp + 0x370, [0x69; 3], a as u8);
            let f21 = ee::add(f21, f24);
            let r0 = D + 0x110 * k as u32 + 0x4D0;
            let r1 = D + 0x110 * (k + 1) as u32 + 0x4D0;
            for j in 0..0x10i32 {
                let v = rem2(j, 2);
                let f0 = ee::div(ee::from_i32(v + 1), 2.0);
                let f2 = ee::div(ee::from_i32(v), 2.0);
                let s0 = ee::add(f2, f25);
                fx.mem.setf(sp + 0x378, s0);
                let s1 = ee::add(f0, f25);
                fx.mem.setf(sp + 0x37C, f20);
                fx.mem.setf(sp + 0x388, s1);
                fx.mem.setf(sp + 0x38C, f20);
                fx.mem.setf(sp + 0x398, s0);
                fx.mem.setf(sp + 0x39C, f21);
                fx.mem.setf(sp + 0x3A8, s1);
                fx.mem.setf(sp + 0x3AC, f21);
                let (o0, o1) = (0x10 * j as u32, 0x10 * (j + 1) as u32);
                let r = fx.project_n(&[r0 + o0, r0 + o1, r1 + o0, r1 + o1], vp, d, w, 0x01E0_0B7C);
                if r == 1 {
                    fx.submit(d, OT, 0x00FF_FFFF);
                }
            }
        }
        let ph = fx.mem.i32(sp + 0xF0).wrapping_add(0x3_3333);
        fx.mem.set32(sp + 0xF0, ph);
        fx.mem.set32(sp + 0xD0, p + 1);
    }
    let age = fx.mem.i32(D + 0x30A8) + 1;
    fx.mem.set32(D + 0x30A8, age);
    if age >= 0x109 {
        fx.mem.write_u8(D + 0x30A4, 0);
    }
}

/// The frame drawn full screen, jittered [F 0x1E00C8C].
fn frame_flash(fx: &mut Fx, sp: u32, f20: f32) {
    let d = sp + DESC;
    desc_reset(fx, sp);
    let a = fx.alpha_preset(0);
    let flags = u32::from(fx.mem.read_u16(d + 2));
    fx.desc_alpha(d, a);
    fx.frame_desc(d, 1, [flags | 0x200, flags]);
    fx.desc_clamp(d, 5);
    let r = fx.rand_from(S0, 0x01E0_0D04) >> 23;
    let (_, m) = fx.div(r, 0x15);
    let s2 = m - 10;
    let r = fx.rand_from(S0, 0x01E0_0D24) >> 23;
    let (_, m) = fx.div(r, 0x15);
    let t0 = m - 10;
    let u = |x: i32| ((x as u32) << 4) & 0x3FFF;
    uvs(fx, sp, u(s2 + 0x14), u(t0 + 0x14), u(s2 + 0x1EC), u(t0 + 0x1EC));
    let a = fix(fx, ee::mul(70.0, f20), 0x01E0_0DDC);
    colour(fx, sp + 0x318, [0xAF; 3], a as u8);
    far_xy(fx, sp + 0x328, 0x7000, 0x7000);
    far_xy(fx, sp + 0x338, 0x9000, 0x9000);
    fx.submit(d, OT, 0);
}

/// The spinning sprites [F 0x1E0103C].
fn spinners(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0xE8 - n, 0, 8)), 8.0);
    for i in 0..0x20u32 {
        let r = D + 0x30B0 + 0x170 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f21 = ee::mul(128.0, Fx::ratio_below(age, 3.0));
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f0 = Fx::ramp_above(age, 32.0, 6.0);
        let a = fix(fx, ee::mul(f20, ee::mul(f21, f0)), 0x01E0_1090);
        colour(fx, sp + 0x318, [0xFF; 3], a as u8);
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let s = ee::mul(17.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
        corners(fx, sp, s, false);
        let f4 = ee::from_i32(fx.mem.i32(r + 4));
        let f2 = ee::div(ee::mul(f32::from_bits(0x3F66_6666), f4), 32.0);
        let f21 = ee::div(ee::mul(f32::from_bits(0x402C_CCCD), f4), 32.0);
        let f22 = ee::add(f32::from_bits(0x3DCC_CCCD), f2);
        let pv = sp + 0x2F0;
        fx.vu_sub4(pv, D + 0x40, r + 0x60);
        fx.vu_scale4(pv, pv, f21);
        fx.vu_add4(pv, pv, r + 0x60);
        for j in 0..0x10u32 {
            let b = u32::from(fx.mem.read_u8(r + 8 + j));
            cell_uvs(fx, sp, b);
            let spin = fx.mem.i32(r + 0x18 + 4 * j);
            let age = fx.mem.i32(r + 4);
            let ang = fx.mult(spin, age) & 0xFFFF;
            fx.identity(sp + 0x2A0);
            fx.rotate_axis(sp + 0x2A0, ang, 2);
            fx.vu_scale4(sp + 0x2E0, r + 0x70 + 0x10 * j, f22);
            fx.vu_apply_to(sp + 0x2E0, sp + 0x2A0, sp + 0x2E0);
            fx.vu_add4(sp + 0x2E0, sp + 0x2E0, pv);
            verts(fx, sp);
            draw(fx, sp, 0x01E0_1328);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// One pass of a burst's 16 points: (point - node) x `k` + node.
#[allow(clippy::too_many_arguments)]
fn burst_pass(fx: &mut Fx, sp: u32, r: u32, rh_at: u32, preset: i32, size: f32, c: [u8; 3], spread: f32, cell_add: u8, ras: [u32; 2]) {
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(sp + DESC, rh, 1);
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(sp + DESC, a);
    let age = fx.mem.i32(r + 4);
    let f = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(8 - age)), 8.0);
    let s = ee::mul(size, ee::add(f32::from_bits(0x3E4C_CCCD), f));
    corners(fx, sp, s, true);
    let age = fx.mem.i32(r + 4);
    let sn = fx.tsin((age << 14) >> 3);
    let a = fix(fx, ee::mul(128.0, sn), ras[0]);
    colour(fx, sp + 0x318, c, a as u8);
    let age = fx.mem.i32(r + 4);
    let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(spread, ee::from_i32(8 - age)), 8.0));
    for j in 0..0x10u32 {
        let pt = sp + 0x2E0;
        fx.vu_sub4(pt, r + 0x30 + 0x10 * j, D + 0x30);
        fx.vu_scale4(pt, pt, f20);
        fx.vu_add4(pt, pt, D + 0x30);
        verts(fx, sp);
        let b = u32::from(fx.mem.read_u8(r + 8 + j).wrapping_add(cell_add));
        cell_uvs(fx, sp, b);
        draw(fx, sp, ras[1]);
    }
}

/// The bursts [F 0x1E014DC].
fn bursts(fx: &mut Fx, sp: u32) {
    for i in 0..8u32 {
        let r = D + 0x5EB0 + 0x130 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        burst_pass(fx, sp, r, 0x10, 1, 3.0, [0xFF, 0xE8, 0xE0], f32::from_bits(0x3F66_6666), 0, [0x01E0_15E4, 0x01E0_17A0]);
        burst_pass(fx, sp, r, 0x1C, 0, 15.0, [0xFF; 3], f32::from_bits(0x3F33_3333), 4, [0x01E0_18C0, 0x01E0_1A6C]);
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 9 {
            fx.mem.write_u8(r, 0);
        }
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x1E0);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20));
    fx.node_position(h, slot, D + 0x30);
    let y = fx.mem.f(D + 0x34);
    fx.mem.setf(D + 0x34, ee::add(y, -3.0));
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = fx.mem.f(D + 0x38);
    fx.mem.setf(D + 0x38, ee::add(z, ee::mul(-8.0, side)));
    let n = fx.mem.i32(D);
    if (0x29..0xE8).contains(&n) {
        let r = D + 0x50 + 0x60 * (n % 0xC) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x10, D + 0x30);
        for j in 0..4u32 {
            let x = fx.rand_from(S0, 0x01E0_0304) >> 23;
            fx.mem.set8(r + 8 + j, rem2(x, 8));
            fx.random_table_vec(r + 0x20 + 0x10 * j, 1.5);
        }
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x100, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x110, d + 8);
    fx.mem.write_u32(sp + 0x120, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    sparks(fx, sp);
    let n = fx.mem.i32(D);
    if n == 8 {
        fx.mem.write_u8(D + 0x30A4, 1);
        fx.mem.write_u32(D + 0x30A8, 0);
    }
    if fx.mem.read_u8(D + 0x30A4) == 1 {
        tubes(fx, sp);
    }
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 8, 0, 0x20));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x110 - n, 0, 0x20));
    let f20 = ee::div(ee::mul(a, b), 1024.0);
    if f20 > 0.0 {
        frame_flash(fx, sp, f20);
    }
    let n = fx.mem.i32(D);
    if (0x29..0xE8).contains(&n) {
        let r = D + 0x30B0 + 0x170 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x60, D + 0x30);
        for j in 0..0x10u32 {
            let x = fx.rand_from(S0, 0x01E0_0F04) >> 23;
            let c = rem2(x, 8);
            let y = fx.rand_from(S0, 0x01E0_0F24) >> 23;
            fx.mem.set8(r + 8 + j, c);
            let m = fx.mult(rem2(y, 256), 0x8AE);
            fx.mem.set32(r + 0x18 + 4 * j, m >> 8);
            fx.random_table_vec(r + 0x70 + 0x10 * j, 4.5);
        }
    }
    desc_reset(fx, sp);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    spinners(fx, sp);
    let y = fx.mem.f(D + 0x34);
    fx.mem.setf(D + 0x34, ee::add(y, -5.0));
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x16 {
        let r = D + 0x5EB0 + 0x130 * rem2(n, 8) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..0x10u32 {
            let x = fx.rand_from(S0, 0x01E0_13F8) >> 23;
            fx.mem.set8(r + 8 + j, rem2(x, 4));
            let p = r + 0x30 + 0x10 * j;
            fx.random_table_vec(p, 3.0);
            let z = fx.mem.f(p + 8);
            // 0x1A7230 fabsf: the argument at its sp.
            fx.mem.write_f32(fx.sp - 0x10, z);
            let az = f32::from_bits(z.to_bits() & 0x7FFF_FFFF);
            let side = ee::from_i32(fx.mem.i32(D + 0xC));
            fx.mem.setf(p + 8, ee::mul(-1.0, ee::mul(side, az)));
            fx.vu_add4(p, p, D + 0x30);
        }
    }
    desc_reset(fx, sp);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0x118 - n, 0, 8);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(n, 0, 8);
    let m = fx.mult(a, b.wrapping_neg() << 7);
    fx.set_stage_light(((m >> 6) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x2D {
        ret = 1;
    } else {
        if (0x38..0xE8).contains(&n) {
            ret = 6;
        }
        if n == 0xE8 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x119 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}
