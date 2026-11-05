//! BMODULE b214 (card 118), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xAB50
//! bytes) [F]: a single unscheduled function in
//! a 0x620 frame (D kept at sp + 0xBC, s0 the OT, the return at sp +
//! 0xC0), the b014 / b030 family. Every update the camera turn with roll
//! (sp + 0x4A0), nodes 0xA / 0x3E (D+0x40 / 0x50) and 0x14 / 0x15 / 0x16
//! / 0x2C / 0x2D / 0x2E (D+0x80 + 0x10 k). On updates 15 c + 12 (c < 11)
//! an emitter (D+0xF0 + 0x20 c) at node 0xA / 0x3E alternately, for 0x80
//! updates leaving a 16-slot ring of two-sprite puffs (D+0x250 + 0x300 c);
//! on 1..0x18F three sprite clouds at the six nodes (D+0x2350 + 0x200 k,
//! D+0x6350 + 0x140 k, 32 round robin) and on 0xAE..0x17B a far-Z one
//! (D+0x8B50 + 0x140 k, 16); from 0x104 for 0x8E updates a 24 x 4 x 16
//! mesh ring (texture D+0x18) of the grid D+0x9F50 (5 x 17 points)
//! about the target's point, alphas from D+0xA9F8. The stage light 0x80
//! - 64 clamp(0x1BA - n) clamp(n) / 1024. Returns 1 on 0xFA, 6 on
//! 0x105..0x191, 2 on 0x192, 8 once the count reaches 0x1BB.
use crate::blib::exe::mul_mem;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x620;
/// The sprite descriptor (prim 0x26 | 0x800, 5 registers).
const DESC: u32 = 0x530;
/// The mesh strip descriptor (prim 0x34, 12 registers).
const STRIP: u32 = 0x588;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR: u64 = 0xFF_FFF8;
const GRID: u32 = D + 0x9F50;
const ALPHAS: u32 = D + 0xA9F8;
const MESH: u32 = D + 0xA9F0;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
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

fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The 32 x 32 atlas cell of byte `b` (4 across) into the sprite's UVs.
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    uv(fx, sp + 0x548, ((c << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x54A, ((r << 9) & 0x3FFF) as u32);
    uv(fx, sp + 0x558, (((32 * c + 32) << 4) & 0x3FFF) as u32);
    uv(fx, sp + 0x55A, (((32 * r + 32) << 4) & 0x3FFF) as u32);
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

/// The sprite descriptor (texture D+`tex`, ALPHA preset `a`, REGION_CLAMP
/// 5) and the camera's view-projection (x identity) at sp + 0x420.
fn sprites(fx: &mut Fx, sp: u32, tex: u32, a: i32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    let p = fx.alpha_preset(a);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, 5);
    fx.camera_vp_to(sp + 0x420);
    fx.identity(sp + 0x460);
    mul_mem(fx, sp + 0x420, sp + 0x460);
}

/// The corners -+`s` at sp + 0x4E0 / 0x4F0 turned by the camera.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    vec4(fx, sp + 0x4E0, [n, n, 0.0, 1.0]);
    vec4(fx, sp + 0x4F0, [s, s, 0.0, 1.0]);
    fx.vu_apply_to(sp + 0x4E0, sp + 0x4A0, sp + 0x4E0);
    fx.vu_apply_to(sp + 0x4F0, sp + 0x4A0, sp + 0x4F0);
}

