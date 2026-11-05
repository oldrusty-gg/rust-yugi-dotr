//! BMODULE b188 (card 240), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xA0D0
//! bytes) [F]: a single unscheduled function in a
//! 0x550 frame (s0 = the ordering table). Every update the camera turn
//! with roll (0x2055E0 into sp + 0x3B0), the attacker's node 0x19 (D+0x30,
//! z - 10 x its side) and nodes 0x2A / 0xD (D+0x50 / 0x60). On updates
//! 0x7B / 0xAD / 0xDF one of four heads (D+0x90, 0x30, 35 updates) leaves
//! the node towards a random point round the target (y kept), with a
//! ring of 16 sparks (D+0x4960, 0x130) at the node and a ribbon record
//! (D+0x6FB0, 0x830: 2 x 32 quads at nodes 0x2A / 0xD, random turns,
//! lives and directions); a head (sprite 6) leaves every update a puff
//! (D+0x150, 16 a head, 16 updates, 4 sprites thrown 0.3) and a smoke
//! (D+0x1950, 32 a head, 32 updates, 4 sprites thrown 0.45), and on age
//! 0x22 a burst (D+0x4E20, 0x860, 64 updates: 64 + 64 sprites thrown 2).
//! On updates 0x12 / 0x32 / 0x52 more ribbon records. The ring: a glow
//! (100, far Z, 128 (1 - falloff)) and 16 sprites (15 (0.5 + 0.5 age /
//! 32)). Ribbons: textured quads (1.4 x 0.7, ST 0..1) at the node plus
//! the direction x (0.1 + 0.9 age / 32), turned by their angles plus
//! (111, 222, 333) x +-(j / 2 + 1) x age, alpha 128 min(age / 3, 1) (1 -
//! min(age / life, 1)). The stage light 0x80 - 96 clamp(n) clamp(0x161 -
//! n) / 1024. Returns 1 on 0x93, 6 on 0x9E..0x120, 2 on 0x121, 8 once the
//! count reaches 0x162.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x550);
    fx.mem.write_u32(sp + 0xC0, D);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp) };
    fx.leave(0x550);
    ret
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xA0D0);
    fx.mem.write_u32(D, 0);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    let s = fx.unit_side(h);
    fx.mem.set32(D + 0xC, s);
    for k in 0..4u32 {
        let r = fx.texture(5, k as i32);
        fx.mem.write_u32(D + 0x10 + 4 * k, r);
    }
    for (at, node, clear) in [(0x20u32, 0x19, 1), (0x40, 0x2A, 0), (0x44, 0xD, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x70);
    fx.mem.write_u32(D + 0x80, 0);
    fx.mem.write_u32(D + 0x84, 0);
    for k in 0..4u32 {
        let r = D + 0x30 * k;
        fx.mem.write_u8(r + 0x90, 0);
        fx.mem.write_u32(r + 0x98, 0);
        fx.mem.write_u32(r + 0x9C, 0);
    }
    for h in 0..4u32 {
        for j in 0..0x10u32 {
            fx.mem.write_u8(D + 0x600 * h + 0x60 * j + 0x150, 0);
        }
    }
    for h in 0..4u32 {
        for j in 0..0x20u32 {
            fx.mem.write_u8(D + 0xC00 * h + 0x60 * j + 0x1950, 0);
        }
    }
    fx.mem.write_u32(D + 0x4950, 0);
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x130 * k + 0x4960, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x860 * k + 0x4E20, 0);
    }
    fx.mem.write_u32(D + 0x6FA0, 0);
    fx.mem.write_u32(D + 0x6FA4, 0);
    for k in 0..6u32 {
        fx.mem.write_u8(D + 0x830 * k + 0x6FB0, 0);
    }
    // The clear loop steps the stack copy of a0.
    fx.mem.write_u32(sp + 0xC0, D + 6 * 0x830);
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

fn fixs(fx: &mut Fx, f: f32, ra: u32) -> i32 {
    fx.fix_residue(f, ra);
    ee::fixsfsi(f)
}

fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x478, u0);
    uv(fx, sp + 0x47A, v0);
    uv(fx, sp + 0x488, u1);
    uv(fx, sp + 0x48A, v1);
}

/// The sprite cell `b` (4 x n cells of 32).
fn cell_uvs(fx: &mut Fx, sp: u32, b: i32) {
    let (c, r) = (rem2(b, 4), b >> 2);
    let m = |x: i32| (x & 0x3FFF) as u32;
    uvs(fx, sp, m(c << 9), m(r << 9), m((32 * c + 32) << 4), m((32 * r + 32) << 4));
}

fn rgba(fx: &mut Fx, at: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(at, c[0]);
    fx.mem.write_u8(at + 1, c[1]);
    fx.mem.write_u8(at + 2, c[2]);
    fx.mem.write_u8(at + 3, a);
    fx.mem.write_u32(at + 4, 0x3F80_0000);
}

const WHITE: [u8; 3] = [0xFF, 0xE8, 0xE0];

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// The sprite descriptor at sp + 0x460 (5 registers 0x42421, prim 0x26 |
/// 0x800).
fn head(fx: &mut Fx, sp: u32) {
    let d = sp + 0x460;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// Alpha preset 1, the texture at D + `tex`, CLAMP 5.
fn textured(fx: &mut Fx, sp: u32, tex: u32) {
    let d = sp + 0x460;
    let p = fx.alpha_preset(1);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The view-projection (x identity) at sp + 0x330.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x330);
    fx.identity(sp + 0x370);
    let id = fx.read_m4(sp + 0x370);
    fx.mul_into(Some(sp + 0x330), &id);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) at sp + 0x3F0 / 0x400 turned
/// by the camera (sp + 0x3B0).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x3F0, n);
    fx.mem.setf(sp + 0x3F4, n);
    fx.mem.write_u32(sp + 0x3F8, 0);
    fx.mem.write_u32(sp + 0x3FC, 0x3F80_0000);
    fx.mem.setf(sp + 0x400, s);
    fx.mem.setf(sp + 0x404, s);
    fx.mem.write_u32(sp + 0x408, 0);
    fx.mem.write_u32(sp + 0x40C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x3F0, sp + 0x3B0, sp + 0x3F0);
    fx.vu_apply_to(sp + 0x400, sp + 0x3B0, sp + 0x400);
}

/// k (base + 0.5 / slope x age / d).
fn size(k: f32, base: u32, slope: u32, age: i32, d: f32) -> f32 {
    let t = ee::div(ee::mul(f32::from_bits(slope), ee::from_i32(age)), d);
    ee::mul(k, ee::add(f32::from_bits(base), t))
}

/// The corners about sp + 0x450 (sp + 0x430 / 0x440) projected (0x204F70)
/// and submitted at the mean w; true when drawn.
fn draw(fx: &mut Fx, sp: u32, ra: Option<u32>) -> bool {
    fx.vu_add4(sp + 0x430, sp + 0x3F0, sp + 0x450);
    fx.vu_add4(sp + 0x440, sp + 0x400, sp + 0x450);
    let d = sp + 0x460;
    if !crate::fxlib2::project(fx, sp + 0x430, PROJ_OUT, sp + 0x330, d, sp + 0x54C) {
        return false;
    }
    if let Some(ra) = ra {
        let w = fx.mem.f(sp + 0x54C);
        let key = fixu(fx, w, ra) as i32;
        fx.submit(d, OT, key);
    }
    true
}

/// A sprite of the direction `dir` x `f` about `pos`, drawn.
fn sprite(fx: &mut Fx, sp: u32, dir: u32, f: f32, pos: u32, ra: u32) {
    fx.vu_scale4(sp + 0x450, dir, f);
    fx.vu_add4(sp + 0x450, sp + 0x450, pos);
    draw(fx, sp, Some(ra));
}

/// 0.1 + 0.9 x the tan falloff (and kin).
fn spread(fx: &mut Fx, age: i32, n: i32, base: u32, slope: u32) -> f32 {
    let t = fx.tan_falloff_p(age, n, 1);
    ee::add(f32::from_bits(base), ee::mul(f32::from_bits(slope), t))
}

