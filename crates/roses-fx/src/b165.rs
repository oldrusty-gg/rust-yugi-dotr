//! BMODULE b165 (card 173), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x92B0
//! bytes) [F]: a single unscheduled function in
//! a 0x330 frame (D at sp + 0xBC, the return at sp + 0xC0, s0 = the OT).
//! Every update the camera turn with roll (sp + 0x210) and the attacker's
//! node 0x5D (D+0x40); the target's point (0x1F65D0 kind 1) at D+0x20.
//! On 0xA1..0x145 a record an update (20 round robin) of 8 sprites at the
//! node (+0x50, 0xA0: cells of the UV table 0x28AE70, directions 0.1) and
//! one at the target (+0xCD0, 0x90: directions 1.0), each 20 updates
//! (sizes 2 / 20 x (0.2 + 0.8 (20 - age) / 20), spread 0.9 + 0.1 (20 -
//! age) / 20, rising -3 + 11 / -35 + 110 x age / 20, alpha 112 sin(32768
//! age / 20) x the window ramp (clamp(n - 0xA0, 0, 8 / 16) x clamp(0x145 -
//! n) / 64 / 256), colour (20, 20, 80), far Z, key 0). On n % 4 == 0 in
//! 1..0x5F a mote (+0x1820, 0x20, 20 round robin) at a random point 3.0
//! round the node, 64 updates: its point pulled to the node by (64 - age)
//! / 64 and swung 7 (64 - age) / 64 (+-1 by the slot) along sin / cos /
//! cos of 32768 (64 - age) / 64, a cell ping-ponging 0..4, alpha 255 age
//! / 64 (FF, 80, B4), keyed by w; every update it leaves a 4-sprite puff
//! (its group of 16 x 0x60 at +0x1AA0 + 0x600 slot) living 16 updates
//! (size 1.3 (0.2 + 0.8 (16 - age) / 16), spread 0.2 + 0.8 age / 16,
//! rising 6 age / 16, alpha 64 cos(age << 10) x the mote's age / 64, (80,
//! 40, 64)). From update 0 a white flat flash (prim 6, ALPHA 1) over 32
//! updates (alpha 128 (1 - falloff)) and, for the whole effect, in a
//! packet layer the frame copied (b451's frame copy, [`crate::lens`]), a
//! flat (60, 00, 80) flash (ALPHA 3) of alpha 4 clamp(0x165 - age, 0, 32)
//! and the copy drawn full screen over it at 0x80 minus that alpha (ALPHA
//! 0, the lens CLAMP), the layer at key 0xFFFFFF. Returns 1 on 0x96, 6 on
//! 0xA1..0x144, 2 on 0x145, 8 once the count reaches 0x166.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, far_z, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x330;
/// The sprite descriptor.
const DESC: u32 = 0x2A0;
/// The flat flash descriptor (prim 6, 3 registers 0x441).
const FLAT: u32 = 0x2F8;
/// The UV table 0x28AE70 (four (u0, u1, v0, v1) halfword cells).
const UV_TABLE: u32 = 0x0028_AE70;
/// The entry's s0 (0x206E40's return), rand's saved s0.
const S0: u32 = OT;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
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
    fx.bzero(D, 0x92B0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..2u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x5D, 1);
    fx.mem.set32(D + 0x30, r);
    for k in 0..0x14u32 {
        fx.mem.write_u8(D + 0x50 + 0xA0 * k, 0);
    }
    for k in 0..0x14u32 {
        fx.mem.write_u8(D + 0xCD0 + 0x90 * k, 0);
    }
    fx.mem.write_u32(D + 0x1810, 0);
    for k in 0..0x14u32 {
        fx.mem.write_u8(D + 0x1820 + 0x20 * k, 0);
        fx.mem.write_u8(D + 0x182A + 0x20 * k, 0);
    }
    for g in 0..0x14u32 {
        for e in 0..0x10u32 {
            fx.mem.write_u8(D + 0x1AA0 + 0x600 * g + 0x60 * e, 0);
        }
    }
    fx.mem.write_u8(D + 0x92A0, 0);
    fx.mem.write_u8(D + 0x92A8, 0);
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's two UVs (sp + 0x2B8 / 0x2BA / 0x2C8 / 0x2CA).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x2B8, u0);
    uv(fx, sp + 0x2BA, v0);
    uv(fx, sp + 0x2C8, u1);
    uv(fx, sp + 0x2CA, v1);
}

