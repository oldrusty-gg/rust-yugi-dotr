//! BMODULE b066 (card 403), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x6A0 frame with D kept at sp +
//! 0xE0 and the OT in s0, data area a0 = 0x1ED0000, 0xD9D0 bytes; two
//! unit-target post callbacks 0x1E03690 (0x220 frame) / 0x1E03BB0 (0x50)
//! with their static D at 0x1E03D00) [F]: b424's
//! family (card 535) at other stack offsets, without its tint / glow /
//! spirals, with b424's bolts drawn as doubled strips, and two new parts:
//! spark records at six attacker nodes dropping trail records. The init
//! takes textures (5, 0..5) (D+0x10..0x24), the target's position (D+0x50),
//! nodes 0x2B (kind 1) / 0x2D / 0x2E / 0x32 / 0x34 / 0x35 (D+0x80..0x94)
//! and lays out the sphere of 17 x 17 points (D+0x110, 0x110 a ring,
//! radius 50). Every update the camera turn with roll (sp + 0x430), the
//! attacker's / target's points (D+0x30 / D+0x60), the six nodes
//! (D+0xA0..0xF0) and the screen points of the attacker's point (D+0x40)
//! and the target's position (D+0x70). Then: on 0x130 b424's white fade
//! (D+0x1328, 0x73 updates; at age 0x1E it ends the frame fan and the
//! attacker pass) and frame fan (D+0x1320, 0x1F updates); on even updates
//! 0xD2..0x14D a two-update zoom of the frame drawn as four widening fans
//! (D+0x1330); every update 0xD2..0x14D one of 4 bolt records (D+0x1340,
//! 0x3A0: 32 strips (80, 80, FF) and sprites at the target's point, life
//! 4 (0.3 + 0.7 r), 5 long, vectors 2.5); every update 0x92..0x10D one of
//! 64 spark records (D+0x21D0, 0x140: six sparks from the nodes towards
//! the target's point, 64 updates, every 17th update starting one of two
//! trail records D+0x71D0 + 0x1A0 k (0xD0: six (C8, 50, E4) sprites
//! rising 30, 32 updates)); on 0x2D the attacker's (40, FF, 40) flash
//! (D+0x21C0, 0x31 updates); from 0x19 the attacker pass (callback A: a
//! (20, 20, 20) veil and the sphere in (80, FF, FF); D+0x100, ratio 100,
//! ramp 4350 / 4250, through the fan); on even updates of the window the
//! target pass (callback B: the veil), on odd ones a (D0, AA, FF, 2D)
//! window tint. The stage light 0x80 - 96 clamp(n, 0, 0x40) clamp(0x1C0 -
//! n, 0, 0x40) / 4096. Returns 1 on 0xC7, 6 on 0xD2..0x14D, 3 on 0x14E, 8
//! once the count reaches 0x1C1.
use crate::b182::{alpha, clamp_regs, head, r256, rgba, uv};
use crate::b424::{fan, fan_desc};
use crate::b440::{copy8, fix, fixs, frame_mask_packet, rem2};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const D_STATIC: u32 = 0x01E0_3D00;
const CB_A: u32 = 0x01E0_3690;
const CB_B: u32 = 0x01E0_3BB0;
/// b424's fan descriptor (sp + 0x4C0 there) sits at sp + 0x5A0.
const FAN_UP: u32 = 0xE0;
/// The fans' temporaries.
const TMP: u32 = 0x4B0;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

const FAN_FRAME: [u32; 12] = [
    0x01E0_09A4, 0x01E0_09B4, 0x01E0_09F8, 0x01E0_0A04, 0x01E0_0A48, 0x01E0_0A54, 0x01E0_0B1C, 0x01E0_0B2C, 0x01E0_0B98, 0x01E0_0BA4, 0x01E0_0C10,
    0x01E0_0C1C,
];
const FAN_ZOOM: [u32; 12] = [
    0x01E0_0ECC, 0x01E0_0EDC, 0x01E0_0F20, 0x01E0_0F2C, 0x01E0_0F70, 0x01E0_0F7C, 0x01E0_1044, 0x01E0_1054, 0x01E0_10C0, 0x01E0_10CC, 0x01E0_1138,
    0x01E0_1144,
];
const FAN_WHITE: [u32; 12] = [
    0x01E0_28EC, 0x01E0_28FC, 0x01E0_294C, 0x01E0_2958, 0x01E0_29A8, 0x01E0_29B4, 0x01E0_2A80, 0x01E0_2A90, 0x01E0_2B00, 0x01E0_2B0C, 0x01E0_2B7C,
    0x01E0_2B88,
];
const FAN_ATTACKER: [u32; 12] = [
    0x01E0_2EDC, 0x01E0_2EEC, 0x01E0_2F3C, 0x01E0_2F48, 0x01E0_2F98, 0x01E0_2FA4, 0x01E0_3078, 0x01E0_3088, 0x01E0_30F8, 0x01E0_3104, 0x01E0_3174,
    0x01E0_3180,
];

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x6A0);
    fx.mem.write_u32(sp + 0xE0, D);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x6A0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xD9D0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for k in 0..6u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    for (k, node) in [0x2Bi32, 0x2D, 0x2E, 0x32, 0x34, 0x35].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let s = fx.request_node(h, node, i32::from(k == 0));
        fx.mem.set32(D + 0x80 + 4 * k as u32, s);
    }
    fx.mem.write_u8(D + 0x100, 0);
    for i in 0..0x11i32 {
        let c = fx.tcos((i * 0x8000) >> 4);
        let f21 = ee::mul(50.0, c);
        let a = Fx::powf2(50.0);
        let b = Fx::powf2(f21);
        let f20 = fx.sqrtf(ee::sub(a, b));
        let row = D + 0x110 * i as u32;
        for j in 0..0x11i32 {
            let a = (j << 16) >> 4;
            let p = row + 0x10 * j as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(p + 0x110, ee::mul(f20, sn));
            fx.mem.setf(p + 0x114, f21);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 0x118, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0x11C, 0x3F80_0000);
        }
    }
    for o in [0x1320u32, 0x1328, 0x1330] {
        fx.mem.write_u8(D + o, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x1340 + 0x3A0 * k, 0);
    }
    fx.mem.write_u8(D + 0x21C0, 0);
    for k in 0..0x40u32 {
        fx.mem.write_u8(D + 0x21D0 + 0x140 * k, 0);
        fx.mem.write_u32(D + 0x2300 + 0x140 * k, 0);
    }
    for k in 0..0x40u32 {
        for j in 0..2u32 {
            fx.mem.write_u8(D + 0x71D0 + 0x1A0 * k + 0xD0 * j, 0);
        }
    }
    0
}

