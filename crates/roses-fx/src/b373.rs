//! BMODULE b373 (card 145), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x8F0 frame, data area a0 =
//! 0x1ED0000, 0xB080 bytes; two unit-target post callbacks 0x1E04890 (b424's
//! sphere, 0x220 frame) / 0x1E04DB0 (b424's veil, 0x50) with their static D
//! at 0x1E04F00) [F]: b704's program (card 10;
//! written fresh on b424's pieces from the listing, the locals at the
//! entry's own offsets) with four nodes, a water surface and a fence, a
//! mote burst and two puffs. The init lays out a 21 x 21 water grid
//! (D+0xE0, 0x150 a row, x / z -450..450, a random phase each at D+0x1C70),
//! four fence edges of 17 posts (D+0x2360 + 0x220 e, a post 0x20: the foot
//! and the foot raised to y = 70), the sphere (radius 90, D+0x2BF0) and 9
//! ramps (D+0x3E08: rgb 128 (1 - i / 8) + 128 i / 8 (blue 228 (1 - i / 8) +
//! 128 i / 8), alpha 128 (1 - i / 8)). Every update the camera turn with
//! roll (sp + 0x570), the attacker's / target's points (D+0x30 / D+0x60),
//! nodes 0xF (kind 1) / 0x13 (D+0x90 / 0xA0) and 6 / 9 (D+0xC0 / 0xD0, each
//! pushed by (-5 side (2 (k % 2) - 1), -10, -15 side)), the screen points of
//! the attacker's point (D+0x40) and the target's position (D+0x70). Then:
//! on 0xA0 and 0x10A a 33-update tint (FF, FF, E0) (D+0x7B48); on 0x195 the
//! white fade (D+0x7B38, 0x73 updates; at age 0x1E it ends the frame fan,
//! the tint, the water and the attacker pass) and the frame fan (D+0x7B30,
//! 0x1F updates); on even updates 0x109..0x1B2 the zoom (D+0x7B40, (C8,
//! C8, C8)); on 0x109 two node glows (D+0x3E00, 0xAB updates, (30, 80,
//! FF), ramp 170 / 166, keyed by their w); every update 0x10A..0x1B2 one of
//! 16 spiral records (D+0x3E30, 0x3D0: two nodes x six ribbons of 8 quads,
//! width 20 / speed -300 side / amp 20, spread (0x238E, 0x11C7)); every
//! update 0x10B..0x1B2 one of 4 bolt records (D+0x7B50: strips (40, 80, FF),
//! sprites (20, 40, FF), width 15, length 2.5; far Z only from 0x142); from
//! 0xA0 the water (D+0x2354, 0x10FF updates: the grid's heights 7 sin(phase
//! + 0x457 age) as textured quads (8C, 8C, B4) at y 15 with drifting ST,
//! for 0x38 updates with line overlays and the fence's 64 quads (FF, FF,
//! E0) to black); on 0xA0 a 512-mote burst at the attacker's point
//! (D+0x8E50, 0x41 updates or until 0xB8); from 0x6E the attacker pass
//! (D+0x2BE0: the sphere callback, five widening fans, keyed from 0x142);
//! on even updates of the window the target pass (the veil); on 0x3C / 0x57
//! a puff at node 6 / 9 (D+0x89D0, 2 x 0x240); on 6 the attacker's (40, 40,
//! FF) flash (D+0xB070) as three fans. The stage light 0x80 - 128 clamp(n,
//! 0, 0x40) clamp(0x207 - n, 0, 0x40) / 4096. Returns 1 on 0x100, 6 on
//! 0x10B..0x1B2, 3 on 0x1B3, 8 once the count reaches 0x208.
use crate::b182::{alpha, clamp_regs, far, head, r256, rgba, uv};
use crate::b424::{camera, fan, fan_desc, sphere, veil, window};
use crate::b440::{copy8, fix, fixs, rem2, vec4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const D_STATIC: u32 = 0x01E0_4F00;
const CB_A: u32 = 0x01E0_4890;
const CB_B: u32 = 0x01E0_4DB0;
const BOLT_CLAMP: u64 = 0x0000_03FC_811F_C0BA;
/// b424's 0x250.. locals (camera, turn, points) sit 0x2A0 higher, its
/// 0x3A8.. descriptors 0x298 higher.
const UPA: u32 = 0x2A0;
const UPB: u32 = 0x298;
/// The mote record (D+0x8E50) and the puffs (D+0x89D0) and the flash.
const MOTE: u32 = 0x8E50;
const PUFF: u32 = 0x89D0;
const FLASH: u32 = 0xB070;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x8F0);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x8F0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xB080);
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
    for (o, node, kind) in [(0x80u32, 0xF, 1), (0x84, 0x13, 0), (0xB0, 6, 0), (0xB4, 9, 0)] {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, kind);
        fx.mem.set32(D + o, s);
    }
    // The water grid: 21 x 21 points, a random phase each.
    for s0 in 0..0x15i32 {
        let f20 = ee::div(ee::mul(900.0, ee::from_i32(s0)), 20.0);
        for s1 in 0..0x15i32 {
            let p = D + 0x150 * s0 as u32 + 0x10 * s1 as u32;
            let f0 = ee::sub(ee::div(ee::mul(900.0, ee::from_i32(s1)), 20.0), 450.0);
            fx.mem.setf(p + 0xE0, ee::sub(f20, 450.0));
            fx.mem.write_u32(p + 0xE4, 0);
            fx.mem.setf(p + 0xE8, f0);
            fx.mem.write_u32(p + 0xEC, 0x3F80_0000);
            let c = r256(fx, 0x01E0_0268);
            fx.mem.set32(D + 0x54 * s0 as u32 + 4 * s1 as u32 + 0x1C70, (c << 16) >> 8);
        }
    }
    fx.mem.write_u8(D + 0x2354, 0);
    // The fence: four edges of 17 posts (foot, then the foot at y = 70).
    for t0 in 0..0x11i32 {
        let a3 = D + 0x20 * t0 as u32;
        let f0 = ee::sub(ee::div(ee::mul(900.0, ee::from_i32(t0)), 16.0), 450.0);
        let one = 0x3F80_0000u32;
        let (x450, n450) = (450.0f32.to_bits(), (-450.0f32).to_bits());
        vec4(fx, a3 + 0x2360, [f0.to_bits(), 0, n450, one]);
        vec4(fx, a3 + 0x2580, [f0.to_bits(), 0, x450, one]);
        vec4(fx, a3 + 0x27A0, [n450, 0, f0.to_bits(), one]);
        vec4(fx, a3 + 0x29C0, [x450, 0, f0.to_bits(), one]);
        for t1 in 0..4u32 {
            let a2 = a3 + 0x220 * t1;
            for w in 0..4u32 {
                let v = fx.mem.read_u32(a2 + 0x2360 + 4 * w);
                fx.mem.write_u32(a2 + 0x2370 + 4 * w, v);
            }
            fx.mem.write_u32(a2 + 0x2374, 0x428C_0000);
        }
    }
    fx.mem.write_u8(D + 0x2BE0, 0);
    // The sphere: 17 rings (angle 0x800 i) of 17 points, radius 90.
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
            fx.mem.setf(p + 0x2BF0, ee::mul(f20, sn));
            fx.mem.setf(p + 0x2BF4, f21);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 0x2BF8, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0x2BFC, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0x3E00, 0);
    fx.mem.write_u32(D + 0x3E04, 0);
    for i in 0..9 {
        let f1 = ee::div(ee::from_i32(i), 8.0);
        let f22 = ee::sub(1.0, f1);
        let f21 = ee::mul(128.0, f1);
        let f23 = ee::mul(128.0, f22);
        let f20 = ee::add(f23, f21);
        let c = D + 0x3E08 + 4 * i as u32;
        let v = fix(fx, f20, 0x01E0_0494);
        fx.mem.write_u8(c, v as u8);
        let v = fix(fx, f20, 0x01E0_04A0);
        fx.mem.write_u8(c + 1, v as u8);
        let v = fix(fx, ee::add(ee::mul(228.0, f22), f21), 0x01E0_04BC);
        fx.mem.write_u8(c + 2, v as u8);
        let v = fix(fx, f23, 0x01E0_04C8);
        fx.mem.write_u8(c + 3, v as u8);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x3E30 + 0x3D0 * k, 0);
    }
    for o in [0x7B30u32, 0x7B38, 0x7B40, 0x7B48] {
        fx.mem.write_u8(D + o, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x7B50 + 0x3A0 * k, 0);
    }
    for k in 0..2u32 {
        fx.mem.write_u8(D + PUFF + 0x240 * k, 0);
    }
    fx.mem.write_u8(D + MOTE, 0);
    fx.mem.write_u8(D + FLASH, 0);
    0
}

