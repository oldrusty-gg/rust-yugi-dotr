//! BMODULE b295 (card 27), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000 kept in
//! s1, 0x9550 bytes) [F]: a single unscheduled
//! function in a 0x3A0 frame on b150's helpers, b643's (card 623) sibling
//! (s0 = the OT, the return at sp + 0xB0); every sprite a textured window
//! sprite (descriptor sp + 0x2E8: 5 registers 0x42421, prim 0x26 | 0x800,
//! clamp 5). The init lays out 33 points of three circles (radii 4 / 4.7 /
//! 5, D+0x8690, 0x30 apart). Every update the camera turn with roll
//! (0x2055E0 into sp + 0x250), the attacker's nodes 0x47 / 0x4A (D+0x20 /
//! D+0x30) and their midpoint D+0x40, node 0x2E (D+0x60); the target's
//! point (0x1F65D0 kind 1) at D+0x70. On 1..0x3B one of 32 spark records
//! (+0x80, 0x60) of four random sparks (0.2) at the midpoint, 32 updates
//! (half size 5 (0.2 + 0.8 age / 32), spread tan falloff, alpha 3 (32 -
//! age), colour (8F, 64, 60), ALPHA preset 0). On 0x43..0xDB one of 48
//! fleck records (+0xC80, 0x1A0) of 16 random flecks (1.0) round the
//! target from the midpoint (z + 4 x the side), sizes 0.2 + 0.8 rand / 256,
//! 48 updates (half size 12 (0.5 + 0.5 age / 48), pushed 1.5 age / 48,
//! alpha 180 ratio_below(age, 4) ramp_above(age, 48, 32) x clamp(0xDC -
//! n) / 32, colour (8C, 38, FF)). On 0x5B..0x171 b643's bursts round the
//! target's point (colour (8B, 39, FF), far Z, key 0). On 0x43..0xDB b643's
//! puffs (alpha 255 sin((age << 15) / 24), colour (80, 28, 8F)). On update
//! 4 one flash record (D+0x8680) at node 0x2E of 128 random sprites (0.3),
//! 32 updates: a sprite (half size 6, alpha 255 (1 - tan falloff), (E4,
//! B4, FF)), a gouraud ring band (prim 0x14, 0x100BA0) of the circles
//! scaled by the squared tan falloff (centre (64, 40, FF), rim (0, 0, 80,
//! 0)) and the sprites (half size 0.5, alpha 255 (32 - age) / 32, (64, 40,
//! FF)), all far Z, key 0. Returns 1 on update 0x50, 6 on 0x5B..0x170, 2
//! on 0x171, 8 once the count reaches 0x173.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x3A0;
const DESC: u32 = 0x2E8;
/// The flash's gouraud band descriptor.
const BAND: u32 = 0x340;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
const SPARKS: u32 = D + 0x80;
const FLECKS: u32 = D + 0xC80;
const BURSTS: u32 = D + 0x5A80;
const PUFFS: u32 = D + 0x7D80;
const FLASH: u32 = D + 0x8680;
const CIRCLES: u32 = D + 0x8690;
const FLASH_CELLS: u32 = D + 0x8CC0;
const FLASH_AT: u32 = D + 0x8D40;
const FLASH_DIRS: u32 = D + 0x8D50;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
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

