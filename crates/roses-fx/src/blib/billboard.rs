//! The B records' copies of the second-library sprite routines (helper A2,
//! 30 Oct 2026): the record-linked functions that build their quads with
//! the executable's second effect library (0x106960..0x10A340, ported in
//! [`crate::fxlib2`]). They are c061's sprite routines (0x1E008D0 /
//! 0x1E00B90 / 0x1E01100) compiled into the BMODULE records with the data
//! area `D` as their first argument and a record-local UV setter (one of
//! four, [`Uv`]) [F word-compare of the instances].
//!
//! Arguments as the original's (a0 = `D`, a1 = the task, a2 = the mode,
//! a3 = a position or table); the UV setter the record links is `uv`
//! ([`uv_callee`] finds it from the record's `jal`). Locals stay at the
//! original's stack addresses below `fx.sp`.
use crate::blib::uv;
use crate::bus::Bus;
use crate::c061::{blend_alpha, desc_uv4, rgbaq_at, to_degrees_neg};
use crate::ee;
use crate::exe::Fx;
use crate::fxlib2 as l2;

/// The scratchpad, where the originals project.
const SPR: u32 = 0x7000_0000;

/// A record-local UV setter `(desc, rect, mode)`.
pub type Uv = fn(&mut Fx, u32, u32, i32);

/// 3a1a2828e348 [8 B records; b060 0x1E00110, 276 words, a leaf]: as
/// [`uv::uv_rect`] for modes 0..3 only (any other mode writes nothing)
/// [F: its four cases are uv_rect's first four, word for word].
pub fn uv_rect_q(fx: &mut Fx, desc: u32, rect: u32, mode: i32) {
    if (0..4).contains(&mode) {
        uv::uv_rect(fx, desc, rect, mode);
    }
}

/// 627901cbd84f [1 B record; b143 0x1E00110, 348 words, a leaf]: as
/// [`uv::uv_rect`] for modes 0, 1, 2, 3 and 5 (any other mode writes
/// nothing) [F].
pub fn uv_rect_m5(fx: &mut Fx, desc: u32, rect: u32, mode: i32) {
    if matches!(mode, 0..=3 | 5) {
        uv::uv_rect(fx, desc, rect, mode);
    }
}

/// The UV setter a record's `jal` at `call` reaches: the four leaves end
/// with `jr ra` at different word indices (550 uv_rect, 271 uv_rect4, 274
/// [`uv_rect_q`], 343 [`uv_rect_m5`]) [F the inventory's instances].
pub fn uv_callee(fx: &mut Fx, call: u32) -> Option<Uv> {
    let j = fx.mem.read_u32(call);
    if j >> 26 != 3 {
        return None;
    }
    let target = (call & 0xF000_0000) | ((j & 0x03FF_FFFF) << 2);
    let end = (0..600u32).find(|&k| fx.mem.read_u32(target + 4 * k) == 0x03E0_0008)?;
    Some(match end {
        550 => uv::uv_rect,
        271 => uv::uv_rect4,
        274 => uv_rect_q,
        343 => uv_rect_m5,
        _ => return None,
    })
}

/// Submit `d` (projected, mean w at `w`) by depth mode `m`: 0 the mean w,
/// 1 the far Z at key 0 (strip-4 XYZ values `far`), 2 key -1.
fn submit_by(fx: &mut Fx, d0: u32, d: u32, m: i32, w: u32, far: &[u32; 4]) {
    let ot = fx.mem.read_u32(d0 + 0x80);
    match m {
        0 => {
            let key = ee::fixunssfsi(fx.mem.f(w)) as i32;
            fx.submit(d, ot, key);
        }
        1 => {
            l2::far_z(fx, far);
            fx.submit(d, ot, 0);
        }
        _ => fx.submit(d, ot, -1),
    }
}

/// add5d81dd3fe [96 B records; b003 0x1E01320, 176 words]:
/// `quad(D, task, mode, pos)`: one textured quad (9 values, FST) of the
/// task's size (i16 +8 / +0xA) turned by its angles +0x20, at `pos` (scale
/// 0.1), coloured +0x60, UVs from D+0x94 in D+0xBE's orientation, texture
/// D+0x88; blend (mode >> 4) & 15, depth mode & 15 (0 the mean w, 1 the far
/// Z at key 0, else key -1); D+0x80 the ordering table (c061's 0x1E008D0
/// with D an argument) [F].
pub fn quad(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    quad_alpha(fx, uv, d0, t, mode, pos, blend_alpha((mode >> 4) & 0xF));
}

/// 72fca25a6a0f [6 B records; b016 0x1E018F0, 180 words]:
/// [`quad`] standing on its lower edge (the lower corners' y 0 before the
/// projection) [F word-compare: the two stores are the only words added].
pub fn quad_standing(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let alpha = blend_alpha((mode >> 4) & 0xF);
    quad_by(fx, uv, d0, t, mode, pos, alpha, 0x1B0, false, false, true);
}

/// 40d081dab923 [2 B records; b183 0x1E00E70, 184 words]:
/// [`quad`] with the blend (mode >> 4) & 15 picking ALPHA 0x42 (0), 0x44
/// (1) or 0x48 (else); its UV setter's `jal` at +0x11C [F word-compare:
/// only the ALPHA selection differs].
pub fn quad_b(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let alpha = match (mode >> 4) & 0xF {
        0 => crate::fxlib::ALPHA_42,
        1 => crate::fxlib::ALPHA_44,
        _ => 0x80_0000_0048,
    };
    quad_alpha(fx, uv, d0, t, mode, pos, alpha);
}

/// fc6290cb1a17 [b082 0x1E03130, 212 words]: [`quad_b`] in a
/// 0x10-larger frame (the descriptor and colour locals at the same
/// caller-relative addresses, the rest at the same sp offsets), its corners
/// turned about z by the task's i16 +0x5A degrees (0x1099E0 axis 2 with
/// D+0xD8's sines, the angle at sp + 0x118) and submitted twice (depth 0:
/// the mean w twice; 1: the far Z, then key 0 twice; else key -1 twice);
/// its UV setter's `jal` at +0x11C [F word-compare].
pub fn quad_b_turned2(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let alpha = match (mode >> 4) & 0xF {
        0 => crate::fxlib::ALPHA_42,
        1 => crate::fxlib::ALPHA_44,
        _ => 0x80_0000_0048,
    };
    quad_by(fx, uv, d0, t, mode, pos, alpha, 0x1C0, true, true, false);
}