fn win(n: i32) -> bool {
    (0x109..0x1B3).contains(&n)
}

/// The screen point of `p` (0x204D80 through sp + 0x4F0 into sp + 0x8E0)
/// into `o` (b424's screen_point at this record's offsets).
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + 0x4F0);
    fx.project_at(p, &m, sp + 0x8E0, sp + 0x8EC);
    let x = (i32::from(fx.mem.read_u16(sp + 0x8E0)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x8E2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x8E0) << 8) >> 40;
    fx.mem.setf(o + 8, z as u32 as f32);
    fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    for k in [0u32, 4] {
        if crate::exe::ee_lt(fx.mem.f(o + k), 0.0) {
            fx.mem.write_u32(o + k, 0);
        }
    }
    for k in [0u32, 4] {
        if !crate::exe::ee_le(fx.mem.f(o + k), 512.0) {
            fx.mem.write_u32(o + k, 0x4400_0000);
        }
    }
}

/// translate_rows (0x188420) noting the upper half of its t0 (`lq t0,
/// 0x10(a1)`: the input's row 1 z / w).
fn translate(fx: &mut Fx, hi: &mut u64, out: u32, m: u32, v: u32) {
    *hi = fx.mem.read_u64(m + 0x18);
    fx.translate_rows(out, m, v);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    let mut hi = 0u64;
    fx.camera_turn_roll(sp + 0x570);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x60);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x80 + 4 * k);
        fx.node_position(h, slot, D + 0x90 + 0x10 * k);
    }
    for k in 0..2i32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0xB0 + 4 * k as u32);
        let p = D + 0xC0 + 0x10 * k as u32;
        fx.node_position(h, slot, p);
        let sign = ee::from_i32(rem2(k, 2) * 2 - 1);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        fx.mem.setf(sp + 0x630, ee::mul(side, ee::mul(-5.0, sign)));
        fx.mem.setf(sp + 0x634, -10.0);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        fx.mem.setf(sp + 0x638, ee::mul(-15.0, side));
        fx.mem.write_u32(sp + 0x63C, 0x3F80_0000);
        fx.vu_add4(p, p, sp + 0x630);
    }
    camera(fx, sp + UPA);
    screen_point(fx, sp, D + 0x30, D + 0x40);
    camera(fx, sp + UPA);
    screen_point(fx, sp, D + 0x50, D + 0x70);

    tint(fx, sp);
    fade(fx, sp);
    frame_fan(fx, sp);
    zoom(fx, sp);
    let f23 = glow(fx, sp, &mut hi);
    spirals(fx, sp, f23);
    bolts(fx, sp, &mut hi);
    water(fx, sp, &mut hi);
    motes(fx, sp);
    attacker_pass(fx, sp, hi);
    target_pass(fx, sp);
    puffs(fx, sp);
    white_pass(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x207 - n, 0, 0x40);
    let n = fx.mem.i32(D);
    let k = Fx::clamp_i(n, 0, 0x40);
    let l = ((k.wrapping_neg() << 7).wrapping_mul(s0) >> 12) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xD0);
    if n == 0x100 {
        ret = 1;
    } else {
        if (0x10B..0x1B3).contains(&n) {
            ret = 6;
        }
        if n == 0x1B3 {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x208 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xD0, ret);
    ret
}

/// The tint (D+0x7B48, 33 updates from 0xA0 and 0x10A) [F 0x1E008C0..0x1E00A24].
fn tint(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n == 0xA0 || n == 0x10A {
        fx.mem.write_u8(D + 0x7B48, 1);
        fx.mem.write_u32(D + 0x7B4C, 0);
    }
    if fx.mem.read_u8(D + 0x7B48) != 1 {
        return;
    }
    window(fx, sp + UPB, 0, [0xFF, 0xFF, 0xE0], |fx| {
        let age = fx.mem.i32(D + 0x7B4C);
        fix(fx, ee::mul(128.0, ee::div(ee::from_i32(0x20 - age), 32.0)), 0x01E0_0964)
    });
    let age = fx.mem.i32(D + 0x7B4C) + 1;
    fx.mem.set32(D + 0x7B4C, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x7B48, 0);
    }
}

/// b424's white fade (D+0x7B38, 0x73 updates from 0x195); at age 0x1E it
/// ends the frame fan, the tint, the water and the attacker pass [F
/// 0x1E00A28..0x1E00BBC].
fn fade(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x195 {
        fx.mem.write_u8(D + 0x7B38, 1);
        fx.mem.write_u32(D + 0x7B3C, 0);
    }
    if fx.mem.read_u8(D + 0x7B38) != 1 {
        return;
    }
    if fx.mem.i32(D + 0x7B3C) == 0x1E {
        for o in [0x7B30u32, 0x7B48, 0x2354, 0x2BE0] {
            fx.mem.write_u8(D + o, 0);
        }
    }
    window(fx, sp + UPB, 1, [0xFF; 3], |fx| {
        let age = fx.mem.i32(D + 0x7B3C);
        let c = Fx::clamp_i(age - 0x32, 0, 0x40);
        let f20 = ee::sub(1.0, fx.tan_falloff_p(c, 0x40, 1));
        let age = ee::from_i32(fx.mem.i32(D + 0x7B3C));
        let q = Fx::ratio_below(age, 30.0);
        fix(fx, ee::mul(128.0, ee::mul(q, f20)), 0x01E0_0B00)
    });
    let age = fx.mem.i32(D + 0x7B3C) + 1;
    fx.mem.set32(D + 0x7B3C, age);
    if age >= 0x73 {
        fx.mem.write_u8(D + 0x7B38, 0);
    }
}