fn words(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0x9550);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..2u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    for (node, clear, at) in [(0x47, 1, 0x18u32), (0x4A, 0, 0x1C), (0x2E, 0, 0x50)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_point(1, t, D + 0x70);
    for k in 0..0x20u32 {
        fx.mem.write_u8(SPARKS + 0x60 * k, 0);
    }
    for k in 0..0x30u32 {
        fx.mem.write_u8(FLECKS + 0x1A0 * k, 0);
    }
    for k in 0..0x10u32 {
        fx.mem.write_u8(BURSTS + 0x230 * k, 0);
    }
    for k in 0..0x18u32 {
        fx.mem.write_u8(PUFFS + 0x60 * k, 0);
    }
    fx.mem.write_u8(FLASH, 0);
    for k in 0..0x21u32 {
        let a = ((k << 16) as i32) >> 5;
        let s = fx.tsin(a);
        fx.mem.setf(sp + 0x2D0, s);
        let c = fx.tcos(a);
        fx.mem.setf(sp + 0x2D4, c);
        fx.mem.write_u32(sp + 0x2D8, 0);
        fx.mem.write_u32(sp + 0x2DC, 0x3F80_0000);
        let e = CIRCLES + 0x30 * k;
        for (j, r) in [4.0f32, f32::from_bits(0x4096_6666), 5.0].into_iter().enumerate() {
            fx.vu_scale4(e + 0x10 * j as u32, sp + 0x2D0, r);
        }
    }
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x300, u0);
    uv(fx, sp + 0x302, v0);
    uv(fx, sp + 0x310, u1);
    uv(fx, sp + 0x312, v1);
}

/// The 4 x 8 cell `b` of the texture into the sprite's two UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let u0 = ((c << 9) & 0x3FFF) as u32;
    let u1 = ((((c << 5) + 0x20) << 4) & 0x3FFF) as u32;
    let v0 = ((r << 9) & 0x3FFF) as u32;
    let v1 = ((((r << 5) + 0x20) << 4) & 0x3FFF) as u32;
    uvs(fx, sp, u0, v0, u1, v1);
}

/// The view-projection (x identity) at sp + 0x1D0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x1D0);
    fx.identity(sp + 0x210);
    let id = fx.read_m4(sp + 0x210);
    fx.mul_into(Some(sp + 0x1D0), &id);
}

/// The descriptor rewritten through its pointers, | 0x800.
fn desc_reset(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// ALPHA preset `preset`, the texture, clamp 5 (texture first when
/// `tex_first`).
fn desc_setup(fx: &mut Fx, rh_at: u32, preset: i32, tex_first: bool, sp: u32) {
    let d = sp + DESC;
    if tex_first {
        let rh = fx.mem.read_u32(D + rh_at);
        fx.desc_texture(d, rh, 1);
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
    } else {
        let a = fx.alpha_preset(preset);
        fx.desc_alpha(d, a);
        let rh = fx.mem.read_u32(D + rh_at);
        fx.desc_texture(d, rh, 1);
    }
    fx.desc_clamp(d, 5);
}

/// The corners (-s, -s, 0, 1) at sp + 0x290 and (s, s, 0, 1) at sp +
/// 0x2A0, turned by the camera (sp + 0x250) when `turn`.
fn corners(fx: &mut Fx, sp: u32, s: f32, turn: bool) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x290, n);
    fx.mem.setf(sp + 0x294, n);
    fx.mem.write_u32(sp + 0x298, 0);
    fx.mem.write_u32(sp + 0x29C, 0x3F80_0000);
    fx.mem.setf(sp + 0x2A0, s);
    fx.mem.setf(sp + 0x2A4, s);
    fx.mem.write_u32(sp + 0x2A8, 0);
    fx.mem.write_u32(sp + 0x2AC, 0x3F80_0000);
    if turn {
        fx.vu_apply_to(sp + 0x290, sp + 0x250, sp + 0x290);
        fx.vu_apply_to(sp + 0x2A0, sp + 0x250, sp + 0x2A0);
    }
}

/// The colour (c, a, Q = 1.0) at sp + 0x2F8.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x2F8, c[0]);
    fx.mem.write_u8(sp + 0x2F9, c[1]);
    fx.mem.write_u8(sp + 0x2FA, c[2]);
    fx.mem.write_u8(sp + 0x2FB, a);
    fx.mem.write_u32(sp + 0x2FC, 0x3F80_0000);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The vertices at sp + 0x2B0 / 0x2C0 = the corners + sp + 0x2D0.