fn win(n: i32) -> bool {
    (0xD2..0x14E).contains(&n)
}

fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x3B0);
    fx.identity(sp + 0x3F0);
    crate::blib::exe::mul_mem(fx, sp + 0x3B0, sp + 0x3F0);
}

/// The screen point of `p` (0x204D80 through sp + 0x3B0 into sp + 0x690)
/// into `o`: x / y ((x16 >> 4) - 0x700), Z (0x17BC10), 1.0, x / y clamped
/// to 0..512.
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + 0x3B0);
    fx.project_at(p, &m, sp + 0x690, sp + 0x69C);
    let x = (i32::from(fx.mem.read_u16(sp + 0x690)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x692)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x690) << 8) >> 40;
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

/// A full-window sprite (descriptor sp + 0x518) of colour `c` / alpha `a`
/// at ALPHA preset `preset`, into the ordering table at `key`.
fn window(fx: &mut Fx, sp: u32, preset: i32, c: [u8; 3], key: i32, a: impl FnOnce(&mut Fx) -> u32) {
    let t = sp + 0x518;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, preset);
    let a = a(fx);
    rgba(fx, sp + 0x528, c, a as u8);
    far_xy(fx, sp + 0x530, 0x7000, 0x7000);
    far_xy(fx, sp + 0x538, 0x9000, 0x9000);
    fx.submit(t, OT, key);
}

/// The fan's colours: sp + 0x5B0 and its two copies.
fn fan_colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    rgba(fx, sp + 0x5B0, c, a);
    copy8(fx, sp + 0x5C8, sp + 0x5B0);
    copy8(fx, sp + 0x5E0, sp + 0x5B0);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x430);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x60);
    for k in 0..6u32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x80 + 4 * k);
        fx.node_position(h, slot, D + 0xA0 + 0x10 * k);
    }
    camera(fx, sp);
    screen_point(fx, sp, D + 0x30, D + 0x40);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x50, D + 0x70);

    // The upper half of t0 at the 0x1F8280 call: the camera turn's row 1
    // z / w after a 0x188420 (`lq t0, 0x10(a1)`), cleared by 0x1F8270's
    // `move128 t0, zero`.
    let mut hi = 0u64;
    fade(fx, sp);
    frame_fan(fx, sp);
    zoom(fx, sp);
    bolts(fx, sp, &mut hi);
    sparks(fx, sp, &mut hi);
    trails(fx, sp);
    white_pass(fx, sp, &mut hi);
    attacker_pass(fx, sp, hi);
    target_pass(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x1C0 - n, 0, 0x40);
    let k = Fx::clamp_i(n, 0, 0x40);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 12) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xB0);
    if n == 0xC7 {
        ret = 1;
    } else {
        if win(n) {
            ret = 6;
        }
        if n == 0x14E {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1C1 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xB0, ret);
    ret
}