/// 2454171f8f96 [cards 192 / 202, 0x1E051E0] and c004d75a62da [cards 84 /
/// 554, 0x1E050A0; the same 196 words]: [`quad_b_turned2`]
/// without the turn ([`quad_b`] in its 0x1B0 frame, submitted twice); its
/// UV setter's `jal` at +0x11C [F fdiff: the seven turn words are the only
/// difference].
pub fn quad_b_twice(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let alpha = match (mode >> 4) & 0xF {
        0 => crate::fxlib::ALPHA_42,
        1 => crate::fxlib::ALPHA_44,
        _ => 0x80_0000_0048,
    };
    quad_by(fx, uv, d0, t, mode, pos, alpha, 0x1B0, false, true, false);
}

/// e7aef8120921 [card 25 0x1E03090, 196 words]:
/// [`quad_b_twice`] compiled for the 0x40-byte tasks: colour +0x28, angles
/// +0x10 [F fdiff: those two offsets are the only differences].
pub fn quad_b_twice_40(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let alpha = match (mode >> 4) & 0xF {
        0 => crate::fxlib::ALPHA_42,
        1 => crate::fxlib::ALPHA_44,
        _ => 0x80_0000_0048,
    };
    quad_by_at(fx, uv, d0, t, mode, pos, alpha, 0x1B0, false, true, false, (0x28, 0x10));
}

/// 8bce3c06cdde [2 B records; b747 (card 363) 0x1E01A00, 192 words]
///: [`quad_b_turned2`] submitted once ([`quad_b`] in the
/// 0x1C0 frame with the corners turned about z by +0x5A); its UV setter's
/// `jal` at +0x11C [F word-compare: the three second submits are the only
/// words missing].
pub fn quad_b_turned(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let alpha = match (mode >> 4) & 0xF {
        0 => crate::fxlib::ALPHA_42,
        1 => crate::fxlib::ALPHA_44,
        _ => 0x80_0000_0048,
    };
    quad_by(fx, uv, d0, t, mode, pos, alpha, 0x1C0, true, false, false);
}

/// 6e60344f604f [6 B records; b491 0x1E01B50, 276 words]:
/// `arc_quads(D, task, mode, pos)`: D+0xB8 textured quads (9 values, FST;
/// one colour +0x60) round `pos` turned by the task's angles +0x20 (no
/// scale), quad k from angle +0x5A (k + 1) to that + the span +0x5C, inner
/// radius +0x48, outer +0x4C, height 0; UVs from D+0x94 in D+0xC0's
/// orientation (the setter's `jal` at +0x17C, once), texture D+0x88; blend
/// (mode >> 4) as i16, depth mode & 15 (0 the mean w, 1 the far Z at key 0,
/// 2 key -1, else nothing projected) [F].
pub fn arc_quads(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x1F0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    let d = sp + 0x158;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    rgbaq_at(fx, sp + 0x1E8, t + 0x60, d + 0x10);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xA0 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x110, 0, t + 0x20, sp + 0xF0);
    for o in [0xB4u32, 0xC4, 0xD4, 0xE4] {
        fx.mem.set32(sp + o, 0);
    }
    let order = fx.mem.i16(d0 + 0xC0);
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let span = fx.mem.i16(t + 0x5C);
    uv(fx, d, d0 + 0x94, order);
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let s = (fx.mem.i16(t + 0x5A) + a) as i16 as i32;
        let e = (a + (fx.mem.i16(t + 0x5A) + span)) as i16 as i32;
        for (deg, r, x, z) in [(s, inner, 0xB0u32, 0xB8u32), (s, outer, 0xD0, 0xD8), (e, inner, 0xC0, 0xC8), (e, outer, 0xE0, 0xE8)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if depth <= 2 && l2::project(fx, sp + 0xB0, SPR, sp + 0x110, d, sp + 0x100) {
            submit_by(fx, d0, d, depth, sp + 0x100, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
        }
        k = (k + 1) as i16 as i32;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x1F0);
}

/// 3307e6a8ec30 [23 B records; b357 0x1E00550, 224 words]:
/// `ring_t(D, task, desc, table)` for the 0x40-byte tasks: D+0xB8 (20
/// when 0) textured quads (9 values, FST, ALPHA 0x48) into the caller's
/// `desc`, of size i16 +0x38, each pushed out by the bytes `table` [k +
/// i16 +8 (+0, +0x64, +0xB4)] / 20 times i16 +0x30 + k / 2, all turned by
/// the camera's angles plus the task's i32 +0x10 / +0x14 / +0x18 (scale
/// 0.1) about the i16 point +0 / +2 / +4 plus i16 +8 / +0xA / +0xC over
/// 10; colour +0x28, UVs from D+0x94 in orientation 0 (the setter's `jal`
/// at +0xC8), texture D+0x88; each keyed by its mean w into D+0x80 [F].
pub fn ring_t(fx: &mut Fx, uv: Uv, d0: u32, t: u32, d: u32, table: u32) {
    ring_t_by(fx, uv, d0, t, d, table, false);
}

/// 8bd9f4ccd910 [24 B records; b018 0x1E00920, 204 words]:
/// [`ring_t`] compiled in a 0x10 smaller frame (the same locals 0x10
/// lower), always 20 quads, about the i16 point +0 / +2 / +4 only, each
/// quad's first two corners reset to (1, 0, 0) / (-1, 0, 0) after the
/// push; the setter's `jal` at +0xC4 [F word-compare].
pub fn ring_t20(fx: &mut Fx, uv: Uv, d0: u32, t: u32, d: u32, table: u32) {
    ring_t_by(fx, uv, d0, t, d, table, true);
}

