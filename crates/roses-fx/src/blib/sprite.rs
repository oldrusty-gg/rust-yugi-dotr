//! Bouncing billboard sprites of the effect library.
use super::exe::{atlas_cell, camera_vp_into, countdown, frame_mask, node_source, tex_rect};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx};

/// 0.7 (0x3F333333) and 1.33 (0x3FAA3D71) as the code loads them.
const BOUNCE: u32 = 0x3F33_3333;
const ASPECT: u32 = 0x3FAA_3D71;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v) as i32;
    fx.mem.set16(a, x);
}

/// bcce571bd494 [49 B records; b098 0x1E02EF0, 668 words] [F]:
/// `bounce(g, ot, n) -> 1` steps and draws `n` sprite records (0x44
/// bytes from `g`; a 0x180 frame, the descriptor at sp + 0xF8). Per
/// record: countdown (0x101AF0); the own-slot texture +8 (0x202530(5));
/// velocity +0x2C += +0x38 (y also + gravity +0x18), position +0x20 +=
/// velocity, size +0x10 += +0x14; below the floor +0x1C the y is held
/// (and with flag 2 the y velocity reversed x 0.7); with +5 the position
/// from a node (0x101BA0 via the local sp + 0xD0); the atlas cell (flag
/// 1: the frame of animation +0xA from the elapsed count, 0x1022D0; flag
/// 4 also a random orientation into flags 0x30) of layout +0xA (0x1020B0)
/// with blend +0xB into the spec sp + 0xE0 (0x101D20); colour +0xC..+0xE,
/// alpha +0xF (faded over the first 16 counts unless flag 0x40); the
/// quad projected through the camera (0x20E2B0, 0x204D80, 0x101600), size
/// 0x1018E0(+0x10) (y x 1.33), corners by orientation flags 0x30; flag
/// 0x80 at the far Z with key 0, else at its depth.
pub fn bounce(fx: &mut Fx, g: u32, ot: u32, n: i32) -> i32 {
    bounce_a3(fx, g, ot, n).0
}

/// [`bounce`] and the a3 it leaves (helper B10, for a later boundary call
/// that reports it) [F b173 0x1E031BC / 0x1E03270; run b173]: per counting
/// record the projection's w pointer sp + 0x17C when it fails or falls
/// outside the window, else the low word of the third vertex's XYZ (sp +
/// 0x128) as it was before its new z (the previous sprite's corner).
/// `None` when no record counted (the caller's a3 is kept).
pub fn bounce_a3(fx: &mut Fx, g: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let sp = fx.enter(0x180);
    let mut a3 = None;
    let mut g = g;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x44;
        }
        one(fx, sp, g, ot, &mut a3);
        i += 1;
    }
    fx.leave(0x180);
    (1, a3)
}