/// b424's white fade (D+0x1328, 0x73 updates from 0x130); at age 0x1E it
/// ends the frame fan and the attacker pass [F 0x1E00664..0x1E00810].
fn fade(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x130 {
        fx.mem.write_u8(D + 0x1328, 1);
        fx.mem.write_u32(D + 0x132C, 0);
    }
    if fx.mem.read_u8(D + 0x1328) != 1 {
        return;
    }
    if fx.mem.i32(D + 0x132C) == 0x1E {
        fx.mem.write_u8(D + 0x1320, 0);
        fx.mem.write_u8(D + 0x100, 0);
    }
    window(fx, sp, 1, [0xFF; 3], 0, |fx| {
        let age = fx.mem.i32(D + 0x132C);
        let c = Fx::clamp_i(age - 0x32, 0, 0x40);
        let f20 = ee::sub(1.0, fx.tan_falloff_p(c, 0x40, 1));
        let age = ee::from_i32(fx.mem.i32(D + 0x132C));
        let q = Fx::ratio_below(age, 30.0);
        fix(fx, ee::mul(128.0, ee::mul(q, f20)), 0x01E0_0748)
    });
    let age = fx.mem.i32(D + 0x132C) + 1;
    fx.mem.set32(D + 0x132C, age);
    if age >= 0x73 {
        fx.mem.write_u8(D + 0x1328, 0);
    }
}

/// b424's frame-textured fan at the target's screen point (D+0x1320, 0x1F
/// updates from 0x130) [F 0x1E00814..0x1E00CD4].
fn frame_fan(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x130 {
        fx.mem.write_u8(D + 0x1320, 1);
        fx.mem.write_u32(D + 0x1324, 0);
    }
    if fx.mem.read_u8(D + 0x1320) != 1 {
        return;
    }
    let d = sp + 0x5A0;
    fan_desc(fx, sp + FAN_UP, 0);
    fx.frame_desc(d, 1, [d + 0x10, d + 2]);
    let age = ee::from_i32(fx.mem.i32(D + 0x1324));
    let q = Fx::ratio_below(age, 20.0);
    let a = fix(fx, ee::mul(128.0, q), 0x01E0_08EC);
    fan_colour(fx, sp, [0x80; 3], a as u8);
    let s = ee::add(1.0, ee::mul(f(0x3DCC_CCCD), ee::from_i32(1)));
    fan(fx, d, D + 0x70, sp + TMP, s, OT, &FAN_FRAME);
    let age = fx.mem.i32(D + 0x1324) + 1;
    fx.mem.set32(D + 0x1324, age);
    if age >= 0x1F {
        fx.mem.write_u8(D + 0x1320, 0);
    }
}

/// The frame zoom (D+0x1330, two updates, on even updates of the window)
/// as four widening white fans of the frame copy [F 0x1E00CD8..0x1E01210].
fn zoom(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if win(n) && rem2(n, 2) == 0 {
        fx.mem.write_u8(D + 0x1330, 1);
        fx.mem.write_u32(D + 0x1334, 0);
    }
    if fx.mem.read_u8(D + 0x1330) != 1 {
        return;
    }
    let d = sp + 0x5A0;
    let a = fan_desc(fx, sp + FAN_UP, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xDC, layer);
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x600, sp + 0x608);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x1334);
    let al = fix(fx, ee::mul(32.0, ee::from_i32(1 - age)), 0x01E0_0E04);
    fan_colour(fx, sp, [0xFF; 3], al as u8);
    for k in 0..4i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f(0x3DCC_CCCD), ee::from_i32(k + 1)), 4.0));
        fan(fx, d, D + 0x70, sp + TMP, s, 0, &FAN_ZOOM);
    }
    let layer = fx.mem.read_u32(sp + 0xDC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x1334) + 1;
    fx.mem.set32(D + 0x1334, age);
    if age >= 2 {
        fx.mem.write_u8(D + 0x1330, 0);
    }
}

/// The strip descriptor sp + 0x618 (prim 0x24 | 0x800, 9 registers) with
/// texture D+`tex`, and the sprite descriptor sp + 0x548 (prim 0x26 |
/// 0x800) with texture D+`tex2`; ALPHA preset 1, clamp 5.
fn strip_descs(fx: &mut Fx, sp: u32, tex: Option<u32>, tex2: u32) {
    if let Some(tex) = tex {
        let b = sp + 0x618;
        head(fx, b, 0x24, 9, 0x4_2424_2421, 0x800);
        alpha(fx, b, 1);
        let rh = fx.mem.read_u32(D + tex);
        fx.desc_texture(b, rh, 1);
        fx.desc_clamp(b, 5);
    }
    let d = sp + 0x548;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + tex2);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// A quad's four vertices (z 0, w 1) at sp + 0x470.
fn quad(fx: &mut Fx, sp: u32, v: [(f32, f32); 4]) {
    for (k, (x, y)) in v.into_iter().enumerate() {
        let o = sp + 0x470 + 0x10 * k as u32;
        fx.mem.setf(o, x);
        fx.mem.setf(o + 4, y);
        fx.mem.write_u32(o + 8, 0);
        fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    }
}

/// The strip's cell UVs (sp + 0x630.. at +0x10 steps).
fn strip_uvs(fx: &mut Fx, sp: u32, u0: u32, u1: u32, v0: u32, v1: u32) {
    for (o, v) in [(0x630u32, u0), (0x632, v0), (0x640, u1), (0x642, v0), (0x650, u0), (0x652, v1), (0x660, u1), (0x662, v1)] {
        uv(fx, sp + o, v);
    }
}

/// The sprite's UVs (sp + 0x560 / 0x570).
fn sprite_uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    for (o, v) in [(0x560u32, u0), (0x562, v0), (0x570, u1), (0x572, v1)] {
        uv(fx, sp + o, v);
    }
}

