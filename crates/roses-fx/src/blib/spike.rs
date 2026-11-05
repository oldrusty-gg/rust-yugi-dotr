//! Spikes of the effect library: thin
//! four-vertex strips from a record's projected origin out to points
//! placed by a table or a spiral (the layout of `beam::beads`).
use super::exe::{aim_angles, atlas_cell, camera_vp_into, countdown, cvt_u32, libm_cosf, mul_mem, node_source, rot_trans_into, tex_rect_strip};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const PI: u32 = 0x4049_0FDB;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

/// cc4f5ef6c4e2 [37 B records; b002 0x1E00870, 696 words] [F]:
/// `spikes(recs, ot, n) -> 1` steps and draws `n` records (0x68 bytes; a
/// 0x290 frame, the descriptor at sp + 0x1F8, a strip of four vertices;
/// a 0x18-byte sprite spec (record-relative `lui` / `addiu` at +0x30 /
/// +0x38) at sp + 0x1C0). Per record: countdown 0x101AF0; flag 4: an
/// own-slot texture +0xA, atlas cell +0xC of layout 0, blend +0xB; else the
/// spec (flag 2 clear: its u1 / v1 less 16); the length left (p[2] less
/// +0xD while the count is below it, else less the count); origin +0x28 +=
/// +0x38, angles +0x48 += +0x58 (i32); the origin from a node (0x101BA0);
/// flag 8: the angles aim at the self unit's node +0xE (0x1F6830,
/// 0x101940); matrix camera x (angles, origin); colours +0x20.. / +0x24..,
/// alpha +0x23 / +0x27 (x count / 8 over the first 8 counts unless flag
/// 0x40); the origin projected (else nothing). Then +8 spikes: flag 1 a
/// table of deltas (+0x10, index +9 stepping 6) x +0x14 / 255, else a
/// 17-degree spiral (radius table x +0x18 / 255, height +0x14 / 2 (1 +
/// next / 255)), times the length left; each projected, half width +0x1C
/// by 0x1018E0, the strip widened across the major axis from the point to
/// the origin; key the nearer depth (flag 0x80: depth 0xFFFFFE, key 0).
pub fn spikes(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    spikes_a3(fx, at, recs, ot, n).0
}

/// [`spikes`] and the a3 it leaves (helper B15, for a later boundary call
/// that reports it) [F 0x1E011A4 / 0x1E013CC / 0x1E01454 / 0x1E01578 of
/// card 682's; run b853]: per counting record the origin projection's w
/// pointer sp + 0x28C, then per spike its projection's sp + 0x288 and,
/// when it is drawn, the low word of the fourth vertex's XYZ (sp + 0x260,
/// `ld a3`) before its new z (the previous spike's tip). `None` when no
/// record counted (the caller's a3 is kept).
pub fn spikes_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let spec = rel(fx, at, 0x30, 0x38);
    let mut a3 = None;
    (body(fx, spec, recs, ot, n, &mut a3), a3)
}

/// f94fca9d1c4f [23 B records; b012 0x1E02660, 844 words] [F]: the same
/// routine compiled with its counters in registers: the spec's `lui` /
/// `addiu` at +0x40 / +0x44 and a 0x220 frame whose locals sit at the
/// same addresses as [`spikes`]' (its sp + 0x70 = this one's sp), so one
/// body serves both (only the dead frame below differs).
pub fn spikes_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x40, 0x44);
    body(fx, spec, recs, ot, n, &mut None)
}

fn body(fx: &mut Fx, spec: u32, recs: u32, ot: u32, n: i32, a3: &mut Option<u32>) -> i32 {
    let sp = fx.enter(0x290);
    let v = fx.mem.read_u64(spec & !15);
    fx.mem.write_u64(sp + 0x1C0, v);
    let v = fx.mem.read_u64((spec & !15) + 8);
    fx.mem.write_u64(sp + 0x1C8, v);
    let v = fx.mem.read_u64((spec + 0x10) & !7);
    fx.mem.write_u64(sp + 0x1D0, v);
    fx.mem.set32(sp + 0xF4, n);
    fx.mem.write_u32(sp + 0xF8, ot);
    fx.mem.set32(sp + 0xB0, 0);
    if 0 < n {
        let id = fx.mem.u8(sp + 0x1C4);
        fx.mem.set32(sp + 0xE0, id);
        let mut p = recs;
        loop {
            if fx.mem.i32(sp + 0xB0) != 0 {
                p += 0x68;
            }
            one(fx, sp, p, a3);
            let v = fx.mem.i32(sp + 0xB0) + 1;
            fx.mem.set32(sp + 0xB0, v);
            if fx.mem.i32(sp + 0xB0) >= fx.mem.i32(sp + 0xF4) {
                break;
            }
        }
    }
    fx.leave(0x290);
    1
}

