//! BMODULE b471 (card 367), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept in
//! s3, 0x3A40 bytes) [F]: a single unscheduled
//! function in a 0x470 frame on b150's helpers (s0 = the OT, the return
//! at sp + 0xB0): window sprites (descriptor sp + 0x320: 0x42421, prim
//! 0x26 | 0x800, clamp 5), lens triangles (sp + 0x378: prim 0x33, 9
//! registers, the lens CLAMP, the frame copied in a packet layer) and a
//! warped grid of the attacker's own picture (sp + 0x3F0: prim 0x24 |
//! 0x800, 9 registers RGBAQ + 4 x UV / XYZ2, 0x1F8270 in a packet layer).
//! The init: 32 spark records' cells, 7 points of radius 10 (D+0x14B0) and
//! a 17 x 17 grid of texels (D+0x2828). Every update the camera turn with
//! roll (sp + 0x240) and the attacker's nodes 0xD / 0x30 / 0x29 / 0x1B /
//! 0x22 / 0x14 (D+0x30.., the midpoint of 0xD and 0x1B at D+0x90). From
//! update 0 for 228 updates a flash sprite at each node (size 10 (1 - 0.3
//! parity) min(age, 32) / 32, (FF, 80, C4)); on 0x2E..0xC3 one of 32 spark
//! records (+0xB0, 0xA0) of 8 table directions round the midpoint, 32
//! updates (size 5 + 5 (32 - age) / 32, eight spinning points (30, -150
//! side) pulled in by (32 - age) / 32, alpha count x 3 age >> 5, (80, 40,
//! 60), UV table 0x28AE70); on 0x4C..0xC3 one of 32 lens records (+0x1520,
//! 0x70) of six 2.0 table points: the radius-10 fan scaled by the tan
//! falloff turned 999 age, each point's triangles textured by the frame
//! shifted by half their screen size along the turn; on 0x4C..0xD3 one of
//! 16 bursts (+0x2320, 0x50) of four 4.0 table points drawn 16 times
//! (sprite 6, (FF, 80, 80), alpha age 128 sin(k / 17) / 16), and from
//! update 0 for 80 updates the grid (16 x 16 quads at the window centre
//! wobbling by 10 sin / cos, alpha 96 sin((age << 15) / 80), (80, 80,
//! FF)). Returns 1 on update 0x41, 6 on 0x4C..0xE3, 2 on 0xE4, 8 once the
//! count reaches 0xEF.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
/// The UV table the sparks read (4 halfwords an entry).
const UV_TABLE: u32 = 0x0028_AE70;
/// 0x101080's direction table (4 x 32 vec3).
const DIRS: u32 = 0x0028_AE90;
const FRAME: u32 = 0x470;
const DESC: u32 = 0x320;
const TRI: u32 = 0x378;
const GRID: u32 = 0x3F0;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const FLASH: u32 = D + 0xA0;
const SPARKS: u32 = D + 0xB0;
const FAN: u32 = D + 0x14B0;
const LENSES: u32 = D + 0x1520;
const BURSTS: u32 = D + 0x2320;
const WARP: u32 = D + 0x2820;
const TEXELS: u32 = D + 0x2828;
const POINTS: u32 = D + 0x3130;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
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

/// x / d by the compiler's magic multiply (HI >> shift + the sign bit).
fn mdiv(fx: &mut Fx, x: i32, magic: i32, shift: u32) -> i32 {
    fx.mult(magic, x);
    let hi = fx.hilo().1;
    (hi >> shift).wrapping_add(((x as u32) >> 31) as i32)
}

