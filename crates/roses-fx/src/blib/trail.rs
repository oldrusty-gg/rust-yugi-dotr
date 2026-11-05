//! Spark streaks of the effect library: each particle drawn as a quad
//! stretched from its position back along its direction.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, Fx};

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v) as i32;
    fx.mem.set16(a, x);
}

/// 0x20C7E0(v): v.xyz / |v.xyz| (libm sqrtf 0x1A7520 of ((x x + y y) + z
/// z)), w kept.
pub fn normalize3(fx: &mut Fx, v: u32) {
    let (x, y, z) = (fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8));
    let acc = ee::add(ee::mul(x, x), ee::mul(y, y));
    let r = fx.sqrtf(ee::add(acc, ee::mul(z, z)));
    for k in 0..3u32 {
        let c = ee::div(fx.mem.f(v + 4 * k), r);
        fx.mem.setf(v + 4 * k, c);
    }
    let w = fx.mem.read_u32(v + 0xC);
    fx.mem.write_u32(v + 0xC, w);
}

/// The atlas UVs of the strip at sp + 0x148 (u at +0, v at +2 of each
/// 0x10-byte vertex): (x, y), (x, y + h - 1), (x + w - 1, y), (x + w - 1,
/// y + h - 1) from the group's i16 x +0x10, y +0x12, w +0x14, h +0x16;
/// flag 0x800 on the descriptor.
fn uvs(fx: &mut Fx, sp: u32, g: u32) {
    let f = fx.mem.read_u16(sp + 0x132);
    fx.mem.write_u16(sp + 0x132, f | 0x800);
    let x = |fx: &mut Fx| fx.mem.i16(g + 0x10);
    let y = |fx: &mut Fx| fx.mem.i16(g + 0x12);
    let v = x(fx);
    fx.set_uv(sp + 0x148, v);
    let v = y(fx);
    fx.set_uv(sp + 0x14A, v);
    let v = x(fx);
    fx.set_uv(sp + 0x158, v);
    let v = y(fx) + fx.mem.i16(g + 0x16) - 1;
    fx.set_uv(sp + 0x15A, v);
    let v = x(fx) + fx.mem.i16(g + 0x14) - 1;
    fx.set_uv(sp + 0x168, v);
    let v = y(fx);
    fx.set_uv(sp + 0x16A, v);
    let v = x(fx) + fx.mem.i16(g + 0x14) - 1;
    fx.set_uv(sp + 0x178, v);
    let v = y(fx) + fx.mem.i16(g + 0x16) - 1;
    fx.set_uv(sp + 0x17A, v);
}

/// e2975c00bd8b [55 B records; b036 0x1E01800, 856 words] [F]:
/// `sparks(ot, g, owner) -> 0` draws the group's g.i16(+0x2C) particles
/// (16-byte records from *(g+0xC): position +0, alpha +0xC, frame +0xD,
/// life +0xE) as textured streaks (RH2 *(g+0), colour +8..+0xA, alpha
/// min(particle, +0xB); ALPHA by flags 0xC / 0xB / 0xA of g+4: 0x44, 0x88,
/// 0x42, else 0x48, FIX 0x80; a 0xF0 frame with the descriptor at sp +
/// 0x130). UVs: flag 3 the fixed rect +0x10..+0x16, else the particle's
/// frame in the atlas (+0x18 / +0x1A origin, +0x1C / +0x1E size; flag 1
/// row-major, else column-major). The tail: flag 0x1E straight down by
/// +0x24, else back along the normalized position (0x20C7E0) by +0x24.
/// Both ends projected through owner+0x30 (0x204D80, on screen 0x101600:
/// flags 0x14 / 0x15); with both, the quad across the longer screen axis,
/// half width |d| x +0x20 / +0x24; CLAMP with flag 0, at the far Z (key
/// 0) with flag 0x13, else the mean depth. Unless flag 3 each life
/// counts up and the frame steps every +0x2F updates, wrapping at +0x2E.
pub fn sparks(fx: &mut Fx, ot: u32, g: u32, owner: u32) -> i32 {
    let sp = fx.enter(0x1B0);
    fx.mem.write_u32(sp + 0x108, ot);
    let d = sp + 0x130;
    fx.mem.write_u16(d + 2, 0x24);
    fx.mem.write_u32(sp + 0xF0, d + 2);
    fx.mem.write_u16(d, 9);
    fx.mem.write_u64(d + 8, 0x4_2424_2421);
    fx.mem.write_u32(d + 4, 0);
    fx.mem.write_u32(sp + 0x100, owner);
    let flags = |fx: &mut Fx| fx.mem.read_u32(g + 4);
    let alpha = if bit(flags(fx), 0xC) {
        0x80_0000_0044
    } else if bit(flags(fx), 0xB) {
        0x80_0000_0088
    } else if bit(flags(fx), 0xA) {
        0x80_0000_0042
    } else {
        0x80_0000_0048
    };
    fx.desc_alpha(d, alpha);
    for (src, dst) in [(0xAu32, 0x140u32), (9, 0x141), (8, 0x142), (0xB, 0x143)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x144, 0x3F80_0000);
    if bit(flags(fx), 3) {
        uvs(fx, sp, g);
    }
    for (slot, v) in [(0xB0u32, 0x14Au32), (0xC0, 0x15A), (0xD0, 0x16A), (0xE0, 0x17A)] {
        fx.mem.write_u32(sp + slot, sp + v);
    }
    let mut off = 0u32;
    let mut i = 0;
    while i < fx.mem.i16(g + 0x2C) {
        let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
        if fx.mem.i16(e + 0xE) >= 0 {
            one(fx, sp, g, e);
        }
        if !bit(flags(fx), 3) {
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            let life = fx.mem.i16(e + 0xE) + 1;
            fx.mem.set16(e + 0xE, life);
            let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
            let step = fx.mem.u8(g + 0x2F);
            let (_, r) = { let a = fx.mem.i16(e + 0xE); fx.div(a, step) };
            if r == 0 {
                let fr = fx.mem.u8(e + 0xD) + 1;
                fx.mem.set8(e + 0xD, fr);
                let e = fx.mem.read_u32(g + 0xC).wrapping_add(off);
                if fx.mem.u8(g + 0x2E) == fx.mem.u8(e + 0xD) {
                    fx.mem.write_u8(e + 0xD, 0);
                }
            }
        }
        off += 0x10;
        i += 1;
    }
    fx.leave(0x1B0);
    0
}

