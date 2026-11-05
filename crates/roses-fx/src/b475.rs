//! BMODULE b475 (card 411), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x3700
//! bytes) [F]: a single unscheduled function in
//! a 0x4C0 frame on b150's helpers. Every update the camera turn with
//! roll (0x2055E0 into sp + 0x2F0), the attacker's node 7 (D+0x40) and
//! nodes 0x26 / 0x35 / 0x2B / 0x30 (D+0x60..0x90). Updates 0x56..0xE9 one
//! of 128 records (+0xA0, 0x50) a update: two textured quads (descriptor
//! sp + 0x440, prim 0x24, colour (B9, FF, 58), alpha 128 x ratio(age, 12))
//! from node 7 drifting out to the target, swaying by the table sine /
//! cosine of their angles + 0x15B3 age; at age 128 each turns into one of
//! 24 records (+0x28B0, 0x40) of two sprite pairs (descriptor sp + 0x3E8,
//! textures D+0x10 / D+0x14, alpha 128 / 64 cos) rising for 24 updates.
//! On updates 0x18 + 3 m (m < 9) one of 12 bursts (+0x2EC0, 0xB0) of 2 x 4
//! sprites (colour (62, 58, 58), additive) at two of the four nodes,
//! spreading by the tan falloff for 32 updates. Returns 1 on update 0xCB,
//! 6 on 0xD6..0x181, 2 on 0x182, 8 once the count reaches 0x184.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The entry's s0 (0x206E40's return), rand's saved s0.
const S0: u32 = OT;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x4C0);
    fx.mem.write_u32(sp + 0xD0, D);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(0x4C0);
    ret
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x3700);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    let r = fx.texture(5, 0);
    fx.mem.write_u32(D + 0x10, r);
    let r = fx.texture(5, 1);
    fx.mem.write_u32(D + 0x14, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 7, 1);
    fx.mem.set32(D + 0x30, r);
    for (k, node) in [0x26, 0x35, 0x2B, 0x30].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, 0);
        fx.mem.set32(D + 0x50 + 4 * k as u32, r);
    }
    for k in 0..0x80u32 {
        fx.mem.write_u8(D + 0xA0 + 0x50 * k, 0);
    }
    fx.mem.write_u32(D + 0x28A0, 0);
    for k in 0..0x18u32 {
        fx.mem.write_u8(D + 0x28B0 + 0x40 * k, 0);
    }
    fx.mem.write_u32(D + 0x2EB0, 0);
    fx.mem.write_u32(D + 0x2EB4, 0);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x2EC0 + 0xB0 * k, 0);
    }
    // The clear loop steps the stack copy of a0.
    fx.mem.write_u32(sp + 0xD0, D + 0xB0 * 0xC);
    0
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

/// (rand >> 23) % 256 x 0.5 / 256 + 0.5.
fn rand_scale(fx: &mut Fx, ra: u32) -> f32 {
    let r = fx.rand_from(S0, ra) >> 23;
    let x = ee::from_i32(rem2(r, 256));
    ee::add(0.5, ee::div(ee::mul(0.5, x), 256.0))
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// A 4 x 8 cell `b` of the texture: (u0, v0, u1, v1).
fn cell(b: u8) -> (u32, u32, u32, u32) {
    let (c, r) = (u32::from(b & 3), u32::from(b >> 2));
    ((c << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF)
}

/// The sprite cell at sp + 0x400 / 0x402 / 0x410 / 0x412.
fn sprite_uvs(fx: &mut Fx, sp: u32, b: u8) {
    let (u0, v0, u1, v1) = cell(b);
    uv(fx, sp + 0x400, u0);
    uv(fx, sp + 0x402, v0);
    uv(fx, sp + 0x410, u1);
    uv(fx, sp + 0x412, v1);
}

fn words(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// The view-projection (x identity) at sp + 0x270.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x270);
    fx.identity(sp + 0x2B0);
    let id = fx.read_m4(sp + 0x2B0);
    fx.mul_into(Some(sp + 0x270), &id);
}

/// Projected (0x204F70 into 0x2D3600) and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, d: u32, ra: u32) {
    if crate::fxlib2::project(fx, sp + 0x370, PROJ_OUT, sp + 0x270, d, sp + 0x4BC) {
        let w = fx.mem.f(sp + 0x4BC);
        fx.fix_residue(w, ra);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, OT, key);
    }
}

