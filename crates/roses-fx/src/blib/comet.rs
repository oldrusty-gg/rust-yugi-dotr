//! Comets of the effect library: a billboard
//! head with a screen-space tail along its point history.
use super::arc::resample;
use super::exe::{atlas_cell, camera_vp_into, countdown, first_free, node_source, set_counter, tex_rect, tex_rect_strip};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

const ASPECT: u32 = 0x3FAA_3D71;

/// The function's address and its compilation: [`comets`]' return
/// offsets name the 17 0x1A2C70 calls, mapped for [`comets_b`].
#[derive(Clone, Copy)]
struct At {
    at: u32,
    b: bool,
    /// [`comets_c`]'s compilation (its own return offsets).
    c: bool,
}

/// [`comets`]' 0x1A2C70 return offsets and [`comets_b`]'s, in call order.
const RAS: [(u32, u32); 17] = [
    (0x780, 0x694), (0x7D0, 0x6D8), (0x834, 0x738), (0x898, 0x774), (0x8AC, 0x784),
    (0xB8C, 0xA2C), (0xBB4, 0xA50), (0xE58, 0xC88), (0xEA4, 0xCD0), (0xEF0, 0xD0C),
    (0xF3C, 0xD48), (0xF64, 0xD7C), (0xFC0, 0xDC8), (0x100C, 0xE0C), (0x1058, 0xE44),
    (0x10A4, 0xE80), (0x10CC, 0xEC0),
];

/// [`comets`]' 0x1A2C70 return offsets and [`comets_c`]'s, in call order
///.
const RAS_C: [(u32, u32); 17] = [
    (0x780, 0x6A0), (0x7D0, 0x6EC), (0x834, 0x750), (0x898, 0x79C), (0x8AC, 0x7B0),
    (0xB8C, 0xA84), (0xBB4, 0xAB0), (0xE58, 0xCC4), (0xEA4, 0xD18), (0xEF0, 0xD64),
    (0xF3C, 0xDB0), (0xF64, 0xDEC), (0xFC0, 0xE48), (0x100C, 0xE94), (0x1058, 0xEE0),
    (0x10A4, 0xF2C), (0x10CC, 0xF70),
];

impl At {
    fn ra(self, off: u32) -> u32 {
        if self.c {
            self.at + RAS_C.iter().find(|r| r.0 == off).map_or(off, |r| r.1)
        } else if self.b {
            self.at + RAS.iter().find(|r| r.0 == off).map_or(off, |r| r.1)
        } else {
            self.at + off
        }
    }
}

/// 0x1A2C70 of `v` from the `jal` at `at + off` (its frame left), as u16.
fn fix(fx: &mut Fx, at: At, off: u32, v: f32) -> u32 {
    fx.fix_residue(v, at.ra(off));
    ee::fixunssfsi(v)
}

fn fix16(fx: &mut Fx, at: At, off: u32, a: u32, v: f32) {
    let x = fix(fx, at, off, v);
    fx.mem.write_u16(a, x as u16);
}

/// z (bits 32..55) of the XYZ2 at `from` into the one at `to`.
fn z_to(fx: &mut Fx, to: u32, from: u32) {
    let z = fx.xyz_z(from);
    fx.set_xyz_z(to, z);
}

