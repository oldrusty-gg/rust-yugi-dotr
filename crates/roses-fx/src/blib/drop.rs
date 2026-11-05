//! Drops of the effect library: records that
//! are each one falling, bouncing, animated billboard sprite.
use super::exe::{atlas_cell, camera_vp_into, countdown, frame_mask, node_source, tex_rect};
use crate::bus::Bus;
use crate::ee;
use crate::exe::{ee_lt, Fx};

const ASPECT: u32 = 0x3FAA_3D71;
const BOUNCE: u32 = 0x3F33_3333;

fn fix16(fx: &mut Fx, a: u32, v: f32) {
    let x = ee::fixunssfsi(v);
    fx.mem.write_u16(a, x as u16);
}

/// e18a9aacafb8 [30 B records; b002 0x1E01350, 600 words] [F]:
/// `drops(recs, ot, n) -> 1` steps and draws `n` records (0x44 bytes; a
/// 0x1C0 frame, the descriptor at sp + 0x138: one colour, four UV / XYZ2
/// pairs). Per record: countdown 0x101AF0; own-slot texture +8; velocity
/// +0x2C.. += acceleration +0x38.. (y also + gravity +0x18), position
/// +0x20.. += velocity, size +0x10 += +0x14; y below the floor +0x1C is
/// set to it (flag 2: y velocity x -0.7); a unit +5 puts the position on
/// its node (0x101BA0); colour +0xC.., alpha +0xF (x count / 16 over the
/// first 16 unless flag 0x40); atlas cell +9 (flag 1: the length gone
/// masked by 0x1022D0 of kind +0xA; flag 4 also a random corner order into
/// flag bits 0x30) of layout +0xA, blend +0xB (0x101D20); projected
/// billboard of half size 0x1018E0(+0x10, w), y x 1.33, corners by flags
/// & 0x30; key w (flag 0x80: depth 0xFFFFFE, key 0).
pub fn drops(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    drops_a3(fx, recs, ot, n).0
}

/// [`drops`] and the a3 it leaves (helper B12, for a later boundary call
/// that reports it) [F b715 0x1E01818 / 0x1E01878; run b715]: per counting
/// record the projection's w pointer sp + 0x1BC (0x204D80's a3) when it
/// fails or falls outside the window, else the low word of the fourth
/// vertex's XYZ (sp + 0x188, `ld a3`) as it was before its new z. `None`
/// when no record counted (the caller's a3 is kept).
pub fn drops_a3(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let mut t = Track::default();
    drops_by(fx, recs, ot, n, &mut t)
}

/// What the last records leave below a drops call: the last
/// record that reached the texture rectangle 0x101D20 (its pointer,
/// index, texture), the last one drawn (its last corner and its key, the
/// last two 0x1A2C70 calls) and whether the rectangle came after it.
#[derive(Default)]
struct Track {
    last: Option<(u32, i32, u32)>,
    key: Option<(f32, f32)>,
    tex_last: bool,
}

fn drops_by(fx: &mut Fx, recs: u32, ot: u32, n: i32, t: &mut Track) -> (i32, Option<u32>) {
    let mut a3 = None;
    let sp = fx.enter(0x1C0);
    fx.mem.write_u32(sp + 0xB8, ot);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x44;
        }
        one(fx, sp, p, i, &mut a3, t);
        i += 1;
    }
    fx.leave(0x1C0);
    (1, a3)
}

/// ea8dfd164aaa's [`drops`] (a 0x180 frame: p in s0, the index in s1, the
/// texture in s2) with the stack its last calls leave below the caller's
/// sp (helper A19, run b710: a later bead-trail emitter's resampled path
/// takes it as its points' w) [F]: 0x101D20's saves (s0..s2 at its sp +
/// 0 / 0x10 / 0x20, ra `ra_tex` at + 0x30) of the last record that reached
/// it, and the soft-float frame (0x30 below the 0x180 frame) of the last
/// drawn record's last corner and key conversions (the key's return
/// `ra_key`), in the order they ran.
pub fn drops_saving(fx: &mut Fx, recs: u32, ot: u32, n: i32, ra_tex: u32, ra_key: u32) -> i32 {
    let mut t = Track::default();
    let r = drops_by(fx, recs, ot, n, &mut t).0;
    let frame = fx.sp - 0x180;
    let fix = |fx: &mut Fx, t: &Track| {
        if let Some((corner, key)) = t.key {
            let sp = fx.sp;
            fx.sp = frame;
            fx.fix_residue(corner, ra_key.wrapping_sub(0x18));
            fx.fix_residue(key, ra_key);
            fx.sp = sp;
        }
    };
    if !t.tex_last {
        // The rectangle's saves, then the drawn record's conversions.
        if let Some((p, i, tex)) = t.last {
            fx.saved(frame - 0x50, &[(0, u64::from(p)), (0x10, i as i64 as u64), (0x20, u64::from(tex)), (0x30, u64::from(ra_tex))]);
        }
        fix(fx, &t);
    } else {
        fix(fx, &t);
        if let Some((p, i, tex)) = t.last {
            fx.saved(frame - 0x50, &[(0, u64::from(p)), (0x10, i as i64 as u64), (0x20, u64::from(tex)), (0x30, u64::from(ra_tex))]);
        }
    }
    r
}