/// A ribbon record at D+0x6FB0 + 0x830 D[0x6FA4] [F 0x1E00598 /
/// 0x1E01FB8]: two groups (nodes 0x2A / 0xD) of 32 (three random angles,
/// a random life 32 (0.5 + rand / 512), a random direction of 2).
fn ribbon_spawn(fx: &mut Fx, ras: [u32; 5]) {
    let r = D + 0x830 * fx.mem.read_u32(D + 0x6FA4);
    fx.mem.write_u8(r + 0x6FB0, 1);
    fx.mem.write_u32(r + 0x6FB4, 0);
    for g in 0..2u32 {
        copy4(fx, r + 0x10 * g + 0x73C0, D + 0x50 + 0x10 * g);
        for j in 0..0x20u32 {
            let a = r + 0x180 * g + 0xC * j;
            for (k, ra) in ras[..3].iter().enumerate() {
                let b = fx.rand_from(OT, *ra) >> 23;
                fx.mem.set32(a + 0x6FB8 + 4 * k as u32, (rem2(b, 0x100) << 16) >> 8);
            }
            let b = fx.rand_from(OT, ras[3]) >> 23;
            let t = ee::add(0.5, ee::div(ee::mul(0.5, ee::from_i32(rem2(b, 0x100))), 256.0));
            let l = fixs(fx, ee::mul(32.0, t), ras[4]);
            fx.mem.set32(r + 0x80 * g + 4 * j + 0x72B8, l);
            fx.random_table_vec(r + 0x200 * g + 0x10 * j + 0x73E0, 2.0);
        }
    }
    let i = fx.mem.i32(D + 0x6FA4) + 1;
    fx.mem.set32(D + 0x6FA4, if i >= 6 { 0 } else { i });
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + 0x3B0);
    let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x20));
    fx.node_position(h, slot, D + 0x30);
    let side = ee::from_i32(fx.mem.i32(D + 0xC));
    let z = fx.mem.f(D + 0x38);
    fx.mem.setf(D + 0x38, ee::add(z, ee::mul(-10.0, side)));
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x40 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    let c = fx.mem.i32(D + 0x80);
    if c < 3 && fx.mem.i32(D) == 50 * c + 0x7B {
        launch(fx, sp);
    }
    heads(fx, sp);
    puffs(fx, sp);
    smokes(fx, sp);
    rings(fx, sp);
    bursts(fx, sp);
    let c = fx.mem.i32(D + 0x6FA0);
    if c < 3 && fx.mem.i32(D) == 32 * c + 0x12 {
        ribbon_spawn(fx, [0x01E0_2038, 0x01E0_2060, 0x01E0_2088, 0x01E0_20B0, 0x01E0_2100]);
        let c = fx.mem.i32(D + 0x6FA0) + 1;
        fx.mem.set32(D + 0x6FA0, c);
    }
    ribbons(fx, sp);

    let n = fx.mem.i32(D);
    let s0 = Fx::clamp_i(0x161 - n, 0, 0x20);
    let k = Fx::clamp_i(n, 0, 0x20);
    let l = ((k.wrapping_mul(-3) << 5).wrapping_mul(s0) >> 10) + 0x80;
    fx.set_stage_light(l & 0xFF);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x93 {
        ret = 1;
    } else if (0x9E..0x121).contains(&n) {
        ret = 6;
    }
    if n == 0x121 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x162 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}