fn alpha(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The sprite corners (s, s, 0, 1) / (-s, -s, 0, 1) at sp + 0x330 / 0x340
/// turned by the camera (sp + 0x2F0).
fn sprite_corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x330, s);
    fx.mem.setf(sp + 0x334, s);
    fx.mem.write_u32(sp + 0x338, 0);
    fx.mem.write_u32(sp + 0x33C, 0x3F80_0000);
    fx.mem.setf(sp + 0x340, n);
    fx.mem.setf(sp + 0x344, n);
    fx.mem.write_u32(sp + 0x348, 0);
    fx.mem.write_u32(sp + 0x34C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x330, sp + 0x2F0, sp + 0x330);
    fx.vu_apply_to(sp + 0x340, sp + 0x2F0, sp + 0x340);
}

/// The sprite descriptor at sp + 0x3E8 (5 registers 0x42421, prim 0x26 |
/// 0x800), stored through the pointers sp + 0x1D0 / 0x1F0 / 0x200.
fn sprite_desc(fx: &mut Fx, sp: u32) {
    let d = sp + 0x3E8;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// Updates 0x56..0xE9 [F 0x1E002D0]: record n % 128 at node 7.
fn spawn_quads(fx: &mut Fx, n: i32) {
    let rec = D + 0x50 * rem2(n, 0x80) as u32;
    fx.mem.write_u8(rec + 0xA0, 1);
    fx.mem.write_u32(rec + 0xA4, 0);
    words(fx, rec + 0xC0, D + 0x40);
    for s5 in 0..2u32 {
        let r = fx.rand_from(S0, 0x01E0_0340) >> 23;
        fx.mem.set8(rec + s5 + 0xA8, rem2(r, 4));
        let f = rand_scale(fx, 0x01E0_0360);
        fx.mem.setf(rec + 4 * s5 + 0xAC, f);
        let r = fx.rand_from(S0, 0x01E0_03AC) >> 23;
        fx.mem.set32(rec + 4 * s5 + 0xB4, (rem2(r, 256) << 16) >> 8);
        let v = rec + 0x10 * s5 + 0xD0;
        fx.random_table_vec(v, 1.0);
        fx.vu_add4(v, v, D + 0x20);
        fx.vu_sub4(v, v, rec + 0xC0);
    }
}

/// The quad records [F 0x1E004E8].
fn quads(fx: &mut Fx, sp: u32) {
    let fp = sp + 0x340;
    fx.mem.write_u32(sp + 0x240, 0);
    fx.mem.write_u32(sp + 0xF0, D);
    for (k, a) in [0x458u32, 0x468, 0x478, 0x488].into_iter().enumerate() {
        fx.mem.write_u32(sp + 0x100 + 0x10 * k as u32, sp + a + 2);
    }
    let d = sp + 0x440;
    for k in 0..0x80u32 {
        let rec = D + 0x50 * k;
        if fx.mem.read_u8(rec + 0xA0) != 0 {
            let age = fx.mem.i32(rec + 0xA4);
            let r = Fx::ratio_below(ee::from_i32(age), 12.0);
            let a = alpha(fx, ee::mul(128.0, r), 0x01E0_0520);
            fx.mem.write_u8(sp + 0x450, 0xB9);
            fx.mem.write_u8(sp + 0x451, 0xFF);
            fx.mem.write_u8(sp + 0x452, 0x58);
            fx.mem.write_u8(sp + 0x453, a as u8);
            fx.mem.write_u32(sp + 0x454, 0x3F80_0000);
            let c = Fx::clamp_i(fx.mem.i32(rec + 0xA4), 0, 0x80);
            let f21 = ee::div(ee::from_i32(c), 128.0);
            let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f21));
            fx.mem.write_u32(sp + 0xE0, rec + 0xC0);
            for s6 in 0..2u32 {
                let scale = fx.mem.f(rec + 4 * s6 + 0xAC);
                let age = fx.mem.i32(rec + 0xA4);
                let f3 = ee::mul(0.5, ee::from_i32(age));
                let v1 = fx.mult(age, 0x15B3);
                let f2 = ee::add(0.5, ee::div(f3, 128.0));
                let f1 = ee::mul(10.0, f2);
                let s7 = fx.mem.i32(rec + 4 * s6 + 0xB4).wrapping_add(v1) & 0xFFFF;
                let f22 = ee::mul(f1, scale);
                let sn = fx.table_trig(s7, false);
                let f23 = ee::mul(f22, ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), sn)));
                let cs = fx.table_trig(s7, true);
                let f0 = ee::mul(f22, ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), cs)));
                let (n23, n0) = (ee::neg(f23), ee::neg(f0));
                for (o, x, y) in [(0x330u32, f23, f23), (0x340, n23, f0), (0x350, f0, n23), (0x360, n0, n0)] {
                    fx.mem.setf(sp + o, x);
                    fx.mem.setf(sp + o + 4, y);
                    fx.mem.write_u32(sp + o + 8, 0);
                    fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
                }
                fx.vu_apply_to(sp + 0x330, sp + 0x2F0, sp + 0x330);
                fx.vu_apply_to(fp, sp + 0x2F0, fp);
                fx.vu_apply_to(sp + 0x350, sp + 0x2F0, sp + 0x350);
                fx.vu_apply_to(sp + 0x360, sp + 0x2F0, sp + 0x360);
                let b = fx.mem.read_u8(rec + s6 + 0xA8);
                let (u0, v0, u1, v1) = cell(b);
                uv(fx, sp + 0x458, u0);
                uv(fx, sp + 0x45A, v0);
                uv(fx, sp + 0x468, u1);
                uv(fx, sp + 0x46A, v0);
                uv(fx, sp + 0x478, u0);
                uv(fx, sp + 0x47A, v1);
                uv(fx, sp + 0x488, u1);
                uv(fx, sp + 0x48A, v1);
                let o = rec + 0x10 * s6;
                let x = ee::mul(fx.mem.f(o + 0xD0), f20);
                fx.mem.setf(sp + 0x3B0, x);
                let y = ee::mul(fx.mem.f(o + 0xD4), f20);
                fx.mem.setf(sp + 0x3B4, y);
                let z = ee::mul(fx.mem.f(o + 0xD8), f21);
                fx.mem.setf(sp + 0x3B8, z);
                fx.vu_add4(sp + 0x3B0, sp + 0x3B0, rec + 0xC0);
                for (out, c) in [(0x370u32, 0x330u32), (0x380, 0x340), (0x390, 0x350), (0x3A0, 0x360)] {
                    fx.vu_add4(sp + out, sp + c, sp + 0x3B0);
                }
                draw(fx, sp, d, 0x01E0_08B4);
                words(fx, sp + 0x3C0 + 0x10 * s6, sp + 0x3B0);
            }
            let age = fx.mem.i32(rec + 0xA4) + 1;
            fx.mem.set32(rec + 0xA4, age);
            if age >= 0x80 {
                fx.mem.write_u8(rec + 0xA0, 0);
                let m = fx.mem.read_u32(D + 0x28A0);
                let s3 = D.wrapping_add(m << 6);
                fx.mem.write_u8(s3 + 0x28B0, 1);
                fx.mem.write_u32(s3 + 0x28B4, 0);
                for s4 in 0..2u32 {
                    let r = fx.rand_from(S0, 0x01E0_0968) >> 23;
                    fx.mem.set8(s3 + s4 + 0x28B8, (r & 3) + 4);
                    let f = rand_scale(fx, 0x01E0_098C);
                    fx.mem.setf(s3 + 4 * s4 + 0x28BC, f);
                    words(fx, s3 + 0x10 * s4 + 0x28D0, sp + 0x3C0 + 0x10 * s4);
                }
                let m = fx.mem.i32(D + 0x28A0) + 1;
                fx.mem.set32(D + 0x28A0, m);
                if m >= 0x18 {
                    fx.mem.write_u32(D + 0x28A0, 0);
                }
            }
        }
        fx.mem.write_u32(sp + 0xF0, rec + 0x50);
        fx.mem.write_u32(sp + 0x240, k + 1);
    }
}

