//! BMODULE b587 (card 664), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080 in a 0x4D0 frame (s0 = the ordering
//! table), data area a0 = 0x1ED0000, 0x8180 bytes) [F]. b188 (card 240, b188.rs) at 0.857: the same heads / puffs /
//! smokes / rings / bursts (data 0x20 lower, the sprite locals moved, new
//! sizes, colours (C8, FF, 20) and lives), no ribbons and no stage light,
//! plus a spark pool and the outcome tail. Every update the camera turn
//! with roll (sp + 0x3E0) and the attacker's nodes 0x59 / 0x69 (D+0x30 /
//! 0x40). On 0x87 / 0x91 / 0x9B / 0xA5 one of four heads (D+0x70, 0x30, 69
//! updates) leaves node k % 2 towards a random point round the target (x
//! kept) with a ring of 16 (D+0x4940, 0x130); a head (sprite 4, (FF, FF,
//! 20)) leaves every update a puff (D+0x130, 16 a head, 16 updates, 4
//! sprites thrown 0.2) and a smoke (D+0x1930, 32 a head, 32 updates, 4
//! thrown 0.3), and on age 0x44 a burst (D+0x4E00, 0x860, 128 updates: 64 +
//! 64 thrown 2). On 1..0x57 a spark record (D+0x6F80, 32 x 0x90: 4 x 2 end
//! points at the two nodes + a unit vector) drawn from the nodes towards
//! its points by falloff(0x20 - age). Then 0x1F8100 on the sparks' last
//! callee's a0..a3; at 1 from 0x112 0x1F77A0(D+8, 1, (FF, 20, 64,
//! clamp(n - 0x111, 0, 0x40))). Returns 1 on 0xC1, 6 on 0xCC..0x168, 2 on
//! 0x169, 8 once the count reaches 0x16B.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x4D0;
/// The sprite descriptor and its RGBAQ / UV words.
const DESC: u32 = 0x470;
const RGBA: u32 = 0x480;
/// The corners (-s, -s) / (s, s) and those about the point.
const C0: u32 = 0x420;
const C1: u32 = 0x430;
const P0: u32 = 0x440;
const P1: u32 = 0x450;
const POS: u32 = 0x460;
const W: u32 = 0x4C8;
const VP: u32 = 0x360;
const ID: u32 = 0x3A0;
const TURN: u32 = 0x3E0;
const COLOUR: [u8; 3] = [0xC8, 0xFF, 0x20];

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xBC, D);
    fx.mem.write_u32(sp + 0xC0, 0);
    let ret = if param >= 0 { init(fx, param) } else { step(fx, sp) };
    fx.leave(FRAME);
    ret
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x8180);
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
    for (at, node, clear) in [(0x1Cu32, 0x59, 1), (0x20, 0x69, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_position(t, D + 0x50);
    fx.mem.write_u32(D + 0x60, 0);
    fx.mem.write_u32(D + 0x64, 0);
    for k in 0..4u32 {
        let r = D + 0x30 * k;
        fx.mem.write_u8(r + 0x70, 0);
        fx.mem.write_u32(r + 0x78, 0);
        fx.mem.write_u32(r + 0x7C, 0);
    }
    for h in 0..4u32 {
        for j in 0..0x10u32 {
            fx.mem.write_u8(D + 0x600 * h + 0x60 * j + 0x130, 0);
        }
    }
    for h in 0..4u32 {
        for j in 0..0x20u32 {
            fx.mem.write_u8(D + 0xC00 * h + 0x60 * j + 0x1930, 0);
        }
    }
    fx.mem.write_u32(D + 0x4930, 0);
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x130 * k + 0x4940, 0);
    }
    for k in 0..4u32 {
        fx.mem.write_u8(D + 0x860 * k + 0x4E00, 0);
    }
    for k in 0..0x20u32 {
        fx.mem.write_u8(D + 0x90 * k + 0x6F80, 0);
    }
    let t = fx.mem.i32(D + 8);
    fx.unit_add_mode(t, 2, 0, [u64::from(OT), param as u32 as u64], 0x01E0_02E8);
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

fn fixu(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + DESC + 0x18, u0);
    uv(fx, sp + DESC + 0x1A, v0);
    uv(fx, sp + DESC + 0x28, u1);
    uv(fx, sp + DESC + 0x2A, v1);
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