const FAN_T: [u32; 12] = [
    0x01E0_0D38, 0x01E0_0D44, 0x01E0_0D88, 0x01E0_0D94, 0x01E0_0DD8, 0x01E0_0DE4, 0x01E0_0E98, 0x01E0_0EA4, 0x01E0_0F10, 0x01E0_0F1C, 0x01E0_0F88,
    0x01E0_0F94,
];
const FAN_A: [u32; 12] = [
    0x01E0_3778, 0x01E0_3784, 0x01E0_37D4, 0x01E0_37E0, 0x01E0_3830, 0x01E0_383C, 0x01E0_38F4, 0x01E0_3900, 0x01E0_3970, 0x01E0_397C, 0x01E0_39F0,
    0x01E0_39FC,
];
const FAN_B: [u32; 12] = [
    0x01E0_4430, 0x01E0_443C, 0x01E0_4480, 0x01E0_448C, 0x01E0_44D0, 0x01E0_44DC, 0x01E0_4590, 0x01E0_459C, 0x01E0_4608, 0x01E0_4614, 0x01E0_4684,
    0x01E0_4690,
];

/// The frame-textured fan at the target's screen point (D+0x7B30, 0x1F
/// updates from 0x195) [F 0x1E00BC0..0x1E0104C].
fn frame_fan(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x195 {
        fx.mem.write_u8(D + 0x7B30, 1);
        fx.mem.write_u32(D + 0x7B34, 0);
    }
    if fx.mem.read_u8(D + 0x7B30) != 1 {
        return;
    }
    let d = sp + 0x758;
    fan_desc(fx, sp + UPB, 0);
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let age = ee::from_i32(fx.mem.i32(D + 0x7B34));
    let q = Fx::ratio_below(age, 20.0);
    let a = fix(fx, ee::mul(128.0, q), 0x01E0_0C88);
    rgba(fx, sp + 0x768, [0x80; 3], a as u8);
    copy8(fx, sp + 0x780, sp + 0x768);
    copy8(fx, sp + 0x798, sp + 0x768);
    let s = ee::add(1.0, ee::mul(f(0x3DCC_CCCD), ee::from_i32(1)));
    fan(fx, d, D + 0x70, sp + 0x5F0, s, OT, &FAN_T);
    let age = fx.mem.i32(D + 0x7B34) + 1;
    fx.mem.set32(D + 0x7B34, age);
    if age >= 0x1F {
        fx.mem.write_u8(D + 0x7B30, 0);
    }
}

/// The frame zoom (D+0x7B40, two updates, on even updates of the window)
/// [F 0x1E01050..0x1E0130C].
fn zoom(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if win(n) && rem2(n, 2) == 0 {
        fx.mem.write_u8(D + 0x7B40, 1);
        fx.mem.write_u32(D + 0x7B44, 0);
    }
    if fx.mem.read_u8(D + 0x7B40) != 1 {
        return;
    }
    let d = sp + 0x670;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x6B0, sp + 0x6B8);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x7B44);
    let al = fix(fx, ee::mul(32.0, ee::from_i32(1 - age)), 0x01E0_1160);
    rgba(fx, sp + 0x680, [0xC8; 3], al as u8);
    uv(fx, sp + 0x688, 0);
    uv(fx, sp + 0x68A, 0);
    uv(fx, sp + 0x698, 0x2000);
    uv(fx, sp + 0x69A, 0x2000);
    for k in 0..4i32 {
        let t = ee::div(ee::mul(f(0x3DCC_CCCD), ee::from_i32(k + 1)), 4.0);
        let f20 = ee::mul(256.0, ee::add(1.0, t));
        let x = fixs(fx, f20, 0x01E0_122C);
        let y = fixs(fx, f20, 0x01E0_1238);
        far_xy(fx, sp + 0x690, ((0x800 - x) << 4) as u16, ((0x800 - y) << 4) as u16);
        far_xy(fx, sp + 0x6A0, ((x + 0x800) << 4) as u16, ((y + 0x800) << 4) as u16);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x7B44) + 1;
    fx.mem.set32(D + 0x7B44, age);
    if age >= 2 {
        fx.mem.write_u8(D + 0x7B40, 0);
    }
}

/// The two node glows (D+0x3E00, from 0x109 for 0xAB updates), keyed by
/// their w [F 0x1E01310..0x1E0159C]. Returns the spirals' alpha factor.
fn glow(fx: &mut Fx, sp: u32, hi: &mut u64) -> f32 {
    if fx.mem.i32(D) == 0x109 {
        fx.mem.write_u8(D + 0x3E00, 1);
        fx.mem.write_u32(D + 0x3E04, 0);
    }
    let age = ee::from_i32(fx.mem.i32(D + 0x3E04));
    let f20 = Fx::ratio_below(age, 4.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x3E04));
    let f0 = Fx::ramp_above(age, 170.0, 166.0);
    let f23 = ee::mul(f20, f0);
    if fx.mem.read_u8(D + 0x3E00) != 1 {
        return f23;
    }
    let d = sp + 0x670;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    alpha(fx, d, 1);
    let m = rem2(fx.mem.i32(D + 0x3E04), 2);
    let s = ee::add(f(0x3E99_999A), ee::mul(f(0x3F33_3333), ee::from_i32(m)));
    let a = fix(fx, ee::mul(ee::mul(255.0, s), f23), 0x01E0_1428);
    rgba(fx, sp + 0x680, [0x30, 0x80, 0xFF], a as u8);
    fx.mem.setf(sp + 0x5B0, -35.0);
    fx.mem.setf(sp + 0x5B4, -35.0);
    fx.mem.write_u32(sp + 0x5B8, 0);
    fx.mem.write_u32(sp + 0x5BC, 0x3F80_0000);
    vec4(fx, sp + 0x5C0, [0x420C_0000, 0x420C_0000, 0, 0x3F80_0000]);
    fx.vu_apply_to(sp + 0x5B0, sp + 0x570, sp + 0x5B0);
    fx.vu_apply_to(sp + 0x5C0, sp + 0x570, sp + 0x5C0);
    uv(fx, sp + 0x688, 0x600);
    uv(fx, sp + 0x68A, 0);
    uv(fx, sp + 0x698, 0x800);
    uv(fx, sp + 0x69A, 0x200);
    for k in 0..2u32 {
        fx.camera_vp_to(sp + 0x4F0);
        fx.identity(sp + 0x530);
        translate(fx, hi, sp + 0x530, sp + 0x530, D + 0x90 + 0x10 * k);
        crate::blib::exe::mul_mem(fx, sp + 0x4F0, sp + 0x530);
        if crate::fxlib2::project(fx, sp + 0x5B0, PROJ_OUT, sp + 0x4F0, d, sp + 0x8EC) {
            let w = fx.mem.f(sp + 0x8EC);
            let key = fix(fx, w, 0x01E0_155C);
            fx.submit(d, OT, key as i32);
        }
    }
    let age = fx.mem.i32(D + 0x3E04) + 1;
    fx.mem.set32(D + 0x3E04, age);
    if age >= 0xAB {
        fx.mem.write_u8(D + 0x3E00, 0);
    }
    f23
}

/// HI of 0x2AAAAAAB x `x` (the record's mult) and the sign bit of `x`.
fn div6(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    hi.wrapping_add(((x as u32) >> 31) as i32)
}

