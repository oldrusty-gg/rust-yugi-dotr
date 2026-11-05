//! BMODULE b141 (card 88), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080 in a 0x310 frame (s7 = D, s0 = the
//! ordering table, s1 = the param), data area a0 = 0x1ED0000, 0xA070
//! bytes) [F]. Six sprite pools drawn through
//! 0x204F70 / 0x206F30 (descriptor sp + 0x2B0, UVs from the table
//! 0x28AE70 or the whole 0x800 cell), then the outcome boundary. Every
//! update the camera turn with roll (sp + 0x220), the attacker's nodes
//! 0x2D / 0x40 (D+0x40 / D+0x50) and their midpoint (D+0x60); the target's
//! point (0x1F65D0 kind 1) is D+0x20 from the init. Pools: A (D+0x70, 16 x
//! 0x140, spawned 1..0x103, 16 updates) 2 x 8 table sprites at the nodes
//! rising from -4 by 10 (80, 20, 10), far Z below 0x78; B (D+0x1470, 32 x
//! 0x50, 1..0x103, 32 updates) 2 whole-cell sprites at the nodes rising
//! from -2 by 8 (FF, 1C, 18); C (D+0x1E70, 48 x 0x110, 0x6F..0x103, 48
//! updates) 8 sprites from round the midpoint towards round the target
//! (80, 26, 14); D (D+0x5170, 16 x 0x230, 0x8D..0x153, 16 updates) 32
//! sprites at the target rising from -25 by 50 (80, 26, 16); E (D+0x7470,
//! 64 x 0x20, 0x8D..0x153, 64 updates) one whole-cell sprite at the target
//! (FF, 1C, 18); F (D+0x7C70, 32 x 0x120, 0x12D..0x171, 32 updates) 16
//! sprites at the target (0A, 0A, 0A, preset 0). Then 0x1F8100 on the
//! last callee's a0..a3 and at 1 from 0x9D 0x1F77A0(D+8, 2, (80, 80, 80,
//! 4 clamp(n - 0x9C, 0, 0x20))). Returns 1 on 0x82, 6 on 0x8D..0x167, 3 on
//! 0x168, 8 once the count reaches 0x173.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// The UV table 0x28AE70 (four (u0, u1, v0, v1) halfword cells).
const UV_TABLE: u32 = 0x0028_AE70;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x310;
/// The camera's view-projection and the identity it is multiplied by.
const VP: u32 = 0x1A0;
const ID: u32 = 0x1E0;
/// The camera turn with roll.
const TURN: u32 = 0x220;
/// The corner offsets, the projected corners and the sprite's centre.
const C0: u32 = 0x260;
const C1: u32 = 0x270;
const P0: u32 = 0x280;
const P1: u32 = 0x290;
const POS: u32 = 0x2A0;
/// The sprite descriptor, its RGBAQ, UV0 / XYZ2 / UV1 / XYZ2.
const DESC: u32 = 0x2B0;
const RGBA: u32 = 0x2C0;
const UV0: u32 = 0x2C8;
const XYZ0: u32 = 0x2D0;
const UV1: u32 = 0x2D8;
const XYZ1: u32 = 0x2E0;
/// The mean-w slot and the fade's colour bytes.
const W: u32 = 0x308;
const FADE: u32 = 0x30C;
/// The target's point.
const TARGET: u32 = D + 0x20;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp, param) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0xA070);
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
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x2D, 1);
    fx.mem.set32(D + 0x30, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x40, 0);
    fx.mem.set32(D + 0x34, r);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x70 + 0x140 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x1470 + 0x50 * k, 0);
    }
    for k in 0..0x30u32 {
        fx.mem.write_u8(D + 0x1E70 + 0x110 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x5170 + 0x230 * k, 0);
    }
    for k in 0..0x40u32 {
        fx.mem.write_u8(D + 0x7470 + 0x20 * k, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x7C70 + 0x120 * k, 0);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_add_mode(t, 2, 0, [u64::from(OT), param as u32 as u64], 0x01E0_023C);
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
/// D + `tex` (filter 1), ALPHA preset `preset`, CLAMP 5.
fn head(fx: &mut Fx, sp: u32, tex: u32, preset: i32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
}

/// The camera's view-projection x the identity at sp + VP.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + ID);
    mul_mem(fx, sp + VP, sp + ID);
}

