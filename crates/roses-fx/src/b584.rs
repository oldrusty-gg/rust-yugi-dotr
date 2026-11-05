//! BMODULE b584 (card 503), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xD3E0
//! bytes) [F]: a single function in a 0x430
//! frame (s1 = D, s0 = the OT, the return at sp + 0xE0). Every update the
//! camera turn with roll (sp + 0x280) and the attacker's nodes 0x22 /
//! 0x1C (D+0x20 / D+0x40); the target's position at D+0x50. Every third
//! update on 0x42..0xDA a flash (10 round robin, +0x2470, 0x20) at node
//! 0x22 for 3 updates: 64 x 8 gouraud triangles (prim 0x33, texture D+0x10)
//! from its centre through the 64 nine-point arcs D+0x70 (set at init:
//! radius (k + 1) / 64, height 50 (64 - k) / 64, a random turn), turned to
//! the side and scaled 0.8 + 0.2 age / 2, alpha 128 / 16 (1 - tan
//! falloff), and a flat fan (the 33 points D+0x25B0, radius 25). On
//! 1..0x27 a burst (32 round robin, +0x27C0, 0x60) of 4 sprites half a
//! table direction from node 0x1C, lowered 10 and pushed 10 x the side,
//! for 32 updates (texture D+0x14, cells by the table 0x28AE70). Every
//! third update on 0x89..0xDB a ring set (24 round robin, +0x33D0, 0x6A0)
//! at a table point round the target for 24 updates: 64 alternating
//! sprites (ALPHA 2 / 1), a flat fan (the 17 points D+0xD2D0, radius 20,
//! key 0xFFFFFF) and 32 x 8 small sprites (texture D+0x18). Returns 1 on
//! 0x7E, 6 on 0x89..0xDB, 2 on 0xDC, 8 once the count reaches 0xF5.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x430;
/// The flash triangles' descriptor (prim 0x33, 9 registers).
const TRI: u32 = 0x310;
/// The fan descriptor (prim 0x13, 6 registers).
const FAN: u32 = 0x388;
/// The sprite descriptor (prim 0x26).
const SPRITE: u32 = 0x3D0;
const FLASHES: u32 = D + 0x2470;
const BURSTS: u32 = D + 0x27C0;
const SETS: u32 = D + 0x33D0;
/// The direction table of 0x101080 (4 rows x 32 vec3).
const DIR_TABLE: u32 = 0x0028_AE90;
/// The sprite cells' UV table (8 bytes a cell).
const CELLS: u32 = 0x0028_AE70;

/// The registers 0x20C0B0 saves that the record leaves (f20, s2, s3)
/// [F; the runner's before the first write not known: 0, H].
#[derive(Default)]
struct Regs {
    f20: f32,
    s2: u64,
    s3: u64,
}

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xE0, 0);
    let ret = if param >= 0 { init(fx) } else { step(fx, sp, param) };
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