fn ring_t_by(fx: &mut Fx, uv: Uv, d0: u32, t: u32, d: u32, table: u32, b20: bool) {
    let frame = if b20 { 0x150 } else { 0x160 };
    let sp = fx.enter(frame);
    // The locals, 0x10 lower in the 0x150 frame.
    let l = |o: u32| if b20 { sp + o - 0x10 } else { sp + o };
    desc_uv4(fx, d);
    fx.desc_alpha(d, 0x80_0000_0048);
    fx.mem.write_u32(d + 0x14, 0);
    for o in [0x27u32, 0x37, 0x47, 0x57] {
        fx.mem.write_u8(d + o, 0);
    }
    rgbaq_at(fx, l(0x158), t + 0x28, d + 0x10);
    uv(fx, d, d0 + 0x94, 0);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    let n = if b20 {
        0x14
    } else {
        match fx.mem.i16(d0 + 0xB8) {
            0 => 0x14,
            v => v,
        }
    };
    for k in 0..3 {
        fx.mem.write_u32(l(0xE0) + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, l(0x140));
    for k in 0..3 {
        let v = fx.mem.read_u32(l(0x140) + 4 * k).wrapping_add(fx.mem.read_u32(t + 0x10 + 4 * k));
        fx.mem.write_u32(l(0x140) + 4 * k, v);
    }
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let p = if b20 { p } else { ee::add(p, ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0)) };
        fx.mem.setf(l(0xD0) + 4 * k, p);
    }
    l2::xform_vp(fx, l(0x100), l(0xE0), l(0x140), l(0xD0));
    let q = l(0x90);
    let mut k = 0i32;
    while k < n {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off)));
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            l2::add_column(fx, q, axis, ee::mul(ee::div(b, 20.0), r));
        }
        if b20 {
            for (o, v) in [(4u32, 0u32), (0x14, 0), (0, 0x3F80_0000), (0x10, 0xBF80_0000), (8, 0), (0x18, 0)] {
                fx.mem.write_u32(q + o, v);
            }
        }
        if l2::project(fx, q, SPR, l(0x100), d, l(0xF0)) {
            let key = ee::fixunssfsi(fx.mem.f(l(0xF0))) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(frame);
}

/// dd90d9e095a8 [2 B records; cards 417 / 596, b500 (card 417) 0x1E04D30,
/// 244 words]: `ring_strip3(D, task, desc, table)`: a ring of
/// D+0xB8 textured gouraud strips into the caller's `desc` (a strip3, 12
/// values: colours +0x2C for corners 0 / 1, +0x28 for 2 / 3, UVs from
/// D+0x94 in order 0, texture D+0x88): each a quad of size i16 +0x38
/// pushed out by the bytes `table` [k + i16 +8 (+0, +0x64, +0xB4)] / 10
/// times i16 +0x30 + k / 2, turned about z by the task's angles +0x10
/// (0x1099E0 axis 2), its corners 2 / 3 then (i16 +0xC, 0, 0) / (-i16
/// +0xC, 0, 0); all through the camera's angles plus +0x10 / +0x14 /
/// +0x18 at the i16 point +0 / +2 / +4 (scale 0.1), each keyed by its mean
/// w into D+0x80 [F].
pub fn ring_strip3(fx: &mut Fx, d0: u32, t: u32, d: u32, table: u32) {
    let sp = fx.enter(0x170);
    l2::desc_strip3(fx, d);
    for (local, c, to) in [(0x150u32, 0x2Cu32, 0x10u32), (0x158, 0x2C, 0x28), (0x160, 0x28, 0x40), (0x168, 0x28, 0x58)] {
        rgbaq_at(fx, sp + local, t + c, d + to);
    }
    l2::uv_strip3(fx, d, d0 + 0x94, 0);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xE0 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x130);
    for k in 0..3 {
        let v = fx.mem.read_u32(sp + 0x130 + 4 * k).wrapping_add(fx.mem.read_u32(t + 0x10 + 4 * k));
        fx.mem.write_u32(sp + 0x130 + 4 * k, v);
    }
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        fx.mem.setf(sp + 0xD0 + 4 * k, p);
    }
    l2::xform_vp(fx, sp + 0xF0, sp + 0xE0, sp + 0x130, sp + 0xD0);
    let c = ee::from_i32(fx.mem.i16(t + 0xC));
    let q = sp + 0x90;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let s = ee::from_i32(fx.mem.i16(t + 0x38));
        l2::quad_xy(fx, q, s, s);
        for (axis, off) in [(0, 0u32), (1, 0x64), (2, 0xB4)] {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off)));
            let r = ee::from_i32(fx.mem.i16(t + 0x30) + (k >> 1));
            l2::add_column(fx, q, axis, ee::mul(ee::div(b, 10.0), r));
        }
        l2::rotate4(fx, q, t + 0x10, 2, d0 + 0xD8);
        fx.mem.write_u32(q + 0x24, 0);
        fx.mem.write_u32(q + 0x34, 0);
        fx.mem.setf(q + 0x20, c);
        fx.mem.setf(q + 0x30, -c);
        fx.mem.write_u32(q + 0x28, 0);
        fx.mem.write_u32(q + 0x38, 0);
        if l2::project(fx, q, SPR, sp + 0xF0, d, sp + 0x140) {
            let key = ee::fixunssfsi(fx.mem.f(sp + 0x140)) as i32;
            let ot = fx.mem.read_u32(d0 + 0x80);
            fx.submit(d, ot, key);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x170);
}

/// 92142265a824 [3 B records; b504 0x1E02F20, 124 words]:
/// `sprite_spin(D, task, blend, pos)`: one textured quad (9 values, FST)
/// of the task's i32 size +0x20 / +0x24 turned about z by +0x28 (0x1099E0
/// axis 2 with D+0xD8's sines), facing the camera (its angles, the second
/// set to 0x4000), at `pos` (scale 0.1), coloured +0x60, UVs from D+0x94
/// in D+0xBE's orientation (the setter's `jal` at +0x10C), texture
/// D+0x88; ALPHA 0x48 (`blend` 0), 0x44 (1), 0x42 (else); keyed by its
/// mean w into D+0x80 [F]. The 0x205580 frame (s0 = the descriptor, s1 =
/// the task, ra at + 0x13C) is left below it.
pub fn sprite_spin(fx: &mut Fx, at: u32, uv: Uv, d0: u32, t: u32, blend: i32, pos: u32) {
    sprite_spin_by(fx, at, uv, d0, t, blend, pos, 0x60, 0x20, false);
}

