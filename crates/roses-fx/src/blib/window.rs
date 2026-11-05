//! Window quads of the effect library: the frame
//! drawn back over a window rectangle, faded in and out.
use super::exe::alpha_mode;
use crate::bus::Bus;
use crate::ee;
use crate::exe::Fx;

/// 97df45561379 [7 B records; b173 0x1E04780, 312 words] [F]:
/// `frame_fade(at, p, ot, n) -> 1`: while the update `n` lies in [u16
/// +0, u16 +2], a textured quad (a 0x140 frame, the descriptor at sp +
/// 0xA8: RGBAQ / ST / XYZF2 x 4) of the frame drawn (0x207EF0, filter 1)
/// over the window corners (28672 / 36864 in 12.4) moved by the i32 x 16
/// at +0xC.. (x0, x1, x2, x3, y0, y1, y2, y3; 0x1A2C70), UV 0 / 0x2000,
/// colour +0x2E / +0x2D / +0x2C with alpha 16 per update for +5 updates
/// in, 8 x (u8 +2 - n) in the last 2 x +5, else 16 x +5; the blend
/// +6's (0x101C80 / 0x207E20), flags | 0x800; z 0xFFFFFE on vertex 4 and
/// 2 (and 1 from it: the original copies 2 onto itself, vertex 3 keeps
/// its z); keyed by +8 (0x1A2C70). `a3`: the caller's a3, reported with
/// 0x207EF0's call.
pub fn frame_fade(fx: &mut Fx, at: u32, p: u32, ot: u32, n: i32, a3: u32) -> i32 {
    frame_fade_with(fx, at, p, ot, n, a3, 0x140, 0, &[0x324, 0x354, 0x384, 0x3B4, 0x3E4, 0x414, 0x444, 0x474, 0x484])
}

/// fc1a53984c3f [10 B records; card 219's 0x1E04C50, 288 words]
/// [F]: [`frame_fade`] compiled with filled delay slots (the
/// same 0x140 frame and locals), its conversions' `jal`s at +0x308 ..
/// +0x420 and the key's at +0x42C.
pub fn frame_fade_b(fx: &mut Fx, at: u32, p: u32, ot: u32, n: i32, a3: u32) -> i32 {
    frame_fade_with(fx, at, p, ot, n, a3, 0x140, 0, &[0x308, 0x330, 0x358, 0x380, 0x3A8, 0x3D0, 0x3F8, 0x420, 0x42C])
}

/// 7e28b625d220 [1 B record; card 653's 0x1E03A10, 340 words] (helper
/// A17) [F]: [`frame_fade`] compiled with a 0xF0 frame (only s0 .. s2
/// saved: p / n / ot, every local 0x50 lower = the same caller-relative
/// addresses) and its conversions' `jal`s at +0x3B4 .. +0x504, the key's
/// at +0x514.
pub fn frame_fade_c(fx: &mut Fx, at: u32, p: u32, ot: u32, n: i32, a3: u32) -> i32 {
    frame_fade_with(fx, at, p, ot, n, a3, 0xF0, 0u32.wrapping_sub(0x50), &[0x3B4, 0x3E4, 0x414, 0x444, 0x474, 0x4A4, 0x4D4, 0x504, 0x514])
}