/// A head and its ring and ribbon [F 0x1E003AC..0x1E00768].
fn launch(fx: &mut Fx, sp: u32) {
    let hd = D + 0x30 * fx.mem.read_u32(D + 0x84);
    fx.mem.write_u8(hd + 0x90, 1);
    fx.mem.write_u32(hd + 0x94, 0);
    let rg = D + 0x130 * fx.mem.read_u32(D + 0x4950);
    fx.mem.write_u8(rg + 0x4960, 1);
    fx.mem.write_u32(rg + 0x4964, 0);
    copy4(fx, hd + 0xA0, D + 0x30);
    copy4(fx, rg + 0x4970, D + 0x30);
    fx.random_table_vec(sp + 0x450, 1.0);
    fx.vu_add4(sp + 0x450, sp + 0x450, D + 0x70);
    let x = fx.mem.f(hd + 0xA0);
    fx.mem.setf(sp + 0x450, x);
    fx.vu_sub4(hd + 0xB0, sp + 0x450, hd + 0xA0);
    for j in 0..0x10u32 {
        let b = fx.rand_from(OT, 0x01E0_04D8) >> 23;
        fx.mem.set8(rg + 0x4980 + j, rem2(b, 8));
        fx.random_table_vec(rg + 0x4990 + 0x10 * j, f32::from_bits(0x3F19_999A));
    }
    let c = fx.mem.i32(D + 0x80) + 1;
    fx.mem.set32(D + 0x80, c);
    let i = fx.mem.i32(D + 0x84) + 1;
    fx.mem.set32(D + 0x84, if i >= 4 { 0 } else { i });
    let i = fx.mem.i32(D + 0x4950) + 1;
    fx.mem.set32(D + 0x4950, if i >= 4 { 0 } else { i });
    ribbon_spawn(fx, [0x01E0_0618, 0x01E0_0640, 0x01E0_0668, 0x01E0_0690, 0x01E0_06E0]);
}

/// The heads (sprite 6, 35 updates, along their direction by age / 34)
/// and what they leave [F 0x1E00768..0x1E00D10].
fn heads(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 0x14);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    fx.mem.setf(sp + 0x3F0, -6.0);
    fx.mem.setf(sp + 0x3F4, -6.0);
    fx.mem.write_u32(sp + 0x3F8, 0);
    fx.mem.write_u32(sp + 0x3FC, 0x3F80_0000);
    fx.mem.setf(sp + 0x400, 6.0);
    fx.mem.setf(sp + 0x404, 6.0);
    fx.mem.write_u32(sp + 0x408, 0);
    fx.mem.write_u32(sp + 0x40C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x3F0, sp + 0x3B0, sp + 0x3F0);
    fx.vu_apply_to(sp + 0x400, sp + 0x3B0, sp + 0x400);
    for k in 0..4u32 {
        let hd = D + 0x30 * k;
        if fx.mem.read_u8(hd + 0x90) == 0 {
            continue;
        }
        rgba(fx, sp + 0x470, WHITE, 0x80);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(hd + 0x94)), 34.0);
        let a = fx.mem.i32(hd + 0x98);
        fx.mem.set32(hd + 0x98, if a + 1 >= 0x10 { 0 } else { a + 1 });
        let b = fx.mem.i32(hd + 0x9C);
        fx.mem.set32(hd + 0x9C, if b + 1 >= 0x20 { 0 } else { b + 1 });
        let puff = D + 0x600 * k + 0x60 * a as u32;
        let smoke = D + 0xC00 * k + 0x60 * b as u32;
        fx.vu_scale4(sp + 0x450, hd + 0xB0, f20);
        fx.vu_add4(sp + 0x450, sp + 0x450, hd + 0xA0);
        draw(fx, sp, Some(0x01E0_0A80));
        fx.mem.write_u8(puff + 0x150, 1);
        fx.mem.write_u32(puff + 0x154, 0);
        copy4(fx, puff + 0x160, sp + 0x450);
        for j in 0..4u32 {
            let b = fx.rand_from(OT, 0x01E0_0AE8) >> 23;
            fx.mem.set8(puff + 0x158 + j, rem2(b, 8));
            fx.random_table_vec(puff + 0x170 + 0x10 * j, f32::from_bits(0x3E99_999A));
        }
        fx.mem.write_u8(smoke + 0x1950, 1);
        fx.mem.write_u32(smoke + 0x1954, 0);
        copy4(fx, smoke + 0x1960, sp + 0x450);
        for j in 0..4u32 {
            fx.random_table_vec(smoke + 0x1970 + 0x10 * j, f32::from_bits(0x3EE6_6666));
        }
        if fx.mem.i32(hd + 0x94) == 0x22 {
            let e = D + 0x860 * k;
            fx.mem.write_u8(e + 0x4E20, 1);
            fx.mem.write_u32(e + 0x4E24, 0);
            copy4(fx, e + 0x4E70, sp + 0x450);
            for j in 0..0x40u32 {
                let b = fx.rand_from(OT, 0x01E0_0BF0) >> 23;
                fx.mem.set8(e + 0x4E28 + j, rem2(b, 8));
                fx.random_table_vec(e + 0x4E80 + 0x10 * j, 2.0);
            }
            for j in 0..0x40u32 {
                fx.random_table_vec(e + 0x5280 + 0x10 * j, 2.0);
            }
        }
        let age = fx.mem.i32(hd + 0x94) + 1;
        fx.mem.set32(hd + 0x94, age);
        if age >= 0x23 {
            fx.mem.write_u8(hd + 0x90, 0);
        }
    }
}