/// UV cell `i` of the table 0x28AE70.
fn table_uvs(fx: &mut Fx, sp: u32, i: u8) {
    let t = UV_TABLE + 8 * u32::from(i);
    let (u0, u1, v0, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 6));
    uvs(fx, sp, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The sprite descriptor (5 registers 0x42421, prim 0x26 | 0x800; its
/// pointers at sp + 0x110 / 0x120 / 0x130), texture D+`rh_at`, ALPHA
/// preset 1, clamp 5, and the view-projection (x identity) at sp + 0x190.
fn desc(fx: &mut Fx, sp: u32, rh_at: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x110, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x120, d + 8);
    fx.mem.write_u32(sp + 0x130, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + rh_at);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x190);
    fx.identity(sp + 0x1D0);
    let id = fx.read_m4(sp + 0x1D0);
    fx.mul_into(Some(sp + 0x190), &id);
}

/// The corners (-s, -s, 0, 1) at sp + 0x250 and (s, s, 0, 1) at sp +
/// 0x260 turned by the camera (sp + 0x210).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x250, n);
    fx.mem.setf(sp + 0x254, n);
    fx.mem.write_u32(sp + 0x258, 0);
    fx.mem.write_u32(sp + 0x25C, 0x3F80_0000);
    fx.mem.setf(sp + 0x260, s);
    fx.mem.setf(sp + 0x264, s);
    fx.mem.write_u32(sp + 0x268, 0);
    fx.mem.write_u32(sp + 0x26C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x250, sp + 0x210, sp + 0x250);
    fx.vu_apply_to(sp + 0x260, sp + 0x210, sp + 0x260);
}

/// The colour (c, a, Q = 1.0) at `p`.
fn colour(fx: &mut Fx, p: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

/// The vertices = the corners + sp + 0x290, projected.
fn project(fx: &mut Fx, sp: u32) -> bool {
    fx.vu_add4(sp + 0x270, sp + 0x250, sp + 0x290);
    fx.vu_add4(sp + 0x280, sp + 0x260, sp + 0x290);
    crate::fxlib2::project(fx, sp + 0x270, PROJ_OUT, sp + 0x190, sp + DESC, sp + 0x32C)
}

/// Drawn on the camera plane (far Z) at key 0.
fn draw_far(fx: &mut Fx, sp: u32) {
    if project(fx, sp) {
        far_z(fx, sp + 0x2C0, &[sp + 0x2D0]);
        fx.submit(sp + DESC, OT, 0);
    }
}

/// Drawn at the mean w; a1..a3 as the draw leaves them.
fn draw_key(fx: &mut Fx, sp: u32, ra: u32) -> [u32; 3] {
    if project(fx, sp) {
        let w = fx.mem.f(sp + 0x32C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
        // a3 = 0 after 0x1A2C70 [run].
        [OT, key, 0]
    } else {
        [PROJ_OUT, sp + 0x190, sp + DESC]
    }
}

/// x / 20 as compiled (the multiply by 0x66666667).
fn div20(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x6666_6667, x);
    let hi = fx.hilo().1;
    (hi >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

/// The window ramp clamp(n - 0xA0, 0, m) x clamp(0x145 - n, 0, m) / d.
fn ramp(fx: &mut Fx, m: i32, d: f32) -> f32 {
    let n = fx.mem.i32(D);
    let a = ee::from_i32(Fx::clamp_i(n - 0xA0, 0, m));
    let n = fx.mem.i32(D);
    let b = ee::from_i32(Fx::clamp_i(0x145 - n, 0, m));
    ee::div(ee::mul(a, b), d)
}

/// The sprite records (A: 8 at the node, +0x50; B: 8 at the target,
/// +0xCD0) [F 0x1E00424 / 0x1E00958].
#[allow(clippy::too_many_arguments)]
fn records(fx: &mut Fx, sp: u32, base: u32, stride: u32, counter: u32, f20: f32, size: f32, rise: (f32, f32), point: Option<u32>, ra: u32) {
    fx.mem.write_u32(sp + counter, 0);
    for i in 0..0x14u32 {
        let s4 = D + stride * i;
        let r = s4 + base;
        if fx.mem.read_u8(r) != 0 {
            let age = fx.mem.i32(r + 4);
            let q = div20(fx, age << 15);
            let sn = fx.tsin(q);
            let a = fix(fx, ee::mul(f20, ee::mul(112.0, sn)), ra);
            colour(fx, sp + 0x2B0, [0x20, 0x20, 0x80], a as u8);
            let age = fx.mem.i32(r + 4);
            let f = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x14 - age)), 20.0);
            let s = ee::mul(size, ee::add(f32::from_bits(0x3E4C_CCCD), f));
            corners(fx, sp, s);
            let age = fx.mem.i32(r + 4);
            let f4 = ee::div(ee::from_i32(age), 20.0);
            let f1 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(0x14 - age)), 20.0);
            let f21 = ee::add(f32::from_bits(0x3F66_6666), f1);
            let f22 = ee::add(rise.0, ee::mul(rise.1, f4));
            let p = match point {
                Some(p) => p,
                None => {
                    fx.mem.write_u32(sp + 0xD0, r + 0x10);
                    r + 0x10
                }
            };
            for j in 0..8u32 {
                let b = fx.mem.read_u8(r + 8 + j);
                table_uvs(fx, sp, b);
                // B keeps no point: its directions at +0x10.
                let dir = r + if point.is_some() { 0x10 } else { 0x20 } + 0x10 * j;
                let pt = sp + 0x290;
                let x = ee::mul(fx.mem.f(dir), f21);
                fx.mem.setf(pt, x);
                let y = ee::add(f22, fx.mem.f(dir + 4));
                fx.mem.setf(pt + 4, y);
                let z = ee::mul(fx.mem.f(dir + 8), f21);
                fx.mem.setf(pt + 8, z);
                fx.vu_add4(pt, pt, p);
                draw_far(fx, sp);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x15 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + counter, i + 1);
    }
}