/// a0..a3 as the camera's 0x20B370 leaves them (0x20E2B0's a2 / a3).
fn camera_regs(fx: &mut Fx, sp: u32) -> [u32; 4] {
    let idx = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    let cam = crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0)) + 0x40;
    [sp + VP, sp + ID, 0, cam]
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

/// The sprite about sp + POS projected and submitted (keyed by its mean w,
/// or far Z at key 0); returns a0..a3 as the last callee leaves them.
fn draw(fx: &mut Fx, sp: u32, far: bool, ra: u32) -> [u32; 4] {
    fx.vu_add4(sp + P0, sp + C0, sp + POS);
    fx.vu_add4(sp + P1, sp + C1, sp + POS);
    let d = sp + DESC;
    if crate::fxlib2::project(fx, sp + P0, PROJ_OUT, sp + VP, d, sp + W) {
        if far {
            crate::lens::far_z(fx, sp + XYZ0, &[sp + XYZ1]);
            fx.submit(d, OT, 0);
            [d, OT, 0, 0]
        } else {
            let w = fx.mem.f(sp + W);
            let key = fixu(fx, w, ra);
            fx.submit(d, OT, key as i32);
            [d, OT, key, 0]
        }
    } else {
        [sp + P0, PROJ_OUT, sp + VP, d]
    }
}

/// clamp(n - lo, 0, 32) clamp(hi - n, 0, 32) / 1024; and the second
/// 0x101050's a0..a3 (a3 the camera's).
fn window(fx: &mut Fx, sp: u32, lo: i32, hi: i32) -> (f32, [u32; 4]) {
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(n - lo, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(hi - n, 0, 0x20);
    let f = ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), 1024.0);
    let cam = camera_regs(fx, sp);
    (f, [b as u32, 0, 0x20, cam[3]])
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30 + 4 * k));
        fx.node_position(h, slot, D + 0x40 + 0x10 * k);
    }
    fx.vu_add4(D + 0x60, D + 0x40, D + 0x50);
    fx.vu_scale4(D + 0x60, D + 0x60, 0.5);
    pool_a(fx, sp);
    pool_b(fx, sp);
    pool_c(fx, sp);
    pool_d(fx, sp);
    pool_e(fx, sp);
    let args = pool_f(fx, sp);
    // The fade of a destroyed target from 0x9D [F 0x1E01D74..0x1E01DD0].
    if fx.outcome(args) == 1 && fx.mem.i32(D) >= 0x9D {
        let n = fx.mem.i32(D);
        let c = Fx::clamp_i(n - 0x9C, 0, 0x20);
        fx.mem.write_u8(sp + FADE + 2, 0x80);
        fx.mem.write_u8(sp + FADE + 1, 0x80);
        fx.mem.write_u8(sp + FADE, 0x80);
        fx.mem.write_u8(sp + FADE + 3, ((c << 7) >> 5) as u8);
        let t = fx.mem.i32(D + 8);
        fx.unit_add_mode(t, 2, sp + FADE, [u64::from(OT), param as u32 as u64], 0x01E0_1DD4);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x82 {
        ret = 1;
    } else {
        if (0x8D..0x168).contains(&n) {
            ret = 6;
        }
        if n == 0x168 {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x173 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}

/// Pool A [F 0x1E002A0..0x1E008A4].
fn pool_a(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x104 {
        let g = D + 0x140 * rem2(n, 16) as u32;
        fx.mem.write_u8(g + 0x70, 1);
        fx.mem.write_u32(g + 0x74, 0);
        for i in 0..2u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(D + 0x40 + 0x10 * i + 4 * c);
                fx.mem.write_u32(g + 0x90 + 0x10 * i + 4 * c, w);
            }
            for j in 0..8u32 {
                let r = fx.rand_from(OT, 0x01E0_0330) >> 23;
                fx.mem.set8(g + 0x78 + 8 * i + j, rem2(r, 4));
                fx.random_table_vec(g + 0xB0 + 0x80 * i + 0x10 * j, f32::from_bits(0x3E4C_CCCD));
            }
        }
    }
    head(fx, sp, 0x10, 1);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(n, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(0x104 - n, 0, 0x20);
    let f20 = ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), 1024.0);
    for k in 0..0x10u32 {
        let g = D + 0x140 * k;
        if fx.mem.read_u8(g + 0x70) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x74);
        let s = fx.tsin((age << 15) >> 4);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), 0x01E0_04C8);
        rgba(fx, sp, [0x80, 0x20, 0x10], al);
        let age = fx.mem.i32(g + 0x74);
        corners(fx, sp, size(3.0, 0x3E4C_CCCD, 0x3F4C_CCCD, age, 16.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x74));
        let f3 = ee::div(age, 16.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), age), 16.0);
        let f21 = ee::add(f32::from_bits(0x3F66_6666), f2);
        let f22 = ee::add(-4.0, ee::mul(10.0, f3));
        for i in 0..2u32 {
            for j in 0..8u32 {
                let b = fx.mem.read_u8(g + 0x78 + 8 * i + j);
                table_uvs(fx, sp, b);
                place(fx, sp, g + 0xB0 + 0x80 * i + 0x10 * j, f21, f22, g + 0x90 + 0x10 * i);
                let far = fx.mem.i32(D) < 0x78;
                draw(fx, sp, far, 0x01E0_07EC);
            }
        }
        let age = fx.mem.i32(g + 0x74) + 1;
        fx.mem.set32(g + 0x74, age);
        if age >= 0x11 {
            fx.mem.write_u8(g + 0x70, 0);
        }
    }
}

