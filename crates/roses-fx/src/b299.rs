//! BMODULE b299 (card 278), hand-ported from the record (linked at
//! 0x1E00000, entry 0x1E058B0, data area a0 = 0x1ED0000) [F] (helper B7, 31
//! Oct 2026): b649's program shape (s0 = D, s1 the parameter then loop
//! offsets, s2 = the OT). On update 2 six arcs (+0x250, `arc::arcs`) along
//! the attacker's node chains 0x46..0x43 / 0x59..0x56; on 40 the screen
//! beams (+0xA0, the record's 0x1E03B80 `beams`, blinking) from node 0x56;
//! on 55 at the target three ray bursts (+0x4F0, `ray::rays`) aimed from
//! node 0x56 and fanned, a mote cloud (+0xBCC, `mote::motes`), a flash
//! (+0xC4), four bars (+0x110, `cross::bars`), eight bead beams (+0x64C)
//! and a camera shake; on 39 the nodes are requested again (the last as
//! node 1). Returns 1 on update 47, 2 on 54, 8 on 105 (and at once for -2
//! / -3).
//!
//! Library (record addresses): `camera_shake` 0x1E00080, `beads`
//! 0x1E00410, `flash` 0x1E011E0, `rays` 0x1E01D90, `arcs` 0x1E02900,
//! `motes` 0x1E044A0, `bars` 0x1E04EA0 [`crate::blib`].
use crate::b076::{copy_v4, node_position_saving, rand_bytes, shake_saving, unit_position_saving};
use crate::blib::{self, exe::{aim_angles, camera_vp_into, countdown, first_free, node_source, set_counter, set_counter4, tex_rect_strip}};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx, OT};

const D: u32 = crate::bmodule::DATA_AREA;
const BYTES: u32 = D + 0x1384;

/// The entry 0x1E058B0 (a0 = D, a1 = param; s0 = D, s1 = param, s2 = the
/// OT).
pub fn program(fx: &mut Fx, param: i32) -> i32 {
    let sp = fx.enter(0x90);
    let ret = if (param.wrapping_add(3) as u32) < 2 {
        8
    } else if param >= 0 {
        init(fx, param)
    } else {
        step(fx, sp, param)
    };
    fx.leave(0x90);
    ret
}

fn nodes(fx: &mut Fx, last: i32) {
    for (at, node, clear) in [(0xCu32, 0x46, 1), (0x10, 0x45, 0), (0x14, 0x44, 0), (0x18, 0x43, 0), (0x1C, 0x59, 0), (0x20, 0x58, 0), (0x24, 0x57, 0), (0x28, last, 0)] {
        let h = fx.mem.i32(D + 4);
        let r = fx.request_node(h, node, clear);
        fx.mem.set32(D + at, r);
    }
}

fn init(fx: &mut Fx, param: i32) -> i32 {
    fx.bzero(D, 0x1490);
    let h = fx.self_handle();
    fx.mem.set32(D + 4, h);
    let t = fx.target_handle();
    fx.mem.set32(D + 8, t);
    let h = fx.mem.i32(D + 4);
    fx.unit_rotation(h, D + 0x2C);
    fx.texture(4, 0xF);
    fx.texture(5, 0);
    nodes(fx, 0x56);
    // The entry's s0 = D, s1 = the parameter, s2 = the OT.
    rand_bytes(fx, BYTES, 0x100, 0x01E0_5A50, [Some(D), Some(param as u32), Some(OT)]);
    0
}

/// 0x101770(xyz) [F]: both 12.4 window coordinates (u16 at +0, +2) within
/// [28672, 36864] (the 512 x 512 window about the centre).
pub(crate) fn in_window(fx: &mut Fx, a: u32) -> bool {
    let inside = |v: i32| {
        let f = ee::from_i32(v);
        !ee_lt(f, 28672.0) && ee_le(f, 36864.0)
    };
    let x = fx.mem.u16(a);
    if !inside(x) {
        return false;
    }
    let y = fx.mem.u16(a + 2);
    inside(y)
}