/// [`frame_fade`] with a `frame` of its own (`sp + shift` standing for the
/// 0x140 frame's sp) and the conversions' `jal` offsets `jals` (the eight
/// corners', then the key's).
fn frame_fade_with(fx: &mut Fx, at: u32, p: u32, ot: u32, n: i32, a3: u32, frame: u32, shift: u32, jals: &[u32; 9]) -> i32 {
    let sp = fx.enter(frame).wrapping_add(shift);
    let s0 = n & 0xFFFF;
    if s0 < fx.mem.u16(p) || fx.mem.u16(p + 2) < s0 {
        fx.leave(frame);
        return 1;
    }
    let d = sp + 0xA8;
    fx.desc_init(d, 0xC, 0x34, 0x4214_2142_1421);
    fx.frame_desc(d, 1, [n as u32, a3]);
    for k in 0..3u32 {
        let v = fx.mem.read_u8(p + 0x2E - k);
        for o in [0x100u32, 0xE8, 0xD0, 0xB8] {
            fx.mem.write_u8(sp + o + k, v);
        }
    }
    for o in [0x104u32, 0xEC, 0xD4, 0xBC] {
        fx.mem.write_u32(sp + o, 0x3F80_0000);
    }
    fx.set_xyz_z(sp + 0x110, 0xFF_FFFE);
    let z = fx.xyz_z(sp + 0x110);
    fx.set_xyz_z(sp + 0xE0, z);
    let z = fx.xyz_z(sp + 0xE0);
    fx.set_xyz_z(sp + 0xC8, z);
    for o in [0x117u32, 0xFF, 0xE7, 0xCF] {
        fx.mem.write_u8(sp + o, 0);
    }
    let fade = fx.mem.u8(p + 5);
    let mut a = (fade << 4) & 0xFF;
    let into = s0 - fx.mem.u16(p);
    if into < fade {
        a = (into << 4) & 0xFF;
    }
    if fx.mem.u16(p + 2) - s0 < 2 * fade {
        a = ((fx.mem.u8(p + 2) - n) << 3) & 0xFF;
    }
    for o in [0x103u32, 0xEB, 0xD3, 0xBB] {
        fx.mem.set8(sp + o, a);
    }
    let b = fx.mem.read_u8(p + 6);
    fx.mem.write_u8(sp + 0x95, b);
    alpha_mode(fx, sp + 0x90, sp + 0x13C);
    let sel = |k: u32| u64::from(k);
    let alpha = sel(u32::from(fx.mem.read_u8(sp + 0x13C)))
        | (sel(u32::from(fx.mem.read_u8(sp + 0x13D))) << 2)
        | (sel(u32::from(fx.mem.read_u8(sp + 0x13E))) << 4)
        | (sel(u32::from(fx.mem.read_u8(sp + 0x13F))) << 6);
    fx.desc_alpha(d, alpha);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    for (o, v) in [(0xC0u32, 0u16), (0xC2, 0), (0xD8, 0x2000), (0xDA, 0), (0xF0, 0), (0xF2, 0x2000), (0x108, 0x2000), (0x10A, 0x2000)] {
        let h = fx.mem.read_u16(sp + o);
        fx.mem.write_u16(sp + o, (h & 0xC000) | v);
    }
    for (k, (dst, base)) in [(0xC8u32, 28672.0f32), (0xE0, 36864.0), (0xF8, 28672.0), (0x110, 36864.0), (0xCA, 28672.0), (0xE2, 28672.0), (0xFA, 36864.0), (0x112, 36864.0)].into_iter().enumerate() {
        let off = jals[k];
        let o = fx.mem.i32(p + 0xC + 4 * k as u32) << 4;
        let v = ee::add(base, ee::from_i32(o));
        fx.fix_residue(v, at + off + 8);
        fx.mem.write_u16(sp + dst, ee::fixunssfsi(v) as u16);
    }
    let w = fx.mem.f(p + 8);
    fx.fix_residue(w, at + jals[8] + 8);
    let key = ee::fixunssfsi(w) as i32;
    fx.submit(d, ot, key);
    fx.leave(frame);
    1
}

/// The stack layout of one compile of the frame-ray routine: every local
/// at `base` + its 8996ae7f8dd9 offset - 0x130 (the descriptor at `base`),
/// the loop count / OT / vertex-pointer slots, the caller's f20 / f21
/// saves, the 0x1A2C70 call offsets (the two window-coordinate
/// conversions, then two per triangle case) and the a2 / a3 each case
/// leaves.
pub struct RayLayout {
    pub frame: u32,
    pub base: u32,
    /// The loop count's slot and the counter's at sp + 0xB0 (`None` /
    /// `false`: in registers).
    pub n_slot: Option<u32>,
    pub counter_slot: bool,
    pub ot_slot: Option<u32>,
    /// A slot holding the pointer to the centre vertex's Y (sp + base + 0x4A).
    pub ptr_slot: Option<u32>,
    /// f20 / f21 saved at sp + 0 / sp + 4 (`swc1`).
    pub fsave: bool,
    pub jals: [u32; 18],
    pub case_regs: [[u32; 2]; 8],
    /// The a2 / a3 the UV merges leave: the halfword read before the last
    /// merge and its pair's pointer, not
    /// `case_regs`.
    pub merge_regs: bool,
}

