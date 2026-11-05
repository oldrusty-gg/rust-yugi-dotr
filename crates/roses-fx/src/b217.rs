//! BMODULE b217 (card 440), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x6C0 frame, data area a0 =
//! 0x1ED0000, 0x4D30 bytes; two unit-target post callbacks 0x1E02FB0 (0x220
//! frame) / 0x1E034D0 (0x50) with their static D at 0x1E03600) [F] (helper
//! D25, 3 Nov 2026): b424's passes (its locals 0x100 higher: the shared
//! pieces run at sp + 0x100) with b182's two-node spirals and bolts and a
//! new puff part. The init lays out the sphere (radius 90, D+0x170) and 17
//! grey ramps (D+0x1388, alpha 128 (16 - i) / 16); nodes 0xE / 0x10 (D+0x80
//! / 0x84 -> D+0x90 / 0xA0, matrices pointing from the target's point
//! D+0xE0 / 0x120) and 0x1F / 0x42 (D+0xB0 / 0xB4 -> D+0xC0 / 0xD0). Then:
//! on 0xFD a 17-update tint (FF, FF, C8); on even updates 0xFE..0x12B a
//! two-update zoom of the frame (0.05 steps); on 0xDB two node glows (0x3E
//! updates, (96, 78, 00)); every update 0xDC..0x117 one of 16 spiral
//! records (D+0x13D0, 0x1F0: two nodes x three ribbons of 16 quads, each
//! ribbon's turn pointing along a random vector); every update 0xFE..0x12B
//! one of 8 bolt records (D+0x32E0, 0x1E0: 16 strips and sprites at the
//! target's point, keyed by their mean w); on 0xA9 / 0xC1 / 0xDB / 0xF3 /
//! 0xF9 one of 5 puff records (D+0x41F0, 0x240: 32 sprite cells rising
//! from node 0x1F / 0x42, 32 updates); from 0x55 for 0xC4 updates the
//! attacker pass (callback A: the sphere) through the fan; on even updates
//! of the window the target pass (callback B: the veil); on 0x34 the
//! attacker's (FF, 9B, 00) flash. The stage light 0x80 - 96 clamp(n, 0,
//! 0x40) clamp(0x16C - n, 0, 0x40) / 4096. Returns 1 on 0xF3, 6 on
//! 0xFE..0x12B, 3 on 0x12C, 8 once the count reaches 0x16D.
use crate::b182::{alpha, clamp_regs, far, head, r256, rgba, uv};
use crate::b424::{camera, fan, fan_desc, screen_point, sphere, veil, window};
use crate::b440::{copy8, fix, fixs, rem2, vec4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const D_STATIC: u32 = 0x01E0_3600;
const CB_A: u32 = 0x01E0_2FB0;
const CB_B: u32 = 0x01E0_34D0;
const BOLT_CLAMP: u64 = 0x0000_03FC_811F_C0BA;
/// b424's locals sit 0x100 lower.
const UP: u32 = 0x100;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x6C0);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x6C0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4D30);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for (k, id) in [(0x10u32, 0), (0x14, 1), (0x18, 2), (0x1C, 3), (0x20, 4)] {
        let r = fx.texture(5, id);
        fx.mem.write_u32(D + k, r);
    }
    let r = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x24, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    for (o, node, kind) in [(0x80u32, 0xE, 1), (0x84, 0x10, 0), (0xB0, 0x1F, 0), (0xB4, 0x42, 0)] {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + o, s);
    }
    fx.mem.write_u8(D + 0x160, 0);
    for i in 0..0x11i32 {
        let c = fx.tcos((i * 0x8000) >> 4);
        let f21 = ee::mul(90.0, c);
        let a = Fx::powf2(90.0);
        let b = Fx::powf2(f21);
        let f20 = fx.sqrtf(ee::sub(a, b));
        let row = D + 0x110 * i as u32;
        for j in 0..0x11i32 {
            let a = (j << 16) >> 4;
            let p = row + 0x10 * j as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(p + 0x170, ee::mul(f20, sn));
            fx.mem.setf(p + 0x174, f21);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 0x178, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0x17C, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0x1380, 0);
    fx.mem.write_u32(D + 0x1384, 0);
    for i in 0..0x11i32 {
        let f3 = ee::div(ee::from_i32(i), 16.0);
        let f0 = ee::mul(128.0, f3);
        let f21 = ee::mul(128.0, ee::sub(1.0, f3));
        let f20 = ee::add(f21, f0);
        let c = D + 0x1388 + 4 * i as u32;
        for (k, ra) in [(0u32, 0x01E0_02DC), (1, 0x01E0_02E8), (2, 0x01E0_02F4)] {
            let v = fix(fx, f20, ra);
            fx.mem.write_u8(c + k, v as u8);
        }
        let v = fix(fx, f21, 0x01E0_0300);
        fx.mem.write_u8(c + 3, v as u8);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x13D0 + 0x1F0 * k, 0);
    }
    fx.mem.write_u8(D + 0x32D0, 0);
    fx.mem.write_u8(D + 0x32D8, 0);
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x32E0 + 0x1E0 * k, 0);
    }
    fx.mem.write_u8(D + 0x41E0, 0);
    for k in 0..5u32 {
        fx.mem.write_u8(D + 0x41F0 + 0x240 * k, 0);
    }
    0
}