/// The motes [F 0x1E00E48]: each draws a sprite and leaves a puff.
fn motes(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0x150, 0);
    fx.mem.write_u32(sp + 0xE0, D);
    let f20 = ee::neg(1.0);
    let pt = sp + 0x290;
    for i in 0..0x14u32 {
        let s1 = D + 0x20 * i;
        let grp = D + 0x600 * i;
        if fx.mem.read_u8(s1 + 0x1820) != 0 {
            let age = fx.mem.i32(s1 + 0x1824);
            let f21 = ee::div(ee::from_i32(age), 64.0);
            let a = fix(fx, ee::mul(255.0, f21), 0x01E0_0E80);
            colour(fx, sp + 0x2B0, [0xFF, 0x80, 0xB4], a as u8);
            let cell = fx.mem.read_u8(s1 + 0x1828);
            let (c, r) = (u32::from(cell & 3) << 5, u32::from(cell >> 2) << 5);
            let step = fx.mem.read_u8(s1 + 0x1829);
            fx.mem.write_u8(s1 + 0x1828, cell.wrapping_add(step));
            let now = fx.mem.read_u8(s1 + 0x1828);
            if now == 0 || now == 4 {
                let st = fx.mem.read_u8(s1 + 0x1829);
                fx.mem.write_u8(s1 + 0x1829, st.wrapping_neg());
            }
            uvs(fx, sp, (c << 4) & 0x3FFF, (r << 4) & 0x3FFF, ((c + 0x20) << 4) & 0x3FFF, ((r + 0x20) << 4) & 0x3FFF);
            fx.mem.setf(sp + 0x250, f20);
            fx.mem.setf(sp + 0x254, f20);
            fx.mem.write_u32(sp + 0x258, 0);
            fx.mem.write_u32(sp + 0x25C, 0x3F80_0000);
            fx.mem.write_u32(sp + 0x260, 0x3F80_0000);
            fx.mem.write_u32(sp + 0x264, 0x3F80_0000);
            fx.mem.write_u32(sp + 0x268, 0);
            fx.mem.write_u32(sp + 0x26C, 0x3F80_0000);
            fx.vu_apply_to(sp + 0x250, sp + 0x210, sp + 0x250);
            fx.vu_apply_to(sp + 0x260, sp + 0x210, sp + 0x260);
            let age = fx.mem.i32(s1 + 0x1824);
            let f22 = ee::div(ee::from_i32(0x40 - age), 64.0);
            let sign = if i & 1 != 0 { 1 } else { -1 };
            let f23 = ee::mul(ee::mul(7.0, f22), ee::from_i32(sign));
            let f = ee::mul(32768.0, f22);
            fx.fix_residue(f, 0x01E0_1034);
            let ang = ee::fixsfsi(f);
            fx.vu_sub4(pt, s1 + 0x1830, D + 0x40);
            fx.vu_scale4(pt, pt, f22);
            fx.vu_add4(pt, pt, D + 0x40);
            let sn = fx.tsin(ang);
            let x = ee::add(fx.mem.f(pt), ee::mul(f23, sn));
            fx.mem.setf(pt, x);
            let cs = fx.tcos(ang);
            let y = ee::add(fx.mem.f(pt + 4), ee::mul(f23, cs));
            fx.mem.setf(pt + 4, y);
            let cs = fx.tcos(ang);
            let z = ee::add(fx.mem.f(pt + 8), ee::mul(f23, cs));
            fx.mem.setf(pt + 8, z);
            if project(fx, sp) {
                let w = fx.mem.f(sp + 0x32C);
                let key = fix(fx, w, 0x01E0_1108);
                fx.submit(sp + DESC, OT, key as i32);
            }
            let k = u32::from(fx.mem.read_u8(s1 + 0x182A));
            let e = grp + 0x60 * k;
            fx.mem.write_u8(e + 0x1AA0, 1);
            fx.mem.write_u32(e + 0x1AA4, 0);
            fx.mem.setf(e + 0x1AAC, f21);
            words(fx, e + 0x1AB0, pt);
            for j in 0..4u32 {
                let r = fx.rand_from(S0, 0x01E0_1178) >> 23;
                fx.mem.set8(e + 0x1AA8 + j, rem2(r, 4));
                fx.random_table_vec(e + 0x1AC0 + 0x10 * j, f32::from_bits(0x3E4C_CCCD));
            }
            let k = fx.mem.read_u8(s1 + 0x182A).wrapping_add(1);
            fx.mem.write_u8(s1 + 0x182A, k);
            if fx.mem.read_u8(s1 + 0x182A) >= 0x10 {
                fx.mem.write_u8(s1 + 0x182A, 0);
            }
            let age = fx.mem.i32(s1 + 0x1824) + 1;
            fx.mem.set32(s1 + 0x1824, age);
            if age >= 0x41 {
                fx.mem.write_u8(s1 + 0x1820, 0);
            }
        }
        fx.mem.write_u32(sp + 0xE0, grp + 0x600);
        fx.mem.write_u32(sp + 0x150, i + 1);
    }
}