/// Pool B [F 0x1E008A8..0x1E00C84].
fn pool_b(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x104 {
        let g = D + 0x50 * rem2(n, 32) as u32;
        fx.mem.write_u8(g + 0x1470, 1);
        fx.mem.write_u32(g + 0x1474, 0);
        for i in 0..2u32 {
            for c in 0..4u32 {
                let w = fx.mem.read_u32(D + 0x40 + 0x10 * i + 4 * c);
                fx.mem.write_u32(g + 0x1480 + 0x10 * i + 4 * c, w);
            }
            fx.random_table_vec(g + 0x14A0 + 0x10 * i, f32::from_bits(0x3E99_999A));
        }
    }
    head(fx, sp, 0x14, 1);
    set_uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(n, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(0x104 - n, 0, 0x20);
    let f20 = ee::div(ee::mul(ee::from_i32(a), ee::from_i32(b)), 1024.0);
    for k in 0..0x20u32 {
        let g = D + 0x50 * k;
        if fx.mem.read_u8(g + 0x1470) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x1474);
        let s = fx.tsin((age << 15) >> 5);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), 0x01E0_0AA8);
        rgba(fx, sp, [0xFF, 0x1C, 0x18], al);
        let age = fx.mem.i32(g + 0x1474);
        corners(fx, sp, size(6.0, 0x3F00_0000, 0x3F00_0000, age, 32.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x1474));
        let f3 = ee::div(age, 32.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3E99_999A), age), 32.0);
        let f21 = ee::add(f32::from_bits(0x3F33_3333), f2);
        let f22 = ee::add(-2.0, ee::mul(8.0, f3));
        for i in 0..2u32 {
            place(fx, sp, g + 0x14A0 + 0x10 * i, f21, f22, g + 0x1480 + 0x10 * i);
            draw(fx, sp, false, 0x01E0_0C34);
        }
        let age = fx.mem.i32(g + 0x1474) + 1;
        fx.mem.set32(g + 0x1474, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0x1470, 0);
        }
    }
}