/// A 4 x 2 / 4 x 2 cell `c` of 32: UVs (u0, u1, v0, v1).
fn cell32(c: i32) -> (u32, u32, u32, u32) {
    let cu = rem2(c, 4);
    let cv = c >> 2;
    (((cu << 9) & 0x3FFF) as u32, ((((cu << 5) + 0x20) << 4) & 0x3FFF) as u32, ((cv << 9) & 0x3FFF) as u32, ((((cv << 5) + 0x20) << 4) & 0x3FFF) as u32)
}

/// The camera's view-projection (sp + 0x3B0) times the camera turn moved
/// to sp + 0x4F0 (sp + 0x3F0, optionally turned about z by `ang`), then
/// the vertices at sp + 0x470 projected for descriptor `d` and submitted.
fn place_draw(fx: &mut Fx, sp: u32, d: u32, ang: Option<i32>, hi: &mut u64) {
    *hi = fx.mem.read_u64(sp + 0x448);
    fx.translate_rows(sp + 0x3F0, sp + 0x430, sp + 0x4F0);
    if let Some(a) = ang {
        fx.rotate_axis(sp + 0x3F0, a, 2);
    }
    crate::blib::exe::mul_mem(fx, sp + 0x3B0, sp + 0x3F0);
    if crate::fxlib2::project(fx, sp + 0x470, PROJ_OUT, sp + 0x3B0, d, sp + 0x69C) {
        fx.submit(d, OT, 0);
    }
}

/// The bolt records (D+0x1340, 4 x 0x3A0): spawned at n % 4 every update
/// of the window at the target's point, 32 doubled strips each with a
/// sprite [F 0x1E01214..0x1E01A60].
fn bolts(fx: &mut Fx, sp: u32, hi: &mut u64) {
    let n = fx.mem.i32(D);
    if win(n) {
        let r = D + 0x3A0 * rem2(n, 4) as u32;
        fx.mem.write_u8(r + 0x1340, 1);
        fx.mem.write_u32(r + 0x1344, 0);
        for s4 in 0..0x20u32 {
            let s1 = r + 4 * s4;
            let s2 = r + 0x10 * s4;
            let c = r256(fx, 0x01E0_1284);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(4.0, ee::add(f(0x3E99_999A), x)), 0x01E0_12E8);
            fx.mem.set32(s1 + 0x1348, life);
            let c = r256(fx, 0x01E0_12F0);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x1448, ee::mul(5.0, ee::add(f(0x3E99_999A), x)));
            let c = r256(fx, 0x01E0_1358);
            fx.mem.set32(s1 + 0x13C8, (c << 16) >> 8);
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0x60 + 4 * w);
                fx.mem.write_u32(r + 0x14D0 + 4 * w, x);
            }
            fx.random_table_vec(s2 + 0x14E0, 2.5);
        }
    }
    strip_descs(fx, sp, Some(0x10), 0x1C);
    for k in 0..4u32 {
        let s3 = D + 0x3A0 * k;
        if fx.mem.read_u8(s3 + 0x1340) == 0 {
            continue;
        }
        let pos = s3 + 0x14D0;
        for s5 in 0..0x20i32 {
            let s1 = s3 + 4 * s5 as u32;
            let s2 = s3 + 0x10 * s5 as u32;
            let age = fx.mem.i32(s3 + 0x1344);
            let life = fx.mem.i32(s1 + 0x1348);
            if life < age {
                continue;
            }
            let (q, _) = fx.div(age << 14, life);
            let f20 = ee::mul(128.0, fx.tcos(q));
            let a = fix(fx, f20, 0x01E0_1580);
            rgba(fx, sp + 0x628, [0x80, 0x80, 0xFF], a as u8);
            let a = fix(fx, f20, 0x01E0_15A8);
            rgba(fx, sp + 0x558, [0x80; 3], a as u8);
            let c4 = rem2(s5, 4);
            let a1 = rem2(c4, 2);
            let row = c4 >> 1;
            let u0 = ((a1 << 9) & 0x3FFF) as u32;
            let u1 = ((((a1 << 5) + 0x20) << 4) & 0x3FFF) as u32;
            let v0 = ((row << 10) & 0x3FFF) as u32;
            let v1 = ((((row << 6) + 0x40) << 4) & 0x3FFF) as u32;
            strip_uvs(fx, sp, u0, u1, v0, v1);
            let (u0, u1, v0, v1) = cell32(rem2(s5, 8));
            sprite_uvs(fx, sp, u0, v0, u1, v1);
            let age = fx.mem.i32(s3 + 0x1344);
            let life = fx.mem.i32(s1 + 0x1348);
            let (q, _) = fx.div(age << 14, life);
            let sn = fx.tsin(q);
            let f20 = fx.mem.f(s1 + 0x1448);
            let f5 = ee::mul(ee::mul(0.5, f20), sn);
            let f2 = ee::neg(f20);
            let f3 = ee::sub(f2, f5);
            let f4 = ee::add(f20, f5);
            let y0 = ee::mul(2.0, f3);
            let y1 = ee::mul(2.0, f4);
            quad(fx, sp, [(ee::add(f2, f5), y0), (f4, y0), (f3, y1), (ee::sub(f20, f5), y1)]);
            fx.camera_vp_to(sp + 0x3B0);
            fx.vu_add4(sp + 0x4F0, s2 + 0x14E0, pos);
            *hi = fx.mem.read_u64(sp + 0x448);
            fx.translate_rows(sp + 0x3F0, sp + 0x430, sp + 0x4F0);
            let ang = fx.mem.i32(s1 + 0x13C8);
            fx.rotate_axis(sp + 0x3F0, ang, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x3B0, sp + 0x3F0);
            let b = sp + 0x618;
            if crate::fxlib2::project(fx, sp + 0x470, PROJ_OUT, sp + 0x3B0, b, sp + 0x69C) {
                let rh = fx.mem.read_u32(D + 0x10);
                fx.desc_texture(b, rh, 1);
                fx.submit(b, OT, 0);
                let rh = fx.mem.read_u32(D + 0x18);
                fx.desc_texture(b, rh, 1);
                fx.submit(b, OT, 0);
            }
            let s = ee::mul(1.5, fx.mem.f(s1 + 0x1448));
            let ns = ee::neg(s);
            for (o, x) in [(0x470u32, ns), (0x480, s)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x3B0);
            let age = ee::from_i32(fx.mem.i32(s3 + 0x1344));
            let life = ee::from_i32(fx.mem.i32(s1 + 0x1348));
            let t = ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), life));
            fx.vu_scale4(sp + 0x4F0, s2 + 0x14E0, ee::mul(f(0xBF4C_CCCD), t));
            fx.vu_add4(sp + 0x4F0, sp + 0x4F0, pos);
            place_draw(fx, sp, sp + 0x548, None, hi);
        }
        let age = fx.mem.i32(s3 + 0x1344) + 1;
        fx.mem.set32(s3 + 0x1344, age);
        if age >= 5 {
            fx.mem.write_u8(s3 + 0x1340, 0);
        }
    }
}