/// f7e939ff758a [1 B record; b124 (card 275) 0x1E00D90, 124 words]
///: [`sprite_spin`] for a 0x40-byte task: coloured +0x28,
/// the i32 size +0x10 / +0x14 turned about z by +0x18 [F fdiff: only
/// those three offsets differ].
pub fn sprite_spin_40(fx: &mut Fx, at: u32, uv: Uv, d0: u32, t: u32, blend: i32, pos: u32) {
    sprite_spin_by(fx, at, uv, d0, t, blend, pos, 0x28, 0x10, false);
}

/// d1a03f6453d6 [12 B records; b148 (card 312) 0x1E01350, 124 words]
///: [`sprite_spin_40`] square, of the i16 size +0x38, facing
/// the camera by its angles as they are (the second not set to 0x4000)
/// [F].
pub fn sprite_square(fx: &mut Fx, at: u32, uv: Uv, d0: u32, t: u32, blend: i32, pos: u32) {
    sprite_spin_by(fx, at, uv, d0, t, blend, pos, 0x28, 0x10, true);
}

/// 7cbed8b660f8 [4 B records: cards 216 / 230 / 256 / 415; b046 (card
/// 230) 0x1E00BB0, 136 words]: `sprite_drift(D, task,
/// blend)`: one textured square (9 values, FST) of the i16 size +0x38
/// facing the camera by its angles as they are, at the task's i16 point
/// plus its i16 drift +8 / +0xA / +0xC over 10 (scale 0.1), coloured
/// +0x28, UVs from D+0x94 in D+0xBE's orientation (the setter's `jal` at
/// +0xE8), texture D+0x88; ALPHA 0x48 (`blend` 0), 0x44 (1), 0x42 (else);
/// keyed by its mean w into D+0x80 [F]. The 0x205580 frame (s0 = the
/// descriptor, s1 = the task, ra at + 0x118) is left below it.
pub fn sprite_drift(fx: &mut Fx, at: u32, uv: Uv, d0: u32, t: u32, blend: i32) {
    let sp = fx.enter(0x1A0);
    let d = sp + 0x108;
    desc_uv4(fx, d);
    let alpha = match blend {
        0 => 0x80_0000_0048,
        1 => crate::fxlib::ALPHA_44,
        _ => crate::fxlib::ALPHA_42,
    };
    fx.desc_alpha(d, alpha);
    rgbaq_at(fx, sp + 0x198, t + 0x28, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x90 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0xF0, u64::from(d), u64::from(t), at + 0x118);
    for k in 0..3 {
        let p = ee::from_i32(fx.mem.i16(t + 2 * k));
        let o = ee::div(ee::from_i32(fx.mem.i16(t + 8 + 2 * k)), 10.0);
        fx.mem.setf(sp + 0x80 + 4 * k, ee::add(p, o));
    }
    l2::xform_vp(fx, sp + 0xB0, sp + 0x90, sp + 0xF0, sp + 0x80);
    let v = ee::from_i32(fx.mem.i16(t + 0x38));
    l2::quad_xy(fx, sp + 0x40, v, v);
    if l2::project(fx, sp + 0x40, SPR, sp + 0xB0, d, sp + 0xA0) {
        submit_by(fx, d0, d, 0, sp + 0xA0, &[0; 4]);
    }
    fx.leave(0x1A0);
}

/// [`sprite_spin`] with the colour at `t + colour` and the size / angles
/// at `t + size` (+0 / +4, the turn at +8); `square`: the i16 size +0x38
/// both ways and the camera's second angle kept.
#[allow(clippy::too_many_arguments)]
fn sprite_spin_by(fx: &mut Fx, at: u32, uv: Uv, d0: u32, t: u32, blend: i32, pos: u32, colour: u32, size: u32, square: bool) {
    let sp = fx.enter(0x1B0);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x40 + 4 * k, w);
    }
    let d = sp + 0x118;
    desc_uv4(fx, d);
    let alpha = match blend {
        0 => 0x80_0000_0048,
        1 => crate::fxlib::ALPHA_44,
        _ => crate::fxlib::ALPHA_42,
    };
    fx.desc_alpha(d, alpha);
    rgbaq_at(fx, sp + 0x1A8, t + colour, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0xA0 + 4 * k, 0x3DCC_CCCD);
    }
    crate::b494::camera_angles_saving(fx, sp + 0x100, u64::from(d), u64::from(t), at + 0x13C);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x40 + 4 * k);
        fx.mem.write_u32(sp + 0x90 + 4 * k, w);
    }
    if !square {
        fx.mem.write_u32(sp + 0x104, 0x4000);
    }
    l2::xform_vp(fx, sp + 0xC0, sp + 0xA0, sp + 0x100, sp + 0x90);
    let (w, h) = if square {
        let v = ee::from_i32(fx.mem.i16(t + 0x38));
        (v, v)
    } else {
        (ee::from_i32(fx.mem.i32(t + size)), ee::from_i32(fx.mem.i32(t + size + 4)))
    };
    l2::quad_xy(fx, sp + 0x50, w, h);
    l2::rotate4(fx, sp + 0x50, t + size, 2, d0 + 0xD8);
    if l2::project(fx, sp + 0x50, SPR, sp + 0xC0, d, sp + 0xB0) {
        submit_by(fx, d0, d, 0, sp + 0xB0, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
    }
    fx.leave(0x1B0);
}

fn quad_alpha(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32, alpha: u64) {
    quad_by(fx, uv, d0, t, mode, pos, alpha, 0x1B0, false, false, false);
}

/// [`quad_alpha`]'s body in a frame of `frame` bytes; `turned`: the
/// corners turned about z by +0x5A ([`quad_b_turned`]); `twice`: the quad
/// submitted twice ([`quad_b_turned2`]); `stand`: the lower corners' y 0
/// ([`quad_standing`]).
#[allow(clippy::too_many_arguments)]
fn quad_by(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32, alpha: u64, frame: u32, turned: bool, twice: bool, stand: bool) {
    quad_by_at(fx, uv, d0, t, mode, pos, alpha, frame, turned, twice, stand, (0x60, 0x20));
}

