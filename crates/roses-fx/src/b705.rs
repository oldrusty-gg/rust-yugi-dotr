//! BMODULE b705 (card 11), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x640 frame, data area a0 =
//! 0x1ED0000, 0x4320 bytes; one unit-target post callback 0x1E03370 (b424's
//! veil) with its static D at 0x1E03500) [F]:
//! b424's program (card 535; its locals 0x80 lower, so the shared pieces
//! run at sp + 0x80) without the glow, the sphere and the attacker pass,
//! with eight nodes, spirals that pick a node each, a sprite at each
//! ribbon's node and a new ring part. The init requests nodes 0x24 (kind
//! 1) / 0x2C / 9 / 0x31 / 0x11 / 0x13 / 0x15 / 0x17 (D+0x80..0x9C ->
//! D+0xA0 + 0x10 k, matrices pointing from the target's point D+0x120 +
//! 0x40 k) and lays out 17 ramps (D+0x320: rgb 128 (1 - i / 16) + 255 i /
//! 16, alpha 128 (1 - i / 16)). Then: on 0x59 a 33-update tint (E8, E0,
//! FF) (D+0x2288); on 0x12C the white fade (D+0x2278, 0x73 updates; at
//! age 0x1E it ends the frame fan and the tint) and the frame fan
//! (D+0x2270, 0x1F updates); on even updates 0x5A..0x149 the zoom
//! (D+0x2280, 0.2 steps); every update 0x59..0x149 one of 16 spiral
//! records (D+0x370, 0x1F0: six ribbons of 16 quads, each at a random node
//! of the eight (+8 bytes), turned by b704's spread (3640, 0x71C), width 10
//! / speed -500 / amp 20, angle step 0x20000, alpha unscaled, plus a (1C,
//! 62, FF) sprite at the node); every update 0x5A..0x149 one of 4 bolt
//! records (D+0x2290: b424's, strips (40, 80, FF), sprites (84, A4, FF));
//! on even updates 0x5A..0x149 the target pass (callback: the veil); on
//! 0x20 the attacker's (FF, 64, E6) flash (D+0x3110, 0x10D updates, alpha
//! 48 ratio(age, 48)) as eight widening fans; every update 0x21..0x149 one
//! of 64 ring records (D+0x3118, 0x48: eight upright quads around the
//! target's position rising with age, life 64 (0.5 + c / 512), colour (208,
//! 128, 128) (0.6 + 0.4 c / 256), drawn at OT key 0xFFFFFF, faded in by
//! clamp(n - 0x20, 0, 0x20) / 32, ended at 0x14A). The stage light 0x80 -
//! 128 clamp(n, 0, 0x20) clamp(0x19E - n, 0, 0x20) / 1024. Returns 1 on
//! 0x4F, 6 on 0x5A..0x149, 3 on 0x14A, 8 once the count reaches 0x19F.
use crate::b182::{alpha, clamp_regs, far, head, r256, rgba, uv};
use crate::b424::{camera, fan, fan_desc, screen_point, veil, window};
use crate::b440::{copy8, fix, fixs, rem2};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const D_STATIC: u32 = 0x01E0_3500;
const CB: u32 = 0x01E0_3370;
const BOLT_CLAMP: u64 = 0x0000_03FC_811F_C0BA;
/// b424's locals sit 0x80 lower.
const UP: u32 = 0x80;
const NODES: [(i32, i32); 8] = [(0x24, 1), (0x2C, 0), (9, 0), (0x31, 0), (0x11, 0), (0x13, 0), (0x15, 0), (0x17, 0)];

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x640);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x640);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x4320);
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
    for (k, &(node, kind)) in NODES.iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + 0x80 + 4 * k as u32, s);
    }
    for i in 0..0x11i32 {
        let f3 = ee::div(ee::from_i32(i), 16.0);
        let f21 = ee::mul(128.0, ee::sub(1.0, f3));
        let f20 = ee::add(f21, ee::mul(255.0, f3));
        let c = D + 0x320 + 4 * i as u32;
        for (k, ra) in [(0u32, 0x01E0_0274), (1, 0x01E0_0280), (2, 0x01E0_028C)] {
            let v = fix(fx, f20, ra);
            fx.mem.write_u8(c + k, v as u8);
        }
        let v = fix(fx, f21, 0x01E0_0298);
        fx.mem.write_u8(c + 3, v as u8);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x370 + 0x1F0 * k, 0);
    }
    for o in [0x2270u32, 0x2278, 0x2280, 0x2288] {
        fx.mem.write_u8(D + o, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x2290 + 0x3A0 * k, 0);
    }
    fx.mem.write_u8(D + 0x3110, 0);
    for k in 0..0x40u32 {
        fx.mem.write_u8(D + 0x3118 + 0x48 * k, 0);
    }
    0
}