/// The spark records (D+0x21D0, 64 x 0x140): spawned at n % 64 every
/// update 0x92..0x10D, six sparks from the nodes moving towards the
/// target's point for 64 updates, every 17th update starting one of the
/// record's two trail records [F 0x1E01A64..0x1E0239C].
fn sparks(fx: &mut Fx, sp: u32, hi: &mut u64) {
    let n = fx.mem.i32(D);
    if (0x92..0x10E).contains(&n) {
        let r = D + 0x140 * rem2(n, 0x40) as u32;
        fx.mem.write_u8(r + 0x21D0, 1);
        fx.mem.write_u32(r + 0x21D4, 0);
        for i in 0..6u32 {
            for w in 0..4u32 {
                let x = fx.mem.read_u32(D + 0xA0 + 0x10 * i + 4 * w);
                fx.mem.write_u32(r + 0x21E0 + 0x10 * i + 4 * w, x);
            }
            fx.vu_sub4(r + 0x22A0 + 0x10 * i, D + 0x60, r + 0x21E0 + 0x10 * i);
            let v = fx.rand_from(OT, 0x01E0_1B20) >> 23;
            fx.mem.set8(r + 0x21D8 + i, rem2(v, 8));
            fx.random_table_vec(r + 0x2240 + 0x10 * i, 2.0);
        }
    }
    strip_descs(fx, sp, Some(0x20), 0x1C);
    let (f21, f20) = (-2.0f32, -2.5f32);
    for k in 0..0x40u32 {
        let r = D + 0x140 * k;
        let tr = D + 0x1A0 * k;
        if fx.mem.read_u8(r + 0x21D0) == 0 {
            continue;
        }
        let mut idx = -1i32;
        let age = fx.mem.i32(r + 0x21D4);
        if fx.div(age, 0x11).1 == 0 {
            idx = fx.mem.i32(r + 0x2300);
            fx.mem.set32(r + 0x2300, idx + 1);
            if idx + 1 >= 2 {
                fx.mem.write_u32(r + 0x2300, 0);
            }
            let t = tr.wrapping_add((0xD0 * idx) as u32);
            fx.mem.write_u8(t + 0x71D0, 1);
            fx.mem.write_u32(t + 0x71D4, 0);
        }
        let age = ee::from_i32(fx.mem.i32(r + 0x21D4));
        let f22 = Fx::ratio_below(age, 4.0);
        let age = ee::from_i32(fx.mem.i32(r + 0x21D4));
        let f0 = Fx::ramp_above(age, 4.0, 60.0);
        let f3 = ee::mul(f22, f0);
        let m = rem2(fx.mem.i32(r + 0x21D4), 2);
        let f1 = ee::add(0.5, ee::mul(0.5, ee::from_i32(m)));
        let f22 = ee::mul(128.0, ee::mul(f1, f3));
        let a = fix(fx, f22, 0x01E0_1DE8);
        rgba(fx, sp + 0x628, [0x80; 3], a as u8);
        let a = fix(fx, f22, 0x01E0_1E0C);
        rgba(fx, sp + 0x558, [0x80; 3], a as u8);
        let f23 = ee::div(ee::from_i32(fx.mem.i32(r + 0x21D4)), 64.0);
        let f22 = ee::add(f(0x3E99_999A), ee::mul(f(0x3F33_3333), f23));
        let t = tr.wrapping_add((0xD0 * idx) as u32);
        for i in 0..6u32 {
            fx.vu_scale4(sp + 0x500, r + 0x22A0 + 0x10 * i, f23);
            fx.vu_scale4(sp + 0x4F0, r + 0x2240 + 0x10 * i, f22);
            fx.vu_add4(sp + 0x4F0, sp + 0x4F0, r + 0x21E0 + 0x10 * i);
            fx.vu_add4(sp + 0x4F0, sp + 0x4F0, sp + 0x500);
            let v = fx.rand_from(OT, 0x01E0_1F1C) >> 23;
            let (u0, u1, v0, v1) = cell32(rem2(v, 8));
            strip_uvs(fx, sp, u0, u1, v0, v1);
            let b = i32::from(fx.mem.read_u8(r + 0x21D8 + i));
            let (u0, u1, v0, v1) = cell32(b);
            sprite_uvs(fx, sp, u0, v0, u1, v1);
            quad(fx, sp, [(f21, f21), (2.0, f21), (f21, 2.0), (2.0, 2.0)]);
            fx.camera_vp_to(sp + 0x3B0);
            *hi = fx.mem.read_u64(sp + 0x448);
            fx.translate_rows(sp + 0x3F0, sp + 0x430, sp + 0x4F0);
            let c = r256(fx, 0x01E0_2158);
            fx.rotate_axis(sp + 0x3F0, (c << 16) >> 8, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x3B0, sp + 0x3F0);
            if crate::fxlib2::project(fx, sp + 0x470, PROJ_OUT, sp + 0x3B0, sp + 0x618, sp + 0x69C) {
                fx.submit(sp + 0x618, OT, 0);
            }
            for (o, x) in [(0x470u32, f20), (0x480, 2.5)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x3B0);
            place_draw(fx, sp, sp + 0x548, None, hi);
            if idx != -1 {
                let v = fx.rand_from(OT, 0x01E0_2280) >> 23;
                fx.mem.set8(t + 0x71D8 + i, rem2(v, 4));
                for w in 0..4u32 {
                    let x = fx.mem.read_u32(sp + 0x4F0 + 4 * w);
                    fx.mem.write_u32(t + 0x71E0 + 0x10 * i + 4 * w, x);
                }
                fx.random_table_vec(t + 0x7240 + 0x10 * i, 0.5);
            }
        }
        let age = fx.mem.i32(r + 0x21D4) + 1;
        fx.mem.set32(r + 0x21D4, age);
        if age >= 0x41 {
            fx.mem.write_u8(r + 0x21D0, 0);
        }
    }
}