/// [`quad_by`] with the task's colour / angles offsets `at` (0x70-byte
/// tasks: +0x60 / +0x20).
#[allow(clippy::too_many_arguments)]
fn quad_by_at(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32, alpha: u64, frame: u32, turned: bool, twice: bool, stand: bool, at: (u32, u32)) {
    let sp = fx.enter(frame);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0x50 + 4 * k, w);
    }
    let d = sp + frame - 0x98;
    desc_uv4(fx, d);
    fx.desc_alpha(d, alpha);
    rgbaq_at(fx, sp + frame - 8, t + at.0, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0x50 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0xA0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0xD0, sp + 0xB0, t + at.1, sp + 0xA0);
    let (w, h) = (ee::from_i32(fx.mem.i16(t + 8)), ee::from_i32(fx.mem.i16(t + 0xA)));
    l2::quad_xy(fx, sp + 0x60, w, h);
    if stand {
        fx.mem.set32(sp + 0x84, 0);
        fx.mem.set32(sp + 0x94, 0);
    }
    if turned {
        let a = fx.mem.i16(t + 0x5A);
        fx.mem.set32(sp + 0x118, a);
        l2::rotate4(fx, sp + 0x60, sp + 0x110, 2, d0 + 0xD8);
    }
    if l2::project(fx, sp + 0x60, SPR, sp + 0xD0, d, sp + 0xC0) {
        let far = [d + 0x50, d + 0x40, d + 0x30, d + 0x20];
        if twice {
            let ot = fx.mem.read_u32(d0 + 0x80);
            match mode & 0xF {
                0 => {
                    for _ in 0..2 {
                        let key = ee::fixunssfsi(fx.mem.f(sp + 0xC0)) as i32;
                        fx.submit(d, ot, key);
                    }
                }
                1 => {
                    l2::far_z(fx, &far);
                    fx.submit(d, ot, 0);
                    fx.submit(d, ot, 0);
                }
                _ => {
                    fx.submit(d, ot, -1);
                    fx.submit(d, ot, -1);
                }
            }
        } else {
            submit_by(fx, d0, d, mode & 0xF, sp + 0xC0, &far);
        }
    }
    fx.leave(frame);
}

/// 7369422df7f8 [89 B records; b042 0x1E00D80, 364 words]:
/// `ring_quads(D, task, mode, table)`: D+0xB8 textured quads (9 values,
/// FST) of size +0x48, each pushed out by the bytes `table` [k + task +8
/// (+0, +100, +180)] / 20 times the radius +0x50 + k / 2. D+0xB0 picks the
/// layout: 0 billboards facing the camera (angles as negated degrees, UVs
/// per quad from D+0xC0 + 2k, the offset turned about y by +0x5A degrees),
/// 1 the task's angles +0x20 with the quad's first two rows reset to
/// (+-size, 0, 0); the transform D+0x10 (scale) / task +0x10
/// (translation). Blend mode >> 4, depth mode & 15 (0, 1, 2 as [`quad`],
/// else nothing drawn) [F].
pub fn ring_quads(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_quads_about(fx, uv, d0, t, mode, table, 0);
}

/// [`ring_quads`]' body: layout 0's offset turned by +0x5A degrees about y
/// (`axis` 0: the angle at sp + 0x164) or z (2: at sp + 0x168, 9dae9a5cb92b
/// [`crate::blib::quads4::ring_quads_z`]) [F word-compare].
pub(crate) fn ring_quads_about(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32, axis: i32) {
    ring_quads_by(fx, uv, d0, t, mode, table, axis, false, false, false);
}

/// 8ff2daa60fe6 (b377's 0x1E03880): [`ring_quads`] with layout 0's quads
/// twice as tall (y half-size 2 x +0x48; `mul.s` for `mov.s`) [F
/// word-compare: the only difference].
pub fn ring_quads_tall(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_quads_by(fx, uv, d0, t, mode, table, 0, true, false, false);
}

/// f7ea211f1ccd [5 B records; b733 0x1E009B0, 372 words]:
/// [`ring_quads`] with layout 1's offset turned by +0x5A degrees about y
/// too (the same 0x109230 call after layout 0's) [F word-compare: the only
/// difference].
pub fn ring_quads_turned(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_quads_by(fx, uv, d0, t, mode, table, 0, false, true, false);
}

/// e46fc6cbac91 (b707 / card 76's 0x1E00C70, its only user):
/// [`ring_quads`] with layout 0's transform turned by the task's angles
/// +0x20 too (0x1097D0's a2 = task + 0x20 for 0) [F word-compare: the only
/// difference].
pub fn ring_quads_angled(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_quads_by(fx, uv, d0, t, mode, table, 0, false, false, true);
}

