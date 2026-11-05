//! BMODULE b612 (card 538), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x6D0 frame, data area a0 =
//! 0x1ED0000, 0x4840 bytes; two unit-target post callbacks 0x1E03650 (0x220
//! frame) / 0x1E03B70 (0x50) with their static D at 0x1E03D00) [F] (helper
//! D26, 3 Nov 2026): b217's program (card 440; its locals 0x10 lower, so the
//! b424-offset pieces run at sp + 0x110) with b424's white fade and frame
//! fan, b424-sized bolts (4 x 0x3A0, 32 strips, life 4) and three puffs.
//! The init lays out the sphere (radius 90, D+0x170) and 17 ramps (D+0x1388:
//! rgb 128 (1 - i / 16) + 255 i / 16, alpha 255 (1 - i / 16), mula / madd).
//! Then: on 0xEA a 33-update tint (FF, 84, C8); on 0x1C2 the white fade
//! (D+0x32D8, 0x73 updates; at age 0x1E it ends the frame fan, the tint
//! and the attacker pass) and the frame fan (D+0x32D0, 0x1F updates); on
//! even updates 0xEB..0x1DF the zoom (D+0x32E0); on 0xEA two node glows
//! (0xF7 updates, (FF, 84, C8), ratio 4, ramp 246 / 242); every update
//! 0xEB..0x1DF one of 16 spiral records (D+0x13D0: two nodes x three
//! ribbons of 16 quads, each turned by b424's spread (0x1554, 0xAAA) about
//! y / x then z, ST halves / thirds, width 5 / speed -300 / amp 15, angle
//! step 0x20000); every update 0xEC..0x1DF one of 4 bolt records
//! (D+0x32F0: strips (FF, 40, 80) alpha 128 cos, sprites (FF, 84, A4)); on
//! 0xA9 / 0xBD / 0xD1 one of 3 puff records (D+0x4180); from 0x50 the
//! attacker pass (D+0x160, ratio 48, ramp 4350 / 4302, ended by the fade);
//! on even updates of the window the target pass; on 0x10 the attacker's
//! (FF, 00, 00) flash (D+0x4170). The stage light 0x80 - 96 clamp(n, 0,
//! 0x40) clamp(0x252 - n, 0, 0x40) / 4096. Returns 1 on 0xE1, 6 on
//! 0xEC..0x1DF, 3 on 0x1E0, 8 once the count reaches 0x253.
use crate::b182::{alpha, clamp_regs, far, head, r256, rgba, uv};
use crate::b424::{camera, fan, fan_desc, screen_point, sphere, veil, window};
use crate::b440::{copy8, fix, fixs, rem2, vec4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const D_STATIC: u32 = 0x01E0_3D00;
const CB_A: u32 = 0x01E0_3650;
const CB_B: u32 = 0x01E0_3B70;
const BOLT_CLAMP: u64 = 0x0000_03FC_811F_C0BA;
/// b424's locals sit 0x110 lower.
const UP: u32 = 0x110;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x6D0);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x6D0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4840);
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
        let f2 = ee::div(ee::from_i32(i), 16.0);
        let f21 = ee::sub(1.0, f2);
        let f20 = ee::add(ee::mul(128.0, f21), ee::mul(255.0, f2));
        let c = D + 0x1388 + 4 * i as u32;
        for (k, ra) in [(0u32, 0x01E0_02E8), (1, 0x01E0_02F4), (2, 0x01E0_0300)] {
            let v = fix(fx, f20, ra);
            fx.mem.write_u8(c + k, v as u8);
        }
        let v = fix(fx, ee::mul(255.0, f21), 0x01E0_0314);
        fx.mem.write_u8(c + 3, v as u8);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x13D0 + 0x1F0 * k, 0);
    }
    for o in [0x32D0u32, 0x32D8, 0x32E0, 0x32E8] {
        fx.mem.write_u8(D + o, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x32F0 + 0x3A0 * k, 0);
    }
    fx.mem.write_u8(D + 0x4170, 0);
    for k in 0..3u32 {
        fx.mem.write_u8(D + 0x4180 + 0x240 * k, 0);
    }
    0
}

fn win(n: i32) -> bool {
    (0xEB..0x1E0).contains(&n)
}

