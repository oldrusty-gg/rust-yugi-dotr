//! BMODULE b151 (card 556), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x560 frame with D in s7 and the
//! OT in s0, data area a0 = 0x1ED0000, 0x9920 bytes; one attacker post
//! callback 0x1E027B0 with its static D at 0x1E02A00) [F] (helper A32,
//! 3 Nov 2026). Each update the point D+0x50 swings from the attacker
//! (D+0x30) to the target's point (D+0x40) on 0x32..0x63 and back on
//! 0xD0..0xFA (x replaced by 10 sin, y 36); from 0xA for 0x10F updates the
//! attacker darkened (tint (0, 0, 0, 80)) and drawn into its target, the
//! callback laying the frame over it as a (FF, 80, 80) window sprite; four
//! bursts (D+0x4300, 0x90: 6 turned eight-segment frame-textured fans) on
//! 0x7D / 0x87 / 0x91 / 0x9B; from 0x14 for 0x105 updates a breathing
//! 16 x 33 ring tube at D+0x50 (D+0x70, (FF, 30, 30) quads with scrolling
//! ST, a turned copy D+0x2170 through the frame copied as a texture) and
//! every fifth update of 0x33..0x62 / 0xD1..0xF9 a shrinking 8-ring echo
//! of it (D+0x9710, 16 x 0x20, rings D+0x8690); sparks (D+0x4840, 18 x
//! 0x340, 48 falling sprites) and puffs (D+0x82C0, 12 x 0x50) at the target
//! on 0x81..0xC7; returns 1 / 6 / 2 / 8 on 0x76 / 0x81..0xC7 / 0xC8 /
//! 0x119.
use crate::b182::{alpha, head, rgba, uv};
use crate::b440::{copy8, fix, fixs, rem2};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x560;
const CB: u32 = 0x01E0_27B0;
const D_STATIC: u32 = 0x01E0_2A00;
/// The pass flag / age (D+0x9910 / 0x9914).
const PASS: u32 = 0x9910;
/// The echo cursor (D+0x8680).
const CURSOR: u32 = 0x8680;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 of b151 (card 556).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp, param) };
    fx.leave(FRAME);
    ret
}

/// rand() >> 23 from `ra` (the caller's s0 = the OT).
fn rnd(fx: &mut Fx, ra: u32) -> i32 {
    fx.rand_from(OT, ra) >> 23
}

fn vec4(fx: &mut Fx, p: u32, x: f32, y: f32, z: f32) {
    fx.mem.setf(p, x);
    fx.mem.setf(p + 4, y);
    fx.mem.setf(p + 8, z);
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x9920);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for id in 0..5u32 {
        let r = fx.texture(5, id as i32);
        fx.mem.write_u32(D + 0x10 + 4 * id, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x40);
    fx.mem.write_u8(D + 0x60, 0);
    // The nine-point fan rim (radius 1.5).
    for k in 0..9i32 {
        let a = (k << 16) >> 3;
        let p = D + 0x4270 + 0x10 * k as u32;
        let sn = fx.tsin(a);
        fx.mem.setf(p, ee::mul(1.5, sn));
        fx.mem.write_u32(p + 4, 0);
        let cs = fx.tcos(a);
        fx.mem.setf(p + 8, ee::mul(1.5, cs));
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x4300 + 0x90 * k, 0);
    }
    for k in 0..0x30u32 {
        fx.random_table_vec(D + 0x4540 + 0x10 * k, 1.5);
    }
    for k in 0..0x12u32 {
        fx.mem.write_u8(D + 0x4840 + 0x340 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x82C0 + 0x50 * k, 0);
    }
    fx.mem.write_u32(D + CURSOR, 0);
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x9710 + 0x20 * k, 0);
    }
    fx.mem.write_u8(D + PASS, 0);
    0
}

/// 0x1F7600(attacker, mode, c) from `ra` (its frame keeps the caller's s0
/// = the OT and `s1`).
fn tint(fx: &mut Fx, mode: i32, c: u32, s1: u64, ra: u32) {
    let h = fx.mem.i32(D + 4);
    fx.unit_tint_mode(h, mode, c, [u64::from(OT), s1], ra);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.camera_turn_roll(sp + 0x300);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    swing(fx, sp);
    if fx.mem.i32(D) == 0xA {
        fx.mem.write_u8(D + PASS, 1);
        fx.mem.write_u32(D + PASS + 4, 0);
    }
    let s1 = i64::from(param) as u64;
    tint(fx, 0, 0, s1, 0x01E0_0494);
    if fx.mem.read_u8(D + PASS) == 1 {
        unit_pass(fx, sp, s1);
    }
    bursts(fx, sp);
    tube(fx, sp);
    sparks(fx, sp);
    puffs(fx, sp);
    echoes(fx, sp);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xB0);
    if n == 0x76 {
        ret = 1;
    } else {
        if n >= 0x81 && n < 0xC8 {
            ret = 6;
        }
        if n == 0xC8 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x119 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xB0, ret);
    ret
}