fn one(fx: &mut Fx, sp: u32, g: u32, ot: u32, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    let add = |fx: &mut Fx, a: u32, b: f32| {
        let v = ee::add(fx.mem.f(a), b);
        fx.mem.setf(a, v);
    };
    let v = fx.mem.f(g + 0x38);
    add(fx, g + 0x2C, v);
    let v = ee::add(fx.mem.f(g + 0x3C), fx.mem.f(g + 0x18));
    add(fx, g + 0x30, v);
    let v = fx.mem.f(g + 0x40);
    add(fx, g + 0x34, v);
    for k in 0..3u32 {
        let v = fx.mem.f(g + 0x2C + 4 * k);
        add(fx, g + 0x20 + 4 * k, v);
    }
    let v = fx.mem.f(g + 0x14);
    add(fx, g + 0x10, v);
    let (y, floor) = (fx.mem.f(g + 0x24), fx.mem.f(g + 0x1C));
    if ee_lt(y, floor) {
        fx.mem.setf(g + 0x24, floor);
        if fx.mem.u8(g + 4) & 2 != 0 {
            let v = ee::neg(ee::mul(f32::from_bits(BOUNCE), fx.mem.f(g + 0x30)));
            fx.mem.setf(g + 0x30, v);
        }
    }
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0xD0);
        for k in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0xD0 + 4 * k);
            fx.mem.write_u32(g + 0x20 + 4 * k, v);
        }
    }
    for k in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xC0 + 4 * k, v);
    }
    camera_vp_into(fx, sp + 0x80);
    let d = sp + 0xF8;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0xEu32, 0x108u32), (0xD, 0x109), (0xC, 0x10A)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x10C, 0x3F80_0000);
    let elapsed = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let mut cell = fx.mem.u8(g + 9);
    if fx.mem.u8(g + 4) & 1 != 0 {
        let kind = fx.mem.u8(g + 0xA);
        cell = frame_mask(fx, kind, elapsed) & 0xFF;
        let f = fx.mem.u8(g + 4);
        if f & 4 != 0 {
            fx.mem.set8(g + 4, f & 0xCF);
            let r = fx.rand();
            let o = (((r >> 4) & 3) << 4) & 0xFF;
            let f = fx.mem.u8(g + 4);
            fx.mem.set8(g + 4, f | o);
        }
    }
    let b = fx.mem.read_u8(g + 0xB);
    fx.mem.write_u8(sp + 0xE5, b);
    let layout = fx.mem.u8(g + 0xA);
    atlas_cell(fx, sp + 0xE0, cell, layout);
    tex_rect(fx, d, sp + 0xE0, tex);
    let a = sp + 0x10B;
    let v = fx.mem.read_u8(g + 0xF);
    fx.mem.write_u8(a, v);
    if fx.mem.u8(g + 4) & 0x40 == 0 {
        let c = fx.mem.u8(g);
        if c < 0x10 {
            let k = fx.mem.u8(g + 0xF) >> 4;
            let v = fx.mult(k, c);
            fx.mem.set8(a, v);
        }
    }
    let m = fx.read_m4(sp + 0x80);
    *a3 = Some(sp + 0x17C);
    if !fx.project_at(sp + 0xC0, &m, sp + 0x170, sp + 0x17C) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x170)) {
        return;
    }
    let (size, w) = (fx.mem.f(g + 0x10), fx.mem.f(sp + 0x17C));
    let half = fx.view_scale(size, w);
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        let mut z = 0xFF_FFFE;
        for o in [0x148u32, 0x138, 0x128, 0x118] {
            if o == 0x128 {
                *a3 = Some(fx.mem.read_u32(sp + o));
            }
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        let mut z = fx.xyz_z(sp + 0x170);
        for o in [0x148u32, 0x138, 0x128, 0x118] {
            if o == 0x128 {
                *a3 = Some(fx.mem.read_u32(sp + o));
            }
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        fx.mem.f(sp + 0x17C)
    };
    // (x minus, x plus, y minus, y plus) corner pairs by orientation.
    let (xm, xp, ym, yp): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match fx.mem.u8(g + 4) & 0x30 {
        0x10 => ([0x128, 0x118], [0x148, 0x138], [0x14A, 0x12A], [0x13A, 0x11A]),
        0x20 => ([0x148, 0x128], [0x138, 0x118], [0x14A, 0x13A], [0x12A, 0x11A]),
        0x30 => ([0x148, 0x138], [0x128, 0x118], [0x13A, 0x11A], [0x14A, 0x12A]),
        _ => ([0x138, 0x118], [0x148, 0x128], [0x12A, 0x11A], [0x14A, 0x13A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x170));
    for o in xm {
        fix16(fx, sp + o, ee::sub(x, half));
    }
    for o in xp {
        fix16(fx, sp + o, ee::add(x, half));
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x172));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for o in ym {
        fix16(fx, sp + o, ee::sub(y, hy));
    }
    for o in yp {
        fix16(fx, sp + o, ee::add(y, hy));
    }
    let key = ee::fixunssfsi(depth) as i32;
    fx.submit(d, ot, key);
}