fn win(n: i32) -> bool {
    (0x5A..0x14A).contains(&n)
}

/// `spr` the entry's stack pointer; `sp` = `spr` + 0x80 addresses the
/// locals at b424's offsets.
fn step(fx: &mut Fx, spr: u32) -> i32 {
    let sp = spr + UP;
    fx.camera_turn_roll(sp + 0x2D0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x60);
    for k in 0..8u32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x80 + 4 * k);
        fx.node_position(h, slot, D + 0xA0 + 0x10 * k);
        fx.vu_sub4(sp + 0x390, D + 0xA0 + 0x10 * k, D + 0x60);
        fx.point_along(D + 0x120 + 0x40 * k, sp + 0x390);
    }
    camera(fx, sp);
    screen_point(fx, sp, D + 0x30, D + 0x40);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x50, D + 0x70);

    tint(fx, sp);
    fade(fx, sp);
    frame_fan(fx, sp);
    zoom(fx, sp);
    spirals(fx, sp);
    bolts(fx, sp);
    target_pass(fx, sp);
    white_pass(fx, sp);
    rings(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x19E - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_neg() << 7).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(spr + 0xD0);
    if n == 0x4F {
        ret = 1;
    } else {
        if win(n) {
            ret = 6;
        }
        if n == 0x14A {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x19F {
        ret = 8;
    }
    fx.mem.set32(spr + 0xD0, ret);
    ret
}

/// The tint (D+0x2288, 33 updates from 0x59) [F 0x1E005CC..0x1E00728].
fn tint(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x59 {
        fx.mem.write_u8(D + 0x2288, 1);
        fx.mem.write_u32(D + 0x228C, 0);
    }
    if fx.mem.read_u8(D + 0x2288) != 1 {
        return;
    }
    window(fx, sp, 0, [0xE8, 0xE0, 0xFF], |fx| {
        let age = fx.mem.i32(D + 0x228C);
        fix(fx, ee::mul(128.0, ee::div(ee::from_i32(0x20 - age), 32.0)), 0x01E0_0664)
    });
    let age = fx.mem.i32(D + 0x228C) + 1;
    fx.mem.set32(D + 0x228C, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x2288, 0);
    }
}

/// b424's white fade (D+0x2278, 0x73 updates from 0x12C); at age 0x1E it
/// ends the frame fan and the tint [F 0x1E0072C..0x1E008B8].
fn fade(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x12C {
        fx.mem.write_u8(D + 0x2278, 1);
        fx.mem.write_u32(D + 0x227C, 0);
    }
    if fx.mem.read_u8(D + 0x2278) != 1 {
        return;
    }
    if fx.mem.i32(D + 0x227C) == 0x1E {
        fx.mem.write_u8(D + 0x2270, 0);
        fx.mem.write_u8(D + 0x2288, 0);
    }
    window(fx, sp, 1, [0xFF; 3], |fx| {
        let age = fx.mem.i32(D + 0x227C);
        let c = Fx::clamp_i(age - 0x32, 0, 0x40);
        let f20 = ee::sub(1.0, fx.tan_falloff_p(c, 0x40, 1));
        let age = ee::from_i32(fx.mem.i32(D + 0x227C));
        let q = Fx::ratio_below(age, 30.0);
        fix(fx, ee::mul(128.0, ee::mul(q, f20)), 0x01E0_07FC)
    });
    let age = fx.mem.i32(D + 0x227C) + 1;
    fx.mem.set32(D + 0x227C, age);
    if age >= 0x73 {
        fx.mem.write_u8(D + 0x2278, 0);
    }
}

