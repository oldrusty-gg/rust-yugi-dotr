//! Heat haze of the effect library: in a packet
//! layer, the attacker drawn into the offscreen target and laid back over
//! the window in vertical strips, each strip shifted up / down at random.
use super::exe::{alpha_mode, camera_vp_into, countdown};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// The window's top / left (28672.0 = 0x7000 in 12.4).
const EDGE: f32 = 28672.0;

/// cd016f6db18e [4 B records; b583 (card 386) 0x1E04EB0, 516 words] [F]:
/// `screen(at, recs, ot, n, a3) -> 1` (`a3` the caller's, reported with
/// the first TEXFLUSH) steps and draws `n` records (0x42C bytes; a 0x1F0
/// frame, ot / n at sp + 0xD8 / 0xD4, the layer (0x2078A0) at sp + 0xBC,
/// the drawn flag sp + 0xC0). Per record: countdown 0x101AF0; when the
/// wobble timer +0xF is out, +8 flips (0 -> 1: +0xF = rand & +0x10, else
/// rand & +0x11, + 1); +0xF counts down. The first record drawn builds the
/// strip descriptor (sp + 0x150, prim 0x34: RGBA 0x80, Q 1, Z 0xFFFFFF),
/// TEXFLUSH, the unit of +0x14 drawn into the target (0x1F8270, filter 0,
/// TEX0 / TEX1 at sp + 0x1C8 / 0x1D0), TEXFLUSH, flag 0x800, UVs (0, 0) /
/// (0x2000, 0) / (0, 0x2000) / (0x2000, 0x2000). Then: the camera (0x20E2B0,
/// 0x1FACE0(0)); the unit's point (0x1F65D0 kind 1) projected (0x204D80)
/// or the record is skipped; its Z on all four corners; the blend +0xC
/// (0x101C80 / 0x207E20); colour +0x1A / +0x19 / +0x18, alpha +0x1B. Strips
/// x = s1 .. s1 + +0x12 (s1 = 0, +0x12, .. below 0x200): the shift i16
/// +0x2C + 2 s1 cleared when set, else with chance +0xD / 256 set to
/// (rand % +0xE) x 2 - +0xE (0x1A2B80); U = x, V 0 .. 0x2000; Y the whole
/// window with +8 0, else shifted by the strip's value (x 16); keyed by
/// the point's w, into the layer (ot 0). The layer is linked into `ot` at
/// key 0 when a record was drawn (0x207910).
pub fn screen(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> i32 {
    screen_a3(fx, at, recs, ot, n, a3).0
}

/// [`screen`] and the a3 it leaves [F card 539 0x1E04EF0..
/// 0x1E056BC; run card 539]: the caller's `a3` unless a counting record
/// reaches its point's projection (then the w pointer sp + 0x1EC), and
/// after the strips the last strip's right edge U ((s1 + step) x 16 &
/// 0x3FFF).
pub fn screen_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> (i32, u32) {
    let mut out = a3;
    let sp = fx.enter(0x1F0);
    fx.mem.write_u32(sp + 0xD8, ot);
    fx.mem.set32(sp + 0xD4, n);
    let layer = fx.layer_begin();
    fx.mem.write_u32(sp + 0xBC, layer);
    fx.mem.set32(sp + 0xC0, 0);
    fx.mem.set32(sp + 0xA0, 0);
    let mut p = recs;
    let mut regs = [n as u32, a3];
    while fx.mem.i32(sp + 0xA0) < fx.mem.i32(sp + 0xD4) {
        if fx.mem.i32(sp + 0xA0) != 0 {
            p += 0x42C;
        }
        one(fx, at, sp, p, &mut regs, &mut out);
        let v = fx.mem.i32(sp + 0xA0) + 1;
        fx.mem.set32(sp + 0xA0, v);
    }
    if fx.mem.i32(sp + 0xC0) != 0 {
        let (layer, ot) = (fx.mem.read_u32(sp + 0xBC), fx.mem.read_u32(sp + 0xD8));
        fx.layer_end(layer, ot, 0);
    }
    fx.leave(0x1F0);
    (1, out)
}

/// rand with s0 = the record, from `at + ra`; a2 = a3 after it the new
/// state's low word - 1 (__muldi3 0x1A0EA8's low product word).
fn rand(fx: &mut Fx, at: u32, p: u32, ra: u32, regs: &mut [u32; 2]) -> i32 {
    let r = fx.rand_from(p, at + ra);
    let a = (fx.rand_state() as u32).wrapping_sub(1);
    *regs = [a, a];
    r >> 4
}

/// The 14-bit UV field of the halfword at `a`, its top bits kept.
fn uv(fx: &mut Fx, a: u32, v: u32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn fix(fx: &mut Fx, at: u32, ra: u32, v: f32) -> u16 {
    fx.fix_residue(v, at + ra);
    ee::fixunssfsi(v) as u16
}

fn one(fx: &mut Fx, at: u32, sp: u32, p: u32, regs: &mut [u32; 2], out: &mut u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    if fx.mem.u8(p + 0xF) == 0 {
        if fx.mem.read_u32(p + 8) != 0 {
            fx.mem.write_u32(p + 8, 0);
            let r = rand(fx, at, p, 0xBC, regs);
            let v = (fx.mem.u8(p + 0x11) & r) + 1;
            fx.mem.set8(p + 0xF, v);
        } else {
            fx.mem.write_u32(p + 8, 1);
            let r = rand(fx, at, p, 0xE0, regs);
            let v = (fx.mem.u8(p + 0x10) & r) + 1;
            fx.mem.set8(p + 0xF, v);
        }
    }
    let v = fx.mem.u8(p + 0xF) - 1;
    fx.mem.set8(p + 0xF, v);
    let d = sp + 0x150;
    if fx.mem.i32(sp + 0xC0) == 0 {
        fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
        for o in [0x1ABu32, 0x193, 0x17B, 0x163, 0x1AA, 0x192, 0x17A, 0x162, 0x1A9, 0x191, 0x179, 0x161, 0x1A8, 0x190, 0x178, 0x160] {
            fx.mem.write_u8(sp + o, 0x80);
        }
        for o in [0x1ACu32, 0x194, 0x17C, 0x164] {
            fx.mem.write_u32(sp + o, 0x3F80_0000);
        }
        // a1 at the first TEXFLUSH: the fourth vertex's XYZ low word as
        // it was before its new Z.
        let a1 = fx.mem.read_u32(sp + 0x1B8);
        let mut z = 0xFF_FFFF;
        for o in [0x1B8u32, 0x1A0, 0x188, 0x170] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        for o in [0x1BFu32, 0x1A7, 0x18F, 0x177] {
            fx.mem.write_u8(sp + o, 0);
        }
        fx.texflush([0, a1, regs[0], regs[1]]);
        let h = fx.mem.i32(p + 0x14);
        let (_, r) = fx.unit_draw(h, 0, sp + 0x1C8, sp + 0x1D0);
        fx.texflush(r);
        fx.mem.set32(sp + 0xC0, 1);
        let f = fx.mem.read_u16(d + 2);
        fx.mem.write_u16(d + 2, f | 0x800);
        for (o, v) in [(0x168u32, 0u32), (0x16A, 0), (0x180, 0x2000), (0x182, 0), (0x198, 0), (0x19A, 0x2000), (0x1B0, 0x2000), (0x1B2, 0x2000)] {
            uv(fx, sp + o, v);
        }
    }
    camera_vp_into(fx, sp + 0xE0);
    fx.camera_eye(0);
    let h = fx.mem.i32(p + 0x14);
    fx.unit_point(1, h, sp + 0x120);
    let m = fx.read_m4(sp + 0xE0);
    *out = sp + 0x1EC;
    if !fx.project_at(sp + 0x120, &m, sp + 0x1E0, sp + 0x1EC) {
        return;
    }
    let mut z = fx.xyz_z(sp + 0x1E0);
    for o in [0x1B8u32, 0x1A0, 0x188, 0x170] {
        fx.set_xyz_z(sp + o, z);
        z = fx.xyz_z(sp + o);
    }
    for o in [0x1BFu32, 0x1A7, 0x18F, 0x177] {
        fx.mem.write_u8(sp + o, 0);
    }
    let b = fx.mem.read_u8(p + 0xC);
    fx.mem.write_u8(sp + 0x135, b);
    alpha_mode(fx, sp + 0x130, sp + 0x1E8);
    let b = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x1E8 + k));
    let alpha = b(fx, 0) | (b(fx, 1) << 2) | (b(fx, 2) << 4) | (b(fx, 3) << 6);
    fx.desc_alpha(d, alpha);
    for (src, k) in [(0x1Bu32, 3u32), (0x1A, 0), (0x19, 1), (0x18, 2)] {
        let v = fx.mem.read_u8(p + src);
        for o in [0x1A8u32, 0x190, 0x178, 0x160] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    let mut s1 = 0i32;
    loop {
        let s6 = p + 0x2C + 2 * s1 as u32;
        if fx.mem.i16(s6) != 0 {
            fx.mem.set16(s6, 0);
        } else {
            let r = rand(fx, at, p, 0x528, regs) & 0xFF;
            if r < fx.mem.u8(p + 0xD) {
                let m = fx.mem.u8(p + 0xE);
                let r = rand(fx, at, p, 0x548, regs);
                let (_, hi) = fx.div(r, m);
                let half = (200 * m) >> 1;
                let v = ee::div(ee::from_i32(200 * hi - half), 100.0);
                fx.fix_residue(v, at + 0x5AC);
                let s = ee::fixsfsi(v);
                fx.mem.set16(s6, s);
            }
        }
        let step = fx.mem.u8(p + 0x12);
        let x = (s1 << 4) as u32;
        let x2 = ((s1 + step) << 4) as u32;
        *out = x2 & 0x3FFF;
        for (o, v) in [(0x168u32, x), (0x16A, 0), (0x180, x2), (0x182, 0), (0x198, x), (0x19A, 0x2000), (0x1B0, x2), (0x1B2, 0x2000)] {
            uv(fx, sp + o, v);
        }
        if fx.mem.read_u32(p + 8) == 0 {
            for o in [0x18Au32, 0x172] {
                fx.mem.write_u16(sp + o, 0x7000);
            }
            for o in [0x1BAu32, 0x1A2] {
                fx.mem.write_u16(sp + o, 0x9000);
            }
        } else {
            let s = fx.mem.i16(s6);
            let y = fix(fx, at, 0x6D0, ee::add(EDGE, ee::from_i32(s << 4)));
            fx.mem.write_u16(sp + 0x18A, y);
            fx.mem.write_u16(sp + 0x172, y);
            let s = fx.mem.i16(s6);
            let y = fix(fx, at, 0x700, ee::add(EDGE, ee::from_i32((s + 0x200) << 4)));
            fx.mem.write_u16(sp + 0x1BA, y);
            fx.mem.write_u16(sp + 0x1A2, y);
        }
        let v = fix(fx, at, 0x720, ee::add(EDGE, ee::from_i32(s1 << 4)));
        fx.mem.write_u16(sp + 0x1A0, v);
        fx.mem.write_u16(sp + 0x170, v);
        let step = fx.mem.u8(p + 0x12);
        let v = fix(fx, at, 0x758, ee::add(EDGE, ee::from_i32((s1 + step) << 4)));
        fx.mem.write_u16(sp + 0x1B8, v);
        fx.mem.write_u16(sp + 0x188, v);
        let w = fx.mem.f(sp + 0x1EC);
        fx.fix_residue(w, at + 0x770);
        fx.submit(d, 0, ee::fixunssfsi(w) as i32);
        s1 += fx.mem.u8(p + 0x12);
        if s1 >= 0x200 {
            break;
        }
    }
}
