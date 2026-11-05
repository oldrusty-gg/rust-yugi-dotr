//! Rods of the effect library: a glowing line
//! between two points, drawn as gouraud strips thickened about it on
//! screen.
use super::exe::{camera_vp_into, countdown, node_source};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_le, Fx};

/// 6c6f6736514f [2 B records; b255 0x1E015F0, b761 0x1E00410, 1132 words]
/// [F]: `rods(at, recs, ot, n) -> 1` steps and draws `n` records (0x38
/// bytes; a 0x1F0 frame, the descriptor at sp + 0x198: RGBAQ / XYZF2 x 4,
/// Q 1.0, ALPHA 0x48 by 0x207E20). Per record: countdown 0x101AF0; unless
/// flag 1, the ends +0x20 / +0x2C from the attacker's nodes +8 / +9
/// (0x101BA0 with +5 = 1). The ends are pushed out: P0 + (P0 - P1) x
/// +0x10 and P1 - (P0 - P1) x +0x14 (each only when > 0). Colours +0x18..
/// (far side) / +0x1C.. (near), alpha +0x1B / +0x1F (halved on odd counts
/// with flag 2). All points must project (0x204D80; the far extension
/// also on screen, 0x101600); half-widths +0xC / (w / 256) x distance /
/// 800 at the two ends (0x1018E0). Flag 0x80: z 0xFFFFFE on the ends and
/// w 0. Each segment (the ends; then P0 to the near extension; then P1 to
/// the far one) is four strips: up, down, left, right of the line, from
/// the line (colour +0x18..) to the offset edge (+0x1C..), the line's end
/// alpha 0 after the first segment; keyed by the first point's w
/// (0x1A2C70). The extensions' half-width is the caller's f22: the
/// original never sets it (`f22`).
pub fn rods(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, f22: f32) -> i32 {
    rods_with(fx, at, recs, ot, n, f22, &RODS)
}

/// Where a compilation of [`rods`] has its `jal 0x1A2C70`s (per segment,
/// per strip: the two coordinates, the key; offsets in the function).
pub struct RodLayout {
    pub segs: [[[u32; 3]; 4]; 3],
    /// The frame, how much lower its locals sit (the same caller-relative
    /// addresses) and whether n / the counter have their sp + 0xCC / 0xB0
    /// slots.
    pub frame: u32,
    pub shift: u32,
    pub slots: bool,
}

/// 6c6f6736514f's layout (b255 0x1E015F0).
pub const RODS: RodLayout = RodLayout { segs: [SEG1, SEG2, SEG3], frame: 0x1F0, shift: 0, slots: true };

/// b0fa3a2e1198 [1 B record; card 647's 0x1E05BE0, 1132 words] (helper
/// A19, 1 Nov 2026) [F]: [`rods`] compiled in a 0x180 frame (the locals
/// 0x70 lower = the same caller-relative addresses; n / the counter /
/// the points' pointers in registers, the old slots this frame's saves:
/// not written), its own conversion offsets; f22 still never set.
pub const RODS_C: RodLayout = RodLayout {
    segs: [
        [[0x5F4, 0x640, 0x668], [0x6C4, 0x710, 0x720], [0x77C, 0x7C8, 0x800], [0x85C, 0x8A8, 0x8B8]],
        [[0xA3C, 0xA88, 0xAB0], [0xB0C, 0xB58, 0xB68], [0xBC4, 0xC10, 0xC48], [0xCA4, 0xCF0, 0xD00]],
        [[0xE84, 0xED0, 0xEF8], [0xF54, 0xFA0, 0xFB0], [0x100C, 0x1058, 0x1090], [0x10EC, 0x1138, 0x1148]],
    ],
    frame: 0x180,
    shift: 0x70,
    slots: false,
};

/// The b0fa3a2e1198 compilation of [`rods`] ([`RODS_C`]).
pub fn rods_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, f22: f32) -> i32 {
    rods_with(fx, at, recs, ot, n, f22, &RODS_C)
}