fn bolt_win(n: i32) -> bool {
    (0xEC..0x1E0).contains(&n)
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
    fade(fx, sp);
    frame_fan(fx, sp);
    zoom(fx, sp);
    let f23 = glow(fx, sp);
    spirals(fx, sp, f23);
    let hi = bolts(fx, sp);
    puffs(fx, sp, spr);
    attacker_pass(fx, sp, hi);
    target_pass(fx, sp);
    white_pass(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x252 - n, 0, 0x40);
    let k = Fx::clamp_i(n, 0, 0x40);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 12) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(spr + 0xD0);
    if n == 0xE1 {
        ret = 1;
    } else {
        if bolt_win(n) {
            ret = 6;
        }
        if n == 0x1E0 {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x253 {
        ret = 8;
    }
    fx.mem.set32(spr + 0xD0, ret);
    ret
}

/// The tint (D+0x32E8, 33 updates from 0xEA) [F 0x1E00678..0x1E007D4].
fn tint(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0xEA {
        fx.mem.write_u8(D + 0x32E8, 1);
        fx.mem.write_u32(D + 0x32EC, 0);
    }
    if fx.mem.read_u8(D + 0x32E8) != 1 {
        return;
    }
    window(fx, sp, 0, [0xFF, 0x84, 0xC8], |fx| {
        let age = fx.mem.i32(D + 0x32EC);
        fix(fx, ee::mul(128.0, ee::div(ee::from_i32(0x20 - age), 32.0)), 0x01E0_0710)
    });
    let age = fx.mem.i32(D + 0x32EC) + 1;
    fx.mem.set32(D + 0x32EC, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x32E8, 0);
    }
}

/// b424's white fade (D+0x32D8, 0x73 updates from 0x1C2); at age 0x1E it
/// ends the frame fan, the tint and the attacker pass [F 0x1E007D8..0x1E00968].
fn fade(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x1C2 {
        fx.mem.write_u8(D + 0x32D8, 1);
        fx.mem.write_u32(D + 0x32DC, 0);
    }
    if fx.mem.read_u8(D + 0x32D8) != 1 {
        return;
    }
    if fx.mem.i32(D + 0x32DC) == 0x1E {
        fx.mem.write_u8(D + 0x32D0, 0);
        fx.mem.write_u8(D + 0x32E8, 0);
        fx.mem.write_u8(D + 0x160, 0);
    }
    window(fx, sp, 1, [0xFF; 3], |fx| {
        let age = fx.mem.i32(D + 0x32DC);
        let c = Fx::clamp_i(age - 0x32, 0, 0x40);
        let f20 = ee::sub(1.0, fx.tan_falloff_p(c, 0x40, 1));
        let age = ee::from_i32(fx.mem.i32(D + 0x32DC));
        let q = Fx::ratio_below(age, 30.0);
        fix(fx, ee::mul(128.0, ee::mul(q, f20)), 0x01E0_08AC)
    });
    let age = fx.mem.i32(D + 0x32DC) + 1;
    fx.mem.set32(D + 0x32DC, age);
    if age >= 0x73 {
        fx.mem.write_u8(D + 0x32D8, 0);
    }
}

/// b424's frame-textured fan at the target's screen point (D+0x32D0, 0x1F
/// updates from 0x1C2) [F 0x1E0096C..0x1E00DFC].
fn frame_fan(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x1C2 {
        fx.mem.write_u8(D + 0x32D0, 1);
        fx.mem.write_u32(D + 0x32D4, 0);
    }
    if fx.mem.read_u8(D + 0x32D0) != 1 {
        return;
    }
    let d = sp + 0x4C0;
    fan_desc(fx, sp, 0);
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let age = ee::from_i32(fx.mem.i32(D + 0x32D4));
    let q = Fx::ratio_below(age, 20.0);
    let a = fix(fx, ee::mul(128.0, q), 0x01E0_0A34);
    rgba(fx, sp + 0x4D0, [0x80; 3], a as u8);
    copy8(fx, sp + 0x4E8, sp + 0x4D0);
    copy8(fx, sp + 0x500, sp + 0x4D0);
    let s = ee::add(1.0, ee::mul(f(0x3DCC_CCCD), ee::from_i32(1)));
    const RAS: [u32; 12] = [
        0x01E0_0AE8, 0x01E0_0AF4, 0x01E0_0B38, 0x01E0_0B44, 0x01E0_0B88, 0x01E0_0B94, 0x01E0_0C48, 0x01E0_0C54, 0x01E0_0CC0, 0x01E0_0CCC, 0x01E0_0D38,
        0x01E0_0D44,
    ];
    fan(fx, d, D + 0x70, sp + 0x350, s, OT, &RAS);
    let age = fx.mem.i32(D + 0x32D4) + 1;
    fx.mem.set32(D + 0x32D4, age);
    if age >= 0x1F {
        fx.mem.write_u8(D + 0x32D0, 0);
    }
}

