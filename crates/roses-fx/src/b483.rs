//! BMODULE b483 (card 589), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0x5C60
//! bytes) [F]: a single unscheduled function in
//! a 0x3F0 frame (s0 = D). Every update the camera turn with roll
//! (0x2055E0 into sp + 0x1E0) and the attacker moved (0x1F7110) along
//! the line from its point (D+0x20) to the target's (D+0x30) by
//! (1 - cos(32768 x ramp)) / 2, 50 up, plus its offset from its point
//! (D+0x40). From update 0xB4 for 0xBD updates a lens: a sprite (75, far
//! Z) at the point 15 above, then in a packet layer (TEXFLUSH, the frame
//! copied to TBP 0x2000 by 0x2095A0 / 0x207F50, TEXFLUSH) a disc of 32
//! rings x 16 quads (the centre ring triangles) on the camera plane whose
//! UVs are the screen positions (0x100C60) of a warped copy of the disc
//! (D+0x3A60: rings swaying by 0x457 / 0x83F x age) and positions those
//! of the plain disc (D+0x1860), far Z, unlinked; the layer at key 0. On
//! 0xDD..0x15B one of 12 records (+0x50, 0x200) of 20 sprites a update
//! rising about the point for 13 updates; then in a second layer the
//! attacker drawn into the offscreen target (0x1F8270) pasted full-window
//! at far Z. The unit's tint mode 1 / (0, 0, 0, 0x80) at the start, mode 0
//! / grey on 0x226. Returns 1 on update 0xD2, 6 on 0xDD..0x167, 3 on
//! 0x168, 8 once the count reaches 0x227.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// XYZ2 low 32 bits and the top byte kept: Z replaced.
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The lens' CLAMP (REGION_CLAMP 0..0x1FF x 0..0x1FF? as the code sets it).
const LENS_CLAMP: u64 = 0x0000_0800_0080_000A;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x3F0);
    fx.mem.write_u32(sp + 0xB0, 0);
    fx.mem.write_u32(sp + 0xC8, OT);
    let ret = if param >= 0 { init(fx, sp, param) } else { step(fx, sp) };
    fx.leave(0x3F0);
    ret
}

/// The plain disc D+0x1860: 32 rings (0x110) of 17 points, radius 37.5
/// (32 - ring) / 32, at the table sine / cosine of 0x1000 k.
fn init(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    fx.bzero(D, 0x5C60);
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
    let h = fx.mem.i32(D + 4);
    fx.unit_point(1, h, D + 0x20);
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x30);
    let h = fx.mem.i32(D + 4);
    fx.unit_position(h, sp + 0x260);
    fx.vu_sub4(D + 0x40, sp + 0x260, D + 0x20);
    for k in 0..0xCu32 {
        fx.mem.write_u8(D + 0x50 + 0x200 * k, 0);
    }
    fx.mem.write_u8(D + 0x1850, 0);
    for s5 in 0..0x20u32 {
        let f20 = ee::div(ee::mul(37.5, ee::from_i32(0x20 - s5 as i32)), 32.0);
        for s1 in 0..0x11u32 {
            let a = (0x10000 * s1 as i32) >> 4;
            let o = D + 0x110 * s5 + 0x10 * s1;
            let sn = fx.table_trig(a, false);
            fx.mem.setf(o + 0x1860, ee::mul(f20, sn));
            let cs = fx.table_trig(a, true);
            fx.mem.setf(o + 0x1864, ee::mul(f20, cs));
            fx.mem.write_u32(o + 0x1868, 0);
            fx.mem.write_u32(o + 0x186C, 0x3F80_0000);
        }
    }
    for (k, v) in [0u8, 0, 0, 0x80].into_iter().enumerate() {
        fx.mem.write_u8(sp + 0x3EC + k as u32, v);
    }
    let h = fx.mem.i32(D + 4);
    fx.unit_tint_mode(h, 1, sp + 0x3EC, [u64::from(D), param as u32 as u64], 0x01E0_022C);
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

