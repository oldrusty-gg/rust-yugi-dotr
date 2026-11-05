//! BMODULE b512 (card 481), hand-ported from the record (linked at
//! 0x1E00000, one function 0x1E00080, data area a0 = 0x1ED0000, 0xFE50
//! bytes) [F]: a single unscheduled function in
//! a 0x490 frame on b150's helpers. The init lays out a cone mesh: 32
//! rings (D+0x170, 0xD0) of 13 vertices (radius 6 (32 - i) / 32 at
//! height 20 (i + 1) / 32, each ring started at a random angle). Every
//! update the attacker's unit flag +0x20A = 1 (0x1F69C0), the camera turn
//! with roll (0x2055E0 into sp + 0x320), the attacker's nodes 2..7 (D+0x50
//! ..) and three aim matrices (0x100E40, D+0xB0 / 0xF0 / 0x130) along the
//! node pairs 2 -> 3, 4 -> 5, 6 -> 7. Three cones (flags D+0x1B70 / 0x1B78
//! / 0x1B80, on at updates 0x47 / 0x3C / 0x53, off at 0x75 / 0x6C / 0x7D
//! or after 0x100 updates) are drawn as triangle fans (prim 0x33, 0x100B00)
//! from the second node of their pair along its aim, ST scrolling by age %
//! 6, the apex (64, A4, FF, 64 x ratio_below(age, 16)) and the rim (0C, 0C,
//! FF, 0), at the far Z with key 0 from update 0x5B; each live cone leaves
//! a puff entry (+0x1B90, 0xF00 a group, 24 x 0xA0) of eight random sprites
//! (0.5) at its node every update, drawn 24 updates (half size 6 (0.8 + 0.2
//! age / 24), spread 0.3 + 0.7 tan falloff, alpha 128 cos((age << 14) /
//! 24), a 7-colour cycle, the whole texture, far Z from 0x5B). When a cone
//! goes off its burst record (+0x4890, 0x110: 16 random directions of 4)
//! spawns for 8 updates one group of 16 sprite bursts (+0x4BC0, 0x3B80 a
//! record, 8 x 0x770) at the target along the directions x (0.1 + 0.9 age /
//! 8), each burst six random sprites (0.5) drawn 32 updates (half size 8
//! (0.7 + 0.3 age / 32), spread 0.2 + 0.8 tan falloff, alpha 200
//! cos((age << 14) >> 5), ALPHA preset 0 for the first two of six, a
//! second 7-colour cycle). Returns 1 on update 0x62, 6 on 0x6D..0x84, 2 on
//! 0x85, 8 once the count reaches 0xA6.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
/// 0x204F70's output buffer.
const PROJ_OUT: u32 = 0x002D_3600;
const FRAME: u32 = 0x490;
/// The triangle descriptor (prim 0x33) and the sprite descriptor.
const TRI: u32 = 0x410;
const SPR: u32 = 0x3B8;
const KEEP: u64 = 0xFF00_0000_FFFF_FFFF;
const FAR_Z: u64 = 0x00FF_FFF8_0000_0000;
/// The puffs' colours by (group + 7 + 0x11 entry) % 7 [F jump table
/// 0x1E017A0].
const PUFF_COLOURS: [[u8; 3]; 7] = [[0, 0x20, 0x20], [0x20, 0x20, 0], [0, 0, 0x20], [0, 0x20, 0], [0x20, 0, 0x20], [0x20, 0x80, 0], [0x20, 0, 0]];
/// The bursts' colours by (record + 7 + 3 group + sub + sprite) % 7 [F
/// jump table 0x1E01780].
const BURST_COLOURS: [[u8; 3]; 7] = [[0x80, 0, 0], [0, 0x80, 0x80], [0x80, 0x80, 0], [0x80, 0, 0x80], [0, 0x80, 0], [0, 0x40, 0x80], [0, 0, 0x80]];

/// The entry 0x1E00080 (a0 = D, a1 = param).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(FRAME);
    fx.mem.write_u32(sp + 0xC0, D);
    fx.mem.write_u32(sp + 0xB0, 0);
    let ret = if param >= 0 { init(fx, sp) } else { step(fx, sp, param) };
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

