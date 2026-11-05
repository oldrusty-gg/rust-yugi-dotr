//! Sprays of the effect library: a cluster of
//! billboards set out about a turning point along directions and radii a
//! byte table gives.
use super::exe::{atlas_cell, camera_vp_into, countdown, mul_mem, node_source, rot_trans_into, tex_rect};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// 1.33 (the billboard's y over x).
const ASPECT: u32 = 0x3FAA_3D71;
/// 2 pi.
const TWO_PI: u32 = 0x40C9_0FDB;

/// The 0x1A2B80 of an angle 65536 x a / 2 pi, low 16 bits.
fn turn16(fx: &mut Fx, a: f32, ra: u32) -> i32 {
    let v = ee::mul(65536.0, ee::div(a, f32::from_bits(TWO_PI)));
    fx.fix_residue(v, ra);
    ee::fixsfsi(v) & 0xFFFF
}

/// 0x101A00(v, out) [F]: the rotation (0x20BDA0) aiming along the vec3
/// at `v`: pitch atan2f(sqrtf(x x + z z), y), yaw atan2f(x, z), as 16-bit
/// turns (0x1A2B80), into the matrix at `out`. Leaves its frame (0x40:
/// the caller's s0 / s1 `saves` +0 / +0x10, ra `ra` +0x20, the angles
/// +0x30).
pub fn aim_rotation(fx: &mut Fx, v: u32, out: u32, saves: [u64; 2], ra: u32) {
    let sp = fx.enter(0x40);
    fx.saved(sp, &[(0, saves[0]), (0x10, saves[1]), (0x20, u64::from(ra))]);
    let (x, y, z) = (fx.mem.f(v), fx.mem.f(v + 4), fx.mem.f(v + 8));
    let r = fx.sqrtf(ee::add(ee::mul(x, x), ee::mul(z, z)));
    let a = fx.atan2f(r, y);
    let p = turn16(fx, a, 0x0010_1A60);
    fx.mem.set32(sp + 0x30, p);
    let (x, z) = (fx.mem.f(v), fx.mem.f(v + 8));
    let a = fx.atan2f(x, z);
    let q = turn16(fx, a, 0x0010_1A9C);
    fx.mem.set32(sp + 0x34, q);
    fx.mem.set32(sp + 0x38, 0);
    let m = fx.rotation_matrix([p, q, 0]);
    fx.write_m4(out, &m);
    fx.leave(0x40);
}

/// ba6c06ceea55 [6 B records; b255 0x1E05160, 572 words] [F]:
/// `sprays(at, recs, ot, n) -> 1` steps and draws `n` records (0x60
/// bytes; a 0x260 frame, the descriptor at sp + 0x1E0: one colour, four
/// UV / XYZ2 pairs). Per record: countdown 0x101AF0; the own-slot texture
/// +0xA (0x202530(5)); position +0x3C += velocity +0x48; angles +0x2C
/// (i32) += +0x54.. (floats, 0x1A2B80); size +0x20 += +0x24, radius +0x14
/// += +0x18; the position from a node with +5 (0x101BA0); through the
/// camera x (angles, position) (0x20E2B0, 0x20C0B0, 0x20B370); colour
/// +0x2A, +0x29, +0x28, alpha +0x2B (x count / 8 over the last 8 unless
/// flag 0x40); atlas cell +0xB (flag 1: elapsed & 7) of layout +0xC, blend
/// +0xD (0x1020B0, 0x101D20). Then +8 sprites, each reading the byte table
/// +0x10 from index +9 on: a radius (flag 8: +0x14; else +0x14 x t / 255,
/// with flag 4 (that + +0x14) / 2), a direction from byte differences
/// (flag 2: two, the radius along x; else three, along y) aimed by
/// 0x101A00 and applied by 0x20C540, with flag 2 a lift +0x1C x t / 255;
/// projected (0x204D80) or skipped (also off screen, 0x101600); half size
/// 0x1018E0(+0x20, w), y x 1.33; key w (0x1A2C70).
pub fn sprays(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    body(fx, at, &SPRAYS, recs, ot, n)
}

