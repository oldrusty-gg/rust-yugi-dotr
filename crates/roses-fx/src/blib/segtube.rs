//! Segment tubes of the effect library: a ring
//! swept along a list of segments, drawn as scrolling textured quads.
use super::exe::{aim_angles, camera_vp_into, countdown, libm_cosf};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx};

const PI: u32 = 0x4049_0FDB;

/// 427eed08045d [6 B records; b369 0x1E02E60, 604 words]
/// [F]: `tubes(at, recs, ot, n) -> 1` steps and draws `n` records (0x2C
/// bytes; a 0x470 frame, the descriptor at sp + 0x3E0: RGBAQ / ST / XYZF2
/// x 4). The routine b761 carries as its own 0x1E06010 (ae37ae16e23f)
/// with its locals 0x10 lower and no stores of the projection flags. Per
/// record: countdown 0x101AF0; the texture (5, +9); radius +0x20 +=
/// +0x24; a ring of k = min(+7, 36) points (libm cosf / sinf, plane by
/// +0xB & 0x18: 0x10 the yz plane, 8 xy, else xz) closed by a copy of the
/// first; S spans +0x10 over the ring, scrolled by the fractions of +0x18
/// / +0x1C x the elapsed count (0x1A2B80). +8 rows (flag 0x20: at most
/// the elapsed count), row r between points r / r + 1 of the list +0xC
/// (12 bytes each; their difference's aim 0x101940 / length 0x20C850
/// computed and dropped); each ring step a quad of the two rings' points
/// (all four must project, 0x204D80); +0xB flag 1: a front-facing quad
/// (screen cross > 0) ends the row. STQ / w, REGION_CLAMP 0 (0x208400),
/// ALPHA 0x48, colour +0x28.. with alpha +0x2B (x count / 8 over the last
/// 8 unless +4 bit 0x40; 0 on odd counts with +0xB flag 2; the far ring's
/// 0 with +0xB 0x40), the texture (0x207E50, filter 1); keyed by the mean
/// w (0 with +4 bit 0x80) through 0x1A2C70 twice.
pub fn tubes(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    tubes_a3(fx, at, recs, ot, n).0
}

/// [`tubes`] and the a3 it leaves (for a later boundary call that reports
/// it) [F 0x204D80 is answered natively, 0x207E50 `ld a3, 0x30(a1)`]:
/// after each ring step the fourth projection's w pointer sp + 0x3CC, the
/// TEX0 low word of a drawn quad (sp + 0x3F0 when the texture is
/// missing), u16 sp + 0x432 when a front-facing quad ends the row. `None`
/// when no ring step ran.
pub fn tubes_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let sp = fx.enter(0x470);
    fx.mem.set32(sp + 0x104, n);
    fx.mem.write_u32(sp + 0x108, ot);
    let mut a3 = None;
    let mut g = recs;
    let mut i = 0;
    fx.mem.set32(sp + 0xB0, i);
    while i < fx.mem.i32(sp + 0x104) {
        if i != 0 {
            g += 0x2C;
        }
        tube(fx, at, sp, g, &mut a3);
        i = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.leave(0x470);
    (1, a3)
}

fn fix(fx: &mut Fx, at: u32, off: u32, v: f32) -> i32 {
    fx.fix_residue(v, at + off + 8);
    ee::fixsfsi(v)
}