fn win(n: i32) -> bool {
    (0xFE..0x12C).contains(&n)
}

/// `spr` the entry's stack pointer; `sp` = `spr` + 0x100 addresses the
/// locals at b424's offsets.
fn step(fx: &mut Fx, spr: u32) -> i32 {
    let sp = spr + UP;
    fx.camera_turn_roll(sp + 0x2D0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x60);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x80 + 4 * k);
        fx.node_position(h, slot, D + 0x90 + 0x10 * k);
        fx.vu_sub4(sp + 0x390, D + 0x90 + 0x10 * k, D + 0x60);
        fx.point_along(D + 0xE0 + 0x40 * k, sp + 0x390);
    }
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0xB0 + 4 * k);
        fx.node_position(h, slot, D + 0xC0 + 0x10 * k);
    }
    camera(fx, sp);
    screen_point(fx, sp, D + 0x30, D + 0x40);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x50, D + 0x70);

    tint(fx, sp);
    zoom(fx, sp);
    let f23 = glow(fx, sp);
    spirals(fx, sp, f23);
    let hi = bolts(fx, sp);
    puffs(fx, sp, spr);
    attacker_pass(fx, sp, hi);
    target_pass(fx, sp);
    white_pass(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x16C - n, 0, 0x40);
    let k = Fx::clamp_i(n, 0, 0x40);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 12) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(spr + 0xD0);
    if n == 0xF3 {
        ret = 1;
    } else {
        if win(n) {
            ret = 6;
        }
        if n == 0x12C {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x16D {
        ret = 8;
    }
    fx.mem.set32(spr + 0xD0, ret);
    ret
}

/// The tint (D+0x32D8, 17 updates from 0xFD) [F 0x1E0065C..0x1E007B4].
fn tint(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0xFD {
        fx.mem.write_u8(D + 0x32D8, 1);
        fx.mem.write_u32(D + 0x32DC, 0);
    }
    if fx.mem.read_u8(D + 0x32D8) != 1 {
        return;
    }
    window(fx, sp, 0, [0xFF, 0xFF, 0xC8], |fx| {
        let age = fx.mem.i32(D + 0x32DC);
        fix(fx, ee::mul(128.0, ee::div(ee::from_i32(0x10 - age), 16.0)), 0x01E0_06F4)
    });
    let age = fx.mem.i32(D + 0x32DC) + 1;
    fx.mem.set32(D + 0x32DC, age);
    if age >= 0x11 {
        fx.mem.write_u8(D + 0x32D8, 0);
    }
}

/// The frame zoom (D+0x32D0, two updates, on even updates of the window)
/// [F 0x1E007B8..0x1E00A74].
fn zoom(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if win(n) && rem2(n, 2) == 0 {
        fx.mem.write_u8(D + 0x32D0, 1);
        fx.mem.write_u32(D + 0x32D4, 0);
    }
    if fx.mem.read_u8(D + 0x32D0) != 1 {
        return;
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x418, sp + 0x420);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x32D4);
    let al = fix(fx, ee::mul(32.0, ee::from_i32(1 - age)), 0x01E0_08C8);
    rgba(fx, sp + 0x3E8, [0xFF; 3], al as u8);
    uv(fx, sp + 0x3F0, 0);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x2000);
    uv(fx, sp + 0x402, 0x2000);
    for k in 0..4i32 {
        let t = ee::div(ee::mul(f(0x3D4C_CCCD), ee::from_i32(k + 1)), 4.0);
        let f20 = ee::mul(256.0, ee::add(1.0, t));
        let x = fixs(fx, f20, 0x01E0_0994);
        let y = fixs(fx, f20, 0x01E0_09A0);
        far_xy(fx, sp + 0x3F8, ((0x800 - x) << 4) as u16, ((0x800 - y) << 4) as u16);
        far_xy(fx, sp + 0x408, ((x + 0x800) << 4) as u16, ((y + 0x800) << 4) as u16);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x32D4) + 1;
    fx.mem.set32(D + 0x32D4, age);
    if age >= 2 {
        fx.mem.write_u8(D + 0x32D0, 0);
    }
}