/// The sprite at `dir` (x `k` in x / z, + `up` in y) from `pt`, projected
/// (0x204F70): drawn at the mean w, or (`far`) at the far Z with key 0.
#[allow(clippy::too_many_arguments)]
fn sprite(fx: &mut Fx, sp: u32, dir: u32, k: f32, up: f32, pt: u32, far: bool, ra: u32) {
    let x = ee::mul(fx.mem.f(dir), k);
    fx.mem.setf(sp + 0x520, x);
    let y = ee::add(up, fx.mem.f(dir + 4));
    fx.mem.setf(sp + 0x524, y);
    let z = ee::mul(fx.mem.f(dir + 8), k);
    fx.mem.setf(sp + 0x528, z);
    fx.vu_add4(sp + 0x520, sp + 0x520, pt);
    fx.vu_add4(sp + 0x500, sp + 0x4E0, sp + 0x520);
    fx.vu_add4(sp + 0x510, sp + 0x4F0, sp + 0x520);
    if crate::fxlib2::project(fx, sp + 0x500, PROJ_OUT, sp + 0x420, sp + DESC, sp + 0x61C) {
        if far {
            far_z(fx, sp + 0x550, &[sp + 0x560]);
            fx.submit(sp + DESC, OT, 0);
        } else {
            let w = fx.mem.f(sp + 0x61C);
            let key = fixu(fx, w, ra);
            fx.submit(sp + DESC, OT, key as i32);
        }
    }
}

fn far_z(fx: &mut Fx, first: u32, others: &[u32]) {
    let v = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (v & KEEP) | (FAR << 32));
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    for &o in others {
        let w = fx.mem.read_u64(o);
        fx.mem.write_u64(o, (w & KEEP) | z);
    }
}

