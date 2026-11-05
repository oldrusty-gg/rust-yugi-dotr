//! Flickering sprites of the effect library: one
//! jittered, animated billboard per record.
use super::exe::{atlas_cell, camera_vp_into, countdown, node_source, tex_rect};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// 1.33 (the billboard's y over x).
const ASPECT: u32 = 0x3FAA_3D71;

/// b7528bb9bd64 [9 B records; b059 0x1E035E0, 696 words] [F]:
/// `flickers(at, recs, ot, n) -> 1` steps and draws `n` records (0x3C
/// bytes; a 0x190 frame, the descriptor at sp + 0x108: one colour, four
/// UV / XYZ2 pairs). Per record: countdown 0x101AF0; the own-slot texture
/// +8 (0x202530(5)); position +0x18 += velocity +0x24; every (+3 &
/// elapsed) == 0 count and on the first, the position from a node (+5,
/// 0x101BA0) and a new jitter +0x30.. = ((rand() >> 4) % +0x14 - +0x14 /
/// 2) / 100 per axis; the jittered point through the camera (0x20E2B0,
/// 0x204D80) or the record is skipped (also off screen, 0x101600);
/// colour +0xE, +0xD, +0xC, alpha +0xF (x count / 8 over the last 8 unless
/// flag 0x40); atlas cell (+3 & elapsed) & 7 (7 - count over the last 4)
/// of layout 0, blend +0xA (0x1020B0, 0x101D20); flag 1 re-rolls the
/// corner mode +4 & 0x30 on cell 0; half size 0x1018E0(+0x10, w), y x
/// 1.33, its corners in the order the mode picks; flag 0x80 at the far Z
/// (0xFFFFFE); key 0. `at` (the function's address in the running record)
/// only names the return addresses its rand / 0x1A2C70 frames leave.
pub fn flickers(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x190);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x3C;
        }
        one(fx, at, sp, g, ot, n);
        i += 1;
    }
    fx.leave(0x190);
    1
}

fn jitter(fx: &mut Fx, g: u32, n: i32, ra: u32, out: u32) {
    let range = fx.mem.i32(g + 0x14);
    let r = fx.rand_from(n as u32, ra) >> 4;
    let (_, rem) = fx.div(r, range);
    let v = ee::div(ee::from_i32(rem.wrapping_sub(range >> 1)), 100.0);
    fx.mem.setf(g + out, v);
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, n: i32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    for o in [0x18u32, 0x1C, 0x20] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0xC));
        fx.mem.setf(g + o, v);
    }
    let (total, c) = (fx.mem.u8(g + 2), fx.mem.u8(g));
    let mut cell = fx.mem.u8(g + 3) & total.wrapping_sub(c);
    if cell == 0 || total.wrapping_sub(1) == c {
        if fx.mem.u8(g + 5) != 0 {
            node_source(fx, g, sp + 0xE0);
            for o in 0..3u32 {
                let v = fx.mem.read_u32(sp + 0xE0 + 4 * o);
                fx.mem.write_u32(g + 0x18 + 4 * o, v);
            }
        }
        jitter(fx, g, n, at + 0x114, 0x30);
        jitter(fx, g, n, at + 0x160, 0x34);
        jitter(fx, g, n, at + 0x1AC, 0x38);
    }
    for o in 0..3u32 {
        let v = ee::add(fx.mem.f(g + 0x18 + 4 * o), fx.mem.f(g + 0x30 + 4 * o));
        fx.mem.setf(sp + 0xD0 + 4 * o, v);
    }
    camera_vp_into(fx, sp + 0x90);
    let d = sp + 0x108;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0xEu32, 0x118u32), (0xD, 0x119), (0xC, 0x11A)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x11C, 0x3F80_0000);
    cell &= 7;
    let c = fx.mem.u8(g);
    if c < 4 {
        cell = 7 - c;
    }
    if fx.mem.u8(g + 4) & 1 != 0 && cell == 0 {
        let r = (fx.rand_from(n as u32, at + 0x2B0) >> 4) & 3;
        let f = (fx.mem.u8(g + 4) & 0xC7) | (r << 4);
        fx.mem.set8(g + 4, f);
    }
    let b = fx.mem.read_u8(g + 0xA);
    fx.mem.write_u8(sp + 0xF5, b);
    atlas_cell(fx, sp + 0xF0, cell, 0);
    tex_rect(fx, d, sp + 0xF0, tex);
    let a = fx.mem.read_u8(g + 0xF);
    fx.mem.write_u8(sp + 0x11B, a);
    if fx.mem.u8(g + 4) & 0x40 == 0 {
        let c = fx.mem.u8(g);
        if c < 8 {
            let k = fx.mem.u8(g + 0xF) >> 3;
            let v = fx.mult(k, c);
            fx.mem.set8(sp + 0x11B, v);
        }
    }
    let m = fx.read_m4(sp + 0x90);
    if !fx.project_at(sp + 0xD0, &m, sp + 0x180, sp + 0x18C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x180)) {
        return;
    }
    let (size, w) = (fx.mem.f(g + 0x10), fx.mem.f(sp + 0x18C));
    let half = fx.view_scale(size, w);
    let mut z = if fx.mem.u8(g + 4) & 0x80 != 0 { 0xFF_FFFE } else { fx.xyz_z(sp + 0x180) };
    for o in [0x158u32, 0x148, 0x138, 0x128] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    // (x - h, x + h, y - hy, y + hy) targets and the last conversion's
    // return offset, by the corner mode.
    let (xl, xr, yl, yh, last): ([u32; 2], [u32; 2], [u32; 2], [u32; 2], u32) = match fx.mem.u8(g + 4) & 0x30 {
        0x10 => ([0x138, 0x128], [0x158, 0x148], [0x15A, 0x13A], [0x14A, 0x12A], 0x670),
        0x20 => ([0x158, 0x138], [0x148, 0x128], [0x15A, 0x14A], [0x13A, 0x12A], 0x7C4),
        0x30 => ([0x158, 0x148], [0x138, 0x128], [0x14A, 0x12A], [0x15A, 0x13A], 0x918),
        _ => ([0x148, 0x128], [0x158, 0x138], [0x13A, 0x12A], [0x15A, 0x14A], 0xA6C),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x180));
    for (o, v) in [(xl, ee::sub(x, half)), (xr, ee::add(x, half))] {
        let h = ee::fixunssfsi(v) as u16;
        for a in o {
            fx.mem.write_u16(sp + a, h);
        }
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x182));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    let top = ee::add(y, hy);
    for (o, v) in [(yl, ee::sub(y, hy)), (yh, top)] {
        let h = ee::fixunssfsi(v) as u16;
        for a in o {
            fx.mem.write_u16(sp + a, h);
        }
    }
    fx.fix_residue(top, at + last);
    fx.submit(d, ot, 0);
}