/// 29cd5954f1de [2 B records; card 319's 0x1E01EC0, card 602's
/// 0x1E04380, 1112 words] [F]: `comets(recs, ot, n) -> 1`
/// steps and draws `n` records (0x44 bytes; a 0x480 frame) sharing the
/// point pool *(recs + 8) (read once, 0x10-byte points: count byte +0, xyz
/// +4). Per record: countdown 0x101AF0; texture (5, +0x1F); the camera
/// (0x20E2B0 into sp + 0xC0); a unit +5 puts the head +0x38 on its node
/// (0x101BA0). At count 0 every history slot (i16 +0xC.., +0x1E of them)
/// clears its point's count and is set to -1, and the record ends. Else
/// each slot's point takes the record's count and, without flag 8, the
/// history shifts one along (the last slot's point cleared), a new point
/// (first free of +0x28, 0x101C30; counted by 0x101B40) taking the head.
/// The points: +0x24 & 4 their xyz at sp + 0x164 + 0x10 k (slot k), else
/// gathered into a path at sp + 0x100 and resampled (0x102550; 4 (n - 1)
/// samples, none: the record ends). The head (sample 0) projected (sp +
/// 0x348 / w + 0x47C) and on screen without flag 1: a billboard (half size
/// 0x1018E0(+0x2C, w), y x 1.33) of colour +0x34.., cell +0x22 (flag 2:
/// the length gone & +3, & 7; 7 - count on the last 7) of layout +0x20,
/// blend +0x21, alpha +0x37 (x count / 4 on the last 4 unless flag 0x40),
/// depth from the projection (flag 0x80: the far Z at key 0). The tail: a
/// strip3 (sp + 0x370) of the same colour, cell +0x23 (flag 4: narrowed to
/// its centre column +-1), from sample 1 to the last but one each segment
/// between the previous and this sample (both projected and on screen,
/// else the record ends) twice: across y, then across x, by the half
/// widths 0x1018E0(+0x30, w) of each end; alphas from +0x37 (x count / 4
/// on the last 4 unless flag 0x40) / (samples) x the samples left (and one
/// fewer at the far end; the first segment's near end 0 with +0x24 & 2);
/// keyed by the nearer w (flag 0x80: the far Z, key 0). `at`: the
/// function's address (the 17 0x1A2C70 calls' returns).
pub fn comets(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x480);
    fx.mem.set32(sp + 0xBC, n);
    let pool = fx.mem.read_u32(recs + 8);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x44;
        }
        one(fx, At { at, b: false, c: false }, sp, g, ot, pool);
        i += 1;
    }
    fx.leave(0x480);
    1
}

/// 88e02ae308e2 [4 B records; card 615's 0x1E05C30, 980 words] (helper
/// B20) [F]: [`comets`] compiled in a 0x4A0 frame (every local 0x20
/// higher; n / ot / the record counter / the point pointer kept at sp +
/// 0xD4 / 0xD8 / 0xB0 / 0xC0), the same operations in the same order; only
/// the 17 conversions' return addresses differ.
pub fn comets_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x4A0);
    fx.mem.set32(sp + 0xD4, n);
    fx.mem.write_u32(sp + 0xD8, ot);
    let pool = fx.mem.read_u32(recs + 8);
    let mut g = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xD4) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            g += 0x44;
        }
        one(fx, At { at, b: true, c: false }, sp + 0x20, g, ot, pool);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x4A0);
    1
}

/// 0eb07a85d3d1 [3 B records; card 487's 0x1E00410 (also 523 / 673),
/// 1024 words] [F]: [`comets`] compiled in a 0x490 frame
/// (every local 0x10 higher; the record counter / n / ot kept at sp +
/// 0xB0 / 0xC4 / 0xC8), the same operations in the same order (the
/// billboard's 1.33 x half computed once); only the 17 conversions' return
/// addresses differ.
pub fn comets_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x490);
    fx.mem.write_u32(sp + 0xC8, ot);
    fx.mem.set32(sp + 0xC4, n);
    let pool = fx.mem.read_u32(recs + 8);
    let mut g = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xC4) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            g += 0x44;
        }
        one(fx, At { at, b: false, c: true }, sp + 0x10, g, ot, pool);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x490);
    1
}

fn slot(fx: &mut Fx, g: u32, k: i32) -> i32 {
    fx.mem.i16((g as i32 + 0xC + 2 * k) as u32)
}