fn one(fx: &mut Fx, sp: u32, p: u32, a3: &mut Option<u32>) {
    if countdown(fx, p) == 0 {
        return;
    }
    let tex = if fx.mem.u8(p + 4) & 4 != 0 {
        let id = fx.mem.u8(p + 0xA);
        let t = fx.texture(5, id);
        let b = fx.mem.read_u8(p + 0xB);
        fx.mem.write_u8(sp + 0x1E5, b);
        let cell = fx.mem.u8(p + 0xC);
        atlas_cell(fx, sp + 0x1E0, cell, 0);
        t
    } else {
        let (kind, id) = (fx.mem.i32(sp + 0x1C0), fx.mem.i32(sp + 0xE0));
        let t = fx.texture(kind, id);
        for k in (0..0x18u32).step_by(8) {
            let v = fx.mem.read_u64(sp + 0x1C0 + k);
            fx.mem.write_u64(sp + 0x1E0 + k, v);
        }
        t
    };
    if fx.mem.u8(p + 4) & 2 == 0 {
        for o in [0x1ECu32, 0x1F4] {
            let v = fx.mem.i32(sp + o).wrapping_sub(0x10);
            fx.mem.set32(sp + o, v);
        }
    }
    let (c, d) = (fx.mem.u8(p), fx.mem.u8(p + 0xD));
    let mut turn = 0.0f32;
    let mut idx = fx.mem.u8(p + 9);
    let left = if c < d { fx.mem.u8(p + 2) - d } else { fx.mem.u8(p + 2) - c };
    fx.mem.set32(sp + 0xC0, left);
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x28 + 4 * k), fx.mem.f(p + 0x38 + 4 * k));
        fx.mem.setf(p + 0x28 + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = fx.mem.i32(p + 0x48 + 4 * k).wrapping_add(fx.mem.i32(p + 0x58 + 4 * k));
        fx.mem.set32(p + 0x48 + 4 * k, v);
    }
    node_source(fx, p, p + 0x28);
    if fx.mem.u8(p + 4) & 8 != 0 {
        let h = fx.self_handle();
        let slot = fx.mem.u8(p + 0xE);
        fx.node_position(h, slot, sp + 0x1B0);
        for k in 0..3u32 {
            let v = ee::sub(fx.mem.f(sp + 0x1B0 + 4 * k), fx.mem.f(p + 0x28 + 4 * k));
            fx.mem.setf(sp + 0x1A0 + 4 * k, v);
        }
        aim_angles(fx, sp + 0x1A0, p + 0x48);
    }
    camera_vp_into(fx, sp + 0x140);
    rot_trans_into(fx, sp + 0x100, p + 0x48, p + 0x28);
    mul_mem(fx, sp + 0x140, sp + 0x100);
    fx.desc_init(sp + 0x1F8, 0xC, 0x34, 0x4214_2142_1421);
    for (s, d) in [(0x22u32, [0x220u32, 0x208]), (0x26, [0x250, 0x238]), (0x21, [0x221, 0x209]), (0x25, [0x251, 0x239]), (0x20, [0x222, 0x20A]), (0x24, [0x252, 0x23A])] {
        let v = fx.mem.read_u8(p + s);
        for o in d {
            fx.mem.write_u8(sp + o, v);
        }
    }
    for o in [0x254u32, 0x23C, 0x224, 0x20C] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let c = fx.mem.u8(p);
    if c < 8 && fx.mem.u8(p + 4) & 0x40 == 0 {
        let a = fx.mem.u8(p + 0x23) >> 3;
        let v = fx.mult(a, c);
        for o in [0x23Bu32, 0x223, 0x20B] {
            fx.mem.set8(sp + o, v);
        }
        // mult1: LO1, not LO.
        let v = (fx.mem.u8(p + 0x27) >> 3).wrapping_mul(fx.mem.u8(p));
        fx.mem.set8(sp + 0x253, v);
    } else {
        let v = fx.mem.read_u8(p + 0x23);
        for o in [0x23Bu32, 0x223, 0x20B] {
            fx.mem.write_u8(sp + o, v);
        }
        let v = fx.mem.read_u8(p + 0x27);
        fx.mem.write_u8(sp + 0x253, v);
    }
    tex_rect_strip(fx, sp + 0x1F8, sp + 0x1E0, tex);
    for o in [0x198u32, 0x194, 0x190] {
        fx.mem.write_u32(sp + o, 0);
    }
    let m = fx.read_m4(sp + 0x140);
    *a3 = Some(sp + 0x28C);
    if !fx.project_at(sp + 0x190, &m, sp + 0x188, sp + 0x28C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x188)) {
        return;
    }
    fx.mem.set32(sp + 0xD0, 0);
    while fx.mem.i32(sp + 0xD0) < fx.mem.u8(p + 8) {
        let (x, y, z);
        if fx.mem.u8(p + 4) & 1 != 0 {
            let t = fx.mem.read_u32(p + 0x10).wrapping_add((idx & 0xFF) as u32);
            let s = fx.mem.f(p + 0x14);
            let b = |fx: &mut Fx, k: u32| fx.mem.u8(t + k);
            let (d0, d1, d2) = (b(fx, 0) - b(fx, 1), b(fx, 2) - b(fx, 3), b(fx, 4) - b(fx, 5));
            idx = (idx + 6) & 0xFF;
            x = ee::div(ee::mul(s, ee::from_i32(d0)), 255.0);
            y = ee::div(ee::mul(s, ee::from_i32(d1)), 255.0);
            z = ee::div(ee::mul(s, ee::from_i32(d2)), 255.0);
        } else {
            turn = ee::add(turn, 17.0);
            if !ee_le(turn, 360.0) {
                turn = ee::sub(turn, 360.0);
            }
            let t = fx.mem.read_u32(p + 0x10);
            let a = cvt_u32(fx.mem.read_u8(t.wrapping_add((idx & 0xFF) as u32)) as u32);
            let r = ee::div(ee::mul(fx.mem.f(p + 0x18), a), 255.0);
            let ang = ee::div(ee::mul(f32::from_bits(PI), turn), 180.0);
            let cs = libm_cosf(fx, ang);
            x = ee::mul(r, cs);
            let sn = fx.libm_sinf(ang);
            z = ee::mul(r, sn);
            let h = ee::div(fx.mem.f(p + 0x14), 2.0);
            let b = cvt_u32(fx.mem.read_u8(t.wrapping_add(((idx + 1) & 0xFF) as u32)) as u32);
            idx = (idx + 2) & 0xFF;
            y = ee::add(h, ee::mul(h, ee::div(b, 255.0)));
        }
        let l = ee::from_i32(fx.mem.i32(sp + 0xC0));
        fx.mem.setf(sp + 0x190, ee::mul(x, l));
        fx.mem.setf(sp + 0x194, ee::mul(y, l));
        fx.mem.setf(sp + 0x198, ee::mul(z, l));
        *a3 = Some(sp + 0x288);
        if fx.project_at(sp + 0x190, &m, sp + 0x180, sp + 0x288) && Fx::on_screen(fx.mem.read_u64(sp + 0x180)) {
            *a3 = Some(fx.mem.read_u32(sp + 0x260));
            spike(fx, sp, p);
        }
        let v = fx.mem.i32(sp + 0xD0) + 1;
        fx.mem.set32(sp + 0xD0, v);
    }
}