/// The two node glows (D+0x1380, 0x3E updates from 0xDB)
/// [F 0x1E00A78..0x1E00D64].
fn glow(fx: &mut Fx, sp: u32) -> f32 {
    if fx.mem.i32(D) == 0xDB {
        fx.mem.write_u8(D + 0x1380, 1);
        fx.mem.write_u32(D + 0x1384, 0);
    }
    let age = ee::from_i32(fx.mem.i32(D + 0x1384));
    let f20 = Fx::ratio_below(age, 16.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x1384));
    let f0 = Fx::ramp_above(age, 61.0, 45.0);
    let f23 = ee::mul(f20, f0);
    if fx.mem.read_u8(D + 0x1380) != 1 {
        return f23;
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
    let m = rem2(fx.mem.i32(D + 0x1384), 2);
    let s = ee::add(f(0x3F33_3333), ee::mul(f(0x3E99_999A), ee::from_i32(m)));
    let a = fix(fx, ee::mul(ee::mul(255.0, s), f23), 0x01E0_0B90);
    rgba(fx, sp + 0x3E8, [0x96, 0x78, 0], a as u8);
    fx.mem.setf(sp + 0x310, -35.0);
    fx.mem.setf(sp + 0x314, -35.0);
    fx.mem.write_u32(sp + 0x318, 0);
    fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
    vec4(fx, sp + 0x320, [0x420C_0000, 0x420C_0000, 0, 0x3F80_0000]);
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(sp + 0x320, sp + 0x2D0, sp + 0x320);
    uv(fx, sp + 0x3F0, 0x600);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x800);
    uv(fx, sp + 0x402, 0x200);
    for k in 0..2u32 {
        fx.camera_vp_to(sp + 0x250);
        fx.identity(sp + 0x290);
        fx.translate_rows(sp + 0x290, sp + 0x290, D + 0x90 + 0x10 * k);
        crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
        if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
            far(fx, sp + 0x3F8, &[sp + 0x408]);
            fx.submit(d, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x1384) + 1;
    fx.mem.set32(D + 0x1384, age);
    if age >= 0x3E {
        fx.mem.write_u8(D + 0x1380, 0);
    }
    f23
}

/// The spiral records (D+0x13D0, 16 x 0x1F0): spawned at n % 16 every
/// update 0xDC..0x117, drawn as two nodes x three ribbons
/// [F 0x1E00D68..0x1E014F4].
fn spirals(fx: &mut Fx, sp: u32, f23: f32) {
    let n = fx.mem.i32(D);
    if (0xDC..0x118).contains(&n) {
        let r = D + 0x1F0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x13D0, 1);
        fx.mem.write_u32(r + 0x13D4, 0);
        for j in 0..2u32 {
            for m in 0..3u32 {
                let s1 = r + 0xC * j + 4 * m;
                let s2 = r + 0xC0 * j + 0x40 * m;
                let s3 = (m << 16) as i32;
                let c = r256(fx, 0x01E0_0DD8);
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                let life = fixs(fx, ee::mul(16.0, ee::add(f(0x3F33_3333), x)), 0x01E0_0E3C);
                fx.mem.set32(s1 + 0x13D8, life);
                let c = r256(fx, 0x01E0_0E44) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x13F0, ee::mul(10.0, ee::add(0.5, x)));
                let c = r256(fx, 0x01E0_0EA0) + 1;
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x1408, ee::mul(-150.0, ee::add(f(0x3F33_3333), x)));
                let c = r256(fx, 0x01E0_0F0C) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x1420, ee::mul(10.0, ee::add(0.5, x)));
                fx.random_table_vec(sp + 0x390, 1.0);
                fx.point_along(s2 + 0x1440, sp + 0x390);
                fx.mult(0x5555_5556, s3);
                let hi = fx.hilo().1;
                fx.rotate_axis(s2 + 0x1440, hi.wrapping_add(((s3 as u32) >> 31) as i32), 2);
            }
        }
    }
    let q = sp + 0x430;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    for k in 0..0x10u32 {
        let rec = D + 0x1F0 * k;
        if fx.mem.read_u8(rec + 0x13D0) == 0 {
            continue;
        }
        for j in 0..2u32 {
            let mut s5 = (0xD05 * k + 0x115C * j) as i32;
            for m in 0..3u32 {
                let s4 = rec + 0xC * j + 4 * m;
                let age = fx.mem.i32(rec + 0x13D4);
                let life = fx.mem.i32(s4 + 0x13D8);
                if life >= age {
                    ribbon(fx, sp, rec, j, m, s4, s5, f23);
                }
                s5 = s5.wrapping_add(0x15B3);
            }
        }
        let age = fx.mem.i32(rec + 0x13D4) + 1;
        fx.mem.set32(rec + 0x13D4, age);
        if age >= 0x11 {
            fx.mem.write_u8(rec + 0x13D0, 0);
        }
    }
}