fn one(fx: &mut Fx, at: At, sp: u32, g: u32, ot: u32, pool: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 0x1F);
    let tex = fx.texture(5, id);
    camera_vp_into(fx, sp + 0xC0);
    if fx.mem.read_u8(g + 5) != 0 {
        node_source(fx, g, sp + 0x330);
        for k in 0..3u32 {
            let w = fx.mem.read_u32(sp + 0x330 + 4 * k);
            fx.mem.write_u32(g + 0x38 + 4 * k, w);
        }
    }
    let gone = (fx.mem.u8(g + 2) - fx.mem.u8(g)) & fx.mem.u8(g + 3);
    let slots = fx.mem.u8(g + 0x1E);
    if fx.mem.u8(g) == 0 {
        for k in 0..slots {
            let h = slot(fx, g, k);
            let p = pool.wrapping_add((h << 4) as u32);
            fx.mem.write_u8(p, 0);
            fx.mem.set16((g as i32 + 0xC + 2 * k) as u32, -1);
        }
        return;
    }
    let mut k = slots;
    while k > 0 {
        let h = slot(fx, g, k - 1);
        if h >= 0 {
            let p = pool.wrapping_add((h << 4) as u32);
            let c = fx.mem.read_u8(g);
            fx.mem.write_u8(p, c);
        }
        if fx.mem.u8(g + 4) & 8 == 0 {
            let h = slot(fx, g, k - 1);
            fx.mem.set16((g as i32 + 0xC + 2 * k) as u32, h);
        }
        k -= 1;
    }
    if fx.mem.u8(g + 4) & 8 == 0 {
        let last = fx.mem.u8(g + 0x1E);
        let h = slot(fx, g, last);
        if h >= 0 {
            let p = pool.wrapping_add((h << 4) as u32);
            fx.mem.write_u8(p, 0);
            fx.mem.set16((g as i32 + 0xC + 2 * last) as u32, -1);
        }
        let max = fx.mem.i32(g + 0x28);
        let f = first_free(fx, pool, 0x10, max);
        fx.mem.set16(g + 0xC, f);
        if f >= 0 {
            let p = pool.wrapping_add((fx.mem.i16(g + 0xC) << 4) as u32);
            let c = fx.mem.u8(g);
            set_counter(fx, p, c, 0);
            for k in 0..3u32 {
                let w = fx.mem.read_u32(g + 0x38 + 4 * k);
                fx.mem.write_u32(p + 4 + 4 * k, w);
            }
        } else {
            fx.mem.set16(g + 0xC, -1);
        }
    }
    let s3;
    if fx.mem.u8(g + 0x24) & 4 != 0 {
        let mut c = 0;
        for k in 0..fx.mem.u8(g + 0x1E) {
            let h = slot(fx, g, k);
            if h < 0 {
                continue;
            }
            let p = pool.wrapping_add((h << 4) as u32);
            for j in 0..3u32 {
                let w = fx.mem.read_u32(p + 4 + 4 * j);
                fx.mem.write_u32(sp + 0x164 + 16 * k as u32 + 4 * j, w);
            }
            c += 1;
        }
        s3 = c;
    } else {
        fx.mem.set32(sp + 0x100, 0);
        for k in 0..fx.mem.u8(g + 0x1E) {
            let h = slot(fx, g, k);
            if h < 0 {
                continue;
            }
            let p = pool.wrapping_add((h << 4) as u32);
            for (j, o) in [0x104u32, 0x124, 0x144].into_iter().enumerate() {
                let w = fx.mem.read_u32(p + 4 + 4 * j as u32);
                let c = fx.mem.read_u32(sp + 0x100);
                fx.mem.write_u32(sp + o + 4 * c, w);
            }
            let c = fx.mem.read_u32(sp + 0x100);
            fx.mem.write_u32(sp + 0x100, c + 1);
        }
        if resample(fx, sp + 0x100) == 0 {
            return;
        }
        s3 = (fx.mem.i32(sp + 0x100) - 1) << 2;
    }
    let m = fx.read_m4(sp + 0xC0);
    let mut ok = fx.project_at(sp + 0x164, &m, sp + 0x348, sp + 0x47C);
    let size = fx.mem.f(g + 0x2C);
    let w = fx.mem.f(sp + 0x47C);
    let half = fx.view_scale(size, w);
    if ok {
        ok = Fx::on_screen(fx.mem.read_u64(sp + 0x348));
    }
    if ok && fx.mem.u8(g + 4) & 1 == 0 {
        head(fx, at, sp, g, ot, tex, gone, half);
    }
    tail(fx, at, sp, g, ot, tex, s3, ok);
}