fn copy8(fx: &mut Fx, to: u32, from: u32) {
    let a = fx.mem.read_u32(from);
    let b = fx.mem.read_u32(from + 4);
    fx.mem.write_u32(to, a);
    fx.mem.write_u32(to + 4, b);
}

fn init(fx: &mut Fx, sp: u32) -> i32 {
    fx.bzero(D, 0xFE50);
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
    for node in 2..8 {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, i32::from(node == 2));
        fx.mem.set32(D + 0x30 + 4 * (node - 2) as u32, r);
    }
    for i in 0..0x20i32 {
        let f20 = ee::div(ee::mul(20.0, ee::from_i32(i + 1)), 32.0);
        let f21 = ee::div(ee::mul(6.0, ee::from_i32(0x20 - i)), 32.0);
        let r = fx.rand_from(i as u32, 0x01E0_0260) >> 23;
        let s2 = (rem2(r, 256) << 16) >> 8;
        let ring = D + 0xD0 * i as u32;
        for k in 0..0xDi32 {
            let x = k << 16;
            fx.mult(0x2AAA_AAAB, x);
            let hi = fx.hilo().1;
            let a = s2.wrapping_add((hi >> 1).wrapping_add(((x as u32) >> 31) as i32)) & 0xFFFF;
            let p = ring + 0x170 + 0x10 * k as u32;
            let sn = fx.tsin(a);
            fx.mem.setf(p, ee::mul(f21, sn));
            let cs = fx.tcos(a);
            fx.mem.setf(p + 4, ee::mul(f21, cs));
            fx.mem.setf(p + 8, f20);
            fx.mem.write_u32(p + 0xC, 0x3F80_0000);
        }
    }
    for k in 0..3u32 {
        fx.mem.write_u8(D + 0x1B70 + 8 * k, 0);
    }
    for g in 0..3u32 {
        for e in 0..0x18u32 {
            fx.mem.write_u8(D + 0x1B90 + 0xF00 * g + 0xA0 * e, 0);
        }
    }
    for k in 0..3u32 {
        fx.mem.write_u8(D + 0x4890 + 0x110 * k, 0);
        fx.mem.write_u32(D + 0x4898 + 0x110 * k, 0);
    }
    // The last clear loop steps the stack copy of a0.
    fx.mem.write_u32(sp + 0xC0, D + 0x330);
    0
}

fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

/// The sprite's two UVs (sp + 0x3D0 / 0x3D2 / 0x3E0 / 0x3E2).
fn uvs(fx: &mut Fx, sp: u32, u0: u32, v0: u32, u1: u32, v1: u32) {
    uv(fx, sp + 0x3D0, u0);
    uv(fx, sp + 0x3D2, v0);
    uv(fx, sp + 0x3E0, u1);
    uv(fx, sp + 0x3E2, v1);
}

/// 0x1A2C70: an unsigned integer from the float (with its residue frame).
fn fix(fx: &mut Fx, f: f32, ra: u32) -> u32 {
    fx.fix_residue(f, ra);
    ee::fixunssfsi(f)
}

/// The far Z into the XYZ2 at `first`, then its Z into the others.
fn far_z(fx: &mut Fx, first: u32, others: &[u32]) {
    let v = fx.mem.read_u64(first);
    fx.mem.write_u64(first, (v & KEEP) | FAR_Z);
    let z = ((fx.mem.read_u64(first) << 8) >> 40) << 32;
    for &o in others {
        let w = fx.mem.read_u64(o);
        fx.mem.write_u64(o, (w & KEEP) | z);
    }
}

/// The view-projection (x identity) at sp + 0x2A0.
fn camera(fx: &mut Fx, sp: u32) {
    fx.camera_vp_to(sp + 0x2A0);
    fx.identity(sp + 0x2E0);
    let id = fx.read_m4(sp + 0x2E0);
    fx.mul_into(Some(sp + 0x2A0), &id);
}