fn init(fx: &mut Fx) -> i32 {
    fx.bzero(D, 0xAB50);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..3u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x20);
    for (at, node, clear) in [(0x30u32, 0xA, 1), (0x34, 0x3E, 0), (0x60, 0x14, 0), (0x64, 0x15, 0), (0x68, 0x16, 0), (0x6C, 0x2C, 0), (0x70, 0x2D, 0), (0x74, 0x2E, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    // The grid: 5 rows of 17 points (two layers) and their alphas.
    for r in 0..5u32 {
        let f20 = ee::div(ee::mul(140.0, ee::from_i32(r as i32)), 4.0);
        for j in 0..0x11u32 {
            let p = GRID + 0x220 * r + 0x20 * j;
            let f21 = ee::sub(ee::div(ee::mul(70.0, ee::from_i32(j as i32)), 16.0), 35.0);
            let s = fx.tsin((((0x20000 * j) >> 4) as i32 - 0x4000) & 0xFFFF);
            let f22 = ee::add(f20, ee::mul(7.0, s));
            let s = fx.tsin(((0x10000 * j) >> 4) as i32);
            let z = ee::mul(4.0, s);
            vec4(fx, p, [f21, f22, z, 1.0]);
            vec4(fx, p + 0x10, [f21, f22, ee::neg(z), 1.0]);
            let s = fx.tsin(((0x8000 * j) >> 4) as i32);
            let c = fx.powf(s, 3.0);
            let a = ee::div(ee::mul(ee::from_i32(4 - r as i32), c), 4.0);
            fx.mem.setf(ALPHAS + 0x44 * r + 4 * j, a);
        }
    }
    0
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x4A0);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30 + 4 * k));
        fx.node_position(h, slot, D + 0x40 + 0x10 * k);
    }
    for k in 0..6u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x60 + 4 * k));
        fx.node_position(h, slot, D + 0x80 + 0x10 * k);
    }
    let c = fx.mem.i32(D + 0xE0);
    if fx.mem.i32(D) == 15 * c + 12 && c < 0xB {
        let e = D + 0x20 * c as u32;
        fx.mem.write_u8(e + 0xF0, 1);
        fx.mem.write_u32(e + 0xF4, 0);
        words(fx, e + 0x100, D + 0x40 + 0x10 * rem2(c + 1, 2) as u32, 4);
        fx.mem.set32(D + 0xE0, c + 1);
    }
    for i in 0..0xBu32 {
        let e = D + 0x20 * i;
        if fx.mem.read_u8(e + 0xF0) == 0 {
            continue;
        }
        let age = fx.mem.i32(e + 0xF4);
        let f = ee::div(ee::from_i32(Fx::clamp_i(0x80 - age, 0, 0x20)), 32.0);
        let age = fx.mem.i32(e + 0xF4);
        let s = D + 0x300 * i + 0x30 * rem2(age, 0x10) as u32;
        fx.mem.write_u8(s + 0x250, 1);
        fx.mem.write_u32(s + 0x254, 0);
        fx.mem.setf(s + 0x25C, f);
        for t in 0..2u32 {
            let r = fx.rand_from(OT, 0x01E0_0688) >> 23;
            fx.mem.set8(s + 0x258 + t, rem2(r, 8));
            fx.random_table_vec(s + 0x260 + 0x10 * t, f32::from_bits(0x3DCC_CCCD));
        }
        let age = fx.mem.i32(e + 0xF4) + 1;
        fx.mem.set32(e + 0xF4, age);
        if age >= 0x81 {
            fx.mem.write_u8(e + 0xF0, 0);
        }
    }
    puffs(fx, sp);
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x190 {
        let g = D + 0x200 * rem2(n, 0x20) as u32;
        fx.mem.write_u8(g + 0x2350, 1);
        fx.mem.write_u32(g + 0x2354, 0);
        for m in 0..6u32 {
            words(fx, g + 0x2370 + 0x10 * m, D + 0x80 + 0x10 * m, 4);
            for t in 0..4u32 {
                let r = fx.rand_from(OT, 0x01E0_0BC0) >> 23;
                fx.mem.set8(g + 0x2358 + 4 * m + t, rem2(r, 4));
                fx.random_table_vec(g + 0x23D0 + 0x40 * m + 0x10 * t, f32::from_bits(0x3E99_999A));
            }
        }
    }
    cloud(fx, sp, &CLOUD_A);
    let n = fx.mem.i32(D);
    if n > 0 && n < 0x190 {
        let g = D + 0x140 * rem2(n, 0x20) as u32;
        spawn2(fx, g + 0x6350, 0x01E0_1110, f32::from_bits(0x3E99_999A));
    }
    cloud(fx, sp, &CLOUD_B);
    let n = fx.mem.i32(D);
    if (0xAE..0x17C).contains(&n) {
        let g = D + 0x140 * rem2(n, 0x10) as u32;
        spawn2(fx, g + 0x8B50, 0x01E0_1670, f32::from_bits(0x3E4C_CCCD));
    }
    cloud(fx, sp, &CLOUD_C);
    if fx.mem.i32(D) == 0x104 {
        fx.mem.write_u8(MESH, 1);
        fx.mem.write_u32(MESH + 4, 0);
    }
    if fx.mem.read_u8(MESH) == 1 {
        mesh(fx, sp);
    }
    let n = fx.mem.i32(D);
    let a = Fx::clamp_i(0x1BA - n, 0, 0x20);
    let n = fx.mem.i32(D);
    let b = Fx::clamp_i(n, 0, 0x20);
    let m = fx.mult(a, b.wrapping_neg() << 6);
    fx.set_stage_light(((m >> 10) + 0x80) & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xFA {
        ret = 1;
    } else {
        if (0x105..0x192).contains(&n) {
            ret = 6;
        }
        if n == 0x192 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x1BB {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// A two-sprite cloud record at `g` (+0 on, +4 age, +8 bytes, +0x20 the
/// six node points, +0x80 the directions).
fn spawn2(fx: &mut Fx, g: u32, ra: u32, len: f32) {
    fx.mem.write_u8(g, 1);
    fx.mem.write_u32(g + 4, 0);
    for m in 0..6u32 {
        words(fx, g + 0x20 + 0x10 * m, D + 0x80 + 0x10 * m, 4);
        for t in 0..2u32 {
            let r = fx.rand_from(OT, ra) >> 23;
            fx.mem.set8(g + 8 + 2 * m + t, rem2(r, 8));
            fx.random_table_vec(g + 0x80 + 0x20 * m + 0x10 * t, len);
        }
    }
}

/// The emitters' puffs [F 0x1E00704].
fn puffs(fx: &mut Fx, sp: u32) {
    sprites(fx, sp, 0x10, 1);
    for i in 0..0xBu32 {
        let pt = D + 0x20 * i + 0x100;
        for j in 0..0x10u32 {
            let s = D + 0x300 * i + 0x30 * j;
            if fx.mem.read_u8(s + 0x250) == 0 {
                continue;
            }
            let age = fx.mem.i32(s + 0x254);
            let c = fx.tsin((age << 15) >> 4);
            let k = fx.mem.f(s + 0x25C);
            let al = fixu(fx, ee::mul(k, ee::mul(164.0, c)), 0x01E0_0818);
            rgba(fx, sp + 0x540, [0xFF, 0x36, 0x30], al as u8);
            let age = fx.mem.i32(s + 0x254);
            let h = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), ee::from_i32(0x10 - age)), 16.0));
            corners(fx, sp, ee::mul(3.0, h));
            let age = fx.mem.i32(s + 0x254);
            let f4 = ee::div(ee::from_i32(age), 16.0);
            let f21 = ee::add(f32::from_bits(0x3F4C_CCCD), ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(0x10 - age)), 16.0));
            let f20 = ee::add(-1.0, ee::mul(8.0, f4));
            for t in 0..2u32 {
                let b = i32::from(fx.mem.read_u8(s + 0x258 + t));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, s + 0x260 + 0x10 * t, f21, f20, pt, false, 0x01E0_0A6C);
            }
            let age = fx.mem.i32(s + 0x254) + 1;
            fx.mem.set32(s + 0x254, age);
            if age >= 0x11 {
                fx.mem.write_u8(s + 0x250, 0);
            }
        }
    }
}

