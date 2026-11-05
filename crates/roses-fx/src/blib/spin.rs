//! Spinning textured particles of the effect library: `motes` (b637's 0x1E01930, moved here) and `strips`.
use super::quadtrail::axis_matrix;
use super::rel;
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, Fx};
use crate::fxlib::{ALPHA_42, ALPHA_44, ALPHA_88};

/// 0ff5e4222a02 [9 B records; b637 0x1E01930, 480 words] [F, helper C8]:
/// `motes(ot, g, x, scale)`: 60 textured quads of half size +0xC (the
/// cell's aspect from the i16 +0x24 / +0x26), positions g + 0x3F0 + 0xC k
/// scaled by `scale` (and, bit 0x1E, all turned by the angles +0x10
/// first), each turned by its angles g + 0x30 + 0x10 k, through `x` +
/// 0x30; colour +8, texture +0 (no filter); UVs integer (bit 0: x +0x20,
/// y +0x22, w +0x24, h +0x26) or ST floats (u +0x20, v +0x24, du +0x28,
/// dv +0x2C); blend from bits 0xC / 0xB / 0xA; far Z (bit 0x13) or the
/// mean w as the key. b637's flags 0x40001001 run the integer-UV, turned,
/// mean-w path only [run]; the ST and far-Z branches are ported from the
/// code, unexercised.
pub fn motes(fx: &mut Fx, ot: u32, g: u32, x: u32, scale: u32) {
    let sp = fx.enter(0x1C0);
    let d = sp + 0x130;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    fx.mem.write_u32(sp + 0xBC, scale);
    fx.alpha_for(d, g + 4, &[(0xC, ALPHA_44), (0xB, ALPHA_88), (0xA, ALPHA_42)]);
    fx.colours4(d, g + 8);
    let uv = [sp + 0x148, sp + 0x160, sp + 0x178, sp + 0x190];
    if bit(fx.mem.read_u32(g + 4), 0) {
        let f = fx.mem.read_u16(d + 2) | 0x800;
        fx.mem.write_u16(d + 2, f);
        let (u, v, w, h) = (fx.mem.i16(g + 0x20), fx.mem.i16(g + 0x22), fx.mem.i16(g + 0x24), fx.mem.i16(g + 0x26));
        for (i, &a) in uv.iter().enumerate() {
            let du = if i % 2 == 1 { w - 1 } else { 0 };
            let dv = if i >= 2 { h - 1 } else { 0 };
            fx.set_uv(a, u + du);
            fx.set_uv(a + 2, v + dv);
        }
    } else {
        for (i, &a) in uv.iter().enumerate() {
            let u = if i % 2 == 1 { ee::add(fx.mem.f(g + 0x20), fx.mem.f(g + 0x28)) } else { fx.mem.f(g + 0x20) };
            fx.mem.setf(a, u);
            let v = if i >= 2 { ee::add(fx.mem.f(g + 0x24), fx.mem.f(g + 0x2C)) } else { fx.mem.f(g + 0x24) };
            fx.mem.setf(a + 4, v);
        }
    }
    if bit(fx.mem.read_u32(g + 4), 0x1E) {
        let rot = std::array::from_fn(|k| fx.mem.read_u32(g + 0x10 + 4 * k as u32) as i32);
        let m = fx.rotation_matrix(rot);
        fx.write_m4(sp + 0xD0, &m);
        for k in 0..0x3Cu32 {
            let p = g + 0x3F0 + 0xC * k;
            for c in 0..3 {
                let w = fx.mem.read_u32(p + 4 * c);
                fx.mem.write_u32(sp + 0x120 + 4 * c, w);
            }
            turn(fx, sp, sp + 0x110, sp + 0x120);
            for c in 0..3 {
                let w = fx.mem.read_u32(sp + 0x110 + 4 * c);
                fx.mem.write_u32(p + 4 * c, w);
            }
        }
    }
    let (w, h) = (fx.mem.i16(g + 0x24), fx.mem.i16(g + 0x26));
    let size = fx.mem.f(g + 0xC);
    let (f20, f21) = if w == h {
        (size, size)
    } else if h < w {
        (size, ee::mul(size, ee::div(ee::from_i32(h), ee::from_i32(w))))
    } else {
        (ee::mul(size, ee::div(ee::from_i32(w), ee::from_i32(h))), size)
    };
    let (f22, f23) = (-f20, -f21);
    let xyz = [sp + 0x150, sp + 0x168, sp + 0x180, sp + 0x198];
    let ws = [sp + 0xC0, sp + 0xC4, sp + 0xC8, sp + 0xCC];
    let m = fx.read_m4(x + 0x30);
    for k in 0..0x3Cu32 {
        let p = g + 0x3F0 + 0xC * k;
        fx.mul3(p, scale);
        let rot = std::array::from_fn(|c| fx.mem.read_u32(g + 0x30 + 0x10 * k + 4 * c as u32) as i32);
        let r = fx.rotation_matrix(rot);
        fx.write_m4(sp + 0xD0, &r);
        for (i, (cx, cy)) in [(f22, f21), (f20, f21), (f22, f23), (f20, f23)].into_iter().enumerate() {
            fx.mem.setf(sp + 0x110, cx);
            fx.mem.setf(sp + 0x114, cy);
            fx.mem.write_u32(sp + 0x118, 0);
            turn(fx, sp, sp + 0x120, sp + 0x110);
            fx.add3(sp + 0x120, p);
            if fx.project_at(sp + 0x120, &m, xyz[i], ws[i]) {
                fx.flag(g + 4, 0x14 + i as u32);
            }
        }
        let rh2 = fx.mem.read_u32(g);
        fx.desc_texture(d, rh2, 0);
        if fx.all_projected(g + 4) {
            if bit(fx.mem.read_u32(g + 4), 0x13) {
                fx.far_z(&xyz);
                fx.submit(d, ot, 0);
            } else {
                let key = fx.mean_key4(ws);
                fx.submit(d, ot, key);
            }
        }
        fx.clear_projected(g + 4);
    }
    fx.leave(0x1C0);
}