/// 0x1A2C70 of `v` from the `jal` at `jal` (its frame left), as u16.
fn fix16(fx: &mut Fx, jal: u32, v: f32) -> i32 {
    fx.fix_residue(v, jal + 8);
    ee::fixunssfsi(v) as u16 as i32
}

/// `from` + `t` x (`to` - `from`) / the record's +2 (`div`, its LO).
fn toward(fx: &mut Fx, g: u32, t: f32, from: i32, to: i32, jal: u32) -> i32 {
    let m = fx.mem.u8(g + 2);
    let q = fx.div(to - from, m).0;
    fix16(fx, jal, ee::add(ee::from_i32(from), ee::mul(t, ee::from_i32(q))))
}

/// The record's 0x1E03B80 [F] (c00e4a33a81f, b299 only): `beams(recs, ot,
/// n) -> 1` draws `n` records (0x24 bytes; a 0x1D0 frame, the descriptor
/// at sp + 0x130: RGBAQ / ST / XYZ2 x 4 at the far Z, the sprite spec
/// 0x1E067C0 and its texture (0x202530), UVs by 0x101EE0). Per record:
/// countdown 0x101AF0, skipped on odd counts with flag 4; the centre from
/// a node with +5 (0x101BA0) projected (or the window centre with flag 1;
/// skipped outside the window, 0x101770); colour +0x20.. at the far edge,
/// +0x1C.. at the centre. Screen quads of width +8 / 2 from the window's
/// top and bottom edges (every +8 of 640) and its left and right edges
/// (every +8 of 448), reaching t = +2 (- count with flag 2) of the +2
/// steps towards the centre; key 0.
fn beams(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.set32(sp + 0xCC, n);
    for k in 0..6u32 {
        let w = fx.mem.read_u32(0x01E0_67C0 + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, w);
    }
    let (kind, id) = (fx.mem.i32(sp + 0x110), fx.mem.u8(sp + 0x114));
    let t = fx.texture(kind, id);
    fx.mem.write_u32(sp + 0xC0, t);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x24;
        }
        fx.mem.set32(sp + 0xB0, i);
        beam(fx, sp, g, ot);
        i += 1;
    }
    fx.mem.set32(sp + 0xB0, i);
    fx.leave(0x1D0);
    1
}