/// b424's frame-textured fan at the target's screen point (D+0x2270, 0x1F
/// updates from 0x12C) [F 0x1E008BC..0x1E00D4C].
fn frame_fan(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x12C {
        fx.mem.write_u8(D + 0x2270, 1);
        fx.mem.write_u32(D + 0x2274, 0);
    }
    if fx.mem.read_u8(D + 0x2270) != 1 {
        return;
    }
    let d = sp + 0x4C0;
    fan_desc(fx, sp, 0);
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let age = ee::from_i32(fx.mem.i32(D + 0x2274));
    let q = Fx::ratio_below(age, 20.0);
    let a = fix(fx, ee::mul(128.0, q), 0x01E0_0984);
    rgba(fx, sp + 0x4D0, [0x80; 3], a as u8);
    copy8(fx, sp + 0x4E8, sp + 0x4D0);
    copy8(fx, sp + 0x500, sp + 0x4D0);
    let s = ee::add(1.0, ee::mul(f(0x3DCC_CCCD), ee::from_i32(1)));
    const RAS: [u32; 12] = [
        0x01E0_0A38, 0x01E0_0A44, 0x01E0_0A88, 0x01E0_0A94, 0x01E0_0AD8, 0x01E0_0AE4, 0x01E0_0B98, 0x01E0_0BA4, 0x01E0_0C10, 0x01E0_0C1C, 0x01E0_0C88,
        0x01E0_0C94,
    ];
    fan(fx, d, D + 0x70, sp + 0x350, s, OT, &RAS);
    let age = fx.mem.i32(D + 0x2274) + 1;
    fx.mem.set32(D + 0x2274, age);
    if age >= 0x1F {
        fx.mem.write_u8(D + 0x2270, 0);
    }
}

/// The frame zoom (D+0x2280, two updates, on even updates of the window,
/// 0.2 steps) [F 0x1E00D50..0x1E0100C].
fn zoom(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if win(n) && rem2(n, 2) == 0 {
        fx.mem.write_u8(D + 0x2280, 1);
        fx.mem.write_u32(D + 0x2284, 0);
    }
    if fx.mem.read_u8(D + 0x2280) != 1 {
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
    let age = fx.mem.i32(D + 0x2284);
    let al = fix(fx, ee::mul(32.0, ee::from_i32(1 - age)), 0x01E0_0E60);
    rgba(fx, sp + 0x3E8, [0xFF; 3], al as u8);
    uv(fx, sp + 0x3F0, 0);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x2000);
    uv(fx, sp + 0x402, 0x2000);
    for k in 0..4i32 {
        let t = ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(k + 1)), 4.0);
        let f20 = ee::mul(256.0, ee::add(1.0, t));
        let x = fixs(fx, f20, 0x01E0_0F2C);
        let y = fixs(fx, f20, 0x01E0_0F38);
        far_xy(fx, sp + 0x3F8, ((0x800 - x) << 4) as u16, ((0x800 - y) << 4) as u16);
        far_xy(fx, sp + 0x408, ((x + 0x800) << 4) as u16, ((y + 0x800) << 4) as u16);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x2284) + 1;
    fx.mem.set32(D + 0x2284, age);
    if age >= 2 {
        fx.mem.write_u8(D + 0x2280, 0);
    }
}

/// HI of 0x2AAAAAAB x `x` plus the sign bit of `x` (x / 6).
fn div6(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    hi.wrapping_add(((x as u32) >> 31) as i32)
}