/// (age << 14) / 24 (the compiled `mult` by 0x2AAAAAAB).
fn angle24(fx: &mut Fx, age: i32) -> i32 {
    let x = age << 14;
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 2) + ((x as u32) >> 31) as i32
}

/// The rising sprite pairs [F 0x1E00A60].
fn pairs(fx: &mut Fx, sp: u32) {
    let d = sp + 0x3E8;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x1D0, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x1F0, d + 8);
    fx.mem.write_u32(sp + 0x200, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x230, 0);
    let fp = sp + 0x340;
    for j in 0..0x18u32 {
        let s2 = D + 0x40 * j;
        if fx.mem.read_u8(s2 + 0x28B0) != 0 {
            let age = fx.mem.i32(s2 + 0x28B4);
            let an = angle24(fx, age);
            let c = fx.table_trig(an, true);
            let a1 = alpha(fx, ee::mul(128.0, c), 0x01E0_0B48);
            fx.mem.write_u8(sp + 0x250, a1 as u8);
            let age = fx.mem.i32(s2 + 0x28B4);
            let an = angle24(fx, age);
            let c = fx.table_trig(an, true);
            let a2 = alpha(fx, ee::mul(64.0, c), 0x01E0_0B8C);
            fx.mem.write_u8(sp + 0xC0, a2 as u8);
            let age = fx.mem.i32(s2 + 0x28B4);
            let f20 = ee::div(ee::mul(30.0, ee::from_i32(age)), 24.0);
            for s3 in 0..2u32 {
                fx.mem.write_u32(sp + 0x140, s2 + 4 * s3);
                fx.mem.write_u32(sp + 0x150, s2 + s3);
                let age = fx.mem.i32(s2 + 0x28B4);
                let f1 = ee::div(ee::mul(0.5, ee::from_i32(age)), 24.0);
                let sc = fx.mem.f(s2 + 4 * s3 + 0x28BC);
                let s = ee::mul(ee::mul(24.0, ee::add(0.5, f1)), sc);
                sprite_corners(fx, sp, s);
                let b = fx.mem.read_u8(s2 + s3 + 0x28B8);
                sprite_uvs(fx, sp, b);
                words(fx, sp + 0x3B0, s2 + 0x10 * s3 + 0x28D0);
                fx.vu_add4(sp + 0x370, sp + 0x330, sp + 0x3B0);
                fx.vu_add4(sp + 0x380, fp, sp + 0x3B0);
                let rh = fx.mem.read_u32(D + 0x10);
                fx.desc_texture(d, rh, 1);
                fx.mem.write_u8(sp + 0x3F8, 0xB9);
                fx.mem.write_u8(sp + 0x3F9, 0xFF);
                fx.mem.write_u8(sp + 0x3FA, 0x58);
                let a = fx.mem.read_u8(sp + 0x250);
                fx.mem.write_u8(sp + 0x3FB, a);
                fx.mem.write_u32(sp + 0x3FC, 0x3F80_0000);
                draw(fx, sp, d, 0x01E0_0D9C);
                let y = ee::add(fx.mem.f(sp + 0x3B4), f20);
                fx.mem.setf(sp + 0x3B4, y);
                fx.vu_add4(sp + 0x370, sp + 0x330, sp + 0x3B0);
                fx.vu_add4(sp + 0x380, fp, sp + 0x3B0);
                let rh = fx.mem.read_u32(D + 0x14);
                fx.desc_texture(d, rh, 1);
                fx.mem.write_u8(sp + 0x3F8, 0xC8);
                fx.mem.write_u8(sp + 0x3F9, 0xFF);
                fx.mem.write_u8(sp + 0x3FA, 0x58);
                let a = fx.mem.read_u8(sp + 0xC0);
                fx.mem.write_u8(sp + 0x3FB, a);
                fx.mem.write_u32(sp + 0x3FC, 0x3F80_0000);
                draw(fx, sp, d, 0x01E0_0E5C);
                fx.mem.write_u32(sp + 0x140, s2 + 4 * s3 + 4);
                fx.mem.write_u32(sp + 0x150, s2 + s3 + 1);
            }
            let age = fx.mem.i32(s2 + 0x28B4) + 1;
            fx.mem.set32(s2 + 0x28B4, age);
            if age >= 0x19 {
                fx.mem.write_u8(s2 + 0x28B0, 0);
            }
        }
        fx.mem.write_u32(sp + 0x230, j + 1);
    }
}