/// D+0x50: from the attacker (D+0x30) to the target's point (D+0x40) by
/// 0.5 (1 - cos) of clamp(n - 0x32, 0, 0x31) / 49 below 0xD0, back by
/// clamp(n - 0xD0, 0, 0x2A) / 42 after; x then 10 sin of the whole turn,
/// y 36 [F 0x1E002B4..0x1E00454].
fn swing(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let (f20, from, to, ras) = if n < 0xD0 {
        let c = Fx::clamp_i(n - 0x32, 0, 0x31);
        (ee::div(ee::from_i32(c), 49.0), D + 0x30, D + 0x40, (0x01E0_0304, 0x01E0_0368))
    } else {
        let c = Fx::clamp_i(n - 0xD0, 0, 0x2A);
        (ee::div(ee::from_i32(c), 42.0), D + 0x40, D + 0x30, (0x01E0_03CC, 0x01E0_0430))
    };
    let a = fixs(fx, ee::mul(32768.0, f20), ras.0) & 0xFFFF;
    let cs = fx.tcos(a);
    let f21 = ee::mul(0.5, ee::sub(1.0, cs));
    fx.vu_sub4(sp + 0x3C0, to, from);
    fx.vu_scale4(sp + 0x3C0, sp + 0x3C0, f21);
    fx.vu_add4(D + 0x50, sp + 0x3C0, from);
    let a = fixs(fx, ee::mul(65536.0, f20), ras.1) & 0xFFFF;
    let sn = fx.tsin(a);
    fx.mem.setf(D + 0x50, ee::mul(10.0, sn));
    fx.mem.write_u32(D + 0x54, 0x4210_0000);
}

/// The attacker darkened and drawn into its target, composited with the
/// callback [F 0x1E00494..0x1E006B8].
fn unit_pass(fx: &mut Fx, sp: u32, s1: u64) {
    let c = sp + 0x55C;
    for (k, b) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(c + k as u32, b);
    }
    tint(fx, 1, c, s1, 0x01E0_04CC);
    let d = sp + 0x3E0;
    head(fx, d, 0x26, 5, 0x4_2421, 0);
    let layer = fx.layer_begin();
    // 0x1F7600 leaves a0 = a1 = 0, a2 = the colour, a3 = the object.
    let h = fx.mem.i32(D + 4);
    let i = fx.object_index(h >> 8, h & 0xFF);
    let o = fx.object_at(i);
    fx.texflush([0, 0, c, o]);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x420, sp + 0x428, CB, &mut |fx, p| callback(fx, p));
    fx.texflush(regs);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    rgba(fx, sp + 0x3F0, [0x80; 3], 0x80);
    uv(fx, sp + 0x3F8, 0);
    uv(fx, sp + 0x3FA, 0);
    uv(fx, sp + 0x408, 0x2000);
    uv(fx, sp + 0x40A, 0x2000);
    far_xy(fx, sp + 0x400, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x410, 0x8FF8, 0x8FF8);
    fx.submit(d, 0, 0);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + PASS + 4) + 1;
    fx.mem.set32(D + PASS + 4, age);
    if age >= 0x10F {
        fx.mem.write_u8(D + PASS, 0);
        tint(fx, 0, 0, u64::from(layer), 0x01E0_06BC);
    }
}