fn words(fx: &mut Fx, to: u32, from: u32, n: u32) {
    for c in 0..n {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

fn vec4(fx: &mut Fx, p: u32, v: [f32; 4]) {
    for (k, x) in v.iter().enumerate() {
        fx.mem.setf(p + 4 * k as u32, *x);
    }
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// A table component: row rand % 4, entry rand % 32 (`ras`), component
/// `c` (rand's saved s0 the OT).
fn table(fx: &mut Fx, c: u32, ras: [u32; 2]) -> f32 {
    let r = fx.rand_from(OT, ras[0]) >> 23;
    let row = DIR_TABLE.wrapping_add(4 * c).wrapping_add((rem2(r, 4) * 0x180) as u32);
    let r = fx.rand_from(OT, ras[1]) >> 23;
    fx.mem.f(row.wrapping_add((rem2(r, 32) * 12) as u32))
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xD3E0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for (k, (kind, id)) in [(4, 0xF), (5, 0), (5, 1)].into_iter().enumerate() {
        let r = fx.texture(kind, id);
        fx.mem.write_u32(D + 0x10 + 4 * k as u32, r);
    }
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x22, 1);
    fx.mem.set32(D + 0x1C, r);
    let h = fx.mem.i32(D + 4);
    let r = fx.request_node(h, 0x1C, 0);
    fx.mem.set32(D + 0x30, r);
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    for s0 in 0..0x40i32 {
        let f20 = ee::div(ee::from_i32(s0 + 1), 64.0);
        let f21 = ee::div(ee::mul(50.0, ee::from_i32(0x40 - s0)), 64.0);
        // rand's saved s0 is the arc counter.
        let r = fx.rand_from(s0 as u32, 0x01E0_01B4) >> 23;
        let s3 = (rem2(r, 256) << 16) >> 8;
        for k in 0..9i32 {
            let a = (s3 + ((k << 16) >> 3)) & 0xFFFF;
            let p = D + 0x70 + 0x90 * s0 as u32 + 0x10 * k as u32;
            let s = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f20, s));
            let c = fx.tcos(a);
            fx.mem.setf(p + 4, ee::mul(f20, c));
            fx.mem.setf(p + 8, f21);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    for (p, n, sh, r) in [(D + 0x25B0, 0x21u32, 5, 25.0f32), (D + 0xD2D0, 0x11, 4, 20.0)] {
        for k in 0..n {
            let a = ((k << 16) as i32) >> sh;
            let q = p + 0x10 * k;
            let s = fx.tsin(a);
            fx.mem.setf(q, ee::mul(r, s));
            let c = fx.tcos(a);
            fx.mem.setf(q + 4, ee::mul(r, c));
            fx.mem.write_u32(q + 8, 0);
            fx.mem.write_u32(q + 0xC, 0x3F80_0000);
        }
        if n == 0x21 {
            for k in 0..0x20u32 {
                fx.mem.write_u8(BURSTS + 0x60 * k, 0);
            }
            fx.mem.write_u32(D + 0x33C0, 0);
            for k in 0..0x18u32 {
                fx.mem.write_u8(SETS + 0x6A0 * k, 0);
            }
        }
    }
    0
}

fn colour(fx: &mut Fx, p: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(p, c[0]);
    fx.mem.write_u8(p + 1, c[1]);
    fx.mem.write_u8(p + 2, c[2]);
    fx.mem.write_u8(p + 3, a);
    fx.mem.write_u32(p + 4, 0x3F80_0000);
}

/// The view-projection (x identity) at sp + 0x200.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x200);
    fx.identity(sp + 0x240);
    crate::blib::exe::mul_mem(fx, sp + 0x200, sp + 0x240);
}

/// The sprite descriptor reset (prim 0x26 | 0x800).
fn sprite_desc(fx: &mut Fx, sp: u32) {
    let d = sp + SPRITE;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

fn fan_desc(fx: &mut Fx, sp: u32) {
    let d = sp + FAN;
    fx.mem.write_u16(d + 2, 0x13);
    fx.mem.write_u16(d, 6);
    fx.mem.write_u64(d + 8, 0x41_4141);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x2C0 / 0x2D0 turned
/// by the camera.
fn corners(fx: &mut Fx, sp: u32, n: f32, s: f32) {
    vec4(fx, sp + 0x2C0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x2D0, [s, s, 0.0, 1.0]);
    fx.vu_apply_to(sp + 0x2C0, sp + 0x280, sp + 0x2C0);
    fx.vu_apply_to(sp + 0x2D0, sp + 0x280, sp + 0x2D0);
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's UVs from the cell `c` of the table 0x28AE70.
fn cell_uvs(fx: &mut Fx, sp: u32, c: u32) {
    let t = CELLS + 8 * c;
    for (a, o) in [(0x3E8, 0), (0x3EA, 4), (0x3F8, 2), (0x3FA, 6)] {
        let v = u32::from(fx.mem.read_u16(t + o));
        uv(fx, sp + a, v);
    }
}

/// The sprite at sp + 0x300 (+ the corners) projected and drawn.
fn sprite_draw(fx: &mut Fx, sp: u32, ra: u32) {
    fx.vu_add4(sp + 0x2E0, sp + 0x2C0, sp + 0x300);
    fx.vu_add4(sp + 0x2F0, sp + 0x2D0, sp + 0x300);
    if crate::fxlib2::project(fx, sp + 0x2E0, PROJ_OUT, sp + 0x200, sp + SPRITE, sp + 0x42C) {
        let w = fx.mem.f(sp + 0x42C);
        let key = fix(fx, w, ra);
        fx.submit(sp + SPRITE, OT, key as i32);
    }
}

/// The fan of triangles from the origin (sp + 0x300) round `p` (`n`
/// segments); `far`: every vertex at Z 0xFFFFF8, key 0, else key `key`.
fn fan(fx: &mut Fx, sp: u32, p: u32, n: u32, far: bool, key: i32, ra: u32) {
    vec4(fx, sp + 0x300, [0.0, 0.0, 0.0, 1.0]);
    for j in 0..n {
        if fx.project_n(&[sp + 0x300, p + 0x10 * j, p + 0x10 * (j + 1)], sp + 0x200, sp + FAN, sp + 0x42C, ra) == 1 {
            if far {
                let v = fx.mem.read_u64(sp + 0x3A0);
                fx.mem.write_u64(sp + 0x3A0, (v & 0xFF00_0000_FFFF_FFFF) | (0xFF_FFF8u64 << 32));
                let z = (fx.mem.read_u64(sp + 0x3A0) >> 32) & 0xFF_FFFF;
                for a in [sp + 0x3B0, sp + 0x3C0] {
                    let v = fx.mem.read_u64(a);
                    fx.mem.write_u64(a, (v & 0xFF00_0000_FFFF_FFFF) | (z << 32));
                }
            }
            fx.submit(sp + FAN, OT, key);
        }
    }
}

/// The flashes [F 0x1E00518].
fn flashes(fx: &mut Fx, sp: u32, rg: &mut Regs) {
    for i in 0..0xAu32 {
        let e = FLASHES + 0x20 * i;
        if fx.mem.read_u8(e) == 0 {
            continue;
        }
        fx.camera_vp_to(sp + 0x200);
        let f = [rg.f20, 0.0, 0.0, 0.0, 0.0];
        fx.trs_into_saving(sp + 0x240, 0, 0, e + 0x10, f, [u64::from(OT), u64::from(D), rg.s2, rg.s3], 0x01E0_0540);
        let side = fx.mem.i32(D + 0xC);
        fx.rotate_axis(sp + 0x240, ((side + 1) >> 1) << 15, 1);
        let age = ee::from_i32(fx.mem.i32(e + 4));
        let s = ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 2.0));
        vec4(fx, sp + 0x300, [s, s, s, 1.0]);
        fx.scale_rows(sp + 0x240, sp + 0x300);
        crate::blib::exe::mul_mem(fx, sp + 0x200, sp + 0x240);
        let age = fx.mem.i32(e + 4);
        let tf = fx.tan_falloff_p(age, 2, 1);
        let f20 = ee::sub(1.0, tf);
        rg.f20 = f20;
        let a = fix(fx, ee::mul(128.0, f20), 0x01E0_0600);
        colour(fx, sp + 0x320, [0xA4, 0xFF, 0x64], a as u8);
        let a = fix(fx, ee::mul(16.0, f20), 0x01E0_0634);
        colour(fx, sp + 0x338, [0x40, 0xFF, 0x20], a as u8);
        words(fx, sp + 0x350, sp + 0x338, 2);
        vec4(fx, sp + 0x300, [0.0, 0.0, 0.0, 1.0]);
        for k in 0..0x40u32 {
            let arc = D + 0x70 + 0x90 * k;
            for j in 0..8u32 {
                for (o, v) in [(0x328, 0x3F1F_0000), (0x32C, 0x3E80_0000), (0x340, 0x3F0E_0000), (0x344, 0), (0x358, 0x3F30_0000), (0x35C, 0)] {
                    fx.mem.write_u32(sp + o, v);
                }
                if fx.project_n(&[sp + 0x300, arc + 0x10 * j, arc + 0x10 * (j + 1)], sp + 0x200, sp + TRI, sp + 0x42C, 0x01E0_06F0) == 1 {
                    let w = fx.mem.f(sp + 0x42C);
                    let key = fix(fx, w, 0x01E0_0704);
                    fx.submit(sp + TRI, OT, key as i32);
                }
            }
        }
        fx.camera_vp_to(sp + 0x200);
        fx.translate_rows(sp + 0x240, sp + 0x280, e + 0x10);
        let age = fx.mem.i32(e + 4);
        let tf = fx.tan_falloff_p(age, 2, 1);
        let s = ee::add(f32::from_bits(0x3F4C_CCCD), ee::mul(f32::from_bits(0x3E4C_CCCD), tf));
        vec4(fx, sp + 0x300, [s, s, s, 1.0]);
        fx.scale_rows(sp + 0x240, sp + 0x300);
        crate::blib::exe::mul_mem(fx, sp + 0x200, sp + 0x240);
        colour(fx, sp + 0x398, [0xFF, 0xC4, 0xA4], 0);
        colour(fx, sp + 0x3A8, [0xC8, 0, 0], 0);
        words(fx, sp + 0x3B8, sp + 0x3A8, 2);
        fan(fx, sp, D + 0x25B0, 0x20, true, 0, 0x01E0_0858);
        rg.s2 = u64::from(D + 0x200);
        rg.s3 = 0x20;
        let age = fx.mem.i32(e + 4) + 1;
        fx.mem.set32(e + 4, age);
        if age >= 3 {
            fx.mem.write_u8(e, 0);
        }
    }
}

/// A new burst [F 0x1E0096C].
fn spawn_burst(fx: &mut Fx, n: i32) {
    let r = BURSTS + 0x60 * rem2(n, 0x20) as u32;
    fx.mem.write_u8(r, 1);
    fx.mem.write_u32(r + 4, 0);
    words(fx, r + 0x10, D + 0x40, 4);
    for k in 0..4u32 {
        let x = fx.rand_from(OT, 0x01E0_09CC) >> 23;
        fx.mem.write_u16(r + 8 + 2 * k, rem2(x, 4) as u16);
        let e = r + 0x20 + 0x10 * k;
        let ras = [[0x01E0_09EC, 0x01E0_0A20], [0x01E0_0A64, 0x01E0_0A98], [0x01E0_0ADC, 0x01E0_0B10]];
        for (c, ra) in ras.into_iter().enumerate() {
            let f = table(fx, c as u32, ra);
            fx.mem.setf(e + 4 * c as u32, ee::mul(0.5, f));
        }
        fx.mem.write_u32(e + 0xC, 0x3F80_0000);
        let y = fx.mem.f(e + 4);
        fx.mem.setf(e + 4, ee::add(y, -10.0));
        let side = ee::from_i32(fx.mem.i32(D + 0xC));
        let z = fx.mem.f(e + 8);
        fx.mem.setf(e + 8, ee::add(z, ee::mul(-10.0, side)));
    }
}

/// The bursts [F 0x1E00B98].
fn bursts(fx: &mut Fx, sp: u32) {
    let d = sp + SPRITE;
    sprite_desc(fx, sp);
    let a = fx.alpha_preset(0);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    for i in 0..0x20u32 {
        let r = BURSTS + 0x60 * i;
        if fx.mem.read_u8(r) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 4));
        let f2 = ee::div(ee::mul(f32::from_bits(0x3F4C_CCCD), age), 32.0);
        let s = ee::mul(5.0, ee::add(f32::from_bits(0x3E4C_CCCD), f2));
        corners(fx, sp, ee::neg(s), s);
        let age = fx.mem.i32(r + 4);
        colour(fx, sp + 0x3E0, [0x80, 0x20, 0x80], (3 * (0x20 - age)) as u8);
        let age = fx.mem.i32(r + 4);
        let f20 = fx.tan_falloff_p(age, 0x20, 1);
        for k in 0..4u32 {
            fx.vu_scale4(sp + 0x300, r + 0x20 + 0x10 * k, f20);
            fx.vu_add4(sp + 0x300, sp + 0x300, r + 0x10);
            let c = u32::from(fx.mem.read_u16(r + 8 + 2 * k));
            fx.vu_add4(sp + 0x2E0, sp + 0x2C0, sp + 0x300);
            fx.vu_add4(sp + 0x2F0, sp + 0x2D0, sp + 0x300);
            cell_uvs(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x2E0, PROJ_OUT, sp + 0x200, d, sp + 0x42C) {
                let w = fx.mem.f(sp + 0x42C);
                let key = fix(fx, w, 0x01E0_0E8C);
                fx.submit(d, OT, key as i32);
            }
        }
        let age = fx.mem.i32(r + 4) + 1;
        fx.mem.set32(r + 4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r, 0);
        }
    }
}