/// The spiral records (D+0x3E30, 16 x 0x3D0): spawned at n % 16 every
/// update 0x10A..0x1B2, two nodes x six ribbons of 8 quads [F
/// 0x1E015A0..0x1E01D70]. Ribbon j (node j / 6): life +0x8 + 4 j, width
/// +0x38, speed +0x68, amp +0x98, turn +0xD0 + 0x40 j.
fn spirals(fx: &mut Fx, sp: u32, f23: f32) {
    let n = fx.mem.i32(D);
    if (0x10A..0x1B3).contains(&n) {
        let r = D + 0x3D0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x3E30, 1);
        fx.mem.write_u32(r + 0x3E34, 0);
        for node in 0..2u32 {
            for m in 0..6u32 {
                let j = 6 * node + m;
                let s1 = r + 4 * j;
                let s2 = r + 0x40 * j;
                let s3 = (m << 16) as i32;
                let c = r256(fx, 0x01E0_1618);
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                let life = fixs(fx, ee::mul(16.0, ee::add(f(0x3F33_3333), x)), 0x01E0_167C);
                fx.mem.set32(s1 + 0x3E38, life);
                let c = r256(fx, 0x01E0_1684) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x3E68, ee::mul(20.0, ee::add(0.5, x)));
                let c = r256(fx, 0x01E0_16E0) + 1;
                let side = ee::from_i32(fx.mem.i32(D + 0xC));
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x3E98, ee::mul(side, ee::mul(-300.0, ee::add(f(0x3F33_3333), x))));
                let c = r256(fx, 0x01E0_1750) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x3EC8, ee::mul(20.0, ee::add(0.5, x)));
                fx.identity(s2 + 0x3F00);
                let c = r256(fx, 0x01E0_17B8);
                fx.rotate_axis(s2 + 0x3F00, (c.wrapping_mul(0x238E) >> 8) - 0x11C7, 1);
                let c = r256(fx, 0x01E0_17F4);
                fx.rotate_axis(s2 + 0x3F00, (c.wrapping_mul(0x238E) >> 8) - 0x11C7, 0);
                let a = div6(fx, s3);
                fx.rotate_axis(s2 + 0x3F00, a, 2);
            }
        }
    }
    let q = sp + 0x6C8;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 5);
    for k in 0..0x10u32 {
        let rec = D + 0x3D0 * k;
        if fx.mem.read_u8(rec + 0x3E30) == 0 {
            continue;
        }
        for node in 0..2u32 {
            let mut s5 = (0xD05 * k + 0x115C * node) as i32;
            for m in 0..6u32 {
                let j = 6 * node + m;
                let s4 = rec + 4 * j;
                let age = fx.mem.i32(rec + 0x3E34);
                let life = fx.mem.i32(s4 + 0x3E38);
                if life >= age {
                    ribbon(fx, sp, rec, node, j, m, s4, s5, f23);
                }
                s5 = s5.wrapping_add(0x15B3);
            }
        }
        let age = fx.mem.i32(rec + 0x3E34) + 1;
        fx.mem.set32(rec + 0x3E34, age);
        if age >= 0x11 {
            fx.mem.write_u8(rec + 0x3E30, 0);
        }
    }
}

/// One ribbon of 8 quads at node `node` (ribbon `j`, `m` within the node).
#[allow(clippy::too_many_arguments)]
fn ribbon(fx: &mut Fx, sp: u32, rec: u32, node: u32, j: u32, m: u32, s4: u32, s5: i32, f23: f32) {
    let q = sp + 0x6C8;
    fx.camera_vp_to(sp + 0x4F0);
    let p = D + 0x90 + 0x10 * node;
    let t = std::array::from_fn(|c| fx.mem.f(p + 4 * c as u32));
    let mm = fx.trs(None, None, Some(t));
    fx.write_m4(sp + 0x530, &mm);
    crate::blib::exe::mul_mem(fx, sp + 0x530, rec + 0x40 * j + 0x3F00);
    crate::blib::exe::mul_mem(fx, sp + 0x4F0, sp + 0x530);
    let age = fx.mem.i32(rec + 0x3E34);
    let life = fx.mem.i32(s4 + 0x3E38);
    let f0 = fx.tan_falloff_p(age, life, 1);
    let f26 = ee::sub(1.0, f0);
    let f20 = ee::div(ee::mul(32.0, ee::from_i32(rem2(m as i32, 4))), 128.0);
    let f24 = ee::add(0.25, f20);
    let f25 = ee::add(1.0, ee::sub(1.0, f26));
    let mut s3 = 0i32;
    for s1 in 0..8i32 {
        let c = D + 0x3E08 + 4 * s1 as u32;
        let a0 = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
        let s6 = fix(fx, ee::mul(f23, ee::mul(a0, f26)), 0x01E0_1A50) & 0xFF;
        let a1 = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
        let v0 = fix(fx, ee::mul(f23, ee::mul(a1, f26)), 0x01E0_1A94);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + k);
            fx.mem.write_u8(sp + 0x6D8 + k, b);
        }
        fx.mem.write_u8(sp + 0x6DB, s6 as u8);
        fx.mem.write_u32(sp + 0x6DC, 0x3F80_0000);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + 4 + k);
            fx.mem.write_u8(sp + 0x708 + k, b);
        }
        fx.mem.write_u8(sp + 0x70B, v0 as u8);
        fx.mem.write_u32(sp + 0x70C, 0x3F80_0000);
        copy8(fx, sp + 0x6F0, sp + 0x6D8);
        copy8(fx, sp + 0x720, sp + 0x708);
        let t1 = rem2(s1, 4);
        let f3 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1)), 4.0), 128.0);
        fx.mem.setf(sp + 0x6E0, f20);
        fx.mem.setf(sp + 0x6E4, f3);
        fx.mem.setf(sp + 0x6F8, f24);
        fx.mem.setf(sp + 0x6FC, f3);
        fx.mem.setf(sp + 0x710, f20);
        let f0 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(t1 + 1)), 4.0), 128.0);
        fx.mem.setf(sp + 0x714, f0);
        fx.mem.setf(sp + 0x728, f24);
        fx.mem.setf(sp + 0x72C, f0);
        let a = s5.wrapping_add(s3 >> 3) & 0xFFFF;
        let f21 = ee::div(ee::from_i32(s1), 8.0);
        let f22 = ee::div(ee::from_i32(s1 + 1), 8.0);
        let f4 = fx.mem.f(s4 + 0x3E98);
        let f29 = ee::mul(fx.mem.f(s4 + 0x3E68), f26);
        let f27 = ee::mul(f4, f21);
        let f28 = ee::mul(f4, f22);
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x3EC8);
        let f21 = ee::mul(f21, ee::mul(f25, ee::mul(amp, sn)));
        let a = s5.wrapping_add(((s1 + 1) << 16) >> 3) & 0xFFFF;
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x3EC8);
        let f3 = ee::neg(f29);
        let f1 = ee::mul(f22, ee::mul(f25, ee::mul(amp, sn)));
        fx.mem.setf(sp + 0x5B0, f21);
        fx.mem.setf(sp + 0x5B4, ee::add(f3, f21));
        fx.mem.setf(sp + 0x5B8, f27);
        fx.mem.write_u32(sp + 0x5BC, 0x3F80_0000);
        fx.mem.setf(sp + 0x5C0, f21);
        fx.mem.setf(sp + 0x5C4, ee::add(f29, f21));
        fx.mem.setf(sp + 0x5C8, f27);
        fx.mem.write_u32(sp + 0x5CC, 0x3F80_0000);
        fx.mem.setf(sp + 0x5D0, f1);
        fx.mem.setf(sp + 0x5D4, ee::add(f3, f1));
        fx.mem.setf(sp + 0x5D8, f28);
        fx.mem.write_u32(sp + 0x5DC, 0x3F80_0000);
        fx.mem.setf(sp + 0x5E0, f1);
        fx.mem.setf(sp + 0x5E4, ee::add(f29, f1));
        fx.mem.setf(sp + 0x5E8, f28);
        fx.mem.write_u32(sp + 0x5EC, 0x3F80_0000);
        let mm = fx.read_m4(sp + 0x4F0);
        if fx.project_desc(sp + 0x5B0, PROJ_OUT, Some(&mm), q, sp + 0x8EC) {
            fx.submit(q, OT, 0);
        }
        s3 = s3.wrapping_add(0x10000);
    }
}

