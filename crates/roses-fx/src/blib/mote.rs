//! Motes of the effect library: a record's
//! pool of falling textured sprites (0x28-byte particles), stepped and
//! drawn as billboards.
use super::exe::{atlas_cell, camera_vp_into, countdown, tex_rect};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx};

const ASPECT: u32 = 0x3FAA_3D71;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

/// 8e418fd9932b [35 B records; b105 0x1E00410, 640 words] [F]:
/// `motes(recs, ot, n) -> 1` steps and draws `n` records (0x38 bytes; a
/// 0x1D0 frame, the descriptor at sp + 0x148: one colour, four UV / XYZ2
/// pairs). The particle pool (0x28 bytes: count, delay, length, corner
/// mode +3, position +4, velocity +0x10, acceleration +0x1C) is the first
/// record's +8. Per record: countdown 0x101AF0; texture own-slot (5,
/// +0x1F) with flag 4, else (4, 2); size +0x28 += +0x2C. Per slot (+0xC,
/// i16 x +0x1D; -1 empty): a particle in its last count frees the slot,
/// else keeps the record alive (count 1); countdown 0x101B50; velocity +=
/// acceleration (y also + gravity +0x30), position += velocity, y floored
/// at +0x34; atlas cell +0x20 (flag 8: by the length gone, shift +0x21,
/// the first 3 and last 7 counts apart) of layout 0, blend +0x1E
/// (0x101D20); colour +0x22.., alpha +0x25 (x count / 16 over the first
/// 16); projected billboard of half size 0x1018E0(+0x28, w), y x 1.33, its
/// corners in the order the mode +3 & 0x30 picks; key w.
pub fn motes(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1D0);
    fx.mem.set32(sp + 0xDC, n);
    let pool = fx.mem.read_u32(recs + 8);
    fx.mem.write_u32(sp + 0xC0, pool);
    let mut p = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xDC) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            p += 0x38;
        }
        one(fx, sp, p, pool, ot);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x1D0);
    1
}

/// 90a569cefb1c [5 B records; card 241's 0x1E00440, 688 words] (helper
/// A19, 1 Nov 2026) [F]: [motes] recompiled in a 0x190 frame: the locals
/// 0x40 lower (the same caller-relative addresses), n / the pool / the
/// counter in registers (their old slots would be this frame's saves: not
/// written), the unsigned conversions inline.
pub fn motes_c(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x190);
    let pool = fx.mem.read_u32(recs + 8);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x38;
        }
        one(fx, sp - 0x40, p, pool, ot);
        i += 1;
    }
    fx.leave(0x190);
    1
}

fn one(fx: &mut Fx, sp: u32, p: u32, pool: u32, ot: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let tex = if fx.mem.u8(p + 4) & 4 != 0 {
        let id = fx.mem.u8(p + 0x1F);
        fx.texture(5, id)
    } else {
        fx.texture(4, 2)
    };
    let v = ee::add(fx.mem.f(p + 0x28), fx.mem.f(p + 0x2C));
    fx.mem.setf(p + 0x28, v);
    let mut k = 0;
    while k < fx.mem.u8(p + 0x1D) {
        mote(fx, sp, p, pool, ot, tex, k as u32);
        k += 1;
    }
}