/// The spiral records (D+0x370, 16 x 0x1F0): spawned at n % 16 every
/// update 0x59..0x149, six ribbons each at a random node (+8 bytes),
/// drawn with a sprite at the node [F 0x1E01010..0x1E01B78].
fn spirals(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x59..0x14A).contains(&n) {
        let r = D + 0x1F0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x370, 1);
        fx.mem.write_u32(r + 0x374, 0);
        for m in 0..6u32 {
            let s2 = r + 4 * m;
            let s3 = r + 0x40 * m;
            let s4 = (m << 16) as i32;
            let v = fx.rand_from(OT, 0x01E0_1088) >> 23;
            fx.mem.write_u8(r + m + 0x378, rem2(v, 8) as u8);
            let c = r256(fx, 0x01E0_10A8);
            let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(16.0, ee::add(f(0x3F33_3333), x)), 0x01E0_110C);
            fx.mem.set32(s2 + 0x380, life);
            let c = r256(fx, 0x01E0_1114) + 1;
            let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
            fx.mem.setf(s2 + 0x398, ee::mul(10.0, ee::add(0.5, x)));
            let c = r256(fx, 0x01E0_1170) + 1;
            let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
            fx.mem.setf(s2 + 0x3B0, ee::mul(-500.0, ee::add(f(0x3F33_3333), x)));
            let c = r256(fx, 0x01E0_11DC) + 1;
            let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
            fx.mem.setf(s2 + 0x3C8, ee::mul(20.0, ee::add(0.5, x)));
            fx.identity(s3 + 0x3E0);
            let c = r256(fx, 0x01E0_1244);
            fx.rotate_axis(s3 + 0x3E0, ((c * 3640) >> 8) - 0x71C, 1);
            let c = r256(fx, 0x01E0_128C);
            fx.rotate_axis(s3 + 0x3E0, ((c * 3640) >> 8) - 0x71C, 0);
            let a = div6(fx, s4);
            fx.rotate_axis(s3 + 0x3E0, a, 2);
        }
    }
    let q = sp + 0x430;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    let d = sp + 0x3D8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
    for k in 0..0x10u32 {
        let rec = D + 0x1F0 * k;
        if fx.mem.read_u8(rec + 0x370) == 0 {
            continue;
        }
        let mut s5 = (0xD05 * k) as i32;
        for m in 0..6u32 {
            let s4 = rec + 4 * m;
            let age = fx.mem.i32(rec + 0x374);
            let life = fx.mem.i32(s4 + 0x380);
            if life >= age {
                ribbon(fx, sp, rec, m, s4, s5);
            }
            s5 = s5.wrapping_add(0x15B3);
        }
        let age = fx.mem.i32(rec + 0x374) + 1;
        fx.mem.set32(rec + 0x374, age);
        if age >= 0x11 {
            fx.mem.write_u8(rec + 0x370, 0);
        }
    }
}

