//! Streaks falling through a box, hand-ported
//! from b013 (card 210) [F]: 5c73d076cb55 scatters a group's points
//! (`setup`), cf1ea681d2e2 draws each as a gouraud line along its
//! direction and moves it (`draw`), a0261be4315b colours the line
//! (`colours2`).
//!
//! A group `g`: +0 flags (0x13 far Z, 0x14 / 0x15 vertex projected, 0x1C
//! drawn twice, 0x1D the wide box), +4 the colour (B, G, R, A), +8 the
//! speed (|`speed`|), +0xC the count, +0x10 + 16 k the points.
use crate::bus::Bus;
use crate::ee;
use crate::exe::{bit, ee_le, ee_lt, Fx, M4};

/// RGBAQ / XYZ2 x 2 (a gouraud line).
const REGS_LINE: u64 = 0x4141;
const ALPHA_48: u64 = 0x80_0000_0048;
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

/// a0261be4315b [6 B records; b013 0x1E00080, 32 words] [F]:
/// `colours2(d, c, fade)`: the colour at `c` (B, G, R) into the line's
/// first RGBAQ (`fade` 0) or its second (else; the other black), both
/// alphas c[3], Q 1, the XYZ2 values' top bytes 0.
pub fn colours2(fx: &mut Fx, d: u32, c: u32, fade: i32) {
    let (on, off) = if fade == 0 { (0x10u32, 0x20u32) } else { (0x20, 0x10) };
    let rgb = [fx.mem.read_u8(c + 2), fx.mem.read_u8(c + 1), fx.mem.read_u8(c)];
    for k in 0..3u32 {
        fx.mem.write_u8(d + off + k, 0);
        fx.mem.write_u8(d + on + k, rgb[k as usize]);
    }
    let a = fx.mem.read_u8(c + 3);
    for o in [0x10u32, 0x20] {
        fx.mem.write_u8(d + o + 3, a);
        fx.mem.write_f32(d + o + 4, 1.0);
    }
    fx.mem.write_u8(d + 0x1F, 0);
    fx.mem.write_u8(d + 0x2F, 0);
}

/// The three rands of a point (s0 = the loop count, return addresses
/// `at` + `ras`), each `(m, offset, slot)`: slot = (rand % m + offset)
/// (x 0.1 when `tenth`).
fn place(fx: &mut Fx, at: u32, k: u32, p: u32, ras: [u32; 3], parts: [(i32, i32, u32); 3], tenth: bool) {
    for (i, (m, off, slot)) in parts.into_iter().enumerate() {
        let r = fx.rand_from(k, at + ras[i]);
        let (_, h) = fx.div(r, m);
        let v = ee::from_i32(h + off);
        fx.mem.setf(p + slot, if tenth { ee::mul(TENTH, v) } else { v });
    }
}

/// 5c73d076cb55 [6 B records; b013 0x1E03D60, 148 words] [F]:
/// `setup(g, n, speed) -> 0`: speed |`speed`| (0x1A7230 fabsf), `n`
/// points; with flag 0x1D (rand % 100 - 50, rand % 100 - 50, rand % 50)
/// for a negative speed, else (rand % 480 - 240, rand % 480 - 240, rand %
/// 240 + 240); without, (rand % 480 - 240, rand % 240, rand % 480 - 240) /
/// 10 (x, z, then y). A 0x50 frame.
pub fn setup(fx: &mut Fx, at: u32, g: u32, n: i32, speed: f32) -> i32 {
    let sp = fx.enter(0x50);
    // fabsf's frame: the argument at its sp.
    fx.mem.write_f32(sp - 0x10, speed);
    fx.mem.write_u32(g + 8, speed.to_bits() & 0x7FFF_FFFF);
    fx.mem.set32(g + 0xC, n);
    let mut k = 0u32;
    while (k as i32) < fx.mem.i32(g + 0xC) {
        let p = g + 16 * k;
        if bit(fx.mem.read_u32(g), 0x1D) {
            if ee_lt(speed, 0.0) {
                place(fx, at, k, p, [0x70, 0x9C, 0xC8], [(100, -50, 0x10), (100, -50, 0x14), (50, 0, 0x18)], false);
            } else {
                place(fx, at, k, p, [0xF8, 0x124, 0x150], [(480, -240, 0x10), (480, -240, 0x14), (240, 240, 0x18)], false);
            }
        } else {
            place(fx, at, k, p, [0x184, 0x1B8, 0x1EC], [(480, -240, 0x10), (480, -240, 0x18), (240, 0, 0x14)], true);
        }
        k += 1;
    }
    fx.leave(0x50);
    0
}