fn mote(fx: &mut Fx, sp: u32, p: u32, pool: u32, ot: u32, tex: u32, k: u32) {
    let slot = p + 2 * k + 0xC;
    let i = fx.mem.i16(slot);
    if i == -1 {
        return;
    }
    let s = pool.wrapping_add(i.wrapping_mul(0x28) as u32);
    if fx.mem.read_u8(s) < 2 {
        fx.mem.set16(slot, -1);
    } else {
        fx.mem.write_u8(p, 1);
    }
    if countdown(fx, s) == 0 {
        return;
    }
    let v = ee::add(fx.mem.f(s + 0x10), fx.mem.f(s + 0x1C));
    fx.mem.setf(s + 0x10, v);
    let a = ee::add(fx.mem.f(s + 0x20), fx.mem.f(p + 0x30));
    let v = ee::add(fx.mem.f(s + 0x14), a);
    fx.mem.setf(s + 0x14, v);
    let v = ee::add(fx.mem.f(s + 0x18), fx.mem.f(s + 0x24));
    fx.mem.setf(s + 0x18, v);
    for o in 0..3u32 {
        let v = ee::add(fx.mem.f(s + 4 + 4 * o), fx.mem.f(s + 0x10 + 4 * o));
        fx.mem.setf(s + 4 + 4 * o, v);
    }
    let floor = fx.mem.f(p + 0x34);
    if ee_lt(fx.mem.f(s + 8), floor) {
        fx.mem.setf(s + 8, floor);
    }
    for o in 0..3u32 {
        let v = fx.mem.read_u32(s + 4 + 4 * o);
        fx.mem.write_u32(sp + 0x120 + 4 * o, v);
    }
    camera_vp_into(fx, sp + 0xE0);
    let d = sp + 0x148;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0x24u32, 0x158u32), (0x23, 0x159), (0x22, 0x15A)] {
        let v = fx.mem.read_u8(p + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x15C, 0x3F80_0000);
    let c = fx.mem.u8(s);
    let gone = fx.mem.u8(s + 2) - c;
    let mut cell = fx.mem.u8(p + 0x20);
    if fx.mem.u8(p + 4) & 8 != 0 {
        let sh = (fx.mem.u8(p + 0x21) & 31) as u32;
        cell = ((gone >> sh) & 3) + 2;
        if gone < 3 {
            cell = gone;
        }
        if c < 7 {
            cell = 7 - (c >> sh);
        }
    }
    let b = fx.mem.read_u8(p + 0x1E);
    fx.mem.write_u8(sp + 0x135, b);
    atlas_cell(fx, sp + 0x130, cell, 0);
    tex_rect(fx, d, sp + 0x130, tex);
    let c = fx.mem.u8(s);
    if c < 16 {
        let a = fx.mem.u8(p + 0x25) >> 4;
        let v = fx.mult(a, c);
        fx.mem.set8(sp + 0x15B, v);
    } else {
        let v = fx.mem.read_u8(p + 0x25);
        fx.mem.write_u8(sp + 0x15B, v);
    }
    let m = fx.read_m4(sp + 0xE0);
    if !fx.project_at(sp + 0x120, &m, sp + 0x1C0, sp + 0x1CC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x1C0)) {
        return;
    }
    let size = fx.mem.f(p + 0x28);
    let w = fx.mem.f(sp + 0x1CC);
    let half = fx.view_scale(size, w);
    for (from, to) in [(0x1C0u32, 0x198u32), (0x198, 0x188), (0x188, 0x178), (0x178, 0x168)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    // (x - h, x + h, y - hy, y + hy) targets by the corner mode.
    let (xl, xr, yl, yh): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match fx.mem.u8(s + 3) & 0x30 {
        0x10 => ([0x178, 0x168], [0x198, 0x188], [0x19A, 0x17A], [0x18A, 0x16A]),
        0x20 => ([0x198, 0x178], [0x188, 0x168], [0x19A, 0x18A], [0x17A, 0x16A]),
        0x30 => ([0x198, 0x188], [0x178, 0x168], [0x18A, 0x16A], [0x19A, 0x17A]),
        _ => ([0x188, 0x168], [0x198, 0x178], [0x17A, 0x16A], [0x19A, 0x18A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x1C0));
    for (o, v) in [(xl, ee::sub(x, half)), (xr, ee::add(x, half))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x1C2));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for (o, v) in [(yl, ee::sub(y, hy)), (yh, ee::add(y, hy))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let key = ee::fixunssfsi(fx.mem.f(sp + 0x1CC)) as i32;
    fx.submit(d, ot, key);
}