fn one(fx: &mut Fx, sp: u32, g: u32, e: u32) {
    let (a, b) = (fx.mem.u8(e + 0xC), fx.mem.u8(g + 0xB));
    fx.mem.set8(sp + 0x143, if a < b { a } else { b });
    let flags = |fx: &mut Fx| fx.mem.read_u32(g + 4);
    if !bit(flags(fx), 3) {
        let fr = fx.mem.u8(e + 0xD);
        if bit(flags(fx), 1) {
            let w = fx.mem.i16(g + 0x14);
            let (cols, _) = { let a = fx.mem.i16(g + 0x1C); fx.div(a, w) };
            let (_, col) = fx.div(fr, cols);
            let wc = fx.mult(w, col);
            let v = fx.mem.i16(g + 0x18).wrapping_add(wc);
            fx.mem.set16(g + 0x10, v);
            let (w, h) = (fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x16));
            let cols = div1(fx.mem.i16(g + 0x1C), w);
            let row = div1(fx.mem.u8(e + 0xD), cols);
            let v = fx.mem.i16(g + 0x1A).wrapping_add(h.wrapping_mul(row));
            fx.mem.set16(g + 0x12, v);
        } else {
            let (h, w) = (fx.mem.i16(g + 0x16), fx.mem.i16(g + 0x14));
            let (rows, _) = { let a = fx.mem.i16(g + 0x1E); fx.div(a, h) };
            let (col, _) = fx.div(fr, rows);
            let wc = fx.mult(w, col);
            let v = fx.mem.i16(g + 0x18).wrapping_add(wc);
            fx.mem.set16(g + 0x10, v);
            let h = fx.mem.i16(g + 0x16);
            let rows = div1(fx.mem.i16(g + 0x1E), h);
            let (_, row) = { let a = fx.mem.u8(e + 0xD); fx.div(a, rows) };
            let v = fx.mem.i16(g + 0x1A).wrapping_add(h.wrapping_mul(row));
            fx.mem.set16(g + 0x12, v);
        }
        uvs(fx, sp, g);
    }
    let t = sp + 0x110;
    if bit(flags(fx), 0x1E) {
        let x = fx.mem.f(e);
        fx.mem.setf(t, x);
        let y = ee::sub(fx.mem.f(e + 4), fx.mem.f(g + 0x24));
        fx.mem.setf(t + 4, y);
        let z = fx.mem.f(e + 8);
        fx.mem.setf(t + 8, z);
    } else {
        for k in 0..3u32 {
            let v = fx.mem.read_u32(e + 4 * k);
            fx.mem.write_u32(t + 4 * k, v);
        }
        normalize3(fx, t);
        for k in 0..3u32 {
            let v = ee::mul(fx.mem.f(t + 4 * k), ee::neg(fx.mem.f(g + 0x24)));
            fx.mem.setf(t + 4 * k, v);
        }
        for k in 0..3u32 {
            let v = ee::add(fx.mem.f(t + 4 * k), fx.mem.f(e + 4 * k));
            fx.mem.setf(t + 4 * k, v);
        }
    }
    let owner = fx.mem.read_u32(sp + 0x100);
    let m = fx.read_m4(owner + 0x30);
    if fx.project_at(e, &m, sp + 0x120, sp + 0x1A8) && Fx::on_screen(fx.mem.read_u64(sp + 0x120)) {
        let f = flags(fx);
        fx.mem.write_u32(g + 4, f | 0x10_0000);
    }
    let owner = fx.mem.read_u32(sp + 0x100);
    let m = fx.read_m4(owner + 0x30);
    if fx.project_at(t, &m, sp + 0x128, sp + 0x1AC) && Fx::on_screen(fx.mem.read_u64(sp + 0x128)) {
        let f = flags(fx);
        fx.mem.write_u32(g + 4, f | 0x20_0000);
    }
    if bit(flags(fx), 0x14) && bit(flags(fx), 0x15) {
        draw(fx, sp, g);
    }
    let f = flags(fx);
    fx.mem.write_u32(g + 4, f & 0xFFCF_FFFF);
}