/// The trail records (D+0x71D0 + 0x1A0 k + 0xD0 j): six (C8, 50, E4)
/// sprites rising 30 over 32 updates [F 0x1E0239C..0x1E0270C].
fn trails(fx: &mut Fx, sp: u32) {
    strip_descs(fx, sp, None, 0x24);
    camera(fx, sp);
    let f21 = -2.0f32;
    for k in 0..0x40u32 {
        for j in 0..2u32 {
            let t = D + 0x1A0 * k + 0xD0 * j;
            if fx.mem.read_u8(t + 0x71D0) == 0 {
                continue;
            }
            let age = fx.mem.i32(t + 0x71D4);
            let c = fx.tcos((age << 15) >> 5);
            let a = fix(fx, ee::mul(255.0, c), 0x01E0_2484);
            rgba(fx, sp + 0x558, [0xC8, 0x50, 0xE4], a as u8);
            let f20 = ee::div(ee::from_i32(fx.mem.i32(t + 0x71D4)), 32.0);
            let f22 = ee::mul(-30.0, f20);
            for (o, x) in [(0x470u32, f21), (0x480, 2.0)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.vu_apply_to(sp + 0x470, sp + 0x430, sp + 0x470);
            fx.vu_apply_to(sp + 0x480, sp + 0x430, sp + 0x480);
            for i in 0..6u32 {
                let b = i32::from(fx.mem.read_u8(t + 0x71D8 + i));
                let a1 = rem2(b, 2);
                let u0 = ((a1 << 10) & 0x3FFF) as u32;
                let u1 = ((((a1 << 6) + 0x40) << 4) & 0x3FFF) as u32;
                let r = b >> 1;
                let v0 = ((r << 10) & 0x3FFF) as u32;
                let v1 = ((((r << 6) + 0x40) << 4) & 0x3FFF) as u32;
                sprite_uvs(fx, sp, u0, v0, u1, v1);
                fx.vu_scale4(sp + 0x4F0, t + 0x7240 + 0x10 * i, f20);
                fx.vu_add4(sp + 0x4F0, sp + 0x4F0, t + 0x71E0 + 0x10 * i);
                let y = ee::add(fx.mem.f(sp + 0x4F4), f22);
                fx.mem.setf(sp + 0x4F4, y);
                fx.vu_add4(sp + 0x4B0, sp + 0x470, sp + 0x4F0);
                fx.vu_add4(sp + 0x4C0, sp + 0x480, sp + 0x4F0);
                if crate::fxlib2::project(fx, sp + 0x4B0, PROJ_OUT, sp + 0x3B0, sp + 0x548, sp + 0x69C) {
                    fx.submit(sp + 0x548, OT, 0);
                }
            }
            let age = fx.mem.i32(t + 0x71D4) + 1;
            fx.mem.set32(t + 0x71D4, age);
            if age >= 0x21 {
                fx.mem.write_u8(t + 0x71D0, 0);
            }
        }
    }
}

/// x / 48 as the record divides (0x2AAAAAAB, HI >> 3 + the sign).
fn div48(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    (fx.hilo().1 >> 3).wrapping_add(((x as u32) >> 31) as i32)
}

/// The attacker's (40, FF, 40) flash (D+0x21C0, 0x31 updates from 0x2D) as
/// three widening fans [F 0x1E02710..0x1E02C64].
fn white_pass(fx: &mut Fx, sp: u32, hi: &mut u64) {
    if fx.mem.i32(D) == 0x2D {
        fx.mem.write_u8(D + 0x21C0, 1);
        fx.mem.write_u32(D + 0x21C4, 0);
    }
    if fx.mem.read_u8(D + 0x21C0) != 1 {
        return;
    }
    let a = fan_desc(fx, sp + FAN_UP, 1);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x3AC, layer);
    fx.texflush(a);
    let h = fx.mem.i32(D + 4);
    *hi = 0;
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x600, sp + 0x608);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x21C4);
    let al = div48(fx, (0x30 - age) << 7);
    fan_colour(fx, sp, [0x40, 0xFF, 0x40], al as u8);
    let age = fx.mem.i32(D + 0x21C4);
    let f20 = ee::mul(0.5, fx.tan_falloff_p(age, 0x30, 1));
    for j in 0..3i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f20, ee::from_i32(j + 1)), 3.0));
        fan(fx, sp + 0x5A0, D + 0x40, sp + TMP, s, 0, &FAN_WHITE);
    }
    let layer = fx.mem.read_u32(sp + 0x3AC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x21C4) + 1;
    fx.mem.set32(D + 0x21C4, age);
    if age >= 0x31 {
        fx.mem.write_u8(D + 0x21C0, 0);
    }
}