#[allow(clippy::too_many_arguments)]
fn head(fx: &mut Fx, at: At, sp: u32, g: u32, ot: u32, tex: u32, gone: i32, half: f32) {
    let d = sp + 0x400;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0x36u32, 0x410u32), (0x35, 0x411), (0x34, 0x412)] {
        let v = fx.mem.read_u8(g + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x414, 0x3F80_0000);
    let cell = if fx.mem.u8(g + 4) & 2 != 0 {
        let c = fx.mem.u8(g);
        if c < 7 {
            7 - c
        } else {
            gone & 7
        }
    } else {
        fx.mem.u8(g + 0x22)
    };
    let b = fx.mem.read_u8(g + 0x21);
    fx.mem.write_u8(sp + 0x355, b);
    let layout = fx.mem.u8(g + 0x20);
    atlas_cell(fx, sp + 0x350, cell, layout);
    tex_rect(fx, d, sp + 0x350, tex);
    let a = fx.mem.read_u8(g + 0x37);
    fx.mem.write_u8(sp + 0x413, a);
    let c = fx.mem.u8(g);
    if fx.mem.u8(g + 4) & 0x40 == 0 && c < 4 {
        let a = fx.mem.u8(g + 0x37) >> 2;
        let v = fx.mult(a, c);
        fx.mem.set8(sp + 0x413, v);
    }
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x450, 0xFF_FFFE);
        0.0
    } else {
        z_to(fx, sp + 0x450, sp + 0x348);
        fx.mem.f(sp + 0x47C)
    };
    for (to, from) in [(0x440u32, 0x450u32), (0x430, 0x440), (0x420, 0x430)] {
        z_to(fx, sp + to, sp + from);
    }
    let x = ee::from_i32(fx.mem.u16(sp + 0x348));
    let v = ee::sub(x, half);
    fix16(fx, at, 0x780, sp + 0x440, v);
    let h = fx.mem.read_u16(sp + 0x440);
    fx.mem.write_u16(sp + 0x420, h);
    let v = ee::add(x, half);
    fix16(fx, at, 0x7D0, sp + 0x450, v);
    let h = fx.mem.read_u16(sp + 0x450);
    fx.mem.write_u16(sp + 0x430, h);
    let y = ee::from_i32(fx.mem.u16(sp + 0x34A));
    let v = ee::sub(y, ee::mul(f32::from_bits(ASPECT), half));
    fix16(fx, at, 0x834, sp + 0x432, v);
    let h = fx.mem.read_u16(sp + 0x432);
    fx.mem.write_u16(sp + 0x422, h);
    let v = ee::add(y, ee::mul(f32::from_bits(ASPECT), half));
    fix16(fx, at, 0x898, sp + 0x452, v);
    let h = fx.mem.read_u16(sp + 0x452);
    fx.mem.write_u16(sp + 0x442, h);
    let key = fix(fx, at, 0x8AC, depth) as i32;
    fx.submit(d, ot, key);
}

