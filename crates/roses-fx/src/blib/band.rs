//! Ring bands of the effect library: a textured
//! band between two growing concentric rings, turned and placed by the
//! record.
use super::exe::{alpha_mode, atlas_cell, camera_vp_into, countdown, libm_cosf, mul_mem, node_source, rot_trans_into};
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// RGBAQ / ST / XYZF2 x 4 (a textured strip).
const REGS_TEX4: u64 = 0x4214_2142_1421;

/// The handlers of 0x102210's jump table (0x2B8390, read at runtime) and
/// the cell size (w, h) each stores [F 0x102238..0x1022B8].
const CELL_SIZES: [(u32, u32, u32); 7] = [
    (0x0010_2238, 0x4300_0000, 0x4280_0000),
    (0x0010_224C, 0x4280_0000, 0x4300_0000),
    (0x0010_2260, 0x4300_0000, 0x4300_0000),
    (0x0010_2270, 0x4380_0000, 0x4280_0000),
    (0x0010_2284, 0x4200_0000, 0x4300_0000),
    (0x0010_2298, 0x4300_0000, 0x4280_0000),
    (0x0010_22AC, 0x4280_0000, 0x4280_0000),
];
const CELL_TABLE: u32 = 0x002B_8390;

/// 0x102210(layout, w, h): the atlas layout's cell size in texels as
/// floats into `w` / `h`; 0 for a layout past 9; `None` (left as they
/// were) for a table entry that is none of the seven handlers.
pub fn cell_size(fx: &mut Fx, layout: i32, w: u32, h: u32) -> Option<()> {
    if layout as u32 >= 10 {
        fx.mem.write_u32(h, 0);
        fx.mem.write_u32(w, 0);
        return Some(());
    }
    let handler = fx.mem.read_u32(CELL_TABLE + 4 * layout as u32);
    let &(_, cw, ch) = CELL_SIZES.iter().find(|c| c.0 == handler)?;
    fx.mem.write_u32(w, cw);
    fx.mem.write_u32(h, ch);
    Some(())
}

/// f6bd4fdaf63f [16 B records; b099 0x1E03690, 508 words] [F]:
/// `bands(at, recs, ot, n) -> 1` steps and draws `n` band records (0x5C
/// bytes; a 0x6C0 frame, the descriptor at sp + 0x620). Per record:
/// countdown 0x101AF0; the own-slot texture +8 (0x202530(5)); radii +0x10
/// / +0x14 += +0x18 / +0x1C, i32 angles +0x3C.. += +0x4C..; two rings of
/// n = min(+7, 36) points (radius x cos / sin of 360 k / n degrees, libm
/// 0x1A7158 / 0x1A7348) in the plane y = 0 (sp + 0x150 / + 0x3A0, closed);
/// the position +0x2C from a node (+5, 0x101BA0); through the camera x
/// (angles +0x3C, position +0x2C) (0x20E2B0, 0x20C0B0, 0x20B370 into sp +
/// 0x110); colours inner +0x26.. / outer +0x2A.., alphas +0x27 / +0x2B (x
/// count / 8 over the last 8); atlas cell +0xB of layout +0xA with blend
/// +9 (0x1020B0), the layout's cell size (0x102210); +0xC & 3: 1 no
/// clamp and the cell width x +0x20, 2 the left half texel column, else
/// the cell's REGION_CLAMP (0x208400); per segment the four corners
/// projected (0x204D80; a corner off the view skips it), STQ over the
/// cell, ALPHA from the blend (0x101C80, stale for an unknown one), the
/// texture (0x207E50 filter 1); flag 0x80 at the far Z with key 0, else
/// keyed by the mean w (0x1A2C70). `at` (the function's address in the
/// running record) only names the conversion's return address.
pub fn bands(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    bands_ra(fx, at + 0x750, recs, ot, n, &mut None)
}

/// [`bands`] and the a3 it leaves (helper C27, as [`bands_d_a3`]'s
/// segments) [run b458]: per segment the projections' w pointers (sp +
/// 0x5F0..0x5FC) as far as they get, then 0x207E50's (the texture's TEX0
/// low word, or the descriptor + 0x10 for a missing texture). `None`
/// when no segment was reached ([H]: a counting record's camera and
/// placement calls not modelled).
pub fn bands_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let mut a3 = None;
    let r = bands_ra(fx, at + 0x750, recs, ot, n, &mut a3);
    (r, a3)
}