/// The corners (-s, -s, 0, 1) at sp + 0x360 and (s, s, 0, 1) at sp +
/// 0x370 turned by the camera (sp + 0x320).
fn corners(fx: &mut Fx, sp: u32, s: f32) {
    let n = ee::neg(s);
    fx.mem.setf(sp + 0x360, n);
    fx.mem.setf(sp + 0x364, n);
    fx.mem.write_u32(sp + 0x368, 0);
    fx.mem.write_u32(sp + 0x36C, 0x3F80_0000);
    fx.mem.setf(sp + 0x370, s);
    fx.mem.setf(sp + 0x374, s);
    fx.mem.write_u32(sp + 0x378, 0);
    fx.mem.write_u32(sp + 0x37C, 0x3F80_0000);
    fx.vu_apply_to(sp + 0x360, sp + 0x320, sp + 0x360);
    fx.vu_apply_to(sp + 0x370, sp + 0x320, sp + 0x370);
}

/// The sprite colour (c, a, Q = 1.0) at sp + 0x3C8.
fn colour(fx: &mut Fx, sp: u32, c: [u8; 3], a: u8) {
    fx.mem.write_u8(sp + 0x3C8, c[0]);
    fx.mem.write_u8(sp + 0x3C9, c[1]);
    fx.mem.write_u8(sp + 0x3CA, c[2]);
    fx.mem.write_u8(sp + 0x3CB, a);
    fx.mem.write_u32(sp + 0x3CC, 0x3F80_0000);
}

/// The sprite at sp + 0x3A0 = dir x f + at; its vertices sp + 0x380 /
/// 0x390.
fn place(fx: &mut Fx, sp: u32, dir: u32, f: f32, at: u32) {
    fx.vu_scale4(sp + 0x3A0, dir, f);
    fx.vu_add4(sp + 0x3A0, sp + 0x3A0, at);
    fx.vu_add4(sp + 0x380, sp + 0x360, sp + 0x3A0);
    fx.vu_add4(sp + 0x390, sp + 0x370, sp + 0x3A0);
}

/// (age << shift) / 24 (the compiled multiply).
fn div24(fx: &mut Fx, x: i32) -> i32 {
    fx.mult(0x2AAA_AAAB, x);
    let hi = fx.hilo().1;
    (hi >> 2).wrapping_add(((x as u32) >> 31) as i32)
}