/// The puffs [F 0x1E012C4]: 20 groups of 16; a1..a3 as the last draw
/// leaves them (else `a`).
fn puffs(fx: &mut Fx, sp: u32, mut a: [u32; 3]) -> [u32; 3] {
    fx.mem.write_u32(sp + 0x140, 0);
    fx.mem.write_u32(sp + 0x100, D);
    let pt = sp + 0x290;
    for g in 0..0x14u32 {
        let grp = D + 0x600 * g;
        fx.mem.write_u32(sp + 0x180, 0);
        for e in 0..0x10u32 {
            let s4 = grp + 0x60 * e;
            if fx.mem.read_u8(s4 + 0x1AA0) != 0 {
                let age = fx.mem.i32(s4 + 0x1AA4);
                let c = fx.tcos((age << 14) >> 4);
                let sc = fx.mem.f(s4 + 0x1AAC);
                let al = fix(fx, ee::mul(sc, ee::mul(64.0, c)), 0x01E0_1304);
                colour(fx, sp + 0x2B0, [0x80, 0x40, 0x64], al as u8);
                let age = fx.mem.i32(s4 + 0x1AA4);
                let f = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0);
                let s = ee::mul(f32::from_bits(0x3FA6_6666), ee::add(f32::from_bits(0x3E4C_CCCD), f));
                corners(fx, sp, s);
                let age = ee::from_i32(fx.mem.i32(s4 + 0x1AA4));
                let f3 = ee::div(age, 16.0);
                let f2 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 16.0);
                let f20 = ee::mul(6.0, f3);
                let f21 = ee::add(f32::from_bits(0x3E4C_CCCD), f2);
                fx.mem.write_u32(sp + 0xF0, s4 + 0x1AB0);
                for j in 0..4u32 {
                    let b = fx.mem.read_u8(s4 + 0x1AA8 + j);
                    table_uvs(fx, sp, b);
                    let dir = s4 + 0x1AC0 + 0x10 * j;
                    let x = ee::mul(fx.mem.f(dir), f21);
                    fx.mem.setf(pt, x);
                    let y = ee::add(f20, ee::mul(fx.mem.f(dir + 4), f21));
                    fx.mem.setf(pt + 4, y);
                    let z = ee::mul(fx.mem.f(dir + 8), f21);
                    fx.mem.setf(pt + 8, z);
                    fx.vu_add4(pt, pt, s4 + 0x1AB0);
                    a = draw_key(fx, sp, 0x01E0_1564);
                }
                let age = fx.mem.i32(s4 + 0x1AA4) + 1;
                fx.mem.set32(s4 + 0x1AA4, age);
                if age >= 0x11 {
                    fx.mem.write_u8(s4 + 0x1AA0, 0);
                }
            }
            fx.mem.write_u32(sp + 0x180, e + 1);
        }
        fx.mem.write_u32(sp + 0x100, grp + 0x600);
        fx.mem.write_u32(sp + 0x140, g + 1);
    }
    a
}