fn spike(fx: &mut Fx, sp: u32, p: u32) {
    let size = fx.mem.f(p + 0x1C);
    let w = fx.mem.f(sp + 0x288);
    let half = fx.view_scale(size, w);
    let depth;
    if fx.mem.u8(p + 4) & 0x80 != 0 {
        depth = 0.0;
        fx.set_xyz_z(sp + 0x260, 0xFF_FFFE);
        for (from, to) in [(0x260u32, 0x248u32), (0x248, 0x230), (0x230, 0x218)] {
            let z = fx.xyz_z(sp + from);
            fx.set_xyz_z(sp + to, z);
        }
    } else {
        let (a, b) = (fx.mem.f(sp + 0x288), fx.mem.f(sp + 0x28C));
        depth = if ee_lt(a, b) { a } else { b };
        for (from, to) in [(0x180u32, 0x230u32), (0x230, 0x218), (0x188, 0x260), (0x260, 0x248)] {
            let z = fx.xyz_z(sp + from);
            fx.set_xyz_z(sp + to, z);
        }
    }
    let (s5, s6) = (fx.mem.u16(sp + 0x18A), fx.mem.u16(sp + 0x182));
    for o in [0x267u32, 0x24F, 0x237, 0x21F] {
        fx.mem.write_u8(sp + o, 0);
    }
    let dy = (s5 - s6).abs();
    let (s2, s4) = (fx.mem.u16(sp + 0x180), fx.mem.u16(sp + 0x188));
    let dx = (s4 - s2).abs();
    let f = |v: i32| ee::from_i32(v);
    if dy < dx {
        fx.mem.set16(sp + 0x21A, s6);
        fix16(fx, sp + 0x232, ee::sub(f(s6), half));
        fix16(fx, sp + 0x24A, ee::add(f(s6), half));
        fx.mem.set16(sp + 0x262, s5);
        if s2 < s4 {
            fix16(fx, sp + 0x218, ee::sub(f(s2), half));
        } else {
            fix16(fx, sp + 0x218, ee::add(f(s2), half));
        }
        fx.mem.set16(sp + 0x248, s2);
        fx.mem.set16(sp + 0x230, s2);
        fx.mem.set16(sp + 0x260, s4);
    } else {
        fx.mem.set16(sp + 0x218, s2);
        fix16(fx, sp + 0x230, ee::sub(f(s2), half));
        fix16(fx, sp + 0x248, ee::add(f(s2), half));
        fx.mem.set16(sp + 0x260, s4);
        if s6 < s5 {
            fix16(fx, sp + 0x21A, ee::sub(f(s6), half));
        } else {
            fix16(fx, sp + 0x21A, ee::add(f(s6), half));
        }
        fx.mem.set16(sp + 0x24A, s6);
        fx.mem.set16(sp + 0x232, s6);
        fx.mem.set16(sp + 0x262, s5);
    }
    let key = ee::fixunssfsi(depth) as i32;
    let ot = fx.mem.read_u32(sp + 0xF8);
    fx.submit(sp + 0x1F8, ot, key);
}

