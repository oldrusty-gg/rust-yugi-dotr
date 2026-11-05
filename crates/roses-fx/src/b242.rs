//! BMODULE b242 (card 565), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x7910
//! bytes) [F]: b483's lens family in a 0x4A0
//! frame (s0 = D, every local 0x38..0xB0 higher). Every update the camera
//! turn with roll (0x2055E0 into sp + 0x1E0) and the attacker's point
//! (0x1F65D0 kind 1 into D+0x20) raised by 2. From update 0 for 0xBE
//! updates the lens: a sprite (24, far Z) at the point, then in a packet
//! layer (TEXFLUSH, the frame copied to TBP 0x2000 by 0x2095A0 /
//! 0x207F50, TEXFLUSH) a disc of 32 rings x 16 quads (the centre ring
//! triangles) of radius 12 on the camera plane, UVs the screen positions
//! of a breathing copy (D+0x2250: radius x (1 + 0.2 sin(0xD05 age + ...)))
//! and positions those of the plain disc (D+0x50), far Z, the layer at key
//! 0. On 0x43..0xBC one of 128 records (+0x4450, 0x50) of two growing
//! turning quads flying from the point toward the target (their key the
//! mean w), live 0x81 updates; on 0x9D..0x121 one of 12 records (+0x6C50,
//! 0x110) of 12 sprites spreading about the target for 13 updates.
//! Returns 1 on update 0x92, 6 on 0x9D..0x12D, 2 on 0x12E, 8 once the
//! count reaches 0x13E.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// XYZ2 low 32 bits and the top byte kept: Z replaced.
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The lens' CLAMP.
const LENS_CLAMP: u64 = 0x0000_0800_0080_000A;
/// The plain disc (32 rings of 17 points, 0x110 apart) and its breathing
/// copy.
const DISC: u32 = D + 0x50;
const WARP: u32 = D + 0x2250;
/// The 128 flying-quad records (0x50) and the 12 sprite records (0x110).
const QUADS: u32 = D + 0x4450;
const SPRITES: u32 = D + 0x6C50;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x4A0);
    fx.mem.write_u32(sp + 0xB0, 0);
    fx.mem.write_u32(sp + 0xC8, OT);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(0x4A0);
    ret
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x7910);
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
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x30);
    fx.mem.write_u8(D + 0x40, 0);
    for s1 in 0..0x20u32 {
        let f20 = ee::div(ee::mul(12.0, ee::from_i32(0x20 - s1 as i32)), 32.0);
        for s2 in 0..0x11u32 {
            let a = (0x10000 * s2 as i32) >> 4;
            let o = DISC + 0x110 * s1 + 0x10 * s2;
            let sn = fx.table_trig(a, false);
            fx.mem.setf(o, ee::mul(f20, sn));
            let cs = fx.table_trig(a, true);
            fx.mem.setf(o + 4, ee::mul(f20, cs));
            fx.mem.write_u32(o + 8, 0);
            fx.mem.write_u32(o + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..0x80u32 {
        fx.mem.write_u8(QUADS + 0x50 * k, 0);
    }
    for k in 0..0xCu32 {
        fx.mem.write_u8(SPRITES + 0x110 * k, 0);
    }
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

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The far Z into the XYZ2 at `first`, then its Z into the others.
fn far_z(fx: &mut Fx, first: u32, others: &[u32]) {
    let v = fx.mem.read_u64(first);
    let v = (v & KEEP) | FAR_Z;
    fx.mem.write_u64(first, v);
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    for &o in others {
        let w = fx.mem.read_u64(o);
        fx.mem.write_u64(o, (w & KEEP) | z);
    }
}

/// A sprite descriptor at `d` (5 registers 0x42421, prim 0x26 | 0x800).
fn sprite_desc(fx: &mut Fx, d: u32) {
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// 0x1A2C70 of `f` (its frame left from `ra`).
fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn two(fx: &mut Fx, to: u32, from: u32) {
    let a = fx.mem.read_u32(from);
    let b = fx.mem.read_u32(from + 4);
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

/// The lens [F 0x1E0025C..0x1E00B08] (D+0x40 = 1).
fn lens(fx: &mut Fx, sp: u32) {
    let d1 = sp + 0x2B8;
    sprite_desc(fx, d1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d1, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 0);
    uv(fx, sp + 0x2D0, 0);
    uv(fx, sp + 0x2D2, 0);
    uv(fx, sp + 0x2E0, 0x200);
    uv(fx, sp + 0x2E2, 0x200);
    let s = 24.0f32;
    fx.mem.setf(sp + 0x220, s);
    fx.mem.setf(sp + 0x224, s);
    fx.mem.write_u32(sp + 0x228, 0);
    fx.mem.write_u32(sp + 0x22C, 0x3F80_0000);
    fx.mem.setf(sp + 0x230, ee::neg(s));
    fx.mem.setf(sp + 0x234, ee::neg(s));
    fx.mem.write_u32(sp + 0x238, 0);
    fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
    let age = ee::from_i32(fx.mem.i32(D + 0x44));
    let f20 = Fx::ratio_below(age, 32.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x44));
    let f1 = ee::mul(f20, Fx::ramp_above(age, 189.0, 157.0));
    let f20 = ee::mul(128.0, f1);
    let a = fixu(fx, f20, 0x01E0_038C);
    fx.mem.write_u8(sp + 0x2C8, 0x80);
    fx.mem.write_u8(sp + 0x2C9, 0x80);
    fx.mem.write_u8(sp + 0x2CA, 0x80);
    fx.mem.write_u8(sp + 0x2CB, a as u8);
    fx.mem.write_u32(sp + 0x2CC, 0x3F80_0000);
    fx.camera_vp_to(sp + 0x160);
    fx.translate_rows(sp + 0x1A0, sp + 0x1E0, D + 0x20);
    let m = fx.read_m4(sp + 0x1A0);
    fx.mul_into(Some(sp + 0x160), &m);
    let mut a23 = [0u32, 0];
    if crate::fxlib2::project(fx, sp + 0x220, PROJ_OUT, sp + 0x160, d1, sp + 0x49C) {
        far_z(fx, sp + 0x2D8, &[sp + 0x2E8]);
        fx.submit(d1, OT, 0);
        a23 = [0, KEEP as u32];
    }
    // The lens descriptors: triangles sp + 0x388 (prim 0x33), quads
    // sp + 0x400 (prim 0x34), each vertex RGBAQ / UV / XYZ2.
    let (d2, d3) = (sp + 0x388, sp + 0x400);
    fx.mem.write_u16(d2 + 2, 0x33);
    fx.mem.write_u16(d2, 9);
    fx.mem.write_u64(d2 + 8, 0x4_2142_1421);
    fx.mem.write_u32(d2 + 4, 0);
    fx.mem.write_u16(d3 + 2, 0x34);
    fx.mem.write_u16(d3, 0xC);
    fx.mem.write_u64(d3 + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d3 + 4, 0);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xCC, layer);
    let p0 = fx.mem.read_u32(crate::world::PACKET_POINTER);
    fx.texflush([0x2142_1421, d2, a23[0], a23[1]]);
    let p1 = fx.mem.read_u32(crate::world::PACKET_POINTER);
    let dest = fx.copy_dest([p1, p1, p0, 0x8000]);
    let regs = fx.frame_texture_packet(dest, 1, sp + 0x3E8, sp + 0x3F0);
    let v = fx.mem.read_u64(sp + 0x3E8);
    fx.mem.write_u64(sp + 0x478, v);
    let v = fx.mem.read_u64(sp + 0x3F0);
    fx.mem.write_u64(sp + 0x480, v);
    fx.texflush([sp + 0x478, regs[1], regs[2], regs[3]]);
    for d in [d2, d3] {
        let a = fx.alpha_preset(0);
        fx.desc_alpha(d, a);
        fx.desc_clamp(d, LENS_CLAMP);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
    }
    let a = fixu(fx, f20, 0x01E0_0598);
    fx.mem.write_u8(sp + 0x398, 0x80);
    fx.mem.write_u8(sp + 0x399, 0x80);
    fx.mem.write_u8(sp + 0x39A, 0xFF);
    fx.mem.write_u8(sp + 0x39B, a as u8);
    fx.mem.write_u32(sp + 0x39C, 0x3F80_0000);
    two(fx, sp + 0x3B0, sp + 0x398);
    two(fx, sp + 0x3C8, sp + 0x398);
    two(fx, sp + 0x490, sp + 0x398);
    for o in [0x410u32, 0x428, 0x440, 0x458] {
        two(fx, sp + o, sp + 0x490);
    }
    breathe(fx, sp);
    fx.mem.write_u32(sp + 0x2A0, 0);
    fx.mem.write_u32(sp + 0x2A4, 0);
    fx.mem.write_u32(sp + 0x2A8, 0);
    fx.mem.write_u32(sp + 0x2AC, 0x3F80_0000);
    for k in 0..0x20u32 {
        fx.mem.write_u32(sp + 0x150, k);
        let fp = 0x110 * k;
        if k == 0x1F {
            for s1 in 0..0x10u32 {
                let (s2, s4) = (fp + 0x10 * s1, fp + 0x10 * (s1 + 1));
                let r = fx.project_n(&[sp + 0x2A0, WARP + s2, WARP + s4], sp + 0x160, d2, sp + 0x49C, 0x01E0_07B0);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x3A0u32, 0x3A8u32), (0x3B8, 0x3C0), (0x3D0, 0x3D8)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let r = fx.project_n(&[sp + 0x2A0, DISC + s2, DISC + s4], sp + 0x160, d2, sp + 0x49C, 0x01E0_0814);
                if r == 1 {
                    far_z(fx, sp + 0x3A8, &[sp + 0x3C0, sp + 0x3D8]);
                    fx.submit(d2, 0, 0);
                }
            }
        } else {
            let next = 0x110 * (k + 1);
            fx.mem.write_u32(sp + 0x120, D + next);
            for s6 in 0..0x10u32 {
                let (s2, s5) = (fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
                let (s1, s4) = (next + 0x10 * s6, next + 0x10 * (s6 + 1));
                let r = fx.project_n(&[WARP + s2, WARP + s5, WARP + s1, WARP + s4], sp + 0x160, d3, sp + 0x49C, 0x01E0_0944);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x418u32, 0x420u32), (0x430, 0x438), (0x448, 0x450), (0x460, 0x468)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let r = fx.project_n(&[DISC + s2, DISC + s5, DISC + s1, DISC + s4], sp + 0x160, d3, sp + 0x49C, 0x01E0_09C0);
                if r == 1 {
                    far_z(fx, sp + 0x420, &[sp + 0x438, sp + 0x450, sp + 0x468]);
                    fx.submit(d3, 0, 0);
                }
            }
        }
    }
    fx.mem.write_u32(sp + 0x150, 0x20);
    let layer = fx.mem.read_u32(sp + 0xCC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x44) + 1;
    fx.mem.set32(D + 0x44, age);
    if age >= 0xBE {
        fx.mem.write_u8(D + 0x40, 0);
    }
}

/// The breathing disc D+0x2250 [F 0x1E00600..0x1E00748]: ring r's point
/// p at radius 12 (32 - r) / 32 x (1 + 0.2 sin(0xD05 age + (0x30000 r) >>
/// 5 + (0x20000 p) >> 4)), angle (0x10000 p) >> 4.
fn breathe(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0xD0, D);
    let mut fp = 0i32;
    for s7 in 0..0x20i32 {
        let s3 = fx.mem.read_u32(sp + 0xD0);
        let f21 = ee::div(ee::mul(12.0, ee::from_i32(0x20 - s7)), 32.0);
        let s4 = fp >> 5;
        let (mut s1, mut s2) = (0i32, 0i32);
        for s5 in 0..0x11u32 {
            let age = fx.mem.i32(D + 0x44);
            let v = s4.wrapping_add(s1 >> 4);
            let m = fx.mult(age, 0xD05);
            let sn = fx.table_trig(m.wrapping_add(v) & 0xFFFF, false);
            let f1 = ee::mul(f32::from_bits(0x3E4C_CCCD), sn);
            let s6 = s2 >> 4;
            let f20 = ee::add(1.0, f1);
            let sn = fx.table_trig(s6, false);
            let f20 = ee::mul(f21, f20);
            let o = s3 + 0x10 * s5;
            fx.mem.setf(o + 0x2250, ee::mul(f20, sn));
            let cs = fx.table_trig(s6, true);
            fx.mem.setf(o + 0x2254, ee::mul(f20, cs));
            fx.mem.write_u32(o + 0x2258, 0);
            fx.mem.write_u32(o + 0x225C, 0x3F80_0000);
            s1 = s1.wrapping_add(0x20000);
            s2 = s2.wrapping_add(0x10000);
        }
        fx.mem.write_u32(sp + 0xD0, s3 + 0x110);
        fp = fp.wrapping_add(0x30000);
    }
}

/// On 0x43..0xBC [F 0x1E00B28]: record n % 128 of two flying quads from
/// the point D+0x20 (size 0.5..1, a random angle, a random direction
/// toward the target).
fn spawn_quads(fx: &mut Fx, n: i32) {
    let rec = QUADS + 0x50 * rem2(n, 0x80) as u32;
    fx.mem.write_u8(rec, 1);
    fx.mem.write_u32(rec + 4, 0);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(D + 0x20 + 4 * c);
        fx.mem.write_u32(rec + 0x20 + 4 * c, w);
    }
    for s5 in 0..2u32 {
        let r = fx.rand_from(D, 0x01E0_0B94) >> 23;
        fx.mem.set8(rec + 8 + s5, rem2(r, 4));
        let r = fx.rand_from(D, 0x01E0_0BB4) >> 23;
        let f = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(rem2(r, 256))), 256.0));
        fx.mem.setf(rec + 0xC + 4 * s5, f);
        let r = fx.rand_from(D, 0x01E0_0C00) >> 23;
        fx.mem.set32(rec + 0x14 + 4 * s5, (rem2(r, 256) << 16) >> 8);
        let v = rec + 0x30 + 0x10 * s5;
        fx.random_table_vec(v, 1.0);
        fx.vu_add4(v, v, D + 0x30);
        fx.vu_sub4(v, v, rec + 0x20);
    }
}