/// One ribbon of 16 quads (node `j`, sub-record `s4`, phase `s5`).
#[allow(clippy::too_many_arguments)]
fn ribbon(fx: &mut Fx, sp: u32, rec: u32, j: u32, m: u32, s4: u32, s5: i32, f23: f32) {
    let q = sp + 0x430;
    fx.camera_vp_to(sp + 0x250);
    let t = std::array::from_fn(|c| fx.mem.f(D + 0x90 + 0x10 * j + 4 * c as u32));
    let mm = fx.trs(None, None, Some(t));
    fx.write_m4(sp + 0x290, &mm);
    crate::blib::exe::mul_mem(fx, sp + 0x290, D + 0xE0 + 0x40 * j);
    crate::blib::exe::mul_mem(fx, sp + 0x290, rec + 0xC0 * j + 0x40 * m + 0x1440);
    crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
    let age = fx.mem.i32(rec + 0x13D4);
    let life = fx.mem.i32(s4 + 0x13D8);
    let f0 = fx.tan_falloff_p(age, life, 1);
    let f26 = ee::sub(1.0, f0);
    let f20 = ee::div(ee::mul(32.0, ee::from_i32(rem2(m as i32, 4))), 128.0);
    let f24 = ee::add(0.25, f20);
    let f25 = ee::add(1.0, ee::sub(1.0, f26));
    let mut s3 = 0i32;
    for s1 in 0..0x10i32 {
        let c = D + 0x1388 + 4 * s1 as u32;
        let a0 = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
        let s6 = fix(fx, ee::mul(f23, ee::mul(a0, f26)), 0x01E0_11C8) & 0xFF;
        let a1 = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
        let v0 = fix(fx, ee::mul(f23, ee::mul(a1, f26)), 0x01E0_120C);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + k);
            fx.mem.write_u8(sp + 0x440 + k, b);
        }
        fx.mem.write_u8(sp + 0x443, s6 as u8);
        fx.mem.write_u32(sp + 0x444, 0x3F80_0000);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + 4 + k);
            fx.mem.write_u8(sp + 0x470 + k, b);
        }
        fx.mem.write_u8(sp + 0x473, v0 as u8);
        fx.mem.write_u32(sp + 0x474, 0x3F80_0000);
        copy8(fx, sp + 0x458, sp + 0x440);
        copy8(fx, sp + 0x488, sp + 0x470);
        let t1 = rem2(s1, 8);
        let f3 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1)), 8.0), 128.0);
        fx.mem.setf(sp + 0x448, f20);
        fx.mem.setf(sp + 0x44C, f3);
        fx.mem.setf(sp + 0x460, f24);
        fx.mem.setf(sp + 0x464, f3);
        fx.mem.setf(sp + 0x478, f20);
        let f0 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1 + 1)), 8.0), 128.0);
        fx.mem.setf(sp + 0x47C, f0);
        fx.mem.setf(sp + 0x490, f24);
        fx.mem.setf(sp + 0x494, f0);
        let a = s5.wrapping_add(s3 >> 4) & 0xFFFF;
        let f21 = ee::div(ee::from_i32(s1), 16.0);
        let f22 = ee::div(ee::from_i32(s1 + 1), 16.0);
        let f4 = fx.mem.f(s4 + 0x1408);
        let f29 = ee::mul(fx.mem.f(s4 + 0x13F0), f26);
        let f27 = ee::mul(f4, f21);
        let f28 = ee::mul(f4, f22);
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x1420);
        let f21 = ee::mul(f21, ee::mul(f25, ee::mul(amp, sn)));
        let a = s5.wrapping_add(((s1 + 1) << 16) >> 4) & 0xFFFF;
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x1420);
        let f3 = ee::neg(f29);
        let f1 = ee::mul(f22, ee::mul(f25, ee::mul(amp, sn)));
        fx.mem.setf(sp + 0x310, f21);
        fx.mem.setf(sp + 0x314, ee::add(f3, f21));
        fx.mem.setf(sp + 0x318, f27);
        fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
        fx.mem.setf(sp + 0x320, f21);
        fx.mem.setf(sp + 0x324, ee::add(f29, f21));
        fx.mem.setf(sp + 0x328, f27);
        fx.mem.write_u32(sp + 0x32C, 0x3F80_0000);
        fx.mem.setf(sp + 0x330, f1);
        fx.mem.setf(sp + 0x334, ee::add(f3, f1));
        fx.mem.setf(sp + 0x338, f28);
        fx.mem.write_u32(sp + 0x33C, 0x3F80_0000);
        fx.mem.setf(sp + 0x340, f1);
        fx.mem.setf(sp + 0x344, ee::add(f29, f1));
        fx.mem.setf(sp + 0x348, f28);
        fx.mem.write_u32(sp + 0x34C, 0x3F80_0000);
        let mm = fx.read_m4(sp + 0x250);
        if fx.project_desc(sp + 0x310, PROJ_OUT, Some(&mm), q, sp + 0x5BC) {
            fx.submit(q, OT, 0);
        }
        s3 = s3.wrapping_add(0x10000);
    }
}