fn one(fx: &mut Fx, sp: u32, p: u32, i: i32, a3: &mut Option<u32>, t: &mut Track) {
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 8);
    let tex = fx.texture(5, id);
    let v = ee::add(fx.mem.f(p + 0x2C), fx.mem.f(p + 0x38));
    fx.mem.setf(p + 0x2C, v);
    let a = ee::add(fx.mem.f(p + 0x3C), fx.mem.f(p + 0x18));
    let v = ee::add(fx.mem.f(p + 0x30), a);
    fx.mem.setf(p + 0x30, v);
    let v = ee::add(fx.mem.f(p + 0x34), fx.mem.f(p + 0x40));
    fx.mem.setf(p + 0x34, v);
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x20 + 4 * k), fx.mem.f(p + 0x2C + 4 * k));
        fx.mem.setf(p + 0x20 + 4 * k, v);
    }
    let v = ee::add(fx.mem.f(p + 0x10), fx.mem.f(p + 0x14));
    fx.mem.setf(p + 0x10, v);
    let floor = fx.mem.f(p + 0x1C);
    if ee_lt(fx.mem.f(p + 0x24), floor) {
        fx.mem.setf(p + 0x24, floor);
        if fx.mem.u8(p + 4) & 2 != 0 {
            let v = ee::neg(ee::mul(f32::from_bits(BOUNCE), fx.mem.f(p + 0x30)));
            fx.mem.setf(p + 0x30, v);
        }
    }
    if fx.mem.read_u8(p + 5) != 0 {
        node_source(fx, p, sp + 0x110);
        for k in 0..3u32 {
            let v = fx.mem.read_u32(sp + 0x110 + 4 * k);
            fx.mem.write_u32(p + 0x20 + 4 * k, v);
        }
    }
    for k in 0..3u32 {
        let v = fx.mem.read_u32(p + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0x100 + 4 * k, v);
    }
    camera_vp_into(fx, sp + 0xC0);
    let d = sp + 0x138;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0xEu32, 0x148u32), (0xD, 0x149), (0xC, 0x14A)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x14C, 0x3F80_0000);
    let gone = fx.mem.u8(p + 2) - fx.mem.u8(p);
    let mut cell = fx.mem.u8(p + 9);
    if fx.mem.u8(p + 4) & 1 != 0 {
        let kind = fx.mem.u8(p + 0xA);
        cell = frame_mask(fx, kind, gone) & 0xFF;
        let f = fx.mem.u8(p + 4);
        if f & 4 != 0 {
            fx.mem.set8(p + 4, f & 0xCF);
            let r = fx.rand();
            let bits = (((r >> 4) & 3) << 4) & 0xFF;
            let f = fx.mem.u8(p + 4);
            fx.mem.set8(p + 4, f | bits);
        }
    }
    let b = fx.mem.read_u8(p + 0xB);
    fx.mem.write_u8(sp + 0x125, b);
    let layout = fx.mem.u8(p + 0xA);
    atlas_cell(fx, sp + 0x120, cell, layout);
    tex_rect(fx, d, sp + 0x120, tex);
    t.last = Some((p, i, tex));
    t.tex_last = true;
    let a = fx.mem.read_u8(p + 0xF);
    fx.mem.write_u8(sp + 0x14B, a);
    if fx.mem.u8(p + 4) & 0x40 == 0 {
        let c = fx.mem.u8(p);
        if c < 16 {
            let a = fx.mem.u8(p + 0xF) >> 4;
            let v = fx.mult(a, c);
            fx.mem.set8(sp + 0x14B, v);
        }
    }
    let m = fx.read_m4(sp + 0xC0);
    *a3 = Some(sp + 0x1BC);
    if !fx.project_at(sp + 0x100, &m, sp + 0x1B0, sp + 0x1BC) || !Fx::on_screen(fx.mem.read_u64(sp + 0x1B0)) {
        return;
    }
    *a3 = Some(fx.mem.read_u32(sp + 0x188));
    let size = fx.mem.f(p + 0x10);
    let w = fx.mem.f(sp + 0x1BC);
    let half = fx.view_scale(size, w);
    let depth = if fx.mem.u8(p + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x188, 0xFF_FFFE);
        0.0
    } else {
        let z = fx.xyz_z(sp + 0x1B0);
        fx.set_xyz_z(sp + 0x188, z);
        fx.mem.f(sp + 0x1BC)
    };
    for (from, to) in [(0x188u32, 0x178u32), (0x178, 0x168), (0x168, 0x158)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    // (x - h, x + h, y - hy, y + hy) targets by the corner order.
    let (xl, xr, yl, yh): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match fx.mem.u8(p + 4) & 0x30 {
        0x10 => ([0x168, 0x158], [0x188, 0x178], [0x18A, 0x16A], [0x17A, 0x15A]),
        0x20 => ([0x188, 0x168], [0x178, 0x158], [0x18A, 0x17A], [0x16A, 0x15A]),
        0x30 => ([0x188, 0x178], [0x168, 0x158], [0x17A, 0x15A], [0x18A, 0x16A]),
        _ => ([0x178, 0x158], [0x188, 0x168], [0x16A, 0x15A], [0x18A, 0x17A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x1B0));
    for (o, v) in [(xl, ee::sub(x, half)), (xr, ee::add(x, half))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x1B2));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for (o, v) in [(yl, ee::sub(y, hy)), (yh, ee::add(y, hy))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let key = ee::fixunssfsi(depth) as i32;
    t.key = Some((ee::add(y, hy), depth));
    t.tex_last = false;
    let ot = fx.mem.read_u32(sp + 0xB8);
    fx.submit(d, ot, key);
}

/// efb41f6a08dd [3 B records; card 237's 0x1E00440, 640 words] (helper
/// A18) [F]: `drops_c(recs, ot, n) -> 1`, [`drops`] in a 0x160 frame (the
/// descriptor at sp + 0xD8, the matrix at + 0x70, the position at + 0xB0,
/// the cell at + 0xC0, the projection at + 0x150 / + 0x15C; ot / n in
/// registers) without the bounce, the node source and the far-Z path:
/// texture flag 4 ? (5, +9) : (4, 2); the floor +0x1C clamps y; the cell
/// +0xA, with flag 8 ((gone >> +0xB) & 3) + 2 (gone below 3: gone), 7 -
/// (count >> +0xB) on the last 7 counts, in orientation 0 with the blend
/// +8; alpha +0xF x count / 16 over the last 16 (no flag 0x40); always
/// keyed by w.
pub fn drops_c(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x160);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x44;
        }
        one_c(fx, sp, p, ot);
        i += 1;
    }
    fx.leave(0x160);
    1
}