/// One ribbon of 16 quads at its node (sub-record `s4`, phase `s5`), then
/// the (1C, 62, FF) sprite at the node.
fn ribbon(fx: &mut Fx, sp: u32, rec: u32, m: u32, s4: u32, s5: i32) {
    let q = sp + 0x430;
    let cell = u32::from(fx.mem.read_u8(rec + m + 0x378));
    fx.camera_vp_to(sp + 0x250);
    let node = D + 0xA0 + 0x10 * cell;
    let t = std::array::from_fn(|c| fx.mem.f(node + 4 * c as u32));
    let mm = fx.trs(None, None, Some(t));
    fx.write_m4(sp + 0x290, &mm);
    crate::blib::exe::mul_mem(fx, sp + 0x290, D + 0x120 + 0x40 * cell);
    crate::blib::exe::mul_mem(fx, sp + 0x290, rec + 0x40 * m + 0x3E0);
    crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
    let age = fx.mem.i32(rec + 0x374);
    let life = fx.mem.i32(s4 + 0x380);
    let f0 = fx.tan_falloff_p(age, life, 1);
    let f26 = ee::sub(1.0, f0);
    let f20 = ee::div(ee::mul(32.0, ee::from_i32(rem2(m as i32, 4))), 128.0);
    let f24 = ee::add(0.25, f20);
    let f25 = ee::add(1.0, ee::sub(1.0, f26));
    let mut s3 = 0i32;
    for s1 in 0..0x10i32 {
        let c = D + 0x320 + 4 * s1 as u32;
        let a0 = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
        let s6 = fix(fx, ee::mul(a0, f26), 0x01E0_15EC) & 0xFF;
        let a1 = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
        let v0 = fix(fx, ee::mul(a1, f26), 0x01E0_162C);
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
        let t1 = rem2(s1, 4);
        let f3 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1)), 4.0), 128.0);
        fx.mem.setf(sp + 0x448, f20);
        fx.mem.setf(sp + 0x44C, f3);
        fx.mem.setf(sp + 0x460, f24);
        fx.mem.setf(sp + 0x464, f3);
        fx.mem.setf(sp + 0x478, f20);
        let f0 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1 + 1)), 4.0), 128.0);
        fx.mem.setf(sp + 0x47C, f0);
        fx.mem.setf(sp + 0x490, f24);
        fx.mem.setf(sp + 0x494, f0);
        let a = s5.wrapping_add(s3 >> 4) & 0xFFFF;
        let f21 = ee::div(ee::from_i32(s1), 16.0);
        let f22 = ee::div(ee::from_i32(s1 + 1), 16.0);
        let f4 = fx.mem.f(s4 + 0x3B0);
        let f29 = ee::mul(fx.mem.f(s4 + 0x398), f26);
        let f28 = ee::mul(f4, f21);
        let f27 = ee::mul(f4, f22);
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x3C8);
        let f21 = ee::mul(f21, ee::mul(f25, ee::mul(amp, sn)));
        let a = s5.wrapping_add(((s1 + 1) << 17) >> 4) & 0xFFFF;
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x3C8);
        let f3 = ee::neg(f29);
        let f1 = ee::mul(f22, ee::mul(f25, ee::mul(amp, sn)));
        fx.mem.setf(sp + 0x310, f21);
        fx.mem.setf(sp + 0x314, ee::add(f3, f21));
        fx.mem.setf(sp + 0x318, f28);
        fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
        fx.mem.setf(sp + 0x320, f21);
        fx.mem.setf(sp + 0x324, ee::add(f29, f21));
        fx.mem.setf(sp + 0x328, f28);
        fx.mem.write_u32(sp + 0x32C, 0x3F80_0000);
        fx.mem.setf(sp + 0x330, f1);
        fx.mem.setf(sp + 0x334, ee::add(f3, f1));
        fx.mem.setf(sp + 0x338, f27);
        fx.mem.write_u32(sp + 0x33C, 0x3F80_0000);
        fx.mem.setf(sp + 0x340, f1);
        fx.mem.setf(sp + 0x344, ee::add(f29, f1));
        fx.mem.setf(sp + 0x348, f27);
        fx.mem.write_u32(sp + 0x34C, 0x3F80_0000);
        let mm = fx.read_m4(sp + 0x250);
        if fx.project_desc(sp + 0x310, PROJ_OUT, Some(&mm), q, sp + 0x5BC) {
            fx.submit(q, OT, 0);
        }
        s3 = s3.wrapping_add(0x20000);
    }
    let d = sp + 0x3D8;
    let age = fx.mem.i32(rec + 0x374);
    let s = ee::add(f(0x3F33_3333), ee::mul(f(0x3E99_999A), ee::from_i32(rem2(age, 2))));
    let a = fix(fx, ee::mul(ee::mul(255.0, s), f26), 0x01E0_18BC);
    rgba(fx, sp + 0x3E8, [0x1C, 0x62, 0xFF], a as u8);
    for (o, x) in [(0x310u32, -8.0f32), (0x320, 8.0)] {
        fx.mem.setf(sp + o, x);
        fx.mem.setf(sp + o + 4, x);
        fx.mem.write_u32(sp + o + 8, 0);
        fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
    }
    fx.vu_apply_to(sp + 0x310, sp + 0x2D0, sp + 0x310);
    fx.vu_apply_to(sp + 0x320, sp + 0x2D0, sp + 0x320);
    uv(fx, sp + 0x3F0, 0x600);
    uv(fx, sp + 0x3F2, 0);
    uv(fx, sp + 0x400, 0x800);
    uv(fx, sp + 0x402, 0x200);
    fx.camera_vp_to(sp + 0x250);
    fx.identity(sp + 0x290);
    fx.translate_rows(sp + 0x290, sp + 0x290, node);
    crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
    if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
        far(fx, sp + 0x3F8, &[sp + 0x408]);
        fx.submit(d, OT, 0);
    }
}