/// A sprite cloud's layout and look.
struct Cloud {
    base: u32,
    stride: u32,
    count: u32,
    /// Per node: the sprites, the byte / direction strides.
    per: u32,
    bytes: u32,
    dirs: u32,
    pts: u32,
    tex: u32,
    alpha: i32,
    /// The life (age shift of the colour sine: 4 for 16, 5 for 32).
    life: i32,
    amp: f32,
    colour: [u8; 3],
    /// The window of the fade (clamp(n - a) clamp(b - n) / 1024, or
    /// clamp(b - n) / 32 with no a).
    from: Option<i32>,
    until: i32,
    /// The size (k, rising: by age, else by the life left) and its scale.
    rising: bool,
    scale: f32,
    /// y rise per life fraction (f21 = rise x age / life + lift) and the
    /// spread 0.7 + 0.3 age / life.
    rise: f32,
    lift: f32,
    far: bool,
    ra_alpha: u32,
    ra_key: u32,
}

const CLOUD_A: Cloud = Cloud { base: 0x2350, stride: 0x200, count: 0x20, per: 4, bytes: 0x2358, dirs: 0x23D0, pts: 0x2370, tex: 0x14, alpha: 1, life: 5, amp: 164.0, colour: [0xFF, 0x36, 0x30], from: None, until: 0x190, rising: false, scale: 1.0, rise: 10.0, lift: 0.0, far: false, ra_alpha: 0x01E0_0D58, ra_key: 0x01E0_0FA4 };
const CLOUD_B: Cloud = Cloud { base: 0x6350, stride: 0x140, count: 0x20, per: 2, bytes: 0x6358, dirs: 0x63D0, pts: 0x6370, tex: 0x10, alpha: 0, life: 5, amp: 128.0, colour: [0xC8, 0xC8, 0xC8], from: None, until: 0x190, rising: true, scale: 2.0, rise: 13.0, lift: 0.0, far: false, ra_alpha: 0x01E0_12A8, ra_key: 0x01E0_14EC };
const CLOUD_C: Cloud = Cloud { base: 0x8B50, stride: 0x140, count: 0x10, per: 2, bytes: 0x8B58, dirs: 0x8BD0, pts: 0x8B70, tex: 0x10, alpha: 1, life: 4, amp: 128.0, colour: [0xFF, 0x40, 0x2E], from: Some(0xAD), until: 0x17C, rising: false, scale: 4.0, rise: 10.0, lift: -2.0, far: true, ra_alpha: 0x01E0_1850, ra_key: 0 };

