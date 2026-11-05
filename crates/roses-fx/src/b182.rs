//! BMODULE b182 (card 40), hand-ported from the record (linked at
//! 0x1E00000, the entry 0x1E00080 in a 0x5D0 frame, data area a0 =
//! 0x1ED0000, 0x41C0 bytes; two unit-target post callbacks 0x1E02CB0 (0x220
//! frame) / 0x1E03210 (0x50) with their static D at 0x1E03380) [F] (helper
//! D24, 3 Nov 2026): the smallest of the 0x1F82B0 family (cards 10 / 535
//! / 538 / 440 / 40 / 145 / 11; both callbacks post-only, t0 = 0). The init
//! lays out a sphere of 17 x 17 points of radius 30 (D+0x140, 0x110 a
//! ring) and 17 colour ramps (D+0x1358: 128 grey, alpha 128 (16 - i) / 16).
//! Every update the camera turn with roll (sp + 0x2E0), the attacker's /
//! target's points (0x1F65D0 kind 1, D+0x30 / D+0x60), the attacker's nodes
//! 0xE / 0x16 (D+0x90 / 0xA0) with matrices pointing from the target's
//! point to them (D+0xB0 / 0xF0) and the screen points of the attacker's
//! point (D+0x40) and the target's position (D+0x70). Then: on 0xAB a
//! white full-window flash (33 updates, 128 (32 - age) / 32); on even
//! updates 0xAC..0x13F a two-update zoom of the frame (four far-Z sprites
//! 256 (1 + 0.1 (k + 1) / 4) half wide, alpha 32 / 0); on 0xAB two glows
//! at the nodes (150 updates, (40, 40, FF) / (FF, 40, 40), flickering 0.7
//! / 1.0); every update 0xAC..0x13F one of 16 spiral records (D+0x13A0,
//! 0x1F0: two nodes x three ribbons of 16 textured quads swinging out of
//! the node, 11..16 updates) and one of 8 bolt records (D+0x32B0, 0x1E0:
//! 16 textured strips at the target's point turned by random angles with a
//! sprite each, 2..8 updates); from 0x40 for 257 updates the attacker
//! drawn into its target with callback A (FRAME masked: a grey veil, then
//! the sphere at the attacker's point under a random turn as 16 x 16
//! textured quads) and pulled through the eight-triangle fan at its screen
//! point (alpha 128 x ramp); on even updates 0xAC..0x13F the target drawn
//! into its target with callback B (a dark veil) and composited by a
//! full-window sprite; on 0x1C a 17-update white flash of the attacker
//! (0x1F8270) as three widening fans. The stage light 0x80 - 96 clamp(n)
//! clamp(0x160 - n) / 1024. Returns 1 on 0xA1, 6 on 0xAC..0x13F, 3 on
//! 0x140, 8 once the count reaches 0x161.
use crate::b440::{copy8, copy_z, fix, fixs, frame_mask_packet, rem2, vec4};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};
use crate::lens::{far_xy, FAR_Z, KEEP, LENS_CLAMP};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's / 0x205210's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The fan's nine screen points (exe rodata, 0x10 apart).
const FAN: u32 = 0x0028_ABE0;
/// The callbacks' D (static at 0x1E03380).
const D_STATIC: u32 = 0x01E0_3380;
const CB_A: u32 = 0x01E0_2CB0;
const CB_B: u32 = 0x01E0_3210;
/// The sprite CLAMP of the glows / bolt sprites (REGION_REPEAT).
const GLOW_CLAMP: u64 = 0x0000_01FC_6133_CB1A;
const BOLT_CLAMP: u64 = 0x0000_03FC_811F_C0BA;

const fn f(b: u32) -> f32 {
    f32::from_bits(b)
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x5D0);
    fx.mem.write_u32(sp + 0xD0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x5D0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x41C0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let side = fx.unit_side(h);
    fx.mem.set32(D + 0xC, side);
    for (k, id) in [(0x10u32, 0), (0x14, 1), (0x18, 2), (0x1C, 3)] {
        let r = fx.texture(5, id);
        fx.mem.write_u32(D + k, r);
    }
    let r = fx.texture(4, 0xF);
    fx.mem.write_u32(D + 0x20, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    let h = fx.mem.i32(D + 4);
    let s = fx.request_node(h, 0xE, 1);
    fx.mem.set32(D + 0x80, s);
    let h = fx.mem.i32(D + 4);
    let s = fx.request_node(h, 0x16, 0);
    fx.mem.set32(D + 0x84, s);
    fx.mem.write_u8(D + 0x130, 0);
    // The sphere: 17 rings (angle 0x800 i) of 17 points of radius 30.
    for i in 0..0x11i32 {
        let c = fx.tcos((i * 0x8000) >> 4);
        let f21 = ee::mul(30.0, c);
        let a = Fx::powf2(30.0);
        let b = Fx::powf2(f21);
        let f20 = fx.sqrtf(ee::sub(a, b));
        let row = D + 0x110 * i as u32;
        for j in 0..0x11i32 {
            let a = (j << 16) >> 4;
            let p = row + 0x10 * j as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(p + 0x140, ee::mul(f20, sn));
            fx.mem.setf(p + 0x144, f21);
            let cs = fx.tcos(a);
            fx.mem.setf(p + 0x148, ee::mul(f20, cs));
            fx.mem.write_u32(p + 0x14C, 0x3F80_0000);
        }
    }
    fx.mem.write_u8(D + 0x1350, 0);
    fx.mem.write_u32(D + 0x1354, 0);
    for i in 0..0x11i32 {
        let f3 = ee::div(ee::from_i32(i), 16.0);
        let f0 = ee::mul(128.0, f3);
        let f21 = ee::mul(128.0, ee::sub(1.0, f3));
        let f20 = ee::add(f21, f0);
        let c = D + 0x1358 + 4 * i as u32;
        for (k, ra) in [(0u32, 0x01E0_02A4), (1, 0x01E0_02B0), (2, 0x01E0_02BC)] {
            let v = fix(fx, f20, ra);
            fx.mem.write_u8(c + k, v as u8);
        }
        let v = fix(fx, f21, 0x01E0_02C8);
        fx.mem.write_u8(c + 3, v as u8);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(D + 0x13A0 + 0x1F0 * k, 0);
    }
    fx.mem.write_u8(D + 0x32A0, 0);
    fx.mem.write_u8(D + 0x32A8, 0);
    for k in 0..8u32 {
        fx.mem.write_u8(D + 0x32B0 + 0x1E0 * k, 0);
    }
    fx.mem.write_u8(D + 0x41B0, 0);
    0
}

pub(crate) fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

pub(crate) fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// A descriptor head: prim `prim` | `or`, `n` registers `regs`.
pub(crate) fn head(fx: &mut Fx, d: u32, prim: u16, n: u16, regs: u64, or: u16) {
    fx.mem.write_u16(d + 2, prim);
    fx.mem.write_u16(d, n);
    fx.mem.write_u64(d + 8, regs);
    fx.mem.write_u32(d + 4, 0);
    if or != 0 {
        let x = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, x | or);
    }
}