/// d0a608dbdcb8 [2 B records; b363 0x1E01160, 480 words]:
/// `ring_points(D, task, mode, table)`: D+0xB8 points on [`ring_quads`]'
/// ring (radius +0x50 + k / 2 times the bytes `table` [k + +8 + 0 / 100 /
/// 180] / 20, turned by +0x5A degrees about y, at D+0x20 (scale 0.1)),
/// each a zero-size quad (UVs from D+0x94 in D+0xC0's orientation, colour
/// +0x60, texture D+0x88). Depth mode & 15: 0 grows it on screen by +0x48
/// / (w / (distance / 10)) (the distance 0x1FAC30, 400 when 0) and
/// submits at key 0; 1 the far Z at key 0 (not grown); 2 key -1 (not
/// grown); blend (mode >> 4) as i16 [F].
pub fn ring_points(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    rgbaq_at(fx, sp + 0x218, t + 0x60, d + 0x10);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x100 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x160);
    to_degrees_neg(fx, sp + 0x164);
    to_degrees_neg(fx, sp + 0x160);
    let a = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x174, a);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x100, 0, sp + 0xF0);
    let order = fx.mem.i16(d0 + 0xC0);
    uv(fx, d, d0 + 0x94, order);
    let mut dist = fx.camera_distance();
    if dist == 0.0 {
        dist = 400.0;
    }
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let mut size = fx.mem.f(t + 0x48);
        l2::quad_xy(fx, sp + 0xA0, 0.0, 0.0);
        for (j, off) in [0u32, 100, 180].into_iter().enumerate() {
            let i = fx.mem.i16(t + 8);
            let b = fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off));
            let r = ee::from_i32(fx.mem.i16(t + 0x50) + (k >> 1));
            let f = ee::mul(ee::div(ee::from_i32(b), 20.0), r);
            fx.mem.setf(sp + 0xE0 + 4 * j as u32, f);
        }
        l2::rotate_deg(fx, sp + 0xE0, sp + 0x170, 0, tab);
        for axis in 0..3 {
            let f = fx.mem.f(sp + 0xE0 + 4 * axis as u32);
            l2::add_column(fx, sp + 0xA0, axis, f);
        }
        let ot = fx.mem.read_u32(d0 + 0x80);
        if depth <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x120, d, sp + 0x110) {
            match depth {
                0 => {
                    let w = ee::div(fx.mem.f(sp + 0x110), ee::div(dist, 10.0));
                    size = ee::div(size, w);
                    // (XY word, grown by -size / +size)
                    for (o, neg) in [(0x20u32, true), (0x22, false), (0x30, false), (0x32, false), (0x40, true), (0x42, true), (0x50, false), (0x52, true)] {
                        let v = ee::from_i32(i32::from(fx.mem.read_u16(d + o)));
                        let x = if neg { ee::sub(v, size) } else { ee::add(v, size) };
                        fx.mem.write_u16(d + o, ee::fixunssfsi(x) as u16);
                    }
                    fx.submit(d, ot, 0);
                }
                1 => {
                    l2::far_z(fx, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
                    fx.submit(d, ot, 0);
                }
                _ => fx.submit(d, ot, -1),
            }
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 0ab74a27a97d [2 B records; b847 0x1E019B0, 368 words]:
/// [`ring_quads`] with layout 0's offset turned by +0x5A degrees about y
/// and then by +0x5C about x (sp + 0x160; a second 0x109230 call, axis 1)
/// [F word-compare: the only differences].
pub fn ring_quads_yx(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_quads_by(fx, uv, d0, t, mode, table, 3, false, false, false);
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn ring_quads_by(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32, axis: i32, tall: bool, turn1: bool, angles0: bool) {
    let sp = fx.enter(0x210);
    let d = sp + 0x178;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha(mode >> 4));
    rgbaq_at(fx, sp + 0x208, t + 0x60, d + 0x10);
    uv(fx, d, d0 + 0x94, 0);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    l2::camera_angles_to(fx, sp + 0x150);
    to_degrees_neg(fx, sp + 0x154);
    to_degrees_neg(fx, sp + 0x150);
    for k in 0..3 {
        let w = fx.mem.read_u32(t + 0x10 + 4 * k);
        fx.mem.write_u32(sp + 0xE0 + 4 * k, w);
    }
    match fx.mem.i16(d0 + 0xB0) {
        0 => l2::xform_vp(fx, sp + 0x110, d0 + 0x10, if angles0 { t + 0x20 } else { 0 }, sp + 0xE0),
        1 => l2::xform_vp(fx, sp + 0x110, d0 + 0x10, t + 0x20, sp + 0xE0),
        _ => {}
    }
    let a = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + if axis == 0 || axis == 3 { 0x164 } else { 0x168 }, a);
    if axis == 3 {
        let b = fx.mem.i16(t + 0x5C);
        fx.mem.set32(sp + 0x160, b);
    }
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let layout = fx.mem.i16(d0 + 0xB0);
        if layout == 0 {
            let order = fx.mem.i16(d0 + 0xC0 + 2 * k as u32);
            uv(fx, d, d0 + 0x94, order);
            let s = fx.mem.f(t + 0x48);
            let h = if tall { ee::mul(2.0, s) } else { s };
            l2::quad_xy(fx, sp + 0xA0, s, h);
            l2::rotate4(fx, sp + 0xA0, sp + 0x150, 1, tab);
            l2::rotate4(fx, sp + 0xA0, sp + 0x150, 0, tab);
        } else if layout == 1 {
            let s = fx.mem.f(t + 0x48);
            l2::quad_xy(fx, sp + 0xA0, s, s);
        }
        for (j, off) in [0u32, 100, 180].into_iter().enumerate() {
            let i = fx.mem.i16(t + 8);
            let b = fx.mem.i8(table.wrapping_add(k as u32).wrapping_add(i as u32).wrapping_add(off));
            let r = ee::from_i32(fx.mem.i16(t + 0x50) + (k >> 1));
            let f = ee::mul(ee::div(ee::from_i32(b), 20.0), r);
            fx.mem.setf(sp + 0xF0 + 4 * j as u32, f);
        }
        if fx.mem.i16(d0 + 0xB0) == 0 {
            if axis == 3 {
                l2::rotate_deg(fx, sp + 0xF0, sp + 0x160, 0, tab);
                l2::rotate_deg(fx, sp + 0xF0, sp + 0x160, 1, tab);
            } else {
                l2::rotate_deg(fx, sp + 0xF0, sp + 0x160, axis, tab);
            }
        }
        if turn1 && fx.mem.i16(d0 + 0xB0) == 1 {
            l2::rotate_deg(fx, sp + 0xF0, sp + 0x160, axis, tab);
        }
        for axis in 0..3 {
            let f = fx.mem.f(sp + 0xF0 + 4 * axis as u32);
            l2::add_column(fx, sp + 0xA0, axis, f);
        }
        if fx.mem.i16(d0 + 0xB0) == 1 {
            fx.mem.set32(sp + 0xA4, 0);
            fx.mem.set32(sp + 0xB4, 0);
            let s = fx.mem.f(t + 0x48);
            fx.mem.setf(sp + 0xA0, s);
            let s = fx.mem.f(t + 0x48);
            fx.mem.setf(sp + 0xB0, ee::neg(s));
            fx.mem.set32(sp + 0xA8, 0);
            fx.mem.set32(sp + 0xB8, 0);
        }
        if depth <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x110, d, sp + 0x100) {
            submit_by(fx, d0, d, depth, sp + 0x100, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x210);
}

/// e6dce326551d [77 B records; b044 0x1E00F00, 216 words]:
/// `arc_strips(D, task, mode, pos)`: D+0xB8 untextured gouraud strips (4
/// vertices) on a circle at `pos` turned by the task's angles +0x20 (scale
/// 0.1): inner radius +0x48 at height 0, outer +0x4C at height +0x34, the
/// k-th from angle a + s to a + s + span degrees (s = +0x5A, span = +0x5C,
/// a stepping by s); colours +0x60 (inner) / +0x64 (outer), the alphas
/// cleared (0x109BC0 after the colours); blend (mode >> 4) as i16, depth
/// mode & 15 (0 the mean w, 1 the far Z at key 0, else nothing drawn) [F].
pub fn arc_strips(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    arc_strips_from(fx, d0, t, mode, pos, 0x5A);
}

/// [`arc_strips`]' body with the start angle at the task's `start` (+0x5A;
/// 3f36e61a971c [`crate::blib::quads4::arc_strips_52`]: +0x52, the step
/// still +0x5A) [F word-compare].
pub(crate) fn arc_strips_from(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32, start: u32) {
    arc_strips_by(fx, d0, t, mode, pos, start, false);
}

/// 004542c6809a [26 B records; b529 0x1E00ED0, 216 words]:
/// [`arc_strips`] with the outer radius at height 0 too (+0x34 unread)
/// [F word-compare: only the two height stores differ].
pub fn arc_strips_flat(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32) {
    arc_strips_by(fx, d0, t, mode, pos, 0x5A, true);
}

fn arc_strips_by(fx: &mut Fx, d0: u32, t: u32, mode: i32, pos: u32, start: u32, flat: bool) {
    let sp = fx.enter(0x200);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, w);
    }
    let span = fx.mem.i16(t + 0x5C);
    let d = sp + 0x170;
    l2::colour_strip4(fx, d, t + 0x60, 2);
    l2::colour_strip4(fx, d, t + 0x64, 1);
    l2::desc_gouraud4(fx, d, 0);
    l2::fullscreen4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xB0 + 4 * k);
        fx.mem.write_u32(sp + 0x110 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0x100 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x110, t + 0x20, sp + 0x100);
    fx.mem.set32(sp + 0xC4, 0);
    fx.mem.set32(sp + 0xD4, 0);
    if flat {
        fx.mem.set32(sp + 0xE4, 0);
        fx.mem.set32(sp + 0xF4, 0);
    } else {
        let y = fx.mem.f(t + 0x34);
        fx.mem.setf(sp + 0xE4, y);
        let y = fx.mem.f(t + 0x34);
        fx.mem.setf(sp + 0xF4, y);
    }
    let (inner, outer) = (fx.mem.f(t + 0x48), fx.mem.f(t + 0x4C));
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut a = 0i32;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let s4 = a as i16 as i32;
        let s = (fx.mem.i16(t + start) + s4) as i16 as i32;
        let e = (s4 + (fx.mem.i16(t + start) + span)) as i16 as i32;
        for (deg, r, x, z) in [(s, inner, 0xC0u32, 0xC8u32), (s, outer, 0xE0, 0xE8), (e, inner, 0xD0, 0xD8), (e, outer, 0xF0, 0xF8)] {
            let c = l2::sin_deg(fx, tab, deg, 1);
            fx.mem.setf(sp + x, ee::mul(r, c));
            let sn = l2::sin_deg(fx, tab, deg, 0);
            fx.mem.setf(sp + z, ee::mul(r, sn));
        }
        if depth <= 1 && l2::project(fx, sp + 0xC0, SPR, sp + 0x120, d, sp + 0x160) {
            let ot = fx.mem.read_u32(d0 + 0x80);
            if depth == 0 {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x160)) as i32;
                fx.submit(d, ot, key);
            } else {
                l2::far_z_desc(fx, d, 1);
                fx.submit(d, ot, 0);
            }
        }
        k += 1;
        a = (a + fx.mem.i16(t + 0x5A)) as i16 as i32;
    }
    fx.leave(0x200);
}