/// The puffs (D+0x150, 16 updates) [F 0x1E00D10..0x1E010B0].
fn puffs(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 0x10);
    camera(fx, sp);
    for h in 0..4u32 {
        for j in 0..0x10u32 {
            let r = D + 0x600 * h + 0x60 * j;
            if fx.mem.read_u8(r + 0x150) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 0x154);
            corners(fx, sp, size(6.75, 0x3F33_3333, 0x3E99_999A, age, 16.0));
            let age = fx.mem.i32(r + 0x154);
            rgba(fx, sp + 0x470, WHITE, (((0x10 - age) << 7) >> 4) as u8);
            let age = fx.mem.i32(r + 0x154);
            let f20 = spread(fx, age, 0x10, 0x3ECC_CCCD, 0x3F19_999A);
            for i in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(r + 0x158 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + 0x170 + 0x10 * i, f20, r + 0x160, 0x01E0_1000);
            }
            let age = fx.mem.i32(r + 0x154) + 1;
            fx.mem.set32(r + 0x154, age);
            if age >= 0x11 {
                fx.mem.write_u8(r + 0x150, 0);
            }
        }
    }
}

/// The smokes (D+0x1950, 32 updates) [F 0x1E010B0..0x1E013EC].
fn smokes(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 0x18);
    camera(fx, sp);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    for h in 0..4u32 {
        for j in 0..0x20u32 {
            let r = D + 0xC00 * h + 0x60 * j;
            if fx.mem.read_u8(r + 0x1950) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 0x1954);
            corners(fx, sp, size(12.75, 0x3F00_0000, 0x3F00_0000, age, 32.0));
            let age = fx.mem.i32(r + 0x1954);
            let a = (i64::from((0x20 - age).wrapping_mul(3)) << 37) >> 37;
            rgba(fx, sp + 0x470, WHITE, a as u8);
            let age = fx.mem.i32(r + 0x1954);
            let f20 = spread(fx, age, 0x20, 0x3E4C_CCCD, 0x3F4C_CCCD);
            for i in 0..4u32 {
                sprite(fx, sp, r + 0x1970 + 0x10 * i, f20, r + 0x1960, 0x01E0_1350);
            }
            let age = fx.mem.i32(r + 0x1954) + 1;
            fx.mem.set32(r + 0x1954, age);
            if age >= 0x21 {
                fx.mem.write_u8(r + 0x1950, 0);
            }
        }
    }
}