/// 707f53d4327d [5 B records; b601 0x1E03C70, 1016 words]
/// [F]: [`rods`] rescheduled (the same frame and locals, its u16 -> float
/// conversions with the never-taken unsigned paths inline), its own
/// conversion offsets.
pub const RODS_B: RodLayout = RodLayout {
    segs: [
        [[0x570, 0x5AC, 0x5D4], [0x624, 0x66C, 0x67C], [0x6C8, 0x708, 0x74C], [0x798, 0x7D8, 0x7E4]],
        [[0x91C, 0x958, 0x984], [0x9D4, 0xA1C, 0xA2C], [0xA78, 0xABC, 0xB00], [0xB4C, 0xB90, 0xB9C]],
        [[0xCD4, 0xD10, 0xD40], [0xD90, 0xDD8, 0xDEC], [0xE38, 0xE7C, 0xEC4], [0xF10, 0xF54, 0xF64]],
    ],
    frame: 0x1F0,
    shift: 0,
    slots: true,
};

/// The 707f53d4327d compilation of [`rods`] ([`RODS_B`]).
pub fn rods_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, f22: f32) -> i32 {
    rods_with(fx, at, recs, ot, n, f22, &RODS_B)
}

/// [`rods`] compiled with layout `l`.
pub fn rods_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, f22: f32, l: &RodLayout) -> i32 {
    let sp = fx.enter(l.frame);
    if l.slots {
        fx.mem.set32(sp + 0xCC, n);
    }
    let mut g = recs;
    let mut i = 0;
    if l.slots {
        fx.mem.set32(sp + 0xB0, i);
    }
    while i < n {
        if i != 0 {
            g += 0x38;
        }
        one(fx, at, sp - l.shift, g, ot, f22, l);
        i += 1;
        if l.slots {
            fx.mem.set32(sp + 0xB0, i);
        }
    }
    fx.leave(l.frame);
    1
}

/// 0x1A2C70 of `v` from the `jal` at `at + off` (its frame left), as u16.
fn fix16(fx: &mut Fx, at: u32, off: u32, v: f32) -> i32 {
    fx.fix_residue(v, at + off + 8);
    ee::fixunssfsi(v) as u16 as i32
}

fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, f22: f32, l: &RodLayout) {
    if countdown(fx, g) == 0 {
        return;
    }
    if fx.mem.u8(g + 4) & 1 == 0 {
        fx.mem.write_u8(g + 5, 1);
        for (src, dst) in [(8u32, 0x20u32), (9, 0x2C)] {
            let s = fx.mem.read_u8(g + src);
            fx.mem.write_u8(g + 6, s);
            node_source(fx, g, sp + 0x150);
            for c in 0..3u32 {
                let v = fx.mem.read_u32(sp + 0x150 + 4 * c);
                fx.mem.write_u32(g + dst + 4 * c, v);
            }
        }
    }
    for c in 0..3u32 {
        let v = fx.mem.read_u32(g + 0x20 + 4 * c);
        fx.mem.write_u32(sp + 0x110 + 4 * c, v);
        let v = fx.mem.read_u32(g + 0x2C + 4 * c);
        fx.mem.write_u32(sp + 0x120 + 4 * c, v);
    }
    for c in 0..3u32 {
        let (a, b) = (fx.mem.f(g + 0x20 + 4 * c), fx.mem.f(g + 0x2C + 4 * c));
        let v = ee::add(a, ee::mul(fx.mem.f(g + 0x10), ee::sub(a, b)));
        fx.mem.setf(sp + 0x130 + 4 * c, v);
    }
    for c in 0..3u32 {
        let (b, a) = (fx.mem.f(g + 0x2C + 4 * c), fx.mem.f(g + 0x20 + 4 * c));
        let v = ee::sub(b, ee::mul(fx.mem.f(g + 0x14), ee::sub(a, b)));
        fx.mem.setf(sp + 0x140 + 4 * c, v);
    }
    camera_vp_into(fx, sp + 0xD0);
    let d = sp + 0x198;
    fx.desc_init(d, 8, 0x14, 0x4141_4141);
    for k in 0..3u32 {
        let v = fx.mem.read_u8(g + 0x1A - k);
        fx.mem.write_u8(sp + 0x1D8 + k, v);
        fx.mem.write_u8(sp + 0x1C8 + k, v);
    }
    for k in 0..3u32 {
        let v = fx.mem.read_u8(g + 0x1E - k);
        fx.mem.write_u8(sp + 0x1B8 + k, v);
        fx.mem.write_u8(sp + 0x1A8 + k, v);
    }
    for o in [0x1DCu32, 0x1CC, 0x1BC, 0x1AC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    fx.desc_alpha(d, 0x48);
    let half = fx.mem.u8(g + 4) & 2 != 0 && fx.mem.u8(g) & 1 != 0;
    let sh = u32::from(half);
    let v = fx.mem.read_u8(g + 0x1F) >> sh;
    fx.mem.write_u8(sp + 0x1BB, v);
    fx.mem.write_u8(sp + 0x1AB, v);
    let v = fx.mem.read_u8(g + 0x1B) >> sh;
    fx.mem.write_u8(sp + 0x1DB, v);
    fx.mem.write_u8(sp + 0x1CB, v);
    let m = fx.read_m4(sp + 0xD0);
    if !fx.project_at(sp + 0x110, &m, sp + 0x160, sp + 0x180) {
        return;
    }
    let m = fx.read_m4(sp + 0xD0);
    if !fx.project_at(sp + 0x120, &m, sp + 0x168, sp + 0x184) {
        return;
    }
    if !ee_le(fx.mem.f(g + 0x10), 0.0) {
        let m = fx.read_m4(sp + 0xD0);
        if !fx.project_at(sp + 0x130, &m, sp + 0x170, sp + 0x188) {
            return;
        }
    }
    if !ee_le(fx.mem.f(g + 0x14), 0.0) {
        let m = fx.read_m4(sp + 0xD0);
        if !fx.project_at(sp + 0x140, &m, sp + 0x178, sp + 0x18C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x178)) {
            return;
        }
    }
    let size = fx.mem.f(g + 0xC);
    let w = fx.mem.f(sp + 0x180);
    let f20 = fx.view_scale(size, w);
    let size = fx.mem.f(g + 0xC);
    let w = fx.mem.f(sp + 0x184);
    let f21 = fx.view_scale(size, w);
    if fx.mem.u8(g + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x170, 0xFF_FFFE);
        let z = fx.xyz_z(sp + 0x170);
        fx.set_xyz_z(sp + 0x168, z);
        let z = fx.xyz_z(sp + 0x168);
        fx.set_xyz_z(sp + 0x160, z);
        for o in [0x188u32, 0x184, 0x180] {
            fx.mem.write_u32(sp + o, 0);
        }
    }
    segment(fx, at, sp, ot, (0x160, 0x168, 0x180), (f20, f21), true, &l.segs[0]);
    fx.mem.write_u8(sp + 0x1DB, 0);
    if !ee_le(fx.mem.f(g + 0x10), 0.0) {
        segment(fx, at, sp, ot, (0x160, 0x170, 0x180), (f20, f22), false, &l.segs[1]);
    }
    if !ee_le(fx.mem.f(g + 0x14), 0.0) {
        segment(fx, at, sp, ot, (0x168, 0x178, 0x184), (f21, f22), false, &l.segs[2]);
    }
}

/// The `jal 0x1A2C70` offsets of each segment's four strips (two
/// coordinates, the key).
const SEG1: [[u32; 3]; 4] = [[0x5B0, 0x5FC, 0x628], [0x688, 0x6DC, 0x6F0], [0x74C, 0x798, 0x7E0], [0x83C, 0x888, 0x898]];
const SEG2: [[u32; 3]; 4] = [[0x9E4, 0xA30, 0xA60], [0xAC0, 0xB14, 0xB28], [0xB84, 0xBD4, 0xC1C], [0xC78, 0xCC8, 0xCD8]];
const SEG3: [[u32; 3]; 4] = [[0xE24, 0xE70, 0xEA4], [0xF04, 0xF58, 0xF70], [0xFCC, 0x101C, 0x1068], [0x10C4, 0x1114, 0x1128]];