/// b424's bolt records (D+0x2290, 4 x 0x3A0): spawned at n % 4 every
/// update of the window at the target's point, 32 strips (40, 80, FF) each
/// with a (84, A4, FF) sprite [F 0x1E01B7C..0x1E024D0].
fn bolts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if win(n) {
        let r = D + 0x3A0 * rem2(n, 4) as u32;
        fx.mem.write_u8(r + 0x2290, 1);
        fx.mem.write_u32(r + 0x2294, 0);
        for s4 in 0..0x20u32 {
            let s1 = r + 4 * s4;
            let s2 = r + 0x10 * s4;
            let c = r256(fx, 0x01E0_1BE4);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(4.0, ee::add(f(0x3E99_999A), x)), 0x01E0_1C48);
            fx.mem.set32(s1 + 0x2298, life);
            let c = r256(fx, 0x01E0_1C50);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x2398, ee::mul(10.0, ee::add(f(0x3E99_999A), x)));
            let c = r256(fx, 0x01E0_1CB8);
            fx.mem.set32(s1 + 0x2318, (c << 16) >> 8);
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x60 + 4 * w);
                fx.mem.write_u32(r + 0x2420 + 4 * w, x);
            }
            fx.random_table_vec(s2 + 0x2430, 1.5);
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
        if fx.mem.read_u8(s3 + 0x2290) == 0 {
            continue;
        }
        let s6 = s3 + 0x2420;
        for s5 in 0..0x20i32 {
            let s1 = s3 + 4 * s5 as u32;
            let s2 = s3 + 0x10 * s5 as u32;
            let age = fx.mem.i32(s3 + 0x2294);
            let life = fx.mem.i32(s1 + 0x2298);
            if life < age {
                continue;
            }
            let (q, _) = fx.div(age << 14, life);
            let f20 = fx.tcos(q);
            let a = fix(fx, ee::mul(64.0, f20), 0x01E0_1EB4);
            rgba(fx, sp + 0x548, [0x40, 0x80, 0xFF], a as u8);
            let a = fix(fx, ee::mul(255.0, f20), 0x01E0_1EE8);
            rgba(fx, sp + 0x3E8, [0x84, 0xA4, 0xFF], a as u8);
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
            let age = fx.mem.i32(s3 + 0x2294);
            let life = fx.mem.i32(s1 + 0x2298);
            let (q, _) = fx.div(age << 14, life);
            let sn = fx.tsin(q);
            let f20 = fx.mem.f(s1 + 0x2398);
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
            fx.vu_add4(sp + 0x390, s2 + 0x2430, s6);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            let ang = fx.mem.i32(s1 + 0x2318);
            fx.rotate_axis(sp + 0x290, ang, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, b, sp + 0x5BC) {
                far(fx, sp + 0x558, &[sp + 0x568, sp + 0x578, sp + 0x588]);
                fx.submit(b, OT, 0);
            }
            let s = ee::mul(f(0x3DCC_CCCD), fx.mem.f(s1 + 0x2398));
            let ns = ee::neg(s);
            for (o, x) in [(0x310u32, ns), (0x320, s)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x250);
            let age = ee::from_i32(fx.mem.i32(s3 + 0x2294));
            let life = ee::from_i32(fx.mem.i32(s1 + 0x2298));
            let t = ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), life));
            fx.vu_scale4(sp + 0x390, s2 + 0x2430, ee::mul(-1.0, t));
            fx.vu_add4(sp + 0x390, sp + 0x390, s6);
            fx.translate_rows(sp + 0x290, sp + 0x2D0, sp + 0x390);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            if crate::fxlib2::project(fx, sp + 0x310, PROJ_OUT, sp + 0x250, d, sp + 0x5BC) {
                far(fx, sp + 0x3F8, &[sp + 0x408]);
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(s3 + 0x2294) + 1;
        fx.mem.set32(s3 + 0x2294, age);
        if age >= 5 {
            fx.mem.write_u8(s3 + 0x2290, 0);
        }
    }
}