fn beam(fx: &mut Fx, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g + 4) & 4 != 0 && fx.mem.u8(g) & 1 != 0 {
        return;
    }
    node_source(fx, g, g + 0xC);
    camera_vp_into(fx, sp + 0xD0);
    if fx.mem.u8(g + 4) & 1 != 0 {
        fx.mem.write_u16(sp + 0x1C2, 0x8000);
        fx.mem.write_u16(sp + 0x1C0, 0x8000);
    } else {
        let m = fx.read_m4(sp + 0xD0);
        if !fx.project_at(g + 0xC, &m, sp + 0x1C0, sp + 0x1CC) || !in_window(fx, sp + 0x1C0) {
            return;
        }
    }
    let d = sp + 0x130;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for k in 0..4u32 {
        let src = if k < 3 { 2 - k } else { 3 };
        let v = fx.mem.read_u8(g + 0x20 + src);
        for o in [0x170u32, 0x158, 0x140] {
            fx.mem.write_u8(sp + o + k, v);
        }
        let v = fx.mem.read_u8(g + 0x1C + src);
        fx.mem.write_u8(sp + 0x188 + k, v);
    }
    for o in [0x18Cu32, 0x174, 0x15C, 0x144] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    fx.set_xyz_z(sp + 0x198, 0xFF_FFFF);
    for (dst, src) in [(0x180u32, 0x198u32), (0x168, 0x180), (0x150, 0x168)] {
        let z = fx.xyz_z(sp + src);
        fx.set_xyz_z(sp + dst, z);
    }
    for o in [0x19Fu32, 0x187, 0x16F, 0x157] {
        fx.mem.write_u8(sp + o, 0);
    }
    let t = fx.mem.read_u32(sp + 0xC0);
    tex_rect_strip(fx, d, sp + 0x110, t);
    let mut t = ee::from_i32(fx.mem.u8(g + 2));
    if fx.mem.u8(g + 4) & 2 != 0 {
        t = ee::sub(t, ee::from_i32(fx.mem.u8(g)));
    }
    let step = fx.mem.i32(g + 8);
    let edge = |s: i32| ee::add(28672.0, ee::from_i32(s << 4));
    let mut s = 0;
    while s < 0x280 {
        let v = fix16(fx, 0x01E0_3EF0, edge(s));
        fx.mem.set16(sp + 0x180, v);
        let cx = fx.mem.u16(sp + 0x1C0);
        let x = fx.mem.u16(sp + 0x180);
        let v = toward(fx, g, t, x, cx, 0x01E0_3F68);
        fx.mem.set16(sp + 0x150, v);
        let v = fix16(fx, 0x01E0_3F9C, edge(s + (step >> 1)));
        fx.mem.set16(sp + 0x198, v);
        let x = fx.mem.u16(sp + 0x198);
        let v = toward(fx, g, t, x, cx, 0x01E0_4010);
        fx.mem.set16(sp + 0x168, v);
        for (y, jal) in [(0x7000, 0x01E0_40A0u32), (0x9000, 0x01E0_414C)] {
            fx.mem.set16(sp + 0x19A, y);
            fx.mem.set16(sp + 0x182, y);
            let cy = fx.mem.u16(sp + 0x1C2);
            let y = fx.mem.u16(sp + 0x182);
            let v = toward(fx, g, t, y, cy, jal);
            fx.mem.set16(sp + 0x16A, v);
            fx.mem.set16(sp + 0x152, v);
            fx.submit(d, ot, 0);
        }
        s += step;
    }
    let mut s = 0;
    while s < 0x1C0 {
        let v = fix16(fx, 0x01E0_41B8, edge(s));
        fx.mem.set16(sp + 0x182, v);
        let cy = fx.mem.u16(sp + 0x1C2);
        let y = fx.mem.u16(sp + 0x182);
        let v = toward(fx, g, t, y, cy, 0x01E0_4234);
        fx.mem.set16(sp + 0x152, v);
        let v = fix16(fx, 0x01E0_4268, edge(s + (step >> 1)));
        fx.mem.set16(sp + 0x19A, v);
        let y = fx.mem.u16(sp + 0x19A);
        let v = toward(fx, g, t, y, cy, 0x01E0_42E0);
        fx.mem.set16(sp + 0x16A, v);
        for (x, jal) in [(0x7000, 0x01E0_4364u32), (0x9000, 0x01E0_4400)] {
            fx.mem.set16(sp + 0x198, x);
            fx.mem.set16(sp + 0x180, x);
            let cx = fx.mem.u16(sp + 0x1C0);
            let x = fx.mem.u16(sp + 0x180);
            let v = toward(fx, g, t, x, cx, jal);
            fx.mem.set16(sp + 0x168, v);
            fx.mem.set16(sp + 0x150, v);
            fx.submit(d, ot, 0);
        }
        s += step;
    }
}

