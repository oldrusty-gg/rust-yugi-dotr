//! BMODULE b100 (card 614), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080 in a 0x3A0 frame (D kept at sp +
//! 0xBC, s3 = the ordering table), data area a0 = 0x1ED0000, 0xBF60
//! bytes) [F]. b141's kind (card 88, b141.rs):
//! sprite pools drawn through 0x204F70 / 0x206F30 (descriptor sp + 0x340,
//! UVs from the table 0x28AE70 or the whole 0x800 cell), then the outcome
//! boundary. Every update the camera turn with roll (sp + 0x2B0) and six
//! attacker nodes 0xA / 0x15 / 0x26 / 0x34 / 0x3D / 0x46 (D+0x50..0xA0);
//! the target's point (0x1F65D0 kind 1) is D+0x20 from the init; on 0x1E
//! the midpoint of the first and last node (D+0xB0) and the way from it to
//! the target (D+0xC0). Pools: A (D+0xD0, 8 x 0x3A0, spawned 1..0xF9, 8
//! updates) 6 x 8 table sprites at the nodes rising from -4 by 10 (80, 30,
//! 20); B (D+0x1DE0, 8 x 0xD0 by the counter D+0x1DD0, every fourth update
//! 4..0xF8, 24 updates) a whole-cell sprite at each node rising 6 (FF, 18,
//! 18); C (D+0x2460, 32 x 0x380, 0x1F..0x3D, 164 updates) a head moving
//! from the midpoint to twice the way that leaves one of 16 two-sprite
//! puffs every update (80, 30, 20); D (D+0x9460, 16 x 0x230, 0x75..0xF9,
//! 16 updates) 32 sprites at the target rising from -25 by 50; E
//! (D+0xB760, 64 x 0x20, 0x75..0xF9, 64 updates) one whole-cell sprite at
//! the target (FF, 18, 18). Then 0x1F8100 on the last callee's a0..a3 and
//! at 1 from 0x85 0x1F77A0(D+8, 2, (80, 80, 80, 4 clamp(n - 0x84, 0,
//! 0x20))). Returns 1 on 0x6A, 6 on 0x75..0xF9, 2 on 0xFA, 8 once the
//! count reaches 0xFC.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The UV table 0x28AE70 (four (u0, u1, v0, v1) halfword cells).
const UV_TABLE: u32 = 0x0028_AE70;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x3A0;
/// The camera's view-projection and the identity it is multiplied by.
const VP: u32 = 0x230;
const ID: u32 = 0x270;
/// The camera turn with roll.
const TURN: u32 = 0x2B0;
/// The corner offsets, the projected corners and the sprite's centre.
const C0: u32 = 0x2F0;
const C1: u32 = 0x300;
const P0: u32 = 0x310;
const P1: u32 = 0x320;
const POS: u32 = 0x330;
/// The sprite descriptor, its RGBAQ, UV0 / UV1.
const DESC: u32 = 0x340;
const RGBA: u32 = 0x350;
const UV0: u32 = 0x358;
const UV1: u32 = 0x368;
/// The mean-w slot and the fade's colour bytes.
const W: u32 = 0x398;
const FADE: u32 = 0x39C;
/// The target's point.
const TARGET: u32 = D + 0x20;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0xBF60);
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
    fx.unit_point(1, t, TARGET);
    for (k, (node, kind)) in [(0xA, 1), (0x15, 0), (0x26, 0), (0x34, 0), (0x3D, 0), (0x46, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, kind);
        fx.mem.set32(D + 0x30 + 4 * k as u32, r);
    }
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0xD0 + 0x3A0 * k, 0);
    }
    fx.mem.write_u32(D + 0x1DD0, 0);
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x1DE0 + 0xD0 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x2460 + 0x380 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x9460 + 0x230 * k, 0);
    }
    for k in 0..0x40u32 {
        fx.mem.write_u8(D + 0xB760 + 0x20 * k, 0);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_add_mode(t, 2, 0, [param as u32 as u64, 0], 0x01E0_02C8);
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

fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// UV0 / UV1 of the sprite: (h & 0xC000) | (v & 0x3FFF).
fn set_uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    for (a, v) in [(UV0, u0), (UV0 + 2, v0), (UV1, u1), (UV1 + 2, v1)] {
        let h = fx.mem.read_u16(sp + a);
        fx.mem.write_u16(sp + a, (h & 0xC000) | (v & 0x3FFF) as u16);
    }
}