fn copy4(fx: &mut Fx, to: u32, from: u32) {
    for c in 0..4u32 {
        let w = fx.mem.read_u32(from + 4 * c);
        fx.mem.write_u32(to + 4 * c, w);
    }
}

/// The sprite descriptor (5 registers 0x42421, prim 0x26 | 0x800).
fn head(fx: &mut Fx, sp: u32) {
    let d = sp + DESC;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// Alpha preset `preset`, the texture at D + `tex`, CLAMP 5.
fn textured(fx: &mut Fx, sp: u32, preset: i32, tex: u32) {
    let d = sp + DESC;
    let p = fx.alpha_preset(preset);
    fx.desc_alpha(d, p);
    let rh = fx.mem.read_u32(D + tex);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
}

/// The view-projection (x identity) at sp + 0x360.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + VP);
    fx.identity(sp + ID);
    let id = fx.read_m4(sp + ID);
    fx.mul_into(Some(sp + VP), &id);
}

/// The corners (-s, -s, 0, 1) / (s, s, 0, 1) turned by the camera.
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + C0, n);
    fx.mem.setf(sp + C0 + 4, n);
    fx.mem.write_u32(sp + C0 + 8, 0);
    fx.mem.write_u32(sp + C0 + 0xC, 0x3F80_0000);
    fx.mem.setf(sp + C1, s);
    fx.mem.setf(sp + C1 + 4, s);
    fx.mem.write_u32(sp + C1 + 8, 0);
    fx.mem.write_u32(sp + C1 + 0xC, 0x3F80_0000);
    fx.vu_apply_to(sp + C0, sp + TURN, sp + C0);
    fx.vu_apply_to(sp + C1, sp + TURN, sp + C1);
}

/// k (base + slope x age / d).
fn size(k: f32, base: u32, slope: u32, age: i32, d: f32) -> f32 {
    let t = ee::div(ee::mul(f32::from_bits(slope), ee::from_i32(age)), d);
    ee::mul(k, ee::add(f32::from_bits(base), t))
}

/// The corners about the point projected (0x204F70) and, with `ra`,
/// submitted at w; true when drawn.
fn draw(fx: &mut Fx, sp: u32, ra: Option<u32>) -> bool {
    fx.vu_add4(sp + P0, sp + C0, sp + POS);
    fx.vu_add4(sp + P1, sp + C1, sp + POS);
    let d = sp + DESC;
    if !crate::fxlib2::project(fx, sp + P0, PROJ_OUT, sp + VP, d, sp + W) {
        return false;
    }
    if let Some(ra) = ra {
        let w = fx.mem.f(sp + W);
        let key = fixu(fx, w, ra) as i32;
        fx.submit(d, OT, key);
    }
    true
}

/// A sprite of the direction `dir` x `f` about `pos`, drawn.
fn sprite(fx: &mut Fx, sp: u32, dir: u32, f: f32, pos: u32, ra: u32) {
    fx.vu_scale4(sp + POS, dir, f);
    fx.vu_add4(sp + POS, sp + POS, pos);
    draw(fx, sp, Some(ra));
}

/// base + slope x the tan falloff.
fn spread(fx: &mut Fx, age: i32, n: i32, base: u32, slope: u32) -> f32 {
    let t = fx.tan_falloff_p(age, n, 1);
    ee::add(f32::from_bits(base), ee::mul(f32::from_bits(slope), t))
}

fn step(fx: &mut Fx, sp: u32) -> i32 {
    fx.camera_turn_roll(sp + TURN);
    for k in 0..2u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x1C + 4 * k));
        fx.node_position(h, slot, D + 0x30 + 0x10 * k);
    }
    let c = fx.mem.i32(D + 0x60);
    if c < 4 && fx.mem.i32(D) == 10 * c + 0x87 {
        launch(fx, sp);
    }
    heads(fx, sp);
    puffs(fx, sp);
    smokes(fx, sp);
    rings(fx, sp);
    bursts(fx, sp);
    spark_spawn(fx);
    let (args, s1) = sparks(fx, sp);
    // The fade of a destroyed target from 0x112 [F 0x1E02200..0x1E0226C].
    if fx.outcome(args) == 1 && fx.mem.i32(D) >= 0x112 {
        let n = fx.mem.i32(D);
        let c = Fx::clamp_i(n - 0x111, 0, 0x40);
        fx.mem.write_u8(sp + 0x4CE, 0x64);
        fx.mem.write_u8(sp + 0x4CD, 0x20);
        fx.mem.write_u8(sp + 0x4CC, 0xFF);
        fx.mem.write_u8(sp + 0x4CF, c as u8);
        let t = fx.mem.i32(D + 8);
        fx.unit_add_mode(t, 1, sp + 0x4CC, [u64::from(OT), u64::from(s1)], 0x01E0_2270);
    }
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0xC1 {
        ret = 1;
    } else {
        if (0xCC..0x169).contains(&n) {
            ret = 6;
        }
        if n == 0x169 {
            ret = 2;
        }
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0x16B {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xC0, ret as u32);
    ret
}