fn words2(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..2u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0x3A40);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 8, s);
    for k in 0..2u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0xC + 4 * k, r);
    }
    for (k, (node, clear)) in [(0xD, 1), (0x30, 0), (0x29, 0), (0x1B, 0), (0x22, 0), (0x14, 0)].into_iter().enumerate() {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + 0x14 + 4 * k as u32, r);
    }
    fx.mem.write_u8(FLASH, 0);
    fx.mem.write_u32(D + 0xA8, 0);
    for k in 0..0x20u32 {
        let r = SPARKS + 0xA0 * k;
        fx.mem.write_u8(r, 0);
        for j in 0..8u32 {
            let v = fx.rand_from(D + 0xA0 * k, 0x01E0_01B0) >> 23;
            fx.mem.write_u16(r + 8 + 2 * j, rem2(v, 4) as u16);
        }
    }
    for k in 0..7u32 {
        let a = mdiv(fx, (k << 16) as i32, 0x2AAA_AAAB, 0);
        let p = FAN + 0x10 * k;
        let s = fx.tsin(a);
        fx.mem.setf(p, ee::mul(10.0, s));
        let c = fx.tcos(a);
        fx.mem.setf(p + 4, ee::mul(10.0, c));
        fx.mem.write_u32(p + 8, 0);
        fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(LENSES + 0x70 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(BURSTS + 0x50 * k, 0);
    }
    fx.mem.write_u8(WARP, 0);
    for row in 0..0x11u32 {
        let u = ((0x19A * row) as i32 >> 4) + 0x33;
        for col in 0..0x11u32 {
            let p = TEXELS + 0x88 * row + 8 * col;
            fx.mem.set32(p, u);
            fx.mem.set32(p + 4, ((0x19A * col) as i32 >> 4) + 0x33);
        }
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x338, u0);
    uv(fx, sp + 0x33A, v0);
    uv(fx, sp + 0x348, u1);
    uv(fx, sp + 0x34A, v1);
}

/// The table's UVs of entry `i` (u0 h0, u1 h2, v0 h4, v1 h6).
fn table_uvs(fx: &mut Fx, sp: u32, i: u16) {
    let t = UV_TABLE + 8 * u32::from(i);
    let (u0, u1, v0, v1) = (fx.mem.read_u16(t), fx.mem.read_u16(t + 2), fx.mem.read_u16(t + 4), fx.mem.read_u16(t + 6));
    uvs(fx, sp, u32::from(u0), u32::from(v0), u32::from(u1), u32::from(v1));
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// 0x1A2B80: a signed integer from the float (with its residue frame).
fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

/// The view-projection (x identity) at sp + 0x1C0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x1C0);
    fx.identity(sp + 0x200);
    let id = fx.read_m4(sp + 0x200);
    fx.mul_into(Some(sp + 0x1C0), &id);
}

/// The sprite descriptor rewritten (through its pointers), | 0x800, ALPHA
/// preset 1, the texture D+`tex`, clamp 5, and the camera.
fn sprite_desc(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
}

/// The corners (-s, -s, 0, 1) at sp + 0x280 and (s, s, 0, 1) at sp +
/// 0x290 turned by the camera (sp + 0x240).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x280, n);
    fx.mem.setf(sp + 0x284, n);
    fx.mem.write_u32(sp + 0x288, 0);
    fx.mem.write_u32(sp + 0x28C, 0x3F80_0000);
    fx.mem.setf(sp + 0x290, s);
    fx.mem.setf(sp + 0x294, s);
    fx.mem.write_u32(sp + 0x298, 0);
    fx.mem.write_u32(sp + 0x29C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x280, sp + 0x240, sp + 0x280);
    fx.vu_apply_to(sp + 0x290, sp + 0x240, sp + 0x290);
}

/// The sprite's colour (c, a, Q = 1.0) at sp + 0x330.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x330, c[0]);
    fx.mem.write_u8(sp + 0x331, c[1]);
    fx.mem.write_u8(sp + 0x332, c[2]);
    fx.mem.write_u8(sp + 0x333, a);
    fx.mem.write_u32(sp + 0x334, 0x3F80_0000);
}