pub(crate) fn alpha(fx: &mut Fx, d: u32, preset: i32) {
    let a = fx.alpha_preset(preset);
    fx.desc_alpha(d, a);
}

/// a0..a3 as 0x208400(d, clamp) leaves them (for the TEXFLUSH after an
/// answered 0x2078A0).
pub(crate) fn clamp_regs(fx: &mut Fx, d: u32, clamp: u64) -> [u32; 4] {
    [u32::from(fx.mem.read_u16(d + 2)), clamp as u32, d + 0x10, d + 2]
}

/// The camera's view-projection at sp + 0x260 times the identity at sp +
/// 0x2A0 (`m` the matrix it is built from: identity when `None`).
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x260);
    fx.identity(sp + 0x2A0);
    crate::blib::exe::mul_mem(fx, sp + 0x260, sp + 0x2A0);
}

/// The screen point of `p` (0x204D80 through sp + 0x260 into sp + 0x5C0):
/// x / y ((x16 >> 4) - 0x700) into `o` / `o` + 4, Z (0x17BC10) into +8,
/// 1.0 into +0xC, x / y clamped to 0..512.
fn screen_point(fx: &mut Fx, sp: u32, p: u32, o: u32) {
    let m = fx.read_m4(sp + 0x260);
    fx.project_at(p, &m, sp + 0x5C0, sp + 0x5CC);
    let x = (i32::from(fx.mem.read_u16(sp + 0x5C0)) >> 4) - 0x700;
    fx.mem.setf(o, ee::from_i32(x));
    let y = (i32::from(fx.mem.read_u16(sp + 0x5C2)) >> 4) - 0x700;
    fx.mem.setf(o + 4, ee::from_i32(y));
    let z = (fx.mem.read_u64(sp + 0x5C0) << 8) >> 40;
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

/// The far Z into the XYZ2 at `first`, its Z into the others.
pub(crate) fn far(fx: &mut Fx, first: u32, others: &[u32]) {
    let x = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (x & KEEP) | FAR_Z);
    for &o in others {
        copy_z(fx, o, first);
    }
}

/// An XYZ2 at `p` of pixel (x, y) at far Z, top byte 0.
pub(crate) fn far_px(fx: &mut Fx, p: u32, x: i32, y: i32) {
    fx.mem.write_u16(p, ((x + 0x700) << 4) as u16);
    fx.mem.write_u16(p + 2, ((y + 0x700) << 4) as u16);
    let v = fx.mem.read_u64(p);
    fx.mem.write_u64(p, (v & KEEP) | FAR_Z);
    fx.mem.write_u8(p + 7, 0);
}

pub(crate) fn fix_at(fx: &mut Fx, a: u32, ra: u32) -> i32 {
    let v = fx.mem.f(a);
    fixs(fx, v, ra)
}

/// The eight-triangle fan (descriptor sp + 0x4D0) about the attacker's
/// screen point D+0x40: texels the fan points, positions scaled by `s`
/// about it, far Z, unlinked; `ras` the 0x1A2B80 return addresses.
fn fan(fx: &mut Fx, sp: u32, s: f32, ras: &[u32; 12]) {
    let c = D + 0x40;
    for k in 0..8u32 {
        let s1 = FAN + 0x10 * k;
        let x = fix_at(fx, c, ras[0]);
        let y = fix_at(fx, c + 4, ras[1]);
        uv(fx, sp + 0x4E8, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x4EA, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1, ras[2]);
        let y = fix_at(fx, s1 + 4, ras[3]);
        uv(fx, sp + 0x500, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x502, ((y << 4) & 0x3FFF) as u32);
        let x = fix_at(fx, s1 + 0x10, ras[4]);
        let y = fix_at(fx, s1 + 0x14, ras[5]);
        uv(fx, sp + 0x518, ((x << 4) & 0x3FFF) as u32);
        uv(fx, sp + 0x51A, ((y << 4) & 0x3FFF) as u32);
        fx.vu_sub4(sp + 0x360, s1, c);
        fx.vu_sub4(sp + 0x370, s1 + 0x10, c);
        fx.vu_scale4(sp + 0x360, sp + 0x360, s);
        fx.vu_scale4(sp + 0x370, sp + 0x370, s);
        fx.vu_add4(sp + 0x360, sp + 0x360, c);
        fx.vu_add4(sp + 0x370, sp + 0x370, c);
        let x = fix_at(fx, c, ras[6]);
        let y = fix_at(fx, c + 4, ras[7]);
        far_px(fx, sp + 0x4F0, x, y);
        let x = fix_at(fx, sp + 0x360, ras[8]);
        let y = fix_at(fx, sp + 0x364, ras[9]);
        far_px(fx, sp + 0x508, x, y);
        let x = fix_at(fx, sp + 0x370, ras[10]);
        let y = fix_at(fx, sp + 0x374, ras[11]);
        far_px(fx, sp + 0x520, x, y);
        fx.submit(sp + 0x4D0, 0, 0);
    }
}