const M: u32 = 0xFFFF_C000;

/// 8996ae7f8dd9's layout (card 197's 0x1E015F0): a 0x1B0 frame, the
/// descriptor at sp + 0x130, f20 saved but not reproduced.
pub const RAYS: RayLayout = RayLayout {
    frame: 0x1B0,
    base: 0x130,
    n_slot: Some(0xCC),
    counter_slot: true,
    merge_regs: false,
    ot_slot: None,
    ptr_slot: None,
    fsave: false,
    jals: [0x124, 0x17C, 0x478, 0x494, 0x520, 0x538, 0x5D0, 0x5F4, 0x684, 0x6AC, 0x740, 0x75C, 0x7E8, 0x800, 0x898, 0x8BC, 0x94C, 0x974],
    case_regs: [[M, 0], [M, 0], [0, M], [M, 0x2000], [M, 0x2000], [M, 0x2000], [0x2000, M], [M, 0]],
};

/// c4d4d9862c69's layout (card 529's 0x1E058E0, 540 words; cards 200, 529,
/// 615, 682) [F]: the same routine compiled with a 0x1C0 frame, every
/// local 0x10 higher, the OT at sp + 0xD8, the centre vertex's Y pointer
/// at sp + 0xC0, the caller's f20 / f21 at sp + 0 / + 4, and the
/// conversions scheduled into the stores (the same values, the same
/// order).
pub const RAYS_B: RayLayout = RayLayout {
    frame: 0x1C0,
    base: 0x140,
    n_slot: Some(0xD4),
    counter_slot: true,
    merge_regs: false,
    ot_slot: Some(0xD8),
    ptr_slot: Some(0xC0),
    fsave: true,
    jals: [0x12C, 0x174, 0x464, 0x480, 0x4DC, 0x4F4, 0x560, 0x570, 0x5D4, 0x5E8, 0x64C, 0x668, 0x6C4, 0x6DC, 0x748, 0x758, 0x7BC, 0x7D0],
    case_regs: [[M, 0], [M, 0], [0, M], [0, M], [M, 0x2000], [M, 0x2000], [0x2000, M], [0x2000, M]],
};

/// b1e33d639b16's layout (card 153's 0x1E02B70, 740 words; cards 14 /
/// 153) [F]: the routine in a 0x170 frame, the
/// descriptor at sp + 0xF0 (every local 0x40 lower), n / the OT / the
/// counter in registers (no slots: the old ones would be this frame's
/// saves), the x / y conversions unscheduled, each UV merge leaving the
/// halfword it read in a2 and its pointer in a3.
pub const RAYS_C: RayLayout = RayLayout {
    frame: 0x170,
    base: 0xF0,
    n_slot: None,
    counter_slot: false,
    merge_regs: true,
    ot_slot: None,
    ptr_slot: None,
    fsave: false,
    jals: [0x11C, 0x174, 0x4C8, 0x500, 0x5A4, 0x5D8, 0x688, 0x6B4, 0x75C, 0x78C, 0x838, 0x870, 0x914, 0x948, 0x9F8, 0xA24, 0xACC, 0xAFC],
    case_regs: [[0, 0]; 8],
};