/// The attacker drawn into its target with callback A and pulled through
/// the fan (D+0x100, from 0x19, ended by the fade) [F 0x1E02C68..0x1E03258].
fn attacker_pass(fx: &mut Fx, sp: u32, hi: u64) {
    if fx.mem.i32(D) == 0x19 {
        fx.mem.write_u8(D + 0x100, 1);
        fx.mem.write_u32(D + 0x104, 0);
    }
    if fx.mem.read_u8(D + 0x100) != 1 {
        return;
    }
    camera(fx, sp);
    for (k, w) in [0u32, 0, 0, 0x3F80_0000].into_iter().enumerate() {
        fx.mem.write_u32(sp + 0x4F0 + 4 * k as u32, w);
    }
    let y = fx.mem.read_u32(D + 0x34);
    fx.mem.write_u32(sp + 0x4F4, y);
    let m = fx.read_m4(sp + 0x3B0);
    fx.project_at(sp + 0x4F0, &m, sp + 0x690, sp + 0x69C);
    let a = fan_desc(fx, sp + FAN_UP, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x3A8, layer);
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x600, sp + 0x608, CB_A, &mut |fx, p| sphere(fx, p, hi));
    fx.texflush(regs);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(D + 0x104));
        let a = Fx::ratio_below(age, 100.0);
        let age = ee::from_i32(fx.mem.i32(D + 0x104));
        ee::mul(a, Fx::ramp_above(age, f(0x4587_F000), f(0x4584_D000)))
    };
    let f21 = ramp(fx);
    let f1 = ramp(fx);
    let f20 = ee::mul(128.0, f21);
    let f21 = ee::mul(128.0, f1);
    let s1 = fix(fx, f20, 0x01E0_2E6C) & 0xFF;
    let v0 = fix(fx, f21, 0x01E0_2EA0);
    rgba(fx, sp + 0x5B0, [0x80; 3], s1 as u8);
    rgba(fx, sp + 0x5C8, [0x80; 3], v0 as u8);
    copy8(fx, sp + 0x5E0, sp + 0x5C8);
    fan(fx, sp + 0x5A0, D + 0x40, sp + TMP, 1.0, 0, &FAN_ATTACKER);
    let layer = fx.mem.read_u32(sp + 0x3A8);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x104) + 1;
    fx.mem.set32(D + 0x104, age);
    if age >= 0x10FF {
        fx.mem.write_u8(D + 0x100, 0);
    }
}