/// A head and its ring from node k % 2 [F 0x1E0032C..0x1E00524].
fn launch(fx: &mut Fx, sp: u32) {
    let i = fx.mem.i32(D + 0x64);
    let hd = D + 0x30 * i as u32;
    fx.mem.write_u8(hd + 0x70, 1);
    fx.mem.write_u32(hd + 0x74, 0);
    let rg = D + 0x130 * fx.mem.read_u32(D + 0x4930);
    fx.mem.write_u8(rg + 0x4940, 1);
    fx.mem.write_u32(rg + 0x4944, 0);
    let node = D.wrapping_add((0x10 * rem2(i, 2)) as u32) + 0x30;
    copy4(fx, hd + 0x80, node);
    copy4(fx, rg + 0x4950, node);
    fx.random_table_vec(sp + POS, 1.0);
    fx.vu_add4(sp + POS, sp + POS, D + 0x50);
    let x = fx.mem.f(hd + 0x80);
    fx.mem.setf(sp + POS, x);
    fx.vu_sub4(hd + 0x90, sp + POS, hd + 0x80);
    for j in 0..0x10u32 {
        let b = fx.rand_from(OT, 0x01E0_0468) >> 23;
        fx.mem.set8(rg + 0x4960 + j, rem2(b, 8));
        fx.random_table_vec(rg + 0x4970 + 0x10 * j, f32::from_bits(0x3E99_999A));
    }
    let c = fx.mem.i32(D + 0x60) + 1;
    fx.mem.set32(D + 0x60, c);
    let i = fx.mem.i32(D + 0x64) + 1;
    fx.mem.set32(D + 0x64, if i >= 4 { 0 } else { i });
    let i = fx.mem.i32(D + 0x4930) + 1;
    fx.mem.set32(D + 0x4930, if i >= 4 { 0 } else { i });
}

/// The heads (sprite 4, 69 updates, along their direction by age / 68)
/// and what they leave [F 0x1E00528..0x1E00AD4].
fn heads(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x14);
    camera(fx, sp);
    uvs(fx, sp, 0x400, 0, 0x600, 0x200);
    corners_raw(fx, sp, 4.0);
    for k in 0..4u32 {
        let hd = D + 0x30 * k;
        if fx.mem.read_u8(hd + 0x70) == 0 {
            continue;
        }
        rgba(fx, sp + RGBA, [0xFF, 0xFF, 0x20], 0x80);
        let f20 = ee::div(ee::from_i32(fx.mem.i32(hd + 0x74)), 68.0);
        let a = fx.mem.i32(hd + 0x78);
        fx.mem.set32(hd + 0x78, if a + 1 >= 0x10 { 0 } else { a + 1 });
        let b = fx.mem.i32(hd + 0x7C);
        fx.mem.set32(hd + 0x7C, if b + 1 >= 0x20 { 0 } else { b + 1 });
        let puff = D + 0x600 * k + 0x60 * a as u32;
        let smoke = D + 0xC00 * k + 0x60 * b as u32;
        fx.vu_scale4(sp + POS, hd + 0x90, f20);
        fx.vu_add4(sp + POS, sp + POS, hd + 0x80);
        draw(fx, sp, Some(0x01E0_0838));
        fx.mem.write_u8(puff + 0x130, 1);
        fx.mem.write_u32(puff + 0x134, 0);
        copy4(fx, puff + 0x140, sp + POS);
        for j in 0..4u32 {
            let b = fx.rand_from(OT, 0x01E0_08A0) >> 23;
            fx.mem.set8(puff + 0x138 + j, rem2(b, 8));
            fx.random_table_vec(puff + 0x150 + 0x10 * j, f32::from_bits(0x3E4C_CCCD));
        }
        fx.mem.write_u8(smoke + 0x1930, 1);
        fx.mem.write_u32(smoke + 0x1934, 0);
        copy4(fx, smoke + 0x1940, sp + POS);
        for j in 0..4u32 {
            fx.random_table_vec(smoke + 0x1950 + 0x10 * j, f32::from_bits(0x3E99_999A));
        }
        if fx.mem.i32(hd + 0x74) == 0x44 {
            let e = D + 0x860 * k;
            fx.mem.write_u8(e + 0x4E00, 1);
            fx.mem.write_u32(e + 0x4E04, 0);
            copy4(fx, e + 0x4E50, sp + POS);
            for j in 0..0x40u32 {
                let b = fx.rand_from(OT, 0x01E0_09B0) >> 23;
                fx.mem.set8(e + 0x4E08 + j, rem2(b, 8));
                fx.random_table_vec(e + 0x4E60 + 0x10 * j, 2.0);
            }
            for j in 0..0x40u32 {
                fx.random_table_vec(e + 0x5260 + 0x10 * j, 2.0);
            }
        }
        let age = fx.mem.i32(hd + 0x74) + 1;
        fx.mem.set32(hd + 0x74, age);
        if age >= 0x45 {
            fx.mem.write_u8(hd + 0x70, 0);
        }
    }
}