/// Update 2 [F 0x1E05AB4]: six arcs (+0x250 + 0x60 k) along the node
/// pairs of the two chains, alternating direction, points at +0xC04.
fn chains(fx: &mut Fx) {
    for i in 0..2u32 {
        for j in 0..3u32 {
            let k = j + 4 * i;
            let g = D + 0x250 + 0x60 * k;
            set_counter4(fx, g, 0x24, 0, 0xFF);
            fx.mem.write_u8(g + 4, 1);
            fx.mem.write_u32(g + 8, D + 0xC04);
            fx.mem.write_u8(g + 0x30, 0x64);
            fx.mem.write_u8(g + 0x31, 8);
            let (a, b) = if j & 1 != 0 { (0x10 + 4 * k, 0xC + 4 * k) } else { (0xC + 4 * k, 0x10 + 4 * k) };
            let v = fx.mem.read_u8(D + a);
            fx.mem.write_u8(g + 0x32, v);
            let v = fx.mem.read_u8(D + b);
            fx.mem.write_u8(g + 0x33, v);
            fx.mem.write_u32(g + 0x38, 0x3F80_0000);
            fx.mem.write_u8(g + 0x34, 1);
            fx.mem.write_u16(g + 0x36, 0x40);
            for (o, v) in [(0x46u32, 0x10u8), (0x45, 0x40), (0x44, 0x80), (0x47, 0xC0), (0x4A, 0x10), (0x49, 0x40), (0x48, 0x80)] {
                fx.mem.write_u8(g + o, v);
            }
            let mut m = 0u32;
            while m < u32::from(fx.mem.read_u8(g + 0x31)) + 1 {
                fx.mem.set16(g + 0x1E + 2 * m, -1);
                fx.mem.set16(g + 0xC + 2 * m, -1);
                m += 1;
            }
        }
    }
}

/// Update 40 [F 0x1E05C14]: the beams (+0xA0) at node +0x28, blinking.
fn beams_start(fx: &mut Fx, s1: u64) {
    let g = D + 0xA0;
    set_counter4(fx, g, 0x10, 0, 0xFF);
    fx.mem.write_u8(g + 5, 0);
    fx.mem.write_u8(g + 4, 4);
    fx.mem.set32(g + 8, 0x20);
    let h = fx.mem.i32(D + 4);
    let n = fx.mem.i32(D + 0x28);
    node_position_saving(fx, h, n, g + 0xC, u64::from(D), Some(s1), 0x01E0_5C54);
    for (o, v) in [(0x1Fu32, 0x90u8), (0x1C, 0x90), (0x1D, 0x90), (0x1E, 0x90), (0x21, 0), (0x22, 0), (0x20, 0x60), (0x23, 0x20)] {
        fx.mem.write_u8(g + o, v);
    }
}

/// (rand() >> 4) % 8 (masks, signed) x 200 - 800, / 100.
fn jitter(fx: &mut Fx, ra: u32) -> f32 {
    let v = (fx.rand_from(D, ra) >> 4).wrapping_rem(8);
    ee::div(ee::from_i32(v.wrapping_mul(200) - 800), 100.0)
}