/// a059c7a06acc [18 B records; b002 0x1E03FD0, 464 words] [F + run]:
/// [`bands`] compiled with its loops rotated (the same 0x6C0 frame and
/// locals), the conversion's return at +0x6B8.
pub fn bands_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    bands_b_a3(fx, at, recs, ot, n).0
}

/// [`bands_b`] and the a3 it leaves, by [`bands_a3`]'s rule
/// [run b379 (card 2): a later frame_fade_b reports it].
pub fn bands_b_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let mut a3 = None;
    let r = bands_ra(fx, at + 0x6B8, recs, ot, n, &mut a3);
    (r, a3)
}

/// 34a784d12fb6 [2 B records; card 261's 0x1E03A80, 576 words] (helper
/// A18) [F]: [`bands`] compiled in a 0x690 frame (every local 0x30 lower,
/// the same caller-relative addresses), ot / n / the record counter / the
/// texture in registers (no slots: the shared base's sp + 0xC0..0xC8 is
/// this frame's saved ra), the angle and the S / T quotients recomputed
/// per use (the same operations in the same order); the conversion's
/// return at +0x880.
pub fn bands_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x690).wrapping_sub(0x30);
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x5C;
        }
        one(fx, at + 0x880, sp, g, ot, false, &mut None);
        i += 1;
    }
    fx.leave(0x690);
    1
}

/// The body; `fix_ra` the 0x1A2C70 call's return address.
fn bands_ra(fx: &mut Fx, fix_ra: u32, recs: u32, ot: u32, n: i32, a3: &mut Option<u32>) -> i32 {
    let at = fix_ra;
    let sp = fx.enter(0x6C0);
    fx.mem.write_u32(sp + 0xC8, ot);
    fx.mem.set32(sp + 0xC4, n);
    let mut g = recs;
    fx.mem.set32(sp + 0xB0, 0);
    while fx.mem.i32(sp + 0xB0) < fx.mem.i32(sp + 0xC4) {
        if fx.mem.i32(sp + 0xB0) != 0 {
            g += 0x5C;
        }
        one(fx, at, sp, g, ot, true, a3);
        let v = fx.mem.i32(sp + 0xB0) + 1;
        fx.mem.set32(sp + 0xB0, v);
    }
    fx.leave(0x6C0);
    1
}