/// The bolt records (D+0x32E0, 8 x 0x1E0): spawned at n % 8 every update
/// of the window at the target's point, 16 strips each with a sprite,
/// keyed by their mean w [F 0x1E014F8..0x1E01D10]. Returns the upper half
/// of the record's t0 after the last 0x188420 (0: the glow's identity).
fn bolts(fx: &mut Fx, sp: u32) -> u64 {
    let mut hi = 0;
    let n = fx.mem.i32(D);
    if win(n) {
        let r = D + 0x1E0 * rem2(n, 8) as u32;
        fx.mem.write_u8(r + 0x32E0, 1);
        fx.mem.write_u32(r + 0x32E4, 0);
        for s4 in 0..0x10u32 {
            let s1 = r + 4 * s4;
            let s2 = r + 0x10 * s4;
            let c = r256(fx, 0x01E0_155C);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(8.0, ee::add(f(0x3E99_999A), x)), 0x01E0_15C0);
            fx.mem.set32(s1 + 0x32E8, life);
            let c = r256(fx, 0x01E0_15C8);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x3368, ee::mul(10.0, ee::add(f(0x3E99_999A), x)));
            let c = r256(fx, 0x01E0_1630);
            fx.mem.set32(s1 + 0x3328, (c << 16) >> 8);
            for w in 0..4u32 {
                let v = fx.mem.read_u32(D + 0x60 + 4 * w);
                fx.mem.write_u32(r + 0x33B0 + 4 * w, v);
            }
            fx.random_table_vec(s2 + 0x33C0, 2.0);
        }
    }
    let b = sp + 0x538;
    head(fx, b, 0x24, 9, 0x4_2424_2421, 0x800);
    alpha(fx, b, 1);
    let rh = fx.mem.read_u32(D + 0x24);
    fx.desc_texture(b, rh, 1);
    fx.desc_clamp(b, BOLT_CLAMP);
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    for k in 0..8u32 {
        let s3 = D + 0x1E0 * k;
        if fx.mem.read_u8(s3 + 0x32E0) == 0 {
            continue;
        }
        let s6 = s3 + 0x33B0;
        for s5 in 0..0x10i32 {
            let s1 = s3 + 4 * s5 as u32;
            let s2 = s3 + 0x10 * s5 as u32;
            let age = fx.mem.i32(s3 + 0x32E4);
            let life = fx.mem.i32(s1 + 0x32E8);
            if life < age {
                continue;
            }
            let (q, _) = fx.div(age << 14, life);
            let f20 = fx.tcos(q);
            let c = [0x96, 0x78, 0];
            let a = fix(fx, ee::mul(164.0, f20), 0x01E0_183C);
            rgba(fx, sp + 0x548, c, a as u8);
            let a = fix(fx, ee::mul(255.0, f20), 0x01E0_186C);
            rgba(fx, sp + 0x3E8, c, a as u8);
            let c4 = rem2(s5, 4);
            let a1 = rem2(c4, 2);
            let row = ((c4 >> 1) << 6) + 0x80;
            let u0 = 59 * a1 + 0xA;
            let (t2, a3) = (((u0 << 4) & 0x3FFF) as u32, (((u0 + 0x3B) << 4) & 0x3FFF) as u32);
            let (t0, a2) = (((row << 4) & 0x3FFF) as u32, (((row + 0x40) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x550, t2);
            uv(fx, sp + 0x552, t0);
            uv(fx, sp + 0x560, a3);
            uv(fx, sp + 0x562, t0);
            uv(fx, sp + 0x570, t2);
            uv(fx, sp + 0x572, a2);
            uv(fx, sp + 0x580, a3);
            uv(fx, sp + 0x582, a2);
            let c8 = rem2(s5, 8);
            let cu = rem2(c8, 4);
            let cv = c8 >> 2;
            uv(fx, sp + 0x3F0, ((cu << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x3F2, ((cv << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x400, ((((cu << 5) + 0x20) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x402, ((((cv << 5) + 0x20) << 4) & 0x3FFF) as u32);
            let age = fx.mem.i32(s3 + 0x32E4);
            let life = fx.mem.i32(s1 + 0x32E8);
            let (q, _) = fx.div(age << 14, life);
            let sn = fx.tsin(q);
            let f20 = fx.mem.f(s1 + 0x3368);
            let f3 = ee::mul(ee::mul(0.5, f20), sn);
            let f2 = ee::neg(f20);
            let (a, b1, c2, d0) = (ee::add(f2, f3), ee::sub(f2, f3), ee::add(f20, f3), ee::sub(f20, f3));
            for (o, x, y) in [(0x310u32, a, b1), (0x320, c2, b1), (0x330, b1, c2), (0x340, d0, c2)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, y);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x250);
            fx.vu_add4(sp + 0x390, s2 + 0x33C0, s6);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            let ang = fx.mem.i32(s1 + 0x3328);
            fx.rotate_axis(sp + 0x290, ang, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, b, sp + 0x5BC) {
                let w = fx.mem.f(sp + 0x5BC);
                let key = fix(fx, w, 0x01E0_1B8C);
                fx.submit(b, OT, key as i32);
            }
            let s = ee::mul(f(0x3DCC_CCCD), fx.mem.f(s1 + 0x3368));
            let ns = ee::neg(s);
            for (o, x) in [(0x310u32, ns), (0x320, s)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x250);
            let age = ee::from_i32(fx.mem.i32(s3 + 0x32E4));
            let life = ee::from_i32(fx.mem.i32(s1 + 0x32E8));
            let t = ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), life));
            fx.vu_scale4(sp + 0x390, s2 + 0x33C0, ee::mul(-1.0, t));
            fx.vu_add4(sp + 0x390, sp + 0x390, s6);
            hi = fx.mem.read_u64(sp + 0x2E8);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
                let w = fx.mem.f(sp + 0x5BC);
                let key = fix(fx, w, 0x01E0_1CAC);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(s3 + 0x32E4) + 1;
        fx.mem.set32(s3 + 0x32E4, age);
        if age >= 9 {
            fx.mem.write_u8(s3 + 0x32E0, 0);
        }
    }
    hi
}

/// The puff records (D+0x41F0, 5 x 0x240): one spawned on each of 0xA9 /
/// 0xC1 / 0xDB / 0xF3 / 0xF9 at node 0x1F / 0x42 (D+0xC0 / 0xD0 by the
/// index's parity): 32 sprite cells (+8 bytes) rising along random
/// vectors (+0x40: y = 0.8 |y|), drawn 32 updates (texture D+0x20, alpha 64
/// cos, 10 (0.5 + age / 64) wide, 3 below the point) [F 0x1E01D14..0x1E0223C].
fn puffs(fx: &mut Fx, sp: u32, spr: u32) {
    let n = fx.mem.i32(D);
    let i = match n {
        0xA9 => 0,
        0xC1 => 1,
        0xDB => 2,
        0xF3 => 3,
        0xF9 => 4,
        _ => -1,
    };
    if i >= 0 {
        let r = D + 0x240 * i as u32;
        fx.mem.write_u8(r + 0x41F0, 1);
        fx.mem.write_u32(r + 0x41F4, 0);
        let src = D + 0x10 * rem2(i, 2) as u32;
        for w in 0..4u32 {
            let v = fx.mem.read_u32(src + 0xC0 + 4 * w);
            fx.mem.write_u32(r + 0x4220 + 4 * w, v);
        }
        for s2 in 0..0x20u32 {
            let v = fx.rand_from(OT, 0x01E0_1E20) >> 23;
            fx.mem.write_u8(r + s2 + 0x41F8, rem2(v, 8) as u8);
            let p = r + 0x10 * s2;
            fx.random_table_vec(p + 0x4230, 1.0);
            // fabsf's frame: the argument at its sp.
            let y = fx.mem.f(p + 0x4234);
            fx.mem.setf(spr - 0x10, y);
            fx.mem.write_u32(p + 0x4234, y.to_bits() & 0x7FFF_FFFF);
            let y = fx.mem.f(p + 0x4234);
            fx.mem.setf(p + 0x4234, ee::mul(y, f(0x3F4C_CCCD)));
        }
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..5u32 {
        let rec = D + 0x240 * k;
        if fx.mem.read_u8(rec + 0x41F0) == 0 {
            continue;
        }
        let age = fx.mem.i32(rec + 0x41F4);
        let c = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(64.0, c), 0x01E0_1F88);
        rgba(fx, sp + 0x3E8, [0xEE; 3], a as u8);
        let age = ee::from_i32(fx.mem.i32(rec + 0x41F4));
        let s = ee::mul(10.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
        let ns = ee::neg(s);
        for (o, x) in [(0x310u32, ns), (0x320, s)] {
            fx.mem.setf(sp + o, x);
            fx.mem.setf(sp + o + 4, x);
            fx.mem.write_u32(sp + o + 8, 0);
            fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
        }
        fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
        fx.vu_apply_to(sp + 0x320, sp + 0x2D0, sp + 0x320);
        let age = fx.mem.i32(rec + 0x41F4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for s5 in 0..0x20u32 {
            fx.vu_scale4(sp + 0x390, rec + 0x10 * s5 + 0x4230, f20);
            fx.vu_add4(sp + 0x390, sp + 0x390, rec + 0x4220);
            let y = fx.mem.f(sp + 0x394);
            fx.mem.setf(sp + 0x394, ee::add(y, -3.0));
            fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x390);
            fx.vu_add4(sp + 0x360, sp + 0x320, sp + 0x390);
            let cell = i32::from(fx.mem.read_u8(rec + s5 + 0x41F8));
            let cu = rem2(cell, 4);
            let cv = cell >> 2;
            uv(fx, sp + 0x3F0, ((cu << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x3F2, ((cv << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x400, ((((cu << 5) + 0x20) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x402, ((((cv << 5) + 0x20) << 4) & 0x3FFF) as u32);
            if crate::fxlib2::project(fx, sp + 0x350, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(rec + 0x41F4) + 1;
        fx.mem.set32(rec + 0x41F4, age);
        if age >= 0x21 {
            fx.mem.write_u8(rec + 0x41F0, 0);
        }
    }
}

/// The attacker drawn into its target with callback A and pulled through
/// the fan (D+0x160, 0xC4 updates from 0x55) [F 0x1E02240..0x1E027CC].
fn attacker_pass(fx: &mut Fx, sp: u32, hi: u64) {
    if fx.mem.i32(D) == 0x55 {
        fx.mem.write_u8(D + 0x160, 1);
        fx.mem.write_u32(D + 0x164, 0);
    }
    if fx.mem.read_u8(D + 0x160) != 1 {
        return;
    }
    camera(fx, sp);
    vec4(fx, sp + 0x390, [0, 0, 0, 0x3F80_0000]);
    let y = fx.mem.read_u32(D + 0x34);
    fx.mem.write_u32(sp + 0x394, y);
    let m = fx.read_m4(sp + 0x250);
    fx.project_at(sp + 0x390, &m, sp + 0x5B0, sp + 0x5BC);
    let a = fan_desc(fx, sp, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x24C, layer);
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let ras = [0x01E0_316C, 0x01E0_31A0, 0x01E0_31D4, 0x01E0_3374];
    let (_, regs) = fx.unit_draw_post(1, sp + 0x520, sp + 0x528, CB_A, &mut |fx, p| sphere(fx, p, hi, CB_A, D_STATIC, 0x80, [0x80; 3], &ras));
    fx.texflush(regs);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(D + 0x164));
        let a = Fx::ratio_below(age, 16.0);
        let age = ee::from_i32(fx.mem.i32(D + 0x164));
        ee::mul(a, Fx::ramp_above(age, 195.0, 179.0))
    };
    let f21 = ramp(fx);
    let f1 = ramp(fx);
    let f20 = ee::mul(128.0, f21);
    let f21 = ee::mul(128.0, f1);
    let s1 = fix(fx, f20, 0x01E0_2410) & 0xFF;
    let v0 = fix(fx, f21, 0x01E0_241C);
    rgba(fx, sp + 0x4D0, [0x80; 3], s1 as u8);
    rgba(fx, sp + 0x4E8, [0x80; 3], v0 as u8);
    copy8(fx, sp + 0x500, sp + 0x4E8);
    const RAS: [u32; 12] = [
        0x01E0_2478, 0x01E0_2484, 0x01E0_24D4, 0x01E0_24E0, 0x01E0_2530, 0x01E0_253C, 0x01E0_25FC, 0x01E0_2608, 0x01E0_2678, 0x01E0_2684, 0x01E0_26F4,
        0x01E0_2700,
    ];
    fan(fx, sp + 0x4C0, D + 0x40, sp + 0x350, 1.0, 0, &RAS);
    let layer = fx.mem.read_u32(sp + 0x24C);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x164) + 1;
    fx.mem.set32(D + 0x164, age);
    if age >= 0xC4 {
        fx.mem.write_u8(D + 0x160, 0);
    }
}

/// On even updates of the window the target pass (callback B)
/// [F 0x1E027D0..0x1E029BC].
fn target_pass(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if !(win(n) && rem2(n, 2) == 0) {
        return;
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let t = fx.mem.i32(D + 8);
    let (_, regs) = fx.unit_target_post(t, 1, sp + 0x418, sp + 0x420, CB_B, &mut |fx, p| veil(fx, p));
    fx.texflush(regs);
    rgba(fx, sp + 0x3E8, [0x80; 3], 0x80);
    uv(fx, sp + 0x3F0, 0);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x2000);
    uv(fx, sp + 0x402, 0x2000);
    far_xy(fx, sp + 0x3F8, 0x7000, 0x7000);
    far_xy(fx, sp + 0x408, 0x9000, 0x9000);
    fx.submit(d, 0, 0);
    fx.layer_end(layer, OT, 0);
}

/// The attacker's (FF, 9B, 00) flash (D+0x41E0, 33 updates from 0x34) as
/// three widening fans [F 0x1E029C0..0x1E02E88].
fn white_pass(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x34 {
        fx.mem.write_u8(D + 0x41E0, 1);
        fx.mem.write_u32(D + 0x41E4, 0);
    }
    if fx.mem.read_u8(D + 0x41E0) != 1 {
        return;
    }
    let a = fan_desc(fx, sp, 1);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x248, layer);
    fx.texflush(a);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x520, sp + 0x528);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x41E4);
    let c = [0xFF, 0x9B, 0];
    rgba(fx, sp + 0x4D0, c, 0);
    rgba(fx, sp + 0x4E8, c, (((0x20 - age) << 7) >> 5) as u8);
    copy8(fx, sp + 0x500, sp + 0x4E8);
    let age = fx.mem.i32(D + 0x41E4);
    let f20 = ee::mul(f(0x3E99_999A), fx.tan_falloff_p(age, 0x20, 1));
    const RAS: [u32; 12] = [
        0x01E0_2B60, 0x01E0_2B6C, 0x01E0_2BB0, 0x01E0_2BBC, 0x01E0_2C00, 0x01E0_2C0C, 0x01E0_2CC0, 0x01E0_2CCC, 0x01E0_2D38, 0x01E0_2D44, 0x01E0_2DB0,
        0x01E0_2DBC,
    ];
    for j in 0..3i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f20, ee::from_i32(j + 1)), 3.0));
        fan(fx, sp + 0x4C0, D + 0x40, sp + 0x350, s, 0, &RAS);
    }
    let layer = fx.mem.read_u32(sp + 0x248);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x41E4) + 1;
    fx.mem.set32(D + 0x41E4, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x41E0, 0);
    }
}