/// The cones [F 0x1E00594]: 32 rings of 12 triangles from the origin of
/// the cone's matrix, each leaving a puff entry.
fn cones(fx: &mut Fx, sp: u32) {
    let d = sp + TRI;
    fx.mem.write_u16(d + 2, 0x33);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2142_1421);
    fx.mem.write_u32(d + 4, 0);
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x10);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 0);
    let o = sp + 0x3A0;
    fx.mem.write_u32(o, 0);
    fx.mem.write_u32(o + 4, 0);
    fx.mem.write_u32(o + 8, 0);
    fx.mem.write_u32(o + 0xC, 0x3F80_0000);
    fx.mem.write_u32(sp + 0xD0, 1);
    fx.mem.write_u32(sp + 0x260, 0);
    fx.mem.write_u32(sp + 0xE0, D);
    fx.mem.write_u32(sp + 0xF0, D);
    fx.mem.write_u32(sp + 0x100, D);
    let (vp, m) = (sp + 0x2A0, sp + 0x2E0);
    for c in 0..3u32 {
        let fp = D + 8 * c;
        if fx.mem.read_u8(fp + 0x1B70) != 0 {
            fx.camera_vp_to(vp);
            fx.identity(m);
            fx.translate_rows(m, m, D + 0x50 + 0x10 * (2 * c + 1));
            let aim = fx.read_m4(D + 0xB0 + 0x40 * c);
            fx.mul_into(Some(m), &aim);
            let mm = fx.read_m4(m);
            fx.mul_into(Some(vp), &mm);
            let age = ee::from_i32(fx.mem.i32(fp + 0x1B74));
            let f0 = Fx::ratio_below(age, 16.0);
            let a = fix(fx, ee::mul(64.0, f0), 0x01E0_06B0);
            fx.mem.write_u8(sp + 0x420, 0x64);
            fx.mem.write_u8(sp + 0x421, 0xA4);
            fx.mem.write_u8(sp + 0x422, 0xFF);
            fx.mem.write_u8(sp + 0x423, a as u8);
            fx.mem.write_u32(sp + 0x424, 0x3F80_0000);
            fx.mem.write_u8(sp + 0x438, 0x0C);
            fx.mem.write_u8(sp + 0x439, 0x0C);
            fx.mem.write_u8(sp + 0x43A, 0xFF);
            fx.mem.write_u8(sp + 0x43B, 0);
            fx.mem.write_u32(sp + 0x43C, 0x3F80_0000);
            copy8(fx, sp + 0x450, sp + 0x438);
            for s6 in 0..0x20i32 {
                let ring = D + 0xD0 * s6 as u32;
                let age = fx.mem.i32(fp + 0x1B74);
                let (_, q) = fx.div(age, 6);
                let f1 = ee::div(ee::mul(3.0, ee::from_i32(s6)), 32.0);
                let f2 = ee::div(ee::mul(-1.0, ee::from_i32(q)), 6.0);
                let f21 = ee::add(f2, f1);
                let f20 = ee::add(4.0, f21);
                for k in 0..0xCu32 {
                    fx.mem.write_u32(sp + 0x428, 0x3F00_0000);
                    fx.mem.setf(sp + 0x42C, f21);
                    fx.mem.write_u32(sp + 0x440, 0);
                    fx.mem.setf(sp + 0x444, f20);
                    fx.mem.write_u32(sp + 0x458, 0x3F80_0000);
                    fx.mem.setf(sp + 0x45C, f20);
                    let (v1, v2) = (ring + 0x170 + 0x10 * k, ring + 0x170 + 0x10 * (k + 1));
                    let r = fx.project_n(&[o, v1, v2], vp, d, sp + 0x48C, 0x01E0_07C0);
                    if r != 1 {
                        continue;
                    }
                    if fx.mem.i32(D) >= 0x5B {
                        far_z(fx, sp + 0x430, &[sp + 0x448, sp + 0x460]);
                        fx.submit(d, OT, 0);
                    } else {
                        let w = fx.mem.f(sp + 0x48C);
                        let key = fix(fx, w, 0x01E0_088C);
                        fx.submit(d, OT, key as i32);
                    }
                }
            }
            // A puff entry at the cone's node.
            let age = fx.mem.i32(fp + 0x1B74);
            let (_, e) = fx.div(age, 0x18);
            let base = D + 0xF00 * c + 0xA0 * e as u32;
            let p = base + 0x1B90;
            fx.mem.write_u8(p, 1);
            fx.mem.write_u32(p + 4, 0);
            words(fx, p + 0x10, D + 0x60 + 0x20 * c);
            for j in 0..8u32 {
                let r = fx.rand_from(base + 0x10 * j, 0x01E0_0930) >> 23;
                fx.mem.set8(p + 8 + j, rem2(r, 8));
                fx.random_table_vec(p + 0x20 + 0x10 * j, 0.5);
            }
            let age = fx.mem.i32(fp + 0x1B74) + 1;
            fx.mem.set32(fp + 0x1B74, age);
            if age >= 0x101 {
                fx.mem.write_u8(fp + 0x1B70, 0);
            }
        }
        fx.mem.set32(sp + 0xD0, 3 + 2 * c as i32);
        fx.mem.write_u32(sp + 0xE0, D + 0x40 * (c + 1));
        fx.mem.write_u32(sp + 0xF0, D + 0xF00 * (c + 1));
        fx.mem.write_u32(sp + 0x100, D + 0x20 * (c + 1));
        fx.mem.write_u32(sp + 0x260, c + 1);
    }
}

/// The sprite descriptor at sp + 0x3B8 with its pointers at sp + 0x200 /
/// 0x210 / 0x220.
fn sprite_desc(fx: &mut Fx, sp: u32) {
    let d = sp + SPR;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u32(sp + 0x200, d + 2);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x210, d + 8);
    fx.mem.write_u32(sp + 0x220, d + 4);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
}