/// `slots`: the texture kept at sp + 0xC0 (else in a register).
fn one(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, slots: bool, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    if slots {
        fx.mem.write_u32(sp + 0xC0, tex);
    }
    for o in [0x10u32, 0x14] {
        let v = ee::add(fx.mem.f(g + o), fx.mem.f(g + o + 8));
        fx.mem.setf(g + o, v);
    }
    for o in [0x3Cu32, 0x40, 0x44] {
        let v = fx.mem.i32(g + o).wrapping_add(fx.mem.i32(g + o + 0x10));
        fx.mem.set32(g + o, v);
    }
    let n = fx.mem.u8(g + 7).min(0x24);
    let step = ee::div(360.0, ee::from_i32(n));
    for ring in 0..2u32 {
        for k in 0..n {
            let deg = ee::mul(step, ee::from_i32(k));
            let a = ee::div(ee::mul(f32::from_bits(0x4049_0FDB), deg), 180.0);
            let p = sp + 0x150 + 0x250 * ring + 16 * k as u32;
            let r = fx.mem.f(g + 0x10 + 4 * ring);
            let c = libm_cosf(fx, a);
            fx.mem.setf(p, ee::mul(r, c));
            let s = fx.libm_sinf(a);
            let r = fx.mem.f(g + 0x10 + 4 * ring);
            fx.mem.setf(p + 8, ee::mul(r, s));
            fx.mem.write_u32(p + 4, 0);
        }
    }
    for base in [0x150u32, 0x3A0] {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(sp + base + 4 * c);
            fx.mem.write_u32(sp + base + 16 * n as u32 + 4 * c, w);
        }
    }
    node_source(fx, g, g + 0x2C);
    camera_vp_into(fx, sp + 0x110);
    rot_trans_into(fx, sp + 0xD0, g + 0x3C, g + 0x2C);
    mul_mem(fx, sp + 0x110, sp + 0xD0);
    let d = sp + 0x620;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    for (k, src) in [(0u32, 0x26u32), (1, 0x25), (2, 0x24)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + 0x648 + k, v);
        fx.mem.write_u8(sp + 0x630 + k, v);
        let v = fx.mem.read_u8(g + src + 4);
        fx.mem.write_u8(sp + 0x678 + k, v);
        fx.mem.write_u8(sp + 0x660 + k, v);
    }
    let b = fx.mem.read_u8(g + 9);
    fx.mem.write_u8(sp + 0x605, b);
    let (cell, layout) = (fx.mem.u8(g + 0xB), fx.mem.u8(g + 0xA));
    atlas_cell(fx, sp + 0x600, cell, layout);
    cell_size(fx, layout, sp + 0x6B4, sp + 0x6B8);
    match fx.mem.u8(g + 0xC) & 3 {
        1 => {
            fx.desc_clamp(d, 0);
            let v = ee::mul(fx.mem.f(sp + 0x6B4), fx.mem.f(g + 0x20));
            fx.mem.setf(sp + 0x6B4, v);
        }
        m => {
            if m == 2 {
                let (u0, u1) = (fx.mem.i32(sp + 0x608), fx.mem.i32(sp + 0x60C));
                let u = u0.wrapping_add(u1.wrapping_sub(u0) >> 1);
                fx.mem.set32(sp + 0x608, u);
                fx.mem.set32(sp + 0x60C, u.wrapping_add(1));
            }
            let (u0, u1, v0, v1) = (fx.mem.i32(sp + 0x608), fx.mem.i32(sp + 0x60C), fx.mem.i32(sp + 0x610), fx.mem.i32(sp + 0x614));
            let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
            fx.desc_clamp(d, clamp);
        }
    }
    let c = fx.mem.u8(g);
    let (ai, ao) = if c < 8 {
        let k = fx.mem.u8(g + 0x27) >> 3;
        let ai = fx.mult(c, k);
        let k = fx.mem.u8(g + 0x2B) >> 3;
        (ai, fx.mem.u8(g).wrapping_mul(k))
    } else {
        (fx.mem.u8(g + 0x27), fx.mem.u8(g + 0x2B))
    };
    for o in [0x64Bu32, 0x633] {
        fx.mem.set8(sp + o, ai);
    }
    for o in [0x67Bu32, 0x663] {
        fx.mem.set8(sp + o, ao);
    }
    let mut k = 0;
    while k < n {
        segment(fx, at, sp, g, ot, k as u32, if slots { None } else { Some(tex) }, a3);
        k += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn segment(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, k: u32, tex: Option<u32>, a3: &mut Option<u32>) {
    let d = sp + 0x620;
    let p = sp + 0x150 + 16 * k;
    let corners = [(p, 0x640u32, 0x5F0u32), (p + 16, 0x658, 0x5F4), (p + 0x250, 0x670, 0x5F8), (p + 0x260, 0x688, 0x5FC)];
    for (v, xyz, w) in corners {
        let m = fx.read_m4(sp + 0x110);
        *a3 = Some(sp + w);
        if !fx.project_at(v, &m, sp + xyz, sp + w) {
            return;
        }
    }
    for o in [0x68Fu32, 0x677, 0x65F, 0x647] {
        fx.mem.write_u8(sp + o, 0);
    }
    let w: [f32; 4] = std::array::from_fn(|i| fx.mem.f(sp + 0x5F0 + 4 * i as u32));
    for (i, o) in [0x634u32, 0x64C, 0x664, 0x67C].into_iter().enumerate() {
        fx.mem.setf(sp + o, ee::div(1.0, w[i]));
    }
    let (cw, ch) = (fx.mem.f(sp + 0x6B4), fx.mem.f(sp + 0x6B8));
    let s0 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x608)), cw);
    let t0 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x610)), ch);
    let s1 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x60C)), cw);
    let t1 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x614)), ch);
    for (o, s, i) in [(0x638u32, s0, 0usize), (0x63C, t0, 0), (0x650, s1, 1), (0x654, t0, 1), (0x668, s0, 2), (0x66C, t1, 2), (0x680, s1, 3), (0x684, t1, 3)] {
        fx.mem.setf(sp + o, ee::div(s, w[i]));
    }
    alpha_mode(fx, sp + 0x600, sp + 0x6BC);
    let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x6BC + k));
    let a = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
    fx.desc_alpha(d, a);
    let tex = tex.unwrap_or_else(|| fx.mem.read_u32(sp + 0xC0));
    *a3 = Some(if fx.mem.read_u16(d + 2) & 0x20 != 0 && tex != 0 { fx.mem.read_u32(tex + 0x30) } else { d + 0x10 });
    fx.desc_texture(d, tex, 1);
    let depth = if fx.mem.u8(g + 4) & 0x80 != 0 {
        let mut z = 0xFF_FFFE;
        for o in [0x688u32, 0x670, 0x658, 0x640] {
            fx.set_xyz_z(sp + o, z);
            z = fx.xyz_z(sp + o);
        }
        0.0
    } else {
        ee::div(ee::add(w[3], ee::add(w[2], ee::add(w[0], w[1]))), 4.0)
    };
    let key = ee::fixunssfsi(depth) as i32;
    fx.fix_residue(depth, at);
    fx.submit(d, ot, key);
}

