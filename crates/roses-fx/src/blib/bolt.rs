//! Bolts of the effect library: a jagged line
//! of points between two ends (0x102A50) drawn as textured strips, a few
//! widened copies per segment.
use super::beam::length3;
use super::exe::{camera_vp_into, countdown, node_source, tex_rect_strip_a3};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

/// 0x102A50(p) [F]: the jagged path p = {n i32, amplitude f32 +4, start
/// +8, end +0x14, out +0x20 (u32)}: out[0] = start, out[n] = end (vec3s of
/// 12 bytes); the step (end - start) / n (sp + 0x90 of a 0xA0 frame);
/// spread a = fixsfsi(200 amplitude |step|) (0x20C850, 0x1A2B80); points k
/// = 1..n-1: start + k step + ((rand >> 4) % a - a / 2) / 100 per axis
/// (the division by a = 0 keeps HI, as the EE).
pub fn jag(fx: &mut Fx, p: u32) {
    let sp = fx.enter(0xA0);
    for k in 0..3u32 {
        let n = ee::from_i32(fx.mem.i32(p));
        let d = ee::sub(fx.mem.f(p + 0x14 + 4 * k), fx.mem.f(p + 8 + 4 * k));
        fx.mem.setf(sp + 0x90 + 4 * k, ee::div(d, n));
    }
    let len = length3(fx, sp + 0x90);
    let amp = ee::mul(fx.mem.f(p + 4), len);
    let out = fx.mem.read_u32(p + 0x20);
    for k in 0..3u32 {
        let v = fx.mem.read_u32(p + 8 + 4 * k);
        fx.mem.write_u32(out + 4 * k, v);
    }
    let spread = ee::mul(200.0, amp);
    let n = fx.mem.read_u32(p);
    let end = out.wrapping_add(n.wrapping_mul(12));
    for k in 0..3u32 {
        let v = fx.mem.read_u32(p + 0x14 + 4 * k);
        fx.mem.write_u32(end + 4 * k, v);
    }
    let half = ee::fixsfsi(spread) >> 1;
    let mut o = out + 0xC;
    let mut i = 1;
    while i < fx.mem.i32(p) {
        for k in 0..3u32 {
            let r = fx.rand() >> 4;
            let d = ee::fixsfsi(spread);
            let (_, hi) = fx.div(r, d);
            let base = ee::add(fx.mem.f(p + 8 + 4 * k), ee::mul(fx.mem.f(sp + 0x90 + 4 * k), ee::from_i32(i)));
            let v = ee::add(base, ee::div(ee::from_i32(hi.wrapping_sub(half)), 100.0));
            fx.mem.setf(o + 4 * k, v);
        }
        i += 1;
        o += 0xC;
    }
    fx.leave(0xA0);
}