/// The puffs [F 0x1E009DC]: 3 groups of 24 entries of eight sprites.
fn puffs(fx: &mut Fx, sp: u32) {
    sprite_desc(fx, sp);
    let d = sp + SPR;
    let a = fx.alpha_preset(1);
    fx.desc_alpha(d, a);
    let rh = fx.mem.read_u32(D + 0x18);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x120, D);
    fx.mem.write_u32(sp + 0x250, 0);
    for g in 0..3u32 {
        fx.mem.write_u32(sp + 0x280, 0);
        fx.mem.write_u32(sp + 0x110, g + 7);
        for e in 0..0x18u32 {
            let p = D + 0xF00 * g + 0xA0 * e + 0x1B90;
            if fx.mem.read_u8(p) != 0 {
                let age = fx.mem.i32(p + 4);
                let s = ee::div(ee::mul(f32::from_bits(0x3E4C_CCCD), ee::from_i32(age)), 24.0);
                let s = ee::mul(6.0, ee::add(f32::from_bits(0x3F4C_CCCD), s));
                corners(fx, sp, s);
                let x = fx.mem.i32(p + 4) << 14;
                let ang = div24(fx, x);
                let cs = fx.tcos(ang);
                let a = fix(fx, ee::mul(128.0, cs), 0x01E0_0B88) & 0xFF;
                let idx = fx.mem.i32(sp + 0x110);
                let (_, i) = fx.div(idx, 7);
                let c = PUFF_COLOURS.get(i as usize).copied().unwrap_or([0, 0x20, 0]);
                colour(fx, sp, c, a as u8);
                let age = fx.mem.i32(p + 4);
                let f0 = fx.tan_falloff_p(age, 0x18, 1);
                let f20 = ee::add(f32::from_bits(0x3E99_999A), ee::mul(f32::from_bits(0x3F33_3333), f0));
                for j in 0..8u32 {
                    place(fx, sp, p + 0x20 + 0x10 * j, f20, p + 0x10);
                    uvs(fx, sp, 0, 0, 0x200, 0x200);
                    if crate::fxlib2::project(fx, sp + 0x380, PROJ_OUT, sp + 0x2A0, d, sp + 0x48C) {
                        if fx.mem.i32(D) >= 0x5B {
                            far_z(fx, sp + 0x3D8, &[sp + 0x3E8]);
                            fx.submit(d, OT, 0);
                        } else {
                            let w = fx.mem.f(sp + 0x48C);
                            let key = fix(fx, w, 0x01E0_0E6C);
                            fx.submit(d, OT, key as i32);
                        }
                    }
                }
                let age = fx.mem.i32(p + 4) + 1;
                fx.mem.set32(p + 4, age);
                if age >= 0x19 {
                    fx.mem.write_u8(p, 0);
                }
            }
            let idx = fx.mem.i32(sp + 0x110) + 0x11;
            fx.mem.set32(sp + 0x110, idx);
            fx.mem.write_u32(sp + 0x280, e + 1);
        }
        fx.mem.write_u32(sp + 0x120, D + 0xF00 * (g + 1));
        fx.mem.write_u32(sp + 0x250, g + 1);
    }
}

/// The burst records [F 0x1E00F00]: for 8 updates a group of 16 bursts at
/// the target.
fn burst_spawn(fx: &mut Fx, sp: u32) {
    fx.mem.write_u32(sp + 0x240, 0);
    fx.mem.write_u32(sp + 0x160, D);
    fx.mem.write_u32(sp + 0x170, D);
    for t in 0..3u32 {
        let tr = D + 0x110 * t;
        if fx.mem.read_u8(tr + 0x4890) != 0 {
            let a1 = fx.mem.i32(tr + 0x4898);
            fx.mem.set32(tr + 0x4898, a1 + 1);
            if fx.mem.i32(tr + 0x4898) >= 8 {
                fx.mem.write_u32(tr + 0x4898, 8);
            }
            if a1 < 8 {
                let base = D + 0x3B80 * t + 0x770 * a1 as u32;
                fx.mem.write_u8(base + 0x4BC0, 1);
                fx.mem.write_u32(base + 0x4BC4, 0);
                fx.mem.write_u32(sp + 0x140, base);
                fx.mem.write_u32(sp + 0x150, base);
                let age = ee::from_i32(fx.mem.i32(tr + 0x4894));
                let f1 = ee::div(ee::mul(f32::from_bits(0x3F66_6666), age), 8.0);
                let f20 = ee::add(f32::from_bits(0x3DCC_CCCD), f1);
                fx.mem.write_u32(sp + 0x130, tr);
                for s6 in 0..0x10u32 {
                    fx.vu_scale4(sp + 0x3A0, tr + 0x48A0 + 0x10 * s6, f20);
                    fx.vu_add4(base + 0x4C30 + 0x10 * s6, sp + 0x3A0, D + 0x20);
                    for s2 in 0..6u32 {
                        let cells = base + 6 * s6 + s2;
                        let r = fx.rand_from(cells, 0x01E0_1018) >> 23;
                        fx.mem.set8(cells + 0x4BC8, rem2(r, 4) + 2);
                        fx.random_table_vec(base + 0x60 * s6 + 0x10 * s2 + 0x4D30, 0.5);
                    }
                    fx.mem.write_u32(sp + 0x130, tr + 0x10 * (s6 + 1));
                    fx.mem.write_u32(sp + 0x140, base + 6 * (s6 + 1));
                    fx.mem.write_u32(sp + 0x150, base + 0x60 * (s6 + 1));
                }
            }
            let age = fx.mem.i32(tr + 0x4894) + 1;
            fx.mem.set32(tr + 0x4894, age);
            if age >= 9 {
                fx.mem.write_u8(tr + 0x4890, 0);
            }
        }
        fx.mem.write_u32(sp + 0x160, D + 0x110 * (t + 1));
        fx.mem.write_u32(sp + 0x170, D + 0x3B80 * (t + 1));
        fx.mem.write_u32(sp + 0x240, t + 1);
    }
}