/// One segment from the XYZ2 at sp + `a` to sp + `b` (half-widths `fa` /
/// `fb`), keyed by the w at sp + `wk`: the strips' near pair (vertices 3
/// / 4, sp + 0x1D0 / + 0x1E0) on the line, the far pair (0x1B0 / 0x1C0)
/// offset; `first`: the F bytes set to 1.
#[allow(clippy::too_many_arguments)]
fn segment(fx: &mut Fx, at: u32, sp: u32, ot: u32, (a, b, wk): (u32, u32, u32), (fa, fb): (f32, f32), first: bool, offs: &[[u32; 3]; 4]) {
    let z = fx.xyz_z(sp + a);
    fx.set_xyz_z(sp + 0x1D0, z);
    let z = fx.xyz_z(sp + 0x1D0);
    fx.set_xyz_z(sp + 0x1B0, z);
    let z = fx.xyz_z(sp + b);
    fx.set_xyz_z(sp + 0x1E0, z);
    let z = fx.xyz_z(sp + 0x1E0);
    fx.set_xyz_z(sp + 0x1C0, z);
    if first {
        for o in [0x1E7u32, 0x1C7, 0x1D7, 0x1B7] {
            fx.mem.write_u8(sp + o, 1);
        }
    }
    let key = |fx: &mut Fx, off: u32| {
        let w = fx.mem.f(sp + wk);
        fx.fix_residue(w, at + off + 8);
        ee::fixunssfsi(w) as i32
    };
    // Up / down: y offset.
    for (s, off) in [(-1i32, &offs[0]), (1, &offs[1])] {
        let (ya, yb) = (fx.mem.u16(sp + a + 2), fx.mem.u16(sp + b + 2));
        if s < 0 {
            fx.mem.set16(sp + 0x1D2, ya);
            fx.mem.set16(sp + 0x1E2, yb);
        }
        let op = |p: f32, q: f32| if s < 0 { ee::sub(p, q) } else { ee::add(p, q) };
        let v = fix16(fx, at, off[0], op(ee::from_i32(ya), fa));
        fx.mem.set16(sp + 0x1B2, v);
        let v = fix16(fx, at, off[1], op(ee::from_i32(yb), fb));
        fx.mem.set16(sp + 0x1C2, v);
        if s < 0 {
            let xa = fx.mem.u16(sp + a);
            fx.mem.set16(sp + 0x1D0, xa);
            fx.mem.set16(sp + 0x1B0, xa);
            let xb = fx.mem.u16(sp + b);
            fx.mem.set16(sp + 0x1E0, xb);
            fx.mem.set16(sp + 0x1C0, xb);
        }
        let k = key(fx, off[2]);
        fx.submit(sp + 0x198, ot, k);
    }
    // Left / right: x offset.
    for (s, off) in [(-1i32, &offs[2]), (1, &offs[3])] {
        let op = |p: f32, q: f32| if s < 0 { ee::sub(p, q) } else { ee::add(p, q) };
        let xa = fx.mem.u16(sp + a);
        let v = fix16(fx, at, off[0], op(ee::from_i32(xa), fa));
        fx.mem.set16(sp + 0x1B0, v);
        let xb = fx.mem.u16(sp + b);
        let v = fix16(fx, at, off[1], op(ee::from_i32(xb), fb));
        fx.mem.set16(sp + 0x1C0, v);
        if s < 0 {
            fx.mem.set16(sp + 0x1D0, xa);
            fx.mem.set16(sp + 0x1E0, xb);
            let ya = fx.mem.u16(sp + a + 2);
            fx.mem.set16(sp + 0x1D2, ya);
            fx.mem.set16(sp + 0x1B2, ya);
            let yb = fx.mem.u16(sp + b + 2);
            fx.mem.set16(sp + 0x1E2, yb);
            fx.mem.set16(sp + 0x1C2, yb);
        }
        let k = key(fx, off[2]);
        fx.submit(sp + 0x198, ot, k);
    }
}