/// 0x102CB0(p) [F]: the helix path p = {n i32, radius f32
/// +4, phase +8, step +0xC (degrees), start +0x10, end +0x1C, out +0x28}:
/// out[0] = start, out[n] = end (vec3s of 12 bytes), then points k =
/// 1..n-1 = R x (radius cos b, len / n x k, radius sin b) + start, b =
/// phase + k step (libm cosf / sinf), R the chord's aim (pitch atan2f of
/// the horizontal length and dy, yaw of dx and dz, 65536 / 2 pi, 0x1A2B80;
/// 0x20BDA0 at sp + 0x60 of a 0xD0 frame), each through 0x20C540 whose w
/// (the stale sp + 0xBC) lands on the next point's x - on out[n].x for the
/// last. The chord sp + 0xA0, the vector sp + 0xB0, the angles sp + 0xC0.
pub fn helix(fx: &mut Fx, p: u32) {
    let sp = fx.enter(0xD0);
    let mut out = fx.mem.read_u32(p + 0x28);
    for c in 0..3u32 {
        let v = fx.mem.read_u32(p + 0x10 + 4 * c);
        fx.mem.write_u32(out + 4 * c, v);
    }
    let n = fx.mem.i32(p);
    let end = out.wrapping_add((n as u32).wrapping_mul(12));
    for c in 0..3u32 {
        let v = fx.mem.read_u32(p + 0x1C + 4 * c);
        fx.mem.write_u32(end + 4 * c, v);
    }
    out += 0xC;
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(p + 0x1C + 4 * c), fx.mem.f(p + 0x10 + 4 * c));
        fx.mem.setf(sp + 0xA0 + 4 * c, v);
    }
    let len = length3(fx, sp + 0xA0);
    let f20 = ee::div(len, ee::from_i32(n));
    let (dx, dz) = (fx.mem.f(sp + 0xA0), fx.mem.f(sp + 0xA8));
    // mula.s / madd.s: the products rounded, then the sum.
    let h2 = ee::add(ee::mul(dx, dx), ee::mul(dz, dz));
    let h = fx.sqrtf(h2);
    let dy = fx.mem.f(sp + 0xA4);
    let a = fx.atan2f(h, dy);
    let f12 = ee::mul(65536.0, ee::div(a, f32::from_bits(0x40C9_0FDB)));
    fx.fix_residue(f12, 0x0010_2DC0);
    fx.mem.write_u32(sp + 0xC0, ee::fixsfsi(f12) as u32 & 0xFFFF);
    let (dx, dz) = (fx.mem.f(sp + 0xA0), fx.mem.f(sp + 0xA8));
    let a = fx.atan2f(dx, dz);
    let f12 = ee::mul(65536.0, ee::div(a, f32::from_bits(0x40C9_0FDB)));
    fx.fix_residue(f12, 0x0010_2DFC);
    fx.mem.write_u32(sp + 0xC4, ee::fixsfsi(f12) as u32 & 0xFFFF);
    fx.mem.write_u32(sp + 0xC8, 0);
    let rot = [fx.mem.i32(sp + 0xC0), fx.mem.i32(sp + 0xC4), 0];
    let m = fx.rotation_matrix(rot);
    fx.write_m4(sp + 0x60, &m);
    let mut k = 1i32;
    while k < fx.mem.i32(p) {
        let fk = ee::from_i32(k);
        let b = ee::add(fx.mem.f(p + 8), ee::mul(fx.mem.f(p + 0xC), fk));
        let f21 = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), b), 180.0);
        let c = super::exe::libm_cosf(fx, f21);
        let r = fx.mem.f(p + 4);
        fx.mem.setf(sp + 0xB0, ee::mul(r, c));
        let s = fx.libm_sinf(f21);
        let r = fx.mem.f(p + 4);
        fx.mem.setf(sp + 0xB8, ee::mul(r, s));
        fx.mem.setf(sp + 0xB4, ee::mul(f20, fk));
        let m = fx.read_m4(sp + 0x60);
        let v = fx.read_v4(sp + 0xB0);
        let t = fx.transform(Some(&m), v);
        fx.write_v4(out, t);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(out + 4 * c), fx.mem.f(p + 0x10 + 4 * c));
            fx.mem.setf(out + 4 * c, v);
        }
        k += 1;
        out += 0xC;
    }
    fx.leave(0xD0);
}

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

fn swap64(fx: &mut Fx, sp: u32, a: u32, b: u32) {
    let v = fx.mem.read_u64(sp + a);
    fx.mem.write_u64(sp + 0x230, v);
    let v = fx.mem.read_u64(sp + b);
    fx.mem.write_u64(sp + a, v);
    let v = fx.mem.read_u64(sp + 0x230);
    fx.mem.write_u64(sp + b, v);
}