fn place(fx: &mut Fx, sp: u32) {
    fx.vu_add4(sp + 0x2B0, sp + 0x290, sp + 0x2D0);
    fx.vu_add4(sp + 0x2C0, sp + 0x2A0, sp + 0x2D0);
}

fn project_from(fx: &mut Fx, sp: u32, input: u32) -> bool {
    crate::fxlib2::project(fx, input, PROJ_OUT, sp + 0x1D0, sp + DESC, sp + 0x39C)
}

/// Projected and submitted at the mean w.
fn draw(fx: &mut Fx, sp: u32, ra: u32) {
    if project_from(fx, sp, sp + 0x2B0) {
        let w = fx.mem.f(sp + 0x39C);
        let key = fix(fx, w, ra);
        fx.submit(sp + DESC, OT, key as i32);
    }
}

/// The far Z on the XYZ values at `xyz[0]` (set) and the rest (copied).
fn far_z(fx: &mut Fx, xyz: &[u32]) {
    let v = fx.mem.read_u64(xyz[0]);
    fx.mem.write_u64(xyz[0], (v & KEEP) | FAR_Z);
    for &a in &xyz[1..] {
        let z = ((fx.mem.read_u64(xyz[0]) << 8) >> 40) << 32;
        let w = fx.mem.read_u64(a);
        fx.mem.write_u64(a, (w & KEEP) | z);
    }
}

/// Projected from `input` and submitted at the far Z with key 0.
fn draw_far(fx: &mut Fx, sp: u32, input: u32) {
    if project_from(fx, sp, input) {
        far_z(fx, &[sp + 0x308, sp + 0x318]);
        fx.submit(sp + DESC, OT, 0);
    }
}

/// clamp(a, 0, 32) x clamp(b, 0, 32) / 1024.
fn window(a: i32, b: i32) -> f32 {
    let f = ee::from_i32(Fx::clamp_i(a, 0, 0x20));
    let g = ee::from_i32(Fx::clamp_i(b, 0, 0x20));
    ee::div(ee::mul(f, g), 1024.0)
}

/// The spark records [F 0x1E0030C / 0x1E003C8].
fn sparks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x3C {
        let base = D + 0x60 * rem2(n, 0x20) as u32;
        let r = base + 0x80;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x10, D + 0x40);
        for j in 0..4u32 {
            let c = fx.rand_from(OT, 0x01E0_0384) >> 23;
            fx.mem.write_u16(r + 8 + 2 * j, rem2(c, 8) as u16);
            fx.random_table_vec(r + 0x20 + 0x10 * j, f32::from_bits(0x3E4C_CCCD));
        }
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
    desc_setup(fx, 0x10, 0, false, sp);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x1C0, 0);
    fx.mem.write_u32(sp + 0xD0, D);
    for k in 0..0x20u32 {
        let base = D + 0x60 * k;
        let r = base + 0x80;
        if fx.mem.read_u8(r) != 0 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 32.0);
            let s = ee::mul(5.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s, true);
            let age = fx.mem.i32(r + 4);
            let a = (0x20 - age).wrapping_mul(3);
            colour(fx, sp, [0x8F, 0x64, 0x60], a as u8);
            let age = fx.mem.i32(r + 4);
            let f20 = fx.tan_falloff_p(age, 0x20, 1);
            fx.mem.write_u32(sp + 0xC0, r + 0x10);
            for j in 0..4u32 {
                fx.vu_scale4(sp + 0x2D0, r + 0x20 + 0x10 * j, f20);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, r + 0x10);
                place(fx, sp);
                let b = fx.mem.read_u16(r + 8 + 2 * j);
                cell_uvs(fx, sp, i32::from(b));
                draw(fx, sp, 0x01E0_0690);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x21 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0xD0, base + 0x60);
        fx.mem.write_u32(sp + 0x1C0, k + 1);
    }
}