/// The frame zoom (D+0x32E0, two updates, on even updates of the window)
/// [F 0x1E00E00..0x1E010BC].
fn zoom(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if win(n) && rem2(n, 2) == 0 {
        fx.mem.write_u8(D + 0x32E0, 1);
        fx.mem.write_u32(D + 0x32E4, 0);
    }
    if fx.mem.read_u8(D + 0x32E0) != 1 {
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
    let age = fx.mem.i32(D + 0x32E4);
    let al = fix(fx, ee::mul(32.0, ee::from_i32(1 - age)), 0x01E0_0F10);
    rgba(fx, sp + 0x3E8, [0xFF; 3], al as u8);
    uv(fx, sp + 0x3F0, 0);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x2000);
    uv(fx, sp + 0x402, 0x2000);
    for k in 0..4i32 {
        let t = ee::div(ee::mul(f(0x3D4C_CCCD), ee::from_i32(k + 1)), 4.0);
        let f20 = ee::mul(256.0, ee::add(1.0, t));
        let x = fixs(fx, f20, 0x01E0_0FDC);
        let y = fixs(fx, f20, 0x01E0_0FE8);
        far_xy(fx, sp + 0x3F8, ((0x800 - x) << 4) as u16, ((0x800 - y) << 4) as u16);
        far_xy(fx, sp + 0x408, ((x + 0x800) << 4) as u16, ((y + 0x800) << 4) as u16);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x32E4) + 1;
    fx.mem.set32(D + 0x32E4, age);
    if age >= 2 {
        fx.mem.write_u8(D + 0x32E0, 0);
    }
}

/// The two node glows (D+0x1380, 0xF7 updates from 0xEA)
/// [F 0x1E010C0..0x1E013AC].
fn glow(fx: &mut Fx, sp: u32) -> f32 {
    if fx.mem.i32(D) == 0xEA {
        fx.mem.write_u8(D + 0x1380, 1);
        fx.mem.write_u32(D + 0x1384, 0);
    }
    let age = ee::from_i32(fx.mem.i32(D + 0x1384));
    let f20 = Fx::ratio_below(age, 4.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x1384));
    let f0 = Fx::ramp_above(age, 246.0, 242.0);
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
    let a = fix(fx, ee::mul(ee::mul(255.0, s), f23), 0x01E0_11D8);
    rgba(fx, sp + 0x3E8, [0xFF, 0x84, 0xC8], a as u8);
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
    if age >= 0xF7 {
        fx.mem.write_u8(D + 0x1380, 0);
    }
    f23
}