/// 9ad39df97494 [1 B record; card 615's 0x1E02700, 844 words] (helper
/// B21) [F]: `spikes_c(recs, ot, n) -> 1`, a ray variant of [`spikes`]
/// (0x6C-byte records, the spec's `lui` / `addiu` at +0x38 / +0x40, the
/// same 0x290 frame and descriptor at sp + 0x1F8). Per record: countdown;
/// flag 2: an own-slot texture +0xA, cell +0xC of layout 0, blend +0xB
/// (else the spec, unchanged); the length: mask +3 = 0xFF: the count, else
/// count & mask (0: +9 = rand >> 4 first); the spiral step 20 + table[+9];
/// origin +0x2C += +0x3C, angles +0x4C += +0x5C; colours +0x24.. /
/// +0x28.., alphas +0x27 / +0x2B as they are. Then +8 rays: a point A
/// (flag 1: table deltas x +0x14 / 255; else the spiral at radius +0x1C x
/// table / 255, height h + h table / 255 with h = +0x14 / 2) times the
/// length, B = +0x18 A, both projected (else the next ray); half widths
/// +0x20 at each end (0x1018E0); key the nearer depth (flag 0x80: depth
/// 0xFFFFFE, key 0); flag 4: one strip widened across the major axis by
/// A's width, else two strips (vertical and horizontal) A to B.
pub fn spikes_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x38, 0x40);
    let sp = fx.enter(0x290);
    let v = fx.mem.read_u64(spec & !15);
    fx.mem.write_u64(sp + 0x1C0, v);
    let v = fx.mem.read_u64((spec & !15) + 8);
    fx.mem.write_u64(sp + 0x1C8, v);
    let v = fx.mem.read_u64((spec + 0x10) & !7);
    fx.mem.write_u64(sp + 0x1D0, v);
    fx.mem.set32(sp + 0x104, n);
    fx.mem.write_u32(sp + 0x108, ot);
    fx.mem.set32(sp + 0xC0, 0);
    if 0 < n {
        let id = fx.mem.u8(sp + 0x1C4);
        fx.mem.set32(sp + 0xF0, id);
        let mut p = recs;
        loop {
            if fx.mem.i32(sp + 0xC0) != 0 {
                p += 0x6C;
            }
            ray_record(fx, sp, p);
            let v = fx.mem.i32(sp + 0xC0) + 1;
            fx.mem.set32(sp + 0xC0, v);
            if fx.mem.i32(sp + 0xC0) >= fx.mem.i32(sp + 0x104) {
                break;
            }
        }
    }
    fx.leave(0x290);
    1
}

