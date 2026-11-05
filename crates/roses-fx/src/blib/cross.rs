//! Crossbars of the effect library: two
//! textured strips from a record's projected origin out at an angle and at
//! the opposite angle, each drawn twice (thin and widened).
use super::exe::{camera_vp_into, countdown, libm_cosf, mul_mem, node_source, tex_rect_strip, translation_into};
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, ee_lt, Fx};

const PI: u32 = 0x4049_0FDB;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

fn abs(v: f32) -> f32 {
    if ee_lt(v, 0.0) {
        ee::neg(v)
    } else {
        v
    }
}

/// 43970c56c681 [28 B records; b059 0x1E02BD0, 644 words] [F]:
/// `bars(recs, ot, n) -> 1` steps and draws `n` records (0x50 bytes; a
/// 0x230 frame, the strip descriptor at sp + 0x198; the sprite spec
/// (record-relative `lui` / `addiu` at +0x4C / +0x50) at sp + 0x170, its
/// texture asked once). Per record: countdown 0x101AF0; origin +0x20 +=
/// +0x30, angle +0x18 += +0x1C (degrees); the point +0x40 projected
/// through camera x translation(origin from a node, 0x101BA0); length
/// +0x10 and width +0x14 by 0x1018E0; the length faded over the last
/// (unless flag 1) and first 8 counts (unless flag 0x40); key w (flag
/// 0x80: depth 0xFFFFFE, key 0); centre colour black, the others +8.. /
/// +0xC.., alpha +0xF (x count / 8 over the first 8 unless flag 0x40).
/// Then at the angle and the angle + 180 (libm cosf / sinf): a strip from
/// the centre to the end, along the major axis, drawn at the thin and the
/// widened offsets.
pub fn bars(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x4C, 0x50);
    bars_by(fx, spec, recs, ot, n, true).0
}

/// 54471fa1c158 [10 B records; b086 0x1E01DC0, 576 words] [F]: [`bars`]
/// scheduled differently (the same 0x230 frame and stack locals), the
/// spec's `lui` / `addiu` at +0x38 / +0x40.
pub fn bars_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x38, 0x40);
    bars_by(fx, spec, recs, ot, n, true).0
}

/// [`bars_b`] and the a3 it leaves (helper B17, for a later boundary call
/// that reports it) [F 0x1E0414C]: the projection's w pointer sp + 0x22C
/// once any record counts (every counting record projects), else `None`.
pub fn bars_b_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let spec = rel(fx, at, 0x38, 0x40);
    let (r, any) = bars_by(fx, spec, recs, ot, n, true);
    (r, any.then_some(fx.sp - 0x230 + 0x22C))
}

/// 9309e4faaf74 [9 B records; b710 0x1E01B90, 692 words] [F]: [`bars`]
/// compiled without scheduling in a 0x1E0 frame whose locals sit at
/// `bars`' addresses (its sp + 0x50 = this one's sp); the count, the
/// record counter and the texture kept in registers, not on the stack;
/// the spec's `lui` / `addiu` at +0x40 / +0x44.
pub fn bars_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let spec = rel(fx, at, 0x40, 0x44);
    bars_by(fx, spec, recs, ot, n, false).0
}

/// `stack`: the count (sp + 0xDC), counter (sp + 0xC0) and texture (sp +
/// 0xD0) kept on the stack as `bars` does.
/// Returns (1, whether any record counted).
fn bars_by(fx: &mut Fx, spec: u32, recs: u32, ot: u32, n: i32, stack: bool) -> (i32, bool) {
    let sp = fx.enter(0x230);
    let mut any = false;
    for k in (0..16u32).step_by(8) {
        let v = fx.mem.read_u64((spec & !15) + k);
        fx.mem.write_u64(sp + 0x170 + k, v);
    }
    let v = fx.mem.read_u64((spec + 0x10) & !7);
    fx.mem.write_u64(sp + 0x180, v);
    if stack {
        fx.mem.set32(sp + 0xDC, n);
    }
    let (kind, id) = (fx.mem.i32(sp + 0x170), fx.mem.u8(sp + 0x174));
    let tex = fx.texture(kind, id);
    let mut p = recs;
    if stack {
        fx.mem.write_u32(sp + 0xD0, tex);
        fx.mem.set32(sp + 0xC0, 0);
        while fx.mem.i32(sp + 0xC0) < fx.mem.i32(sp + 0xDC) {
            if fx.mem.i32(sp + 0xC0) != 0 {
                p += 0x50;
            }
            let tex = fx.mem.read_u32(sp + 0xD0);
            any |= one(fx, sp, p, ot, tex);
            let v = fx.mem.i32(sp + 0xC0) + 1;
            fx.mem.set32(sp + 0xC0, v);
        }
    } else {
        for i in 0..n.max(0) {
            if i != 0 {
                p += 0x50;
            }
            any |= one(fx, sp, p, ot, tex);
        }
    }
    fx.leave(0x230);
    (1, any)
}