/// What 0x204D80 (`Fx::project_at`) leaves at the caller's sp - 0x10
/// [F 0x204D9C..0x204E48]: its local vec4, `v` with w 1 through `m`
/// (VU0, 0x20C540) and x, y, z then scaled by 1 / w (stored whether or not
/// the point is accepted). Helper C5: b013's quad trail later keys on
/// this w [run].
fn project_residue(fx: &mut Fx, v: u32, m: &M4) {
    let p = [fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8), 1.0];
    let o = Fx::vu_apply(m, p);
    let q = ee::div(1.0, o[3]);
    let a = fx.sp - 0x10;
    for k in 0..3u32 {
        fx.mem.setf(a + 4 * k, ee::mul(o[k as usize], q));
    }
    fx.mem.setf(a + 0xC, o[3]);
}

/// cf1ea681d2e2 [6 B records; b013 0x1E03FB0, 344 words] [F]:
/// `draw(ot, g, x, speed) -> 0` (descriptor at sp + 0xD0: 4 values, blend
/// 0x48): each point a line from it to it + its direction (0x20C7E0) x
/// +8, coloured by `colours2` (fading toward the tail for a negative
/// speed), projected through `x` + 0x30 (w at sp + 0x108 / + 0x10C); the
/// point moved back along its direction by `speed`; out of the box (flag
/// 0x1D: z at 300; else y at 0) respawned (x 0.1: rand % 480 - 240, rand %
/// 480 - 240 / rand % 240 into (x, y, z) / (x, z, y)); both ends: far Z
/// key 0 (flag 0x13) or the mean-w key, twice with flag 0x1C; vertex bits
/// cleared. A 0x110 frame.
pub fn draw(fx: &mut Fx, at: u32, ot: u32, g: u32, x: u32, speed: f32) -> i32 {
    let sp = fx.enter(0x110);
    let d = sp + 0xD0;
    fx.desc_init(d, 4, 0x11, REGS_LINE);
    fx.desc_alpha(d, ALPHA_48);
    let (xyz0, xyz1, w0, w1) = (sp + 0xE8, sp + 0xF8, sp + 0x108, sp + 0x10C);
    let mut k = 0u32;
    while (k as i32) < fx.mem.i32(g + 0xC) {
        let p = g + 16 * k;
        colours2(fx, d, g + 4, if ee_lt(speed, 0.0) { 1 } else { 0 });
        for c in 0..3u32 {
            let v = fx.mem.read_u32(p + 0x10 + 4 * c);
            fx.mem.write_u32(sp + 0xC0 + 4 * c, v);
            fx.mem.write_u32(sp + 0xB0 + 4 * c, v);
        }
        let m = fx.read_m4(x + 0x30);
        if fx.project_at(sp + 0xB0, &m, xyz0, w0) {
            fx.flag(g, 0x14);
        }
        project_residue(fx, sp + 0xB0, &m);
        super::trail::normalize3(fx, sp + 0xB0);
        for c in 0..3u32 {
            let v = ee::add(fx.mem.f(sp + 0xC0 + 4 * c), ee::mul(fx.mem.f(sp + 0xB0 + 4 * c), fx.mem.f(g + 8)));
            fx.mem.setf(sp + 0xC0 + 4 * c, v);
        }
        let m = fx.read_m4(x + 0x30);
        if fx.project_at(sp + 0xC0, &m, xyz1, w1) {
            fx.flag(g, 0x15);
        }
        project_residue(fx, sp + 0xC0, &m);
        for c in 0..3u32 {
            let v = ee::sub(fx.mem.f(p + 0x10 + 4 * c), ee::mul(fx.mem.f(sp + 0xB0 + 4 * c), speed));
            fx.mem.setf(p + 0x10 + 4 * c, v);
        }
        if bit(fx.mem.read_u32(g), 0x1D) {
            if !ee_lt(fx.mem.f(p + 0x18), 300.0) {
                place(fx, at, k, p, [0x240, 0x274, 0x2A8], [(480, -240, 0x10), (480, -240, 0x14), (240, 0, 0x18)], true);
            }
        } else if ee_le(fx.mem.f(p + 0x14), 0.0) {
            place(fx, at, k, p, [0x2FC, 0x330, 0x364], [(480, -240, 0x10), (480, -240, 0x18), (240, 0, 0x14)], true);
        }
        let flags = fx.mem.read_u32(g);
        if bit(flags, 0x14) && bit(flags, 0x15) {
            let twice = |fx: &mut Fx| bit(fx.mem.read_u32(g), 0x1C);
            if bit(flags, 0x13) {
                fx.far_z(&[xyz0, xyz1]);
                fx.submit(d, ot, 0);
                if twice(fx) {
                    fx.submit(d, ot, 0);
                }
            } else {
                let key = |fx: &mut Fx| ee::fixunssfsi(ee::div(ee::add(fx.mem.f(w0), fx.mem.f(w1)), 2.0)) as i32;
                let k0 = key(fx);
                fx.submit(d, ot, k0);
                if twice(fx) {
                    let k1 = key(fx);
                    fx.submit(d, ot, k1);
                }
            }
        }
        fx.clear_projected(g);
        k += 1;
    }
    fx.leave(0x110);
    0
}