/// On updates 0x18 + 3 m (m < 9) [F 0x1E00ED4]: burst D+0x2EB4 at two of
/// the four node points (by m's parity).
fn spawn_burst(fx: &mut Fx, sp: u32) {
    let m = fx.mem.i32(D + 0x2EB0);
    if m >= 9 || fx.mem.i32(D) != 3 * m + 0x18 {
        return;
    }
    fx.mem.write_u32(sp + 0x220, 0);
    let e = D.wrapping_add(0xB0u32.wrapping_mul(fx.mem.read_u32(D + 0x2EB4)));
    fx.mem.write_u32(sp + 0x1E0, e);
    fx.mem.write_u8(e + 0x2EC0, 1);
    fx.mem.write_u32(e + 0x2EC4, 0);
    fx.mem.write_u32(sp + 0x160, e);
    fx.mem.write_u32(sp + 0x170, e);
    for t in 0..2u32 {
        let m = fx.mem.i32(D + 0x2EB0);
        let src = D.wrapping_add((2 * rem2(m, 2) + t as i32) as u32 * 0x10) + 0x60;
        words(fx, e + 0x10 * t + 0x2ED0, src);
        for s2 in 0..4u32 {
            let r = fx.rand_from(S0, 0x01E0_0FB0) >> 23;
            fx.mem.set8(e + 4 * t + s2 + 0x2EC8, rem2(r, 8));
            let v = e + 0x40 * t + 0x10 * s2 + 0x2EF0;
            fx.random_table_vec(v, f32::from_bits(0x3E4C_CCCD));
            let y = fx.mem.f(v + 4);
            // 0x1A7230 fabsf: the argument at its sp.
            fx.mem.write_f32(fx.sp - 0x10, y);
            fx.mem.write_u32(v + 4, y.to_bits() & 0x7FFF_FFFF);
        }
        fx.mem.write_u32(sp + 0x160, e + 0x10 * t + 0x10);
        fx.mem.write_u32(sp + 0x1E0, e + 4 * t + 4);
        fx.mem.write_u32(sp + 0x170, e + 0x40 * t + 0x40);
        fx.mem.write_u32(sp + 0x220, t + 1);
    }
    let m = fx.mem.i32(D + 0x2EB0) + 1;
    fx.mem.set32(D + 0x2EB0, m);
    let i = fx.mem.i32(D + 0x2EB4) + 1;
    fx.mem.set32(D + 0x2EB4, i);
    if i >= 0xC {
        fx.mem.write_u32(D + 0x2EB4, 0);
    }
}