/// The bursts (D+0x4300, 4 x 0x90): six eight-segment fans (frame-textured,
/// rim D+0x4270, centre 30 up) turned by a random angle about y at D+0x50
/// plus a 0.5 table vector, scaled by ratio(age, 4) ramp(age, life, life -
/// 4) [F 0x1E006BC..0x1E00B54].
fn bursts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    let idx = match n {
        0x9B => 3,
        0x91 => 2,
        0x87 => 1,
        0x7D => 0,
        _ => -1,
    };
    if idx >= 0 {
        let r = D + 0x90 * idx as u32;
        fx.mem.write_u8(r + 0x4300, 1);
        fx.mem.write_u32(r + 0x4304, 0);
        fx.mem.write_u32(r + 0x4308, if idx == 3 { 0x31 } else { 0x10 });
        for k in 0..6u32 {
            let c = rem2(rnd(fx, 0x01E0_0774), 0x100);
            fx.mem.set32(r + 0x430C + 4 * k, (c << 16) >> 8);
            fx.random_table_vec(r + 0x4330 + 0x10 * k, 0.5);
            fx.mem.write_u32(r + 0x4334 + 0x10 * k, 0);
        }
    }
    let t = sp + 0x438;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    fx.frame_desc(t, 1, [t + 0x10, t + 2]);
    for b in 0..4u32 {
        let r = D + 0x90 * b;
        if fx.mem.read_u8(r + 0x4300) == 0 {
            continue;
        }
        let life = fx.mem.i32(r + 0x4308);
        let age = ee::from_i32(fx.mem.i32(r + 0x4304));
        let f20 = Fx::ramp_above(age, ee::from_i32(life), ee::from_i32(life - 4));
        let age = ee::from_i32(fx.mem.i32(r + 0x4304));
        let s = ee::mul(Fx::ratio_below(age, 4.0), f20);
        vec4(fx, sp + 0x3D0, s, s, s);
        for k in 0..6u32 {
            fx.camera_vp_to(sp + 0x280);
            fx.identity(sp + 0x2C0);
            fx.vu_add4(sp + 0x3C0, r + 0x4330 + 0x10 * k, D + 0x50);
            fx.translate_rows(sp + 0x2C0, sp + 0x2C0, sp + 0x3C0);
            let a = fx.mem.i32(r + 0x430C + 4 * k);
            fx.rotate_axis(sp + 0x2C0, a, 1);
            fx.scale_rows(sp + 0x2C0, sp + 0x3D0);
            crate::blib::exe::mul_mem(fx, sp + 0x280, sp + 0x2C0);
            fx.mem.write_u32(sp + 0x3C0, 0);
            fx.mem.write_u32(sp + 0x3C4, 0x41F0_0000);
            fx.mem.write_u32(sp + 0x3C8, 0);
            fx.mem.write_u32(sp + 0x3CC, 0x3F80_0000);
            for s1 in 0..8i32 {
                let sn = fx.tsin((s1 << 15) >> 3);
                let c0 = fix(fx, ee::mul(255.0, sn), 0x01E0_0964) & 0xFF;
                let g = ee::mul(64.0, sn);
                let c1 = fix(fx, g, 0x01E0_0980) & 0xFF;
                let c2 = fix(fx, g, 0x01E0_098C);
                rgba(fx, sp + 0x460, [c0 as u8, c1 as u8, c2 as u8], 0x80);
                copy8(fx, sp + 0x448, sp + 0x460);
                let sn = fx.tsin(((s1 + 1) << 15) >> 3);
                let c0 = fix(fx, ee::mul(255.0, sn), 0x01E0_09E4);
                let g = ee::mul(64.0, sn);
                let c1 = fix(fx, g, 0x01E0_0A00);
                let c2 = fix(fx, g, 0x01E0_0A0C);
                rgba(fx, sp + 0x478, [c0 as u8, c1 as u8, c2 as u8], 0x80);
                fx.mem.write_u32(sp + 0x450, 0x3F00_0000);
                fx.mem.write_u32(sp + 0x454, 0);
                let s = ee::div(ee::div(ee::mul(512.0, ee::from_i32(s1)), 8.0), 512.0);
                fx.mem.setf(sp + 0x468, s);
                fx.mem.write_u32(sp + 0x46C, 0x3F80_0000);
                let s = ee::div(ee::div(ee::mul(512.0, ee::from_i32(s1 + 1)), 8.0), 512.0);
                fx.mem.setf(sp + 0x480, s);
                fx.mem.write_u32(sp + 0x484, 0x3F80_0000);
                let vs = [sp + 0x3C0, D + 0x4270 + 0x10 * s1 as u32, D + 0x4280 + 0x10 * s1 as u32];
                if fx.project_n(&vs, sp + 0x280, t, sp + 0x554, 0x01E0_0AA0) == 1 {
                    let w = fx.mem.f(sp + 0x554);
                    let key = fix(fx, w, 0x01E0_0AB4);
                    fx.submit(t, OT, key as i32);
                }
            }
        }
        let age = fx.mem.i32(r + 0x4304) + 1;
        fx.mem.set32(r + 0x4304, age);
        if fx.mem.i32(r + 0x4308) < age {
            fx.mem.write_u8(r + 0x4300, 0);
        }
    }
}

fn quad_desc(fx: &mut Fx, q: u32) {
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
}