/// The spiral records (D+0x13D0, 16 x 0x1F0): spawned at n % 16 every
/// update 0xEB..0x1DF, drawn as two nodes x three ribbons
/// [F 0x1E013B0..0x1E01BA4].
fn spirals(fx: &mut Fx, sp: u32, f23: f32) {
    let n = fx.mem.i32(D);
    if win(n) {
        let r = D + 0x1F0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x13D0, 1);
        fx.mem.write_u32(r + 0x13D4, 0);
        for j in 0..2u32 {
            for m in 0..3u32 {
                let s1 = r + 0xC * j + 4 * m;
                let s2 = r + 0xC0 * j + 0x40 * m;
                let s3 = (m << 16) as i32;
                let c = r256(fx, 0x01E0_1420);
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                let life = fixs(fx, ee::mul(16.0, ee::add(f(0x3F33_3333), x)), 0x01E0_1484);
                fx.mem.set32(s1 + 0x13D8, life);
                let c = r256(fx, 0x01E0_148C) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x13F0, ee::mul(5.0, ee::add(0.5, x)));
                let c = r256(fx, 0x01E0_14E8) + 1;
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x1408, ee::mul(-300.0, ee::add(f(0x3F33_3333), x)));
                let c = r256(fx, 0x01E0_1554) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x1420, ee::mul(15.0, ee::add(0.5, x)));
                fx.identity(s2 + 0x1440);
                let c = r256(fx, 0x01E0_15BC);
                fx.rotate_axis(s2 + 0x1440, ((c * 0x1554) >> 8) - 0xAAA, 1);
                let c = r256(fx, 0x01E0_15F8);
                fx.rotate_axis(s2 + 0x1440, ((c * 0x1554) >> 8) - 0xAAA, 0);
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
    let f20 = ee::div(ee::mul(32.0, ee::from_i32(rem2(m as i32, 2))), 128.0);
    let f24 = ee::add(0.25, f20);
    let f25 = ee::add(1.0, ee::sub(1.0, f26));
    let mut s3 = 0i32;
    for s1 in 0..0x10i32 {
        let c = D + 0x1388 + 4 * s1 as u32;
        let a0 = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
        let s6 = fix(fx, ee::mul(f23, ee::mul(a0, f26)), 0x01E0_1878) & 0xFF;
        let a1 = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
        let v0 = fix(fx, ee::mul(f23, ee::mul(a1, f26)), 0x01E0_18BC);
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
        let (_, t1) = fx.div(s1, 3);
        let f3 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1)), 3.0), 128.0);
        fx.mem.setf(sp + 0x448, f20);
        fx.mem.setf(sp + 0x44C, f3);
        fx.mem.setf(sp + 0x460, f24);
        fx.mem.setf(sp + 0x464, f3);
        fx.mem.setf(sp + 0x478, f20);
        let f0 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1 + 1)), 3.0), 128.0);
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
        let a = s5.wrapping_add(((s1 + 1) << 17) >> 4) & 0xFFFF;
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
        s3 = s3.wrapping_add(0x20000);
    }
}

/// The bolt records (D+0x32F0, 4 x 0x3A0): spawned at n % 4 every update
/// 0xEC..0x1DF at the target's point, 32 strips each with a sprite,
/// keyed by their mean w [F 0x1E01BA8..0x1E023C8]. Returns the upper half
/// of the record's t0 after the last 0x188420 (0: the glow's identity).
fn bolts(fx: &mut Fx, sp: u32) -> u64 {
    let mut hi = 0;
    let n = fx.mem.i32(D);
    if bolt_win(n) {
        let r = D + 0x3A0 * rem2(n, 4) as u32;
        fx.mem.write_u8(r + 0x32F0, 1);
        fx.mem.write_u32(r + 0x32F4, 0);
        for s4 in 0..0x20u32 {
            let s1 = r + 4 * s4;
            let s2 = r + 0x10 * s4;
            let c = r256(fx, 0x01E0_1C0C);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(4.0, ee::add(f(0x3E99_999A), x)), 0x01E0_1C70);
            fx.mem.set32(s1 + 0x32F8, life);
            let c = r256(fx, 0x01E0_1C78);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x33F8, ee::mul(10.0, ee::add(f(0x3E99_999A), x)));
            let c = r256(fx, 0x01E0_1CE0);
            fx.mem.set32(s1 + 0x3378, (c << 16) >> 8);
            for w in 0..4u32 {
                let v = fx.mem.read_u32(D + 0x60 + 4 * w);
                fx.mem.write_u32(r + 0x3480 + 4 * w, v);
            }
            fx.random_table_vec(s2 + 0x3490, 2.0);
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
    for k in 0..4u32 {
        let s3 = D + 0x3A0 * k;
        if fx.mem.read_u8(s3 + 0x32F0) == 0 {
            continue;
        }
        let s6 = s3 + 0x3480;
        for s5 in 0..0x20i32 {
            let s1 = s3 + 4 * s5 as u32;
            let s2 = s3 + 0x10 * s5 as u32;
            let age = fx.mem.i32(s3 + 0x32F4);
            let life = fx.mem.i32(s1 + 0x32F8);
            if life < age {
                continue;
            }
            let (q, _) = fx.div(age << 14, life);
            let f20 = fx.tcos(q);
            let a = fix(fx, ee::mul(128.0, f20), 0x01E0_1EEC);
            rgba(fx, sp + 0x548, [0xFF, 0x40, 0x80], a as u8);
            let a = fix(fx, ee::mul(255.0, f20), 0x01E0_1F20);
            rgba(fx, sp + 0x3E8, [0xFF, 0x84, 0xA4], a as u8);
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
            let age = fx.mem.i32(s3 + 0x32F4);
            let life = fx.mem.i32(s1 + 0x32F8);
            let (q, _) = fx.div(age << 14, life);
            let sn = fx.tsin(q);
            let f20 = fx.mem.f(s1 + 0x33F8);
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
            fx.vu_add4(sp + 0x390, s2 + 0x3490, s6);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            let ang = fx.mem.i32(s1 + 0x3378);
            fx.rotate_axis(sp + 0x290, ang, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, b, sp + 0x5BC) {
                let w = fx.mem.f(sp + 0x5BC);
                let key = fix(fx, w, 0x01E0_2244);
                fx.submit(b, OT, key as i32);
            }
            let s = ee::mul(f(0x3DCC_CCCD), fx.mem.f(s1 + 0x33F8));
            let ns = ee::neg(s);
            for (o, x) in [(0x310u32, ns), (0x320, s)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x250);
            let age = ee::from_i32(fx.mem.i32(s3 + 0x32F4));
            let life = ee::from_i32(fx.mem.i32(s1 + 0x32F8));
            let t = ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), life));
            fx.vu_scale4(sp + 0x390, s2 + 0x3490, ee::mul(-1.0, t));
            fx.vu_add4(sp + 0x390, sp + 0x390, s6);
            hi = fx.mem.read_u64(sp + 0x2E8);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
                let w = fx.mem.f(sp + 0x5BC);
                let key = fix(fx, w, 0x01E0_2364);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(s3 + 0x32F4) + 1;
        fx.mem.set32(s3 + 0x32F4, age);
        if age >= 5 {
            fx.mem.write_u8(s3 + 0x32F0, 0);
        }
    }
    hi
}