/// cdd83c712258 [1 B record; card 615's 0x1E05400, 524 words] (helper
/// B20) [F]: [`drops_c`] compiled in a 0x1A0 frame, every local 0x40
/// higher (the descriptor at sp + 0x118, the matrix at + 0xB0, the
/// position at + 0xF0, the cell at + 0x100, the projection at + 0x190 /
/// + 0x19C), the same operations in the same order.
pub fn drops_d(fx: &mut Fx, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x1A0);
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x44;
        }
        one_c(fx, sp + 0x40, p, ot);
        i += 1;
    }
    fx.leave(0x1A0);
    1
}

fn one_c(fx: &mut Fx, sp: u32, p: u32, ot: u32) {
    if countdown(fx, p) == 0 {
        return;
    }
    let tex = if fx.mem.u8(p + 4) & 4 != 0 {
        let id = fx.mem.u8(p + 9);
        fx.texture(5, id)
    } else {
        fx.texture(4, 2)
    };
    let v = ee::add(fx.mem.f(p + 0x2C), fx.mem.f(p + 0x38));
    fx.mem.setf(p + 0x2C, v);
    let a = ee::add(fx.mem.f(p + 0x3C), fx.mem.f(p + 0x18));
    let v = ee::add(fx.mem.f(p + 0x30), a);
    fx.mem.setf(p + 0x30, v);
    let v = ee::add(fx.mem.f(p + 0x34), fx.mem.f(p + 0x40));
    fx.mem.setf(p + 0x34, v);
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x20 + 4 * k), fx.mem.f(p + 0x2C + 4 * k));
        fx.mem.setf(p + 0x20 + 4 * k, v);
    }
    let v = ee::add(fx.mem.f(p + 0x10), fx.mem.f(p + 0x14));
    fx.mem.setf(p + 0x10, v);
    if ee_lt(fx.mem.f(p + 0x24), fx.mem.f(p + 0x1C)) {
        let floor = fx.mem.read_u32(p + 0x1C);
        fx.mem.write_u32(p + 0x24, floor);
    }
    for k in 0..3u32 {
        let v = fx.mem.read_u32(p + 0x20 + 4 * k);
        fx.mem.write_u32(sp + 0xB0 + 4 * k, v);
    }
    camera_vp_into(fx, sp + 0x70);
    let d = sp + 0xD8;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0xEu32, 0xE8u32), (0xD, 0xE9), (0xC, 0xEA)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0xEC, 0x3F80_0000);
    let gone = fx.mem.u8(p + 2) - fx.mem.u8(p);
    let mut cell = fx.mem.u8(p + 0xA);
    if fx.mem.u8(p + 4) & 8 != 0 {
        let sh = fx.mem.u8(p + 0xB) & 31;
        cell = ((gone >> sh) & 3) + 2;
        if gone < 3 {
            cell = gone;
        }
        let c = fx.mem.u8(p);
        if c < 7 {
            cell = 7 - (c >> sh);
        }
    }
    let b = fx.mem.read_u8(p + 8);
    fx.mem.write_u8(sp + 0xC5, b);
    atlas_cell(fx, sp + 0xC0, cell, 0);
    tex_rect(fx, d, sp + 0xC0, tex);
    let c = fx.mem.u8(p);
    if c < 16 {
        let a = fx.mem.u8(p + 0xF) >> 4;
        let v = fx.mult(a, c);
        fx.mem.set8(sp + 0xEB, v);
    } else {
        let a = fx.mem.read_u8(p + 0xF);
        fx.mem.write_u8(sp + 0xEB, a);
    }
    let m = fx.read_m4(sp + 0x70);
    if !fx.project_at(sp + 0xB0, &m, sp + 0x150, sp + 0x15C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x150)) {
        return;
    }
    let size = fx.mem.f(p + 0x10);
    let w = fx.mem.f(sp + 0x15C);
    let half = fx.view_scale(size, w);
    let z = fx.xyz_z(sp + 0x150);
    fx.set_xyz_z(sp + 0x128, z);
    for (from, to) in [(0x128u32, 0x118u32), (0x118, 0x108), (0x108, 0xF8)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    let (xl, xr, yl, yh): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match fx.mem.u8(p + 4) & 0x30 {
        0x10 => ([0x108, 0xF8], [0x128, 0x118], [0x12A, 0x10A], [0x11A, 0xFA]),
        0x20 => ([0x128, 0x108], [0x118, 0xF8], [0x12A, 0x11A], [0x10A, 0xFA]),
        0x30 => ([0x128, 0x118], [0x108, 0xF8], [0x11A, 0xFA], [0x12A, 0x10A]),
        _ => ([0x118, 0xF8], [0x128, 0x108], [0x10A, 0xFA], [0x12A, 0x11A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x150));
    for (o, v) in [(xl, ee::sub(x, half)), (xr, ee::add(x, half))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x152));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for (o, v) in [(yl, ee::sub(y, hy)), (yh, ee::add(y, hy))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let key = ee::fixunssfsi(w) as i32;
    fx.submit(d, ot, key);
}

/// 0f8b7add37bb [3 B records; b128 (card 64) 0x1E02890, 768 words] (helper
/// A16): `scatter_drops(recs, ot, n) -> 1`, [`drops`] recompiled for
/// 0x3C-byte records in a 0x180 frame (the descriptor at sp + 0xF8, the
/// matrix at + 0x80, the position at + 0xC0, the projection at + 0x170 /
/// + 0x17C). Per record: countdown 0x101AF0; own-slot texture +8; position
/// +0x18.. += velocity +0x24..; while the length gone masked by +3 is 0,
/// and on the first count, a unit +5 puts the position on its node
/// (0x101BA0) and three rands scatter +0x30.. by ((rand >> 4) % +0x14 -
/// +0x14 / 2) / 100; drawn at the position plus the scatter; colour
/// +0xC.., alpha +0xF (x count / 8 over the first 8 unless flag 0x40);
/// atlas cell = the masked length gone & 7 (7 - count below 4) of layout
/// +0xA in orientation 0 (flag 1 at cell 0: a random corner order into
/// flag bits 0x30); projected billboard of half size 0x1018E0(+0x10, w),
/// y x 1.33, corners by flags & 0x30; depth from the projection (flag
/// 0x80: 0xFFFFFE), always key 0. `at`: the function's address (rand's
/// frames: s0 = the record) [F; fnparity].
pub fn scatter_drops(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    scatter_drops_with(fx, at, recs, ot, n, &SCATTER)
}

/// Where a compilation of [`scatter_drops`] keeps its frame and rand calls.
pub struct ScatterLayout {
    /// The frame size (the locals are caller-relative the same in both).
    pub frame: u32,
    /// n / ot stored at these offsets of the 0x180-frame base (`None`: in
    /// registers).
    pub args: Option<(u32, u32)>,
    /// rand's saved s0: the record (`None`) or this offset of the base.
    pub rand_s0: Option<u32>,
    /// The return addresses of the three scatter rands and the corner rand.
    pub ras: [u32; 4],
}

/// 0f8b7add37bb's layout (b128 0x1E02890).
pub const SCATTER: ScatterLayout = ScatterLayout { frame: 0x180, args: None, rand_s0: None, ras: [0x110, 0x164, 0x1B8, 0x2D8] };

/// e5a6a9362688 [3 B records; b131 (card 110) 0x1E05440, 616 words] (helper
/// B16) [F]: [`scatter_drops`] compiled in [`drops`]' 0x1C0 frame (every
/// local 0x40 higher: the descriptor at sp + 0x138, the matrix at + 0xC0,
/// the position at + 0x100, the projection at + 0x1B0 / + 0x1BC, so at the
/// same caller-relative addresses), n / ot stored at sp + 0xB4 / 0xB8,
/// rand's saved s0 the vertex-3 pointer sp + 0x168, its returns at +0x120
/// / +0x15C / +0x198 / +0x288.
pub const SCATTER_B: ScatterLayout = ScatterLayout { frame: 0x1C0, args: Some((0x74, 0x78)), rand_s0: Some(0x128), ras: [0x120, 0x15C, 0x198, 0x288] };

/// The e5a6a9362688 compilation of [`scatter_drops`] ([`SCATTER_B`]).
pub fn scatter_drops_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    scatter_drops_with(fx, at, recs, ot, n, &SCATTER_B)
}