/// Fixed corners +-s turned by the camera.
fn corners_raw(fx: &mut Fx, sp: u32, s: f32) {
    corners(fx, sp, s);
}

/// The puffs (D+0x130, 16 updates) [F 0x1E00AD8..0x1E00E74].
fn puffs(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 0, 0x10);
    camera(fx, sp);
    for h in 0..4u32 {
        for j in 0..0x10u32 {
            let r = D + 0x600 * h + 0x60 * j;
            if fx.mem.read_u8(r + 0x130) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 0x134);
            corners(fx, sp, size(4.5, 0x3F33_3333, 0x3E99_999A, age, 16.0));
            let age = fx.mem.i32(r + 0x134);
            rgba(fx, sp + RGBA, COLOUR, (((0x10 - age) << 7) >> 4) as u8);
            let age = fx.mem.i32(r + 0x134);
            let f20 = spread(fx, age, 0x10, 0x3ECC_CCCD, 0x3F19_999A);
            for i in 0..4u32 {
                let b = i32::from(fx.mem.read_u8(r + 0x138 + i));
                cell_uvs(fx, sp, b);
                sprite(fx, sp, r + 0x150 + 0x10 * i, f20, r + 0x140, 0x01E0_0DC8);
            }
            let age = fx.mem.i32(r + 0x134) + 1;
            fx.mem.set32(r + 0x134, age);
            if age >= 0x11 {
                fx.mem.write_u8(r + 0x130, 0);
            }
        }
    }
}

/// The smokes (D+0x1930, 32 updates) [F 0x1E00E78..0x1E011B4].
fn smokes(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    textured(fx, sp, 1, 0x18);
    camera(fx, sp);
    uvs(fx, sp, 0, 0, 0x800, 0x800);
    for h in 0..4u32 {
        for j in 0..0x20u32 {
            let r = D + 0xC00 * h + 0x60 * j;
            if fx.mem.read_u8(r + 0x1930) == 0 {
                continue;
            }
            let age = fx.mem.i32(r + 0x1934);
            corners(fx, sp, size(8.5, 0x3F00_0000, 0x3F00_0000, age, 32.0));
            let age = fx.mem.i32(r + 0x1934);
            let a = (i64::from((0x20 - age).wrapping_mul(3)) << 37) >> 37;
            rgba(fx, sp + RGBA, COLOUR, a as u8);
            let age = fx.mem.i32(r + 0x1934);
            let f20 = spread(fx, age, 0x20, 0x3E4C_CCCD, 0x3F4C_CCCD);
            for i in 0..4u32 {
                sprite(fx, sp, r + 0x1950 + 0x10 * i, f20, r + 0x1940, 0x01E0_1110);
            }
            let age = fx.mem.i32(r + 0x1934) + 1;
            fx.mem.set32(r + 0x1934, age);
            if age >= 0x21 {
                fx.mem.write_u8(r + 0x1930, 0);
            }
        }
    }
}