/// The flying quads [F 0x1E00C70..0x1E01148].
fn quads(fx: &mut Fx, sp: u32) {
    let d = sp + 0x310;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    let id = fx.read_m4(sp + 0x1A0);
    fx.mul_into(Some(sp + 0x160), &id);
    fx.mem.write_u32(sp + 0xF0, sp + 0x32A);
    fx.mem.write_u32(sp + 0x100, sp + 0x33A);
    fx.mem.write_u32(sp + 0x110, sp + 0x34A);
    for k in 0..0x80u32 {
        fx.mem.write_u32(sp + 0x140, k);
        let s4 = QUADS + 0x50 * k;
        if fx.mem.read_u8(s4) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(s4 + 4));
        let f20 = ee::mul(128.0, Fx::ratio_below(age, 12.0));
        let age = ee::from_i32(fx.mem.i32(s4 + 4));
        let f0 = Fx::ramp_above(age, 128.0, 96.0);
        let al = fixu(fx, ee::mul(f20, f0), 0x01E0_0D74);
        fx.mem.write_u8(sp + 0x320, 0x80);
        fx.mem.write_u8(sp + 0x321, 0x80);
        fx.mem.write_u8(sp + 0x322, 0xFF);
        fx.mem.write_u8(sp + 0x323, al as u8);
        fx.mem.write_u32(sp + 0x324, 0x3F80_0000);
        let f4 = ee::from_i32(fx.mem.i32(s4 + 4));
        let f2 = ee::div(ee::mul(f32::from_bits(0x3F33_3333), f4), 128.0);
        fx.mem.write_u32(sp + 0xE0, s4 + 0x20);
        let f21 = ee::div(ee::mul(1.5, f4), 128.0);
        let f20 = ee::add(f32::from_bits(0x3E99_999A), f2);
        for s6 in 0..2u32 {
            let age = fx.mem.i32(s4 + 4);
            let size = fx.mem.f(s4 + 0xC + 4 * s6);
            let f3 = ee::mul(0.5, ee::from_i32(age));
            let m = fx.mult(age, 0x15B3);
            let f2 = ee::add(0.5, ee::div(f3, 128.0));
            let f22 = ee::mul(ee::mul(10.0, f2), size);
            let s7 = fx.mem.i32(s4 + 0x14 + 4 * s6).wrapping_add(m) & 0xFFFF;
            let sn = fx.table_trig(s7, false);
            let f23 = ee::mul(f22, ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), sn)));
            let cs = fx.table_trig(s7, true);
            let h = ee::mul(f22, ee::add(f32::from_bits(0x3F66_6666), ee::mul(f32::from_bits(0x3DCC_CCCD), cs)));
            for (o, x, y) in [(0x220u32, f23, f23), (0x230, ee::neg(f23), h), (0x240, h, ee::neg(f23)), (0x250, ee::neg(h), ee::neg(h))] {
                fx.mem.setf(sp + o, x);
                fx.mem.setf(sp + o + 4, y);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            for o in [0x220u32, 0x230, 0x240, 0x250] {
                fx.vu_apply_to(sp + o, sp + 0x1E0, sp + o);
            }
            let b = u32::from(fx.mem.read_u8(s4 + 8 + s6));
            let (c, r) = (b & 3, b >> 2);
            let (u0, u1) = ((c << 9) & 0x3FFF, ((32 * c + 32) << 4) & 0x3FFF);
            let (v0, v1) = ((r << 9) & 0x3FFF, ((32 * r + 32) << 4) & 0x3FFF);
            for (o, v) in [(0x328u32, u0), (0x32A, v0), (0x338, u1), (0x33A, v0), (0x348, u0), (0x34A, v1), (0x358, u1), (0x35A, v1)] {
                uv(fx, sp + o, v);
            }
            let v = s4 + 0x30 + 0x10 * s6;
            for (c, f) in [(0u32, f20), (4, f20), (8, f21)] {
                let x = ee::mul(fx.mem.f(v + c), f);
                fx.mem.setf(sp + 0x2A0 + c, x);
            }
            let start = fx.mem.read_u32(sp + 0xE0);
            fx.vu_add4(sp + 0x2A0, sp + 0x2A0, start);
            for (o, c) in [(0x260u32, 0x220u32), (0x270, 0x230), (0x280, 0x240), (0x290, 0x250)] {
                fx.vu_add4(sp + o, sp + c, sp + 0x2A0);
            }
            if crate::fxlib2::project(fx, sp + 0x260, PROJ_OUT, sp + 0x160, d, sp + 0x49C) {
                let w = fx.mem.f(sp + 0x49C);
                let key = fixu(fx, w, 0x01E0_10E8);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(s4 + 4) + 1;
        fx.mem.set32(s4 + 4, age);
        if age >= 0x81 {
            fx.mem.write_u8(s4, 0);
        }
    }
    fx.mem.write_u32(sp + 0x140, 0x80);
}