/// 7754ed71673e [18 B records; b124 0x1E01B50, 724 words] [F]:
/// `bolts(recs, ot, n) -> 1` steps and draws `n` records (0x40 bytes; a
/// 0x340 frame, the strip descriptor at sp + 0x2A8; two 0x18-byte sprite
/// specs (record-relative `lui` / `addiu` at +0x44 / +0x48) at sp +
/// 0x270). Per record: countdown 0x101AF0; own-slot texture +0xB; the ends:
/// with a unit +5 its nodes +9 and (flag 2: an offset +0x1C.. from it) +0xA
/// (0x101BA0), else +0x1C.. and +0x28.. both moving by +0x34..; the path of
/// +8 points, amplitude +0x10 (0x102A50) into sp + 0x150; colour +0x18..
/// (alpha +0x1B), spec +0xC (0x101EE0). Segments from the count gone
/// (unless flag 4) to the length left (flag 1) or the end: both ends
/// projected, (p[0] & 3) + 1 strips of half widths growing from |AB| /
/// +0xD by +0x14, across the minor axis, corners rotated by count +
/// segment; key the nearer depth (flag 0x80: depth 0xFFFFFE, key 0).
pub fn bolts(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x44, 0x48);
    body(fx, spec, recs, ot, n, &mut None, false)
}

/// [`bolts`] and the a3 its compile leaves [F b268
/// 0x1E02340..0x1E02A08; run b268]: as [`bolts_b_a3`] (the same
/// caller-relative w pointers) except per strip the last `ld a3` of the
/// z chain (flag 0x80: vertex 2's (sp + 0x2E0) low XYZ word, else vertex
/// 4's) and, when the corners turn, the last swapped pointer (sp + 0x2F8
/// for turn 1, sp + 0x310 for 2 / 3).
pub fn bolts_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let spec = rel(fx, at, 0x44, 0x48);
    let mut a3 = None;
    let r = body(fx, spec, recs, ot, n, &mut a3, true);
    (r, a3)
}

/// 263ac4803f99 [13 B records; b002 0x1E04CE0, 676 words] [F]: the same
/// routine with its counters in registers, a 0x360 frame whose locals sit
/// at [`bolts`]' addresses (its sp + 0x20 = this one's sp) and the specs'
/// `lui` / `addiu` at +0x30 / +0x38.
pub fn bolts_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    bolts_b_a3(fx, at, recs, ot, n).0
}

/// [`bolts_b`] and the a3 it leaves (helper B19, for a later boundary
/// call that reports it) [F b706 0x1E00F0C..0x1E01448; run b706]: per
/// counting record the strip's spec a3 (0x101EE0), then per segment the
/// projections' w pointers (sp + 0x358 / 0x35C of the 0x360 frame) as far
/// as they get, then per strip the low word of the fourth vertex's XYZ
/// (`ld a3`) before its new z. `None` when no record counted.
pub fn bolts_b_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let spec = rel(fx, at, 0x30, 0x38);
    let mut a3 = None;
    let r = body(fx, spec, recs, ot, n, &mut a3, false);
    (r, a3)
}

/// `plain`: [`bolts`]' own compile's a3 ([`bolts_a3`]), else [`bolts_b`]'s.
fn body(fx: &mut Fx, spec: u32, recs: u32, ot: u32, n: i32, a3: &mut Option<u32>, plain: bool) -> i32 {
    let sp = fx.enter(0x340);
    fx.mem.set32(sp + 0xDC, n);
    for q in 0..3u32 {
        for k in (0..16u32).step_by(8) {
            let v = fx.mem.read_u64(((spec + 0x10 * q) & !15) + k);
            fx.mem.write_u64(sp + 0x270 + 0x10 * q + k, v);
        }
    }
    let mut p = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xDC) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            p += 0x40;
        }
        one(fx, sp, p, ot, a3, plain);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x340);
    1
}

fn copy3(fx: &mut Fx, src: u32, dst: u32) {
    for k in 0..3u32 {
        let v = fx.mem.read_u32(src + 4 * k);
        fx.mem.write_u32(dst + 4 * k, v);
    }
}