/// The puff records (D+0x4180, 3 x 0x240): one spawned on each of 0xA9 /
/// 0xBD / 0xD1 at node 0x1F / 0x42 (D+0xC0 / 0xD0 by the
/// index's parity): 32 sprite cells (+8 bytes) rising along random
/// vectors (+0x40: y = 0.8 |y|), drawn 32 updates (texture D+0x20, alpha 64
/// cos, 10 (0.5 + age / 64) wide, 3 below the point) [F 0x1E023CC..0x1E028C4].
fn puffs(fx: &mut Fx, sp: u32, spr: u32) {
    let n = fx.mem.i32(D);
    let i = match n {
        0xA9 => 0,
        0xBD => 1,
        0xD1 => 2,
        _ => -1,
    };
    if i >= 0 {
        let r = D + 0x240 * i as u32;
        fx.mem.write_u8(r + 0x4180, 1);
        fx.mem.write_u32(r + 0x4184, 0);
        let src = D + 0x10 * rem2(i, 2) as u32;
        for w in 0..4u32 {
            let v = fx.mem.read_u32(src + 0xC0 + 4 * w);
            fx.mem.write_u32(r + 0x41B0 + 4 * w, v);
        }
        for s2 in 0..0x20u32 {
            let v = fx.rand_from(OT, 0x01E0_24A8) >> 23;
            fx.mem.write_u8(r + s2 + 0x4188, rem2(v, 8) as u8);
            let p = r + 0x10 * s2;
            fx.random_table_vec(p + 0x41C0, 1.0);
            // fabsf's frame: the argument at its sp.
            let y = fx.mem.f(p + 0x41C4);
            fx.mem.setf(spr - 0x10, y);
            fx.mem.write_u32(p + 0x41C4, y.to_bits() & 0x7FFF_FFFF);
            let y = fx.mem.f(p + 0x41C4);
            fx.mem.setf(p + 0x41C4, ee::mul(y, f(0x3F4C_CCCD)));
        }
    }
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for k in 0..3u32 {
        let rec = D + 0x240 * k;
        if fx.mem.read_u8(rec + 0x4180) == 0 {
            continue;
        }
        let age = fx.mem.i32(rec + 0x4184);
        let c = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(64.0, c), 0x01E0_2610);
        rgba(fx, sp + 0x3E8, [0xEE; 3], a as u8);
        let age = ee::from_i32(fx.mem.i32(rec + 0x4184));
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
        let age = fx.mem.i32(rec + 0x4184);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for s5 in 0..0x20u32 {
            fx.vu_scale4(sp + 0x390, rec + 0x10 * s5 + 0x41C0, f20);
            fx.vu_add4(sp + 0x390, sp + 0x390, rec + 0x41B0);
            let y = fx.mem.f(sp + 0x394);
            fx.mem.setf(sp + 0x394, ee::add(y, -3.0));
            fx.vu_add4(sp + 0x350, sp + 0x310, sp + 0x390);
            fx.vu_add4(sp + 0x360, sp + 0x320, sp + 0x390);
            let cell = i32::from(fx.mem.read_u8(rec + s5 + 0x4188));
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
        let age = fx.mem.i32(rec + 0x4184) + 1;
        fx.mem.set32(rec + 0x4184, age);
        if age >= 0x21 {
            fx.mem.write_u8(rec + 0x4180, 0);
        }
    }
}