/// The sprite at sp + 0x2A0 / 0x2B0 = the corners + `at`, projected and
/// submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, at: u32, ra: u32) {
    fx.vu_add4(sp + 0x2A0, sp + 0x280, at);
    fx.vu_add4(sp + 0x2B0, sp + 0x290, at);
    if crate::fxlib2::project(fx, sp + 0x2A0, PROJ_OUT, sp + 0x1C0, sp + DESC, sp + 0x46C) {
        let w = fx.mem.f(sp + 0x46C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// `p` = 2 x (the table directions of three rand picks at column `s`) x
/// `f`, w 1, then + the midpoint (rand's frames from `ras`).
fn table_point(fx: &mut Fx, p: u32, s: u32, f: f32, ras: [u32; 3]) {
    for (c, ra) in [0u32, 4, 8].into_iter().zip(ras) {
        let q = rem2(fx.rand_from(OT, ra) >> 23, 4) as u32;
        let e = DIRS + 0x180 * q + 0xC * s;
        let v = fx.mem.f(e + c);
        fx.mem.setf(p + c, if f == 1.0 { v } else { ee::mul(f, v) });
    }
    fx.mem.write_u32(p + 0xC, 0x3F80_0000);
    fx.vu_add4(p, p, D + 0x90);
}

/// The flash at the nodes [F 0x1E00418].
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.read_u8(FLASH) != 1 {
        return;
    }
    let age = fx.mem.i32(FLASH + 4);
    let v = (0xE4 - age).min(0x20);
    colour(fx, sp, [0xFF, 0x80, 0xC4], ((v * 255) >> 5) as u8);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    let s6 = fx.mem.i32(FLASH + 4).min(0x20);
    for j in 0..6u32 {
        let age = fx.mem.i32(FLASH + 4);
        let par = ee::from_i32(rem2(age + j as i32, 2));
        let f3 = ee::sub(1.0, ee::mul(f32::from_bits(0x3E99_999A), par));
        let f1 = ee::mul(ee::from_i32(s6), ee::mul(10.0, f3));
        let f20 = ee::div(f1, 32.0);
        corners(fx, sp, f20);
        draw(fx, sp, D + 0x30 + 0x10 * j, 0x01E0_05E8);
    }
    let age = fx.mem.i32(FLASH + 4) + 1;
    fx.mem.set32(FLASH + 4, age);
    if age >= 0xE5 {
        fx.mem.write_u8(FLASH, 0);
    }
}

/// The spark records [F 0x1E00628 / 0x1E007D4].
fn sparks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x2E..0xC4).contains(&n) {
        let r = SPARKS + 0xA0 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..8u32 {
            let v = rem2(fx.rand_from(OT, 0x01E0_0680) >> 23, 256);
            let n = fx.mem.i32(D);
            let s = rem2(n + j as i32 + v, 0x20) as u32;
            table_point(fx, r + 0x20 + 0x10 * j, s, 1.0, [0x01E0_06C0, 0x01E0_070C, 0x01E0_074C]);
        }
        let c = fx.mem.i32(D + 0xA8) + 1;
        fx.mem.set32(D + 0xA8, c);
        if fx.mem.i32(D + 0xA8) >= 0x21 {
            fx.mem.set32(D + 0xA8, 0x20);
        }
    }
    sprite_desc(fx, sp, 0x10);
    fx.mem.write_u32(sp + 0x170, sp + 0x33A);
    fx.mem.write_u32(sp + 0x180, sp + 0x34A);
    for k in 0..0x20u32 {
        let r = SPARKS + 0xA0 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 4);
        let f1 = ee::mul(5.0, ee::from_i32(0x20 - age));
        let f20 = ee::add(5.0, ee::div(f1, 32.0));
        corners(fx, sp, f20);
        let age = fx.mem.i32(r + 4);
        let count = fx.mem.i32(D + 0xA8);
        let three = ((((3 * age) as i64) << 37) >> 37) as i32;
        let a = fx.mult(count, three) >> 5;
        colour(fx, sp, [0x80, 0x40, 0x60], a as u8);
        let f20 = ee::div(ee::from_i32(0x20 - fx.mem.i32(r + 4)), 32.0);
        fx.identity(sp + 0x2C0);
        let age = fx.mem.i32(r + 4);
        let a = fx.mult(age, 0x3E7);
        fx.rotate_axis(sp + 0x2C0, a, 2);
        for j in 0..8u32 {
            let a = ((0x10000 * j) as i32) >> 3;
            let s = fx.tsin(a);
            fx.mem.setf(sp + 0x300, ee::mul(30.0, s));
            let c = fx.tcos(a);
            fx.mem.setf(sp + 0x304, ee::mul(30.0, c));
            let side = ee::from_i32(fx.mem.i32(D + 8));
            fx.mem.setf(sp + 0x308, ee::mul(-150.0, side));
            fx.mem.write_u32(sp + 0x30C, 0x3F80_0000);
            fx.vu_apply_to(sp + 0x300, sp + 0x2C0, sp + 0x300);
            fx.vu_add4(sp + 0x300, sp + 0x300, r + 0x20 + 0x10 * j);
            fx.vu_sub4(sp + 0x300, sp + 0x300, D + 0x90);
            fx.vu_scale4(sp + 0x300, sp + 0x300, f20);
            fx.vu_add4(sp + 0x300, sp + 0x300, D + 0x90);
            let b = fx.mem.read_u16(r + 8 + 2 * j);
            table_uvs(fx, sp, b);
            draw(fx, sp, sp + 0x300, 0x01E0_0B80);
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The lens records [F 0x1E00BE0 / 0x1E00D98].
fn lenses(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x4C..0xC4).contains(&n) {
        let r = LENSES + 0x70 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..6u32 {
            let v = rem2(fx.rand_from(OT, 0x01E0_0C38) >> 23, 256);
            let n = fx.mem.i32(D);
            let s = rem2(n + j as i32 + v, 0x20) as u32;
            table_point(fx, r + 0x10 + 0x10 * j, s, 2.0, [0x01E0_0C78, 0x01E0_0CD4, 0x01E0_0D24]);
        }
    }
    let d = sp + TRI;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    fx.desc_clamp(d, crate::lens::LENS_CLAMP);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    for (o, a) in [(0x388u32, 0x80u8), (0x3A0, 0)] {
        fx.mem.write_u8(sp + o, 0x80);
        fx.mem.write_u8(sp + o + 1, 0x80);
        fx.mem.write_u8(sp + o + 2, 0xFF);
        fx.mem.write_u8(sp + o + 3, a);
        fx.mem.write_u32(sp + o + 4, 0x3F80_0000);
    }
    words2(fx, sp + 0x3B8, sp + 0x3A0);
    fx.mem.write_u32(sp + 0xD0, 0);
    fx.mem.write_u32(sp + 0x1A0, 0);
    fx.mem.write_u32(sp + 0x100, D);
    let mut layer = 0;
    for k in 0..0x20u32 {
        let r = LENSES + 0x70 * k;
        if fx.mem.read_u8(r) != 0 {
            if fx.mem.read_u32(sp + 0xD0) == 0 {
                fx.mem.write_u32(sp + 0xD0, 1);
                layer = fx.layer_begin();
                fx.mem.write_u32(sp + 0xCC, layer);
                // a0..a3 at the call: sp + 0x2C0 (the identity's, set in a
                // delay slot), sp + 0x3A0 (the colours'), and 0x208400's
                // a2 / a3 (d + 0x10, d + 2).
                let p0 = fx.mem.read_u32(crate::world::PACKET_POINTER);
                fx.texflush([sp + 0x2C0, sp + 0x3A0, d + 0x10, d + 2]);
                let p1 = fx.mem.read_u32(crate::world::PACKET_POINTER);
                let dest = fx.copy_dest([p1, p1, p0, 0x8000]);
                let regs = fx.frame_texture_packet(dest, 1, sp + 0x3D8, sp + 0x3E0);
                fx.texflush(regs);
            }
            lens(fx, sp, r);
        }
        fx.mem.write_u32(sp + 0x100, D + 0x70 * (k + 1));
        fx.mem.write_u32(sp + 0x1A0, k + 1);
    }
    if fx.mem.read_u32(sp + 0xD0) == 1 {
        let layer = if layer != 0 { layer } else { fx.mem.read_u32(sp + 0xCC) };
        fx.layer_end(layer, OT, 0);
    }
}