/// The fan's triangle descriptor at sp + 0x4D0 (prim 0x33 | 0x800, 9
/// registers), ALPHA preset `a`, the lens CLAMP.
fn fan_desc(fx: &mut Fx, sp: u32, a: i32) -> [u32; 4] {
    let d = sp + 0x4D0;
    head(fx, d, 0x33, 9, 0x4_2142_1421, 0x800);
    alpha(fx, d, a);
    fx.desc_clamp(d, LENS_CLAMP);
    clamp_regs(fx, d, LENS_CLAMP)
}

const FAN_A: [u32; 12] = [
    0x01E0_2178, 0x01E0_2184, 0x01E0_21C8, 0x01E0_21D4, 0x01E0_2218, 0x01E0_2224, 0x01E0_22E0, 0x01E0_22EC, 0x01E0_2358, 0x01E0_2364, 0x01E0_23D0,
    0x01E0_23DC,
];
const FAN_B: [u32; 12] = [
    0x01E0_2858, 0x01E0_2864, 0x01E0_28A8, 0x01E0_28B4, 0x01E0_28F8, 0x01E0_2904, 0x01E0_29B8, 0x01E0_29C4, 0x01E0_2A30, 0x01E0_2A3C, 0x01E0_2AA8,
    0x01E0_2AB4,
];

fn in_window(n: i32) -> bool {
    (0xAC..0x140).contains(&n)
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x2E0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x30);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x60);
    for k in 0..2u32 {
        let h = fx.mem.i32(D + 4);
        let slot = fx.mem.i32(D + 0x80 + 4 * k);
        fx.node_position(h, slot, D + 0x90 + 0x10 * k);
        fx.vu_sub4(sp + 0x3A0, D + 0x90 + 0x10 * k, D + 0x60);
        fx.point_along(D + 0xB0 + 0x40 * k, sp + 0x3A0);
    }
    camera(fx, sp);
    screen_point(fx, sp, D + 0x30, D + 0x40);
    camera(fx, sp);
    screen_point(fx, sp, D + 0x50, D + 0x70);

    flash(fx, sp);
    zoom(fx, sp);
    let f23 = glows(fx, sp);
    spirals(fx, sp, f23);
    let hi = bolts(fx, sp);
    attacker_pass(fx, sp, hi);
    target_pass(fx, sp);
    white_pass(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x160 - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);

    let n = fx.mem.i32(D);
    let mut ret = fx.mem.i32(sp + 0xD0);
    if n == 0xA1 {
        ret = 1;
    } else {
        if in_window(n) {
            ret = 6;
        }
        if n == 0x140 {
            ret = 3;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x161 {
        ret = 8;
    }
    fx.mem.set32(sp + 0xD0, ret);
    ret
}

/// The white full-window flash (D+0x32A8, 33 updates from 0xAB)
/// [F 0x1E005D4..0x1E00728].
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0xAB {
        fx.mem.write_u8(D + 0x32A8, 1);
        fx.mem.write_u32(D + 0x32AC, 0);
    }
    if fx.mem.read_u8(D + 0x32A8) != 1 {
        return;
    }
    let t = sp + 0x3B8;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 0);
    let age = fx.mem.i32(D + 0x32AC);
    let a = fix(fx, ee::mul(128.0, ee::div(ee::from_i32(0x20 - age), 32.0)), 0x01E0_066C);
    rgba(fx, sp + 0x3C8, [0xFF; 3], a as u8);
    far_xy(fx, sp + 0x3D0, 0x7000, 0x7000);
    far_xy(fx, sp + 0x3D8, 0x9000, 0x9000);
    fx.submit(t, OT, 0);
    let age = fx.mem.i32(D + 0x32AC) + 1;
    fx.mem.set32(D + 0x32AC, age);
    if age >= 0x21 {
        fx.mem.write_u8(D + 0x32A8, 0);
    }
}

/// The frame zoom (D+0x32A0, two updates, on even updates of the window)
/// [F 0x1E0072C..0x1E009E8].
fn zoom(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if in_window(n) && rem2(n, 2) == 0 {
        fx.mem.write_u8(D + 0x32A0, 1);
        fx.mem.write_u32(D + 0x32A4, 0);
    }
    if fx.mem.read_u8(D + 0x32A0) != 1 {
        return;
    }
    let d = sp + 0x3E8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    let regs = crate::lens::frame_copy(fx, a, 1, sp + 0x428, sp + 0x430);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x32A4);
    let al = fix(fx, ee::mul(32.0, ee::from_i32(1 - age)), 0x01E0_083C);
    rgba(fx, sp + 0x3F8, [0xFF; 3], al as u8);
    uv(fx, sp + 0x400, 0);
    uv(fx, sp + 0x402, 0);
    uv(fx, sp + 0x410, 0x2000);
    uv(fx, sp + 0x412, 0x2000);
    for k in 0..4i32 {
        let t = ee::div(ee::mul(f(0x3DCC_CCCD), ee::from_i32(k + 1)), 4.0);
        let f20 = ee::mul(256.0, ee::add(1.0, t));
        let x = fixs(fx, f20, 0x01E0_0908);
        let y = fixs(fx, f20, 0x01E0_0914);
        far_xy(fx, sp + 0x408, ((0x800 - x) << 4) as u16, ((0x800 - y) << 4) as u16);
        far_xy(fx, sp + 0x418, ((x + 0x800) << 4) as u16, ((y + 0x800) << 4) as u16);
        fx.submit(d, 0, 0);
    }
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x32A4) + 1;
    fx.mem.set32(D + 0x32A4, age);
    if age >= 2 {
        fx.mem.write_u8(D + 0x32A0, 0);
    }
}