/// 98b02c86a827 [19 B records; b038 0x1E00920, 348 words]:
/// `ring_facing(D, task, mode, table)`: c061's 0x1E00B90 with D an
/// argument: D+0xB8 textured quads (9 values, FST) of size +0x48 facing
/// the camera (angles as negated degrees, turned by +0x5C about z) around
/// D+0x20, each pushed out by the bytes `table` [k + task +8 (+0, +0x64,
/// +0xB4)] / 20 times the radius +0x50 + k / 2, the xy offset turned by
/// +0x5A degrees about y; UVs from D+0x94 in D+0xC0's orientation; blend
/// (mode >> 4) as i16, depth mode & 15 (0, 1, 2 as [`quad`], else nothing
/// drawn). Its 0x206E40 (the ordering table, unused) has no effect [F].
pub fn ring_facing(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32) {
    ring_facing_turned(fx, uv, d0, t, mode, table, true, 0);
}

/// [`ring_facing`]'s body; without `turn_z` the quads are not turned by
/// +0x5C about z (9c13e08cfd2f, [`crate::blib::quads4::ring_facing_flat`])
/// [F word-compare].
/// `push_axis` 0 turns the push offsets by +0x5A about y (angle at sp +
/// 0x174), 2 about z (angle at sp + 0x178; 2f5a28a746a2).
#[allow(clippy::too_many_arguments)]
pub(crate) fn ring_facing_turned(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, table: u32, turn_z: bool, push_axis: i32) {
    let sp = fx.enter(0x220);
    let d = sp + 0x188;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) as i16 as i32));
    rgbaq_at(fx, sp + 0x218, t + 0x60, d + 0x10);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        fx.mem.write_u32(sp + 0x100 + 4 * k, 0x3DCC_CCCD);
    }
    l2::camera_angles_to(fx, sp + 0x160);
    to_degrees_neg(fx, sp + 0x164);
    to_degrees_neg(fx, sp + 0x160);
    if turn_z {
        let v = fx.mem.i16(t + 0x5C);
        fx.mem.set32(sp + 0x168, v);
    }
    let v = fx.mem.i16(t + 0x5A);
    fx.mem.set32(sp + 0x174 + 2 * push_axis as u32, v);
    for k in 0..3 {
        let w = fx.mem.read_u32(d0 + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xF0 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x120, sp + 0x100, 0, sp + 0xF0);
    let order = fx.mem.i16(d0 + 0xC0);
    uv(fx, d, d0 + 0x94, order);
    let depth = mode & 0xF;
    let tab = d0 + 0xD8;
    let mut k = 0i32;
    while k < fx.mem.i16(d0 + 0xB8) {
        let size = fx.mem.f(t + 0x48);
        l2::quad_xy(fx, sp + 0xA0, size, size);
        for axis in [2, 1, 0] {
            if axis != 2 || turn_z {
                l2::rotate4(fx, sp + 0xA0, sp + 0x160, axis, tab);
            }
        }
        let s6 = table.wrapping_add(k as u32);
        let half = k >> 1;
        let push = |fx: &mut Fx, off: u32| {
            let i = fx.mem.i16(t + 8);
            let b = ee::from_i32(fx.mem.i8(s6.wrapping_add(i as u32).wrapping_add(off)));
            let r = ee::from_i32(fx.mem.i16(t + 0x50) + half);
            ee::mul(ee::div(b, 20.0), r)
        };
        let x = push(fx, 0);
        fx.mem.setf(sp + 0xE0, x);
        let y = push(fx, 0x64);
        fx.mem.setf(sp + 0xE4, y);
        fx.mem.set32(sp + 0xE8, 0);
        l2::rotate_deg(fx, sp + 0xE0, sp + 0x170, push_axis, tab);
        for (o, from) in [(0u32, 0xE0u32), (4, 0xE4)] {
            let f = fx.mem.f(sp + from);
            for c in [0xA0u32, 0xB0, 0xC0, 0xD0] {
                let v = ee::add(fx.mem.f(sp + c + o), f);
                fx.mem.setf(sp + c + o, v);
            }
        }
        let z = push(fx, 0xB4);
        fx.mem.setf(sp + 0xE8, z);
        for c in [0xA8u32, 0xB8, 0xC8, 0xD8] {
            let v = ee::add(fx.mem.f(sp + c), z);
            fx.mem.setf(sp + c, v);
        }
        if depth <= 2 && l2::project(fx, sp + 0xA0, SPR, sp + 0x120, d, sp + 0x110) {
            submit_by(fx, d0, d, depth, sp + 0x110, &[d + 0x50, d + 0x40, d + 0x30, d + 0x20]);
        }
        k = (k + 1) as i16 as i32;
    }
    fx.leave(0x220);
}