/// The rings (D+0x4940, 32 updates): a far glow and 16 sprites [F
/// 0x1E011B8..0x1E017A4].
fn rings(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    camera(fx, sp);
    for k in 0..4u32 {
        let r = D + 0x130 * k;
        if fx.mem.read_u8(r + 0x4940) == 0 {
            continue;
        }
        textured(fx, sp, 1, 0x14);
        uvs(fx, sp, 0x600, 0, 0x800, 0x200);
        corners(fx, sp, 5.0);
        let age = fx.mem.i32(r + 0x4944);
        let t = fx.tan_falloff_p(age, 0x20, 1);
        let al = fixu(fx, ee::mul(128.0, ee::sub(1.0, t)), 0x01E0_1368) as u8;
        rgba(fx, sp + RGBA, COLOUR, al);
        copy4(fx, sp + POS, r + 0x4950);
        if draw(fx, sp, None) {
            crate::lens::far_z(fx, sp + DESC + 0x20, &[sp + DESC + 0x30]);
            fx.submit(sp + DESC, OT, 0);
        }
        textured(fx, sp, 0, 0x10);
        let age = fx.mem.i32(r + 0x4944);
        corners(fx, sp, size(5.0, 0x3F00_0000, 0x3F00_0000, age, 32.0));
        let age = fx.mem.i32(r + 0x4944);
        rgba(fx, sp + RGBA, COLOUR, (((0x20 - age) << 7) >> 5) as u8);
        let age = fx.mem.i32(r + 0x4944);
        let f21 = fx.tan_falloff_p(age, 0x20, 1);
        for j in 0..0x10u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x4960 + j));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x4970 + 0x10 * j, f21, r + 0x4950, 0x01E0_1700);
        }
        let age = fx.mem.i32(r + 0x4944) + 1;
        fx.mem.set32(r + 0x4944, age);
        if age >= 0x21 {
            fx.mem.write_u8(r + 0x4940, 0);
        }
    }
}

/// The bursts (D+0x4E00, 128 updates) [F 0x1E017A8..0x1E01D48].
fn bursts(fx: &mut Fx, sp: u32) {
    head(fx, sp);
    camera(fx, sp);
    for k in 0..4u32 {
        let r = D + 0x860 * k;
        if fx.mem.read_u8(r + 0x4E00) == 0 {
            continue;
        }
        textured(fx, sp, 0, 0x10);
        let age = fx.mem.i32(r + 0x4E04);
        corners(fx, sp, size(14.0, 0x3ECC_CCCD, 0x3F19_999A, age, 128.0));
        let age = fx.mem.i32(r + 0x4E04);
        let c = fx.tcos((age << 14) >> 7);
        let al = fixu(fx, ee::mul(116.0, c), 0x01E0_1918) as u8;
        rgba(fx, sp + RGBA, COLOUR, al);
        let age = fx.mem.i32(r + 0x4E04);
        let f20 = spread(fx, age, 0x80, 0x3DCC_CCCD, 0x3F66_6666);
        for j in 0..0x40u32 {
            let b = i32::from(fx.mem.read_u8(r + 0x4E08 + j));
            cell_uvs(fx, sp, b);
            sprite(fx, sp, r + 0x4E60 + 0x10 * j, f20, r + 0x4E50, 0x01E0_1AA0);
        }
        textured(fx, sp, 1, 0x18);
        let age = fx.mem.i32(r + 0x4E04);
        corners(fx, sp, size(8.0, 0x3F00_0000, 0x3F00_0000, age, 128.0));
        let age = fx.mem.i32(r + 0x4E04);
        let c = fx.tcos((age << 14) >> 7);
        let al = fixu(fx, ee::mul(128.0, c), 0x01E0_1BB0) as u8;
        rgba(fx, sp + RGBA, COLOUR, al);
        uvs(fx, sp, 0, 0, 0x800, 0x800);
        let age = fx.mem.i32(r + 0x4E04);
        let f20 = spread(fx, age, 0x80, 0x3DCC_CCCD, 0x3F66_6666);
        for j in 0..0x40u32 {
            sprite(fx, sp, r + 0x5260 + 0x10 * j, f20, r + 0x4E50, 0x01E0_1CF0);
        }
        let age = fx.mem.i32(r + 0x4E04) + 1;
        fx.mem.set32(r + 0x4E04, age);
        if age >= 0x81 {
            fx.mem.write_u8(r + 0x4E00, 0);
        }
    }
}