/// The attacker drawn into its target with callback A and pulled through
/// the fan (D+0x160, from 0x50, ended by the fade) [F 0x1E028C8..0x1E02E64].
fn attacker_pass(fx: &mut Fx, sp: u32, hi: u64) {
    if fx.mem.i32(D) == 0x50 {
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
    let ras = [0x01E0_380C, 0x01E0_3840, 0x01E0_3874, 0x01E0_3A14];
    let (_, regs) = fx.unit_draw_post(1, sp + 0x520, sp + 0x528, CB_A, &mut |fx, p| sphere(fx, p, hi, CB_A, D_STATIC, 0x80, [0x80; 3], &ras));
    fx.texflush(regs);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(D + 0x164));
        let a = Fx::ratio_below(age, 48.0);
        let age = ee::from_i32(fx.mem.i32(D + 0x164));
        ee::mul(a, Fx::ramp_above(age, 4350.0, 4302.0))
    };
    let f21 = ramp(fx);
    let f1 = ramp(fx);
    let f20 = ee::mul(128.0, f21);
    let f21 = ee::mul(128.0, f1);
    let s1 = fix(fx, f20, 0x01E0_2AA8) & 0xFF;
    let v0 = fix(fx, f21, 0x01E0_2AB4);
    rgba(fx, sp + 0x4D0, [0x80; 3], s1 as u8);
    rgba(fx, sp + 0x4E8, [0x80; 3], v0 as u8);
    copy8(fx, sp + 0x500, sp + 0x4E8);
    const RAS: [u32; 12] = [
        0x01E0_2B10, 0x01E0_2B1C, 0x01E0_2B6C, 0x01E0_2B78, 0x01E0_2BC8, 0x01E0_2BD4, 0x01E0_2C94, 0x01E0_2CA0, 0x01E0_2D10, 0x01E0_2D1C, 0x01E0_2D8C,
        0x01E0_2D98,
    ];
    fan(fx, sp + 0x4C0, D + 0x40, sp + 0x350, 1.0, 0, &RAS);
    let layer = fx.mem.read_u32(sp + 0x24C);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x164) + 1;
    fx.mem.set32(D + 0x164, age);
    if age >= 0x10FF {
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

/// The attacker's (FF, 00, 00) flash (D+0x4170, 33 updates from 0x10) as
/// three widening fans [F 0x1E03058..0x1E03524].
fn white_pass(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x10 {
        fx.mem.write_u8(D + 0x4170, 1);
        fx.mem.write_u32(D + 0x4174, 0);
    }
    if fx.mem.read_u8(D + 0x4170) != 1 {
        return;
    }
    let a = fan_desc(fx, sp, 1);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x248, layer);
    fx.texflush(a);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x520, sp + 0x528);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x4174);
    let c = [0xFF, 0, 0];
    rgba(fx, sp + 0x4D0, c, 0);
    rgba(fx, sp + 0x4E8, c, (((0x20 - age) << 7) >> 5) as u8);
    copy8(fx, sp + 0x500, sp + 0x4E8);
    let age = fx.mem.i32(D + 0x4174);
    let f20 = ee::mul(f(0x3E99_999A), fx.tan_falloff_p(age, 0x20, 1));
    const RAS: [u32; 12] = [
        0x01E0_31F8, 0x01E0_3204, 0x01E0_3248, 0x01E0_3254, 0x01E0_3298, 0x01E0_32A4, 0x01E0_3358, 0x01E0_3364, 0x01E0_33D0, 0x01E0_33DC, 0x01E0_3448,
        0x01E0_3454,
    ];
    for j in 0..3i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f20, ee::from_i32(j + 1)), 3.0));
        fan(fx, sp + 0x4C0, D + 0x40, sp + 0x350, s, 0, &RAS);
    }
    let layer = fx.mem.read_u32(sp + 0x248);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x4174) + 1;
    fx.mem.set32(D + 0x4174, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x4170, 0);
    }
}