/// UV cell `i` of the table 0x28AE70.
fn table_uvs(fx: &mut Fx, sp: u32, i: u8) {
    let t = UV_TABLE + 8 * u32::from(i);
    let (u0, v0, u1, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 6));
    set_uvs(fx, sp, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

fn rgba(fx: &mut Fx, sp: u32, c: [u8; 3], a: u32) {
    fx.mem.write_u8(sp + RGBA, c[0]);
    fx.mem.write_u8(sp + RGBA + 1, c[1]);
    fx.mem.write_u8(sp + RGBA + 2, c[2]);
    fx.mem.write_u8(sp + RGBA + 3, a as u8);
    fx.mem.write_u32(sp + RGBA + 4, 0x3F80_0000);
}

/// The sprite descriptor head (prim 0x26 | 0x800, 5 registers): texture
/// D + `tex` (filter 1), ALPHA preset 1, CLAMP 5.
fn head(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
}

/// The camera's view-projection x the identity at sp + VP.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + ID);
    mul_mem(fx, sp + VP, sp + ID);
}

/// The camera record + 0x40 (0x20E2B0's a3, left by the camera's calls).
fn camera_a3(fx: &mut Fx) -> u32 {
    let idx = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0)) + 0x40
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    for (o, v) in [(C0, n), (C1, s)] {
        fx.mem.setf(sp + o, v);
        fx.mem.setf(sp + o + 4, v);
        fx.mem.write_u32(sp + o + 8, 0);
        fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
    }
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// k (lo + slope age / d).
fn size(k: f32, lo: u32, slope: u32, age: i32, d: f32) -> f32 {
    let f = ee::div(ee::mul(f32::from_bits(slope), ee::from_i32(age)), d);
    ee::mul(k, ee::add(f32::from_bits(lo), f))
}

/// The centre (v.x f21, f22 + v.y, v.z f21) + `base` (its w as the frame
/// left it, plus the base's).
fn place(fx: &mut Fx, sp: u32, v: u32, f21: f32, f22: f32, base: u32) {
    let x = ee::mul(fx.mem.f(v), f21);
    fx.mem.setf(sp + POS, x);
    let y = ee::add(f22, fx.mem.f(v + 4));
    fx.mem.setf(sp + POS + 4, y);
    let z = ee::mul(fx.mem.f(v + 8), f21);
    fx.mem.setf(sp + POS + 8, z);
    fx.vu_add4(sp + POS, sp + POS, base);
}

/// The sprite about sp + POS projected and submitted keyed by its mean w;
/// returns a0..a3 as the last callee leaves them.
fn draw(fx: &mut Fx, sp: u32, ra: u32) -> [u32; 4] {
    fx.vu_add4(sp + P0, sp + C0, sp + POS);
    fx.vu_add4(sp + P1, sp + C1, sp + POS);
    let d = sp + DESC;
    if crate::fxlib2::project(fx, sp + P0, PROJ_OUT, sp + VP, d, sp + W) {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra);
        fx.submit(d, OT, key as i32);
        [d, OT, key, 0]
    } else {
        [sp + P0, PROJ_OUT, sp + VP, d]
    }
}