/// 086d46d618c2 [3 B records: cards 281 / 512 / 518; card 281 0x1E02B80,
/// 196 words]: `quad_cross(D, task, mode, pos)`: standing textured
/// quads (9 values, FST) of the task's size i16 +8 / +0xA, raised by i16
/// +0xC, turned about y by i16 +0x5A (+ 90 per quad) and about x by i16
/// +0x5C (+ 36 per group) with D+0xD8's sines, through TRS(scale 0.1,
/// the task's angles +0x20, `pos`) x the camera; D+0xB0 0: one group of
/// two (a cross), 1: ten groups of one (a star); coloured +0x60, UVs
/// D+0x94 in D+0xBE's orientation, texture D+0x88; ALPHA by blend mode >>
/// 4 (0 0x42, else (k << 2) | 0x40); keyed by the mean w into D+0x80 [F].
pub fn quad_cross(fx: &mut Fx, uv: Uv, d0: u32, t: u32, mode: i32, pos: u32) {
    let sp = fx.enter(0x240);
    for k in 0..4 {
        let w = fx.mem.read_u32(pos + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, w);
    }
    let d = sp + 0x1A8;
    desc_uv4(fx, d);
    fx.desc_alpha(d, blend_alpha((mode >> 4) & 0xF));
    rgbaq_at(fx, sp + 0x238, t + 0x60, d + 0x10);
    let order = fx.mem.i16(d0 + 0xBE);
    uv(fx, d, d0 + 0x94, order);
    let tex = fx.mem.read_u32(d0 + 0x88);
    fx.desc_texture(d, tex, 1);
    for k in 0..3 {
        let w = fx.mem.read_u32(sp + 0xC0 + 4 * k);
        fx.mem.write_u32(sp + 0x120 + 4 * k, 0x3DCC_CCCD);
        fx.mem.write_u32(sp + 0x110 + 4 * k, w);
    }
    l2::xform_vp(fx, sp + 0x140, sp + 0x120, t + 0x20, sp + 0x110);
    fx.mem.set32(sp + 0x180, 0);
    fx.mem.set32(sp + 0x188, 0);
    let kind = fx.mem.i16(d0 + 0xB0);
    if kind == 0 {
        fx.mem.set32(sp + 0xB0, 1);
    }
    if kind == 1 {
        fx.mem.set32(sp + 0xB0, 0xA);
    }
    // Any other kind: the group count is the stack's and the per-group
    // count the caller's s2 [F]; the records set 0 / 1 only.
    let per = match kind {
        0 => 2,
        1 => 1,
        _ => 0,
    };
    let groups = fx.mem.i32(sp + 0xB0);
    let tab = d0 + 0xD8;
    let mut g = 0;
    while g < groups {
        fx.mem.set32(sp + 0x194, 0);
        let a = fx.mem.i16(t + 0x5C) + 0x24 * g;
        fx.mem.set32(sp + 0x190, a);
        fx.mem.set32(sp + 0x198, 0);
        let lift = ee::from_i32(fx.mem.i16(t + 0xC));
        for q in 0..per {
            let (w, h) = (ee::from_i32(fx.mem.i16(t + 8)), ee::from_i32(fx.mem.i16(t + 0xA)));
            l2::quad_xy(fx, sp + 0xD0, w, h);
            fx.mem.set32(sp + 0x180, 0);
            let a = fx.mem.i16(t + 0x5A) + 0x5A * q;
            fx.mem.set32(sp + 0x184, a);
            fx.mem.set32(sp + 0x188, 0);
            fx.mem.set32(sp + 0xF4, 0);
            fx.mem.set32(sp + 0x104, 0);
            l2::rotate4(fx, sp + 0xD0, sp + 0x180, 0, tab);
            l2::add_column(fx, sp + 0xD0, 1, lift);
            l2::rotate4(fx, sp + 0xD0, sp + 0x190, 1, tab);
            if l2::project(fx, sp + 0xD0, SPR, sp + 0x140, d, sp + 0x130) {
                let key = ee::fixunssfsi(fx.mem.f(sp + 0x130)) as i32;
                let ot = fx.mem.read_u32(d0 + 0x80);
                fx.submit(d, ot, key);
            }
        }
        g += 1;
    }
    fx.leave(0x240);
}