/// 0x20C540(out, sp + 0xD0, v).
fn turn(fx: &mut Fx, sp: u32, out: u32, v: u32) {
    let m = fx.read_m4(sp + 0xD0);
    let x = fx.read_v4(v);
    let r = fx.transform(Some(&m), x);
    fx.write_v4(out, r);
}

/// 266b9c69e194 [7 B records; b423 0x1E02440, 472 words] [F]:
/// `strips(ot, g, parts, x) -> 0`: spinning animated three-quad strips
/// (a 0x2A0 frame, the descriptor at sp + 0x228: RGBAQ / UV / XYZF2 x 4,
/// integer UVs). Header `g`: texture +0 (0x207E50 filter 1), colour +6 /
/// +5 / +4 with alpha capped at +7, flags +8 (bit 0x1F: the count i16
/// +0x76, else +0x78; blend from bits 0xC / 0xB / 0xA, else 0x48; bit 1:
/// frames along the rows; bit 0xF: sizes x +0x64 each update), the cell
/// w / h +0x14 / +0x16 at u / v +0x18 / +0x1A in a sheet of +0x1C / +0x1E,
/// the four strip points +0x20 (vec4s), the top lift +0x60, frames +0x7C
/// every +0x7E updates. Per 0x20-byte particle: position +0, alpha +0xC,
/// frame +0xD, age i16 +0xE (negative: waiting, counted up), size +0x10,
/// angle +0x14 (+0x400 a update) about the record's axis (rodata from its
/// `lui` / `addiu` at +0x24 / +0x2C; 0x20BEC0), the points turned by it
/// (0x20C670); three quads between points k and k + 1 (the upper pair
/// lifted by +0x60), each a third of the cell wide, projected together
/// (0x204F70 through `x` + 0x30, the mean w the key, 0x1A2C70),
/// REGION_CLAMP of its third (0x208400).
pub fn strips(fx: &mut Fx, at: u32, ot: u32, g: u32, parts: u32, x: u32) -> i32 {
    let sp = fx.enter(0x2A0);
    let axis = rel(fx, at, 0x24, 0x2C);
    let v = fx.read_v4(axis);
    fx.write_v4(sp + 0x190, v);
    fx.mem.write_u32(sp + 0x138, ot);
    fx.mem.write_u32(sp + 0x130, x);
    let flags = |fx: &mut Fx, b: u32| bit(fx.mem.read_u32(g + 8), b);
    let count = if flags(fx, 0x1F) { fx.mem.i16(g + 0x76) } else { fx.mem.i16(g + 0x78) };
    fx.mem.set32(sp + 0xB0, count);
    let d = sp + 0x228;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    let f = fx.mem.read_u16(d + 2) | 0x800;
    fx.mem.write_u16(d + 2, f);
    let rh2 = fx.mem.read_u32(g);
    fx.desc_texture(d, rh2, 1);
    let alpha = if flags(fx, 0xC) {
        ALPHA_44
    } else if flags(fx, 0xB) {
        ALPHA_88
    } else if flags(fx, 0xA) {
        ALPHA_42
    } else {
        crate::fxlib::ALPHA_ADD
    };
    fx.desc_alpha(d, alpha);
    for (src, o) in [(6u32, 0x238u32), (5, 0x239), (4, 0x23A)] {
        let b = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + o, b);
    }
    fx.mem.write_u32(sp + 0x23C, 0x3F80_0000);
    fx.mem.write_u32(sp + 0xA0, 0);
    let (cw, ch) = (fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x16));
    let sw = fx.mem.i16(g + 0x1C);
    let cols = fx.div(sw, cw).0 as i16 as i32;
    let sh = fx.mem.i16(g + 0x1E);
    let rows = fx.div(sh, ch).0 as i16 as i32;
    fx.mult(0x5555_5556, cw);
    let third = (fx.hilo().1 + ((cw as u32) >> 31) as i32) as i16 as i32;
    let mut p = parts;
    let mut i = 0;
    while i < count {
        one(fx, at, sp, g, p, ot, x, cols, rows, third);
        i += 1;
        p += 0x20;
    }
    fx.leave(0x2A0);
    0
}