/// The two node glows (D+0x1350, 150 updates from 0xAB)
/// [F 0x1E009EC..0x1E00D74].
fn glows(fx: &mut Fx, sp: u32) -> f32 {
    if fx.mem.i32(D) == 0xAB {
        fx.mem.write_u8(D + 0x1350, 1);
        fx.mem.write_u32(D + 0x1354, 0);
    }
    let age = ee::from_i32(fx.mem.i32(D + 0x1354));
    let f20 = Fx::ratio_below(age, 16.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x1354));
    let f0 = Fx::ramp_above(age, 149.0, 133.0);
    let f23 = ee::mul(f20, f0);
    if fx.mem.read_u8(D + 0x1350) != 1 {
        return f23;
    }
    let d = sp + 0x3E8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, GLOW_CLAMP);
    alpha(fx, d, 1);
    fx.mem.setf(sp + 0x320, -3.0);
    fx.mem.setf(sp + 0x324, -3.0);
    fx.mem.write_u32(sp + 0x328, 0);
    fx.mem.write_u32(sp + 0x32C, 0x3F80_0000);
    vec4(fx, sp + 0x330, [0x4040_0000, 0x4040_0000, 0, 0x3F80_0000]);
    fx.vu_apply_to(sp + 0x320, sp + 0x2E0, sp + 0x320);
    fx.vu_apply_to(sp + 0x330, sp + 0x2E0, sp + 0x330);
    uv(fx, sp + 0x400, 0xB10);
    uv(fx, sp + 0x402, 0x610);
    uv(fx, sp + 0x410, 0xCF0);
    uv(fx, sp + 0x412, 0x7F0);
    for k in 0..2u32 {
        let m = rem2(fx.mem.i32(D + 0x1354), 2);
        let s = ee::add(f(0x3F33_3333), ee::mul(f(0x3E99_999A), ee::from_i32(m)));
        let x = ee::mul(ee::mul(255.0, s), f23);
        if k == 0 {
            let a = fix(fx, x, 0x01E0_0BDC);
            rgba(fx, sp + 0x3F8, [0x40, 0x40, 0xFF], a as u8);
        } else {
            let a = fix(fx, x, 0x01E0_0C60);
            rgba(fx, sp + 0x3F8, [0xFF, 0x40, 0x40], a as u8);
        }
        fx.camera_vp_to(sp + 0x260);
        fx.identity(sp + 0x2A0);
        fx.translate_rows(sp + 0x2A0, sp + 0x2A0, D + 0x90 + 0x10 * k);
        crate::blib::exe::mul_mem(fx, sp + 0x260, sp + 0x2A0);
        if crate::fxlib2::project(fx, sp + 0x320, PROJ_OUT, sp + 0x260, d, sp + 0x5CC) {
            far(fx, sp + 0x408, &[sp + 0x418]);
            fx.submit(d, OT, 0);
        }
    }
    let age = fx.mem.i32(D + 0x1354) + 1;
    fx.mem.set32(D + 0x1354, age);
    if age >= 0x96 {
        fx.mem.write_u8(D + 0x1350, 0);
    }
    f23
}

/// (rand >> 23) % 256 (signed remainder).
pub(crate) fn r256(fx: &mut Fx, ra: u32) -> i32 {
    let v = fx.rand_from(OT, ra) >> 23;
    rem2(v, 0x100)
}

/// The spiral records (D+0x13A0, 16 x 0x1F0): spawned at n % 16 every
/// update of the window, drawn as two nodes x three ribbons
/// [F 0x1E00D78..0x1E015A8].
fn spirals(fx: &mut Fx, sp: u32, f23: f32) {
    let n = fx.mem.i32(D);
    if in_window(n) {
        let r = D + 0x1F0 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r + 0x13A0, 1);
        fx.mem.write_u32(r + 0x13A4, 0);
        for j in 0..2u32 {
            for m in 0..3u32 {
                let s1 = r + 0xC * j + 4 * m;
                let s2 = r + 0xC0 * j + 0x40 * m;
                let s3 = (m << 16) as i32;
                let c = r256(fx, 0x01E0_0DE8);
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                let life = fixs(fx, ee::mul(16.0, ee::add(f(0x3F33_3333), x)), 0x01E0_0E4C);
                fx.mem.set32(s1 + 0x13A8, life);
                let c = r256(fx, 0x01E0_0E54) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x13C0, ee::mul(5.0, ee::add(0.5, x)));
                let c = r256(fx, 0x01E0_0EB0) + 1;
                let x = ee::div(ee::mul(f(0x3E99_999A), ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x13D8, ee::mul(-300.0, ee::add(f(0x3F33_3333), x)));
                let c = r256(fx, 0x01E0_0F1C) + 1;
                let x = ee::div(ee::mul(0.5, ee::from_i32(c)), 256.0);
                fx.mem.setf(s1 + 0x13F0, ee::mul(25.0, ee::add(0.5, x)));
                fx.identity(s2 + 0x1410);
                let c = r256(fx, 0x01E0_0F84);
                fx.rotate_axis(s2 + 0x1410, ((c * 1820) >> 8) - 0x38E, 1);
                let c = r256(fx, 0x01E0_0FCC);
                fx.rotate_axis(s2 + 0x1410, ((c * 1820) >> 8) - 0x38E, 0);
                fx.mult(0x5555_5556, s3);
                let hi = fx.hilo().1;
                fx.rotate_axis(s2 + 0x1410, hi.wrapping_add(((s3 as u32) >> 31) as i32), 2);
            }
        }
    }
    let q = sp + 0x440;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    fx.desc_clamp(q, 5);
    for k in 0..0x10u32 {
        let rec = D + 0x1F0 * k;
        if fx.mem.read_u8(rec + 0x13A0) == 0 {
            continue;
        }
        for j in 0..2u32 {
            let tex = if j == 0 { 0x18 } else { 0x1C };
            let rh = fx.mem.read_u32(D + tex);
            fx.desc_texture(q, rh, 1);
            let mut s5 = (0xD05 * k + 0x115C * j) as i32;
            for m in 0..3u32 {
                let s4 = rec + 0xC * j + 4 * m;
                let age = fx.mem.i32(rec + 0x13A4);
                let life = fx.mem.i32(s4 + 0x13A8);
                if life >= age {
                    ribbon(fx, sp, rec, j, m, s4, s5, f23);
                }
                s5 = s5.wrapping_add(0x15B3);
            }
        }
        let age = fx.mem.i32(rec + 0x13A4) + 1;
        fx.mem.set32(rec + 0x13A4, age);
        if age >= 0x11 {
            fx.mem.write_u8(rec + 0x13A0, 0);
        }
    }
}