/// c28fb306ddd6 [11 B records; b103's 0x1E003A0, 564 words]
/// [F]: `sparks(recs, ot, n) -> 1` steps and draws `n` emitter records
/// (0x38 bytes; a 0x1D0 frame, the descriptor at sp + 0x148: RGBAQ, then
/// UV / XYZF2 x 4) and their sparks (0x28-byte records of the pool at the
/// first record's +8, indexed by the emitter's i16 list +0xC, byte +0x1D
/// long; -1 empty). Per emitter: countdown 0x101AF0; the texture (5, +0x1F)
/// with flag 4, else (4, 2); size +0x28 += +0x2C. Per spark: one under 2
/// counts leaves the list, else keeps the emitter alive (count 1);
/// countdown 0x101B50; velocity += (+0x1C, +0x20 + emitter +0x30, +0x24),
/// position += velocity, y floored at emitter +0x34; the camera
/// (0x20E2B0); colour +0x24 / +0x23 / +0x22, Q 1; atlas cell +0x20 (flag 8:
/// 2 + (elapsed >> +0x21) & 3, the elapsed under 3, 7 - (count >> +0x21)
/// in the last 7) of layout 0 with blend +0x1E (0x1020B0, 0x101D20);
/// alpha +0x25 (x count / 16 over the last 16, `mult`); projected
/// (0x204D80, 0x101600), half size 0x1018E0(+0x28) (y x 1.33), corners by
/// the spark's orientation bits 0x30 (as [`bounce`]); keyed by its depth.
pub fn sparks(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    sparks_a3(fx, recs, ot, n).0
}

/// [`sparks`] and the a3 it leaves (helper B21, for a later 0x207EF0
/// report) [F 0x1E00678 / 0x1E006B0 of card 615's]: per counting spark the
/// projection's w pointer sp + 0x1CC, then when it is on screen the low
/// word of the first vertex's XYZ (sp + 0x168, `ld a3`) before its new z
/// (the previous drawn spark's XY). `None` when no spark counted [H: what
/// 0x101AF0 / 0x202530 / 0x101B50 leave then].
pub fn sparks_a3(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let mut a3 = None;
    let sp = fx.enter(0x1D0);
    let pool = fx.mem.read_u32(recs + 8);
    fx.mem.write_u32(sp + 0xC0, pool);
    fx.mem.set32(sp + 0xD4, n);
    fx.mem.write_u32(sp + 0xD8, ot);
    fx.mem.set32(sp + 0xB0, 0);
    let mut g = recs;
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xD4) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            g += 0x38;
        }
        spark_group(fx, sp, g, &mut a3);
        let i = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, i);
    }
    fx.leave(0x1D0);
    (1, a3)
}

fn spark_group(fx: &mut Fx, sp: u32, g: u32, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    let tex = if fx.mem.u8(g + 4) & 4 != 0 {
        let id = fx.mem.u8(g + 0x1F);
        fx.texture(5, id)
    } else {
        fx.texture(4, 2)
    };
    let v = ee::add(fx.mem.f(g + 0x28), fx.mem.f(g + 0x2C));
    fx.mem.setf(g + 0x28, v);
    let mut k = 0u32;
    while (k as i32) < fx.mem.u8(g + 0x1D) {
        let idx = fx.mem.i16(g + 0xC + 2 * k);
        if idx == -1 {
            k += 1;
            continue;
        }
        let s = fx.mem.read_u32(sp + 0xC0).wrapping_add(0x28u32.wrapping_mul(idx as u32));
        if fx.mem.u8(s) < 2 {
            fx.mem.set16(g + 0xC + 2 * k, -1);
        } else {
            fx.mem.write_u8(g, 1);
        }
        if countdown(fx, s) != 0 {
            spark(fx, sp, g, s, tex, a3);
        }
        k += 1;
    }
}