/// On even updates of the window the target drawn into its target with
/// callback B (b424's veil), composited by a full-window sprite; on odd
/// ones a (D0, AA, FF, 2D) window tint [F 0x1E0325C..0x1E03568].
fn target_pass(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if win(n) && rem2(n, 2) == 0 {
        let d = sp + 0x548;
        head(fx, d, 0x26, 5, 0x4_2421, 0x800);
        alpha(fx, d, 0);
        fx.desc_clamp(d, LENS_CLAMP);
        let a = clamp_regs(fx, d, LENS_CLAMP);
        let layer = fx.layer_begin();
        fx.texflush(a);
        fx.mem.write_u32(D_STATIC, D);
        let t = fx.mem.i32(D + 8);
        let (_, regs) = fx.unit_target_post(t, 1, sp + 0x588, sp + 0x590, CB_B, &mut |fx, p| crate::b424::veil(fx, p));
        fx.texflush(regs);
        rgba(fx, sp + 0x558, [0x80; 3], 0x80);
        sprite_uvs(fx, sp, 0, 0, 0x2000, 0x2000);
        far_xy(fx, sp + 0x568, 0x7000, 0x7000);
        far_xy(fx, sp + 0x578, 0x9000, 0x9000);
        fx.submit(d, 0, 0);
        fx.layer_end(layer, OT, 0);
    }
    let n = fx.mem.i32(D);
    if win(n) && rem2(n, 2) == 1 {
        window(fx, sp, 0, [0xD0, 0xAA, 0xFF], 0xFF_FFFF, |_| 0x2D);
    }
}

/// Callback A 0x1E03690(p) [F]: b424's sphere callback with a (20, 20, 20)
/// veil, the sphere at D+0x110 and the quads in (80, FF, FF). The quad's
/// TEX0 / TEX1 slots (sp + 0x200 / 0x208) hold 0x209800's save of
/// 0x1F0FD0's s1 (the callback's address and `hi`) when the texture is
/// missing.
fn sphere(fx: &mut Fx, p: u32, hi: u64) -> u32 {
    let sp = fx.enter(0x220);
    fx.mem.write_u64(sp + 0x200, u64::from(CB_A));
    fx.mem.write_u64(sp + 0x208, hi);
    let d = fx.mem.read_u32(D_STATIC);
    fx.mem.write_u32(sp + 0xB0, d);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let t = sp + 0x158;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 2);
    rgba(fx, sp + 0x168, [0x20; 3], 0x80);
    far_xy(fx, sp + 0x170, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x178, 0x8FF8, 0x8FF8);
    let mut s6 = fx.submit_at(s0, t, 0, 0);
    let q = sp + 0x188;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let rh = fx.mem.read_u32(d + 0x14);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    fx.camera_vp_to(sp + 0xD0);
    fx.identity(sp + 0x110);
    fx.translate_rows(sp + 0x110, sp + 0x110, d + 0x30);
    for (axis, ra) in [0x01E0_384Cu32, 0x01E0_3880, 0x01E0_38B4].into_iter().enumerate() {
        let v = fx.rand_from(s0, ra) >> 23;
        fx.rotate_axis(sp + 0x110, (rem2(v, 0x100) << 16) >> 8, axis as i32);
    }
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    rgba(fx, sp + 0x198, [0x80, 0xFF, 0xFF], 0x80);
    for to in [0x1B0, 0x1C8, 0x1E0] {
        copy8(fx, sp + to, sp + 0x198);
    }
    let mut a23 = [0u32, 0];
    for i in 0..0x10i32 {
        let s3 = d + 0x20 + 0x110 * i as u32;
        let s5 = s3 + 0x110;
        let ri = rem2(i, 4);
        let f20 = ee::div(ee::from_i32(ri), 4.0);
        let f21 = ee::div(ee::from_i32(ri + 1), 4.0);
        for j in 0..0x10i32 {
            let rj = rem2(j, 4);
            let f2 = ee::div(ee::from_i32(rj), 4.0);
            let f0 = ee::div(ee::from_i32(rj + 1), 4.0);
            fx.mem.setf(sp + 0x1A0, f2);
            fx.mem.setf(sp + 0x1A4, f20);
            fx.mem.setf(sp + 0x1B8, f0);
            fx.mem.setf(sp + 0x1BC, f20);
            fx.mem.setf(sp + 0x1D0, f2);
            fx.mem.setf(sp + 0x1D4, f21);
            fx.mem.setf(sp + 0x1E8, f0);
            fx.mem.setf(sp + 0x1EC, f21);
            let (a, b) = (0x10 * j as u32 + 0xF0, 0x10 * (j + 1) as u32 + 0xF0);
            if fx.project_n(&[s3 + a, s3 + b, s5 + a, s5 + b], sp + 0xD0, q, sp + 0x21C, 0x01E0_3A54) != 1 {
                a23 = [sp + 0xD0, q];
                continue;
            }
            crate::b182::far(fx, sp + 0x1A8, &[sp + 0x1C0, sp + 0x1D8, sp + 0x1F0]);
            s6 = fx.submit_at(s6, q, 0, 0);
            a23 = [0, 0];
        }
    }
    let r = frame_mask_packet(fx, s6, 0, a23);
    fx.leave(0x220);
    r
}