/// A new ring set [F 0x1E00F40].
fn spawn_set(fx: &mut Fx) {
    let c = fx.mem.i32(D + 0x33C0);
    fx.mult(c, 0x6A0);
    let g = SETS + 0x6A0 * c as u32;
    fx.mem.write_u8(g, 1);
    fx.mem.write_u32(g + 4, 1);
    let ras = [[0x01E0_0F6C, 0x01E0_0FA0], [0x01E0_0FE4, 0x01E0_1018], [0x01E0_105C, 0x01E0_1090]];
    for (k, ra) in ras.into_iter().enumerate() {
        let f = table(fx, k as u32, ra);
        fx.mult(c, 0x6A0);
        fx.mem.setf(g + 0x90 + 4 * k as u32, f);
    }
    fx.mem.write_u32(g + 0x9C, 0x3F80_0000);
    fx.mult(c, 0x6A0);
    fx.vu_add4(g + 0x90, g + 0x90, D + 0x50);
    for s3 in 0..0x40u32 {
        let x = fx.rand_from(OT, 0x01E0_1114) >> 23;
        fx.mult(c, 0x6A0);
        fx.mem.write_u16(g + 8 + 2 * s3, rem2(x, 4) as u16);
        let ras = [[0x01E0_1148, 0x01E0_1180], [0x01E0_11DC, 0x01E0_1214], [0x01E0_1270, 0x01E0_12A8]];
        for (k, ra) in ras.into_iter().enumerate() {
            let r = fx.rand_from(OT, ra[0]) >> 23;
            let row = DIR_TABLE.wrapping_add((rem2(r, 4) * 0x180) as u32);
            let r = fx.rand_from(OT, ra[1]) >> 23;
            let f = fx.mem.f(row.wrapping_add((rem2(r, 32) * 12) as u32) + 4 * k as u32);
            fx.mult(c, 0x6A0);
            fx.mem.setf(g + 0xA0 + 0x10 * s3 + 4 * k as u32, ee::mul(f32::from_bits(0x3E4C_CCCD), f));
        }
        fx.mem.write_u32(g + 0xAC + 0x10 * s3, 0x3F80_0000);
    }
    for k in 0..0x20u32 {
        fx.mult(c, 0x6A0);
        fx.random_table_vec(g + 0x4A0 + 0x10 * k, 2.5);
    }
    let c = fx.mem.i32(D + 0x33C0) + 1;
    fx.mem.set32(D + 0x33C0, c);
    if c >= 0x18 {
        fx.mem.write_u32(D + 0x33C0, 0);
    }
}