fn one(fx: &mut Fx, sp: u32, p: u32, ot: u32, a3: &mut Option<u32>, plain: bool) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 0xB);
    let tex = fx.texture(5, id);
    if fx.mem.read_u8(p + 5) != 0 {
        let v = fx.mem.read_u8(p + 9);
        fx.mem.write_u8(p + 6, v);
        node_source(fx, p, sp + 0x240);
        copy3(fx, sp + 0x240, sp + 0x250);
        if fx.mem.u8(p + 4) & 2 != 0 {
            for k in 0..3u32 {
                let v = ee::add(fx.mem.f(sp + 0x250 + 4 * k), fx.mem.f(p + 0x1C + 4 * k));
                fx.mem.setf(sp + 0x25C + 4 * k, v);
            }
        } else {
            let v = fx.mem.read_u8(p + 0xA);
            fx.mem.write_u8(p + 6, v);
            node_source(fx, p, sp + 0x240);
            copy3(fx, sp + 0x240, sp + 0x25C);
        }
    } else {
        for base in [0x1Cu32, 0x28] {
            for k in 0..3u32 {
                let v = ee::add(fx.mem.f(p + base + 4 * k), fx.mem.f(p + 0x34 + 4 * k));
                fx.mem.setf(p + base + 4 * k, v);
            }
        }
        copy3(fx, p + 0x1C, sp + 0x250);
        copy3(fx, p + 0x28, sp + 0x25C);
    }
    camera_vp_into(fx, sp + 0xE0);
    let pts = fx.mem.u8(p + 8);
    fx.mem.set32(sp + 0x120, pts);
    let a = fx.mem.read_u32(p + 0x10);
    fx.mem.write_u32(sp + 0x124, a);
    fx.mem.write_u32(sp + 0x140, sp + 0x150);
    copy3(fx, sp + 0x250, sp + 0x128);
    copy3(fx, sp + 0x25C, sp + 0x134);
    jag(fx, sp + 0x120);
    let d = sp + 0x2A8;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (s, o) in [(0x1Au32, 0u32), (0x19, 1), (0x18, 2)] {
        let v = fx.mem.read_u8(p + s);
        for b in [0x300u32, 0x2E8, 0x2D0, 0x2B8] {
            fx.mem.write_u8(sp + b + o, v);
        }
    }
    for o in [0x304u32, 0x2EC, 0x2D4, 0x2BC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let v = fx.mem.read_u8(p + 0x1B);
    for o in [0x303u32, 0x2EB, 0x2D3, 0x2BB] {
        fx.mem.write_u8(sp + o, v);
    }
    let spec = sp + 0x270 + 0x18 * fx.mem.u8(p + 0xC) as u32;
    *a3 = Some(tex_rect_strip_a3(fx, d, spec, tex));
    let f = fx.mem.u8(p + 4);
    let total = fx.mem.i32(sp + 0x120);
    let c = fx.mem.u8(p);
    let mut s2 = if f & 4 == 0 && c < total { total - c } else { 0 };
    let mut last = total;
    if f & 1 != 0 {
        let left = fx.mem.u8(p + 2) - fx.mem.u8(p);
        fx.mem.set32(sp + 0xC0, left);
        if left < total {
            last = left;
        }
    }
    fx.mem.set32(sp + 0xC0, last);
    while s2 < fx.mem.i32(sp + 0xC0) {
        segment(fx, sp, p, ot, s2, a3, plain);
        s2 += 1;
    }
}