/// On even updates of the window the target drawn into its target with
/// the veil callback, composited by a full-window sprite
/// [F 0x1E024D4..0x1E026E4].
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
    let (_, regs) = fx.unit_target_post(t, 1, sp + 0x418, sp + 0x420, CB, &mut |fx, p| veil(fx, p));
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

/// The attacker's (FF, 64, E6) flash (D+0x3110, 0x10D updates from 0x20,
/// alpha 48 ratio(age, 48)) as eight widening fans (1 + 0.2 k / 7 - 0.1)
/// [F 0x1E026E8..0x1E02BE0].
fn white_pass(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x20 {
        fx.mem.write_u8(D + 0x3110, 1);
        fx.mem.write_u32(D + 0x3114, 0);
    }
    if fx.mem.read_u8(D + 0x3110) != 1 {
        return;
    }
    let a = fan_desc(fx, sp, 1);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x24C, layer);
    fx.texflush(a);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x520, sp + 0x528);
    fx.texflush(regs);
    let age = ee::from_i32(fx.mem.i32(D + 0x3114));
    let q = Fx::ratio_below(age, 48.0);
    let a = fix(fx, ee::mul(48.0, q), 0x01E0_27CC);
    rgba(fx, sp + 0x4D0, [0xFF, 0x64, 0xE6], a as u8);
    copy8(fx, sp + 0x4E8, sp + 0x4D0);
    copy8(fx, sp + 0x500, sp + 0x4D0);
    const RAS: [u32; 12] = [
        0x01E0_28A0, 0x01E0_28AC, 0x01E0_28F4, 0x01E0_2900, 0x01E0_2948, 0x01E0_2954, 0x01E0_2A0C, 0x01E0_2A18, 0x01E0_2A88, 0x01E0_2A94, 0x01E0_2B04,
        0x01E0_2B10,
    ];
    for k in 0..8i32 {
        let t = ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(k)), 7.0);
        let s = ee::sub(ee::add(1.0, t), f(0x3DCC_CCCD));
        fan(fx, sp + 0x4C0, D + 0x40, sp + 0x350, s, 0, &RAS);
    }
    let layer = fx.mem.read_u32(sp + 0x24C);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x3114) + 1;
    fx.mem.set32(D + 0x3114, age);
    if age >= 0x10D {
        fx.mem.write_u8(D + 0x3110, 0);
    }
}