/// The fleck records [F 0x1E00708 / 0x1E00868].
fn flecks(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x43..0xDC).contains(&n) {
        let (_, k) = fx.div(n, 0x30);
        let base = D + 0x1A0 * k as u32;
        let r = base + 0xC80;
        fx.mem.write_u8(r, 1);
        fx.mem.write_u32(r + 4, 0);
        words(fx, r + 0x90, D + 0x40);
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let z = ee::add(fx.mem.f(r + 0x98), ee::mul(4.0, side));
        fx.mem.setf(r + 0x98, z);
        fx.mem.write_u32(sp + 0xE0, r + 0x90);
        for j in 0..0x10u32 {
            let c = fx.rand_from(OT, 0x01E0_07B0) >> 23;
            fx.mem.set32(r + 8 + 4 * j, rem2(c, 8));
            let q = fx.rand_from(OT, 0x01E0_07D0) >> 23;
            let f = ee::from_i32(rem2(q, 256));
            let f = ee::add(f32::from_bits(0x3E4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), f), 256.0));
            fx.mem.setf(r + 0x48 + 4 * j, f);
            let p = r + 0xA0 + 0x10 * j;
            fx.random_table_vec(p, 1.0);
            fx.vu_add4(p, p, D + 0x70);
            fx.vu_sub4(p, p, r + 0x90);
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x10, 1, false, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = ee::div(ee::from_i32(Fx::clamp_i(0xDC - n, 0, 0x20)), 32.0);
    fx.mem.write_u32(sp + 0x1B0, 0);
    fx.mem.write_u32(sp + 0x100, D);
    for k in 0..0x30u32 {
        let base = D + 0x1A0 * k;
        let r = base + 0xC80;
        if fx.mem.read_u8(r) != 0 {
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f21 = ee::mul(180.0, Fx::ratio_below(age, 4.0));
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f0 = ee::mul(f21, Fx::ramp_above(age, 48.0, 32.0));
            let a = fix(fx, ee::mul(f20, f0), 0x01E0_0990);
            colour(fx, sp, [0x8C, 0x38, 0xFF], a as u8);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let s = ee::div(ee::mul(0.5, age), 48.0);
            let s = ee::mul(12.0, ee::add(0.5, s));
            corners(fx, sp, s, true);
            let age = ee::from_i32(fx.mem.i32(r + 4));
            let f21 = ee::div(ee::mul(1.5, age), 48.0);
            fx.mem.write_u32(sp + 0xF0, r + 0x90);
            for j in 0..0x10u32 {
                let b = fx.mem.i32(r + 8 + 4 * j);
                cell_uvs(fx, sp, b);
                let sz = fx.mem.f(r + 0x48 + 4 * j);
                fx.vu_scale4(sp + 0x2B0, sp + 0x290, sz);
                let sz = fx.mem.f(r + 0x48 + 4 * j);
                fx.vu_scale4(sp + 0x2C0, sp + 0x2A0, sz);
                fx.vu_scale4(sp + 0x2D0, r + 0xA0 + 0x10 * j, f21);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, r + 0x90);
                fx.vu_add4(sp + 0x2B0, sp + 0x2B0, sp + 0x2D0);
                fx.vu_add4(sp + 0x2C0, sp + 0x2C0, sp + 0x2D0);
                draw(fx, sp, 0x01E0_0B98);
            }
            let age = fx.mem.i32(r + 4) + 1;
            fx.mem.set32(r + 4, age);
            if age >= 0x31 {
                fx.mem.write_u8(r, 0);
            }
        }
        fx.mem.write_u32(sp + 0x100, base + 0x1A0);
        fx.mem.write_u32(sp + 0x1B0, k + 1);
    }
}