/// A sprite cloud [F 0x1E00C50 / 0x1E011A0 / 0x1E01710].
fn cloud(fx: &mut Fx, sp: u32, c: &Cloud) {
    sprites(fx, sp, c.tex, c.alpha);
    let n = fx.mem.i32(D);
    let f20 = match c.from {
        Some(a) => {
            let x = ee::from_i32(Fx::clamp_i(n - a, 0, 0x20));
            let n = fx.mem.i32(D);
            let y = ee::from_i32(Fx::clamp_i(c.until - n, 0, 0x20));
            ee::div(ee::mul(x, y), 1024.0)
        }
        None => ee::div(ee::from_i32(Fx::clamp_i(c.until - n, 0, 0x20)), 32.0),
    };
    let life = 1 << c.life;
    let lf = ee::from_i32(life);
    for k in 0..c.count {
        let g = D + c.stride * k;
        if fx.mem.read_u8(g + c.base) == 0 {
            continue;
        }
        let age = fx.mem.i32(g + c.base + 4);
        let s = fx.tsin((age << 15) >> c.life);
        let al = fixu(fx, ee::mul(f20, ee::mul(c.amp, s)), c.ra_alpha);
        rgba(fx, sp + 0x540, c.colour, al as u8);
        let age = fx.mem.i32(g + c.base + 4);
        let t = if c.rising { ee::from_i32(age) } else { ee::from_i32(life - age) };
        let h = ee::add(f32::from_bits(0x3E99_999A), ee::div(ee::mul(f32::from_bits(0x3F33_3333), t), lf));
        corners(fx, sp, if c.scale == 1.0 { h } else { ee::mul(c.scale, h) });
        let age = ee::from_i32(fx.mem.i32(g + c.base + 4));
        let f3 = ee::div(age, lf);
        let f2 = ee::div(ee::mul(f32::from_bits(0x3E99_999A), age), lf);
        let up = if c.lift == 0.0 { ee::mul(c.rise, f3) } else { ee::add(c.lift, ee::mul(c.rise, f3)) };
        let k_ = ee::add(f32::from_bits(0x3F33_3333), f2);
        for m in 0..6u32 {
            let pt = g + c.pts + 0x10 * m;
            for t in 0..c.per {
                let b = i32::from(fx.mem.read_u8(g + c.bytes + c.per * m + t));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, g + c.dirs + 0x10 * (c.per * m + t), k_, up, pt, c.far, c.ra_key);
            }
        }
        let age = fx.mem.i32(g + c.base + 4) + 1;
        fx.mem.set32(g + c.base + 4, age);
        if age >= life + 1 {
            fx.mem.write_u8(g + c.base, 0);
        }
    }
}