#[allow(clippy::too_many_arguments)]
fn tail(fx: &mut Fx, at: At, sp: u32, g: u32, ot: u32, tex: u32, s3: i32, head_ok: bool) {
    let d = sp + 0x370;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    for (s, k) in [(0x36u32, 0u32), (0x35, 1), (0x34, 2)] {
        let v = fx.mem.read_u8(g + s);
        for o in [0x3C8u32, 0x3B0, 0x398, 0x380] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x3CCu32, 0x3B4, 0x39C, 0x384] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    let b = fx.mem.read_u8(g + 0x21);
    fx.mem.write_u8(sp + 0x355, b);
    let (cell, layout) = (fx.mem.u8(g + 0x23), fx.mem.u8(g + 0x20));
    atlas_cell(fx, sp + 0x350, cell, layout);
    if fx.mem.u8(g + 4) & 4 != 0 {
        let (u0, u1) = (fx.mem.i32(sp + 0x358), fx.mem.i32(sp + 0x35C));
        let a = u1.wrapping_sub(u0);
        fx.mem.set32(sp + 0x358, u0.wrapping_add(a >> 1).wrapping_sub(1));
        let u1 = fx.mem.i32(sp + 0x35C);
        fx.mem.set32(sp + 0x35C, u1.wrapping_sub(a >> 1).wrapping_add(1));
    }
    tex_rect_strip(fx, d, sp + 0x350, tex);
    let size = fx.mem.f(g + 0x30);
    let w = fx.mem.f(sp + 0x47C);
    let mut f21 = fx.view_scale(size, w);
    let mut ok = head_ok;
    let mut s2 = 1;
    while s2 < s3 - 1 {
        let prev = fx.mem.read_u64(sp + 0x348);
        fx.mem.write_u64(sp + 0x340, prev);
        let pw = fx.mem.read_u32(sp + 0x47C);
        fx.mem.write_u32(sp + 0x478, pw);
        let f22 = f21;
        let prev_ok = ok;
        let m = fx.read_m4(sp + 0xC0);
        ok = fx.project_at(sp + 0x164 + 16 * s2 as u32, &m, sp + 0x348, sp + 0x47C);
        if ok {
            ok = Fx::on_screen(fx.mem.read_u64(sp + 0x348));
        }
        if !prev_ok || !ok {
            return;
        }
        let size = fx.mem.f(g + 0x30);
        let w = fx.mem.f(sp + 0x47C);
        f21 = fx.view_scale(size, w);
        let c = fx.mem.u8(g);
        let base = if c < 4 && fx.mem.u8(g + 4) & 0x40 == 0 {
            ee::div(ee::mul(ee::from_i32(c), ee::from_i32(fx.mem.u8(g + 0x37))), 4.0)
        } else {
            ee::from_i32(fx.mem.u8(g + 0x37))
        };
        let f20 = ee::div(base, ee::from_i32(s3));
        let a = fix(fx, at, 0xB8C, ee::mul(f20, ee::from_i32(s3 - s2))) as u8;
        fx.mem.write_u8(sp + 0x3B3, a);
        fx.mem.write_u8(sp + 0x383, a);
        let a = fix(fx, at, 0xBB4, ee::mul(f20, ee::from_i32(s3 - s2 - 1))) as u8;
        fx.mem.write_u8(sp + 0x3CB, a);
        fx.mem.write_u8(sp + 0x39B, a);
        if s2 == 1 && fx.mem.u8(g + 0x24) & 2 != 0 {
            fx.mem.write_u8(sp + 0x3B3, 0);
            fx.mem.write_u8(sp + 0x383, 0);
        }
        let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
            fx.set_xyz_z(sp + 0x3D8, 0xFF_FFFE);
            for (to, from) in [(0x3C0u32, 0x3D8u32), (0x3A8, 0x3C0), (0x390, 0x3A8)] {
                z_to(fx, sp + to, sp + from);
            }
            0.0
        } else {
            for (to, from) in [(0x3C0u32, 0x340u32), (0x390, 0x3C0), (0x3D8, 0x348), (0x3A8, 0x3D8)] {
                z_to(fx, sp + to, sp + from);
            }
            let (pw, cw) = (fx.mem.f(sp + 0x478), fx.mem.f(sp + 0x47C));
            if crate::exe::ee_lt(pw, cw) {
                pw
            } else {
                cw
            }
        };
        // Across y.
        let (px, py) = (fx.mem.u16(sp + 0x340), fx.mem.u16(sp + 0x342));
        let (cx, cy) = (fx.mem.u16(sp + 0x348), fx.mem.u16(sp + 0x34A));
        fix16(fx, at, 0xE58, sp + 0x392, ee::sub(ee::from_i32(py), f22));
        fix16(fx, at, 0xEA4, sp + 0x3AA, ee::sub(ee::from_i32(cy), f21));
        fix16(fx, at, 0xEF0, sp + 0x3C2, ee::add(ee::from_i32(py), f22));
        fix16(fx, at, 0xF3C, sp + 0x3DA, ee::add(ee::from_i32(cy), f21));
        for (o, v) in [(0x3C0u32, px), (0x390, px), (0x3D8, cx), (0x3A8, cx)] {
            fx.mem.write_u16(sp + o, v as u16);
        }
        let key = fix(fx, at, 0xF64, depth) as i32;
        fx.submit(d, ot, key);
        // Across x.
        let (px, cx) = (fx.mem.u16(sp + 0x340), fx.mem.u16(sp + 0x348));
        fix16(fx, at, 0xFC0, sp + 0x390, ee::sub(ee::from_i32(px), f22));
        fix16(fx, at, 0x100C, sp + 0x3A8, ee::sub(ee::from_i32(cx), f21));
        fix16(fx, at, 0x1058, sp + 0x3C0, ee::add(ee::from_i32(px), f22));
        fix16(fx, at, 0x10A4, sp + 0x3D8, ee::add(ee::from_i32(cx), f21));
        let (py, cy) = (fx.mem.u16(sp + 0x342), fx.mem.u16(sp + 0x34A));
        for (o, v) in [(0x3C2u32, py), (0x392, py), (0x3DA, cy), (0x3AA, cy)] {
            fx.mem.write_u16(sp + o, v as u16);
        }
        let key = fix(fx, at, 0x10CC, depth) as i32;
        fx.submit(d, ot, key);
        s2 += 1;
    }
}