/// The bolt records (D+0x7B50, 4 x 0x3A0): b424's, spawned at n % 4 every
/// update 0x10B..0x1B2 (width 15, length 2.5), far Z only from 0x142 [F
/// 0x1E01D74..0x1E02718].
fn bolts(fx: &mut Fx, sp: u32, hi: &mut u64) {
    let n = fx.mem.i32(D);
    if (0x10B..0x1B3).contains(&n) {
        let r = D + 0x3A0 * rem2(n, 4) as u32;
        fx.mem.write_u8(r + 0x7B50, 1);
        fx.mem.write_u32(r + 0x7B54, 0);
        for s4 in 0..0x20u32 {
            let s1 = r + 4 * s4;
            let s2 = r + 0x10 * s4;
            let c = r256(fx, 0x01E0_1DDC);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(4.0, ee::add(f(0x3E99_999A), x)), 0x01E0_1E40);
            fx.mem.set32(s1 + 0x7B58, life);
            let c = r256(fx, 0x01E0_1E48);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x7C58, ee::mul(15.0, ee::add(f(0x3E99_999A), x)));
            let c = r256(fx, 0x01E0_1EB0);
            fx.mem.set32(s1 + 0x7BD8, (c << 16) >> 8);
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x60 + 4 * w);
                fx.mem.write_u32(r + 0x7CE0 + 4 * w, x);
            }
            fx.random_table_vec(s2 + 0x7CF0, 2.5);
        }
    }
    let b = sp + 0x7D0;
    head(fx, b, 0x24, 9, 0x4_2424_2421, 0x800);
    alpha(fx, b, 1);
    let rh = fx.mem.read_u32(D + 0x24);
    fx.desc_texture(b, rh, 1);
    fx.desc_clamp(b, BOLT_CLAMP);
    let d = sp + 0x670;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    for k in 0..4u32 {
        let s3 = D + 0x3A0 * k;
        if fx.mem.read_u8(s3 + 0x7B50) == 0 {
            continue;
        }
        let s6 = s3 + 0x7CE0;
        for s5 in 0..0x20i32 {
            let s1 = s3 + 4 * s5 as u32;
            let s2 = s3 + 0x10 * s5 as u32;
            let age = fx.mem.i32(s3 + 0x7B54);
            let life = fx.mem.i32(s1 + 0x7B58);
            if life < age {
                continue;
            }
            let (q, _) = fx.div(age << 14, life);
            let f20 = fx.tcos(q);
            let a = fix(fx, ee::mul(64.0, f20), 0x01E0_20D4);
            rgba(fx, sp + 0x7E0, [0x40, 0x80, 0xFF], a as u8);
            let a = fix(fx, ee::mul(255.0, f20), 0x01E0_2108);
            rgba(fx, sp + 0x680, [0x20, 0x40, 0xFF], a as u8);
            let c4 = rem2(s5, 4);
            let a1 = rem2(c4, 2);
            let row = ((c4 >> 1) << 6) + 0x80;
            let u0 = 59 * a1 + 0xA;
            let (t2, a3) = (((u0 << 4) & 0x3FFF) as u32, (((u0 + 0x3B) << 4) & 0x3FFF) as u32);
            let (t0, a2) = (((row << 4) & 0x3FFF) as u32, (((row + 0x40) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x7E8, t2);
            uv(fx, sp + 0x7EA, t0);
            uv(fx, sp + 0x7F8, a3);
            uv(fx, sp + 0x7FA, t0);
            uv(fx, sp + 0x808, t2);
            uv(fx, sp + 0x80A, a2);
            uv(fx, sp + 0x818, a3);
            uv(fx, sp + 0x81A, a2);
            let c8 = rem2(s5, 8);
            let cu = rem2(c8, 4);
            let cv = c8 >> 2;
            uv(fx, sp + 0x688, ((cu << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x68A, ((cv << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x698, ((((cu << 5) + 0x20) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x69A, ((((cv << 5) + 0x20) << 4) & 0x3FFF) as u32);
            let age = fx.mem.i32(s3 + 0x7B54);
            let life = fx.mem.i32(s1 + 0x7B58);
            let (q, _) = fx.div(age << 14, life);
            let sn = fx.tsin(q);
            let f20 = fx.mem.f(s1 + 0x7C58);
            let f3 = ee::mul(ee::mul(0.5, f20), sn);
            let f2 = ee::neg(f20);
            let (a, b1, c2, d0) = (ee::add(f2, f3), ee::sub(f2, f3), ee::add(f20, f3), ee::sub(f20, f3));
            for (o, x, y) in [(0x5B0u32, a, b1), (0x5C0, c2, b1), (0x5D0, b1, c2), (0x5E0, d0, c2)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, y);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x4F0);
            fx.vu_add4(sp + 0x630, s2 + 0x7CF0, s6);
            translate(fx, hi, sp + 0x530, sp + 0x570, sp + 0x630);
            let ang = fx.mem.i32(s1 + 0x7BD8);
            fx.rotate_axis(sp + 0x530, ang, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x4F0, sp + 0x530);
            if crate::fxlib2::project(fx, sp + 0x5B0, PROJ_OUT, sp + 0x4F0, b, sp + 0x8EC) {
                if fx.mem.i32(D) >= 0x142 {
                    far(fx, sp + 0x7F0, &[sp + 0x800, sp + 0x810, sp + 0x820]);
                }
                fx.submit(b, OT, 0);
            }
            let s = ee::mul(f(0x3DCC_CCCD), fx.mem.f(s1 + 0x7C58));
            let ns = ee::neg(s);
            for (o, x) in [(0x5B0u32, ns), (0x5C0, s)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x4F0);
            let age = ee::from_i32(fx.mem.i32(s3 + 0x7B54));
            let life = ee::from_i32(fx.mem.i32(s1 + 0x7B58));
            let t = ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), life));
            fx.vu_scale4(sp + 0x630, s2 + 0x7CF0, ee::mul(-1.0, t));
            fx.vu_add4(sp + 0x630, sp + 0x630, s6);
            translate(fx, hi, sp + 0x530, sp + 0x570, sp + 0x630);
            crate::blib::exe::mul_mem(fx, sp + 0x4F0, sp + 0x530);
            if crate::fxlib2::project(fx, sp + 0x5B0, PROJ_OUT, sp + 0x4F0, d, sp + 0x8EC) {
                if fx.mem.i32(D) >= 0x142 {
                    far(fx, sp + 0x690, &[sp + 0x6A0]);
                }
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(s3 + 0x7B54) + 1;
        fx.mem.set32(s3 + 0x7B54, age);
        if age >= 5 {
            fx.mem.write_u8(s3 + 0x7B50, 0);
        }
    }
}

/// The water (D+0x2354, 0x10FF updates from 0xA0, ended by the fade): for
/// 0x38 updates the fence's 4 x 16 quads (FF, FF, E0) fading up to black,
/// then the 20 x 20 grid quads (8C, 8C, B4) at y 15, their heights 7
/// sin(phase + 0x457 age), ST drifting by 0.2 (sin, cos)(-(age % 128) <<
/// 9), with line overlays (FF, FF, E0) while the fence shows [F
/// 0x1E0271C..0x1E02EB0].
fn water(fx: &mut Fx, sp: u32, hi: &mut u64) {
    if fx.mem.i32(D) == 0xA0 {
        fx.mem.write_u8(D + 0x2354, 1);
        fx.mem.write_u32(D + 0x2358, 0);
    }
    if fx.mem.read_u8(D + 0x2354) != 1 {
        return;
    }
    let b = sp + 0x7D0;
    head(fx, b, 0x24, 9, 0x4_2424_2421, 0);
    alpha(fx, b, 0);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(b, rh, 1);
    fx.desc_clamp(b, 0);
    let l = sp + 0x848;
    head(fx, l, 4, 5, 0x4_4441, 0);
    alpha(fx, l, 1);
    let g = sp + 0x888;
    head(fx, g, 0x14, 8, 0x4141_4141, 0);
    alpha(fx, g, 1);
    let age = ee::from_i32(fx.mem.i32(D + 0x2358));
    let f20 = Fx::ramp_above(age, f(0x4587_F000), f(0x4586_F000));
    let age = fx.mem.i32(D + 0x2358);
    let c = Fx::clamp_i(0x38 - age, 0, 8);
    let f3 = ee::div(ee::from_i32(c), 8.0);
    let m = rem2(fx.mem.i32(D + 0x2358), 2);
    let f0 = ee::add(f(0x3F66_6666), ee::mul(f(0x3DCC_CCCD), ee::from_i32(m)));
    let f21 = ee::mul(f0, f3);
    let a = fix(fx, ee::mul(128.0, f21), 0x01E0_28E4);
    rgba(fx, sp + 0x858, [0xFF, 0xFF, 0xE0], a as u8);
    if fx.mem.i32(D + 0x2358) < 0x38 {
        let a = fix(fx, ee::mul(255.0, f21), 0x01E0_2924);
        rgba(fx, sp + 0x898, [0xFF, 0xFF, 0xE0], a as u8);
        copy8(fx, sp + 0x8A8, sp + 0x898);
        rgba(fx, sp + 0x8B8, [0; 3], 0);
        copy8(fx, sp + 0x8C8, sp + 0x8B8);
        fx.camera_vp_to(sp + 0x4F0);
        fx.identity(sp + 0x530);
        vec4(fx, sp + 0x630, [0, 0x3F80_0000, 0, 0x3F80_0000]);
        translate(fx, hi, sp + 0x530, sp + 0x530, sp + 0x630);
        crate::blib::exe::mul_mem(fx, sp + 0x4F0, sp + 0x530);
        for e in 0..4u32 {
            let s5 = D + 0x220 * e;
            for s3 in 0..0x10u32 {
                let p = s5 + 0x20 * s3;
                let q = s5 + 0x20 * (s3 + 1);
                let vs = [p + 0x2360, q + 0x2360, p + 0x2370, q + 0x2370];
                if fx.project_n(&vs, sp + 0x4F0, g, sp + 0x8EC, 0x01E0_2A14) == 1 {
                    let w = fx.mem.f(sp + 0x8EC);
                    let key = fix(fx, w, 0x01E0_2A28);
                    fx.submit(g, OT, key as i32);
                }
            }
        }
    }
    let f20 = ee::mul(70.0, f20);
    // One pass (the record's loop runs once): phase 0, ST shift 0.
    let pass = 0i32;
    fx.camera_vp_to(sp + 0x4F0);
    fx.identity(sp + 0x530);
    let y = ee::sub(15.0, ee::mul(2.0, ee::mul(7.0, ee::from_i32(pass))));
    vec4(fx, sp + 0x630, [0, y.to_bits(), 0, 0x3F80_0000]);
    translate(fx, hi, sp + 0x530, sp + 0x530, sp + 0x630);
    crate::blib::exe::mul_mem(fx, sp + 0x4F0, sp + 0x530);
    let a = fix(fx, f20, 0x01E0_2B14) & 0xFF;
    rgba(fx, sp + 0x7E0, [0x8C, 0x8C, 0xB4], a as u8);
    for r in 0..0x15u32 {
        for c in 0..0x15u32 {
            let ph = fx.mem.i32(D + 0x54 * r + 4 * c + 0x1C70);
            let age = fx.mem.i32(D + 0x2358);
            let a = ph.wrapping_add(age.wrapping_mul(0x457)) & 0xFFFF;
            let s = fx.tsin(a);
            fx.mem.setf(D + 0x150 * r + 0x10 * c + 0xE4, ee::mul(7.0, s));
        }
    }
    let v1 = fx.mem.i32(D + 0x2358);
    let a0 = (rem2(v1, 0x80) << 16) >> 7;
    let sign = rem2(pass, 2) * 2 - 1;
    let s3 = sign.wrapping_mul(a0) & 0xFFFF;
    let f21 = ee::mul(f(0x3E4C_CCCD), fx.tsin(s3));
    let f22 = ee::mul(f(0x3E4C_CCCD), fx.tcos(s3));
    for r in 0..0x14i32 {
        let mr = rem2(r, 2);
        let f23 = ee::add(ee::div(ee::from_i32(mr), 2.0), f22);
        let f24 = ee::add(ee::div(ee::from_i32(mr + 1), 2.0), f22);
        let row = D + 0x150 * r as u32;
        let next = row + 0x150;
        for c in 0..0x14i32 {
            let mc = rem2(c, 2);
            let f0 = ee::div(ee::from_i32(mc + 1), 2.0);
            let f1 = ee::add(ee::div(ee::from_i32(mc), 2.0), f21);
            let f0 = ee::add(f0, f21);
            fx.mem.setf(sp + 0x7E8, f1);
            fx.mem.setf(sp + 0x7EC, f23);
            fx.mem.setf(sp + 0x7F8, f0);
            fx.mem.setf(sp + 0x7FC, f23);
            fx.mem.setf(sp + 0x808, f1);
            fx.mem.setf(sp + 0x80C, f24);
            fx.mem.setf(sp + 0x818, f0);
            fx.mem.setf(sp + 0x81C, f24);
            let (o, o1) = (0x10 * c as u32 + 0xE0, 0x10 * (c + 1) as u32 + 0xE0);
            let vs = [row + o, row + o1, next + o, next + o1];
            if fx.project_n(&vs, sp + 0x4F0, b, sp + 0x8EC, 0x01E0_2DA4) != 1 {
                continue;
            }
            if fx.mem.i32(D + 0x2358) < 0x38 {
                for (to, from) in [(0x860u32, 0x7F0u32), (0x868, 0x800), (0x870, 0x810), (0x878, 0x820)] {
                    let v = fx.mem.read_u64(sp + from);
                    fx.mem.write_u64(sp + to, v);
                }
                fx.submit(l, OT, 0);
            }
            fx.submit(b, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x2358) + 1;
    fx.mem.set32(D + 0x2358, age);
    if age >= 0x10FF {
        fx.mem.write_u8(D + 0x2354, 0);
    }
}

/// The mote burst (D+0x8E50: cells +8, the attacker's point +0x210, 512
/// vectors of length 3.5 +0x220): on 0xA0, drawn 0x41 updates (or until
/// 0xB8) as (FF, FA, A4) sprites (texture D+0x1C) moving out by 0.8 + 0.2
/// age / 64 and rising 5 age / 64, 3.5 (0.2 + 0.8 (64 - age) / 64) wide,
/// alpha 128 ramp(age, 64, 42) [F 0x1E02EB4..0x1E03488].
fn motes(fx: &mut Fx, sp: u32) {
    let r = D + MOTE;
    if fx.mem.i32(D) == 0xA0 {
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for w in 0..4u32 {
            let x = fx.mem.read_u32(D + 0x30 + 4 * w);
            fx.mem.write_u32(r + 0x210 + 4 * w, x);
        }
        for s3 in 0..0x200u32 {
            let v = fx.rand_from(OT, 0x01E0_2F30) >> 23;
            fx.mem.write_u8(r + 8 + s3, rem2(v, 4) as u8);
            fx.random_table_vec(r + 0x220 + 0x10 * s3, 3.5);
        }
    }
    let d = sp + 0x670;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    fx.desc_clamp(d, 5);
    camera(fx, sp + UPA);
    if fx.mem.read_u8(r) == 0 {
        return;
    }
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    alpha(fx, d, 1);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let ramp = Fx::ramp_above(age, 64.0, 42.0);
    let a = fix(fx, ee::mul(1.0, ee::mul(128.0, ramp)), 0x01E0_30A8);
    let age = fx.mem.i32(r + 4);
    let x = ee::div(ee::mul(f(0x3F4C_CCCD), ee::from_i32(0x40 - age)), 64.0);
    let s = ee::mul(3.5, ee::add(f(0x3E4C_CCCD), x));
    let ns = ee::neg(s);
    for (o, v) in [(0x5B0u32, ns), (0x5C0, s)] {
        fx.mem.setf(sp + o, v);
        fx.mem.setf(sp + o + 4, v);
        fx.mem.write_u32(sp + o + 8, 0);
        fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
    }
    fx.vu_apply_to(sp + 0x5B0, sp + 0x570, sp + 0x5B0);
    fx.vu_apply_to(sp + 0x5C0, sp + 0x570, sp + 0x5C0);
    let age = ee::from_i32(fx.mem.i32(r + 4));
    let f2 = ee::div(ee::mul(f(0x3E4C_CCCD), age), 64.0);
    let f21 = ee::div(ee::mul(5.0, age), 64.0);
    let f20 = ee::add(f(0x3F4C_CCCD), f2);
    rgba(fx, sp + 0x680, [0xFF, 0xFA, 0xA4], a as u8);
    for s5 in 0..0x200u32 {
        let cell = i32::from(fx.mem.read_u8(r + 8 + s5));
        let cu = cell & 3;
        let cv = cell >> 2;
        uv(fx, sp + 0x688, ((cu << 9) & 0x3FFF) as u32);
        uv(fx, sp + 0x68A, ((cv << 9) & 0x3FFF) as u32);
        uv(fx, sp + 0x698, ((((cu << 5) + 0x20) << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x69A, ((((cv << 5) + 0x20) << 4) & 0x3FFF) as u32);
        fx.vu_scale4(sp + 0x630, r + 0x220 + 0x10 * s5, f20);
        fx.vu_add4(sp + 0x630, sp + 0x630, r + 0x210);
        let y = fx.mem.f(sp + 0x634);
        fx.mem.setf(sp + 0x634, ee::add(y, f21));
        fx.vu_add4(sp + 0x5F0, sp + 0x5B0, sp + 0x630);
        fx.vu_add4(sp + 0x600, sp + 0x5C0, sp + 0x630);
        if crate::fxlib2::project(fx, sp + 0x5F0, PROJ_OUT, sp + 0x4F0, d, sp + 0x8EC) {
            far(fx, sp + 0x690, &[sp + 0x6A0]);
            fx.submit(d, OT, 0);
        }
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x41 {
        fx.mem.write_u8(r, 0);
    }
    if fx.mem.i32(D) == 0xB8 {
        fx.mem.write_u8(r, 0);
    }
}

/// The attacker drawn into its target with callback A (b424's sphere,
/// (80, 80, FF)) and pulled through five widening fans (D+0x2BE0, from
/// 0x6E, ended by the fade), the layer keyed by its w from 0x142 [F
/// 0x1E0348C..0x1E03AFC]. `hi`: the upper half of the record's t0.
fn attacker_pass(fx: &mut Fx, sp: u32, hi: u64) {
    if fx.mem.i32(D) == 0x6E {
        fx.mem.write_u8(D + 0x2BE0, 1);
        fx.mem.write_u32(D + 0x2BE4, 0);
    }
    if fx.mem.read_u8(D + 0x2BE0) != 1 {
        return;
    }
    camera(fx, sp + UPA);
    vec4(fx, sp + 0x630, [0, 0, 0, 0x3F80_0000]);
    let y = fx.mem.read_u32(D + 0x34);
    fx.mem.write_u32(sp + 0x634, y);
    let m = fx.read_m4(sp + 0x4F0);
    fx.project_at(sp + 0x630, &m, sp + 0x8E0, sp + 0x8EC);
    let a = fan_desc(fx, sp + UPB, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x4EC, layer);
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let ras = [0x01E0_4A4C, 0x01E0_4A80, 0x01E0_4AB4, 0x01E0_4C54];
    let (_, regs) = fx.unit_draw_post(1, sp + 0x7B8, sp + 0x7C0, CB_A, &mut |fx, p| sphere(fx, p, hi, CB_A, D_STATIC, 0x2B00, [0x80, 0x80, 0xFF], &ras));
    fx.texflush(regs);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(D + 0x2BE4));
        let a = Fx::ratio_below(age, 48.0);
        let age = ee::from_i32(fx.mem.i32(D + 0x2BE4));
        ee::mul(a, Fx::ramp_above(age, f(0x4587_F000), f(0x4586_7000)))
    };
    let f20 = ramp(fx);
    let f21 = ramp(fx);
    for j in 0..5i32 {
        let s3 = div6(fx, (j + 1) << 15);
        let f22 = ee::sub(ee::add(1.0, ee::div(ee::mul(f(0x3E4C_CCCD), ee::from_i32(j)), 4.0)), f(0x3DCC_CCCD));
        let sn = fx.tsin(s3);
        let s4 = fix(fx, ee::mul(f20, ee::mul(64.0, sn)), 0x01E0_36F8) & 0xFF;
        let sn = fx.tsin(s3);
        let v0 = fix(fx, ee::mul(f21, ee::mul(64.0, sn)), 0x01E0_371C);
        rgba(fx, sp + 0x768, [0x80; 3], s4 as u8);
        rgba(fx, sp + 0x780, [0x80; 3], v0 as u8);
        copy8(fx, sp + 0x798, sp + 0x780);
        fan(fx, sp + 0x758, D + 0x40, sp + 0x5F0, f22, 0, &FAN_A);
    }
    let key = if fx.mem.i32(D) < 0x142 {
        0
    } else {
        let w = fx.mem.f(sp + 0x8EC);
        fix(fx, w, 0x01E0_3AD0) as i32
    };
    let layer = fx.mem.read_u32(sp + 0x4EC);
    fx.layer_end(layer, OT, key);
    let age = fx.mem.i32(D + 0x2BE4) + 1;
    fx.mem.set32(D + 0x2BE4, age);
    if age >= 0x10FF {
        fx.mem.write_u8(D + 0x2BE0, 0);
    }
}

/// On even updates of the window the target drawn into its target with
/// callback B (the veil), composited by a full-window sprite [F
/// 0x1E03B00..0x1E03CF0].
fn target_pass(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if !(win(n) && rem2(n, 2) == 0) {
        return;
    }
    let d = sp + 0x670;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let t = fx.mem.i32(D + 8);
    let (_, regs) = fx.unit_target_post(t, 1, sp + 0x6B0, sp + 0x6B8, CB_B, &mut |fx, p| veil(fx, p));
    fx.texflush(regs);
    rgba(fx, sp + 0x680, [0x80; 3], 0x80);
    uv(fx, sp + 0x688, 0);
    uv(fx, sp + 0x68A, 0);
    uv(fx, sp + 0x698, 0x2000);
    uv(fx, sp + 0x69A, 0x2000);
    far_xy(fx, sp + 0x690, 0x7000, 0x7000);
    far_xy(fx, sp + 0x6A0, 0x9000, 0x9000);
    fx.submit(d, 0, 0);
    fx.layer_end(layer, OT, 0);
}

/// The puffs (D+0x89D0, 2 x 0x240): on 0x3C / 0x57 at node 6 / 9 (D+0xC0 /
/// 0xD0): 32 cells (+8, (rand >> 23) % 8) rising along random vectors
/// (+0x40, length 1.8, y = 0.8 |y|), drawn 32 updates as (EE, EE, EE)
/// sprites (texture D+0x20) 17 (0.5 + age / 64) wide, alpha 64 cos [F
/// 0x1E03CF4..0x1E0425C].
fn puffs(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n == 0x3C || n == 0x57 {
        let i = u32::from(n != 0x3C);
        let r = D + PUFF + 0x240 * i;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for w in 0..4u32 {
            let v = fx.mem.read_u32(D + 0xC0 + 0x10 * i + 4 * w);
            fx.mem.write_u32(r + 0x30 + 4 * w, v);
        }
        for s in 0..0x20u32 {
            let v = fx.rand_from(OT, 0x01E0_3DC0) >> 23;
            fx.mem.write_u8(r + 8 + s, rem2(v, 8) as u8);
            let p = r + 0x40 + 0x10 * s;
            fx.random_table_vec(p, f(0x3FE6_6666));
            // fabsf's frame: the argument at its sp.
            let y = fx.mem.f(p + 4);
            fx.mem.setf(sp - 0x10, y);
            fx.mem.write_u32(p + 4, y.to_bits() & 0x7FFF_FFFF);
            let y = fx.mem.f(p + 4);
            fx.mem.setf(p + 4, ee::mul(y, f(0x3F4C_CCCD)));
        }
    }
    let d = sp + 0x670;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp + UPA);
    for k in 0..2u32 {
        let rec = D + PUFF + 0x240 * k;
        if fx.mem.read_u8(rec) == 0 {
            continue;
        }
        let age = fx.mem.i32(rec + 4);
        let c = fx.tcos((age << 14) >> 5);
        let a = fix(fx, ee::mul(64.0, c), 0x01E0_3F6C);
        rgba(fx, sp + 0x680, [0xEE; 3], a as u8);
        let age = ee::from_i32(fx.mem.i32(rec + 4));
        let s = ee::mul(17.0, ee::add(0.5, ee::div(ee::mul(0.5, age), 32.0)));
        let ns = ee::neg(s);
        for (o, x) in [(0x5B0u32, ns), (0x5C0, s)] {
            fx.mem.setf(sp + o, x);
            fx.mem.setf(sp + o + 4, x);
            fx.mem.write_u32(sp + o + 8, 0);
            fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
        }
        fx.vu_apply_to(sp + 0x5B0, sp + 0x570, sp + 0x5B0);
        fx.vu_apply_to(sp + 0x5C0, sp + 0x570, sp + 0x5C0);
        let age = fx.mem.i32(rec + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for s5 in 0..0x20u32 {
            fx.vu_scale4(sp + 0x630, rec + 0x40 + 0x10 * s5, f20);
            fx.vu_add4(sp + 0x630, sp + 0x630, rec + 0x30);
            let y = fx.mem.f(sp + 0x634);
            fx.mem.setf(sp + 0x634, ee::add(y, 0.0));
            fx.vu_add4(sp + 0x5F0, sp + 0x5B0, sp + 0x630);
            fx.vu_add4(sp + 0x600, sp + 0x5C0, sp + 0x630);
            let cell = i32::from(fx.mem.read_u8(rec + 8 + s5));
            let cu = cell & 3;
            let cv = cell >> 2;
            uv(fx, sp + 0x688, ((cu << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x68A, ((cv << 9) & 0x3FFF) as u32);
            uv(fx, sp + 0x698, ((((cu << 5) + 0x20) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x69A, ((((cv << 5) + 0x20) << 4) & 0x3FFF) as u32);
            if crate::fxlib2::project(fx, sp + 0x5F0, PROJ_OUT, sp + 0x4F0, d, sp + 0x8EC) {
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(rec + 4) + 1;
        fx.mem.set32(rec + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(rec, 0);
        }
    }
}

/// The attacker's (40, 40, FF) flash (D+0xB070, 33 updates from 6) as
/// three widening fans [F 0x1E04260..0x1E04770].
fn white_pass(fx: &mut Fx, sp: u32) {
    let e = D + FLASH;
    if fx.mem.i32(D) == 6 {
        fx.mem.write_u8(e, 1);
        fx.mem.write_u32(e + 4, 0);
    }
    if fx.mem.read_u8(e) != 1 {
        return;
    }
    let a = fan_desc(fx, sp + UPB, 1);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x4E8, layer);
    fx.texflush(a);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x7B8, sp + 0x7C0);
    fx.texflush(regs);
    let age = fx.mem.i32(e + 4);
    let c = [0x40, 0x40, 0xFF];
    rgba(fx, sp + 0x768, c, 0);
    rgba(fx, sp + 0x780, c, (((0x20 - age) << 7) >> 5) as u8);
    copy8(fx, sp + 0x798, sp + 0x780);
    let age = fx.mem.i32(e + 4);
    let f20 = ee::mul(f(0x3E99_999A), fx.tan_falloff_p(age, 0x20, 1));
    for j in 0..3i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f20, ee::from_i32(j + 1)), 3.0));
        fan(fx, sp + 0x758, D + 0x40, sp + 0x5F0, s, 0, &FAN_B);
    }
    let layer = fx.mem.read_u32(sp + 0x4E8);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(e + 4) + 1;
    fx.mem.set32(e + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(e, 0);
    }
}