/// The mesh ring [F 0x1E01C80..0x1E02320].
fn mesh(fx: &mut Fx, sp: u32) {
    let d = sp + STRIP;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    fx.desc_clamp(d, 0);
    let age = fx.mem.i32(MESH + 4);
    let a = ee::from_i32(Fx::clamp_i(age, 0, 0x20));
    let age = fx.mem.i32(MESH + 4);
    let b = ee::from_i32(Fx::clamp_i(0x8E - age, 0, 0x20));
    let f20 = ee::mul(128.0, ee::div(ee::mul(a, b), 1024.0));
    for c in 0..0x18i32 {
        fx.camera_vp_to(sp + 0x420);
        fx.identity(sp + 0x460);
        fx.mult(0x2AAA_AAAB, 0x30000 * c);
        let ang = ((0x30000 * c) / 24) & 0xFFFF;
        let s = fx.tsin(ang);
        fx.mem.setf(sp + 0x520, ee::mul(8.0, s));
        fx.mem.write_u32(sp + 0x524, 0);
        let co = fx.tcos(ang);
        fx.mem.setf(sp + 0x528, ee::mul(8.0, co));
        fx.mem.write_u32(sp + 0x52C, 0x3F80_0000);
        fx.vu_add4(sp + 0x520, sp + 0x520, D + 0x20);
        let y = ee::add(fx.mem.f(sp + 0x524), f32::from_bits(0xC23A_AAAB));
        fx.mem.setf(sp + 0x524, y);
        fx.translate_rows(sp + 0x460, sp + 0x460, sp + 0x520);
        fx.mult(0x2AAA_AAAB, 0x10000 * c);
        let turn = (0x10000 * c) / 24 + if c < 0xC { 0 } else { 0xAAA };
        fx.rotate_axis(sp + 0x460, turn, 1);
        mul_mem(fx, sp + 0x420, sp + 0x460);
        fx.mult(0x2AAA_AAAB, 0xC * c);
        let ofs = (0xC * c) / 24 + 0x18;
        let par = (c & 1) as u32;
        for r in 0..4u32 {
            let age = fx.mem.i32(MESH + 4);
            let (_, h) = fx.div(age, ofs);
            let f1 = ee::div(ee::mul(3.0, ee::from_i32(r as i32)), 4.0);
            let f0 = ee::div(ee::mul(3.0, ee::from_i32(r as i32 + 1)), 4.0);
            let f2 = ee::div(ee::mul(2.0, ee::from_i32(h)), ee::from_i32(ofs));
            let f21 = ee::sub(f1, f2);
            let f22 = ee::sub(f0, f2);
            for j in 0..0x10u32 {
                let al = ALPHAS + 0x44 * r + 4 * j;
                if r == 0 {
                    rgba(fx, sp + 0x598, [0xFF, 0x5A, 0x40], 0);
                    rgba(fx, sp + 0x5B0, [0xFF, 0x5A, 0x40], 0);
                } else {
                    let w = fx.mem.f(al);
                    let a = fixu(fx, ee::mul(f20, w), 0x01E0_1FF0);
                    rgba(fx, sp + 0x598, [0xFF, 0x5A, 0x40], a as u8);
                    let w = fx.mem.f(al + 4);
                    let a = fixu(fx, ee::mul(f20, w), 0x01E0_2028);
                    rgba(fx, sp + 0x5B0, [0xFF, 0x5A, 0x40], a as u8);
                }
                let w = fx.mem.f(al + 0x44);
                    let a = fixu(fx, ee::mul(f20, w), 0x01E0_2064);
                rgba(fx, sp + 0x5C8, [0xFF, 0x5A, 0x40], a as u8);
                let w = fx.mem.f(al + 0x48);
                    let a = fixu(fx, ee::mul(f20, w), 0x01E0_209C);
                rgba(fx, sp + 0x5E0, [0xFF, 0x5A, 0x40], a as u8);
                let s0 = ee::div(ee::mul(3.0, ee::from_i32(j as i32)), 16.0);
                let s1 = ee::div(ee::mul(3.0, ee::from_i32(j as i32 + 1)), 16.0);
                for (o, v) in [(0x5A0u32, s0), (0x5A4, f21), (0x5B8, s1), (0x5BC, f21), (0x5D0, s0), (0x5D4, f22), (0x5E8, s1), (0x5EC, f22)] {
                    fx.mem.setf(sp + o, v);
                }
                let v = |rr: u32, jj: u32| GRID + 0x220 * rr + 0x20 * jj + 0x10 * par;
                if fx.project_n(&[v(r, j), v(r, j + 1), v(r + 1, j), v(r + 1, j + 1)], sp + 0x420, d, sp + 0x61C, 0x01E0_216C) == 1 {
                    far_z(fx, sp + 0x5A8, &[sp + 0x5C0, sp + 0x5D8, sp + 0x5F0]);
                    fx.submit(d, OT, 0);
                }
            }
        }
    }
    let age = fx.mem.i32(MESH + 4) + 1;
    fx.mem.set32(MESH + 4, age);
    if age >= 0x8F {
        fx.mem.write_u8(MESH, 0);
    }
}