/// The 16 x 33 ring grid at D + `base`: ring k of radius 17.5 (n - k) / n
/// breathing by 0.9 + 0.1 sin(a) sin(b) over the age at `age_at` [F].
fn breathing_rings(fx: &mut Fx, base: u32, rings: i32, age_at: u32) {
    for k in 0..rings {
        let f21 = ee::div(ee::mul(17.5, ee::from_i32(rings - k)), ee::from_i32(rings));
        let rb = D + 0x210 * k as u32 + base;
        for s in 0..0x21i32 {
            let (s1, s2, s3) = (s.wrapping_mul(0x60000), s.wrapping_mul(0x20000), s << 16);
            let age = fx.mem.i32(age_at);
            let b = ((s2 >> 5).wrapping_add(age.wrapping_mul(0x457))) & 0xFFFF;
            let a = ((s1 >> 5).wrapping_add(age.wrapping_mul(0x1C8))) & 0xFFFF;
            let sa = fx.tsin(a);
            let f20 = ee::mul(f(0x3DCC_CCCD), sa);
            let sb = fx.tsin(b);
            let f20 = ee::mul(f21, ee::add(f(0x3F66_6666), ee::mul(f20, sb)));
            let p = rb + 0x10 * s as u32;
            let sn = fx.tsin(s3 >> 5);
            fx.mem.setf(p, ee::mul(f20, sn));
            fx.mem.write_u32(p + 4, 0);
            let cs = fx.tcos(s3 >> 5);
            fx.mem.setf(p + 8, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
}

/// The scrolling-ST quads of a breathing ring grid (D + `base`, `rings`
/// rings) into `q` with the colours at sp + 0x4C0.. (alpha `a` but 0 on the
/// outer ring), the last ring fanned to the centre sp + 0x3C0, keyed by the
/// centre's w (sp + 0x558); `ras` the four return addresses.
fn ring_quads(fx: &mut Fx, sp: u32, q: u32, base: u32, rings: i32, age: i32, a: u8, ras: [u32; 4]) -> [u32; 4] {
    let f20 = ee::div(ee::mul(0.5, ee::from_i32(rem2(age, 0x40))), 64.0);
    let mut last = [0u32; 4];
    let nf = ee::from_i32(rings);
    for s6 in 0..rings {
        let s3 = D + 0x210 * s6 as u32 + base;
        let al = if s6 == 0 { 0 } else { a };
        fx.mem.write_u8(sp + 0x4C3, al);
        fx.mem.write_u8(sp + 0x4DB, al);
        let f21 = ee::add(ee::div(ee::mul(0.5, ee::from_i32(s6)), nf), f20);
        let f22 = ee::add(ee::div(ee::mul(0.5, ee::from_i32(s6 + 1)), nf), f20);
        let fan = s6 == rings - 1;
        let us = if fan { 0.5 } else { 2.0 };
        for s2 in 0..0x20u32 {
            let f2 = ee::div(ee::mul(us, ee::from_i32(s2 as i32)), 32.0);
            let f0 = ee::div(ee::mul(us, ee::from_i32(s2 as i32 + 1)), 32.0);
            for (o, x) in [(0x4C8u32, f2), (0x4CC, f21), (0x4E0, f0), (0x4E4, f21), (0x4F8, f2), (0x4FC, f22), (0x510, f0), (0x514, f22)] {
                fx.mem.setf(sp + o, x);
            }
            let vs = if fan {
                [s3 + 0x10 * s2, s3 + 0x10 * (s2 + 1), sp + 0x3C0, sp + 0x3C0]
            } else {
                let s4 = s3 + 0x210;
                [s3 + 0x10 * s2, s3 + 0x10 * (s2 + 1), s4 + 0x10 * s2, s4 + 0x10 * (s2 + 1)]
            };
            let (pra, fra) = if fan { (ras[0], ras[1]) } else { (ras[2], ras[3]) };
            if fx.project_n(&vs, sp + 0x280, q, sp + 0x554, pra) == 1 {
                let w = fx.mem.f(sp + 0x558);
                let key = fix(fx, w, fra);
                fx.submit(q, OT, key as i32);
                last = [q, OT, key, q];
            } else {
                last = [fx.sp - 0x40, PROJ_OUT, sp + 0x280, q];
            }
        }
    }
    last
}

/// The camera at D+0x50's screen w (sp + 0x558), then translated to `t`.
fn camera_at(fx: &mut Fx, sp: u32, t: u32) {
    fx.camera_vp_to(sp + 0x280);
    fx.identity(sp + 0x2C0);
    crate::blib::exe::mul_mem(fx, sp + 0x280, sp + 0x2C0);
    let m = fx.read_m4(sp + 0x280);
    fx.project_at(D + 0x50, &m, sp + 0x540, sp + 0x558);
    fx.camera_vp_to(sp + 0x280);
    fx.identity(sp + 0x2C0);
    fx.translate_rows(sp + 0x2C0, sp + 0x2C0, t);
    crate::blib::exe::mul_mem(fx, sp + 0x280, sp + 0x2C0);
}

/// The ring tube at D+0x50 (D+0x60 from 0x14 for 0x105 updates) and its
/// refracting copy, spawning the echoes [F 0x1E00B58..0x1E01840].
fn tube(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x14 {
        fx.mem.write_u8(D + 0x60, 1);
        fx.mem.write_u32(D + 0x64, 0);
    }
    if fx.mem.read_u8(D + 0x60) != 1 {
        return;
    }
    camera_at(fx, sp, D + 0x50);
    let n = fx.mem.i32(D);
    if ((0x33..0x63).contains(&n) || (0xD1..0xFA).contains(&n)) && fx.div(n, 5).1 == 0 {
        let c = fx.mem.read_u32(D + CURSOR);
        let r = D + 0x20 * c;
        fx.mem.write_u8(r + 0x9710, 1);
        fx.mem.write_u32(r + 0x9714, 0);
        let age = fx.mem.read_u32(D + 0x64);
        fx.mem.write_u32(r + 0x9718, age);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(D + 0x50 + 4 * w);
            fx.mem.write_u32(r + 0x9720 + 4 * w, x);
        }
        let c = fx.mem.i32(D + CURSOR) + 1;
        fx.mem.set32(D + CURSOR, if c >= 0x10 { 0 } else { c });
    }
    breathing_rings(fx, 0x70, 0x10, D + 0x64);
    let q = sp + 0x4B0;
    quad_desc(fx, q);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    let age = ee::from_i32(fx.mem.i32(D + 0x64));
    let f20 = Fx::ratio_below(age, 30.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x64));
    let f0 = Fx::ramp_above(age, 260.0, 230.0);
    let a = fix(fx, ee::mul(255.0, ee::mul(f20, f0)), 0x01E0_0EC4) & 0xFF;
    rgba(fx, sp + 0x4C0, [0xFF, 0x30, 0x30], a as u8);
    for o in [0x4D8u32, 0x4F0, 0x508] {
        copy8(fx, sp + o, sp + 0x4C0);
    }
    vec4(fx, sp + 0x3C0, 0.0, 0.0, 0.0);
    let age = fx.mem.i32(D + 0x64);
    let last = ring_quads(fx, sp, q, 0x70, 0x10, age, a as u8, [0x01E0_10A4, 0x01E0_10B8, 0x01E0_1190, 0x01E0_11A4]);
    // The refracting copy.
    let t = sp + 0x438;
    head(fx, t, 0x33, 9, 0x4_2142_1421, 0);
    quad_desc(fx, q);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, [0x2142_1421, last[1], last[2], last[3]], 1, sp + 0x498, sp + 0x4A0);
    copy8(fx, sp + 0x528, sp + 0x498);
    copy8(fx, sp + 0x530, sp + 0x4A0);
    fx.texflush([sp + 0x528, regs[1], regs[2], regs[3]]);
    alpha(fx, t, 0);
    fx.desc_clamp(t, LENS_CLAMP);
    let fl = fx.mem.read_u16(t + 2);
    fx.mem.write_u16(t + 2, fl | 0x800);
    alpha(fx, q, 0);
    fx.desc_clamp(q, LENS_CLAMP);
    let fl = fx.mem.read_u16(q + 2);
    fx.mem.write_u16(q + 2, fl | 0x800);
    let age = ee::from_i32(fx.mem.i32(D + 0x64));
    let f20 = Fx::ratio_below(age, 30.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x64));
    let f0 = Fx::ramp_above(age, 260.0, 230.0);
    let a = fix(fx, ee::mul(128.0, ee::mul(f20, f0)), 0x01E0_136C) & 0xFF;
    fx.mem.write_u8(sp + 0x260, a as u8);
    rgba(fx, sp + 0x448, [0xFF, 0x44, 0x44], a as u8);
    copy8(fx, sp + 0x460, sp + 0x448);
    copy8(fx, sp + 0x478, sp + 0x448);
    copy8(fx, sp + 0x548, sp + 0x448);
    for o in [0x4C0u32, 0x4D8, 0x4F0, 0x508] {
        copy8(fx, sp + o, sp + 0x548);
    }
    // The turned copy D+0x2170.
    for k in 0..0x10i32 {
        let rb = D + 0x210 * k as u32;
        let f20 = ee::div(ee::mul(17.5, ee::from_i32(0x10 - k)), 16.0);
        let f21 = ee::div(ee::from_i32(k + 1), 16.0);
        let fp = (k << 17) >> 4;
        let s6 = (k << 16) >> 4;
        for s5 in 0..0x21i32 {
            let (s1, s2) = (s5.wrapping_mul(0x20000), s5 << 16);
            let age = fx.mem.i32(D + 0x64);
            let a = (age.wrapping_mul(0x83F).wrapping_add(fp + (s1 >> 5))) & 0xFFFF;
            let c = fx.tcos(a);
            let f22 = ee::add(1.0, ee::mul(f21, ee::mul(f(0x3E4C_CCCD), c)));
            let age = fx.mem.i32(D + 0x64);
            let b = (s6.wrapping_add(age.wrapping_mul(0x457))) & 0xFFFF;
            let c = fx.tcos(b);
            let f1 = ee::mul(f21, ee::mul(1456.0, c));
            let ang = fixs(fx, ee::add(ee::from_i32(s2 >> 5), f1), 0x01E0_1524) & 0xFFFF;
            let f22 = ee::mul(f20, f22);
            let p = rb + 0x10 * s5 as u32 + 0x2170;
            let sn = fx.tsin(ang);
            fx.mem.setf(p, ee::mul(f22, sn));
            fx.mem.write_u32(p + 4, 0);
            let cs = fx.tcos(ang);
            fx.mem.setf(p + 8, ee::mul(f22, cs));
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    vec4(fx, sp + 0x3C0, 0.0, 0.0, 0.0);
    for k in 0..0x10u32 {
        let s6 = D + 0x210 * k;
        if k == 0xF {
            for s2 in 0..0x20u32 {
                let (s1, s3) = (s6 + 0x10 * s2, s6 + 0x10 * (s2 + 1));
                if fx.project_n(&[sp + 0x3C0, s1 + 0x2170, s3 + 0x2170], sp + 0x280, t, sp + 0x554, 0x01E0_1620) != 1 {
                    continue;
                }
                for (u, x) in [(0x450u32, 0x458u32), (0x468, 0x470), (0x480, 0x488)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&[sp + 0x3C0, s1 + 0x70, s3 + 0x70], sp + 0x280, t, sp + 0x554, 0x01E0_1684) == 1 {
                    fx.submit(t, 0, 0);
                }
            }
        } else {
            let al = if k == 0 { 0 } else { fx.mem.read_u8(sp + 0x260) };
            fx.mem.write_u8(sp + 0x4C3, al);
            fx.mem.write_u8(sp + 0x4DB, al);
            let fp = s6 + 0x210;
            for s5 in 0..0x20u32 {
                let c = |base: u32| [s6 + 0x10 * s5 + base, s6 + 0x10 * (s5 + 1) + base, fp + 0x10 * s5 + base, fp + 0x10 * (s5 + 1) + base];
                if fx.project_n(&c(0x2170), sp + 0x280, q, sp + 0x554, 0x01E0_1740) != 1 {
                    continue;
                }
                for (u, x) in [(0x4C8u32, 0x4D0u32), (0x4E0, 0x4E8), (0x4F8, 0x500), (0x510, 0x518)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                if fx.project_n(&c(0x70), sp + 0x280, q, sp + 0x554, 0x01E0_17BC) == 1 {
                    fx.submit(q, 0, 0);
                }
            }
        }
    }
    let w = fx.mem.f(sp + 0x558);
    let key = fix(fx, w, 0x01E0_1814);
    fx.layer_end(layer, OT, key as i32);
    let age = fx.mem.i32(D + 0x64) + 1;
    fx.mem.set32(D + 0x64, age);
    if age >= 0x105 {
        fx.mem.write_u8(D + 0x60, 0);
    }
}

fn sprite_head(fx: &mut Fx, d: u32, tex: u32, clamp: u64) {
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, 0);
    fx.desc_clamp(d, clamp);
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x280);
    fx.identity(sp + 0x2C0);
    crate::blib::exe::mul_mem(fx, sp + 0x280, sp + 0x2C0);
}