fn tube(fx: &mut Fx, at: u32, sp: u32, g: u32, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 9);
    let t = fx.texture(5, id);
    fx.mem.write_u32(sp + 0xC0, t);
    let r = ee::add(fx.mem.f(g + 0x20), fx.mem.f(g + 0x24));
    fx.mem.setf(g + 0x20, r);
    let mut k = fx.mem.u8(g + 7);
    fx.mem.set32(sp + 0xE0, k);
    if k >= 0x25 {
        k = 0x24;
        fx.mem.set32(sp + 0xE0, k);
    }
    let step = ee::div(360.0, ee::from_i32(k));
    for j in 0..k {
        let plane = fx.mem.u8(g + 0xB) & 0x18;
        let a = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(j))), 180.0);
        let p = sp + 0x150 + 16 * j as u32;
        let (co, so, zo) = match plane {
            0x10 => (8u32, 4u32, 0u32),
            8 => (0, 4, 8),
            _ => (0, 8, 4),
        };
        let c = libm_cosf(fx, a);
        let c = ee::mul(fx.mem.f(g + 0x20), c);
        fx.mem.setf(p + co, c);
        let s = fx.libm_sinf(a);
        let s = ee::mul(fx.mem.f(g + 0x20), s);
        fx.mem.setf(p + so, s);
        fx.mem.write_u32(p + zo, 0);
    }
    let k = fx.mem.i32(sp + 0xE0);
    for c in 0..4u32 {
        let w = fx.mem.read_u32(sp + 0x150 + 4 * c);
        fx.mem.write_u32(sp + 0x150 + 16 * k as u32 + 4 * c, w);
    }
    let su = ee::div(fx.mem.f(g + 0x10), ee::from_i32(k));
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let mut s0 = ee::mul(fx.mem.f(g + 0x18), ee::from_i32(e));
    if !ee_le(s0, 1.0) {
        let w = fix(fx, at, 0x2E0, s0);
        s0 = ee::sub(s0, ee::from_i32(w));
    }
    let mut t0 = ee::mul(fx.mem.f(g + 0x1C), ee::from_i32(e));
    if !ee_le(t0, 1.0) {
        let w = fix(fx, at, 0x324, t0);
        t0 = ee::sub(t0, ee::from_i32(w));
    }
    let d = sp + 0x3E0;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    let rows = fx.mem.u8(g + 8);
    fx.mem.set32(sp + 0xF0, rows);
    if fx.mem.u8(g + 0xB) & 0x20 != 0 {
        let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
        if e < fx.mem.i32(sp + 0xF0) {
            fx.mem.set32(sp + 0xF0, e);
        }
    }
    let ta = ee::add(1.0, t0);
    fx.mem.set32(sp + 0xD0, 0);
    let mut row = 0;
    while row < fx.mem.i32(sp + 0xF0) {
        let q = 12 * row as u32;
        for c in 0..3u32 {
            let pts = fx.mem.read_u32(g + 0xC).wrapping_add(q);
            let v = ee::sub(fx.mem.f(pts + 0xC + 4 * c), fx.mem.f(pts + 4 * c));
            fx.mem.setf(sp + 0x3B0 + 4 * c, v);
        }
        aim_angles(fx, sp + 0x3B0, sp + 0x3D0);
        super::beam::length3(fx, sp + 0x3B0);
        camera_vp_into(fx, sp + 0x110);
        let mut j = 0;
        while j < fx.mem.i32(sp + 0xE0) {
            if !quad(fx, at, sp, g, q, j, su, s0, t0, ta, a3) {
                break;
            }
            j += 1;
        }
        row = fx.mem.i32(sp + 0xD0) + 1;
        fx.mem.set32(sp + 0xD0, row);
    }
}