/// One ribbon of 16 quads (sub-record `s4`, phase `s5`).
#[allow(clippy::too_many_arguments)]
fn ribbon(fx: &mut Fx, sp: u32, rec: u32, j: u32, m: u32, s4: u32, s5: i32, f23: f32) {
    let q = sp + 0x440;
    fx.camera_vp_to(sp + 0x260);
    let t = std::array::from_fn(|c| fx.mem.f(D + 0x90 + 0x10 * j + 4 * c as u32));
    let mm = fx.trs(None, None, Some(t));
    fx.write_m4(sp + 0x2A0, &mm);
    crate::blib::exe::mul_mem(fx, sp + 0x2A0, D + 0xB0 + 0x40 * j);
    crate::blib::exe::mul_mem(fx, sp + 0x2A0, rec + 0xC0 * j + 0x40 * m + 0x1410);
    crate::blib::exe::mul_mem(fx, sp + 0x260, sp + 0x2A0);
    let age = fx.mem.i32(rec + 0x13A4);
    let life = fx.mem.i32(s4 + 0x13A8);
    let f0 = fx.tan_falloff_p(age, life, 1);
    let f26 = ee::sub(1.0, f0);
    let f20 = ee::div(ee::mul(32.0, ee::from_i32(rem2(m as i32, 4))), 128.0);
    let f24 = ee::add(0.25, f20);
    let f25 = ee::add(1.0, ee::sub(1.0, f26));
    let mut s3 = 0i32;
    for s1 in 0..0x10i32 {
        let c = D + 0x1358 + 4 * s1 as u32;
        let a0 = ee::from_i32(i32::from(fx.mem.read_u8(c + 3)));
        let s6 = fix(fx, ee::mul(f23, ee::mul(a0, f26)), 0x01E0_1280) & 0xFF;
        let a1 = ee::from_i32(i32::from(fx.mem.read_u8(c + 7)));
        let v0 = fix(fx, ee::mul(f23, ee::mul(a1, f26)), 0x01E0_12C4);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + k);
            fx.mem.write_u8(sp + 0x450 + k, b);
        }
        fx.mem.write_u8(sp + 0x453, s6 as u8);
        fx.mem.write_u32(sp + 0x454, 0x3F80_0000);
        for k in 0..3 {
            let b = fx.mem.read_u8(c + 4 + k);
            fx.mem.write_u8(sp + 0x480 + k, b);
        }
        fx.mem.write_u8(sp + 0x483, v0 as u8);
        fx.mem.write_u32(sp + 0x484, 0x3F80_0000);
        let f21 = ee::div(ee::from_i32(s1), 16.0);
        copy8(fx, sp + 0x468, sp + 0x450);
        copy8(fx, sp + 0x498, sp + 0x480);
        let hi = s1 % 3;
        let f4 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(hi)), 3.0), 128.0);
        fx.mem.setf(sp + 0x458, f20);
        fx.mem.setf(sp + 0x45C, f4);
        fx.mem.setf(sp + 0x470, f24);
        fx.mem.setf(sp + 0x474, f4);
        fx.mem.setf(sp + 0x488, f20);
        let f0 = ee::div(ee::div(ee::mul(128.0, ee::from_i32(hi + 1)), 3.0), 128.0);
        fx.mem.setf(sp + 0x48C, f0);
        fx.mem.setf(sp + 0x4A0, f24);
        fx.mem.setf(sp + 0x4A4, f0);
        let f22 = ee::div(ee::from_i32(s1 + 1), 16.0);
        let f3 = fx.mem.f(s4 + 0x13D8);
        let f29 = ee::mul(fx.mem.f(s4 + 0x13C0), f26);
        let f27 = ee::mul(f3, f21);
        let f28 = ee::mul(f3, f22);
        let sn = fx.tsin(s5.wrapping_add(s3 >> 4) & 0xFFFF);
        let amp = fx.mem.f(s4 + 0x13F0);
        let a = s5.wrapping_add(((s1 + 1) << 17) >> 4) & 0xFFFF;
        let f21 = ee::mul(f21, ee::mul(f25, ee::mul(amp, sn)));
        let sn = fx.tsin(a);
        let amp = fx.mem.f(s4 + 0x13F0);
        let f3 = ee::neg(f29);
        let f1 = ee::mul(f22, ee::mul(f25, ee::mul(amp, sn)));
        fx.mem.setf(sp + 0x320, f21);
        fx.mem.setf(sp + 0x324, ee::add(f3, f21));
        fx.mem.setf(sp + 0x328, f27);
        fx.mem.write_u32(sp + 0x32C, 0x3F80_0000);
        fx.mem.setf(sp + 0x330, f21);
        fx.mem.setf(sp + 0x334, ee::add(f29, f21));
        fx.mem.setf(sp + 0x338, f27);
        fx.mem.write_u32(sp + 0x33C, 0x3F80_0000);
        fx.mem.setf(sp + 0x340, f1);
        fx.mem.setf(sp + 0x344, ee::add(f3, f1));
        fx.mem.setf(sp + 0x348, f28);
        fx.mem.write_u32(sp + 0x34C, 0x3F80_0000);
        fx.mem.setf(sp + 0x350, f1);
        fx.mem.setf(sp + 0x354, ee::add(f29, f1));
        fx.mem.setf(sp + 0x358, f28);
        fx.mem.write_u32(sp + 0x35C, 0x3F80_0000);
        let mm = fx.read_m4(sp + 0x260);
        if fx.project_desc(sp + 0x320, PROJ_OUT, Some(&mm), q, sp + 0x5CC) {
            fx.submit(q, OT, 0);
        }
        s3 = s3.wrapping_add(0x20000);
    }
}