fn ray_record(fx: &mut Fx, sp: u32, p: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let tex = if fx.mem.u8(p + 4) & 2 != 0 {
        let id = fx.mem.u8(p + 0xA);
        let t = fx.texture(5, id);
        let b = fx.mem.read_u8(p + 0xB);
        fx.mem.write_u8(sp + 0x1E5, b);
        let cell = fx.mem.u8(p + 0xC);
        atlas_cell(fx, sp + 0x1E0, cell, 0);
        t
    } else {
        let (kind, id) = (fx.mem.i32(sp + 0x1C0), fx.mem.i32(sp + 0xF0));
        let t = fx.texture(kind, id);
        for k in (0..0x18u32).step_by(8) {
            let v = fx.mem.read_u64(sp + 0x1C0 + k);
            fx.mem.write_u64(sp + 0x1E0 + k, v);
        }
        t
    };
    let mut s4 = fx.mem.u8(p + 3);
    let s2;
    if s4 == 0xFF {
        s4 = fx.mem.u8(p + 2);
        s2 = s4 - fx.mem.u8(p);
    } else {
        s2 = s4 - (fx.mem.u8(p) & s4);
        if s2 == s4 {
            let r = fx.rand();
            fx.mem.set8(p + 9, r >> 4);
        }
    }
    let mut idx = fx.mem.u8(p + 9);
    let t = fx.mem.read_u32(p + 0x10);
    let step = ee::add(20.0, cvt_u32(u32::from(fx.mem.read_u8(t.wrapping_add(idx as u32)))));
    idx = (idx + 1) & 0xFF;
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x2C + 4 * k), fx.mem.f(p + 0x3C + 4 * k));
        fx.mem.setf(p + 0x2C + 4 * k, v);
    }
    for k in 0..3u32 {
        let v = fx.mem.i32(p + 0x4C + 4 * k).wrapping_add(fx.mem.i32(p + 0x5C + 4 * k));
        fx.mem.set32(p + 0x4C + 4 * k, v);
    }
    let mut turn = 0.0f32;
    node_source(fx, p, p + 0x2C);
    camera_vp_into(fx, sp + 0x150);
    rot_trans_into(fx, sp + 0x110, p + 0x4C, p + 0x2C);
    mul_mem(fx, sp + 0x150, sp + 0x110);
    fx.desc_init(sp + 0x1F8, 0xC, 0x34, 0x4214_2142_1421);
    for (s, d) in [(0x26u32, [0x220u32, 0x208]), (0x2A, [0x250, 0x238]), (0x25, [0x221, 0x209]), (0x29, [0x251, 0x239]), (0x24, [0x222, 0x20A]), (0x28, [0x252, 0x23A])] {
        let v = fx.mem.read_u8(p + s);
        for o in d {
            fx.mem.write_u8(sp + o, v);
        }
    }
    for o in [0x254u32, 0x23C, 0x224, 0x20C] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let v = fx.mem.read_u8(p + 0x27);
    for o in [0x23Bu32, 0x223, 0x20B] {
        fx.mem.write_u8(sp + o, v);
    }
    let v = fx.mem.read_u8(p + 0x2B);
    fx.mem.write_u8(sp + 0x253, v);
    tex_rect_strip(fx, sp + 0x1F8, sp + 0x1E0, tex);
    fx.mem.set32(sp + 0xE0, s4 - s2);
    let m = fx.read_m4(sp + 0x150);
    fx.mem.set32(sp + 0xD0, 0);
    while fx.mem.i32(sp + 0xD0) < fx.mem.u8(p + 8) {
        let (x, y, z);
        let t = fx.mem.read_u32(p + 0x10);
        if fx.mem.u8(p + 4) & 1 != 0 {
            let q = t.wrapping_add(idx as u32);
            let s = fx.mem.f(p + 0x14);
            let b = |fx: &mut Fx, k: u32| fx.mem.u8(q + k);
            let (d0, d1, d2) = (b(fx, 0) - b(fx, 1), b(fx, 2) - b(fx, 3), b(fx, 4) - b(fx, 5));
            idx = (idx + 6) & 0xFF;
            x = ee::div(ee::mul(s, ee::from_i32(d0)), 255.0);
            y = ee::div(ee::mul(s, ee::from_i32(d1)), 255.0);
            z = ee::div(ee::mul(s, ee::from_i32(d2)), 255.0);
        } else {
            turn = ee::add(turn, step);
            if !ee_le(turn, 360.0) {
                turn = ee::sub(turn, 360.0);
            }
            let a = cvt_u32(u32::from(fx.mem.read_u8(t.wrapping_add(idx as u32))));
            let r = ee::div(ee::mul(fx.mem.f(p + 0x1C), a), 255.0);
            let ang = ee::div(ee::mul(f32::from_bits(PI), turn), 180.0);
            let cs = libm_cosf(fx, ang);
            x = ee::mul(r, cs);
            let sn = fx.libm_sinf(ang);
            z = ee::mul(r, sn);
            let h = ee::div(fx.mem.f(p + 0x14), 2.0);
            let b = cvt_u32(u32::from(fx.mem.read_u8(t.wrapping_add(((idx + 1) & 0xFF) as u32))));
            idx = (idx + 2) & 0xFF;
            y = ee::add(h, ee::div(ee::mul(h, b), 255.0));
        }
        let l = ee::from_i32(fx.mem.i32(sp + 0xE0));
        let a = [ee::mul(x, l), ee::mul(y, l), ee::mul(z, l)];
        for (k, v) in a.into_iter().enumerate() {
            fx.mem.setf(sp + 0x1A0 + 4 * k as u32, v);
            let b = ee::mul(fx.mem.f(p + 0x18), v);
            fx.mem.setf(sp + 0x1B0 + 4 * k as u32, b);
        }
        if fx.project_at(sp + 0x1A0, &m, sp + 0x190, sp + 0x288)
            && Fx::on_screen(fx.mem.read_u64(sp + 0x190))
            && fx.project_at(sp + 0x1B0, &m, sp + 0x198, sp + 0x28C)
            && Fx::on_screen(fx.mem.read_u64(sp + 0x198))
        {
            ray(fx, sp, p);
        }
        let v = fx.mem.i32(sp + 0xD0) + 1;
        fx.mem.set32(sp + 0xD0, v);
    }
}