/// Ring step `j` of the row at point offset `q`: false when a front-facing
/// quad ends the row.
#[allow(clippy::too_many_arguments)]
fn quad(fx: &mut Fx, at: u32, sp: u32, g: u32, q: u32, j: i32, su: f32, s0: f32, t0: f32, ta: f32, a3: &mut Option<u32>) -> bool {
    let ring = sp + 0x150 + 16 * j as u32;
    let mut ok = [false; 4];
    for (v, (po, ro, xyz, w)) in [(0u32, 0u32, 0x400u32, 0x3C0u32), (0, 0x10, 0x418, 0x3C4), (0xC, 0, 0x430, 0x3C8), (0xC, 0x10, 0x448, 0x3CC)].into_iter().enumerate() {
        for c in 0..3u32 {
            let pts = fx.mem.read_u32(g + 0xC).wrapping_add(q);
            let a = ee::add(fx.mem.f(pts + po + 4 * c), fx.mem.f(ring + ro + 4 * c));
            fx.mem.setf(sp + 0x3A0 + 4 * c, a);
        }
        let m = fx.read_m4(sp + 0x110);
        ok[v] = fx.project_at(sp + 0x3A0, &m, sp + xyz, sp + w);
    }
    *a3 = Some(sp + 0x3CC);
    if ok.contains(&false) {
        return true;
    }
    if fx.mem.u8(g + 0xB) & 1 != 0 {
        let (cx, cy) = (fx.mem.u16(sp + 0x430), fx.mem.u16(sp + 0x432));
        *a3 = Some(cy as u32);
        let (dx, dy) = (fx.mem.u16(sp + 0x400) - cx, fx.mem.u16(sp + 0x44A) - cy);
        let a = fx.mult(dy, dx);
        let b = (fx.mem.u16(sp + 0x402) - cy).wrapping_mul(fx.mem.u16(sp + 0x448) - cx);
        if !ee_le(ee::from_i32(a.wrapping_sub(b)), 0.0) {
            return false;
        }
    }
    let (w0, w1, w2, w3) = (fx.mem.f(sp + 0x3C0), fx.mem.f(sp + 0x3C4), fx.mem.f(sp + 0x3C8), fx.mem.f(sp + 0x3CC));
    fx.mem.setf(sp + 0x3F4, ee::div(1.0, w0));
    let sa = ee::add(s0, ee::mul(su, ee::from_i32(j)));
    fx.mem.setf(sp + 0x3F8, ee::div(sa, w0));
    let sb = ee::add(s0, ee::mul(su, ee::from_i32(j + 1)));
    fx.mem.setf(sp + 0x40C, ee::div(1.0, w1));
    fx.mem.setf(sp + 0x424, ee::div(1.0, w2));
    fx.mem.setf(sp + 0x428, ee::div(sa, w2));
    fx.mem.setf(sp + 0x42C, ee::div(t0, w2));
    fx.mem.setf(sp + 0x43C, ee::div(1.0, w3));
    fx.mem.setf(sp + 0x3FC, ee::div(ta, w0));
    fx.mem.setf(sp + 0x410, ee::div(sb, w1));
    fx.mem.setf(sp + 0x414, ee::div(ta, w1));
    fx.mem.setf(sp + 0x440, ee::div(sb, w3));
    fx.mem.setf(sp + 0x444, ee::div(t0, w3));
    let d = sp + 0x3E0;
    fx.desc_clamp(d, 0);
    fx.desc_alpha(d, 0x48);
    let count = fx.mem.u8(g);
    let alpha = if fx.mem.u8(g + 4) & 0x40 == 0 && count < 8 {
        let a = fx.mem.u8(g + 0x2B) >> 3;
        fx.mult(a, count) & 0xFF
    } else {
        fx.mem.u8(g + 0x2B)
    };
    let alpha = if fx.mem.u8(g + 0xB) & 2 != 0 && fx.mem.u8(g) & 1 != 0 { 0 } else { alpha };
    for o in [0x43Bu32, 0x423, 0x40B, 0x3F3] {
        fx.mem.set8(sp + o, alpha);
    }
    if fx.mem.u8(g + 0xB) & 0x40 != 0 {
        fx.mem.write_u8(sp + 0x43B, 0);
        fx.mem.write_u8(sp + 0x423, 0);
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u8(g + 0x2A - c);
        for o in [0x438u32, 0x420, 0x408, 0x3F0] {
            fx.mem.write_u8(sp + o + c, v);
        }
    }
    for o in [0x44Fu32, 0x437, 0x41F, 0x407] {
        fx.mem.write_u8(sp + o, 0);
    }
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.desc_texture(d, t, 1);
    *a3 = Some(if t != 0 { fx.mem.read_u32(t + 0x30) } else { d + 0x10 });
    let f = if fx.mem.u8(g + 4) & 0x80 != 0 {
        0.0
    } else {
        let w = ee::div(ee::add(w3, ee::add(w2, ee::add(w0, w1))), 4.0);
        fx.fix_residue(w, at + 0x880);
        let k = ee::fixunssfsi(w);
        if (k as i32) < 0 {
            let h = ee::from_i32(((k >> 1) | (k & 1)) as i32);
            ee::add(h, h)
        } else {
            ee::from_i32(k as i32)
        }
    };
    fx.fix_residue(f, at + 0x8B8);
    let key = ee::fixunssfsi(f) as i32;
    let ot = fx.mem.read_u32(sp + 0x108);
    fx.submit(d, ot, key);
    true
}