/// The bursts [F 0x1E010F4]: 3 records x 8 groups x 16 bursts of six
/// sprites.
fn bursts(fx: &mut Fx, sp: u32) {
    let d = sp + SPR;
    fx.mem.write_u16(d + 2, 0x26);
    fx.mem.write_u16(d, 5);
    fx.mem.write_u64(d + 8, 0x4_2421);
    fx.mem.write_u32(d + 4, 0);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    let rh = fx.mem.read_u32(D + 0x14);
    fx.desc_texture(d, rh, 1);
    fx.desc_clamp(d, 5);
    camera(fx, sp);
    fx.mem.write_u32(sp + 0x230, 0);
    for t in 0..3u32 {
        let rec = D + 0x3B80 * t;
        fx.mem.write_u32(sp + 0x1F0, rec);
        fx.mem.write_u32(sp + 0x270, 0);
        fx.mem.write_u32(sp + 0x1D0, rec);
        fx.mem.write_u32(sp + 0x1E0, t + 7);
        for b in 0..8u32 {
            let base = rec + 0x770 * b;
            let g = base + 0x4BC0;
            if fx.mem.read_u8(g) != 0 {
                let age = ee::from_i32(fx.mem.i32(g + 4));
                let s = ee::div(ee::mul(f32::from_bits(0x3E99_999A), age), 32.0);
                let s = ee::mul(8.0, ee::add(f32::from_bits(0x3F33_3333), s));
                corners(fx, sp, s);
                let age = fx.mem.i32(g + 4);
                let cs = fx.tcos((age << 14) >> 5);
                let s6 = fix(fx, ee::mul(200.0, cs), 0x01E0_1264) & 0xFF;
                let age = fx.mem.i32(g + 4);
                let f0 = fx.tan_falloff_p(age, 0x20, 1);
                let f20 = ee::add(f32::from_bits(0x3E4C_CCCD), ee::mul(f32::from_bits(0x3F4C_CCCD), f0));
                fx.mem.write_u32(sp + 0x290, 0);
                let first = fx.mem.i32(sp + 0x1E0);
                fx.mem.set32(sp + 0x190, first);
                fx.mem.write_u32(sp + 0x1A0, base);
                fx.mem.write_u32(sp + 0x1B0, base);
                fx.mem.write_u32(sp + 0x1C0, base);
                for k in 0..0x10u32 {
                    let s1 = fx.mem.i32(sp + 0x190);
                    let pos = base + 0x10 * k + 0x4C30;
                    fx.mem.write_u32(sp + 0x180, pos);
                    for s0 in 0..6u32 {
                        let a = fx.alpha_preset(if s0 < 2 { 0 } else { 1 });
                        fx.desc_alpha(d, a);
                        let (_, i) = fx.div(s1 + s0 as i32, 7);
                        let c = BURST_COLOURS.get(i as usize).copied().unwrap_or([0x80, 0, 0x80]);
                        colour(fx, sp, c, s6 as u8);
                        place(fx, sp, base + 0x60 * k + 0x10 * s0 + 0x4D30, f20, pos);
                        let cb = fx.mem.read_u8(base + 6 * k + s0 + 0x4BC8);
                        let (cc, r) = (u32::from(cb & 3), u32::from(cb >> 2));
                        uvs(fx, sp, (cc << 9) & 0x3FFF, (r << 9) & 0x3FFF, ((cc * 32 + 32) << 4) & 0x3FFF, ((r * 32 + 32) << 4) & 0x3FFF);
                        if crate::fxlib2::project(fx, sp + 0x380, PROJ_OUT, sp + 0x2A0, d, sp + 0x48C) {
                            let w = fx.mem.f(sp + 0x48C);
                            let key = fix(fx, w, 0x01E0_1568);
                            fx.submit(d, OT, key as i32);
                        }
                    }
                    fx.mem.set32(sp + 0x190, s1 + 1);
                    fx.mem.write_u32(sp + 0x1A0, base + 0x60 * (k + 1));
                    fx.mem.write_u32(sp + 0x1B0, base + 6 * (k + 1));
                    fx.mem.write_u32(sp + 0x1C0, base + 0x10 * (k + 1));
                    fx.mem.write_u32(sp + 0x290, k + 1);
                }
                let age = fx.mem.i32(g + 4) + 1;
                fx.mem.set32(g + 4, age);
                if age >= 0x21 {
                    fx.mem.write_u8(g, 0);
                }
            }
            fx.mem.write_u32(sp + 0x1D0, base + 0x770);
            let idx = fx.mem.i32(sp + 0x1E0) + 3;
            fx.mem.set32(sp + 0x1E0, idx);
            fx.mem.write_u32(sp + 0x270, b + 1);
        }
        fx.mem.write_u32(sp + 0x1F0, rec + 0x3B80);
        fx.mem.write_u32(sp + 0x230, t + 1);
    }
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    crate::b369::unit_flag(fx, 1, param as i64 as u64, 0x01E0_03A4);
    fx.camera_turn_roll(sp + 0x320);
    for k in 0..6u32 {
        let (h, slot) = (fx.mem.i32(D + 4), fx.mem.i32(D + 0x30 + 4 * k));
        fx.node_position(h, slot, D + 0x50 + 0x10 * k);
    }
    for m in 0..3u32 {
        fx.vu_sub4(sp + 0x3A0, D + 0x50 + 0x10 * (2 * m + 1), D + 0x50 + 0x20 * m);
        fx.point_along(D + 0xB0 + 0x40 * m, sp + 0x3A0);
    }
    for (at, k) in [(0x47, 0u32), (0x3C, 1), (0x53, 2)] {
        if fx.mem.i32(D) == at {
            fx.mem.write_u8(D + 0x1B70 + 8 * k, 1);
            fx.mem.write_u32(D + 0x1B74 + 8 * k, 0);
        }
    }
    for (at, k) in [(0x75, 0u32), (0x6C, 1), (0x7D, 2)] {
        if fx.mem.i32(D) == at {
            fx.mem.write_u8(D + 0x1B70 + 8 * k, 0);
            let tr = D + 0x4890 + 0x110 * k;
            fx.mem.write_u8(tr, 1);
            fx.mem.write_u32(tr + 4, 0);
            for j in 0..0x10u32 {
                fx.random_table_vec(tr + 0x10 + 0x10 * j, 4.0);
            }
        }
    }
    cones(fx, sp);
    puffs(fx, sp);
    burst_spawn(fx, sp);
    bursts(fx, sp);
    let n = fx.mem.i32(D);
    let mut ret = 0;
    if n == 0x62 {
        ret = 1;
    } else if (0x6D..0x85).contains(&n) {
        ret = 6;
    } else if n == 0x85 {
        ret = 2;
    }
    fx.mem.set32(D, n + 1);
    if n + 1 >= 0xA6 {
        ret = 8;
    }
    fx.mem.write_u32(sp + 0xB0, ret as u32);
    ret
}