/// Cell `c` of a 4 x 2 sprite atlas into the sprite's UVs (sp + 0x3F8..).
fn atlas(fx: &mut Fx, sp: u32, c: i32) {
    let u = rem2(c, 4);
    let w = c >> 2;
    uv(fx, sp + 0x3F8, ((u << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x3FA, ((w << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x408, ((((u << 5) + 0x20) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x40A, ((((w << 5) + 0x20) << 4) & 0x3FFF) as u32);
}

/// Corners (s, s) / (-s, -s) at sp + 0x340 / 0x350 turned by the camera
/// turn.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    vec4(fx, sp + 0x340, s, s, 0.0);
    vec4(fx, sp + 0x350, ee::neg(s), ee::neg(s), 0.0);
    fx.vu_apply_to(sp + 0x340, sp + 0x300, sp + 0x340);
    fx.vu_apply_to(sp + 0x350, sp + 0x300, sp + 0x350);
}

/// The sprite about sp + 0x3C0 projected and keyed by its w.
fn sprite_at(fx: &mut Fx, sp: u32, d: u32, ra: u32) {
    fx.vu_add4(sp + 0x380, sp + 0x340, sp + 0x3C0);
    fx.vu_add4(sp + 0x390, sp + 0x350, sp + 0x3C0);
    if crate::fxlib2::project(fx, sp + 0x380, PROJ_OUT, sp + 0x280, d, sp + 0x554) {
        let w = fx.mem.f(sp + 0x554);
        let key = fix(fx, w, ra);
        fx.submit(d, OT, key as i32);
    }
}

/// The sparks (D+0x4840, 18 x 0x340): 48 sprites at the target's point
/// along the init's 1.5 vectors plus 0.5 ones, falling [F 0x1E01844..0x1E01C9C].
fn sparks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x81 && n < 0xC8 {
        let (_, k) = fx.div(n, 0x12);
        let r = D + 0x340 * k as u32;
        fx.mem.write_u8(r + 0x4840, 1);
        fx.mem.write_u32(r + 0x4844, 0);
        for s4 in 0..0x30u32 {
            let c = rem2(rnd(fx, 0x01E0_18A4), 8);
            fx.mem.write_u8(r + 0x4848 + s4, c as u8);
            fx.random_table_vec(sp + 0x3C0, 0.5);
            fx.vu_add4(r + 0x4880 + 0x10 * s4, sp + 0x3C0, D + 0x4540 + 0x10 * s4);
        }
    }
    let d = sp + 0x3E0;
    sprite_head(fx, d, 0x10, 5);
    camera(fx, sp);
    for k in 0..0x12u32 {
        let r = D + 0x340 * k;
        if fx.mem.read_u8(r + 0x4840) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 0x4844));
        let f0 = Fx::ramp_above(age, 18.0, 12.0);
        let a = fix(fx, ee::mul(1.0, ee::mul(50.0, f0)), 0x01E0_19F0);
        rgba(fx, sp + 0x3F0, [0xC8, 0, 0], a as u8);
        let age = fx.mem.i32(r + 0x4844);
        let t = fx.tan_falloff_p(age, 0x12, 1);
        let s = ee::mul(5.0, ee::add(f(0x3F4C_CCCD), ee::mul(f(0x3E4C_CCCD), t)));
        corners(fx, sp, s);
        let agef = ee::from_i32(fx.mem.i32(r + 0x4844));
        let f21 = ee::add(f(0x3D4C_CCCD), ee::div(agef, 18.0));
        let p = fx.powf(agef, 2.0);
        let f20 = ee::mul(12.0, ee::mul(0.5, ee::mul(f(0xBC32_864D), p)));
        for s5 in 0..0x30u32 {
            let c = i32::from(fx.mem.read_u8(r + 0x4848 + s5));
            atlas(fx, sp, c);
            fx.vu_scale4(sp + 0x3C0, r + 0x4880 + 0x10 * s5, f21);
            fx.vu_add4(sp + 0x3C0, sp + 0x3C0, D + 0x40);
            let y = ee::add(fx.mem.f(sp + 0x3C4), f20);
            fx.mem.setf(sp + 0x3C4, y);
            sprite_at(fx, sp, d, 0x01E0_1C24);
        }
        let age = fx.mem.i32(r + 0x4844) + 1;
        fx.mem.set32(r + 0x4844, age);
        if age >= 0x13 {
            fx.mem.write_u8(r + 0x4840, 0);
        }
    }
}

/// The puffs (D+0x82C0, 12 x 0x50): four sprites flung 1.3 table vectors
/// from the target's point [F 0x1E01CA0..0x1E020CC].
fn puffs(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n >= 0x81 && n < 0xC8 {
        let (_, k) = fx.div(n, 0xC);
        let r = D + 0x50 * k as u32;
        fx.mem.write_u8(r + 0x82C0, 1);
        fx.mem.write_u32(r + 0x82C4, 0);
        for s4 in 0..4u32 {
            let c = rem2(rnd(fx, 0x01E0_1D04), 8);
            fx.mem.write_u8(r + 0x82C8 + s4, c as u8);
            fx.random_table_vec(r + 0x82D0 + 0x10 * s4, f(0x3FA6_6666));
        }
    }
    let d = sp + 0x3E0;
    sprite_head(fx, d, 0x1C, 5);
    camera(fx, sp);
    for k in 0..0xCu32 {
        let r = D + 0x50 * k;
        if fx.mem.read_u8(r + 0x82C0) == 0 {
            continue;
        }
        let x = fx.mem.i32(r + 0x82C4) << 14;
        fx.mult(0x2AAA_AAAB, x);
        let a = (fx.hilo().1 >> 1).wrapping_add(((x as u32) >> 31) as i32);
        let c = fx.tcos(a);
        let a = fix(fx, ee::mul(1.0, ee::mul(50.0, c)), 0x01E0_1E6C);
        rgba(fx, sp + 0x3F0, [0xC8, 0, 0], a as u8);
        corners(fx, sp, 5.0);
        let agef = ee::from_i32(fx.mem.i32(r + 0x82C4));
        let f21 = ee::add(f(0x3E99_999A), ee::div(ee::mul(f(0x3F33_3333), agef), 12.0));
        for s5 in 0..4u32 {
            let c = i32::from(fx.mem.read_u8(r + 0x82C8 + s5));
            atlas(fx, sp, c);
            fx.vu_scale4(sp + 0x3C0, r + 0x82D0 + 0x10 * s5, f21);
            fx.vu_add4(sp + 0x3C0, sp + 0x3C0, D + 0x40);
            sprite_at(fx, sp, d, 0x01E0_2034);
        }
        let age = fx.mem.i32(r + 0x82C4) + 1;
        fx.mem.set32(r + 0x82C4, age);
        if age >= 0xD {
            fx.mem.write_u8(r + 0x82C0, 0);
        }
    }
}

/// The echoes (D+0x9710, 16 x 0x20): an eight-ring copy of the tube at its
/// spawn point and age, (FF, 40, 40) fading over 16 updates [F
/// 0x1E020D0..0x1E026E8].
fn echoes(fx: &mut Fx, sp: u32) {
    for j in 0..0x10u32 {
        let e = D + 0x20 * j;
        if fx.mem.read_u8(e + 0x9710) == 0 {
            continue;
        }
        camera_at(fx, sp, e + 0x9720);
        breathing_rings(fx, 0x8690, 8, e + 0x9718);
        let q = sp + 0x4B0;
        quad_desc(fx, q);
        alpha(fx, q, 0);
        let rh = fx.mem.read_u32(D + 0x20);
        fx.desc_texture(q, rh, 1);
        fx.desc_clamp(q, 0);
        let age = fx.mem.i32(e + 0x9714);
        let a = fix(fx, ee::mul(100.0, ee::div(ee::from_i32(0x10 - age), 16.0)), 0x01E0_2358) & 0xFF;
        fx.mem.write_u8(sp + 0x250, a as u8);
        rgba(fx, sp + 0x4C0, [0xFF, 0x40, 0x40], a as u8);
        for o in [0x4D8u32, 0x4F0, 0x508] {
            copy8(fx, sp + o, sp + 0x4C0);
        }
        vec4(fx, sp + 0x3C0, 0.0, 0.0, 0.0);
        let age = fx.mem.i32(e + 0x9714);
        ring_quads(fx, sp, q, 0x8690, 8, age, a as u8, [0x01E0_2540, 0x01E0_2554, 0x01E0_2640, 0x01E0_2654]);
        let age = fx.mem.i32(e + 0x9714) + 1;
        fx.mem.set32(e + 0x9714, age);
        if age >= 0x11 {
            fx.mem.write_u8(e + 0x9710, 0);
        }
    }
}

/// The post callback 0x1E027B0(p) [F]: run by 0x1F0FD0 after the
/// attacker's render into its target. FRAME_1 masked (0x2096C0
/// 0xFF000000), a TEXFLUSH, the frame drawn (0x207FC0 -1, filter 1) as the
/// texture of a full-window sprite (FF, 80, 80, 255 ratio(age, 30)
/// ramp(age, 270, 240)) at ALPHA preset 0 and the lens CLAMP (TEX1 merged
/// into 0x209800's saved ra, the slot at sp + 0xA0), buffered; FRAME_1
/// unmasked.
fn callback(fx: &mut Fx, p: u32) -> u32 {
    let sp = fx.enter(0xB0);
    let dd = fx.mem.read_u32(D_STATIC);
    let s0 = crate::b440::frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let d = sp + 0x58;
    head(fx, d, 0x26, 5, 0x4_2421, 0);
    let (a1, a2, a3) = mask_regs(fx);
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [s0, a1, a2, a3]);
    let p1 = fx.texflush_packet(s0);
    // TEX1's slot holds 0x209800's saved ra (0x1F0FD0's call at 0x1F103C).
    fx.mem.write_u64(sp + 0xA0, 0x001F_1044);
    fx.mem.write_u64(sp + 0xA8, 0);
    let p2 = fx.frame_texture(p1, -1, 1, sp + 0x98, sp + 0xA0);
    // 0x207FC0 without a copy leaves a1..a3 = 0, 0x1F, 0 [run].
    fx.boundary(0x0020_EA20, "packet writer 0x20EA20", [p2, 0, 0x1F, 0]);
    let s2 = fx.texflush_packet(p2);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let fl = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, fl | 0x800);
    let age = ee::from_i32(fx.mem.i32(dd + PASS + 4));
    let f20 = ee::mul(255.0, Fx::ratio_below(age, 30.0));
    let age = ee::from_i32(fx.mem.i32(dd + PASS + 4));
    let r = Fx::ramp_above(age, 270.0, 240.0);
    let a = fix(fx, ee::mul(f20, r), 0x01E0_28BC);
    rgba(fx, sp + 0x68, [0xFF, 0x80, 0x80], a as u8);
    uv(fx, sp + 0x70, 0);
    uv(fx, sp + 0x72, 0);
    uv(fx, sp + 0x80, 0x2000);
    uv(fx, sp + 0x82, 0x2000);
    far_xy(fx, sp + 0x78, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x88, 0x8FF8, 0x8FF8);
    let v = fx.submit_at(s2, d, 0, 0);
    let r = crate::b440::frame_mask_packet(fx, v, 0, [0, 0]);
    fx.leave(0xB0);
    r
}

/// a1..a3 as 0x2096C0 leaves them (b768's): FBP | FBW << 16, FBW << 16 and
/// the display record 0x2E93A0.
fn mask_regs(fx: &mut Fx) -> (u32, u32, u32) {
    let a3c = 0x002E_93A0u32;
    let t0 = if fx.mem.read_u32(0x002E_963C) & 1 != 0 { 0x002E_93D0 + 0x150 } else { 0x002E_93D0 + 0x60 };
    let flags = fx.mem.read_u8(a3c + 0x20);
    if flags & 1 != 0 && flags & 2 != 0 {
        let k = fx.mem.read_u32(a3c);
        let fbw = ((fx.mem.read_u32(0x0029_322C + 0x28 * k) as i32 >> 6) << 16) as u32;
        let fbp = fx.mem.read_u32(a3c + 0xC);
        (fbp | fbw, fbw, k.wrapping_mul(40))
    } else {
        let fbw = (u32::from(fx.mem.read_u8(t0 + 2)) & 0x3F) << 16;
        let fbp = u32::from(fx.mem.read_u16(t0)) & 0x1FF;
        (fbp | fbw, fbw, a3c)
    }
}