/// The burst records [F 0x1E00C10 / 0x1E00CB8] (b643's, at the target's
/// point D+0x70, far Z).
fn bursts(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x5B..0x172).contains(&n) {
        let q = BURSTS + 0x230 * rem2(n, 0x10) as u32;
        fx.mem.write_u8(q, 1);
        fx.mem.write_u32(q + 4, 0);
        for j in 0..0x20u32 {
            let r = fx.rand_from(OT, 0x01E0_0C74) >> 23;
            fx.mem.set8(q + 8 + j, rem2(r, 8));
            fx.random_table_vec(q + 0x30 + 0x10 * j, f32::from_bits(0x3FA6_6666));
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x10, 1, true, sp);
    camera(fx, sp);
    let n = fx.mem.i32(D);
    let f20 = window(n - 0x5A, 0x172 - n);
    fx.mem.write_u32(sp + 0x1A0, 0);
    fx.mem.write_u32(sp + 0x110, D);
    for i in 0..0x10u32 {
        let q = BURSTS + 0x230 * i;
        if fx.mem.read_u8(q) != 0 {
            let age = fx.mem.i32(q + 4);
            let sn = fx.tsin((age << 15) >> 4);
            let a = fix(fx, ee::mul(f20, ee::mul(96.0, sn)), 0x01E0_0DD8);
            colour(fx, sp, [0x8B, 0x39, 0xFF], a as u8);
            let age = fx.mem.i32(q + 4);
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x10 - age)), 16.0);
            let s = ee::mul(15.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s, true);
            let age = fx.mem.i32(q + 4);
            let f4 = ee::div(ee::from_i32(age), 16.0);
            let f1 = ee::div(ee::mul(f32::from_bits(0x3DCC_CCCD), ee::from_i32(0x10 - age)), 16.0);
            let f21 = ee::add(f32::from_bits(0x3F66_6666), f1);
            let f22 = ee::add(-25.0, ee::mul(80.0, f4));
            for j in 0..0x20u32 {
                let b = fx.mem.read_u8(q + 8 + j);
                cell_uvs(fx, sp, i32::from(b));
                let p = q + 0x30 + 0x10 * j;
                let x = ee::mul(fx.mem.f(p), f21);
                fx.mem.setf(sp + 0x2D0, x);
                let y = ee::add(f22, fx.mem.f(p + 4));
                fx.mem.setf(sp + 0x2D4, y);
                let z = ee::mul(fx.mem.f(p + 8), f21);
                fx.mem.setf(sp + 0x2D8, z);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, D + 0x70);
                place(fx, sp);
                draw_far(fx, sp, sp + 0x2B0);
            }
            let age = fx.mem.i32(q + 4) + 1;
            fx.mem.set32(q + 4, age);
            if age >= 0x11 {
                fx.mem.write_u8(q, 0);
            }
        }
        fx.mem.write_u32(sp + 0x110, D + 0x230 * (i + 1));
        fx.mem.write_u32(sp + 0x1A0, i + 1);
    }
}