/// The flat flash descriptor (prim 6, 3 registers 0x441) with ALPHA
/// `preset`.
fn flat(fx: &mut Fx, sp: u32, preset: i32) {
    let d = sp + FLAT;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(preset);
    fx.desc_alpha(d, p);
}

/// The flash's colour (c, a), full screen 0x7000..0x9000 at far Z,
/// submitted into `ot` (0: the open layer) at key 0.
fn flat_rest(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8, ot: u32) {
    let d = sp + FLAT;
    colour(fx, sp + 0x308, c, a);
    far_xy(fx, sp + 0x310, 0x7000, 0x7000);
    far_xy(fx, sp + 0x318, 0x9000, 0x9000);
    fx.submit(d, ot, 0);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x210);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30));
    fx.node_position(h, slot, D + 0x40);
    let n = fx.mem.i32(D);
    if (0xA1..0x145).contains(&n) {
        let r = D + 0xA0 * (n % 0x14) as u32 + 0x50;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x10, D + 0x40);
        for j in 0..8u32 {
            let x = fx.rand_from(S0, 0x01E0_02DC) >> 23;
            fx.mem.set8(r + 8 + j, rem2(x, 4));
            fx.random_table_vec(r + 0x20 + 0x10 * j, f32::from_bits(0x3DCC_CCCD));
        }
    }
    desc(fx, sp, 0x10);
    let f20 = ramp(fx, 8, 64.0);
    records(fx, sp, 0x50, 0xA0, 0x170, f20, 2.0, (-3.0, 11.0), None, 0x01E0_0478);
    let n = fx.mem.i32(D);
    if (0xA1..0x145).contains(&n) {
        let r = D + 0x90 * (n % 0x14) as u32 + 0xCD0;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..8u32 {
            let x = fx.rand_from(S0, 0x01E0_0824) >> 23;
            fx.mem.set8(r + 8 + j, rem2(x, 4));
            fx.random_table_vec(r + 0x10 + 0x10 * j, 1.0);
        }
    }
    desc(fx, sp, 0x10);
    let f20 = ramp(fx, 0x10, 256.0);
    records(fx, sp, 0xCD0, 0x90, 0x160, f20, 20.0, (-35.0, 110.0), Some(D + 0x20), 0x01E0_09AC);
    let n = fx.mem.i32(D);
    if rem2(n, 4) == 0 && n > 0 && n < 0x60 {
        let k = fx.mem.i32(D + 0x1810);
        let r = D + 0x20 * k as u32 + 0x1820;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        fx.mem.write_u8(r + 8, 0);
        fx.mem.write_u8(r + 9, 1);
        fx.random_table_vec(r + 0x10, 3.0);
        fx.vu_add4(r + 0x10, r + 0x10, D + 0x40);
        let k = fx.mem.i32(D + 0x1810) + 1;
        fx.mem.set32(D + 0x1810, k);
        if k >= 0x14 {
            fx.mem.write_u32(D + 0x1810, 0);
        }
    }
    desc(fx, sp, 0x14);
    motes(fx, sp);
    desc(fx, sp, 0x10);
    // a1..a3 as the view-projection leaves them: 0x20B370 (a1 the
    // identity), 0x20E2B0's a2 = 0 and a3 past the camera's matrix.
    let cam = 0x002E_D720 + 0x1A0 * fx.mem.read_u32(0x002E_D6C0) + 0x40;
    let a = puffs(fx, sp, [sp + 0x1D0, 0, cam]);
    let mut a = [1, a[0], a[1], a[2]];
    let n = fx.mem.i32(D);
    if n == 0 {
        fx.mem.write_u8(D + 0x92A0, 1);
        fx.mem.write_u32(D + 0x92A4, 0);
    }
    if fx.mem.read_u8(D + 0x92A0) == 1 {
        flat(fx, sp, 1);
        let age = fx.mem.i32(D + 0x92A4);
        let f0 = fx.tan_falloff_p(age, 0x20, 1);
        let al = fix(fx, ee::mul(128.0, ee::sub(1.0, f0)), 0x01E0_16AC);
        flat_rest(fx, sp, [0xFF, 0xFF, 0xFF], al as u8, OT);
        a = [sp + FLAT, OT, 0, 0];
        let age = fx.mem.i32(D + 0x92A4) + 1;
        fx.mem.set32(D + 0x92A4, age);
        if age >= 0x21 {
            fx.mem.write_u8(D + 0x92A0, 0);
        }
    }
    let n = fx.mem.i32(D);
    if n == 0 {
        fx.mem.write_u8(D + 0x92A8, 1);
        fx.mem.write_u32(D + 0x92AC, 0);
    }
    if fx.mem.read_u8(D + 0x92A8) == 1 {
        let layer = fx.layer_begin();
        let d = sp + DESC;
        fx.mem.write_u16(d + 2, 0x26);
        fx.mem.write_u16(d, 5);
        fx.mem.write_u64(d + 8, 0x4_2421);
        fx.mem.write_u32(d + 4, 0);
        let regs = crate::lens::frame_copy(fx, a, 0, sp + 0x2E0, sp + 0x2E8);
        fx.texflush(regs);
        flat(fx, sp, 3);
        let age = fx.mem.i32(D + 0x92AC);
        let v = Fx::clamp_i(0x165 - age, 0, 0x20);
        let al = (v << 7) >> 5;
        let s2 = (al & 0xFF) as u8;
        flat_rest(fx, sp, [0x60, 0x00, 0x80], al as u8, 0);
        let p = fx.alpha_preset(0);
        fx.desc_alpha(d, p);
        fx.desc_clamp(d, LENS_CLAMP);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        colour(fx, sp + 0x2B0, [0x80, 0x80, 0x80], 0x80u8.wrapping_sub(s2));
        uvs(fx, sp, 0, 0, 0x2000, 0x2000);
        far_xy(fx, sp + 0x2C0, 0x6FF8, 0x6FF8);
        far_xy(fx, sp + 0x2D0, 0x8FF8, 0x9FF8);
        fx.submit(d, 0, 0);
        fx.layer_end(layer, OT, 0x00FF_FFFF);
        let age = fx.mem.i32(D + 0x92AC) + 1;
        fx.mem.set32(D + 0x92AC, age);
        if age >= 0x166 {
            fx.mem.write_u8(D + 0x92A8, 0);
        }
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x96 {
        ret = 1;
    } else {
        if (0xA1..0x145).contains(&n) {
            ret = 6;
        }
        if n == 0x145 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x166 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}