/// clamp(n - lo, 0, c) clamp(hi - n, 0, c) / c^2; and the second
/// 0x101050's a0..a3 (a3 the camera's).
fn window(fx: &mut Fx, lo: i32, hi: i32, c: i32) -> (f32, [u32; 4]) {
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(n - lo, 0, c);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(hi - n, 0, c);
    let f = ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), ee::from_i32(c * c));
    let a3 = camera_a3(fx);
    (f, [b as u32, 0, c as u32, a3])
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..6u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    if fx.mem.i32(D) == 0x1E {
        fx.vu_add4(D + 0xB0, D + 0x50, D + 0xA0);
        fx.vu_scale4(D + 0xB0, D + 0xB0, 0.5);
        fx.vu_sub4(D + 0xC0, TARGET, D + 0xB0);
    }
    pool_a(fx, sp);
    pool_b(fx, sp);
    pool_c(fx, sp);
    pool_d(fx, sp);
    let args = pool_e(fx, sp);
    // The fade of a destroyed target from 0x85 [F 0x1E01CB8..0x1E01D1C].
    if fx.outcome(args) == 1 && fx.mem.i32(D) >= 0x85 {
        let n = fx.mem.i32(D);
        let c = Fx::clamp_i(n - 0x84, 0, 0x20);
        fx.mem.write_u8(sp + FADE + 2, 0x80);
        fx.mem.write_u8(sp + FADE + 1, 0x80);
        fx.mem.write_u8(sp + FADE, 0x80);
        fx.mem.write_u8(sp + FADE + 3, ((c << 7) >> 5) as u8);
        let t = fx.mem.i32(D + 8);
        fx.unit_add_mode(t, 2, sp + FADE, [u64::from(D + 0x800), 0x40], 0x01E0_1D20);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x6A {
        ret = 1;
    } else {
        if (0x75..0xFA).contains(&n) {
            ret = 6;
        }
        if n == 0xFA {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xFC {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// Pool A [F 0x1E00360..0x1E008D4].
fn pool_a(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0xFA {
        let g = D + 0x3A0 * rem2(n, 8) as u32;
        fx.mem.write_u8(g + 0xD0, 1);
        fx.mem.write_u32(g + 0xD4, 0);
        for i in 0..6u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(D + 0x50 + 0x10 * i + 4 * c);
                fx.mem.write_u32(g + 0x110 + 0x10 * i + 4 * c, w);
            }
            for j in 0..8u32 {
                let r = fx.rand_from(j, 0x01E0_0400) >> 23;
                fx.mem.set8(g + 0xD8 + 8 * i + j, rem2(r, 4));
                fx.random_table_vec(g + 0x170 + 0x80 * i + 0x10 * j, f32::from_bits(0x3E99_999A));
            }
        }
    }
    head(fx, sp, 0x10);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(n, 0, 0x10);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(0xFA - n, 0, 0x10);
    let f20 = ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), 256.0);
    for k in 0..8u32 {
        let g = D + 0x3A0 * k;
        if fx.mem.read_u8(g + 0xD0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xD4);
        let s = fx.tsin((age << 15) >> 3);
        let al = fixu(fx, ee::mul(f20, ee::mul(80.0, s)), 0x01E0_05A0);
        rgba(fx, sp, [0x80, 0x30, 0x20], al);
        let age = fx.mem.i32(g + 0xD4);
        corners(fx, sp, size(5.0, 0x3E4C_CCCD, 0x3F4C_CCCD, age, 8.0));
        let age = ee::from_i32(fx.mem.i32(g + 0xD4));
        let f3 = ee::div(age, 8.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), age), 8.0);
        let f21 = ee::add(f32::from_bits(0x3F66_6666), f2);
        let f22 = ee::add(-4.0, ee::mul(10.0, f3));
        for i in 0..6u32 {
            for j in 0..8u32 {
                let b = fx.mem.read_u8(g + 0xD8 + 8 * i + j);
                table_uvs(fx, sp, b);
                place(fx, sp, g + 0x170 + 0x80 * i + 0x10 * j, f21, f22, g + 0x110 + 0x10 * i);
                draw(fx, sp, 0x01E0_0820);
            }
        }
        let age = fx.mem.i32(g + 0xD4) + 1;
        fx.mem.set32(g + 0xD4, age);
        if age >= 9 {
            fx.mem.write_u8(g + 0xD0, 0);
        }
    }
}