fn segment(fx: &mut Fx, sp: u32, p: u32, ot: u32, s2: i32, a3: &mut Option<u32>, plain: bool) {
    let m = fx.read_m4(sp + 0xE0);
    let a = (sp + 0x150).wrapping_add((s2 as u32).wrapping_mul(12));
    *a3 = Some(sp + 0x338);
    if !fx.project_at(a, &m, sp + 0x220, sp + 0x338) || !Fx::on_screen(fx.mem.read_u64(sp + 0x220)) {
        return;
    }
    *a3 = Some(sp + 0x33C);
    if !fx.project_at(a + 12, &m, sp + 0x228, sp + 0x33C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x228)) {
        return;
    }
    let dx = ee::from_i32((fx.mem.u16(sp + 0x220) - fx.mem.u16(sp + 0x228)).abs());
    let dy = ee::from_i32((fx.mem.u16(sp + 0x222) - fx.mem.u16(sp + 0x22A)).abs());
    let len = fx.sqrtf(ee::add(ee::mul(dx, dx), ee::mul(dy, dy)));
    let mut w = ee::div(len, ee::from_i32(fx.mem.u8(p + 0xD)));
    let fl = |v: i32| ee::from_i32(v);
    let mut s3 = 0;
    while s3 < (fx.mem.u8(p) & 3) + 1 {
        let g = ee::mul(w, fx.mem.f(p + 0x14));
        w = ee::add(w, ee::mul(fl(s3), g));
        if !ee_le(dx, dy) {
            let y0 = fx.mem.u16(sp + 0x222);
            fix16(fx, sp + 0x2CA, ee::sub(fl(y0), w));
            let y1 = fx.mem.u16(sp + 0x22A);
            fix16(fx, sp + 0x2E2, ee::sub(fl(y1), w));
            fix16(fx, sp + 0x2FA, ee::add(fl(y0), w));
            fix16(fx, sp + 0x312, ee::add(fl(y1), w));
            let x0 = fx.mem.read_u16(sp + 0x220);
            fx.mem.write_u16(sp + 0x2F8, x0);
            fx.mem.write_u16(sp + 0x2C8, x0);
            let x1 = fx.mem.read_u16(sp + 0x228);
            fx.mem.write_u16(sp + 0x310, x1);
            fx.mem.write_u16(sp + 0x2E0, x1);
        } else {
            let x0 = fx.mem.u16(sp + 0x220);
            fix16(fx, sp + 0x2C8, ee::sub(fl(x0), w));
            let x1 = fx.mem.u16(sp + 0x228);
            fix16(fx, sp + 0x2E0, ee::sub(fl(x1), w));
            fix16(fx, sp + 0x2F8, ee::add(fl(x0), w));
            fix16(fx, sp + 0x310, ee::add(fl(x1), w));
            let y0 = fx.mem.read_u16(sp + 0x222);
            fx.mem.write_u16(sp + 0x2FA, y0);
            fx.mem.write_u16(sp + 0x2CA, y0);
            let y1 = fx.mem.read_u16(sp + 0x22A);
            fx.mem.write_u16(sp + 0x312, y1);
            fx.mem.write_u16(sp + 0x2E2, y1);
        }
        let far = fx.mem.u8(p + 4) & 0x80 != 0;
        *a3 = Some(fx.mem.read_u32(sp + if plain && far { 0x2E0 } else { 0x310 }));
        let depth = if far {
            fx.set_xyz_z(sp + 0x310, 0xFF_FFFE);
            for (from, to) in [(0x310u32, 0x2F8u32), (0x2F8, 0x2E0), (0x2E0, 0x2C8)] {
                let z = fx.xyz_z(sp + from);
                fx.set_xyz_z(sp + to, z);
            }
            0.0
        } else {
            for (from, to) in [(0x220u32, 0x2E0u32), (0x2E0, 0x2C8), (0x228, 0x310), (0x310, 0x2F8)] {
                let z = fx.xyz_z(sp + from);
                fx.set_xyz_z(sp + to, z);
            }
            let (a, b) = (fx.mem.f(sp + 0x338), fx.mem.f(sp + 0x33C));
            if ee_lt(a, b) {
                a
            } else {
                b
            }
        };
        for o in [0x2E7u32, 0x2CF, 0x317, 0x2FF] {
            fx.mem.write_u8(sp + o, 0);
        }
        let turn = (fx.mem.u8(p) + s2) & 3;
        if plain && turn != 0 {
            *a3 = Some(sp + if turn == 1 { 0x2F8 } else { 0x310 });
        }
        match turn {
            1 => {
                swap64(fx, sp, 0x2C8, 0x310);
                swap64(fx, sp, 0x2E0, 0x2F8);
            }
            2 => {
                swap64(fx, sp, 0x2C8, 0x2E0);
                swap64(fx, sp, 0x2F8, 0x310);
            }
            3 => {
                swap64(fx, sp, 0x2C8, 0x2F8);
                swap64(fx, sp, 0x2E0, 0x310);
            }
            _ => {}
        }
        let key = ee::fixunssfsi(depth) as i32;
        fx.submit(sp + 0x2A8, ot, key);
        s3 += 1;
    }
}