/// A spark record on 1..0x57 (D+0x6F80 + 0x90 (n % 32)): four pairs of
/// end points, node i + a unit table vector, cells rand % 8 [F
/// 0x1E01D4C..0x1E01E4C].
fn spark_spawn(fx: &mut Fx) {
    let n = fx.mem.i32(D);
    if n <= 0 || n >= 0x58 {
        return;
    }
    let r = D + 0x90 * (n & 0x1F) as u32;
    fx.mem.write_u8(r + 0x6F80, 1);
    fx.mem.write_u32(r + 0x6F84, 0);
    for g in 0..4u32 {
        for i in 0..2u32 {
            let b = fx.rand_from(OT, 0x01E0_1DC8) >> 23;
            fx.mem.set8(r + 2 * g + i + 0x6F88, rem2(b, 8));
            let p = r + 0x20 * g + 0x10 * i + 0x6F90;
            fx.random_table_vec(p, 1.0);
            fx.vu_add4(p, p, D + 0x30 + 0x10 * i);
        }
    }
}

/// The sparks (32 updates): eight sprites (FF, FF, FF, 64 sin(age 512)) of
/// half size 8 (0.2 + 0.8 (0x20 - age) / 32) from node i towards each end
/// point by falloff(0x20 - age) [F 0x1E01E50..0x1E021FC]. Returns a0..a3
/// as the loop's last callee leaves them (the outcome call's arguments)
/// and s1 (the fade's saved s1).
fn sparks(fx: &mut Fx, sp: u32) -> ([u32; 4], u32) {
    head(fx, sp);
    textured(fx, sp, 0, 0x10);
    camera(fx, sp);
    let idx = fx.mem.read_u32(crate::world::CAMERA_STACK_INDEX);
    let cam = crate::world::CAMERA_RECORD.wrapping_add(idx.wrapping_mul(0x1A0)) + 0x40;
    let d = sp + DESC;
    let mut last = [sp + VP, sp + ID, 0, cam];
    let mut s1 = 0;
    for k in 0..0x20u32 {
        let r = D + 0x90 * k;
        if fx.mem.read_u8(r + 0x6F80) == 0 {
            continue;
        }
        let age = fx.mem.i32(r + 0x6F84);
        corners(fx, sp, size(8.0, 0x3E4C_CCCD, 0x3F4C_CCCD, 0x20 - age, 32.0));
        let age = fx.mem.i32(r + 0x6F84);
        let sn = fx.tsin((age << 14) >> 5);
        let al = fixu(fx, ee::mul(64.0, sn), 0x01E0_1FDC) as u8;
        rgba(fx, sp + RGBA, [0xFF; 3], al);
        let age = fx.mem.i32(r + 0x6F84);
        let f20 = fx.tan_falloff_p(0x20 - age, 0x20, 1);
        for g in 0..4u32 {
            for i in 0..2u32 {
                let node = D + 0x30 + 0x10 * i;
                fx.vu_sub4(sp + POS, r + 0x20 * g + 0x10 * i + 0x6F90, node);
                fx.vu_scale4(sp + POS, sp + POS, f20);
                fx.vu_add4(sp + POS, sp + POS, node);
                fx.vu_add4(sp + P0, sp + C0, sp + POS);
                fx.vu_add4(sp + P1, sp + C1, sp + POS);
                let b = i32::from(fx.mem.read_u8(r + 2 * g + i + 0x6F88));
                cell_uvs(fx, sp, b);
                if crate::fxlib2::project(fx, sp + P0, PROJ_OUT, sp + VP, d, sp + W) {
                    let w = fx.mem.f(sp + W);
                    let key = fixu(fx, w, 0x01E0_2150);
                    fx.submit(d, OT, key as i32);
                    last = [d, OT, key, 0];
                } else {
                    last = [sp + P0, PROJ_OUT, sp + VP, d];
                }
            }
            s1 = D + 0x20;
        }
        let age = fx.mem.i32(r + 0x6F84) + 1;
        fx.mem.set32(r + 0x6F84, age);
        if age >= 0x21 {
            fx.mem.write_u8(r + 0x6F80, 0);
        }
    }
    (last, s1)
}