/// Update 55 [F 0x1E05C88]: at the target (sp + 0x70) three ray bursts
/// aimed from node +0x28 and fanned, the mote cloud, the flash, four bars,
/// eight bead beams and the shake.
fn strike(fx: &mut Fx, sp: u32) {
    let at = sp + 0x70;
    for k in 0..3u32 {
        let r = D + 0x4F0 + 0x74 * k;
        set_counter4(fx, r, 0x12, 0, 0xFF);
        fx.mem.write_u8(r + 4, 0);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 8, 0x1E);
        let x = fx.rand_from(D, 0x01E0_5CE0);
        fx.mem.set8(r + 9, x >> 4);
        fx.mem.write_u32(r + 0x18, 0x42B4_0000);
        fx.mem.write_u32(r + 0xC, BYTES);
        fx.mem.write_u32(r + 0x10, 0x4080_0000);
        fx.mem.write_u32(r + 0x14, 0x3F99_999A);
        for (o, v) in [(0x1Eu32, 0x80u8), (0x1D, 0x80), (0x1C, 0x10), (0x20, 0x80), (0x21, 0x80), (0x22, 0x80), (0x23, 0xC0)] {
            fx.mem.write_u8(r + o, v);
        }
        copy_v4(fx, at, r + 0x24);
        let h = fx.mem.i32(D + 4);
        let n = fx.mem.i32(D + 0x28);
        node_position_saving(fx, h, n, sp + 0x80, u64::from(D), Some(u64::from(0x74 * k)), 0x01E0_5D70);
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x80 + 4 * c), fx.mem.f(at + 4 * c));
            fx.mem.setf(sp + 0x80 + 4 * c, v);
        }
        aim_angles(fx, sp + 0x80, r + 0x54);
        let (o, d) = match k {
            0 => (0x54u32, -0x2000),
            1 => (0x58, 0x2000),
            _ => (0x58, -0x2000),
        };
        let v = fx.mem.i32(r + o).wrapping_add(d);
        fx.mem.set32(r + o, v);
    }
    let g = D + 0xBCC;
    set_counter4(fx, g, 1, 4, 0xFF);
    fx.mem.write_u8(g + 4, 4);
    fx.mem.write_u32(g + 8, D + 0x1244);
    for (o, v) in [(0x1Cu32, 8u8), (0x1D, 8), (0x1E, 1), (0x1F, 0), (0x20, 4), (0x22, 0xA0), (0x23, 0xA0), (0x24, 0xA0), (0x25, 0x40)] {
        fx.mem.write_u8(g + o, v);
    }
    fx.mem.write_u32(g + 0x28, 0x4316_0000);
    fx.mem.write_u32(g + 0x2C, 0x4120_0000);
    fx.mem.write_u32(g + 0x34, 0);
    fx.mem.write_u32(g + 0x30, 0);
    let mut k = 0u32;
    while k < u32::from(fx.mem.read_u8(g + 0x1D)) {
        let i = first_free(fx, D + 0x1244, 0x28, 8);
        k += 1;
        if i < 0 {
            continue;
        }
        fx.mem.set16(g + 0xC + 2 * (k - 1), i);
        let p = D + 0x1244 + 0x28 * i as u32;
        let delay = (fx.rand_from(D, 0x01E0_5EFC) >> 4) & 7;
        set_counter(fx, p, 0x20, delay);
        let v = ((fx.rand_from(D, 0x01E0_5F1C) >> 4) & 3) << 4;
        fx.mem.set8(p + 3, v);
        for (o, src, ra) in [(4u32, 0x70u32, 0x01E0_5F34u32), (0xC, 0x78, 0x01E0_5F98), (8, 0x74, 0x01E0_5FFC)] {
            let j = jitter(fx, ra);
            let v = ee::add(fx.mem.f(sp + src), j);
            fx.mem.setf(p + o, v);
        }
        for o in [0x24u32, 0x20, 0x1C, 0x18, 0x14, 0x10] {
            fx.mem.write_u32(p + o, 0);
        }
    }
    let r = D + 0xC4;
    set_counter4(fx, r, 0x16, 0, 0xFF);
    fx.mem.write_u8(r + 5, 0);
    fx.mem.write_u8(r + 4, 0x81);
    fx.mem.write_u8(r + 7, 9);
    fx.mem.write_u32(r + 0x10, 0x442F_0000);
    fx.mem.write_u32(r + 0x14, 0);
    for (o, v) in [(0xAu32, 0xC0u8), (0xC, 0xC0), (0xD, 0xC0), (0xE, 0xC0), (8, 0x20), (9, 0x20), (0xF, 0xC0)] {
        fx.mem.write_u8(r + o, v);
    }
    for o in [0x44u32, 0x40, 0x3C] {
        fx.mem.write_u32(r + o, 0);
    }
    copy_v4(fx, at, r + 0x1C);
    for k in 0..4u32 {
        let r = D + 0x110 + 0x50 * k;
        set_counter4(fx, r, 0x14, 0, 0xFF);
        fx.mem.write_u8(r + 5, 0);
        fx.mem.write_u8(r + 4, 0x80);
        fx.mem.write_u32(r + 0x10, if k != 0 { 0x4496_0000 } else { 0x44FA_0000 });
        fx.mem.setf(r + 0x18, ee::mul(45.0, ee::from_i32(k as i32)));
        fx.mem.write_u32(r + 0x14, 0x4248_0000);
        for (o, v) in [(0xCu32, 0x80u8), (0xD, 0x80), (0xE, 0x80), (0xF, 0x20), (0xA, 0x80), (9, 0x10), (8, 0x20)] {
            fx.mem.write_u8(r + o, v);
        }
        fx.mem.write_u32(r + 0x1C, 0x3FC0_0000);
        copy_v4(fx, at, r + 0x20);
    }
    for j in 0..8i32 {
        let i = first_free(fx, D + 0x64C, 0x58, 0x10);
        if i < 0 {
            continue;
        }
        let count = (((fx.rand_from(D, 0x01E0_6238) >> 4) & 3) + 0x18) & 0xFF;
        let delay = (fx.rand_from(D, 0x01E0_6250) >> 4) & 7;
        let b = D + 0x64C + 0x58 * i as u32;
        set_counter4(fx, b, count, delay, 0xFF);
        fx.mem.write_u8(b + 4, 1);
        fx.mem.write_u8(b + 5, 0);
        fx.mem.write_u8(b + 8, 0x1E);
        let x = fx.rand_from(D, 0x01E0_62A4);
        fx.mem.set8(b + 9, x >> 4);
        fx.mem.write_u8(b + 0xA, 0);
        fx.mem.setf(b + 0x1C, ee::add(10.0, ee::mul(2.0, ee::from_i32(j))));
        fx.mem.write_u32(b + 0x20, 0);
        fx.mem.write_u32(b + 0x10, BYTES);
        let x = fx.rand_from(D, 0x01E0_62F4) >> 4;
        let h = fx.div(x, 0x64).1 - 0x32;
        fx.mem.setf(b + 0x14, ee::add(f32::from_bits(0x3F99_999A), ee::div(ee::from_i32(h), 100.0)));
        for (o, v) in [(0x26u32, 0x80u8), (0x25, 0x30), (0x24, 0x10), (0x27, 0xF0)] {
            fx.mem.write_u8(b + o, v);
        }
        copy_v4(fx, at, b + 0x28);
    }
    let s = D + 0x40;
    set_counter4(fx, s, 0x14, 0, 0xFF);
    fx.mem.write_u8(s + 5, 0);
    fx.mem.write_u8(s + 4, 1);
    fx.mem.write_u32(s + 0x14, 0x4000_0000);
    fx.mem.write_u32(s + 0x18, 0x3FE6_6666);
    fx.mem.write_u32(s + 0x1C, 0x3F80_0000);
    fx.mem.write_u32(s + 0x30, 0);
    fx.mem.write_u32(s + 0x34, 0x42A0_0000);
    fx.mem.write_u32(s + 0x50, 0x4100_0000);
    copy_v4(fx, at, D + 0x60);
}