/// One lens record's six fans [F 0x1E00EC4..0x1E011D8].
fn lens(fx: &mut Fx, sp: u32, r: u32) {
    let d = sp + TRI;
    fx.identity(sp + 0x2C0);
    let age = fx.mem.i32(r + 4);
    let a = fx.mult(age, 0x3E7);
    fx.rotate_axis(sp + 0x2C0, a, 2);
    let age = fx.mem.i32(r + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 2);
    for o in [0x310u32, 0x314, 0x318] {
        fx.mem.setf(sp + o, f0);
    }
    fx.mem.write_u32(sp + 0x31C, 0x3F80_0000);
    fx.mem.write_u32(sp + 0x1B0, 0);
    fx.mem.write_u32(sp + 0xE0, 0);
    let f21 = ee::div(ee::from_i32(0x20 - fx.mem.i32(r + 4)), 32.0);
    fx.mem.write_u32(sp + 0xF0, r - 0x1520);
    let mut f20 = 0.0f32;
    for j in 0..6u32 {
        let s5 = mdiv(fx, (0x10000 * j) as i32, 0x2AAA_AAAB, 0);
        let s = fx.tsin(s5);
        fx.mem.setf(sp + 0x300, ee::mul(80.0, s));
        let c = fx.tcos(s5);
        fx.mem.setf(sp + 0x304, ee::mul(80.0, c));
        let side = ee::from_i32(fx.mem.i32(D + 8));
        fx.mem.setf(sp + 0x308, ee::mul(-250.0, side));
        fx.mem.write_u32(sp + 0x30C, 0x3F80_0000);
        fx.vu_apply_to(sp + 0x300, sp + 0x2C0, sp + 0x300);
        fx.vu_add4(sp + 0x300, sp + 0x300, r + 0x10 + 0x10 * j);
        fx.vu_sub4(sp + 0x300, sp + 0x300, D + 0x90);
        fx.vu_scale4(sp + 0x300, sp + 0x300, f21);
        fx.vu_add4(sp + 0x300, sp + 0x300, D + 0x90);
        fx.camera_vp_to(sp + 0x1C0);
        fx.translate_rows(sp + 0x200, sp + 0x240, sp + 0x300);
        fx.scale_rows(sp + 0x200, sp + 0x310);
        let m = fx.read_m4(sp + 0x200);
        fx.mul_into(Some(sp + 0x1C0), &m);
        for o in [0x300u32, 0x304, 0x308] {
            fx.mem.write_u32(sp + o, 0);
        }
        fx.mem.write_u32(sp + 0x30C, 0x3F80_0000);
        let s5 = s5 & 0xFFFF;
        let mut sized = false;
        for s1 in 0..6u32 {
            let vs = [sp + 0x300, FAN + 0x10 * s1, FAN + 0x10 * (s1 + 1)];
            if fx.project_n(&vs, sp + 0x1C0, d, sp + 0x46C, 0x01E0_1098) != 1 {
                continue;
            }
            if !sized {
                let dx = (i32::from(fx.mem.read_u16(sp + 0x398)) >> 4) - (i32::from(fx.mem.read_u16(sp + 0x3B0)) >> 4);
                let a = Fx::powf2(ee::from_i32(dx));
                let dy = (i32::from(fx.mem.read_u16(sp + 0x39A)) >> 4) - (i32::from(fx.mem.read_u16(sp + 0x3B2)) >> 4);
                let b = Fx::powf2(ee::from_i32(dy));
                let q = fx.sqrtf(ee::add(a, b));
                f20 = ee::div(q, 2.0);
                sized = true;
            }
            let s = fx.tsin(s5);
            let du = fixs(fx, ee::mul(f20, s), 0x01E0_112C);
            let c = fx.tcos(s5);
            let dv = fixs(fx, ee::mul(f20, c), 0x01E0_1140);
            fx.uv_from_xy(sp + 0x390, sp + 0x398, du, dv);
            fx.uv_from_xy(sp + 0x3A8, sp + 0x3B0, 0, 0);
            fx.uv_from_xy(sp + 0x3C0, sp + 0x3C8, 0, 0);
            fx.submit(d, 0, 0);
        }
        fx.mem.write_u32(sp + 0xE0, 0x10000 * (j + 1));
        fx.mem.write_u32(sp + 0xF0, r - 0x1520 + 0x10 * (j + 1));
        fx.mem.write_u32(sp + 0x1B0, j + 1);
    }
    let age = fx.mem.i32(r + 4) + 1;
    fx.mem.set32(r + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(r, 0);
    }
}