#[allow(clippy::too_many_arguments)]
fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, p: u32, ot: u32, x: u32, cols: i32, rows: i32, third: i32) {
    let age = fx.mem.i16(p + 0xE);
    if age < 0 {
        fx.mem.set16(p + 0xE, age + 1);
        return;
    }
    let frame = fx.mem.u8(p + 0xD);
    let (cw, ch) = (fx.mem.i16(g + 0x14), fx.mem.i16(g + 0x16));
    let (u0, v0) = (fx.mem.i16(g + 0x18), fx.mem.i16(g + 0x1A));
    let (u, v) = if bit(fx.mem.read_u32(g + 8), 1) {
        let (q, r) = fx.div(frame, cols);
        ((u0 + cw * r) as i16 as i32, (v0 + ch * q) as i16 as i32)
    } else {
        let (q, r) = fx.div(frame, rows);
        ((u0 + cw * q) as i16 as i32, (v0 + ch * r) as i16 as i32)
    };
    let period = fx.mem.i16(g + 0x7E);
    let age = fx.mem.i16(p + 0xE);
    if fx.div(age, period).1 == 0 {
        let f = fx.mem.read_u8(p + 0xD).wrapping_add(1);
        fx.mem.write_u8(p + 0xD, f);
        if fx.mem.i16(g + 0x7C) - 1 < f as i32 {
            fx.mem.write_u8(p + 0xD, 0);
        }
    }
    if bit(fx.mem.read_u32(g + 8), 0xF) {
        let s = ee::mul(fx.mem.f(p + 0x10), fx.mem.f(g + 0x64));
        fx.mem.setf(p + 0x10, s);
    }
    let a = fx.mem.i16(p + 0xE);
    fx.mem.set16(p + 0xE, a + 1);
    let angle = fx.mem.i32(p + 0x14);
    axis_matrix(fx, sp + 0x140, sp + 0x190, angle);
    let m = fx.read_m4(sp + 0x140);
    for k in 0..4u32 {
        let v = fx.read_v4(g + 0x20 + 0x10 * k);
        let r = fx.transform(Some(&m), v);
        fx.write_v4(sp + 0x1E0 + 0x10 * k, r);
        if k == 3 {
            // 0x20C670 keeps each result in its 0x10 frame first.
            fx.write_v4(sp - 0x10, r);
        }
    }
    fx.mem.set32(p + 0x14, angle.wrapping_add(0x400));
    let c = fx.mem.read_u8(p + 0xC);
    let cap = fx.mem.read_u8(g + 7);
    fx.mem.write_u8(sp + 0x23B, if c < cap { c } else { cap });
    let d = sp + 0x228;
    for k in 0..3i32 {
        let s1 = sp + 0x1E0 + 0x10 * k as u32;
        let size = |fx: &mut Fx| fx.mem.f(p + 0x10);
        for (out, row, lift) in [(0x1A0u32, 0u32, true), (0x1B0, 0x10, true), (0x1C0, 0, false), (0x1D0, 0x10, false)] {
            let s = size(fx);
            let vx = ee::add(fx.mem.f(p), ee::mul(fx.mem.f(s1 + row), s));
            fx.mem.setf(sp + out, vx);
            let s = size(fx);
            let vy = ee::add(fx.mem.f(p + 4), ee::mul(fx.mem.f(s1 + row + 4), s));
            let vy = if lift { ee::add(fx.mem.f(g + 0x60), vy) } else { vy };
            fx.mem.setf(sp + out + 4, vy);
            let s = size(fx);
            let vz = ee::add(fx.mem.f(p + 8), ee::mul(fx.mem.f(s1 + row + 8), s));
            fx.mem.setf(sp + out + 8, vz);
        }
        if !crate::fxlib2::project(fx, sp + 0x1A0, 0x7000_0000, x + 0x30, d, sp + 0x180) {
            continue;
        }
        let ua = u + k * third;
        let ub = u + third * (k + 1);
        let h = fx.mem.i16(g + 0x16);
        fx.set_uv(sp + 0x240, ua);
        fx.set_uv(sp + 0x242, v);
        fx.set_uv(sp + 0x250, ub);
        fx.set_uv(sp + 0x252, v);
        fx.set_uv(sp + 0x260, ua);
        fx.set_uv(sp + 0x262, v + h);
        fx.set_uv(sp + 0x270, ub);
        fx.set_uv(sp + 0x272, v + h);
        let clamp = (((ua as i64) << 4) | 0xA | (((ub - 1) as i64) << 14) | ((v as i64) << 24) | (((v + h - 1) as i64) << 34)) as u64;
        fx.desc_clamp(d, clamp);
        let w = fx.mem.f(sp + 0x180);
        fx.fix_residue(w, at + 0x6D4);
        let key = ee::fixunssfsi(w) as i32;
        fx.submit(d, ot, key);
    }
}