/// df1e4b64bb5a [b040 (card 156) 0x1E05C30, 592 words] [F]:
/// `frustums(at, recs, ot, n) -> 1` steps and draws `n` records (0x48
/// bytes; a 0x680 frame, the descriptor at sp + 0x5F0: RGBAQ / ST / XYZF2
/// x 4, the camera at sp + 0xD0, ring A at sp + 0x110 and ring B at sp +
/// 0x360). [`tubes`]' sibling with one segment per record and two radii:
/// countdown 0x101AF0; the texture (5, +8); radii +0x1C / +0x20 += +0x24
/// / +0x28; two rings of k = min(+7, 36) points (libm cosf / sinf, plane
/// by +0xA & 0xC: 8 the yz plane, 4 xy, else xz) closed by a copy of the
/// first; S spans +0xC over the ring, scrolled by the fractions of +0x14 /
/// +0x18 x the elapsed count (0x1A2B80); the axis +0x38 - +0x2C aimed
/// (0x101940) and measured (0x20C850) and dropped; each ring step a quad
/// of ring A about +0x2C and ring B about +0x38 (all four must project,
/// 0x204D80); +0xA flag 1: a front-facing quad ends the record. STQ / w,
/// REGION_CLAMP 0, ALPHA 0x48, colour +0x44.. with alpha +0x47 (x elapsed
/// / 16 over the first 16 with +0xA 0x10; x count / 8 over the last 8
/// unless +4 bit 0x40; 0 on odd counts with +0xA 2; ring B's 0 with +0xA
/// 0x20), the texture (0x207E50, filter 1); keyed by the mean w (0 with
/// +4 bit 0x80) through 0x1A2C70 twice.
pub fn frustums(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    frustums_a3(fx, at, recs, ot, n).0
}

/// [`frustums`] and the a3 it leaves (as [`tubes_a3`]): the fourth
/// projection's w pointer sp + 0x5DC after each ring step, u16 sp + 0x642
/// when a front-facing quad ends the record, the TEX0 low word of a drawn
/// quad (sp + 0x600 when the texture is missing).
pub fn frustums_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let sp = fx.enter(0x680);
    fx.mem.set32(sp + 0xC4, n);
    fx.mem.write_u32(sp + 0xC8, ot);
    let mut a3 = None;
    let mut g = recs;
    let mut i = 0;
    fx.mem.set32(sp + 0xB0, i);
    while i < fx.mem.i32(sp + 0xC4) {
        if i != 0 {
            g += 0x48;
        }
        frustum(fx, at, sp, g, &mut a3);
        i = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.leave(0x680);
    (1, a3)
}