fn copy16(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
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

/// The sprite descriptor at sp + 0x280 (5 registers 0x42421, prim 0x26 |
/// 0x800).
fn sprite_desc(fx: &mut Fx, sp: u32) {
    let d = sp + 0x280;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

fn alpha(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The lens [F 0x1E00380..0x1E00CE4] (D+0x1850 = 1).
fn lens(fx: &mut Fx, sp: u32, a23: [u32; 2]) -> [u32; 2] {
    let d1 = sp + 0x280;
    sprite_desc(fx, sp);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d1, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 0);
    uv(fx, sp + 0x298, 0);
    uv(fx, sp + 0x29A, 0);
    uv(fx, sp + 0x2A8, 0x200);
    uv(fx, sp + 0x2AA, 0x200);
    let s = 75.0f32;
    fx.mem.setf(sp + 0x220, s);
    fx.mem.setf(sp + 0x224, s);
    fx.mem.write_u32(sp + 0x228, 0);
    fx.mem.write_u32(sp + 0x22C, 0x3F80_0000);
    fx.mem.setf(sp + 0x230, ee::neg(s));
    fx.mem.setf(sp + 0x234, ee::neg(s));
    fx.mem.write_u32(sp + 0x238, 0);
    fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
    let age = ee::from_i32(fx.mem.i32(D + 0x1854));
    let f20 = Fx::ratio_below(age, 32.0);
    let age = ee::from_i32(fx.mem.i32(D + 0x1854));
    let f20 = ee::mul(f20, Fx::ramp_above(age, 188.0, 156.0));
    let a = alpha(fx, ee::mul(255.0, f20), 0x01E0_04A8);
    fx.mem.write_u8(sp + 0x290, 0x80);
    fx.mem.write_u8(sp + 0x291, 0x80);
    fx.mem.write_u8(sp + 0x292, 0x94);
    fx.mem.write_u8(sp + 0x293, a as u8);
    fx.mem.write_u32(sp + 0x294, 0x3F80_0000);
    fx.camera_vp_to(sp + 0x160);
    copy16(fx, sp + 0x260, sp + 0x270);
    let y = ee::add(fx.mem.f(sp + 0x264), 15.0);
    fx.mem.setf(sp + 0x264, y);
    fx.translate_rows(sp + 0x1A0, sp + 0x1E0, sp + 0x260);
    let m = fx.read_m4(sp + 0x1A0);
    fx.mul_into(Some(sp + 0x160), &m);
    let mut a23 = a23;
    if crate::fxlib2::project(fx, sp + 0x220, PROJ_OUT, sp + 0x160, d1, sp + 0x3E8) {
        far_z(fx, sp + 0x2A0, &[sp + 0x2B0]);
        fx.submit(d1, OT, 0);
        a23 = [0, KEEP as u32];
    }
    // The lens descriptors: triangles sp + 0x2D8 (prim 0x33), quads
    // sp + 0x350 (prim 0x34), each vertex RGBAQ / UV / XYZ2.
    let (d2, d3) = (sp + 0x2D8, sp + 0x350);
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
    // a0..a3 as 0x207BB0 leaves them: the new packet pointer twice, the
    // old one, 0x8000 [run].
    let p1 = fx.mem.read_u32(crate::world::PACKET_POINTER);
    let dest = fx.copy_dest([p1, p1, p0, 0x8000]);
    let regs = fx.frame_texture_packet(dest, 1, sp + 0x338, sp + 0x340);
    let v = fx.mem.read_u64(sp + 0x338);
    fx.mem.write_u64(sp + 0x3C8, v);
    let v = fx.mem.read_u64(sp + 0x340);
    fx.mem.write_u64(sp + 0x3D0, v);
    // a0 = sp + 0x3C8 (set for the copy of TEX0).
    fx.texflush([sp + 0x3C8, regs[1], regs[2], regs[3]]);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d2, a);
    fx.desc_clamp(d2, LENS_CLAMP);
    let f = fx.mem.read_u16(d2 + 2);
    fx.mem.write_u16(d2 + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d3, a);
    fx.desc_clamp(d3, LENS_CLAMP);
    let f = fx.mem.read_u16(d3 + 2);
    fx.mem.write_u16(d3 + 2, f | 0x800);
    let a = alpha(fx, ee::mul(128.0, f20), 0x01E0_06E4);
    fx.mem.write_u8(sp + 0x2E8, 0x80);
    fx.mem.write_u8(sp + 0x2E9, 0x80);
    fx.mem.write_u8(sp + 0x2EA, 0xA4);
    fx.mem.write_u8(sp + 0x2EB, a as u8);
    fx.mem.write_u32(sp + 0x2EC, 0x3F80_0000);
    let two = |fx: &mut Fx, to: u32, from: u32| {
        let a = fx.mem.read_u32(from);
        let b = fx.mem.read_u32(from + 4);
        fx.mem.write_u32(to, a);
        fx.mem.write_u32(to + 4, b);
    };
    two(fx, sp + 0x300, sp + 0x2E8);
    two(fx, sp + 0x318, sp + 0x2E8);
    two(fx, sp + 0x3E0, sp + 0x2E8);
    for o in [0x360u32, 0x378, 0x390, 0x3A8] {
        two(fx, sp + o, sp + 0x3E0);
    }
    warp(fx, sp);
    fx.mem.write_u32(sp + 0x260, 0);
    fx.mem.write_u32(sp + 0x264, 0);
    fx.mem.write_u32(sp + 0x268, 0);
    fx.mem.write_u32(sp + 0x26C, 0x3F80_0000);
    for k in 0..0x20u32 {
        fx.mem.write_u32(sp + 0x150, k);
        let fp = D + 0x110 * k;
        if k == 0x1F {
            for s1 in 0..0x10u32 {
                let (s2, s4) = (fp + 0x10 * s1, fp + 0x10 * (s1 + 1));
                let r = fx.project_n(&[sp + 0x260, s2 + 0x3A60, s4 + 0x3A60], sp + 0x160, d2, sp + 0x3E8, 0x01E0_0988);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x2F0u32, 0x2F8u32), (0x308, 0x310), (0x320, 0x328)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let r = fx.project_n(&[sp + 0x260, s2 + 0x1860, s4 + 0x1860], sp + 0x160, d2, sp + 0x3E8, 0x01E0_09EC);
                if r == 1 {
                    far_z(fx, sp + 0x2F8, &[sp + 0x310, sp + 0x328]);
                    fx.submit(d2, 0, 0);
                }
            }
        } else {
            let next = D + 0x110 * (k + 1);
            fx.mem.write_u32(sp + 0x100, next);
            for s6 in 0..0x10u32 {
                let (s2, s5) = (fp + 0x10 * s6, fp + 0x10 * (s6 + 1));
                let (s1, s4) = (next + 0x10 * s6, next + 0x10 * (s6 + 1));
                let r = fx.project_n(&[s2 + 0x3A60, s5 + 0x3A60, s1 + 0x3A60, s4 + 0x3A60], sp + 0x160, d3, sp + 0x3E8, 0x01E0_0B1C);
                if r != 1 {
                    continue;
                }
                for (u, x) in [(0x368u32, 0x370u32), (0x380, 0x388), (0x398, 0x3A0), (0x3B0, 0x3B8)] {
                    fx.uv_from_xy(sp + u, sp + x, 0, 0);
                }
                let r = fx.project_n(&[s2 + 0x1860, s5 + 0x1860, s1 + 0x1860, s4 + 0x1860], sp + 0x160, d3, sp + 0x3E8, 0x01E0_0B98);
                if r == 1 {
                    far_z(fx, sp + 0x370, &[sp + 0x388, sp + 0x3A0, sp + 0x3B8]);
                    fx.submit(d3, 0, 0);
                }
            }
        }
    }
    fx.mem.write_u32(sp + 0x150, 0x20);
    let layer = fx.mem.read_u32(sp + 0xCC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(D + 0x1854) + 1;
    fx.mem.set32(D + 0x1854, age);
    if age >= 0xBD {
        fx.mem.write_u8(D + 0x1850, 0);
    }
    a23
}

/// The warped disc D+0x3A60 [F 0x1E007A4]: ring fp's point s5 at radius
/// 37.5 (32 - fp) / 32 x (1 + (fp + 1) / 32 x 0.2 cos(...)), angle 16
/// (0x1000 s5 ... ) + (fp + 1) / 32 x 910 cos(...).
fn warp(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0xD0, D);
    fx.mem.write_u32(sp + 0xE0, 0);
    fx.mem.write_u32(sp + 0xF0, 0);
    for fp in 0..0x20i32 {
        let s3 = fx.mem.read_u32(sp + 0xD0);
        let f20 = ee::div(ee::mul(37.5, ee::from_i32(0x20 - fp)), 32.0);
        let s7 = fx.mem.i32(sp + 0xE0) >> 5;
        let s4 = fx.mem.i32(sp + 0xF0) >> 5;
        let f21 = ee::div(ee::from_i32(fp + 1), 32.0);
        let (mut s1, mut s2) = (0i32, 0i32);
        for s5 in 0..0x11u32 {
            let age = fx.mem.i32(D + 0x1854);
            let v = s7.wrapping_add(s1 >> 4);
            let a = age.wrapping_mul(0x83F).wrapping_add(v) & 0xFFFF;
            let c = fx.table_trig(a, true);
            let f1 = ee::mul(f21, ee::mul(f32::from_bits(0x3E4C_CCCD), c));
            let age = fx.mem.i32(D + 0x1854);
            let m = fx.mult(age, 0x457);
            let f22 = ee::add(1.0, f1);
            let c2 = fx.table_trig(s4.wrapping_add(m) & 0xFFFF, true);
            let f1 = ee::mul(f21, ee::mul(910.0, c2));
            let x = ee::add(ee::from_i32(s2 >> 4), f1);
            fx.fix_residue(x, 0x01E0_0898);
            let s6 = ee::fixsfsi(x) & 0xFFFF;
            let sn = fx.table_trig(s6, false);
            let f22 = ee::mul(f20, f22);
            let o = s3 + 0x10 * s5;
            fx.mem.setf(o + 0x3A60, ee::mul(f22, sn));
            let cs = fx.table_trig(s6, true);
            fx.mem.setf(o + 0x3A64, ee::mul(f22, cs));
            fx.mem.write_u32(o + 0x3A68, 0);
            fx.mem.write_u32(o + 0x3A6C, 0x3F80_0000);
            s1 = s1.wrapping_add(0x20000);
            s2 = s2.wrapping_add(0x10000);
        }
        let e = fx.mem.read_u32(sp + 0xE0).wrapping_add(0x20000);
        fx.mem.write_u32(sp + 0xE0, e);
        let f = fx.mem.read_u32(sp + 0xF0).wrapping_add(0x10000);
        fx.mem.write_u32(sp + 0xF0, f);
        fx.mem.write_u32(sp + 0xD0, s3 + 0x110);
    }
}