/// The rings (D+0x4960, 32 updates): a far glow and 16 sprites [F
/// 0x1E013EC..0x1E019E0].
fn rings(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    camera(fx, sp);
    for k in 0..4u32 {
        let r = D + 0x130 * k;
        if fx.mem.read_u8(r + 0x4960) == 0 {
            continue;
        }
        textured(fx, sp, 0x14);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        fx.mem.setf(sp + 0x3F0, -100.0);
        fx.mem.setf(sp + 0x3F4, -100.0);
        fx.mem.write_u32(sp + 0x3F8, 0);
        fx.mem.write_u32(sp + 0x3FC, 0x3F80_0000);
        fx.mem.setf(sp + 0x400, 100.0);
        fx.mem.setf(sp + 0x404, 100.0);
        fx.mem.write_u32(sp + 0x408, 0);
        fx.mem.write_u32(sp + 0x40C, 0x3F80_0000);
        fx.vu_apply_to(sp + 0x3F0, sp + 0x3B0, sp + 0x3F0);
        fx.vu_apply_to(sp + 0x400, sp + 0x3B0, sp + 0x400);
        let age = fx.mem.i32(r + 0x4964);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let al = fixu(fx, ee::mul(128.0, ee::sub(1.0, t)), 0x01E0_15A4) as u8;
        rgba(fx, sp + 0x470, WHITE, al);
        copy4(fx, sp + 0x450, r + 0x4970);
        if draw(fx, sp, None) {
            crate::lens::far_z(fx, sp + 0x480, &[sp + 0x490]);
            fx.submit(sp + 0x460, OT, 0);
        }
        textured(fx, sp, 0x10);
        let age = fx.mem.i32(r + 0x4964);
        corners(fx, sp, size(15.0, 0x3F00_0000, 0x3F00_0000, age, 32.0));
        let age = fx.mem.i32(r + 0x4964);
        rgba(fx, sp + 0x470, WHITE, (((0x20 - age) << 7) >> 5) as u8);
        let age = fx.mem.i32(r + 0x4964);
        let f21 = fx.tan_falloff_p(age, 0x20, 1);
        for j in 0..0x10u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x4980 + j));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x4990 + 0x10 * j, f21, r + 0x4970, 0x01E0_1940);
        }
        let age = fx.mem.i32(r + 0x4964) + 1;
        fx.mem.set32(r + 0x4964, age);
        if age >= 0x21 {
            fx.mem.write_u8(r + 0x4960, 0);
        }
    }
}

/// The bursts (D+0x4E20, 64 updates) [F 0x1E019E0..0x1E01F8C].
fn bursts(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    camera(fx, sp);
    for k in 0..4u32 {
        let r = D + 0x860 * k;
        if fx.mem.read_u8(r + 0x4E20) == 0 {
            continue;
        }
        textured(fx, sp, 0x10);
        let age = fx.mem.i32(r + 0x4E24);
        corners(fx, sp, size(16.0, 0x3ECC_CCCD, 0x3F19_999A, age, 64.0));
        let age = fx.mem.i32(r + 0x4E24);
        let c = fx.tcos((age << 14) >> 6);
        let al = fixu(fx, ee::mul(116.0, c), 0x01E0_1B50) as u8;
        rgba(fx, sp + 0x470, WHITE, al);
        let age = fx.mem.i32(r + 0x4E24);
        let f20 = spread(fx, age, 0x40, 0x3DCC_CCCD, 0x3F66_6666);
        for j in 0..0x40u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x4E28 + j));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x4E80 + 0x10 * j, f20, r + 0x4E70, 0x01E0_1CD8);
        }
        textured(fx, sp, 0x18);
        let age = fx.mem.i32(r + 0x4E24);
        corners(fx, sp, size(10.0, 0x3F00_0000, 0x3F00_0000, age, 64.0));
        let age = fx.mem.i32(r + 0x4E24);
        let c = fx.tcos((age << 14) >> 6);
        let al = fixu(fx, ee::mul(128.0, c), 0x01E0_1DE8) as u8;
        rgba(fx, sp + 0x470, WHITE, al);
        uvs(fx, sp, 0, 0, 0x800, 0x800);
        let age = fx.mem.i32(r + 0x4E24);
        let f20 = spread(fx, age, 0x40, 0x3DCC_CCCD, 0x3F66_6666);
        for j in 0..0x40u32 {
            sprite(fx, sp, r + 0x5280 + 0x10 * j, f20, r + 0x4E70, 0x01E0_1F30);
        }
        let age = fx.mem.i32(r + 0x4E24) + 1;
        fx.mem.set32(r + 0x4E24, age);
        if age >= 0x41 {
            fx.mem.write_u8(r + 0x4E20, 0);
        }
    }
}