/// 0ecd639dee5c [3 B records; card 539's 0x1E03790, 516 words] (helper
/// B20) [F]: [`sprays`] compiled with the record loop's test first and
/// other registers (the same 0x260 frame and locals), so only the
/// conversions' return addresses differ.
pub fn sprays_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    body(fx, at, &SPRAYS_B, recs, ot, n)
}

/// The return addresses (from `at`) of a compilation's calls: the angle
/// conversions (0x1A2B80), the aim (0x101A00), the eight corners and the
/// key (0x1A2C70).
struct Ras {
    angle: [u32; 3],
    aim: u32,
    corners: [u32; 8],
    key: u32,
}

const SPRAYS: Ras = Ras { angle: [0xD0, 0xEC, 0x108], aim: 0x440, corners: [0x56C, 0x5D4, 0x620, 0x670, 0x6BC, 0x70C, 0x758, 0x7A4], key: 0x850 };
const SPRAYS_B: Ras = Ras { angle: [0xD8, 0xF0, 0x108], aim: 0x404, corners: [0x504, 0x55C, 0x59C, 0x5DC, 0x61C, 0x65C, 0x69C, 0x6DC], key: 0x784 };

fn body(fx: &mut Fx, at: u32, ras: &Ras, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x260);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x60;
        }
        one(fx, at, ras, sp, g, ot);
        i += 1;
    }
    fx.leave(0x260);
    1
}

fn byte(fx: &mut Fx, g: u32, i: u32) -> i32 {
    let t = fx.mem.read_u32(g + 0x10);
    fx.mem.u8(t.wrapping_add(i))
}