/// On 0xDD..0x15B [F 0x1E00D00]: record n % 12 of 20 sprites.
fn spawn(fx: &mut Fx, n: i32) {
    let (_, e) = fx.div(n, 12);
    let rec = D.wrapping_add((e as u32) << 9);
    fx.mem.write_u8(rec + 0x50, 1);
    fx.mem.write_u32(rec + 0x54, 0);
    for s4 in 0..0x14u32 {
        let r = fx.rand_from(D, 0x01E0_0D38) >> 23;
        let (_, m) = fx.div(r, 6);
        fx.mem.set8(rec + s4 + 0x58, m);
        let r = fx.rand_from(D, 0x01E0_0D58) >> 23;
        fx.mem.set32(rec + 4 * s4 + 0x6C, (rem2(r, 256) << 16) >> 8);
        let r = fx.rand_from(D, 0x01E0_0D80) >> 23;
        let f = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), ee::from_i32(rem2(r, 256))), 256.0));
        fx.mem.setf(rec + 4 * s4 + 0xBC, f);
        let v = rec + 0x10 * s4 + 0x110;
        fx.random_table_vec(v, f32::from_bits(0x3F33_3333));
        let y = ee::mul(fx.mem.f(v + 4), f32::from_bits(0x3F66_6666));
        fx.mem.setf(v + 4, y);
    }
}