/// The bursts' sprites [F 0x1E0107C].
fn bursts(fx: &mut Fx, sp: u32) {
    let d = sp + 0x3E8;
    sprite_desc(fx, sp);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x210, 0);
    fx.mem.write_u32(sp + 0x1C0, D);
    let fp = sp + 0x340;
    for k in 0..0xCu32 {
        let e = D + 0xB0 * k;
        if fx.mem.read_u8(e + 0x2EC0) != 0 {
            let age = fx.mem.i32(e + 0x2EC4);
            let f1 = ee::div(ee::mul(0.5, ee::from_i32(age)), 32.0);
            let s = ee::mul(5.0, ee::add(0.5, f1));
            sprite_corners(fx, sp, s);
            let age = fx.mem.i32(e + 0x2EC4);
            let c = fx.table_trig((age << 14) >> 5, true);
            let a = alpha(fx, ee::mul(128.0, c), 0x01E0_11D0);
            fx.mem.write_u8(sp + 0x3F8, 0x62);
            fx.mem.write_u8(sp + 0x3F9, 0x58);
            fx.mem.write_u8(sp + 0x3FA, 0x58);
            fx.mem.write_u8(sp + 0x3FB, a as u8);
            fx.mem.write_u32(sp + 0x3FC, 0x3F80_0000);
            let age = fx.mem.i32(e + 0x2EC4);
            let f0 = fx.tan_falloff_p(age, 0x20, 1);
            let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::mul(f32::from_bits(0x3F66_6666), f0));
            fx.mem.write_u32(sp + 0x260, 0);
            for t in 0..2u32 {
                fx.mem.write_u32(sp + 0x190, e + 4 * t);
                fx.mem.write_u32(sp + 0x1A0, e + 0x40 * t);
                fx.mem.write_u32(sp + 0x1B0, e + 0x10 * t);
                let centre = e + 0x10 * t + 0x2ED0;
                fx.mem.write_u32(sp + 0x180, centre);
                for s1 in 0..4u32 {
                    let b = fx.mem.read_u8(e + 4 * t + s1 + 0x2EC8);
                    sprite_uvs(fx, sp, b);
                    fx.vu_scale4(sp + 0x3B0, e + 0x40 * t + 0x10 * s1 + 0x2EF0, f20);
                    fx.vu_add4(sp + 0x3B0, sp + 0x3B0, centre);
                    fx.vu_add4(sp + 0x370, sp + 0x330, sp + 0x3B0);
                    fx.vu_add4(sp + 0x380, fp, sp + 0x3B0);
                    draw(fx, sp, d, 0x01E0_1358);
                }
                fx.mem.write_u32(sp + 0x190, e + 4 * t + 4);
                fx.mem.write_u32(sp + 0x1A0, e + 0x40 * t + 0x40);
                fx.mem.write_u32(sp + 0x1B0, e + 0x10 * t + 0x10);
                fx.mem.write_u32(sp + 0x260, t + 1);
            }
            let age = fx.mem.i32(e + 0x2EC4) + 1;
            fx.mem.set32(e + 0x2EC4, age);
            if age >= 0x21 {
                fx.mem.write_u8(e + 0x2EC0, 0);
            }
        }
        fx.mem.write_u32(sp + 0x1C0, e + 0xB0);
        fx.mem.write_u32(sp + 0x210, k + 1);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x2F0);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30));
    fx.node_position(h, slot, D + 0x40);
    for s3 in 0..4u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x50 + 4 * s3));
        fx.node_position(h, slot, D + 0x60 + 0x10 * s3);
    }
    let n = fx.mem.i32(D);
    if (0x56..0xEA).contains(&n) {
        spawn_quads(fx, n);
    }
    let d = sp + 0x440;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    quads(fx, sp);
    pairs(fx, sp);
    spawn_burst(fx, sp);
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xCB {
        ret = 1;
    } else if (0xD6..0x182).contains(&n) {
        ret = 6;
    } else if n == 0x182 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x184 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