/// The ring records (D+0x3118, 64 x 0x48): spawned at n % 64 every update
/// 0x21..0x149, eight upright quads (+0x08 life bytes, +0x10 angles, +0x30
/// colours) at radius 300 (0.5 + k / 16) about the target's position,
/// rising 1000 age / life - 450, drawn at OT key 0xFFFFFF
/// [F 0x1E02BE4..0x1E03250].
fn rings(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x21..0x14A).contains(&n) {
        let r = D + 0x48 * rem2(n, 0x40) as u32;
        fx.mem.write_u8(r + 0x3118, 1);
        fx.mem.write_u32(r + 0x311C, 0);
        for s4 in 0..8u32 {
            let c = r256(fx, 0x01E0_2C44);
            let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
            let life = fix(fx, ee::mul(64.0, ee::add(0.5, x)), 0x01E0_2C94);
            fx.mem.write_u8(r + s4 + 0x3120, life as u8);
            let c = r256(fx, 0x01E0_2C9C);
            fx.mem.set32(r + 4 * s4 + 0x3128, (c << 16) >> 8);
            for (k, (ra, ra2, sc)) in [(0x01E0_2CC4, 0x01E0_2D28, 208.0f32), (0x01E0_2D30, 0x01E0_2D94, 128.0), (0x01E0_2D9C, 0x01E0_2E00, 128.0)].into_iter().enumerate() {
                let c = r256(fx, ra);
                let x = ee::div(ee::mul(f(0x3ECC_CCCD), ee::from_i32(c)), 256.0);
                let v = fix(fx, ee::mul(sc, ee::add(f(0x3F19_999A), x)), ra2);
                fx.mem.write_u8(r + 3 * s4 + k as u32 + 0x3148, v as u8);
            }
        }
    }
    let q = sp + 0x430;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 0);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    let n = fx.mem.i32(D);
    let f21 = ee::div(ee::from_i32(Fx::clamp_i(n - 0x20, 0, 0x20)), 32.0);
    for k in 0..0x40u32 {
        let rec = D + 0x48 * k;
        if fx.mem.read_u8(rec + 0x3118) == 0 {
            continue;
        }
        for s4 in 0..8u32 {
            let age = fx.mem.i32(rec + 0x311C);
            let life = i32::from(fx.mem.read_u8(rec + s4 + 0x3120));
            if life < age {
                continue;
            }
            fx.camera_vp_to(sp + 0x250);
            fx.identity(sp + 0x290);
            let a = ee::from_i32(fx.mem.i32(rec + 0x311C));
            let life = i32::from(fx.mem.read_u8(rec + s4 + 0x3120));
            let f22 = ee::sub(ee::div(ee::mul(1000.0, a), ee::from_i32(life)), 500.0);
            let s5 = fx.mem.i32(rec + 4 * s4 + 0x3128);
            let f20 = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(s4 as i32)), 8.0));
            let f23 = ee::mul(300.0, f20);
            let cs = fx.tcos(s5);
            fx.mem.setf(sp + 0x390, ee::mul(f23, cs));
            fx.mem.setf(sp + 0x394, ee::add(50.0, f22));
            let sn = fx.tsin(s5);
            fx.mem.setf(sp + 0x398, ee::mul(f23, sn));
            fx.mem.write_u32(sp + 0x39C, 0x3F80_0000);
            fx.translate_rows(sp + 0x290, sp + 0x290, sp + 0x390);
            fx.rotate_axis(sp + 0x290, s5.wrapping_neg(), 1);
            crate::blib::exe::mul_mem(fx, sp + 0x250, sp + 0x290);
            let a = ee::from_i32(fx.mem.i32(rec + 0x311C));
            let life = i32::from(fx.mem.read_u8(rec + s4 + 0x3120));
            let f22 = Fx::ramp_above(a, ee::from_i32(life), ee::from_i32(life - 4));
            let a = ee::from_i32(fx.mem.i32(rec + 0x311C));
            let f0 = Fx::ratio_below(a, 4.0);
            let al = fix(fx, ee::mul(f21, ee::mul(ee::mul(32.0, f0), f22)), 0x01E0_3080);
            let cb = rec + 3 * s4 + 0x3148;
            let c = [fx.mem.read_u8(cb), fx.mem.read_u8(cb + 1), fx.mem.read_u8(cb + 2)];
            rgba(fx, sp + 0x440, c, al as u8);
            for o in [0x458u32, 0x470, 0x488] {
                copy8(fx, sp + o, sp + 0x440);
            }
            let f5 = ee::mul(30.0, f20);
            let f2 = ee::neg(f5);
            let f3 = ee::mul(6.0, f2);
            let f4 = ee::mul(6.0, f5);
            for (o, y, z) in [(0x310u32, f3, f2), (0x320, f3, f5), (0x330, f4, f2), (0x340, f4, f5)] {
                fx.mem.write_u32(sp + o, 0);
                fx.mem.setf(sp + o + 4, y);
                fx.mem.setf(sp + o + 8, z);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            let t2 = rem2(s4 as i32, 4);
            let s0 = ee::div(ee::mul(32.0, ee::from_i32(t2)), 128.0);
            let s1 = ee::add(0.25, s0);
            for (o, s, t) in [(0x448u32, s0, 0x3F80_0000u32), (0x460, s1, 0x3F80_0000), (0x478, s0, 0), (0x490, s1, 0)] {
                fx.mem.setf(sp + o, s);
                fx.mem.write_u32(sp + o + 4, t);
            }
            let mm = fx.read_m4(sp + 0x250);
            if fx.project_desc(sp + 0x310, PROJ_OUT, Some(&mm), q, sp + 0x5BC) {
                fx.submit(q, OT, 0xFF_FFFF);
            }
        }
        let age = fx.mem.i32(rec + 0x311C) + 1;
        fx.mem.set32(rec + 0x311C, age);
        if age >= 0x41 || fx.mem.i32(D) == 0x14A {
            fx.mem.write_u8(rec + 0x3118, 0);
        }
    }
}