/// The bolt records (D+0x32B0, 8 x 0x1E0): spawned at n % 8 every update
/// of the window at the target's point, 16 strips each with a sprite
/// [F 0x1E015AC..0x1E01F40].
fn bolts(fx: &mut Fx, sp: u32) -> u64 {
    let mut hi = 0;
    let n = fx.mem.i32(D);
    if in_window(n) {
        let r = D + 0x1E0 * rem2(n, 8) as u32;
        fx.mem.write_u8(r + 0x32B0, 1);
        fx.mem.write_u32(r + 0x32B4, 0);
        for s4 in 0..0x10u32 {
            let s1 = r + 4 * s4;
            let s2 = r + 0x10 * s4;
            let c = r256(fx, 0x01E0_160C);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            let life = fixs(fx, ee::mul(8.0, ee::add(f(0x3E99_999A), x)), 0x01E0_1670);
            fx.mem.set32(s1 + 0x32B8, life);
            let c = r256(fx, 0x01E0_1678);
            let x = ee::div(ee::mul(f(0x3F33_3333), ee::from_i32(c)), 256.0);
            fx.mem.setf(s1 + 0x3338, ee::mul(10.0, ee::add(f(0x3E99_999A), x)));
            let c = r256(fx, 0x01E0_16E0);
            fx.mem.set32(s1 + 0x32F8, (c << 16) >> 8);
            for w in 0..4u32 {
                let v = fx.mem.read_u32(D + 0x60 + 4 * w);
                fx.mem.write_u32(r + 0x3380 + 4 * w, v);
            }
            fx.random_table_vec(s2 + 0x3390, 1.5);
        }
    }
    let b = sp + 0x548;
    head(fx, b, 0x24, 9, 0x4_2424_2421, 0x800);
    alpha(fx, b, 1);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(b, rh, 1);
    fx.desc_clamp(b, BOLT_CLAMP);
    let d = sp + 0x3E8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 1);
    let rh = fx.mem.read_u32(D + 0x20);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, GLOW_CLAMP);
    for k in 0..8u32 {
        let s3 = D + 0x1E0 * k;
        if fx.mem.read_u8(s3 + 0x32B0) == 0 {
            continue;
        }
        let s6 = s3 + 0x3380;
        for s5 in 0..0x10i32 {
            let s1 = s3 + 4 * s5 as u32;
            let s2 = s3 + 0x10 * s5 as u32;
            let age = fx.mem.i32(s3 + 0x32B4);
            let life = fx.mem.i32(s1 + 0x32B8);
            if life < age {
                continue;
            }
            let (q, _) = fx.div(age << 14, life);
            let f20 = fx.tcos(q);
            let (c, ras) = if rem2(s5, 2) == 0 { ([0xFF, 0x40, 0x40], [0x01E0_1920, 0x01E0_1950]) } else { ([0x40, 0x40, 0xFF], [0x01E0_1984, 0x01E0_19B4]) };
            let a = fix(fx, ee::mul(64.0, f20), ras[0]);
            rgba(fx, sp + 0x558, c, a as u8);
            let a = fix(fx, ee::mul(255.0, f20), ras[1]);
            rgba(fx, sp + 0x3F8, c, a as u8);
            let v = rem2(s5, 4);
            let a1 = rem2(v, 2);
            let row = ((v >> 1) << 6) + 0x80;
            let u0 = 59 * a1 + 0xA;
            let (t2, a3) = (((u0 << 4) & 0x3FFF) as u32, (((u0 + 0x3B) << 4) & 0x3FFF) as u32);
            let (t0, a2) = (((row << 4) & 0x3FFF) as u32, (((row + 0x40) << 4) & 0x3FFF) as u32);
            uv(fx, sp + 0x560, t2);
            uv(fx, sp + 0x562, t0);
            uv(fx, sp + 0x570, a3);
            uv(fx, sp + 0x572, t0);
            uv(fx, sp + 0x580, t2);
            uv(fx, sp + 0x582, a2);
            uv(fx, sp + 0x590, a3);
            uv(fx, sp + 0x592, a2);
            uv(fx, sp + 0x400, 0xB00);
            uv(fx, sp + 0x402, 0x600);
            uv(fx, sp + 0x410, 0xD00);
            uv(fx, sp + 0x412, 0x800);
            let age = fx.mem.i32(s3 + 0x32B4);
            let life = fx.mem.i32(s1 + 0x32B8);
            let (q, _) = fx.div(age << 14, life);
            let sn = fx.tsin(q);
            let f20 = fx.mem.f(s1 + 0x3338);
            let f3 = ee::mul(ee::mul(0.5, f20), sn);
            let f2 = ee::neg(f20);
            let (a, b1, c2, d0) = (ee::add(f2, f3), ee::sub(f2, f3), ee::add(f20, f3), ee::sub(f20, f3));
            for (o, x, y) in [(0x320u32, a, b1), (0x330, c2, b1), (0x340, b1, c2), (0x350, d0, c2)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, y);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x260);
            fx.vu_add4(sp + 0x3A0, s2 + 0x3390, s6);
            fx.translate_rows(sp + 0x2A0, sp + 0x2E0, sp + 0x3A0);
            // 0x188420's `lq t0, 0x10(a1)`: t0's upper half the turn's
            // row 1 z / w.
            hi = fx.mem.read_u64(sp + 0x2F8);
            let ang = fx.mem.i32(s1 + 0x32F8);
            fx.rotate_axis(sp + 0x2A0, ang, 2);
            crate::blib::exe::mul_mem(fx, sp + 0x260, sp + 0x2A0);
            if crate::fxlib2::project(fx, sp + 0x320, PROJ_OUT, sp + 0x260, b, sp + 0x5CC) {
                far(fx, sp + 0x568, &[sp + 0x578, sp + 0x588, sp + 0x598]);
                fx.submit(b, OT, 0);
            }
            let s = ee::mul(f(0x3D4C_CCCD), fx.mem.f(s1 + 0x3338));
            let ns = ee::neg(s);
            for (o, x) in [(0x320u32, ns), (0x330, s)] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, x);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.camera_vp_to(sp + 0x260);
            let age = ee::from_i32(fx.mem.i32(s3 + 0x32B4));
            let life = ee::from_i32(fx.mem.i32(s1 + 0x32B8));
            let t = ee::add(f(0x3F4C_CCCD), ee::div(ee::mul(f(0x3E4C_CCCD), age), life));
            fx.vu_scale4(sp + 0x3A0, s2 + 0x3390, ee::mul(-1.0, t));
            fx.vu_add4(sp + 0x3A0, sp + 0x3A0, s6);
            fx.translate_rows(sp + 0x2A0, sp + 0x2E0, sp + 0x3A0);
            crate::blib::exe::mul_mem(fx, sp + 0x260, sp + 0x2A0);
            if crate::fxlib2::project(fx, sp + 0x320, PROJ_OUT, sp + 0x260, d, sp + 0x5CC) {
                far(fx, sp + 0x408, &[sp + 0x418]);
                fx.submit(d, OT, 0);
            }
        }
        let age = fx.mem.i32(s3 + 0x32B4) + 1;
        fx.mem.set32(s3 + 0x32B4, age);
        if age >= 9 {
            fx.mem.write_u8(s3 + 0x32B0, 0);
        }
    }
    hi
}