/// x / 24 as compiled (the multiply by 0x2AAAAAAB, then >> 2).
fn div24(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 2).wrapping_add(((x as u32) >> 31) as i32)
}

/// The ring sets [F 0x1E013C8].
fn sets(fx: &mut Fx, sp: u32) {
    let d = sp + SPRITE;
    for i in 0..0x18u32 {
        let g = SETS + 0x6A0 * i;
        if fx.mem.read_u8(g) == 0 {
            continue;
        }
        sprite_desc(fx, sp);
        let rh = fx.mem.read_u32(D + 0x14);
        fx.desc_texture(d, rh, 1);
        fx.desc_clamp(d, 5);
        camera(fx, sp);
        let age = ee::from_i32(fx.mem.i32(g + 4));
        let f2 = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), age), 24.0);
        let s = ee::mul(4.0, ee::add(f32::from_bits(0x3F4C_CCCD), f2));
        corners(fx, sp, ee::neg(s), s);
        let age = fx.mem.i32(g + 4);
        let v1 = 0x18 - age;
        let a1 = div24(fx, 255 * v1) as u8;
        let a0 = div24(fx, 116 * v1) as u8;
        for k in 0..0x40u32 {
            let age = fx.mem.i32(g + 4);
            let mut f21 = fx.tan_falloff_p(age, 0x18, 1);
            if k % 2 == 0 {
                f21 = ee::mul(f21, f32::from_bits(0x3F4C_CCCD));
                let a = fx.alpha_preset(2);
                fx.desc_alpha(d, a);
                colour(fx, sp + 0x3E0, [0x80, 0x80, 0x80], a0);
            } else {
                let a = fx.alpha_preset(1);
                fx.desc_alpha(d, a);
                colour(fx, sp + 0x3E0, [0x40, 0x80, 0x30], a1);
            }
            fx.vu_scale4(sp + 0x300, g + 0xA0 + 0x10 * k, f21);
            fx.vu_add4(sp + 0x300, sp + 0x300, g + 0x90);
            fx.vu_add4(sp + 0x2E0, sp + 0x2C0, sp + 0x300);
            fx.vu_add4(sp + 0x2F0, sp + 0x2D0, sp + 0x300);
            let c = u32::from(fx.mem.read_u16(g + 8 + 2 * k));
            cell_uvs(fx, sp, c);
            if crate::fxlib2::project(fx, sp + 0x2E0, PROJ_OUT, sp + 0x200, d, sp + 0x42C) {
                let w = fx.mem.f(sp + 0x42C);
                let key = fix(fx, w, 0x01E0_1784);
                fx.submit(d, OT, key as i32);
            }
        }
        fx.camera_vp_to(sp + 0x200);
        fx.translate_rows(sp + 0x240, sp + 0x280, g + 0x90);
        let age = fx.mem.i32(g + 4);
        let tf = fx.tan_falloff_p(age, 0x18, 1);
        let s = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), tf));
        vec4(fx, sp + 0x300, [s, s, s, 1.0]);
        fx.scale_rows(sp + 0x240, sp + 0x300);
        crate::blib::exe::mul_mem(fx, sp + 0x200, sp + 0x240);
        let age = fx.mem.i32(g + 4);
        let f1 = ee::div(ee::from_i32(0x18 - age), 24.0);
        let a = fix(fx, ee::mul(196.0, f1), 0x01E0_1874);
        colour(fx, sp + 0x398, [0xB4, 0xFF, 0xA4], a as u8);
        colour(fx, sp + 0x3A8, [0x20, 0xFF, 0x20], 0);
        words(fx, sp + 0x3B8, sp + 0x3A8, 2);
        fan(fx, sp, D + 0xD2D0, 0x10, false, 0x00FF_FFFF, 0x01E0_1918);
        sprite_desc(fx, sp);
        let rh = fx.mem.read_u32(D + 0x18);
        fx.desc_texture(d, rh, 1);
        fx.desc_clamp(d, 5);
        let a = fx.alpha_preset(1);
        fx.desc_alpha(d, a);
        for (a, v) in [(0x3E8, 0), (0x3EA, 0), (0x3F8, 0x800), (0x3FA, 0x800)] {
            uv(fx, sp + a, v);
        }
        camera(fx, sp);
        corners(fx, sp, -1.0, 1.0);
        let age = fx.mem.i32(g + 4);
        let f21 = fx.tan_falloff_p(age, 0x18, 1);
        for j in 0..0x20u32 {
            let p = g + 0x4A0 + 0x10 * j;
            for k in 0..8i32 {
                colour(fx, sp + 0x3E0, [0xFF, 0x60, 0x60], 0);
                let s = ee::add(f21, ee::mul(f32::from_bits(0x3BE5_6042), ee::from_i32(k)));
                fx.vu_scale4(sp + 0x300, p, s);
                fx.vu_add4(sp + 0x300, sp + 0x300, g + 0x90);
                sprite_draw(fx, sp, 0x01E0_1B70);
            }
        }
        let age = fx.mem.i32(g + 4) + 1;
        fx.mem.set32(g + 4, age);
        if age >= 0x19 {
            fx.mem.write_u8(g, 0);
        }
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let mut rg = Regs { s2: param as i64 as u64, ..Regs::default() };
    fx.camera_turn_roll(sp + 0x280);
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x1C);
    fx.node_position(h, s, D + 0x20);
    let h = fx.mem.i32(D + 4);
    let s = fx.mem.i32(D + 0x30);
    fx.node_position(h, s, D + 0x40);
    let n = fx.mem.i32(D);
    let (_, m) = fx.div(n, 3);
    if m == 0 && (0x42..0xDB).contains(&n) {
        let c = fx.mem.read_u32(D + 0x60);
        let e = FLASHES + 0x20 * c;
        fx.mem.write_u8(e, 1);
        fx.mem.write_u32(e + 4, 0);
        words(fx, e + 0x10, D + 0x20, 4);
        let c = fx.mem.i32(D + 0x60) + 1;
        fx.mem.set32(D + 0x60, c);
        if c >= 0xA {
            fx.mem.write_u32(D + 0x60, 0);
        }
    }
    let t = sp + TRI;
    fx.mem.write_u16(t + 2, 0x33);
    fx.mem.write_u16(t, 9);
    fx.mem.write_u64(t + 8, 0x4_2142_1421);
    fx.mem.write_u32(t + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(t, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(t, rh, 1);
    fx.desc_clamp(t, 0x0000_01FC_012B_C8FA);
    fan_desc(fx, sp);
    flashes(fx, sp, &mut rg);
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x28 {
        spawn_burst(fx, n);
    }
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    let (_, m) = fx.div(n, 3);
    if m == 0 && (0x89..0xDC).contains(&n) {
        spawn_set(fx);
    }
    fan_desc(fx, sp);
    sets(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x7E {
        ret = 1;
    } else {
        if (0x89..0xDC).contains(&n) {
            ret = 6;
        }
        if n == 0xDC {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xF5 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xE0, ret as u32);
    ret
}