/// The records' sprites [F 0x1E00E18].
fn sprites(fx: &mut Fx, sp: u32) {
    let d1 = sp + 0x280;
    sprite_desc(fx, sp);
    fx.mem.write_u32(sp + 0x110, d1 + 2);
    fx.mem.write_u32(sp + 0x120, d1 + 8);
    fx.mem.write_u32(sp + 0x130, d1 + 4);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d1, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d1, rh, 1);
    fx.desc_clamp(d1, 0);
    fx.camera_vp_to(sp + 0x160);
    fx.identity(sp + 0x1A0);
    let id = fx.read_m4(sp + 0x1A0);
    fx.mul_into(Some(sp + 0x160), &id);
    let f21 = fx.mem.f(sp + 0x278);
    let fp = sp + 0x230;
    for k in 0..0xCu32 {
        fx.mem.write_u32(sp + 0x140, k);
        let s4 = D + 0x200 * k;
        if fx.mem.read_u8(s4 + 0x50) == 0 {
            continue;
        }
        let age = fx.mem.i32(s4 + 0x54);
        let x = age << 15;
        fx.mult(0x2AAA_AAAB, x);
        let a = (fx.hilo().1 >> 1) + ((x as u32) >> 31) as i32;
        let sn = fx.table_trig(a, false);
        let al = alpha(fx, ee::mul(200.0, sn), 0x01E0_0F18);
        for o in 0..3u32 {
            fx.mem.write_u8(sp + 0x290 + o, 0x80);
        }
        fx.mem.write_u8(sp + 0x293, al as u8);
        fx.mem.write_u32(sp + 0x294, 0x3F80_0000);
        let age = fx.mem.i32(s4 + 0x54);
        let f20 = ee::mul(5.0, ee::div(ee::from_i32(age), 12.0));
        for s6 in 0..0x14u32 {
            let s = ee::mul(3.0, fx.mem.f(s4 + 4 * s6 + 0xBC));
            fx.mem.setf(sp + 0x220, s);
            fx.mem.setf(sp + 0x224, s);
            fx.mem.write_u32(sp + 0x228, 0);
            fx.mem.write_u32(sp + 0x22C, 0x3F80_0000);
            fx.mem.setf(fp, ee::neg(s));
            fx.mem.setf(sp + 0x234, ee::neg(s));
            fx.mem.write_u32(sp + 0x238, 0);
            fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
            fx.vu_apply_to(sp + 0x220, sp + 0x1E0, sp + 0x220);
            fx.vu_apply_to(fp, sp + 0x1E0, fp);
            let age = fx.mem.i32(s4 + 0x54);
            let v = fx.mem.i32(s4 + 4 * s6 + 0x6C).wrapping_add(age.wrapping_mul(222));
            let sn = fx.table_trig(v & 0xFFFF, false);
            let b = fx.mem.read_u8(s4 + s6 + 0x58);
            let (c, r) = (u32::from(b & 3), u32::from(b >> 2));
            uv(fx, sp + 0x298, (c << 9) & 0x3FFF);
            uv(fx, sp + 0x29A, (r << 9) & 0x3FFF);
            uv(fx, sp + 0x2A8, ((32 * c + 32) << 4) & 0x3FFF);
            uv(fx, sp + 0x2AA, ((32 * r + 32) << 4) & 0x3FFF);
            let v = s4 + 0x10 * s6 + 0x110;
            fx.vu_scale4(sp + 0x260, v, sn);
            let y0 = fx.mem.f(v + 4);
            fx.mem.setf(sp + 0x264, y0);
            let x = ee::add(fx.mem.f(sp + 0x260), fx.mem.f(sp + 0x270));
            fx.mem.setf(sp + 0x260, x);
            let f1 = ee::add(ee::add(10.0, fx.mem.f(D + 0x24)), f20);
            let y = ee::add(fx.mem.f(sp + 0x264), f1);
            fx.mem.setf(sp + 0x264, y);
            let z = ee::add(fx.mem.f(sp + 0x268), f21);
            fx.mem.setf(sp + 0x268, z);
            fx.vu_add4(sp + 0x240, sp + 0x220, sp + 0x260);
            fx.vu_add4(sp + 0x250, fp, sp + 0x260);
            if crate::fxlib2::project(fx, sp + 0x240, PROJ_OUT, sp + 0x160, d1, sp + 0x3E8) {
                far_z(fx, sp + 0x2A0, &[sp + 0x2B0]);
                fx.submit(d1, OT, 0);
            }
        }
        let age = fx.mem.i32(s4 + 0x54) + 1;
        fx.mem.set32(s4 + 0x54, age);
        if age >= 0xD {
            fx.mem.write_u8(s4 + 0x50, 0);
        }
    }
    fx.mem.write_u32(sp + 0x140, 0xC);
}