/// The puff records [F 0x1E01118 / 0x1E011D0] (b643's).
fn puffs(fx: &mut Fx, sp: u32) {
    let n = fx.mem.i32(D);
    if (0x43..0xDC).contains(&n) {
        let (_, k) = fx.div(n, 0x18);
        let p = PUFFS + 0x60 * k as u32;
        fx.mem.write_u8(p, 1);
        fx.mem.write_u32(p + 4, 0);
        words(fx, p + 0x10, D + 0x40);
        for j in 0..4u32 {
            let r = fx.rand_from(OT, 0x01E0_1190) >> 23;
            fx.mem.write_u16(p + 8 + 2 * j, rem2(r, 4) as u16);
            fx.random_table_vec(p + 0x20 + 0x10 * j, 0.5);
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, false, sp);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x190, 0);
    fx.mem.write_u32(sp + 0x130, D);
    for i in 0..0x18u32 {
        let p = PUFFS + 0x60 * i;
        if fx.mem.read_u8(p) != 0 {
            let age = fx.mem.i32(p + 4);
            let s = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), ee::from_i32(0x18 - age)), 24.0);
            let s = ee::mul(4.0, ee::add(f32::from_bits(0x3E4C_CCCD), s));
            corners(fx, sp, s, true);
            let x = fx.mem.i32(p + 4) << 15;
            fx.mult(0x2AAA_AAAB, x);
            let hi = fx.hilo().1;
            let ang = (hi >> 2).wrapping_add(((x as u32) >> 31) as i32);
            let sn = fx.tsin(ang);
            let a = fix(fx, ee::mul(255.0, sn), 0x01E0_1360);
            colour(fx, sp, [0x80, 0x28, 0x8F], a as u8);
            let age = fx.mem.i32(p + 4);
            let f20 = fx.tan_falloff_p(age, 0x18, 1);
            fx.mem.write_u32(sp + 0x120, p + 0x10);
            for j in 0..4u32 {
                fx.vu_scale4(sp + 0x2D0, p + 0x20 + 0x10 * j, f20);
                fx.vu_add4(sp + 0x2D0, sp + 0x2D0, p + 0x10);
                place(fx, sp);
                let b = fx.mem.read_u16(p + 8 + 2 * j);
                cell_uvs(fx, sp, i32::from(b));
                draw(fx, sp, 0x01E0_14B8);
            }
            let age = fx.mem.i32(p + 4) + 1;
            fx.mem.set32(p + 4, age);
            if age >= 0x19 {
                fx.mem.write_u8(p, 0);
            }
        }
        fx.mem.write_u32(sp + 0x130, D + 0x60 * (i + 1));
        fx.mem.write_u32(sp + 0x190, i + 1);
    }
}

/// The view-projection x (the camera turn moved to the flash's point) at
/// sp + 0x1D0, the turn scaled by `scale` first when given.
fn flash_camera(fx: &mut Fx, sp: u32, scale: Option<u32>) {
    fx.camera_vp_to(sp + 0x1D0);
    fx.translate_rows(sp + 0x210, sp + 0x250, FLASH_AT);
    if let Some(v) = scale {
        fx.scale_rows(sp + 0x210, v);
    }
    let m = fx.read_m4(sp + 0x210);
    fx.mul_into(Some(sp + 0x1D0), &m);
}