/// Pool B [F 0x1E008D8..0x1E00D08].
fn pool_b(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if rem2(n, 4) == 0 && n > 0 && n < 0xFA {
        let c = fx.mem.i32(D + 0x1DD0);
        let g = D + 0xD0 * c as u32;
        fx.mem.write_u8(g + 0x1DE0, 1);
        fx.mem.write_u32(g + 0x1DE4, 0);
        for i in 0..6u32 {
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x50 + 0x10 * i + 4 * w);
                fx.mem.write_u32(g + 0x1DF0 + 0x10 * i + 4 * w, x);
            }
            fx.random_table_vec(g + 0x1E50 + 0x10 * i, f32::from_bits(0x3E99_999A));
        }
        let c = fx.mem.i32(D + 0x1DD0) + 1;
        fx.mem.set32(D + 0x1DD0, c);
        if c >= 8 {
            fx.mem.write_u32(D + 0x1DD0, 0);
        }
    }
    head(fx, sp, 0x14);
    set_uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(n, 0, 0x10);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(0xFA - n, 0, 0x10);
    let f21 = ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), 256.0);
    for k in 0..8u32 {
        let g = D + 0xD0 * k;
        if fx.mem.read_u8(g + 0x1DE0) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x1DE4);
        let s = fx.tsin((age << 15) / 24);
        let al = fixu(fx, ee::mul(f21, ee::mul(128.0, s)), 0x01E0_0B48);
        rgba(fx, sp, [0xFF, 0x18, 0x18], al);
        let age = fx.mem.i32(g + 0x1DE4);
        corners(fx, sp, size(10.0, 0x3F00_0000, 0x3F00_0000, age, 24.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x1DE4));
        let f4 = ee::div(age, 24.0);
        let f20 = ee::mul(6.0, f4);
        let f22 = ee::add(0.5, ee::div(ee::mul(0.5, age), 24.0));
        for i in 0..6u32 {
            place(fx, sp, g + 0x1E50 + 0x10 * i, f22, f20, g + 0x1DF0 + 0x10 * i);
            draw(fx, sp, 0x01E0_0CAC);
        }
        let age = fx.mem.i32(g + 0x1DE4) + 1;
        fx.mem.set32(g + 0x1DE4, age);
        if age >= 0x19 {
            fx.mem.write_u8(g + 0x1DE0, 0);
        }
    }
}

/// Pool C [F 0x1E00D0C..0x1E0137C]: a head per record leaving a puff.
fn pool_c(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x1F && n < 0x3E {
        let g = D + 0x380 * rem2(n, 32) as u32;
        fx.mem.write_u8(g + 0x2460, 1);
        fx.mem.write_u32(g + 0x2464, 0);
        for i in 0..0x10u32 {
            fx.mem.write_u8(g + 0x2468 + i, 0);
        }
    }
    head(fx, sp, 0x10);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0xC2 - n, 0, 0x20)), 32.0);
    for k in 0..0x20u32 {
        let g = D + 0x380 * k;
        if fx.mem.read_u8(g + 0x2460) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x2464);
        let s5 = rem2(age, 16) as u32;
        fx.mem.write_u8(g + 0x2468 + s5, 1);
        fx.mem.write_u32(g + 0x2478 + 4 * s5, 0);
        let age = ee::from_i32(fx.mem.i32(g + 0x2464));
        let f = ee::div(ee::mul(2.0, age), 164.0);
        let c = g + 0x24E0 + 0x10 * s5;
        fx.vu_scale4(c, D + 0xC0, f);
        fx.vu_add4(c, c, D + 0xB0);
        for j in 0..2u32 {
            let r = fx.rand_from(sp + UV1, 0x01E0_0F20) >> 23;
            fx.mem.set8(g + 0x24B8 + 2 * s5 + j, rem2(r, 4));
            fx.random_table_vec(g + 0x25E0 + 0x20 * s5 + 0x10 * j, f32::from_bits(0x3E99_999A));
        }
        let age = fx.mem.i32(g + 0x2464);
        let a = Fx::clamp_i(age, 0, 0x10);
        let age = fx.mem.i32(g + 0x2464);
        let b = Fx::clamp_i(0xA4 - age, 0, 0x10);
        let f21 = ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), 256.0);
        for i in 0..0x10u32 {
            if fx.mem.read_u8(g + 0x2468 + i) == 0 {
                continue;
            }
            let age = fx.mem.i32(g + 0x2478 + 4 * i);
            let s = fx.tsin((age << 15) >> 4);
            let f0 = ee::mul(f21, ee::mul(80.0, s));
            let al = fixu(fx, ee::mul(f20, f0), 0x01E0_1010);
            rgba(fx, sp, [0x80, 0x30, 0x20], al);
            let age = fx.mem.i32(g + 0x2478 + 4 * i);
            corners(fx, sp, size(4.0, 0x3E4C_CCCD, 0x3F4C_CCCD, age, 16.0));
            let age = ee::from_i32(fx.mem.i32(g + 0x2478 + 4 * i));
            let f3 = ee::div(age, 16.0);
            let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), age), 16.0);
            let f22 = ee::add(f32::from_bits(0x3F66_6666), f2);
            let f23 = ee::add(-8.0, ee::mul(10.0, f3));
            for j in 0..2u32 {
                let b = fx.mem.read_u8(g + 0x24B8 + 2 * i + j);
                table_uvs(fx, sp, b);
                place(fx, sp, g + 0x25E0 + 0x20 * i + 0x10 * j, f22, f23, g + 0x24E0 + 0x10 * i);
                draw(fx, sp, 0x01E0_1280);
            }
            let age = fx.mem.i32(g + 0x2478 + 4 * i) + 1;
            fx.mem.set32(g + 0x2478 + 4 * i, age);
            if age >= 0x11 {
                fx.mem.write_u8(g + 0x2468 + i, 0);
            }
        }
        let age = fx.mem.i32(g + 0x2464) + 1;
        fx.mem.set32(g + 0x2464, age);
        if age >= 0xA5 {
            fx.mem.write_u8(g + 0x2460, 0);
        }
    }
}