/// The burst records [F 0x1E01250 / 0x1E01408].
fn bursts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x4C..0xD4).contains(&n) {
        let r = BURSTS + 0x50 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        for j in 0..4u32 {
            let v = rem2(fx.rand_from(OT, 0x01E0_12A8) >> 23, 256);
            let n = fx.mem.i32(D);
            let s = rem2(n + j as i32 + v, 0x20) as u32;
            table_point(fx, r + 0x10 + 0x10 * j, s, 4.0, [0x01E0_12E8, 0x01E0_1344, 0x01E0_1394]);
        }
    }
    sprite_desc_uv(fx, sp);
    corners(fx, sp, 6.0);
    for k in 0..0x10u32 {
        let r = BURSTS + 0x50 * k;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        for s6 in 0..0x10i32 {
            let ang = mdiv(fx, (s6 + 1) << 15, 0x7878_7879, 3);
            let sn = fx.tsin(ang);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let a = fix(fx, ee::div(ee::mul(age, ee::mul(128.0, sn)), 16.0), 0x01E0_15D8);
            colour(fx, sp, [0xFF, 0x80, 0x80], a as u8);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let mut f20 = ee::add(age, ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(s6)));
            if !crate::exe::ee_le(f20, 16.0) {
                f20 = 16.0;
            }
            let f21 = ee::div(ee::sub(16.0, f20), 16.0);
            fx.identity(sp + 0x2C0);
            let t = fixs(fx, ee::mul(f32::from_bits(0x4479_C000), f20), 0x01E0_1678);
            fx.rotate_axis(sp + 0x2C0, t, 2);
            for s5 in 0..4u32 {
                let a = ((0x10000 * s5) as i32) >> 2;
                let s = fx.tsin(a);
                fx.mem.setf(sp + 0x300, ee::mul(80.0, s));
                let c = fx.tcos(a);
                fx.mem.setf(sp + 0x304, ee::mul(80.0, c));
                let side = ee::from_i32(fx.mem.i32(D + 8));
                fx.mem.setf(sp + 0x308, ee::mul(-250.0, side));
                fx.mem.write_u32(sp + 0x30C, 0x3F80_0000);
                fx.vu_apply_to(sp + 0x300, sp + 0x2C0, sp + 0x300);
                fx.vu_add4(sp + 0x300, sp + 0x300, r + 0x10 + 0x10 * s5);
                fx.vu_sub4(sp + 0x300, sp + 0x300, D + 0x90);
                fx.vu_scale4(sp + 0x300, sp + 0x300, f21);
                fx.vu_add4(sp + 0x300, sp + 0x300, D + 0x90);
                draw(fx, sp, sp + 0x300, 0x01E0_178C);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x11 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// The bursts' sprite descriptor: texture D+0xC, the whole 128 x 128.
fn sprite_desc_uv(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0xC);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    camera(fx, sp);
}

/// One XYZ2 value of the grid: (point + 0x700) x 16, the far Z.
fn grid_xyz(fx: &mut Fx, at: u32, p: u32) {
    let x = fx.mem.read_u16(p).wrapping_add(0x700) << 4;
    fx.mem.write_u16(at, x);
    let y = fx.mem.read_u16(p + 4).wrapping_add(0x700) << 4;
    fx.mem.write_u16(at + 2, y);
    let v = fx.mem.read_u64(at);
    fx.mem.write_u64(at, (v & KEEP) | FAR_Z);
    fx.mem.write_u8(at + 7, 0);
}

/// The warped grid [F 0x1E017F8 / 0x1E01810].
fn warp(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(WARP, 1);
        fx.mem.write_u32(WARP + 4, 0);
    }
    if fx.mem.read_u8(WARP) != 1 {
        return;
    }
    let d = sp + GRID;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xCC, layer);
    // a0..a3 as 0x207E20 leaves them: the descriptor, the ALPHA value, the
    // flags | 0x200 and the flags.
    let flags = u32::from(fx.mem.read_u16(d + 2)) & !0x200;
    fx.texflush([d, a as u32, flags | 0x200, flags]);
    let h = fx.mem.i32(D + 4);
    let (_, regs) = fx.unit_draw(h, 1, sp + 0x450, sp + 0x458);
    fx.texflush(regs);
    fx.desc_clamp(d, crate::lens::LENS_CLAMP);
    let age = fx.mem.i32(WARP + 4);
    let ang = mdiv(fx, age << 15, 0x6666_6667, 5);
    let sn = fx.tsin(ang);
    let a = fix(fx, ee::mul(96.0, sn), 0x01E0_18F8);
    fx.mem.write_u8(sp + 0x400, 0x80);
    fx.mem.write_u8(sp + 0x401, 0x80);
    fx.mem.write_u8(sp + 0x402, 0xFF);
    fx.mem.write_u8(sp + 0x403, a as u8);
    fx.mem.write_u32(sp + 0x404, 0x3F80_0000);
    for row in 0..0x11u32 {
        fx.mem.write_u32(sp + 0x110, 0x2B67 * row);
        fx.mem.write_u32(sp + 0x120, 0x200 * row);
        let age = fx.mem.i32(WARP + 4);
        let t = fx.mult(age, 0x15B3);
        let a = (t + fx.mem.i32(sp + 0x110)) & 0xFFFF;
        let s = fx.tsin(a);
        let f = ee::add(ee::from_i32(fx.mem.i32(sp + 0x120) >> 4), ee::mul(10.0, s));
        let x = fixs(fx, f, 0x01E0_196C);
        for col in 0..0x11u32 {
            let age = fx.mem.i32(WARP + 4);
            let t = fx.mult(age, 0x15B3);
            let a = (t + (0x2B67 * col) as i32) & 0xFFFF;
            let c = fx.tcos(a);
            let f = ee::add(ee::from_i32(((0x200 * col) as i32) >> 4), ee::mul(10.0, c));
            let y = fixs(fx, f, 0x01E0_19C0);
            let p = POINTS + 0x88 * row + 8 * col;
            fx.mem.set32(p, x);
            fx.mem.set32(p + 4, y);
        }
    }
    fx.mem.write_u32(sp + 0x110, 0x2B67 * 0x11);
    fx.mem.write_u32(sp + 0x120, 0x200 * 0x11);
    fx.mem.write_u32(sp + 0x190, 0);
    fx.mem.write_u32(sp + 0x130, D);
    for row in 0..0x10u32 {
        for col in 0..0x10u32 {
            let s1 = D + 0x88 * row + 8 * col;
            for (at, o) in [(0x408u32, 0x2828u32), (0x40A, 0x282C), (0x418, 0x28B0), (0x41A, 0x28B4), (0x428, 0x2830), (0x42A, 0x2834), (0x438, 0x28B8), (0x43A, 0x28BC)] {
                let v = (fx.mem.i32(s1 + o) << 4) as u32;
                uv(fx, sp + at, v);
            }
            for (at, o) in [(0x410u32, 0x3130u32), (0x420, 0x31B8), (0x430, 0x3138), (0x440, 0x31C0)] {
                grid_xyz(fx, sp + at, s1 + o);
            }
            fx.submit(d, 0, 0);
        }
        fx.mem.write_u32(sp + 0x130, D + 0x88 * (row + 1));
        fx.mem.write_u32(sp + 0x190, row + 1);
    }
    let layer = fx.mem.read_u32(sp + 0xCC);
    fx.layer_end(layer, OT, 0);
    let age = fx.mem.i32(WARP + 4) + 1;
    fx.mem.set32(WARP + 4, age);
    if age >= 0x51 {
        fx.mem.write_u8(WARP, 0);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x240);
    for k in 0..6u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x14 + 4 * k));
        fx.node_position(h, slot, D + 0x30 + 0x10 * k);
    }
    fx.vu_add4(D + 0x90, D + 0x30, D + 0x60);
    fx.vu_scale4(D + 0x90, D + 0x90, 0.5);
    if fx.mem.i32(D) == 0 {
        fx.mem.write_u8(FLASH, 1);
        fx.mem.write_u32(FLASH + 4, 0);
    }
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x140, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x150, d + 8);
    fx.mem.write_u32(sp + 0x160, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0xC);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    flash(fx, sp);
    sparks(fx, sp);
    lenses(fx, sp);
    bursts(fx, sp);
    warp(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x41 {
        ret = 1;
    } else {
        if (0x4C..0xE4).contains(&n) {
            ret = 6;
        }
        if n == 0xE4 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xEF {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