fn spark(fx: &mut Fx, sp: u32, g: u32, s: u32, tex: u32, a3: &mut Option<u32>) {
    let v = ee::add(fx.mem.f(s + 0x10), fx.mem.f(s + 0x1C));
    fx.mem.setf(s + 0x10, v);
    let dv = ee::add(fx.mem.f(s + 0x20), fx.mem.f(g + 0x30));
    let v = ee::add(fx.mem.f(s + 0x14), dv);
    fx.mem.setf(s + 0x14, v);
    let v = ee::add(fx.mem.f(s + 0x18), fx.mem.f(s + 0x24));
    fx.mem.setf(s + 0x18, v);
    for c in 0..3u32 {
        let v = ee::add(fx.mem.f(s + 4 + 4 * c), fx.mem.f(s + 0x10 + 4 * c));
        fx.mem.setf(s + 4 + 4 * c, v);
    }
    let floor = fx.mem.f(g + 0x34);
    if ee_lt(fx.mem.f(s + 8), floor) {
        fx.mem.setf(s + 8, floor);
    }
    for c in 0..3u32 {
        let w = fx.mem.read_u32(s + 4 + 4 * c);
        fx.mem.write_u32(sp + 0x120 + 4 * c, w);
    }
    camera_vp_into(fx, sp + 0xE0);
    let d = sp + 0x148;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (o, src) in [(0x158u32, 0x24u32), (0x159, 0x23), (0x15A, 0x22)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x15C, 0x3F80_0000);
    let mut cell = fx.mem.u8(g + 0x20);
    let (total, count) = (fx.mem.u8(s + 2), fx.mem.u8(s));
    if fx.mem.u8(g + 4) & 8 != 0 {
        let elapsed = total - count;
        let sh = fx.mem.u8(g + 0x21);
        cell = ((elapsed >> sh) & 3) + 2;
        if elapsed < 3 {
            cell = elapsed;
        }
        if count < 7 {
            cell = 7 - (count >> sh);
        }
    }
    let b = fx.mem.read_u8(g + 0x1E);
    fx.mem.write_u8(sp + 0x135, b);
    atlas_cell(fx, sp + 0x130, cell, 0);
    tex_rect(fx, d, sp + 0x130, tex);
    let c = fx.mem.u8(s);
    if c < 0x10 {
        let a = fx.mem.u8(g + 0x25) >> 4;
        let v = fx.mult(a, c);
        fx.mem.set8(sp + 0x15B, v);
    } else {
        let v = fx.mem.read_u8(g + 0x25);
        fx.mem.write_u8(sp + 0x15B, v);
    }
    let m = fx.read_m4(sp + 0xE0);
    *a3 = Some(sp + 0x1CC);
    if !fx.project_at(sp + 0x120, &m, sp + 0x1C0, sp + 0x1CC) {
        return;
    }
    if !Fx::on_screen(fx.mem.read_u64(sp + 0x1C0)) {
        return;
    }
    let (size, w) = (fx.mem.f(g + 0x28), fx.mem.f(sp + 0x1CC));
    let half = fx.view_scale(size, w);
    *a3 = Some(fx.mem.read_u32(sp + 0x168));
    let mut z = fx.xyz_z(sp + 0x1C0);
    for o in [0x198u32, 0x188, 0x178, 0x168] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    // (x minus, x plus, y minus, y plus) corner pairs by orientation, the
    // vertices at sp + 0x168 / 0x178 / 0x188 / 0x198.
    let (xm, xp, ym, yp): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match fx.mem.u8(s + 3) & 0x30 {
        0x10 => ([0x178, 0x168], [0x198, 0x188], [0x19A, 0x17A], [0x18A, 0x16A]),
        0x20 => ([0x198, 0x178], [0x188, 0x168], [0x19A, 0x18A], [0x17A, 0x16A]),
        0x30 => ([0x198, 0x188], [0x178, 0x168], [0x18A, 0x16A], [0x19A, 0x17A]),
        _ => ([0x188, 0x168], [0x198, 0x178], [0x17A, 0x16A], [0x19A, 0x18A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x1C0));
    for o in xm {
        fix16(fx, sp + o, ee::sub(x, half));
    }
    for o in xp {
        fix16(fx, sp + o, ee::add(x, half));
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x1C2));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for o in ym {
        fix16(fx, sp + o, ee::sub(y, hy));
    }
    for o in yp {
        fix16(fx, sp + o, ee::add(y, hy));
    }
    let key = ee::fixunssfsi(fx.mem.f(sp + 0x1CC)) as i32;
    let ot = fx.mem.read_u32(sp + 0xD8);
    fx.submit(d, ot, key);
}