/// Pool D [F 0x1E01380..0x1E018CC].
fn pool_d(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x75 && n < 0xFA {
        let g = D + 0x230 * rem2(n, 16) as u32;
        fx.mem.write_u8(g + 0x9460, 1);
        fx.mem.write_u32(g + 0x9464, 0);
        for j in 0..0x20u32 {
            let r = fx.rand_from(sp + UV1, 0x01E0_13FC) >> 23;
            fx.mem.set8(g + 0x9468 + j, rem2(r, 4));
            fx.random_table_vec(g + 0x9490 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    head(fx, sp, 0x10);
    camera(fx, sp);
    let (f20, _) = window(fx, 0x74, 0xFA, 0x20);
    for k in 0..0x10u32 {
        let g = D + 0x230 * k;
        if fx.mem.read_u8(g + 0x9460) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x9464);
        let s = fx.tsin((age << 15) >> 4);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), 0x01E0_1590);
        rgba(fx, sp, [0x80, 0x30, 0x20], al);
        let age = fx.mem.i32(g + 0x9464);
        corners(fx, sp, size(10.0, 0x3E4C_CCCD, 0x3F4C_CCCD, age, 16.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x9464));
        let f3 = ee::div(age, 16.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), age), 16.0);
        let f21 = ee::add(f32::from_bits(0x3F66_6666), f2);
        let f22 = ee::add(-25.0, ee::mul(50.0, f3));
        for j in 0..0x20u32 {
            let b = fx.mem.read_u8(g + 0x9468 + j);
            table_uvs(fx, sp, b);
            place(fx, sp, g + 0x9490 + 0x10 * j, f21, f22, TARGET);
            draw(fx, sp, 0x01E0_1834);
        }
        let age = fx.mem.i32(g + 0x9464) + 1;
        fx.mem.set32(g + 0x9464, age);
        if age >= 0x11 {
            fx.mem.write_u8(g + 0x9460, 0);
        }
    }
}

/// Pool E [F 0x1E018D0..0x1E01CB4]; returns a0..a3 as the last callee
/// before 0x1F8100 leaves them.
fn pool_e(fx: &mut Fx, sp: u32) -> [u32; 4] {
    let n = fx.mem.i32(D);
    if n >= 0x75 && n < 0xFA {
        let g = D + 0x20 * rem2(n, 64) as u32;
        fx.mem.write_u8(g + 0xB760, 1);
        fx.mem.write_u32(g + 0xB764, 0);
        fx.random_table_vec(g + 0xB770, 1.5);
    }
    head(fx, sp, 0x14);
    set_uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    let (f20, mut last) = window(fx, 0x74, 0xFA, 0x20);
    for k in 0..0x40u32 {
        let g = D + 0x20 * k;
        if fx.mem.read_u8(g + 0xB760) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0xB764);
        let s = fx.tsin((age << 15) >> 6);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), 0x01E0_1AC0);
        rgba(fx, sp, [0xFF, 0x18, 0x18], al);
        let age = fx.mem.i32(g + 0xB764);
        corners(fx, sp, size(20.0, 0x3F00_0000, 0x3F00_0000, age, 64.0));
        let age = ee::from_i32(fx.mem.i32(g + 0xB764));
        let f6 = ee::div(age, 64.0);
        let f1 = ee::add(-25.0, ee::mul(50.0, f6));
        let f3 = ee::add(0.5, ee::div(ee::mul(0.5, age), 64.0));
        place(fx, sp, g + 0xB770, f3, f1, TARGET);
        last = draw(fx, sp, 0x01E0_1C54);
        let age = fx.mem.i32(g + 0xB764) + 1;
        fx.mem.set32(g + 0xB764, age);
        if age >= 0x41 {
            fx.mem.write_u8(g + 0xB760, 0);
        }
    }
    last
}