fn ray(fx: &mut Fx, sp: u32, p: u32) {
    let size = fx.mem.f(p + 0x20);
    let w = fx.mem.f(sp + 0x288);
    let wa = fx.view_scale(size, w);
    let size = fx.mem.f(p + 0x20);
    let w = fx.mem.f(sp + 0x28C);
    let wb = fx.view_scale(size, w);
    let depth;
    if fx.mem.u8(p + 4) & 0x80 != 0 {
        depth = 0.0;
        fx.set_xyz_z(sp + 0x260, 0xFF_FFFE);
        for (from, to) in [(0x260u32, 0x248u32), (0x248, 0x230), (0x230, 0x218)] {
            let z = fx.xyz_z(sp + from);
            fx.set_xyz_z(sp + to, z);
        }
    } else {
        let (a, b) = (fx.mem.f(sp + 0x288), fx.mem.f(sp + 0x28C));
        depth = if ee_lt(a, b) { a } else { b };
        for (from, to) in [(0x190u32, 0x230u32), (0x230, 0x218), (0x198, 0x260), (0x260, 0x248)] {
            let z = fx.xyz_z(sp + from);
            fx.set_xyz_z(sp + to, z);
        }
    }
    for o in [0x267u32, 0x24F, 0x237, 0x21F] {
        fx.mem.write_u8(sp + o, 0);
    }
    let f = |v: i32| ee::from_i32(v);
    let ot = fx.mem.read_u32(sp + 0x108);
    if fx.mem.u8(p + 4) & 4 != 0 {
        let (s5, s6) = (fx.mem.u16(sp + 0x19A), fx.mem.u16(sp + 0x192));
        let dy = (s5 - s6).abs();
        let (s2, s4) = (fx.mem.u16(sp + 0x190), fx.mem.u16(sp + 0x198));
        let dx = (s4 - s2).abs();
        let half = wa;
        if dy < dx {
            fx.mem.set16(sp + 0x21A, s6);
            fix16(fx, sp + 0x232, ee::sub(f(s6), half));
            fix16(fx, sp + 0x24A, ee::add(f(s6), half));
            fx.mem.set16(sp + 0x262, s5);
            if s2 < s4 {
                fix16(fx, sp + 0x218, ee::sub(f(s2), half));
            } else {
                fix16(fx, sp + 0x218, ee::add(f(s2), half));
            }
            fx.mem.set16(sp + 0x248, s2);
            fx.mem.set16(sp + 0x230, s2);
            fx.mem.set16(sp + 0x260, s4);
        } else {
            fx.mem.set16(sp + 0x218, s2);
            fix16(fx, sp + 0x230, ee::sub(f(s2), half));
            fix16(fx, sp + 0x248, ee::add(f(s2), half));
            fx.mem.set16(sp + 0x260, s4);
            if s6 < s5 {
                fix16(fx, sp + 0x21A, ee::sub(f(s6), half));
            } else {
                fix16(fx, sp + 0x21A, ee::add(f(s6), half));
            }
            fx.mem.set16(sp + 0x24A, s6);
            fx.mem.set16(sp + 0x232, s6);
            fx.mem.set16(sp + 0x262, s5);
        }
        let key = ee::fixunssfsi(depth) as i32;
        fx.submit(sp + 0x1F8, ot, key);
        return;
    }
    // Vertical: A's y -/+ wa to B's y -/+ wb at A's / B's x.
    let (ay, by) = (fx.mem.u16(sp + 0x192), fx.mem.u16(sp + 0x19A));
    fix16(fx, sp + 0x21A, ee::sub(f(ay), wa));
    fix16(fx, sp + 0x232, ee::sub(f(by), wb));
    fix16(fx, sp + 0x24A, ee::add(f(ay), wa));
    fix16(fx, sp + 0x262, ee::add(f(by), wb));
    let ax = fx.mem.read_u16(sp + 0x190);
    fx.mem.write_u16(sp + 0x248, ax);
    fx.mem.write_u16(sp + 0x218, ax);
    let bx = fx.mem.read_u16(sp + 0x198);
    fx.mem.write_u16(sp + 0x260, bx);
    fx.mem.write_u16(sp + 0x230, bx);
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(sp + 0x1F8, ot, key);
    // Horizontal: A's x -/+ wa to B's x -/+ wb at A's / B's y.
    let (ax, bx) = (fx.mem.u16(sp + 0x190), fx.mem.u16(sp + 0x198));
    fix16(fx, sp + 0x218, ee::sub(f(ax), wa));
    fix16(fx, sp + 0x230, ee::sub(f(bx), wb));
    fix16(fx, sp + 0x248, ee::add(f(ax), wa));
    fix16(fx, sp + 0x260, ee::add(f(bx), wb));
    let ay = fx.mem.read_u16(sp + 0x192);
    fx.mem.write_u16(sp + 0x24A, ay);
    fx.mem.write_u16(sp + 0x21A, ay);
    let by = fx.mem.read_u16(sp + 0x19A);
    fx.mem.write_u16(sp + 0x262, by);
    fx.mem.write_u16(sp + 0x232, by);
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(sp + 0x1F8, ot, key);
}