/// Pool C [F 0x1E00C88..0x1E01100].
fn pool_c(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x6F && n < 0x104 {
        let (_, r) = fx.div(n, 0x30);
        let g = D + 0x110 * r as u32;
        fx.mem.write_u8(g + 0x1E70, 1);
        fx.mem.write_u32(g + 0x1E74, 0);
        for j in 0..8u32 {
            let r = fx.rand_from(OT, 0x01E0_0CE0) >> 23;
            fx.mem.set8(g + 0x1E78 + j, rem2(r, 4));
            let a = g + 0x1E80 + 0x10 * j;
            fx.random_table_vec(a, f32::from_bits(0x3E4C_CCCD));
            fx.vu_add4(a, a, D + 0x60);
            let b = g + 0x1F00 + 0x10 * j;
            fx.random_table_vec(b, 1.0);
            fx.vu_add4(b, b, TARGET);
            fx.vu_sub4(b, b, a);
        }
    }
    head(fx, sp, 0x10, 1);
    camera(fx, sp);
    let (f20, _) = window(fx, sp, 0x6E, 0x104);
    for k in 0..0x30u32 {
        let g = D + 0x110 * k;
        if fx.mem.read_u8(g + 0x1E70) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(g + 0x1E74));
        let r = Fx::ramp_above(age, 48.0, 24.0);
        let al = fixu(fx, ee::mul(f20, ee::mul(100.0, r)), 0x01E0_0E8C);
        rgba(fx, sp, [0x80, 0x26, 0x14], al);
        let age = fx.mem.i32(g + 0x1E74);
        corners(fx, sp, size(15.0, 0x3E4C_CCCD, 0x3F4C_CCCD, age, 48.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x1E74));
        let f21 = ee::div(ee::mul(1.5, age), 48.0);
        for j in 0..8u32 {
            let b = fx.mem.read_u8(g + 0x1E78 + j);
            table_uvs(fx, sp, b);
            fx.vu_scale4(sp + POS, g + 0x1F00 + 0x10 * j, f21);
            fx.vu_add4(sp + POS, sp + POS, g + 0x1E80 + 0x10 * j);
            draw(fx, sp, false, 0x01E0_109C);
        }
        let age = fx.mem.i32(g + 0x1E74) + 1;
        fx.mem.set32(g + 0x1E74, age);
        if age >= 0x31 {
            fx.mem.write_u8(g + 0x1E70, 0);
        }
    }
}

/// Pool D [F 0x1E01104..0x1E0158C].
fn pool_d(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x8D && n < 0x154 {
        let g = D + 0x230 * rem2(n, 16) as u32;
        fx.mem.write_u8(g + 0x5170, 1);
        fx.mem.write_u32(g + 0x5174, 0);
        for j in 0..0x20u32 {
            let r = fx.rand_from(OT, 0x01E0_1168) >> 23;
            fx.mem.set8(g + 0x5178 + j, rem2(r, 4));
            fx.random_table_vec(g + 0x51A0 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    head(fx, sp, 0x10, 1);
    camera(fx, sp);
    let (f20, _) = window(fx, sp, 0x8C, 0x154);
    for k in 0..0x10u32 {
        let g = D + 0x230 * k;
        if fx.mem.read_u8(g + 0x5170) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x5174);
        let s = fx.tsin((age << 15) >> 4);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), 0x01E0_12C8);
        rgba(fx, sp, [0x80, 0x26, 0x16], al);
        let age = fx.mem.i32(g + 0x5174);
        corners(fx, sp, size(10.0, 0x3E4C_CCCD, 0x3F4C_CCCD, age, 16.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x5174));
        let f3 = ee::div(age, 16.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), age), 16.0);
        let f21 = ee::add(f32::from_bits(0x3F66_6666), f2);
        let f22 = ee::add(-25.0, ee::mul(50.0, f3));
        for j in 0..0x20u32 {
            let b = fx.mem.read_u8(g + 0x5178 + j);
            table_uvs(fx, sp, b);
            place(fx, sp, g + 0x51A0 + 0x10 * j, f21, f22, TARGET);
            draw(fx, sp, false, 0x01E0_1528);
        }
        let age = fx.mem.i32(g + 0x5174) + 1;
        fx.mem.set32(g + 0x5174, age);
        if age >= 0x11 {
            fx.mem.write_u8(g + 0x5170, 0);
        }
    }
}