fn step(fx: &mut Fx, sp: u32, param: i32) -> i32 {
    let n = fx.mem.u16(D) + 1;
    fx.mem.set16(D, n);
    let t = fx.mem.i32(D + 8);
    unit_position_saving(fx, t, sp + 0x70, u64::from(D), 0x01E0_5A78);
    match fx.mem.u16(D) {
        0x69 => return 8,
        0x37 => strike(fx, sp),
        0x28 => beams_start(fx, i64::from(param) as u64),
        2 => chains(fx),
        _ => {}
    }
    shake_saving(fx, D + 0x40, u64::from(D), 0x01E0_643C, 0x01E0_00E0, [0x01E0_03E0, 0x01E0_03F4]);
    blib::star::flash(fx, 0x01E0_11E0, D + 0xC4, OT, 1);
    blib::cross::bars(fx, 0x01E0_4EA0, D + 0x110, OT, 4);
    blib::arc::arcs(fx, 0x01E0_2900, D + 0x250, OT, 7);
    blib::ray::rays(fx, 0x01E0_1D90, D + 0x4F0, OT, 3);
    blib::beam::beads(fx, 0x01E0_0410, D + 0x64C, OT, 8);
    beams(fx, D + 0xA0, OT, 1);
    blib::mote::motes(fx, D + 0xBCC, OT, 1);
    if fx.mem.u16(D) == 0x27 {
        nodes(fx, 1);
    }
    match fx.mem.u16(D) {
        0x36 => 2,
        0x2F => 1,
        _ => 0,
    }
}