fn frustum(fx: &mut Fx, at: u32, sp: u32, g: u32, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let t = fx.texture(5, id);
    fx.mem.write_u32(sp + 0xC0, t);
    for o in [0x1Cu32, 0x20] {
        let r = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 8));
        fx.mem.setf(g + o, r);
    }
    let mut k = fx.mem.u8(g + 7);
    if k >= 0x25 {
        k = 0x24;
    }
    let step = ee::div(360.0, ee::from_i32(k));
    for j in 0..k {
        let plane = fx.mem.u8(g + 0xA) & 0xC;
        let a = ee::div(ee::mul(f32::from_bits(PI), ee::mul(step, ee::from_i32(j))), 180.0);
        let p = sp + 0x110 + 16 * j as u32;
        let (co, so, zo) = match plane {
            8 => (8u32, 4u32, 0u32),
            4 => (0, 4, 8),
            _ => (0, 8, 4),
        };
        for (ro, ring) in [(0x1Cu32, 0u32), (0x20, 0x250)] {
            let c = libm_cosf(fx, a);
            let c = ee::mul(fx.mem.f(g + ro), c);
            fx.mem.setf(p + ring + co, c);
            let s = fx.libm_sinf(a);
            let s = ee::mul(fx.mem.f(g + ro), s);
            fx.mem.setf(p + ring + so, s);
            fx.mem.write_u32(p + ring + zo, 0);
        }
    }
    for ring in [0u32, 0x250] {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(sp + 0x110 + ring + 4 * c);
            fx.mem.write_u32(sp + 0x110 + ring + 16 * k as u32 + 4 * c, w);
        }
    }
    let su = ee::div(fx.mem.f(g + 0xC), ee::from_i32(k));
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let mut s0 = ee::mul(fx.mem.f(g + 0x14), ee::from_i32(e));
    if !ee_le(s0, 1.0) {
        let w = fix(fx, at, 0x374, s0);
        s0 = ee::sub(s0, ee::from_i32(w));
    }
    let mut t0 = ee::mul(fx.mem.f(g + 0x18), ee::from_i32(e));
    if !ee_le(t0, 1.0) {
        let w = fix(fx, at, 0x3BC, t0);
        t0 = ee::sub(t0, ee::from_i32(w));
    }
    let d = sp + 0x5F0;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for c in 0..3u32 {
        let v = ee::sub(fx.mem.f(g + 0x38 + 4 * c), fx.mem.f(g + 0x2C + 4 * c));
        fx.mem.setf(sp + 0x5C0 + 4 * c, v);
    }
    aim_angles(fx, sp + 0x5C0, sp + 0x5E0);
    super::beam::length3(fx, sp + 0x5C0);
    camera_vp_into(fx, sp + 0xD0);
    let ta = ee::add(1.0, t0);
    let mut j = 0;
    while j < k {
        if !frustum_quad(fx, at, sp, g, j, su, s0, t0, ta, a3) {
            break;
        }
        j += 1;
    }
}