/// Pool E [F 0x1E0158C..0x1E018FC].
fn pool_e(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x8D && n < 0x154 {
        let g = D + 0x20 * rem2(n, 64) as u32;
        fx.mem.write_u8(g + 0x7470, 1);
        fx.mem.write_u32(g + 0x7474, 0);
        fx.random_table_vec(g + 0x7480, 2.0);
    }
    head(fx, sp, 0x14, 1);
    set_uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
    let (f20, _) = window(fx, sp, 0x8C, 0x154);
    for k in 0..0x40u32 {
        let g = D + 0x20 * k;
        if fx.mem.read_u8(g + 0x7470) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x7474);
        let s = fx.tsin((age << 15) >> 6);
        let al = fixu(fx, ee::mul(f20, ee::mul(128.0, s)), 0x01E0_1750);
        rgba(fx, sp, [0xFF, 0x1C, 0x18], al);
        let age = fx.mem.i32(g + 0x7474);
        corners(fx, sp, size(20.0, 0x3F00_0000, 0x3F00_0000, age, 64.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x7474));
        let f6 = ee::div(age, 64.0);
        let f1 = ee::add(-25.0, ee::mul(50.0, f6));
        let f3 = ee::add(0.5, ee::div(ee::mul(0.5, age), 64.0));
        place(fx, sp, g + 0x7480, f3, f1, TARGET);
        draw(fx, sp, false, 0x01E0_18C0);
        let age = fx.mem.i32(g + 0x7474) + 1;
        fx.mem.set32(g + 0x7474, age);
        if age >= 0x41 {
            fx.mem.write_u8(g + 0x7470, 0);
        }
    }
}

/// Pool F [F 0x1E01900..0x1E01D70]; returns a0..a3 as the last callee
/// before 0x1F8100 leaves them.
fn pool_f(fx: &mut Fx, sp: u32) -> [u32; 4] {
    let n = fx.mem.i32(D);
    if n >= 0x12D && n < 0x172 {
        let g = D + 0x120 * rem2(n, 32) as u32;
        fx.mem.write_u8(g + 0x7C70, 1);
        fx.mem.write_u32(g + 0x7C74, 0);
        for j in 0..0x10u32 {
            let r = fx.rand_from(OT, 0x01E0_195C) >> 23;
            fx.mem.set8(g + 0x7C78 + j, rem2(r, 4));
            fx.random_table_vec(g + 0x7C90 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    head(fx, sp, 0x10, 0);
    camera(fx, sp);
    let (f20, mut last) = window(fx, sp, 0x12C, 0x172);
    for k in 0..0x20u32 {
        let g = D + 0x120 * k;
        if fx.mem.read_u8(g + 0x7C70) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + 0x7C74);
        let s = fx.tsin((age << 15) >> 5);
        let al = fixu(fx, ee::mul(f20, ee::mul(200.0, s)), 0x01E0_1AB8);
        rgba(fx, sp, [0x0A; 3], al);
        let age = fx.mem.i32(g + 0x7C74);
        corners(fx, sp, size(10.0, 0x3E4C_CCCD, 0x3F4C_CCCD, age, 32.0));
        let age = ee::from_i32(fx.mem.i32(g + 0x7C74));
        let f3 = ee::div(age, 32.0);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), age), 32.0);
        let f21 = ee::add(f32::from_bits(0x3F66_6666), f2);
        let f22 = ee::add(-25.0, ee::mul(50.0, f3));
        for j in 0..0x10u32 {
            let b = fx.mem.read_u8(g + 0x7C78 + j);
            table_uvs(fx, sp, b);
            place(fx, sp, g + 0x7C90 + 0x10 * j, f21, f22, TARGET);
            last = draw(fx, sp, false, 0x01E0_1D10);
        }
        let age = fx.mem.i32(g + 0x7C74) + 1;
        fx.mem.set32(g + 0x7C74, age);
        if age >= 0x21 {
            fx.mem.write_u8(g + 0x7C70, 0);
        }
    }
    last
}