/// The flash record [F 0x1E01530 / 0x1E015EC].
fn flash(fx: &mut Fx, sp: u32) {
    if fx.mem.i32(D) == 4 {
        fx.mem.write_u8(FLASH, 1);
        fx.mem.write_u32(FLASH + 4, 0);
        words(fx, FLASH_AT, D + 0x60);
        for j in 0..0x80u32 {
            let r = fx.rand_from(OT, 0x01E0_1594) >> 23;
            fx.mem.set8(FLASH_CELLS + j, rem2(r, 4));
            fx.random_table_vec(FLASH_DIRS + 0x10 * j, f32::from_bits(0x3E99_999A));
        }
    }
    if fx.mem.read_u8(FLASH) != 1 {
        return;
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, true, sp);
    uvs(fx, sp, 0x600, 0, 0x800, 0x200);
    flash_camera(fx, sp, None);
    let age = fx.mem.i32(FLASH + 4);
    let f0 = fx.tan_falloff_p(age, 0x20, 1);
    let a = fix(fx, ee::mul(255.0, ee::sub(1.0, f0)), 0x01E0_1720);
    colour(fx, sp, [0xE4, 0xB4, 0xFF], a as u8);
    corners(fx, sp, 6.0, false);
    draw_far(fx, sp, sp + 0x290);
    // The ring band: a 4-vertex gouraud strip (8 registers RGBAQ / XYZF2).
    let d2 = sp + BAND;
    fx.mem.write_u16(d2 + 2, 0x14);
    fx.mem.write_u16(d2, 8);
    fx.mem.write_u64(d2 + 8, 0x4141_4141);
    fx.mem.write_u32(d2 + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d2, a);
    let age = fx.mem.i32(FLASH + 4);
    let f20 = fx.tan_falloff_p(age, 0x20, 2);
    fx.camera_vp_to(sp + 0x1D0);
    fx.translate_rows(sp + 0x210, sp + 0x250, FLASH_AT);
    fx.mem.setf(sp + 0x2D0, f20);
    fx.mem.setf(sp + 0x2D4, f20);
    fx.mem.write_u32(sp + 0x2D8, 0);
    fx.mem.write_u32(sp + 0x2DC, 0x3F80_0000);
    fx.scale_rows(sp + 0x210, sp + 0x2D0);
    let m = fx.read_m4(sp + 0x210);
    fx.mul_into(Some(sp + 0x1D0), &m);
    let age = fx.mem.i32(FLASH + 4);
    fx.mem.write_u8(sp + 0x350, 0x64);
    fx.mem.write_u8(sp + 0x351, 0x40);
    fx.mem.write_u8(sp + 0x352, 0xFF);
    fx.mem.write_u8(sp + 0x353, (((0x20 - age) * 255) >> 5) as u8);
    fx.mem.write_u32(sp + 0x354, 0x3F80_0000);
    words2(fx, sp + 0x360, sp + 0x350);
    fx.mem.write_u8(sp + 0x370, 0);
    fx.mem.write_u8(sp + 0x371, 0);
    fx.mem.write_u8(sp + 0x372, 0x80);
    fx.mem.write_u8(sp + 0x373, 0);
    fx.mem.write_u32(sp + 0x374, 0x3F80_0000);
    words2(fx, sp + 0x380, sp + 0x370);
    for k in 0..0x20u32 {
        let (a, b) = (CIRCLES + 0x30 * k, CIRCLES + 0x30 * (k + 1));
        fx.mem.write_u32(sp + 0x170, D + 0x30 * (k + 1));
        fx.mem.write_u32(sp + 0x180, b + 0x10);
        for (outer, ra) in [(0u32, 0x01E0_19D8u32), (0x20, 0x01E0_1AF0)] {
            let vs = [a + 0x10, b + 0x10, a + outer, b + outer];
            if fx.project_n(&vs, sp + 0x1D0, d2, sp + 0x39C, ra) == 1 {
                far_z(fx, &[sp + 0x358, sp + 0x368, sp + 0x378, sp + 0x388]);
                fx.submit(d2, OT, 0);
            }
        }
    }
    desc_reset(fx, sp);
    desc_setup(fx, 0x14, 1, true, sp);
    camera(fx, sp);
    let age = fx.mem.i32(FLASH + 4);
    colour(fx, sp, [0x64, 0x40, 0xFF], (((0x20 - age) * 255) >> 5) as u8);
    corners(fx, sp, 0.5, true);
    for j in 0..0x80u32 {
        let b = fx.mem.read_u8(FLASH_CELLS + j);
        cell_uvs(fx, sp, i32::from(b));
        fx.vu_scale4(sp + 0x2D0, FLASH_DIRS + 0x10 * j, f20);
        fx.vu_add4(sp + 0x2D0, sp + 0x2D0, FLASH_AT);
        place(fx, sp);
        draw_far(fx, sp, sp + 0x2B0);
    }
    let age = fx.mem.i32(FLASH + 4) + 1;
    fx.mem.set32(FLASH + 4, age);
    if age >= 0x21 {
        fx.mem.write_u8(FLASH, 0);
    }
}

fn words2(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..2u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x250);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x18 + 4 * k));
        fx.node_position(h, slot, D + 0x20 + 0x10 * k);
    }
    fx.vu_add4(D + 0x40, D + 0x20, D + 0x30);
    fx.vu_scale4(D + 0x40, D + 0x40, 0.5);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x50));
    fx.node_position(h, slot, D + 0x60);
    sparks(fx, sp);
    flecks(fx, sp);
    bursts(fx, sp);
    puffs(fx, sp);
    flash(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x50 {
        ret = 1;
    } else {
        if (0x5B..0x171).contains(&n) {
            ret = 6;
        }
        if n == 0x171 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x173 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