/// The ribbons (D+0x6FB0, 32 updates) [F 0x1E021A4..0x1E02584].
fn ribbons(fx: &mut Fx, sp: u32) {
    let d = sp + 0x4B8;
    fx.mem.write_u16(d + 2, 0x34);
    fx.mem.write_u16(d, 0xC);
    fx.mem.write_u64(d + 8, 0x4214_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let p = fx.alpha_preset(0);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + 0x1C);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    let (w, h) = (f32::from_bits(0x3FB3_3333), f32::from_bits(0x3F33_3333));
    for (o, x, y) in [(0x3F0u32, ee::neg(w), ee::neg(h)), (0x400, w, ee::neg(h)), (0x410, ee::neg(w), h), (0x420, w, h)] {
        fx.mem.setf(sp + o, x);
        fx.mem.setf(sp + o + 4, y);
        fx.mem.write_u32(sp + o + 8, 0);
        fx.mem.write_u32(sp + o + 0xC, 0x3F80_0000);
    }
    for k in 0..6u32 {
        let r = D + 0x830 * k;
        if fx.mem.read_u8(r + 0x6FB0) == 0 {
            continue;
        }
        let age = ee::from_i32(fx.mem.i32(r + 0x6FB4));
        let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 32.0));
        for g in 0..2u32 {
            let pos = r + 0x10 * g + 0x73C0;
            for j in 0..0x20u32 {
                let age = ee::from_i32(fx.mem.i32(r + 0x6FB4));
                let life = ee::from_i32(fx.mem.i32(r + 0x80 * g + 4 * j + 0x72B8));
                let f21 = ee::sub(1.0, Fx::ratio_below(age, life));
                let age = ee::from_i32(fx.mem.i32(r + 0x6FB4));
                let f0 = ee::mul(128.0, Fx::ratio_below(age, 3.0));
                let al = fixu(fx, ee::mul(f0, f21), 0x01E0_234C) as u8;
                rgba(fx, sp + 0x4C8, [0x80, 0x80, 0x80], al);
                for o in [0x4E0u32, 0x4F8, 0x510] {
                    let (a, b) = (fx.mem.read_u32(sp + 0x4C8), fx.mem.read_u32(sp + 0x4CC));
                    fx.mem.write_u32(sp + o, a);
                    fx.mem.write_u32(sp + o + 4, b);
                }
                fx.vu_scale4(sp + 0x450, r + 0x200 * g + 0x10 * j + 0x73E0, f20);
                fx.vu_add4(sp + 0x450, sp + 0x450, pos);
                fx.camera_vp_to(sp + 0x330);
                fx.identity(sp + 0x370);
                fx.translate_rows(sp + 0x370, sp + 0x370, sp + 0x450);
                let age = fx.mem.i32(r + 0x6FB4);
                let a0 = ((j as i32) >> 1) + 1;
                let a0 = a0.wrapping_mul(age);
                let s6 = (2 * rem2(j as i32, 2) - 1).wrapping_mul(a0);
                let ang = r + 0x180 * g + 0xC * j + 0x6FB8;
                for (axis, m) in [(0, 111), (1, 222), (2, 333)] {
                    let base = fx.mem.i32(ang + 4 * axis as u32);
                    fx.rotate_axis(sp + 0x370, base.wrapping_add(s6.wrapping_mul(m)), axis);
                }
                let m = fx.read_m4(sp + 0x370);
                fx.mul_into(Some(sp + 0x330), &m);
                for (o, v) in [(0x4D0u32, 0u32), (0x4D4, 0), (0x4E8, 0x3F80_0000), (0x4EC, 0), (0x500, 0), (0x504, 0x3F80_0000), (0x518, 0x3F80_0000), (0x51C, 0x3F80_0000)] {
                    fx.mem.write_u32(sp + o, v);
                }
                let m = fx.read_m4(sp + 0x330);
                if fx.project_desc(sp + 0x3F0, PROJ_OUT, Some(&m), d, sp + 0x54C) {
                    let w = fx.mem.f(sp + 0x54C);
                    let key = fixu(fx, w, 0x01E0_24E0) as i32;
                    fx.submit(d, OT, key);
                }
            }
        }
        let age = fx.mem.i32(r + 0x6FB4) + 1;
        fx.mem.set32(r + 0x6FB4, age);
        if age >= 0x21 {
            fx.mem.write_u8(r + 0x6FB0, 0);
        }
    }
}