/// The attacker drawn into its target with callback A and pulled through
/// the fan (D+0x130, 257 updates from 0x40) [F 0x1E01F44..0x1E024A4].
/// `hi`: the upper half of the record's t0 at the 0x1F8280 call (the post
/// callback's register, 0x1F0FD0's s1).
fn attacker_pass(fx: &mut Fx, sp: u32, hi: u64) {
    if fx.mem.i32(D) == 0x40 {
        fx.mem.write_u8(D + 0x130, 1);
        fx.mem.write_u32(D + 0x134, 0);
    }
    if fx.mem.read_u8(D + 0x130) != 1 {
        return;
    }
    camera(fx, sp);
    vec4(fx, sp + 0x3A0, [0, 0, 0, 0x3F80_0000]);
    let y = fx.mem.read_u32(D + 0x34);
    fx.mem.write_u32(sp + 0x3A4, y);
    let m = fx.read_m4(sp + 0x260);
    fx.project_at(sp + 0x3A0, &m, sp + 0x5C0, sp + 0x5CC);
    let a = fan_desc(fx, sp, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x25C, layer);
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let (_, regs) = fx.unit_draw_post(1, sp + 0x530, sp + 0x538, CB_A, &mut |fx, p| sphere(fx, p, hi));
    fx.texflush(regs);
    let ramp = |fx: &mut Fx| {
        let age = ee::from_i32(fx.mem.i32(D + 0x134));
        let a = Fx::ratio_below(age, 16.0);
        let age = ee::from_i32(fx.mem.i32(D + 0x134));
        ee::mul(a, Fx::ramp_above(age, 256.0, 240.0))
    };
    let f21 = ramp(fx);
    let f1 = ramp(fx);
    let f20 = ee::mul(128.0, f21);
    let f21 = ee::mul(128.0, f1);
    let s1 = fix(fx, f20, 0x01E0_2110) & 0xFF;
    let v0 = fix(fx, f21, 0x01E0_211C);
    rgba(fx, sp + 0x4E0, [0x80; 3], s1 as u8);
    rgba(fx, sp + 0x4F8, [0x80; 3], v0 as u8);
    copy8(fx, sp + 0x510, sp + 0x4F8);
    fan(fx, sp, 1.0, &FAN_A);
    let layer = fx.mem.read_u32(sp + 0x25C);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x134) + 1;
    fx.mem.set32(D + 0x134, age);
    if age >= 0x101 {
        fx.mem.write_u8(D + 0x130, 0);
    }
}

/// On even updates of the window the target drawn into its target with
/// callback B, composited by a full-window sprite [F 0x1E024A8..0x1E026B8].
fn target_pass(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if !(in_window(n) && rem2(n, 2) == 0) {
        return;
    }
    let d = sp + 0x3E8;
    head(fx, d, 0x26, 5, 0x4_2421, 0x800);
    alpha(fx, d, 0);
    fx.desc_clamp(d, LENS_CLAMP);
    let a = clamp_regs(fx, d, LENS_CLAMP);
    let layer = fx.layer_begin();
    fx.texflush(a);
    fx.mem.write_u32(D_STATIC, D);
    let t = fx.mem.i32(D + 8);
    let (_, regs) = fx.unit_target_post(t, 1, sp + 0x428, sp + 0x430, CB_B, &mut |fx, p| veil(fx, p));
    fx.texflush(regs);
    rgba(fx, sp + 0x3F8, [0x80; 3], 0x80);
    uv(fx, sp + 0x400, 0);
    uv(fx, sp + 0x402, 0);
    uv(fx, sp + 0x410, 0x2000);
    uv(fx, sp + 0x412, 0x2000);
    far_xy(fx, sp + 0x408, 0x7000, 0x7000);
    far_xy(fx, sp + 0x418, 0x9000, 0x9000);
    fx.submit(d, 0, 0);
    fx.layer_end(layer, OT, 0);
}