/// b1e33d639b16 [2 B records; card 153's 0x1E02B70, 740 words] (helper
/// A19) [F]: [`frame_rays`] compiled on [`RAYS_C`].
pub fn frame_rays_c(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> i32 {
    frame_rays_with(fx, at, recs, ot, n, a3, &RAYS_C, [0, 0]).0
}

/// [`frame_rays_c`] and the a3 it leaves.
pub fn frame_rays_c_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> (i32, u32) {
    frame_rays_with(fx, at, recs, ot, n, a3, &RAYS_C, [0, 0])
}

/// 8996ae7f8dd9 [5 B records; card 197's 0x1E015F0, 648 words] (helper
/// D8) [F]: `frame_rays(at, recs, ot, n) -> 1` draws `n` records (0x30
/// bytes; a 0x1B0 frame, the descriptor at sp + 0x130: RGBAQ / UV / XYZ2 x
/// 3, the frame drawn (0x207EF0, filter 1) at its own window position, UV
/// = 12.4 window coordinate - 28672) as up to eight triangles from the
/// record's centre to the window's edges. Per record: countdown 0x101AF0;
/// on its first count the vec +0xC projected into +0x28 (0x20E2B0,
/// 0x204D80, w at sp + 0x1A8) and kept inside the window (0x101770), else
/// the count is cleared; colour +0x22 / +0x21 / +0x20 (alpha +0x23) at the
/// outer vertices, +0x1E / +0x1D / +0x1C (alpha +0x1F) at the centre,
/// alphas x count / 8 over the last 8; the far Z; the blend +9's
/// (0x101C80 / 0x207E20), flags | 0x800; the reach r = 16 (+8 + 2 (count
/// & 3)); triangle k (bit k of +0xA) to the window edge (28672 - r or
/// 36864 + r): 0 left-mid / top-left, 1 top-left / top-mid, 2 top-mid /
/// top-right, 3 top-right / right-mid, 4 right-mid / bottom-right, 5
/// bottom-right / bottom-mid, 6 bottom-mid / bottom-left, 7 bottom-left /
/// left-mid; key 0. `a3`: the caller's a3, reported with the first
/// 0x207EF0 call.
pub fn frame_rays(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> i32 {
    frame_rays_with(fx, at, recs, ot, n, a3, &RAYS, [0, 0]).0
}

/// [`frame_rays`] and the a3 it leaves (the projection's w pointer sp +
/// 0x1A8 after a first count, the centre vertex's XYZ pointer sp + 0x178
/// after the descriptor, then the last triangle's constant).
pub fn frame_rays_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32) -> (i32, u32) {
    frame_rays_with(fx, at, recs, ot, n, a3, &RAYS, [0, 0])
}

/// c4d4d9862c69 [4 B records; card 529's 0x1E058E0, 540 words] (helper
/// B15) [F]: [`frame_rays`] compiled on [`RAYS_B`]; `f2021`: the caller's
/// f20 / f21 bits (saved at sp + 0 / + 4).
pub fn frame_rays_b(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32, f2021: [u32; 2]) -> i32 {
    frame_rays_with(fx, at, recs, ot, n, a3, &RAYS_B, f2021).0
}

/// [`frame_rays_b`] and the a3 it leaves.
pub fn frame_rays_b_a3(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32, f2021: [u32; 2]) -> (i32, u32) {
    frame_rays_with(fx, at, recs, ot, n, a3, &RAYS_B, f2021)
}

fn frame_rays_with(fx: &mut Fx, at: u32, recs: u32, ot: u32, n: i32, a3: u32, l: &RayLayout, f2021: [u32; 2]) -> (i32, u32) {
    let sp = fx.enter(l.frame);
    if let Some(o) = l.n_slot {
        fx.mem.set32(sp + o, n);
    }
    if let Some(o) = l.ot_slot {
        fx.mem.write_u32(sp + o, ot);
    }
    if l.fsave {
        fx.mem.write_u32(sp, f2021[0]);
        fx.mem.write_u32(sp + 4, f2021[1]);
    }
    let mut a23 = [n as u32, a3];
    if l.counter_slot {
        fx.mem.set32(sp + 0xB0, 0);
    }
    if n > 0 {
        if let Some(o) = l.ptr_slot {
            fx.mem.write_u32(sp + o, sp + l.base + 0x4A);
        }
        let mut g = recs;
        let mut i = 0;
        while i < n {
            if i != 0 {
                g += 0x30;
            }
            if l.counter_slot {
                fx.mem.set32(sp + 0xB0, i);
            }
            frame_ray(fx, at, sp, g, ot, &mut a23, l);
            i += 1;
        }
        if l.counter_slot {
            fx.mem.set32(sp + 0xB0, i);
        }
    }
    fx.leave(l.frame);
    (1, a23[1])
}

/// The window coordinate `v` (12.4) as the u16 the code stores (0x1A2C70,
/// its residue left; `jal` at `at` + `off`).
fn fix16(fx: &mut Fx, at: u32, off: u32, v: f32) -> u16 {
    fx.fix_residue(v, at + off + 8);
    ee::fixunssfsi(v) as u16
}

/// A coordinate half: the low 14 bits of `v` into the u16 at `a`.
fn uv14(fx: &mut Fx, a: u32, v: i32) {
    let h = fx.mem.read_u16(a);
    fx.mem.write_u16(a, (h & 0xC000) | (v & 0x3FFF) as u16);
}

fn frame_ray(fx: &mut Fx, at: u32, sp: u32, g: u32, ot: u32, a23: &mut [u32; 2], l: &RayLayout) {
    if super::exe::countdown(fx, g) == 0 {
        return;
    }
    let b = sp + l.base;
    if fx.mem.u8(g) == fx.mem.u8(g + 2) - 1 {
        super::exe::camera_vp_into(fx, b - 0x60);
        let m = fx.read_m4(b - 0x60);
        let ok = fx.project_at(g + 0xC, &m, g + 0x28, b + 0x78);
        *a23 = [sp.wrapping_sub(0x10), b + 0x78];
        if !ok {
            fx.mem.write_u8(g, 0);
            return;
        }
        let inside = |fx: &mut Fx, a: u32| {
            let f = ee::from_i32(fx.mem.u16(a));
            !crate::exe::ee_lt(f, 28672.0) && crate::exe::ee_le(f, 36864.0)
        };
        if !inside(fx, g + 0x28) || !inside(fx, g + 0x2A) {
            fx.mem.write_u8(g, 0);
            return;
        }
    }
    let f = ee::sub(ee::from_i32(fx.mem.u16(g + 0x28)), 28672.0);
    fx.fix_residue(f, at + l.jals[0] + 8);
    let s1 = ee::fixsfsi(f);
    let f = ee::sub(ee::from_i32(fx.mem.u16(g + 0x2A)), 28672.0);
    fx.fix_residue(f, at + l.jals[1] + 8);
    let s2 = ee::fixsfsi(f);
    let d = b;
    fx.desc_init(d, 9, 0x33, 0x4_2142_1421);
    fx.frame_desc(d, 1, *a23);
    for (k, (outer, inner)) in [(0x22u32, 0x1Eu32), (0x21, 0x1D), (0x20, 0x1C)].into_iter().enumerate() {
        let k = k as u32;
        let v = fx.mem.read_u8(g + outer);
        fx.mem.write_u8(b + 0x28 + k, v);
        fx.mem.write_u8(b + 0x10 + k, v);
        let v = fx.mem.read_u8(g + inner);
        fx.mem.write_u8(b + 0x40 + k, v);
    }
    for o in [0x44u32, 0x2C, 0x14] {
        fx.mem.write_u32(b + o, 0x3F80_0000);
    }
    fx.set_xyz_z(b + 0x50, 0xFF_FFFF);
    let z = fx.xyz_z(b + 0x50);
    fx.set_xyz_z(b + 0x38, z);
    let z = fx.xyz_z(b + 0x38);
    fx.set_xyz_z(b + 0x20, z);
    for o in [0x57u32, 0x3F, 0x27] {
        fx.mem.write_u8(b + o, 0);
    }
    let v = fx.mem.read_u8(g + 9);
    fx.mem.write_u8(b - 0x1B, v);
    alpha_mode(fx, b - 0x20, b + 0x7C);
    let sel = |fx: &mut Fx, o: u32| u64::from(fx.mem.read_u8(b + o));
    let alpha = sel(fx, 0x7C) | (sel(fx, 0x7D) << 2) | (sel(fx, 0x7E) << 4) | (sel(fx, 0x7F) << 6);
    fx.desc_alpha(d, alpha);
    let f = fx.mem.read_u16(d + 2);
    fx.mem.write_u16(d + 2, f | 0x800);
    // a2 / a3 after the centre vertex: the mask and its XYZ pointer (RAYS)
    // or the masked x and the mask (RAYS_B).
    *a23 = if l.ptr_slot.is_some() { [(s1 & 0x3FFF) as u32, M] } else { [M, b + 0x48] };
    uv14(fx, b + 0x48, s1);
    if l.merge_regs {
        *a23 = [u32::from(fx.mem.read_u16(b + 0x4A)), b + 0x48];
    }
    uv14(fx, b + 0x4A, s2);
    let (cx, cy) = (fx.mem.read_u16(g + 0x28), fx.mem.read_u16(g + 0x2A));
    fx.mem.write_u16(b + 0x50, cx);
    fx.mem.write_u16(b + 0x52, cy);
    let c = fx.mem.u8(g);
    if c < 8 {
        let k = fx.mem.u8(g + 0x23) >> 3;
        let v = fx.mult(k, c);
        fx.mem.set8(b + 0x2B, v);
        fx.mem.set8(b + 0x13, v);
        let k = fx.mem.u8(g + 0x1F) >> 3;
        let c = fx.mem.u8(g);
        // mult1 (pipeline 1): LO / HI untouched.
        let v = k.wrapping_mul(c);
        fx.mem.set8(b + 0x43, v);
    } else {
        let v = fx.mem.read_u8(g + 0x23);
        fx.mem.write_u8(b + 0x2B, v);
        fx.mem.write_u8(b + 0x13, v);
        let v = fx.mem.read_u8(g + 0x1F);
        fx.mem.write_u8(b + 0x43, v);
    }
    let reach = fx.mem.u8(g + 8) + ((fx.mem.u8(g) & 3) << 1);
    let r = ee::from_i32(reach << 4);
    let lo = ee::sub(28672.0, r);
    let hi = ee::add(36864.0, r);
    let (v1x, v1y, v2x, v2y) = (b + 0x20, b + 0x22, b + 0x38, b + 0x3A);
    let mut s3 = 0u32;
    while s3 < 8 {
        if fx.mem.u8(g + 0xA) & (1 << s3) != 0 {
            // (v1 X, v1 Y, v2 X, v2 Y).
            let xy: [i32; 4] = match s3 {
                0 => [0, s2, 0, 0],
                1 => [0, 0, s1, 0],
                2 => [s1, 0, 0x2000, 0],
                3 => [0x2000, 0, 0x2000, s2],
                4 => [0x2000, s2, 0x2000, 0x2000],
                5 => [0x2000, 0x2000, s1, 0x2000],
                6 => [s1, 0x2000, 0, 0x2000],
                _ => [0, 0x2000, 0, s2],
            };
            *a23 = l.case_regs[s3 as usize];
            for (k, o) in [0x18u32, 0x1A, 0x30, 0x32].into_iter().enumerate() {
                if l.merge_regs && o == 0x32 {
                    *a23 = [u32::from(fx.mem.read_u16(b + o)), b + 0x30];
                }
                uv14(fx, b + o, xy[k]);
            }
            let j = &l.jals[2 + 2 * s3 as usize..];
            match s3 {
                0 => {
                    let v = fix16(fx, at, j[0], lo);
                    fx.mem.write_u16(v2x, v);
                    fx.mem.write_u16(v1x, v);
                    fx.mem.write_u16(v1y, cy);
                    let v = fix16(fx, at, j[1], lo);
                    fx.mem.write_u16(v2y, v);
                }
                1 => {
                    let v = fix16(fx, at, j[0], lo);
                    fx.mem.write_u16(v1x, v);
                    fx.mem.write_u16(v2x, cx);
                    let v = fix16(fx, at, j[1], lo);
                    fx.mem.write_u16(v2y, v);
                    fx.mem.write_u16(v1y, v);
                }
                2 => {
                    fx.mem.write_u16(v1x, cx);
                    let v = fix16(fx, at, j[0], hi);
                    fx.mem.write_u16(v2x, v);
                    let v = fix16(fx, at, j[1], lo);
                    fx.mem.write_u16(v2y, v);
                    fx.mem.write_u16(v1y, v);
                }
                3 => {
                    let v = fix16(fx, at, j[0], hi);
                    fx.mem.write_u16(v2x, v);
                    fx.mem.write_u16(v1x, v);
                    let v = fix16(fx, at, j[1], lo);
                    fx.mem.write_u16(v1y, v);
                    fx.mem.write_u16(v2y, cy);
                }
                4 => {
                    let v = fix16(fx, at, j[0], hi);
                    fx.mem.write_u16(v2x, v);
                    fx.mem.write_u16(v1x, v);
                    fx.mem.write_u16(v1y, cy);
                    let v = fix16(fx, at, j[1], hi);
                    fx.mem.write_u16(v2y, v);
                }
                5 => {
                    let v = fix16(fx, at, j[0], hi);
                    fx.mem.write_u16(v1x, v);
                    fx.mem.write_u16(v2x, cx);
                    let v = fix16(fx, at, j[1], hi);
                    fx.mem.write_u16(v2y, v);
                    fx.mem.write_u16(v1y, v);
                }
                6 => {
                    fx.mem.write_u16(v1x, cx);
                    let v = fix16(fx, at, j[0], lo);
                    fx.mem.write_u16(v2x, v);
                    let v = fix16(fx, at, j[1], hi);
                    fx.mem.write_u16(v2y, v);
                    fx.mem.write_u16(v1y, v);
                }
                _ => {
                    let v = fix16(fx, at, j[0], lo);
                    fx.mem.write_u16(v2x, v);
                    fx.mem.write_u16(v1x, v);
                    let v = fix16(fx, at, j[1], hi);
                    fx.mem.write_u16(v1y, v);
                    fx.mem.write_u16(v2y, cy);
                }
            }
            fx.submit(d, ot, 0);
            a23[0] = 0;
        }
        s3 += 1;
    }
}

/// d10bfc19ceda [2 B records; b379 (card 2) 0x1E02130, 96 words] (helper
/// B18) [F]: `flat(at, p, ot, n) -> 1`: a packet layer (0x2078A0, opened
/// on every call) holding, while the update `n` lies in [u16 +0, u16 +2],
/// a window sprite (0x6FF8 .. 0x8FF8, Z 0xFFFFFF / 0xFFFFFE; a 0x80 frame,
/// the descriptor at sp + 0x50: RGBAQ / XYZF2 x 2) of colour +0xE / +0xD /
/// +0xC, alpha +0xF, ALPHA preset 3 (0x208450), keyed by +8 (0x1A2C70 returning to
/// +0x134), no ordering table; the layer closed into `ot` at key 0
/// (0x207910) only then (b398's own window compiled apart).
pub fn flat(fx: &mut Fx, at: u32, p: u32, ot: u32, n: i32) -> i32 {
    let sp = fx.enter(0x80);
    let layer = fx.layer_begin();
    let s = n & 0xFFFF;
    if s < fx.mem.u16(p) || fx.mem.u16(p + 2) < s {
        fx.leave(0x80);
        return 1;
    }
    let d = sp + 0x50;
    fx.mem.write_u16(d + 2, 6);
    fx.mem.write_u16(d, 3);
    fx.mem.write_u64(d + 8, 0x441);
    fx.mem.write_u32(d + 4, 0);
    for (o, src) in [(0x60u32, 0xEu32), (0x61, 0xD), (0x62, 0xC), (0x63, 0xF)] {
        let v = fx.mem.read_u8(p + src);
        fx.mem.write_u8(sp + o, v);
    }
    fx.mem.write_u32(sp + 0x64, 0x3F80_0000);
    for (o, xy, z) in [(0x68u32, 0x6FF8u16, 0xFF_FFFFu64), (0x70, 0x8FF8, 0xFF_FFFE)] {
        fx.mem.write_u16(sp + o, xy);
        fx.mem.write_u16(sp + o + 2, xy);
        fx.set_xyz_z(sp + o, z);
        fx.mem.write_u8(sp + o + 7, 0);
    }
    let alpha = fx.alpha_preset(3);
    fx.desc_alpha(d, alpha);
    let w = fx.mem.f(p + 8);
    fx.fix_residue(w, at + 0x134);
    fx.submit(d, 0, ee::fixunssfsi(w) as i32);
    fx.layer_end(layer, ot, 0);
    fx.leave(0x80);
    1
}