/// The attacker pasted full-window from the offscreen target [F
/// 0x1E01204].
fn paste(fx: &mut Fx, sp: u32) -> u32 {
    let d1 = sp + 0x280;
    fx.mem.write_u16(d1 + 2, 0x26);
    fx.mem.write_u16(d1, 5);
    fx.mem.write_u64(d1 + 8, 0x4_2421);
    fx.mem.write_u32(d1 + 4, 0);
    let f = fx.mem.read_u16(d1 + 2);
    fx.mem.write_u16(d1 + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d1, a);
    fx.desc_clamp(d1, LENS_CLAMP);
    let layer = fx.layer_begin();
    // a0..a3 as 0x208400 leaves them: the flags | 0x4000, the clamp's low
    // word, the descriptor + 0x10, the flags' address.
    let flags = u32::from(fx.mem.read_u16(d1 + 2));
    fx.texflush([flags, LENS_CLAMP as u32, d1 + 0x10, d1 + 2]);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 0, sp + 0x2C0, sp + 0x2C8);
    fx.texflush(regs);
    for o in 0..4u32 {
        fx.mem.write_u8(sp + 0x290 + o, 0x80);
    }
    fx.mem.write_u32(sp + 0x294, 0x3F80_0000);
    uv(fx, sp + 0x298, 0);
    uv(fx, sp + 0x29A, 0);
    uv(fx, sp + 0x2A8, 0x2000);
    uv(fx, sp + 0x2AA, 0x2000);
    for (o, xy) in [(0x2A0u32, 0x7000u16), (0x2B0, 0x9000)] {
        fx.mem.write_u16(sp + o, xy);
        fx.mem.write_u16(sp + o + 2, xy);
        let v = fx.mem.read_u64(sp + o);
        fx.mem.write_u64(sp + o, (v & KEEP) | FAR_Z);
        fx.mem.write_u8(sp + o + 7, 0);
    }
    fx.submit(d1, 0, 0);
    fx.layer_end(layer, OT, 0);
    layer
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x1E0);
    let n = fx.mem.i32(D);
    let v = Fx::clamp_i(n - 0x20, 0, 0x76);
    let f20 = ee::div(ee::from_i32(v), 118.0);
    let n = fx.mem.i32(D);
    let v = Fx::clamp_i(n - 0x1A2, 0, 0x84);
    let f0 = ee::sub(1.0, ee::div(ee::from_i32(v), 132.0));
    let x = ee::mul(32768.0, ee::mul(f20, f0));
    fx.fix_residue(x, 0x01E0_02BC);
    let a = ee::fixsfsi(x) & 0xFFFF;
    let c = fx.table_trig(a, true);
    let t = ee::mul(-0.5, ee::sub(c, 1.0));
    let (ax, bx) = (fx.mem.f(D + 0x20), fx.mem.f(D + 0x30));
    fx.mem.setf(sp + 0x260, ee::add(ax, ee::mul(t, ee::sub(bx, ax))));
    let (az, bz) = (fx.mem.f(D + 0x28), fx.mem.f(D + 0x38));
    fx.mem.setf(sp + 0x268, ee::add(az, ee::mul(t, ee::sub(bz, az))));
    fx.mem.write_u32(sp + 0x264, 0x4248_0000);
    copy16(fx, sp + 0x270, sp + 0x260);
    fx.vu_add4(sp + 0x260, sp + 0x260, D + 0x40);
    fx.mem.write_u32(sp + 0x264, 0x4248_0000);
    let h = fx.mem.i32(D + 4);
    fx.unit_set_position(h, sp + 0x260, u64::from(D), 0x01E0_0354);
    if fx.mem.i32(D) == 0xB4 {
        fx.mem.write_u8(D + 0x1850, 1);
        fx.mem.write_u32(D + 0x1854, 0);
    }
    if fx.mem.read_u8(D + 0x1850) == 1 {
        lens(fx, sp, [0, 0]);
    }
    let n = fx.mem.i32(D);
    if (0xDD..0x15C).contains(&n) {
        spawn(fx, n);
    }
    sprites(fx, sp);
    let layer = paste(fx, sp);
    if fx.mem.i32(D) == 0x226 {
        let h = fx.mem.i32(D + 4);
        fx.unit_tint_mode(h, 0, 0, [u64::from(D), u64::from(layer)], 0x01E0_13DC);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xD2 {
        ret = 1;
    } else if (0xDD..0x168).contains(&n) {
        ret = 6;
    } else if n == 0x168 {
        ret = 3;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x227 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