/// On 0x9D..0x121 [F 0x1E0115C]: record n % 12 of 12 sprites (cell 0..7,
/// size 0.5..1, a random direction 1.5 long).
fn spawn_sprites(fx: &mut Fx, n: i32) {
    let (_, e) = fx.div(n, 12);
    let rec = SPRITES + 0x110 * e as u32;
    fx.mem.write_u8(rec, 1);
    fx.mem.write_u32(rec + 4, 0);
    for s4 in 0..0xCu32 {
        let r = fx.rand_from(D, 0x01E0_11A8) >> 23;
        fx.mem.set8(rec + 8 + s4, rem2(r, 8));
        let r = fx.rand_from(D, 0x01E0_11C8) >> 23;
        let f = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(rem2(r, 256))), 256.0));
        fx.mem.setf(rec + 0x14 + 4 * s4, f);
        fx.random_table_vec(rec + 0x50 + 0x10 * s4, 1.5);
    }
}

/// The sprite records [F 0x1E01238..0x1E01570]: about the target point
/// D+0x30, spreading by 0.8 + 0.2 age / 12, alpha 100 cos(age x 30
/// degrees / 12 ...), key the mean w.
fn sprites(fx: &mut Fx, sp: u32) {
    let d = sp + 0x2B8;
    sprite_desc(fx, d);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, 0);
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    let id = fx.read_m4(sp + 0x1A0);
    fx.mul_into(Some(sp + 0x160), &id);
    let s5 = sp + 0x230;
    for k in 0..0xCu32 {
        fx.mem.write_u32(sp + 0x130, k);
        let s4 = SPRITES + 0x110 * k;
        if fx.mem.read_u8(s4) == 0 {
            continue;
        }
        let age = fx.mem.i32(s4 + 4);
        let x = age << 14;
        fx.mult(0x2AAA_AAAB, x);
        let a = (fx.hilo().1 >> 1) + ((x as u32) >> 31) as i32;
        let cs = fx.table_trig(a, true);
        let al = fixu(fx, ee::mul(100.0, cs), 0x01E0_1314);
        fx.mem.write_u8(sp + 0x2C8, 0x80);
        fx.mem.write_u8(sp + 0x2C9, 0x80);
        fx.mem.write_u8(sp + 0x2CA, 0xFF);
        fx.mem.write_u8(sp + 0x2CB, al as u8);
        fx.mem.write_u32(sp + 0x2CC, 0x3F80_0000);
        let f2 = ee::from_i32(fx.mem.i32(s4 + 4));
        let f1 = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), f2), 12.0);
        let f20 = ee::add(f32::from_bits(0x3F4C_CCCD), f1);
        for s6 in 0..0xCu32 {
            let f1 = ee::from_i32(fx.mem.i32(s4 + 4));
            let size = fx.mem.f(s4 + 0x14 + 4 * s6);
            let f1 = ee::div(ee::mul(0.5, f1), 12.0);
            let f1 = ee::mul(12.0, ee::add(0.5, f1));
            let s = ee::mul(f1, size);
            for (o, v) in [(0x220u32, s), (0x230, ee::neg(s))] {
                fx.mem.setf(sp + o, v);
                fx.mem.setf(sp + o + 4, v);
                fx.mem.write_u32(sp + o + 8, 0);
                fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
            }
            fx.vu_apply_to(sp + 0x220, sp + 0x1E0, sp + 0x220);
            fx.vu_apply_to(s5, sp + 0x1E0, s5);
            let b = u32::from(fx.mem.read_u8(s4 + 8 + s6));
            let (c, r) = (b & 3, b >> 2);
            uv(fx, sp + 0x2D0, (c << 9) & 0x3FFF);
            uv(fx, sp + 0x2D2, (r << 9) & 0x3FFF);
            uv(fx, sp + 0x2E0, ((32 * c + 32) << 4) & 0x3FFF);
            uv(fx, sp + 0x2E2, ((32 * r + 32) << 4) & 0x3FFF);
            fx.vu_scale4(sp + 0x2A0, s4 + 0x50 + 0x10 * s6, f20);
            fx.vu_add4(sp + 0x2A0, sp + 0x2A0, D + 0x30);
            fx.vu_add4(sp + 0x260, sp + 0x220, sp + 0x2A0);
            fx.vu_add4(sp + 0x270, s5, sp + 0x2A0);
            if crate::fxlib2::project(fx, sp + 0x260, PROJ_OUT, sp + 0x160, d, sp + 0x49C) {
                let w = fx.mem.f(sp + 0x49C);
                let key = fixu(fx, w, 0x01E0_150C);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(s4 + 4) + 1;
        fx.mem.set32(s4 + 4, age);
        if age >= 0xD {
            fx.mem.write_u8(s4, 0);
        }
    }
    fx.mem.write_u32(sp + 0x130, 0xC);
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x1E0);
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    let y = ee::add(fx.mem.f(D + 0x24), 2.0);
    fx.mem.setf(D + 0x24, y);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(D + 0x40, 1);
        fx.mem.write_u32(D + 0x44, 0);
    }
    if fx.mem.read_u8(D + 0x40) == 1 {
        lens(fx, sp);
    }
    let n = fx.mem.i32(D);
    if (0x43..0xBD).contains(&n) {
        spawn_quads(fx, n);
    }
    quads(fx, sp);
    let n = fx.mem.i32(D);
    if (0x9D..0x122).contains(&n) {
        spawn_sprites(fx, n);
    }
    sprites(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x92 {
        ret = 1;
    } else {
        if (0x9D..0x12E).contains(&n) {
            ret = 6;
        }
        if n == 0x12E {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x13E {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