fn one(fx: &mut Fx, at: u32, ras: &Ras, sp: u32, g: u32, ot: u32) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 0xA);
    let tex = fx.texture(5, id);
    for o in [0x3Cu32, 0x40, 0x44] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 0xC));
        fx.mem.setf(g + o, v);
    }
    for (o, ra) in [0x2Cu32, 0x30, 0x34].into_iter().zip(ras.angle) {
        let v = ee::add(ee::from_i32(fx.mem.i32(g + o)), fx.mem.f(g + o + 0x28));
        fx.fix_residue(v, at + ra);
        fx.mem.set32(g + o, ee::fixsfsi(v));
    }
    let v = ee::add(fx.mem.f(g + 0x20), fx.mem.f(g + 0x24));
    fx.mem.setf(g + 0x20, v);
    let v = ee::add(fx.mem.f(g + 0x14), fx.mem.f(g + 0x18));
    fx.mem.setf(g + 0x14, v);
    for o in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x3C + 4 * o);
        fx.mem.write_u32(sp + 0x1A0 + 4 * o, v);
    }
    node_source(fx, g, sp + 0x1A0);
    camera_vp_into(fx, sp + 0x110);
    rot_trans_into(fx, sp + 0xD0, g + 0x2C, sp + 0x1A0);
    mul_mem(fx, sp + 0x110, sp + 0xD0);
    let mut s1 = fx.mem.u8(g + 9) as u32;
    let elapsed = fx.mem.u8(g + 2) - fx.mem.u8(g);
    let d = sp + 0x1E0;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (src, dst) in [(0x2Au32, 0x1F0u32), (0x29, 0x1F1), (0x28, 0x1F2)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + dst, v);
    }
    fx.mem.write_u32(sp + 0x1F4, 0x3F80_0000);
    let cell = if fx.mem.u8(g + 4) & 1 != 0 { elapsed & 7 } else { fx.mem.u8(g + 0xB) };
    let b = fx.mem.read_u8(g + 0xD);
    fx.mem.write_u8(sp + 0x1C5, b);
    let layout = fx.mem.u8(g + 0xC);
    atlas_cell(fx, sp + 0x1C0, cell, layout);
    tex_rect(fx, d, sp + 0x1C0, tex);
    let a = fx.mem.read_u8(g + 0x2B);
    fx.mem.write_u8(sp + 0x1F3, a);
    let c = fx.mem.u8(g);
    if fx.mem.u8(g + 4) & 0x40 == 0 && c < 8 {
        let k = fx.mem.u8(g + 0x2B) >> 3;
        let v = fx.mult(k, c);
        fx.mem.set8(sp + 0x1F3, v);
    }
    for o in [0x1A8u32, 0x1A4, 0x1A0] {
        fx.mem.write_u32(sp + o, 0);
    }
    let mut k = 0;
    while k < fx.mem.u8(g + 8) {
        let flags = fx.mem.u8(g + 4);
        let mut r = fx.mem.f(g + 0x14);
        if flags & 8 == 0 {
            let t = ee::div(ee::from_i32(byte(fx, g, s1)), 255.0);
            let big = fx.mem.f(g + 0x14);
            r = ee::mul(big, t);
            s1 = (s1 + 1) & 0xFF;
            if flags & 4 != 0 {
                r = ee::mul(0.5, ee::add(r, big));
            }
        }
        let diff = |fx: &mut Fx, i: u32| ee::from_i32(byte(fx, g, s1 + i) - byte(fx, g, s1 + i + 1));
        if flags & 2 != 0 {
            let a = diff(fx, 0);
            fx.mem.setf(sp + 0x1B0, a);
            let b = diff(fx, 2);
            fx.mem.setf(sp + 0x1B8, b);
            fx.mem.write_u32(sp + 0x1B4, 0);
            s1 = (s1 + 4) & 0xFF;
            fx.mem.setf(sp + 0x1A0, r);
        } else {
            let a = diff(fx, 0);
            fx.mem.setf(sp + 0x1B0, a);
            let b = diff(fx, 2);
            fx.mem.setf(sp + 0x1B8, b);
            let c = diff(fx, 4);
            fx.mem.setf(sp + 0x1B4, c);
            s1 = (s1 + 6) & 0xFF;
            fx.mem.setf(sp + 0x1A4, r);
        }
        aim_rotation(fx, sp + 0x1B0, sp + 0x150, [u64::from(g), u64::from(s1)], at + ras.aim);
        let m = fx.read_m4(sp + 0x150);
        let v = fx.read_v4(sp + 0x1A0);
        let p = fx.transform(Some(&m), v);
        fx.write_v4(sp + 0x190, p);
        if fx.mem.u8(g + 4) & 2 != 0 {
            let t = ee::from_i32(byte(fx, g, s1));
            let lift = ee::div(ee::mul(fx.mem.f(g + 0x1C), t), 255.0);
            let y = ee::add(fx.mem.f(sp + 0x194), lift);
            fx.mem.setf(sp + 0x194, y);
            s1 = (s1 + 1) & 0xFF;
        }
        k += 1;
        let m = fx.read_m4(sp + 0x110);
        if !fx.project_at(sp + 0x190, &m, sp + 0x200, sp + 0x25C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x200)) {
            continue;
        }
        let (size, w) = (fx.mem.f(g + 0x20), fx.mem.f(sp + 0x25C));
        let half = fx.view_scale(size, w);
        let x = ee::from_i32(fx.mem.u16(sp + 0x200));
        let y = ee::from_i32(fx.mem.u16(sp + 0x202));
        let hy = ee::mul(f32::from_bits(ASPECT), half);
        let corners = [
            (0x210u32, ee::add(x, half)),
            (0x212, ee::sub(y, hy)),
            (0x220, ee::sub(x, half)),
            (0x222, ee::add(y, hy)),
            (0x230, ee::add(x, half)),
            (0x232, ee::add(y, hy)),
            (0x200, ee::sub(x, half)),
            (0x202, ee::sub(y, hy)),
        ];
        for ((o, v), ra) in corners.into_iter().zip(ras.corners) {
            fx.fix_residue(v, at + ra);
            fx.mem.write_u16(sp + o, ee::fixunssfsi(v) as u16);
        }
        let mut z = fx.xyz_z(sp + 0x200);
        for o in [0x230u32, 0x220, 0x210] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        for o in [0x237u32, 0x227, 0x217, 0x207] {
            fx.mem.write_u8(sp + o, 1);
        }
        let w = fx.mem.f(sp + 0x25C);
        fx.fix_residue(w, at + ras.key);
        fx.submit(d, ot, ee::fixunssfsi(w) as i32);
    }
}