/// The attacker's white flash (D+0x41B0, 17 updates from 0x1C) as three
/// widening fans [F 0x1E026BC..0x1E02B80].
fn white_pass(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0x1C {
        fx.mem.write_u8(D + 0x41B0, 1);
        fx.mem.write_u32(D + 0x41B4, 0);
    }
    if fx.mem.read_u8(D + 0x41B0) != 1 {
        return;
    }
    let a = fan_desc(fx, sp, 1);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0x258, layer);
    fx.texflush(a);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x530, sp + 0x538);
    fx.texflush(regs);
    let age = fx.mem.i32(D + 0x41B4);
    rgba(fx, sp + 0x4E0, [0xFF; 3], 0);
    rgba(fx, sp + 0x4F8, [0xFF; 3], (((0x10 - age) << 7) >> 4) as u8);
    copy8(fx, sp + 0x510, sp + 0x4F8);
    let age = fx.mem.i32(D + 0x41B4);
    let f20 = ee::mul(0.5, fx.tan_falloff_p(age, 0x10, 1));
    for j in 0..3i32 {
        let s = ee::add(1.0, ee::div(ee::mul(f20, ee::from_i32(j + 1)), 3.0));
        fan(fx, sp, s, &FAN_B);
    }
    let layer = fx.mem.read_u32(sp + 0x258);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x41B4) + 1;
    fx.mem.set32(D + 0x41B4, age);
    if age >= 0x11 {
        fx.mem.write_u8(D + 0x41B0, 0);
    }
}

/// Callback A 0x1E02CB0(p) [F]: run by 0x1F0FD0 after the attacker's
/// render into its target. FRAME_1 masked (0x2096C0 0xFF000000), a grey
/// full-window veil (40, 40, 40, 80) at ALPHA preset 2, then the sphere
/// (D+0x140) at the attacker's point under a random turn as 16 x 16
/// textured quads (texture D+0x10 / D+0x14 by the update's parity, ST
/// quarters), far Z, buffered; FRAME_1 unmasked. The quad's TEX0 / TEX1
/// slots (sp + 0x200 / 0x208) are not written when the texture is missing:
/// they hold 0x209800's save of 0x1F0FD0's s1 (`sq s1, 0x10(sp)` at the
/// caller's sp - 0x20) - the callback's address and `hi`, its register's
/// upper half.
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
    rgba(fx, sp + 0x168, [0x40; 3], 0x80);
    far_xy(fx, sp + 0x170, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x178, 0x8FF8, 0x8FF8);
    let mut s6 = fx.submit_at(s0, t, 0, 0);
    let q = sp + 0x188;
    head(fx, q, 0x34, 0xC, 0x4214_2142_1421, 0);
    alpha(fx, q, 1);
    let tex = if rem2(fx.mem.i32(d), 2) == 0 { 0x10 } else { 0x14 };
    let rh = fx.mem.read_u32(d + tex);
    fx.desc_texture(q, rh, 1);
    fx.desc_clamp(q, 0);
    fx.camera_vp_to(sp + 0xD0);
    fx.identity(sp + 0x110);
    fx.translate_rows(sp + 0x110, sp + 0x110, d + 0x30);
    for (axis, ra) in [(0, 0x01E0_2EAC), (1, 0x01E0_2EE0), (2, 0x01E0_2F14)] {
        let v = fx.rand_from(s0, ra) >> 23;
        fx.rotate_axis(sp + 0x110, (rem2(v, 0x100) << 16) >> 8, axis);
    }
    crate::blib::exe::mul_mem(fx, sp + 0xD0, sp + 0x110);
    rgba(fx, sp + 0x198, [0x80; 3], 0x80);
    for to in [0x1B0, 0x1C8, 0x1E0] {
        copy8(fx, sp + to, sp + 0x198);
    }
    // a2 / a3 at the closing 0x2096C0: 0x206F90's zeros after a drawn
    // quad, else 0x205210's untouched m / desc of the last rejected one.
    let mut a23 = [0u32, 0];
    for i in 0..0x10i32 {
        let s3 = d + 0x110 * i as u32;
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
            let (a, b) = (0x10 * j as u32 + 0x140, 0x10 * (j + 1) as u32 + 0x140);
            if fx.project_n(&[s3 + a, s3 + b, s5 + a, s5 + b], sp + 0xD0, q, sp + 0x21C, 0x01E0_30B4) != 1 {
                a23 = [sp + 0xD0, q];
                continue;
            }
            far(fx, sp + 0x1A8, &[sp + 0x1C0, sp + 0x1D8, sp + 0x1F0]);
            s6 = fx.submit_at(s6, q, 0, 0);
            a23 = [0, 0];
        }
    }
    let r = frame_mask_packet(fx, s6, 0, a23);
    fx.leave(0x220);
    r
}

/// Callback B 0x1E03210(p) [F]: run after the target's render into its
/// target: FRAME_1 masked, a dark full-window veil (80, 80, 80, FF) at
/// ALPHA preset 3, buffered; FRAME_1 unmasked.
fn veil(fx: &mut Fx, p: u32) -> u32 {
    let sp = fx.enter(0x50);
    let s0 = frame_mask_packet(fx, p, 0xFF00_0000, [0, 0]);
    let t = sp + 0x20;
    head(fx, t, 6, 3, 0x441, 0);
    alpha(fx, t, 3);
    rgba(fx, sp + 0x30, [0x80; 3], 0xFF);
    far_xy(fx, sp + 0x38, 0x6FF8, 0x6FF8);
    far_xy(fx, sp + 0x40, 0x8FF8, 0x8FF8);
    let v = fx.submit_at(s0, t, 0, 0);
    let r = frame_mask_packet(fx, v, 0, [0, 0]);
    fx.leave(0x50);
    r
}