/// div1's quotient (pipeline 1; by zero 0 here [H]).
fn div1(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        a.wrapping_div(b)
    }
}

fn draw(fx: &mut Fx, sp: u32, g: u32) {
    let (x0, x1) = (fx.mem.u16(sp + 0x120), fx.mem.u16(sp + 0x128));
    let ax = ee::abs(ee::from_i32(x0 - x1));
    let (y0, y1) = (fx.mem.u16(sp + 0x122), fx.mem.u16(sp + 0x12A));
    let ay = ee::abs(ee::from_i32(y0 - y1));
    let acc = ee::mul(ax, ax);
    let r = fx.sqrtf(ee::add(acc, ee::mul(ay, ay)));
    let narrow = ee_le(ax, ay);
    let half = ee::div(ee::mul(r, fx.mem.f(g + 0x20)), fx.mem.f(g + 0x24));
    let f = |v: i32| ee::from_i32(v);
    if !narrow {
        let x0 = fx.mem.u16(sp + 0x120);
        fx.mem.set16(sp + 0x170, x0);
        fx.mem.set16(sp + 0x150, x0);
        let x1 = fx.mem.u16(sp + 0x128);
        fx.mem.set16(sp + 0x180, x1);
        fx.mem.set16(sp + 0x160, x1);
        let y0 = fx.mem.u16(sp + 0x122);
        fix16(fx, sp + 0x152, ee::sub(f(y0), half));
        let y1 = fx.mem.u16(sp + 0x12A);
        fix16(fx, sp + 0x162, ee::sub(f(y1), half));
        fix16(fx, sp + 0x172, ee::add(f(y0), half));
        fix16(fx, sp + 0x182, ee::add(f(y1), half));
    } else {
        let x0 = fx.mem.u16(sp + 0x120);
        fix16(fx, sp + 0x150, ee::sub(f(x0), half));
        let x1 = fx.mem.u16(sp + 0x128);
        fix16(fx, sp + 0x160, ee::sub(f(x1), half));
        fix16(fx, sp + 0x170, ee::add(f(x0), half));
        fix16(fx, sp + 0x180, ee::add(f(x1), half));
        let y0 = fx.mem.u16(sp + 0x122);
        fx.mem.set16(sp + 0x172, y0);
        fx.mem.set16(sp + 0x152, y0);
        let y1 = fx.mem.u16(sp + 0x12A);
        fx.mem.set16(sp + 0x182, y1);
        fx.mem.set16(sp + 0x162, y1);
    }
    let mut z = fx.xyz_z(sp + 0x120);
    for o in [0x180u32, 0x170, 0x160, 0x150] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    let d = sp + 0x130;
    if bit(fx.mem.read_u32(g + 4), 0) {
        let (x, w, y, h) = (fx.mem.i16(g + 0x10), fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x12), fx.mem.i16(g + 0x16));
        let clamp = ((i64::from(x) << 4) as u64 | 0xA) | ((i64::from(x + w - 1) << 14) as u64) | ((i64::from(y) << 24) as u64) | ((i64::from(y + h - 1) << 34) as u64);
        fx.desc_clamp(d, clamp);
    }
    let rh2 = fx.mem.read_u32(g);
    fx.desc_texture(d, rh2, 1);
    let ot = fx.mem.read_u32(sp + 0x108);
    if bit(fx.mem.read_u32(g + 4), 0x13) {
        let mut z = 0xFF_FFFF;
        for o in [0x180u32, 0x170, 0x160, 0x150] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        fx.submit(d, ot, 0);
    } else {
        let mean = ee::div(ee::add(fx.mem.f(sp + 0x1A8), fx.mem.f(sp + 0x1AC)), 2.0);
        let key = ee::fixunssfsi(mean) as i32;
        fx.submit(d, ot, key);
    }
}