/// 973cfb0e1a63 [3 B records; card 539's 0x1E03FA0, 516 words] (helper
/// B20) [F]: `bands_d(at, recs, ot, n) -> 1`, tilted bands in a 0x6D0
/// frame (the descriptor at sp + 0x628, the spec at sp + 0x610, every
/// other local of [`bands`] 0x10 higher). First the camera (0x20E2B0
/// into sp + 0x110) and the depth base sp + 0x6C0 from the first record:
/// +0xC flag 0x20 the far 0x4F7FFFFF, else the w of unit point +0xD
/// (0x1F65D0 kind 1, sp + 0x5F0, projected into sp + 0x6B8) or 0 off the
/// view. Per record: countdown; texture (5, +8); radius +0x10 += +0x14,
/// i32 angles +0x3C.. += +0x4C..; n = min(+7, 36) points of two rings,
/// radius +0x10 -/+ +0x18 cos(+0x1C deg) at y = +/- +0x18 sin(+0x1C deg)
/// (libm), closed; placed as [`bands`]; cell +0xB of layout +0xA, blend
/// +9; +0xC & 3: 1 no clamp and width x +0x20, 2 the cell's middle
/// (u0 + h - 1, u1 - h + 1 with h = (u1 - u0) / 2), else the cell's
/// clamp; alphas +0x27 / +0x2B, with +0xC 8 and elapsed < 31 (alpha >> 5)
/// x elapsed, then with +0xC 0x10 and count < 31 (alpha >> 5) x count;
/// segments as [`bands`] keyed by the base + +0xE x (record + 1).
pub fn bands_d(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> i32 {
    bands_d_a3(fx, at, recs, ot, n).0
}

/// [`bands_d`] and the a3 it leaves [F card 539 0x1E03FA0..0x1E0472C; run
/// card 539]: per segment the projections' w pointers (sp + 0x600..0x60C)
/// as far as they get, then 0x207E50's (the texture's TEX0 low word, or
/// the descriptor + 0x10 for a missing texture); before any, the camera
/// matrix 0x20E2B0 points at (with +0xC 0x20) or the depth base's w
/// pointer sp + 0x6C0 ([H]: a counting record's other calls keep a3).
pub fn bands_d_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32) -> (i32, Option<u32>) {
    let mut a3;
    let sp = fx.enter(0x6D0);
    fx.mem.write_u32(sp + 0xC8, ot);
    fx.mem.set32(sp + 0xC4, n);
    camera_vp_into(fx, sp + 0x110);
    // 0x20E2B0 leaves a3 at the camera record's matrix.
    let cam = 0x002E_D760u32.wrapping_add(fx.mem.read_u32(0x002E_D6C0).wrapping_mul(0x1A0));
    a3 = Some(cam);
    if fx.mem.u8(recs + 0xC) & 0x20 != 0 {
        fx.mem.write_u32(sp + 0x6C0, 0x4F7F_FFFF);
    } else {
        a3 = Some(sp + 0x6C0);
        let slot = fx.mem.u8(recs + 0xD);
        fx.unit_point(1, slot, sp + 0x5F0);
        let m = fx.read_m4(sp + 0x110);
        if !fx.project_at(sp + 0x5F0, &m, sp + 0x6B8, sp + 0x6C0) {
            fx.mem.write_u32(sp + 0x6C0, 0);
        }
    }
    let mut g = recs;
    let mut i = 0;
    while i < n {
        if i != 0 {
            g += 0x5C;
        }
        one_d(fx, at, sp, g, ot, i, &mut a3);
        i += 1;
    }
    fx.leave(0x6D0);
    (1, a3)
}