/// One record; whether it counted.
fn one(fx: &mut Fx, sp: u32, p: u32, ot: u32, tex: u32) -> bool {
    if countdown(fx, p) == 0 {
        return false;
    }
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x20 + 4 * k), fx.mem.f(p + 0x30 + 4 * k));
        fx.mem.setf(p + 0x20 + 4 * k, v);
    }
    let v = ee::add(fx.mem.f(p + 0x18), fx.mem.f(p + 0x1C));
    fx.mem.setf(p + 0x18, v);
    for k in 0..4u32 {
        let v = fx.mem.read_u32(p + 0x40 + 4 * k);
        fx.mem.write_u32(sp + 0x160 + 4 * k, v);
    }
    node_source(fx, p, p + 0x20);
    camera_vp_into(fx, sp + 0x120);
    translation_into(fx, sp + 0xE0, p + 0x20);
    mul_mem(fx, sp + 0x120, sp + 0xE0);
    let d = sp + 0x198;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for o in [0x1A8u32, 0x1A9, 0x1AA] {
        fx.mem.write_u8(sp + o, 0);
    }
    for (s, dsts) in [(0xAu32, &[0x1D8u32, 0x1C0][..]), (9, &[0x1D9, 0x1C1]), (8, &[0x1DA, 0x1C2]), (0xE, &[0x1F0]), (0xD, &[0x1F1]), (0xC, &[0x1F2])] {
        let v = fx.mem.read_u8(p + s);
        for &o in dsts {
            fx.mem.write_u8(sp + o, v);
        }
    }
    for o in [0x1F4u32, 0x1DC, 0x1C4, 0x1AC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    tex_rect_strip(fx, d, sp + 0x170, tex);
    let a = fx.mem.u8(p + 0xF);
    let c = fx.mem.u8(p);
    let v = if c < 8 && fx.mem.u8(p + 4) & 0x40 == 0 { fx.mult(a >> 3, c) } else { a };
    for o in [0x1F3u32, 0x1DB, 0x1C3] {
        fx.mem.set8(sp + o, v);
    }
    fx.mem.write_u8(sp + 0x1AB, 0);
    let m = fx.read_m4(sp + 0x120);
    if !fx.project_at(sp + 0x160, &m, sp + 0x190, sp + 0x22C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x190)) {
        return true;
    }
    let w = fx.mem.f(sp + 0x22C);
    let size = fx.mem.f(p + 0x10);
    let len = fx.view_scale(size, w);
    let size = fx.mem.f(p + 0x14);
    let width = fx.view_scale(size, w);
    let depth = if fx.mem.u8(p + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x190, 0xFF_FFFE);
        0.0
    } else {
        fx.mem.f(sp + 0x22C)
    };
    for (from, to) in [(0x190u32, 0x200u32), (0x200, 0x1E8), (0x1E8, 0x1D0), (0x1D0, 0x1B8)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    for o in [0x207u32, 0x1EF, 0x1D7, 0x1BF] {
        fx.mem.write_u8(sp + o, 1);
    }
    let mut r = len;
    let f = fx.mem.u8(p + 4);
    if f & 0x40 == 0 {
        if f & 1 == 0 {
            let g = (fx.mem.u8(p + 2) - fx.mem.u8(p)) & 0xFF;
            if g < 8 {
                r = ee::mul(len, ee::mul(0.125, ee::from_i32(g)));
            }
        }
        let c = fx.mem.u8(p);
        if c < 8 {
            r = ee::mul(len, ee::mul(0.125, ee::from_i32(c)));
        }
    }
    let key = ee::fixunssfsi(depth) as i32;
    let fl = |v: i32| ee::from_i32(v);
    for s2 in 0..2 {
        let half = ee::mul(180.0, fl(s2));
        let ang = ee::div(ee::mul(f32::from_bits(PI), ee::add(fx.mem.f(p + 0x18), half)), 180.0);
        let dx = ee::mul(r, libm_cosf(fx, ang));
        let sn = fx.libm_sinf(ang);
        let dy = ee::mul(r, sn);
        if !ee_le(abs(dx), abs(dy)) {
            let x = fx.mem.u16(sp + 0x190);
            let v = ee::add(fl(x), dx);
            fix16(fx, sp + 0x1E8, v);
            fix16(fx, sp + 0x1B8, v);
            fx.mem.set16(sp + 0x200, x);
            fx.mem.set16(sp + 0x1D0, x);
            let y = fx.mem.u16(sp + 0x192);
            let e = ee::add(fl(y), dy);
            fix16(fx, sp + 0x1BA, ee::sub(e, width));
            fix16(fx, sp + 0x1D2, ee::sub(fl(y), width));
            fix16(fx, sp + 0x1EA, e);
            fx.mem.set16(sp + 0x202, y);
            fx.submit(d, ot, key);
            let y = fx.mem.u16(sp + 0x192);
            fix16(fx, sp + 0x1BA, ee::add(width, ee::add(fl(y), dy)));
            fix16(fx, sp + 0x1D2, ee::add(fl(y), width));
            fx.submit(d, ot, key);
        } else {
            let y = fx.mem.u16(sp + 0x192);
            let v = ee::add(fl(y), dy);
            fix16(fx, sp + 0x1EA, v);
            fix16(fx, sp + 0x1BA, v);
            fx.mem.set16(sp + 0x202, y);
            fx.mem.set16(sp + 0x1D2, y);
            let x = fx.mem.u16(sp + 0x190);
            let e = ee::add(fl(x), dx);
            fix16(fx, sp + 0x1B8, ee::sub(e, width));
            fix16(fx, sp + 0x1D0, ee::sub(fl(x), width));
            fix16(fx, sp + 0x1E8, e);
            fx.mem.set16(sp + 0x200, x);
            fx.submit(d, ot, key);
            let x = fx.mem.u16(sp + 0x190);
            fix16(fx, sp + 0x1B8, ee::add(width, ee::add(fl(x), dx)));
            fix16(fx, sp + 0x1D0, ee::add(fl(x), width));
            fx.submit(d, ot, key);
        }
    }
    true
}