/// [`scatter_drops`] compiled with layout `l`; `sp` below is the
/// 0x180-frame base (the locals' caller-relative addresses).
pub fn scatter_drops_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, l: &ScatterLayout) -> i32 {
    let sp = fx.enter(l.frame) + l.frame - 0x180;
    if let Some((an, ao)) = l.args {
        fx.mem.set32(sp + an, n);
        fx.mem.write_u32(sp + ao, ot);
    }
    let mut p = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            p += 0x3C;
        }
        scatter_one(fx, at, sp, p, ot, l);
        i += 1;
    }
    fx.leave(l.frame);
    1
}

fn scatter_one(fx: &mut Fx, at: u32, sp: u32, p: u32, ot: u32, l: &ScatterLayout) {
    let s0 = l.rand_s0.map_or(p, |o| sp + o);
    if countdown(fx, p) == 0 {
        return;
    }
    let id = fx.mem.u8(p + 8);
    let tex = fx.texture(5, id);
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x18 + 4 * k), fx.mem.f(p + 0x24 + 4 * k));
        fx.mem.setf(p + 0x18 + 4 * k, v);
    }
    let gone = (fx.mem.u8(p + 2) - fx.mem.u8(p)) & fx.mem.u8(p + 3);
    if gone == 0 || fx.mem.u8(p + 2) - 1 == fx.mem.u8(p) {
        if fx.mem.read_u8(p + 5) != 0 {
            node_source(fx, p, sp + 0xD0);
            for k in 0..3u32 {
                let v = fx.mem.read_u32(sp + 0xD0 + 4 * k);
                fx.mem.write_u32(p + 0x18 + 4 * k, v);
            }
        }
        for (k, ra) in l.ras[..3].iter().enumerate() {
            let r = fx.rand_from(s0, at + ra) >> 4;
            let m = fx.mem.i32(p + 0x14);
            let (_, h) = fx.div(r, m);
            let v = ee::div(ee::from_i32(h - (m >> 1)), 100.0);
            fx.mem.setf(p + 0x30 + 4 * k as u32, v);
        }
    }
    for k in 0..3u32 {
        let v = ee::add(fx.mem.f(p + 0x18 + 4 * k), fx.mem.f(p + 0x30 + 4 * k));
        fx.mem.setf(sp + 0xC0 + 4 * k, v);
    }
    camera_vp_into(fx, sp + 0x80);
    let d = sp + 0xF8;
    fx.desc_init(d, 9, 0x24, 0x4_2424_2421);
    for (s, o) in [(0xEu32, 0x108u32), (0xD, 0x109), (0xC, 0x10A)] {
        let v = fx.mem.read_u8(p + s);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x10C, 0x3F80_0000);
    let mut cell = gone & 7;
    if fx.mem.u8(p) < 4 {
        cell = 7 - fx.mem.u8(p);
    }
    if fx.mem.u8(p + 4) & 1 != 0 && cell == 0 {
        let r = fx.rand_from(s0, at + l.ras[3]);
        let bits = (((r >> 4) & 3) << 4) & 0xFF;
        let f = fx.mem.u8(p + 4);
        fx.mem.set8(p + 4, (f & 0xC7) | bits);
    }
    let layout = fx.mem.read_u8(p + 0xA);
    fx.mem.write_u8(sp + 0xE5, layout);
    atlas_cell(fx, sp + 0xE0, cell, 0);
    tex_rect(fx, d, sp + 0xE0, tex);
    let a = fx.mem.read_u8(p + 0xF);
    fx.mem.write_u8(sp + 0x10B, a);
    if fx.mem.u8(p + 4) & 0x40 == 0 {
        let c = fx.mem.u8(p);
        if c < 8 {
            let a = fx.mem.u8(p + 0xF) >> 3;
            let v = fx.mult(a, c);
            fx.mem.set8(sp + 0x10B, v);
        }
    }
    let m = fx.read_m4(sp + 0x80);
    if !fx.project_at(sp + 0xC0, &m, sp + 0x170, sp + 0x17C) || !Fx::on_screen(fx.mem.read_u64(sp + 0x170)) {
        return;
    }
    let size = fx.mem.f(p + 0x10);
    let w = fx.mem.f(sp + 0x17C);
    let half = fx.view_scale(size, w);
    if fx.mem.u8(p + 4) & 0x80 != 0 {
        fx.set_xyz_z(sp + 0x148, 0xFF_FFFE);
    } else {
        let z = fx.xyz_z(sp + 0x170);
        fx.set_xyz_z(sp + 0x148, z);
    }
    for (from, to) in [(0x148u32, 0x138u32), (0x138, 0x128), (0x128, 0x118)] {
        let z = fx.xyz_z(sp + from);
        fx.set_xyz_z(sp + to, z);
    }
    // (x - h, x + h, y - hy, y + hy) targets by the corner order.
    let (xl, xr, yl, yh): ([u32; 2], [u32; 2], [u32; 2], [u32; 2]) = match fx.mem.u8(p + 4) & 0x30 {
        0x10 => ([0x128, 0x118], [0x148, 0x138], [0x14A, 0x12A], [0x13A, 0x11A]),
        0x20 => ([0x148, 0x128], [0x138, 0x118], [0x14A, 0x13A], [0x12A, 0x11A]),
        0x30 => ([0x148, 0x138], [0x128, 0x118], [0x13A, 0x11A], [0x14A, 0x12A]),
        _ => ([0x138, 0x118], [0x148, 0x128], [0x12A, 0x11A], [0x14A, 0x13A]),
    };
    let x = ee::from_i32(fx.mem.u16(sp + 0x170));
    for (o, v) in [(xl, ee::sub(x, half)), (xr, ee::add(x, half))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    let y = ee::from_i32(fx.mem.u16(sp + 0x172));
    let hy = ee::mul(f32::from_bits(ASPECT), half);
    for (o, v) in [(yl, ee::sub(y, hy)), (yh, ee::add(y, hy))] {
        for a in o {
            fix16(fx, sp + a, v);
        }
    }
    fx.submit(d, ot, 0);
}