/// Ring step `j` of a frustum: false when a front-facing quad ends it.
#[allow(clippy::too_many_arguments)]
fn frustum_quad(fx: &mut Fx, at: u32, sp: u32, g: u32, j: i32, su: f32, s0: f32, t0: f32, ta: f32, a3: &mut Option<u32>) -> bool {
    let ring = sp + 0x110 + 16 * j as u32;
    let mut ok = [false; 4];
    for (v, (po, ro, xyz, w)) in [(0x2Cu32, 0u32, 0x610u32, 0x5D0u32), (0x2C, 0x10, 0x628, 0x5D4), (0x38, 0x250, 0x640, 0x5D8), (0x38, 0x260, 0x658, 0x5DC)].into_iter().enumerate() {
        for c in 0..3u32 {
            let a = ee::add(fx.mem.f(g + po + 4 * c), fx.mem.f(ring + ro + 4 * c));
            fx.mem.setf(sp + 0x5B0 + 4 * c, a);
        }
        let m = fx.read_m4(sp + 0xD0);
        ok[v] = fx.project_at(sp + 0x5B0, &m, sp + xyz, sp + w);
    }
    *a3 = Some(sp + 0x5DC);
    if ok.contains(&false) {
        return true;
    }
    if fx.mem.u8(g + 0xA) & 1 != 0 {
        let (cx, cy) = (fx.mem.u16(sp + 0x640), fx.mem.u16(sp + 0x642));
        *a3 = Some(cy as u32);
        let (dx, dy) = (fx.mem.u16(sp + 0x610) - cx, fx.mem.u16(sp + 0x65A) - cy);
        let a = fx.mult(dy, dx);
        let b = (fx.mem.u16(sp + 0x612) - cy).wrapping_mul(fx.mem.u16(sp + 0x658) - cx);
        if !ee_le(ee::from_i32(a.wrapping_sub(b)), 0.0) {
            return false;
        }
    }
    let (w0, w1, w2, w3) = (fx.mem.f(sp + 0x5D0), fx.mem.f(sp + 0x5D4), fx.mem.f(sp + 0x5D8), fx.mem.f(sp + 0x5DC));
    fx.mem.setf(sp + 0x604, ee::div(1.0, w0));
    let sa = ee::add(s0, ee::mul(su, ee::from_i32(j)));
    fx.mem.setf(sp + 0x608, ee::div(sa, w0));
    let sb = ee::add(s0, ee::mul(su, ee::from_i32(j + 1)));
    fx.mem.setf(sp + 0x61C, ee::div(1.0, w1));
    fx.mem.setf(sp + 0x634, ee::div(1.0, w2));
    fx.mem.setf(sp + 0x638, ee::div(sa, w2));
    fx.mem.setf(sp + 0x63C, ee::div(t0, w2));
    fx.mem.setf(sp + 0x64C, ee::div(1.0, w3));
    fx.mem.setf(sp + 0x60C, ee::div(ta, w0));
    fx.mem.setf(sp + 0x620, ee::div(sb, w1));
    fx.mem.setf(sp + 0x624, ee::div(ta, w1));
    fx.mem.setf(sp + 0x650, ee::div(sb, w3));
    fx.mem.setf(sp + 0x654, ee::div(t0, w3));
    let d = sp + 0x5F0;
    fx.desc_clamp(d, 0);
    fx.desc_alpha(d, 0x48);
    let e = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let count = fx.mem.u8(g);
    let base = fx.mem.u8(g + 0x47);
    let flags = fx.mem.u8(g + 0xA);
    let mut alpha = base;
    if flags & 0x10 != 0 && e < 0x10 {
        alpha = fx.mult(e, base >> 4) & 0xFF;
    }
    if fx.mem.u8(g + 4) & 0x40 == 0 && count < 8 {
        alpha = fx.mult(base >> 3, count) & 0xFF;
    }
    let alpha = if flags & 2 != 0 && count & 1 != 0 { 0 } else { alpha };
    for o in [0x64Bu32, 0x633, 0x61B, 0x603] {
        fx.mem.set8(sp + o, alpha);
    }
    if fx.mem.u8(g + 0xA) & 0x20 != 0 {
        fx.mem.write_u8(sp + 0x64B, 0);
        fx.mem.write_u8(sp + 0x633, 0);
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u8(g + 0x46 - c);
        for o in [0x648u32, 0x630, 0x618, 0x600] {
            fx.mem.write_u8(sp + o + c, v);
        }
    }
    for o in [0x65Fu32, 0x647, 0x62F, 0x617] {
        fx.mem.write_u8(sp + o, 0);
    }
    let t = fx.mem.read_u32(sp + 0xC0);
    fx.desc_texture(d, t, 1);
    *a3 = Some(if t != 0 { fx.mem.read_u32(t + 0x30) } else { d + 0x10 });
    let f = if fx.mem.u8(g + 4) & 0x80 != 0 {
        0.0
    } else {
        let w = ee::div(ee::add(w3, ee::add(w2, ee::add(w0, w1))), 4.0);
        fx.fix_residue(w, at + 0x878);
        let k = ee::fixunssfsi(w);
        if (k as i32) < 0 {
            let h = ee::from_i32(((k >> 1) | (k & 1)) as i32);
            ee::add(h, h)
        } else {
            ee::from_i32(k as i32)
        }
    };
    fx.fix_residue(f, at + 0x8B0);
    let key = ee::fixunssfsi(f) as i32;
    let ot = fx.mem.read_u32(sp + 0xC8);
    fx.submit(d, ot, key);
    true
}