fn one_d(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, rec: i32, a3: &mut Option<u32>) {
    if countdown(fx, g) == 0 {
        return;
    }
    let id = fx.mem.u8(g + 8);
    let tex = fx.texture(5, id);
    let v = ee::add(fx.mem.f(g + 0x10), fx.mem.f(g + 0x14));
    fx.mem.setf(g + 0x10, v);
    for o in [0x3Cu32, 0x40, 0x44] {
        let v = fx.mem.i32(g + o).wrapping_add(fx.mem.i32(g + o + 0x10));
        fx.mem.set32(g + o, v);
    }
    let n = fx.mem.u8(g + 7).min(0x24);
    let step = ee::div(360.0, ee::from_i32(n));
    let pi = f32::from_bits(0x4049_0FDB);
    let tilt = ee::div(ee::mul(pi, fx.mem.f(g + 0x1C)), 180.0);
    let c = libm_cosf(fx, tilt);
    let dr = ee::mul(fx.mem.f(g + 0x18), c);
    let tilt = ee::div(ee::mul(pi, fx.mem.f(g + 0x1C)), 180.0);
    let s = fx.libm_sinf(tilt);
    let dy = ee::mul(fx.mem.f(g + 0x18), s);
    if n > 0 {
        for ring in 0..2u32 {
            for k in 0..n {
                let deg = ee::mul(step, ee::from_i32(k));
                let a = ee::div(ee::mul(pi, deg), 180.0);
                let p = sp + 0x150 + 0x250 * ring + 16 * k as u32;
                let r = |fx: &mut Fx| if ring == 0 { ee::sub(fx.mem.f(g + 0x10), dr) } else { ee::add(fx.mem.f(g + 0x10), dr) };
                let c = libm_cosf(fx, a);
                let x = ee::mul(r(fx), c);
                fx.mem.setf(p, x);
                let s = fx.libm_sinf(a);
                let z = ee::mul(r(fx), s);
                fx.mem.setf(p + 8, z);
                fx.mem.write_u32(p + 4, if ring == 0 { dy.to_bits() } else { dy.to_bits() ^ 0x8000_0000 });
            }
        }
    }
    for base in [0x150u32, 0x3A0] {
        for c in 0..4u32 {
            let w = fx.mem.read_u32(sp + base + 4 * c);
            fx.mem.write_u32(sp + base + 16 * n as u32 + 4 * c, w);
        }
    }
    node_source(fx, g, g + 0x2C);
    camera_vp_into(fx, sp + 0x110);
    rot_trans_into(fx, sp + 0xD0, g + 0x3C, g + 0x2C);
    mul_mem(fx, sp + 0x110, sp + 0xD0);
    let d = sp + 0x628;
    fx.desc_init(d, 0xC, 0x34, REGS_TEX4);
    for (k, src) in [(0u32, 0x26u32), (1, 0x25), (2, 0x24)] {
        let v = fx.mem.read_u8(g + src);
        fx.mem.write_u8(sp + 0x650 + k, v);
        fx.mem.write_u8(sp + 0x638 + k, v);
        let v = fx.mem.read_u8(g + src + 4);
        fx.mem.write_u8(sp + 0x680 + k, v);
        fx.mem.write_u8(sp + 0x668 + k, v);
    }
    let b = fx.mem.read_u8(g + 9);
    fx.mem.write_u8(sp + 0x615, b);
    let (cell, layout) = (fx.mem.u8(g + 0xB), fx.mem.u8(g + 0xA));
    atlas_cell(fx, sp + 0x610, cell, layout);
    cell_size(fx, layout, sp + 0x6C4, sp + 0x6C8);
    match fx.mem.u8(g + 0xC) & 3 {
        1 => {
            fx.desc_clamp(d, 0);
            let v = ee::mul(fx.mem.f(sp + 0x6C4), fx.mem.f(g + 0x20));
            fx.mem.setf(sp + 0x6C4, v);
        }
        m => {
            if m == 2 {
                let (u0, u1) = (fx.mem.i32(sp + 0x618), fx.mem.i32(sp + 0x61C));
                let h = u1.wrapping_sub(u0) >> 1;
                fx.mem.set32(sp + 0x618, u0.wrapping_add(h).wrapping_sub(1));
                fx.mem.set32(sp + 0x61C, u1.wrapping_sub(h).wrapping_add(1));
            }
            let (u0, u1, v0, v1) = (fx.mem.i32(sp + 0x618), fx.mem.i32(sp + 0x61C), fx.mem.i32(sp + 0x620), fx.mem.i32(sp + 0x624));
            let clamp = ((i64::from(u0) << 4) as u64 | 0xA) | ((i64::from(u1.wrapping_sub(1)) << 14) as u64) | ((i64::from(v0) << 24) as u64) | ((i64::from(v1.wrapping_sub(1)) << 34) as u64);
            fx.desc_clamp(d, clamp);
        }
    }
    let (mut ai, mut ao) = (fx.mem.u8(g + 0x27), fx.mem.u8(g + 0x2B));
    let elapsed = fx.mem.u8(g + 2) - fx.mem.u8(g);
    if fx.mem.u8(g + 0xC) & 8 != 0 && elapsed < 0x1F {
        let k = fx.mem.u8(g + 0x27) >> 5;
        ai = fx.mult(elapsed, k);
        ao = elapsed.wrapping_mul(fx.mem.u8(g + 0x2B) >> 5);
    }
    if fx.mem.u8(g + 0xC) & 0x10 != 0 {
        let c = fx.mem.u8(g);
        if c < 0x1F {
            let k = fx.mem.u8(g + 0x27) >> 5;
            ai = fx.mult(c, k);
            ao = c.wrapping_mul(fx.mem.u8(g + 0x2B) >> 5);
        }
    }
    for o in [0x653u32, 0x63B] {
        fx.mem.set8(sp + o, ai);
    }
    for o in [0x683u32, 0x66B] {
        fx.mem.set8(sp + o, ao);
    }
    let mut k = 0;
    while k < n {
        let step = fx.mem.u8(g + 0xE);
        segment_d(fx, at + 0x784, sp, ot, k as u32, tex, step, rec, a3);
        k += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn segment_d(fx: &mut Fx, fix_ra: u32, sp: u32, ot: u32, k: u32, tex: u32, step: i32, rec: i32, a3: &mut Option<u32>) {
    let d = sp + 0x628;
    let p = sp + 0x150 + 16 * k;
    let corners = [(p, 0x648u32, 0x600u32), (p + 16, 0x660, 0x604), (p + 0x250, 0x678, 0x608), (p + 0x260, 0x690, 0x60C)];
    for (v, xyz, w) in corners {
        let m = fx.read_m4(sp + 0x110);
        *a3 = Some(sp + w);
        if !fx.project_at(v, &m, sp + xyz, sp + w) {
            return;
        }
    }
    for o in [0x697u32, 0x67F, 0x667, 0x64F] {
        fx.mem.write_u8(sp + o, 0);
    }
    let w: [f32; 4] = std::array::from_fn(|i| fx.mem.f(sp + 0x600 + 4 * i as u32));
    for (i, o) in [0x63Cu32, 0x654, 0x66C, 0x684].into_iter().enumerate() {
        fx.mem.setf(sp + o, ee::div(1.0, w[i]));
    }
    let (cw, ch) = (fx.mem.f(sp + 0x6C4), fx.mem.f(sp + 0x6C8));
    let s0 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x618)), cw);
    let t0 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x620)), ch);
    let s1 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x61C)), cw);
    let t1 = ee::div(ee::from_i32(fx.mem.i32(sp + 0x624)), ch);
    for (o, s, i) in [(0x640u32, s0, 0usize), (0x644, t0, 0), (0x658, s1, 1), (0x65C, t0, 1), (0x670, s0, 2), (0x674, t1, 2), (0x688, s1, 3), (0x68C, t1, 3)] {
        fx.mem.setf(sp + o, ee::div(s, w[i]));
    }
    alpha_mode(fx, sp + 0x610, sp + 0x6CC);
    let s = |fx: &mut Fx, k: u32| u64::from(fx.mem.read_u8(sp + 0x6CC + k));
    let a = s(fx, 0) | (s(fx, 1) << 2) | (s(fx, 2) << 4) | (s(fx, 3) << 6);
    fx.desc_alpha(d, a);
    *a3 = Some(if fx.mem.read_u16(d + 2) & 0x20 != 0 && tex != 0 { fx.mem.read_u32(tex + 0x30) } else { d + 0x10 });
    fx.desc_texture(d, tex, 1);
    let m = fx.mult(step, rec + 1);
    let depth = ee::add(fx.mem.f(sp + 0x6C0), ee::from_i32(m));
    let key = ee::fixunssfsi(depth) as i32;
    fx.fix_residue(depth, fix_ra);
    fx.submit(d, ot, key);
}
